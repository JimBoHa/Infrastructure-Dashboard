use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, ValueEnum};
use core_server_rs::services::analysis::lake::{read_replication_state, AnalysisLakeConfig};
use core_server_rs::services::analysis::parquet_duckdb::DuckDbQueryService;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::fs;
use std::path::PathBuf;
use std::time::Duration as StdDuration;

const DEFAULT_SETUP_CONFIG_PATH: &str = "/Users/Shared/FarmDashboard/setup/config.json";
const DEFAULT_HOT_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/lake/hot";
const DEFAULT_TMP_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/tmp";

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Direct,
    Api,
}

#[derive(Debug, Parser)]
#[command(about = "Spot-check Postgres metrics vs Parquet lake parity for a few sensors/windows.")]
struct Args {
    /// Mode: direct filesystem access (requires lake file permissions) or controller API job.
    #[arg(long, value_enum, default_value_t = Mode::Direct)]
    mode: Mode,

    /// Core-server base URL used when --mode=api.
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    api_base_url: String,

    /// API token string (Bearer). Used when --mode=api.
    #[arg(long)]
    auth_token: Option<String>,

    /// File containing a single-line API token string (Bearer). Used when --mode=api.
    #[arg(long)]
    auth_token_file: Option<PathBuf>,

    /// Poll interval while waiting for the parity job (ms). Used when --mode=api.
    #[arg(long, default_value_t = 300)]
    poll_interval_ms: u64,

    /// Setup config path (used to read database_url when --database-url is not provided).
    #[arg(long, default_value = DEFAULT_SETUP_CONFIG_PATH)]
    config: String,

    /// Postgres connection string (overrides setup config).
    #[arg(long)]
    database_url: Option<String>,

    #[arg(long)]
    hot_path: Option<PathBuf>,

    #[arg(long)]
    cold_path: Option<PathBuf>,

    #[arg(long)]
    tmp_path: Option<PathBuf>,

    /// Number of shards (must match CORE_ANALYSIS_LAKE_SHARDS).
    #[arg(long, default_value_t = 16)]
    shards: u32,

    /// RFC3339 start timestamp (inclusive).
    #[arg(long)]
    start: String,

    /// RFC3339 end timestamp (inclusive).
    #[arg(long)]
    end: String,

    /// Comma-separated sensor IDs. If omitted, the tool picks the first N sensors.
    #[arg(long)]
    sensor_ids: Option<String>,

    /// Number of sensors to check when --sensor-ids is omitted.
    #[arg(long, default_value_t = 5)]
    sample: u32,

    /// Output report path (Markdown). Recommended under reports/. If omitted, prints to stdout only.
    #[arg(long)]
    report: Option<PathBuf>,

    /// Fail if any mismatches are found (non-zero exit).
    #[arg(long, default_value_t = false)]
    fail_on_mismatch: bool,
}

#[derive(Debug, serde::Deserialize)]
struct ConfigFile {
    database_url: Option<String>,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn read_database_url(args: &Args) -> Result<String> {
    if let Some(url) = args
        .database_url
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Ok(url.to_string());
    }
    let contents = fs::read_to_string(&args.config)
        .with_context(|| format!("failed to read {}", args.config))?;
    let cfg: ConfigFile =
        serde_json::from_str(&contents).context("failed to parse setup config JSON")?;
    cfg.database_url
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .context("database_url missing in setup config; pass --database-url explicitly")
}

fn parse_ts(label: &str, value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim())
        .with_context(|| format!("invalid {} timestamp: {}", label, value))?
        .with_timezone(&Utc)
        .pipe(Ok)
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

fn read_api_token(args: &Args) -> Result<String> {
    if let Some(token) = args
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Ok(token.to_string());
    }
    if let Some(path) = args.auth_token_file.as_ref() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let token = raw.lines().next().unwrap_or("").trim();
        anyhow::ensure!(
            !token.is_empty(),
            "auth token file is empty: {}",
            path.display()
        );
        return Ok(token.to_string());
    }
    anyhow::bail!("missing auth token (provide --auth-token or --auth-token-file)");
}

