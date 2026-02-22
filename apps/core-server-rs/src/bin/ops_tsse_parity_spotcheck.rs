use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, Utc};
use clap::Parser;
use duckdb::Connection;
use serde::Deserialize;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use url::Url;
use xxhash_rust::xxh3::xxh3_64;

const DEFAULT_CONFIG_PATH: &str = "/Users/Shared/FarmDashboard/setup/config.json";
const DEFAULT_DATA_ROOT: &str = "/Users/Shared/FarmDashboard";
const DEFAULT_SHARDS: u32 = 16;
const DEFAULT_LAG_SECONDS: i64 = 300;
const METRICS_DATASET_V1: &str = "metrics/v1";

#[derive(Debug, Parser)]
#[command(about = "Ops tool: spot-check Postgres<->Parquet parity for small sensor windows.")]
struct Args {
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: String,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long)]
    data_root: Option<PathBuf>,
    #[arg(long)]
    hot_path: Option<PathBuf>,
    #[arg(long)]
    cold_path: Option<PathBuf>,
    #[arg(long)]
    tmp_path: Option<PathBuf>,
    #[arg(long)]
    shards: Option<u32>,
    #[arg(long = "sensor-id", value_delimiter = ',', num_args = 1..)]
    sensor_ids: Vec<String>,
    #[arg(long)]
    start: Option<String>,
    #[arg(long)]
    end: Option<String>,
    #[arg(long, default_value_t = 60)]
    window_minutes: i64,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    database_url: Option<String>,
    #[serde(default)]
    data_root: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ReplicationState {
    #[serde(default)]
    computed_through_ts: Option<String>,
    #[serde(default)]
    last_inserted_at: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct LakeManifest {
    #[serde(default)]
    datasets: BTreeMap<String, DatasetManifest>,
}

#[derive(Debug, Deserialize, Default)]
struct DatasetManifest {
    #[serde(default)]
    partitions: BTreeMap<String, PartitionManifest>,
}

#[derive(Debug, Deserialize, Default)]
struct PartitionManifest {
    #[serde(default)]
    location: String,
}

#[derive(Debug, Clone, Default)]
struct AggStats {
    count: i64,
    min_ts: Option<DateTime<Utc>>,
    max_ts: Option<DateTime<Utc>>,
    min_value: Option<f64>,
    max_value: Option<f64>,
    avg_value: Option<f64>,
    min_quality: Option<i32>,
    max_quality: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct PgAggRow {
    count: i64,
    min_ts: Option<DateTime<Utc>>,
    max_ts: Option<DateTime<Utc>>,
    min_value: Option<f64>,
    max_value: Option<f64>,
    avg_value: Option<f64>,
    min_quality: Option<i32>,
    max_quality: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct SensorNameRow {
    sensor_id: String,
    name: String,
}

fn normalize_database_url(url: String) -> String {
    if let Some(stripped) = url.strip_prefix("postgresql+psycopg://") {
        return format!("postgresql://{stripped}");
    }
    url
}

fn redact_database_url(url: &str) -> String {
    if let Ok(mut parsed) = Url::parse(url) {
        if parsed.password().is_some() {
            let _ = parsed.set_password(Some("REDACTED"));
        }
        return parsed.to_string();
    }
    url.to_string()
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn env_u32(key: &str) -> Option<u32> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
}

fn env_i64(key: &str) -> Option<i64> {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 0)
}

fn parse_rfc3339(raw: &str) -> Result<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(raw)
        .with_context(|| format!("invalid RFC3339 timestamp: {raw}"))?;
    Ok(parsed.with_timezone(&Utc))
}

fn escape_single_quotes(raw: &str) -> String {
    raw.replace('\'', "''")
}

fn read_config(path: &str) -> Option<ConfigFile> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return None;
    }
    let raw = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn read_replication_state(hot_path: &Path) -> Result<ReplicationState> {
    let path = hot_path.join("_state/replication.json");
    if !path.exists() {
        return Ok(ReplicationState::default());
    }
    let bytes = std::fs::read(&path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn read_manifest(hot_path: &Path) -> Result<LakeManifest> {
    let path = hot_path.join("_state/manifest.json");
    if !path.exists() {
        return Ok(LakeManifest::default());
    }
    let bytes = std::fs::read(&path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn list_dates_in_range(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<NaiveDate> {
    if end < start {
        return vec![];
    }
    let mut dates = Vec::new();
    let mut cursor = start.date_naive();
    let end_date = end.date_naive();
    while cursor <= end_date {
        dates.push(cursor);
        let next = cursor.succ_opt().unwrap_or(cursor);
        if next == cursor {
            break;
        }
        cursor = next;
    }
    dates
}

fn shard_for_sensor_id(shards: u32, sensor_id: &str) -> u32 {
    let shards = shards.max(1);
    (xxh3_64(sensor_id.as_bytes()) % shards as u64) as u32
}

fn shard_set_for_sensor_ids(shards: u32, sensor_ids: &[String]) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for sensor_id in sensor_ids {
        let trimmed = sensor_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.insert(shard_for_sensor_id(shards, trimmed));
    }
    out
}

fn partition_dir(root: &Path, dataset: &str, date: NaiveDate, shard: u32) -> PathBuf {
    root.join(dataset)
        .join(format!("date={}", date.format("%Y-%m-%d")))
        .join(format!("shard={:02}", shard))
}

fn list_parquet_files_for_range(
    hot_path: &Path,
    cold_path: Option<&Path>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    shard_set: &BTreeSet<u32>,
) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let dates = list_dates_in_range(start, end);
    let manifest = read_manifest(hot_path).unwrap_or_default();
    let dataset = manifest.datasets.get(METRICS_DATASET_V1);

    for date in dates {
        let location = dataset
            .and_then(|ds| ds.partitions.get(&date.format("%Y-%m-%d").to_string()))
            .map(|partition| partition.location.as_str())
            .filter(|value| !value.is_empty());

        for shard in shard_set {
            let mut candidate_dirs: Vec<PathBuf> = Vec::new();
            match location {
                Some("cold") => {
                    if let Some(cold_root) = cold_path {
                        candidate_dirs.push(partition_dir(
                            cold_root,
                            METRICS_DATASET_V1,
                            date,
                            *shard,
                        ));
                    } else {
                        candidate_dirs.push(partition_dir(
                            hot_path,
                            METRICS_DATASET_V1,
                            date,
                            *shard,
                        ));
                    }
                }
                Some("hot") => {
                    candidate_dirs.push(partition_dir(hot_path, METRICS_DATASET_V1, date, *shard));
                }
                _ => {
                    candidate_dirs.push(partition_dir(hot_path, METRICS_DATASET_V1, date, *shard));
                    if let Some(cold_root) = cold_path {
                        candidate_dirs.push(partition_dir(
                            cold_root,
                            METRICS_DATASET_V1,
                            date,
                            *shard,
                        ));
                    }
                }
            }

            for dir in candidate_dirs {
                if !dir.exists() {
                    continue;
                }
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().and_then(|v| v.to_str()) == Some("parquet") {
                        out.push(path);
                    }
                }
            }
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

async fn fetch_pg_stats(
    pool: &PgPool,
    sensor_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<AggStats> {
    let row: PgAggRow = sqlx::query_as(
        r#"
        SELECT
          COUNT(*)::bigint as count,
          MIN(ts) as min_ts,
          MAX(ts) as max_ts,
          MIN(value) as min_value,
          MAX(value) as max_value,
          AVG(value) as avg_value,
          MIN(quality)::int as min_quality,
          MAX(quality)::int as max_quality
        FROM metrics
        WHERE sensor_id = $1
          AND ts >= $2
          AND ts <= $3
        "#,
    )
    .bind(sensor_id)
    .bind(start)
    .bind(end)
    .fetch_one(pool)
    .await
    .context("failed to aggregate Postgres metrics")?;

    Ok(AggStats {
        count: row.count,
        min_ts: row.min_ts,
        max_ts: row.max_ts,
        min_value: row.min_value,
        max_value: row.max_value,
        avg_value: row.avg_value,
        min_quality: row.min_quality,
        max_quality: row.max_quality,
    })
}

fn duckdb_aggregate(
    parquet_files: &[PathBuf],
    sensor_ids: &[String],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    tmp_path: &Path,
) -> Result<HashMap<String, AggStats>> {
    let mut out: HashMap<String, AggStats> = HashMap::new();
    if parquet_files.is_empty() || sensor_ids.is_empty() {
        return Ok(out);
    }

    let conn = Connection::open_in_memory()?;
    let tmp_dir = tmp_path.join("duckdb");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let _ = conn.execute("PRAGMA threads=2", []);
    let _ = conn.execute("PRAGMA enable_progress_bar=false", []);
    let tmp_dir_display = tmp_dir.display().to_string();
    let _ = conn.execute(
        &format!(
            "SET temp_directory='{}'",
            escape_single_quotes(&tmp_dir_display)
        ),
        [],
    );

    let files_sql = parquet_files
        .iter()
        .map(|p| {
            let display = p.display().to_string();
            format!("'{}'", escape_single_quotes(&display))
        })
        .collect::<Vec<_>>()
        .join(", ");

    let sensors_sql = sensor_ids
        .iter()
        .map(|s| format!("'{}'", escape_single_quotes(s.trim())))
        .collect::<Vec<_>>()
        .join(", ");

    let start_sql = start.to_rfc3339();
    let end_sql = end.to_rfc3339();

    let sql = format!(
        r#"
        SELECT
          sensor_id,
          COUNT(*)::BIGINT as count,
          MIN(ts) as min_ts,
          MAX(ts) as max_ts,
          MIN(value) as min_value,
          MAX(value) as max_value,
          AVG(value) as avg_value,
          MIN(quality)::INT as min_quality,
          MAX(quality)::INT as max_quality
        FROM read_parquet([{files_sql}], hive_partitioning=1, union_by_name=1)
        WHERE sensor_id IN ({sensors_sql})
          AND ts >= '{start_sql}'::TIMESTAMP
          AND ts <= '{end_sql}'::TIMESTAMP
        GROUP BY sensor_id
        ORDER BY sensor_id
        "#
    );

    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let sensor_id: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        let min_ts: Option<NaiveDateTime> = row.get(2)?;
        let max_ts: Option<NaiveDateTime> = row.get(3)?;
        let min_value: Option<f64> = row.get(4)?;
        let max_value: Option<f64> = row.get(5)?;
        let avg_value: Option<f64> = row.get(6)?;
        let min_quality: Option<i32> = row.get(7)?;
        let max_quality: Option<i32> = row.get(8)?;

        out.insert(
            sensor_id,
            AggStats {
                count,
                min_ts: min_ts.map(|ts| DateTime::<Utc>::from_naive_utc_and_offset(ts, Utc)),
                max_ts: max_ts.map(|ts| DateTime::<Utc>::from_naive_utc_and_offset(ts, Utc)),
                min_value,
                max_value,
                avg_value,
                min_quality,
                max_quality,
            },
        );
    }

    Ok(out)
}

fn fmt_ts(value: &Option<DateTime<Utc>>) -> String {
    value
        .map(|ts| ts.to_rfc3339())
        .unwrap_or_else(|| "null".to_string())
}

fn fmt_f64(value: &Option<f64>) -> String {
    value
        .map(|v| format!("{v:.6}"))
        .unwrap_or_else(|| "null".to_string())
}

fn fmt_i32(value: &Option<i32>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn compare_stats(pg: &AggStats, pq: &AggStats) -> Vec<String> {
    let mut diffs = Vec::new();
    const EPSILON: f64 = 1e-6;

    if pg.count != pq.count {
        diffs.push(format!("count pg={} parquet={}", pg.count, pq.count));
    }
    if pg.min_ts != pq.min_ts {
        diffs.push(format!(
            "min_ts pg={} parquet={}",
            fmt_ts(&pg.min_ts),
            fmt_ts(&pq.min_ts)
        ));
    }
    if pg.max_ts != pq.max_ts {
        diffs.push(format!(
            "max_ts pg={} parquet={}",
            fmt_ts(&pg.max_ts),
            fmt_ts(&pq.max_ts)
        ));
    }
    match (pg.min_value, pq.min_value) {
        (Some(pg_val), Some(pq_val)) if (pg_val - pq_val).abs() > EPSILON => {
            diffs.push(format!("min_value pg={} parquet={}", pg_val, pq_val));
        }
        (None, Some(_)) | (Some(_), None) => {
            diffs.push(format!(
                "min_value pg={} parquet={}",
                fmt_f64(&pg.min_value),
                fmt_f64(&pq.min_value)
            ));
        }
        _ => {}
    }
    match (pg.max_value, pq.max_value) {
        (Some(pg_val), Some(pq_val)) if (pg_val - pq_val).abs() > EPSILON => {
            diffs.push(format!("max_value pg={} parquet={}", pg_val, pq_val));
        }
        (None, Some(_)) | (Some(_), None) => {
            diffs.push(format!(
                "max_value pg={} parquet={}",
                fmt_f64(&pg.max_value),
                fmt_f64(&pq.max_value)
            ));
        }
        _ => {}
    }
    match (pg.avg_value, pq.avg_value) {
        (Some(pg_val), Some(pq_val)) if (pg_val - pq_val).abs() > EPSILON => {
            diffs.push(format!("avg_value pg={} parquet={}", pg_val, pq_val));
        }
        (None, Some(_)) | (Some(_), None) => {
            diffs.push(format!(
                "avg_value pg={} parquet={}",
                fmt_f64(&pg.avg_value),
                fmt_f64(&pq.avg_value)
            ));
        }
        _ => {}
    }
    if pg.min_quality != pq.min_quality {
        diffs.push(format!(
            "min_quality pg={} parquet={}",
            fmt_i32(&pg.min_quality),
            fmt_i32(&pq.min_quality)
        ));
    }
    if pg.max_quality != pq.max_quality {
        diffs.push(format!(
            "max_quality pg={} parquet={}",
            fmt_i32(&pg.max_quality),
            fmt_i32(&pq.max_quality)
        ));
    }

    diffs
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let mut sensor_ids: Vec<String> = args
        .sensor_ids
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    sensor_ids.sort();
    sensor_ids.dedup();
    anyhow::ensure!(
        !sensor_ids.is_empty(),
        "at least one --sensor-id is required"
    );

    let config = read_config(&args.config);
    let database_url = args
        .database_url
        .or_else(|| config.as_ref().and_then(|c| c.database_url.clone()))
        .or_else(|| std::env::var("CORE_DATABASE_URL").ok())
        .map(normalize_database_url)
        .context("database_url not provided and not found in config/env")?;

    let data_root = args
        .data_root
        .or_else(|| {
            config
                .as_ref()
                .and_then(|c| c.data_root.as_ref())
                .map(|v| PathBuf::from(v.trim()))
                .filter(|p| !p.as_os_str().is_empty())
        })
        .or_else(|| env_path("CORE_DATA_ROOT"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DATA_ROOT));

    let hot_path = args
        .hot_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_HOT_PATH"))
        .unwrap_or_else(|| data_root.join("storage/analysis/lake/hot"));
    let cold_path = args
        .cold_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_COLD_PATH"));
    let tmp_path = args
        .tmp_path
        .or_else(|| env_path("CORE_ANALYSIS_TMP_PATH"))
        .unwrap_or_else(|| data_root.join("storage/analysis/tmp"));
    let shards = args
        .shards
        .or_else(|| env_u32("CORE_ANALYSIS_LAKE_SHARDS"))
        .unwrap_or(DEFAULT_SHARDS)
        .max(1);
    let replication_lag_seconds = env_i64("CORE_ANALYSIS_REPLICATION_LAG_SECONDS")
        .unwrap_or(DEFAULT_LAG_SECONDS)
        .max(0);

    let replication_state = read_replication_state(&hot_path).unwrap_or_default();
    let computed_through = replication_state
        .computed_through_ts
        .as_deref()
        .and_then(|raw| parse_rfc3339(raw).ok());

    let (start, end) = match (args.start.as_deref(), args.end.as_deref()) {
        (Some(start_raw), Some(end_raw)) => (parse_rfc3339(start_raw)?, parse_rfc3339(end_raw)?),
        (None, Some(end_raw)) => {
            let end = parse_rfc3339(end_raw)?;
            let start = end - Duration::minutes(args.window_minutes.max(1));
            (start, end)
        }
        (Some(_), None) => {
            anyhow::bail!("--end is required when --start is provided (or omit --start to use --window-minutes)");
        }
        (None, None) => {
            let end = computed_through
                .unwrap_or_else(|| Utc::now() - Duration::seconds(replication_lag_seconds));
            let start = end - Duration::minutes(args.window_minutes.max(1));
            (start, end)
        }
    };

    anyhow::ensure!(end >= start, "end must be >= start");

    println!("TSSE Parquet parity spot-check");
    println!("Database: {}", redact_database_url(&database_url));
    println!("Lake hot: {}", hot_path.display());
    if let Some(cold) = &cold_path {
        println!("Lake cold: {}", cold.display());
    } else {
        println!("Lake cold: (none)");
    }
    println!("Lake shards: {}", shards);
    println!(
        "Window (UTC): {} -> {}",
        start.to_rfc3339(),
        end.to_rfc3339()
    );
    if let Some(ct) = computed_through {
        println!(
            "Replication watermark: computed_through_ts={}",
            ct.to_rfc3339()
        );
        if end > ct {
            eprintln!("Warning: window end is newer than computed_through_ts; parity may lag.");
        }
    } else {
        println!("Replication watermark: computed_through_ts=(missing)");
        let lag_end = Utc::now() - Duration::seconds(replication_lag_seconds);
        if end > lag_end {
            eprintln!("Warning: window end is newer than now - replication lag; parity may lag.");
        }
    }
    if let Some(last_inserted) = replication_state.last_inserted_at.as_deref() {
        println!("Replication last_inserted_at={}", last_inserted);
    }

    let pool = PgPool::connect(&database_url)
        .await
        .context("failed to connect to Postgres")?;

    let sensor_rows: Vec<SensorNameRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, name
        FROM sensors
        WHERE sensor_id = ANY($1)
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut sensor_names: HashMap<String, String> = HashMap::new();
    for row in sensor_rows {
        sensor_names.insert(row.sensor_id, row.name);
    }

    let shard_set = shard_set_for_sensor_ids(shards, &sensor_ids);
    let parquet_files =
        list_parquet_files_for_range(&hot_path, cold_path.as_deref(), start, end, &shard_set)
            .context("failed to enumerate parquet files")?;

    println!("Parquet files scanned: {}", parquet_files.len());

    let parquet_stats = {
        let sensor_ids = sensor_ids.clone();
        let parquet_files = parquet_files.clone();
        let tmp_path = tmp_path.clone();
        tokio::task::spawn_blocking(move || {
            duckdb_aggregate(&parquet_files, &sensor_ids, start, end, &tmp_path)
        })
        .await
        .context("duckdb task join failed")??
    };

    let mut mismatch_count = 0;
    for sensor_id in &sensor_ids {
        let pg_stats = fetch_pg_stats(&pool, sensor_id, start, end).await?;
        let pq_stats = parquet_stats.get(sensor_id).cloned().unwrap_or_default();
        let diffs = compare_stats(&pg_stats, &pq_stats);
        let name = sensor_names
            .get(sensor_id)
            .map(|name| format!(" ({name})"))
            .unwrap_or_default();

        if diffs.is_empty() {
            println!("- {}{}: OK (count={})", sensor_id, name, pg_stats.count);
        } else {
            mismatch_count += 1;
            println!("- {}{}: MISMATCH", sensor_id, name);
            for diff in diffs {
                println!("  - {}", diff);
            }
        }
    }

    if mismatch_count > 0 {
        anyhow::bail!("parity mismatch detected: {mismatch_count} sensor(s)");
    }

    println!("Parity check PASS ({} sensor(s))", sensor_ids.len());
    Ok(())
}