fn api_headers(token: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token)).context("invalid auth token")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}

fn parse_sensor_ids_csv(raw: Option<&str>) -> Vec<String> {
    let mut ids: Vec<String> = raw
        .unwrap_or("")
        .split(',')
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

async fn pick_sensor_ids(db: &PgPool, args: &Args) -> Result<Vec<String>> {
    if let Some(raw) = args.sensor_ids.as_deref() {
        let ids = parse_sensor_ids_csv(Some(raw));
        anyhow::ensure!(!ids.is_empty(), "sensor_ids cannot be empty");
        return Ok(ids);
    }

    let limit = args.sample.max(1) as i64;
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE deleted_at IS NULL
        ORDER BY sensor_id
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(db)
    .await
    .context("failed to pick sensors from db")?;

    anyhow::ensure!(
        !rows.is_empty(),
        "no sensors available (provide --sensor-ids explicitly)"
    );
    Ok(rows)
}

#[derive(Debug, Clone)]
struct Counts {
    count: i64,
}

async fn pg_counts(
    db: &PgPool,
    sensor_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Counts> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT
            count(*)::bigint as count
        FROM metrics
        WHERE sensor_id = $1
          AND ts >= $2
          AND ts <= $3
        "#,
    )
    .bind(sensor_id)
    .bind(start)
    .bind(end)
    .fetch_one(db)
    .await
    .with_context(|| format!("postgres query failed for sensor_id={}", sensor_id))?;

    Ok(Counts { count })
}

async fn run_direct(args: Args) -> Result<()> {
    let database_url = read_database_url(&args)?;
    let db = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .with_context(|| format!("failed to connect to db at {}", database_url))?;

    let hot_path = args
        .hot_path
        .clone()
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_HOT_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HOT_PATH));
    let cold_path = args
        .cold_path
        .clone()
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_COLD_PATH"));
    let tmp_path = args
        .tmp_path
        .clone()
        .or_else(|| env_path("CORE_ANALYSIS_TMP_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TMP_PATH));

    let start = parse_ts("start", &args.start)?;
    let end_inclusive = parse_ts("end", &args.end)?;
    anyhow::ensure!(end_inclusive > start, "end must be after start");

    let lake = AnalysisLakeConfig {
        hot_path,
        cold_path,
        tmp_path: tmp_path.clone(),
        shards: args.shards.max(1),
        hot_retention_days: 90,
        late_window_hours: 48,
        replication_interval: StdDuration::from_secs(60),
        replication_lag: StdDuration::from_secs(300),
    };

    let replication = read_replication_state(&lake).unwrap_or_default();
    let computed_through = replication
        .computed_through_ts
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts.trim()).ok())
        .map(|ts| ts.with_timezone(&Utc));

    let effective_end = computed_through
        .map(|ct| std::cmp::min(ct, end_inclusive))
        .unwrap_or(end_inclusive);
    let end = effective_end + Duration::microseconds(1);

    let sensor_ids = pick_sensor_ids(&db, &args).await?;
    let duckdb = DuckDbQueryService::new(tmp_path, 1);

    let mut mismatches: u64 = 0;
    let mut lines: Vec<String> = Vec::new();
    lines.push("# TSSE Lake Parity Check\n".to_string());
    lines.push(format!("- Date: {}\n", Utc::now().to_rfc3339()));
    lines.push(format!(
        "- Window (requested): `{}` → `{}`\n",
        start.to_rfc3339(),
        end_inclusive.to_rfc3339()
    ));
    lines.push(format!(
        "- Window (checked): `{}` → `{}`\n",
        start.to_rfc3339(),
        effective_end.to_rfc3339()
    ));
    lines.push(format!(
        "- computed_through_ts: `{}`\n",
        computed_through
            .map(|v| v.to_rfc3339())
            .unwrap_or_else(|| "—".to_string())
    ));
    lines.push(
        "\n| Sensor | Postgres count | Lake count | Match |\n| --- | ---: | ---: | :---: |\n"
            .to_string(),
    );

    for sensor_id in sensor_ids.iter() {
        let pg = pg_counts(&db, sensor_id, start, effective_end).await?;
        let lake_rows = duckdb
            .read_metrics_points_from_lake(&lake, start, end, vec![sensor_id.clone()], None)
            .await
            .with_context(|| format!("duckdb read failed for sensor_id={}", sensor_id))?;
        let lake_count = lake_rows.len() as i64;
        let ok = pg.count == lake_count;
        if !ok {
            mismatches += 1;
        }
        lines.push(format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            sensor_id,
            pg.count,
            lake_count,
            if ok { "OK" } else { "MISMATCH" }
        ));
    }

    if mismatches > 0 {
        lines.push(format!("\nMismatches: `{}`\n", mismatches));
    } else {
        lines.push("\nMismatches: `0`\n".to_string());
    }

    let report_text = lines.join("");
    if let Some(path) = args.report.as_ref() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(path, &report_text)
            .with_context(|| format!("failed to write {}", path.display()))?;
        println!(
            "tsse_lake_parity_check: wrote report to {} (mismatches={})",
            path.display(),
            mismatches
        );
    } else {
        println!("{}", report_text);
    }

    if args.fail_on_mismatch && mismatches > 0 {
        anyhow::bail!("parity mismatches detected");
    }

    Ok(())
}

async fn run_api(args: Args) -> Result<()> {
    let token = read_api_token(&args)?;
    let headers = api_headers(&token)?;
    let base_url = args.api_base_url.trim_end_matches('/').to_string();
    let http = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()?;

    let sensor_ids = parse_sensor_ids_csv(args.sensor_ids.as_deref());
    let params = json!({
        "start": args.start,
        "end": args.end,
        "sensor_ids": sensor_ids,
        "sample": args.sample,
        "fail_on_mismatch": args.fail_on_mismatch,
    });
    let create_body = json!({
        "job_type": "lake_parity_check_v1",
        "params": params,
        "dedupe": false,
    });

    let created: serde_json::Value = http
        .post(format!("{}/api/analysis/jobs", base_url))
        .headers(headers.clone())
        .json(&create_body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("failed to parse create_job response")?;
    let job_id = created
        .get("job")
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .context("create_job response missing job.id")?
        .to_string();

    let mut completed = false;
    for _ in 0..2_000 {
        let status: serde_json::Value = http
            .get(format!("{}/api/analysis/jobs/{}", base_url, job_id))
            .headers(headers.clone())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("failed to parse get_job response")?;
        let state = status
            .get("job")
            .and_then(|v| v.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        match state {
            "completed" => {
                completed = true;
                break;
            }
            "failed" => anyhow::bail!("parity job failed (job_id={})", job_id),
            "canceled" => anyhow::bail!("parity job canceled (job_id={})", job_id),
            _ => {}
        }
        tokio::time::sleep(StdDuration::from_millis(args.poll_interval_ms.max(50))).await;
    }
    anyhow::ensure!(completed, "parity job timed out (job_id={})", job_id);

    let result: serde_json::Value = http
        .get(format!("{}/api/analysis/jobs/{}/result", base_url, job_id))
        .headers(headers.clone())
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
        .context("failed to parse get_job_result response")?;

    let report_markdown = result
        .get("result")
        .and_then(|v| v.get("report_markdown"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    anyhow::ensure!(
        !report_markdown.trim().is_empty(),
        "job result missing report_markdown (job_id={})",
        job_id
    );
    let mismatches = result
        .get("result")
        .and_then(|v| v.get("mismatches"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if let Some(path) = args.report.as_ref() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(path, &report_markdown)
            .with_context(|| format!("failed to write {}", path.display()))?;
        println!(
            "tsse_lake_parity_check: wrote report to {} (mismatches={}, job_id={})",
            path.display(),
            mismatches,
            job_id
        );
    } else {
        println!("{}", report_markdown);
    }

    if args.fail_on_mismatch && mismatches > 0 {
        anyhow::bail!("parity mismatches detected (job_id={})", job_id);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    match args.mode {
        Mode::Direct => run_direct(args).await,
        Mode::Api => run_api(args).await,
    }
}
