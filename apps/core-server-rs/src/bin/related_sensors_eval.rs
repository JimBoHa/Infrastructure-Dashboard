use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, ValueEnum};
use core_server_rs::services::analysis::jobs::eval::execute_related_sensors_unified_v2;
use core_server_rs::services::analysis::jobs::{AnalysisJobProgress, AnalysisJobRow};
use core_server_rs::services::analysis::lake::AnalysisLakeConfig;
use core_server_rs::services::analysis::parquet_duckdb::DuckDbQueryService;
use core_server_rs::services::analysis::tsse::types::{
    RelatedSensorsUnifiedJobParamsV2, RelatedSensorsUnifiedResultV2, UnifiedConfidenceTierV2,
    UnifiedRelationshipModeV2,
};
use reqwest::Url;
use serde::Deserialize;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const DEFAULT_CONFIG_PATH: &str = "/Users/Shared/FarmDashboard/setup/config.json";
const DEFAULT_DATA_ROOT: &str = "/Users/Shared/FarmDashboard";
const DEFAULT_SHARDS: u32 = 16;
const DEFAULT_INTERVAL_SECONDS: i64 = 60;
const DEFAULT_CANDIDATE_LIMIT: u32 = 200;
const DEFAULT_MAX_RESULTS: u32 = 20;

#[derive(Debug, Clone, Copy, ValueEnum)]
#[clap(rename_all = "snake_case")]
enum ExecMode {
    /// Execute Unified v2 by calling the installed controller API (recommended; avoids lake permissions issues).
    Api,
    /// Execute Unified v2 directly in-process (requires direct lake/DB access).
    Direct,
}

#[derive(Debug, Parser)]
#[command(about = "Evaluate Related Sensors Unified v2 quality against a labeled case set.")]
struct Args {
    /// Execution mode for running Unified v2.
    #[arg(long, value_enum, default_value_t = ExecMode::Api)]
    exec_mode: ExecMode,

    /// Setup config path (used to read database_url and data_root when overrides are not provided).
    #[arg(long, default_value = DEFAULT_CONFIG_PATH)]
    config: String,

    /// Postgres connection string (overrides setup config).
    #[arg(long)]
    database_url: Option<String>,

    /// Data root (overrides setup config; used to derive default hot/tmp paths).
    #[arg(long)]
    data_root: Option<PathBuf>,

    /// Analysis lake hot path root.
    #[arg(long)]
    hot_path: Option<PathBuf>,

    /// Analysis lake cold path root (optional).
    #[arg(long)]
    cold_path: Option<PathBuf>,

    /// Analysis tmp path root (DuckDB scratch).
    #[arg(long)]
    tmp_path: Option<PathBuf>,

    /// Number of lake shards (must match CORE_ANALYSIS_LAKE_SHARDS).
    #[arg(long, default_value_t = DEFAULT_SHARDS)]
    shards: u32,

    /// Core-server base URL when `--exec-mode=api`.
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    api_base_url: String,

    /// API token string (Bearer) when `--exec-mode=api`.
    #[arg(long)]
    auth_token: Option<String>,

    /// File containing a single-line API token string (Bearer) when `--exec-mode=api`.
    #[arg(long)]
    auth_token_file: Option<PathBuf>,

    /// Poll interval (ms) when `--exec-mode=api`.
    #[arg(long, default_value_t = 250)]
    poll_ms: u64,

    /// Per-case timeout (seconds) when `--exec-mode=api`.
    #[arg(long, default_value_t = 120)]
    timeout_seconds: u64,

    /// Labeled cases JSON file.
    #[arg(long, default_value = "reports/related_sensors_eval/cases.json")]
    cases: PathBuf,

    /// Output report path (Markdown).
    #[arg(long, default_value = "reports/related_sensors_eval/report.md")]
    report_md: PathBuf,

    /// Output report path (JSON).
    #[arg(long, default_value = "reports/related_sensors_eval/report.json")]
    report_json: PathBuf,

    /// Unified bucket interval in seconds (may be increased automatically for very large windows).
    #[arg(long, default_value_t = DEFAULT_INTERVAL_SECONDS)]
    interval_seconds: i64,

    /// Candidate limit for Unified v2 runs.
    #[arg(long, default_value_t = DEFAULT_CANDIDATE_LIMIT)]
    candidate_limit: u32,

    /// Max results to keep from Unified v2 (top N).
    #[arg(long, default_value_t = DEFAULT_MAX_RESULTS)]
    max_results: u32,

    /// Limit number of cases executed (for smoke runs).
    #[arg(long)]
    case_limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    database_url: Option<String>,
    #[serde(default)]
    data_root: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct LabeledCase {
    id: String,
    focus_sensor_id: String,
    start: String,
    end: String,
    expected_related_sensor_ids: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CaseSet {
    schema_version: u32,
    cases: Vec<LabeledCase>,
}

#[derive(Debug, Clone)]
struct CaseMetrics {
    precision_at_5: f64,
    precision_at_10: f64,
    precision_at_20: f64,
    mrr: f64,
    strong_coverage: bool,
}

#[derive(Debug, Clone)]
struct CaseRunResult {
    case: LabeledCase,
    duration_ms: u64,
    result_count: usize,
    strong_count: usize,
    top_sensor_ids: Vec<String>,
    metrics: CaseMetrics,
    error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
struct JsonReport {
    schema_version: u32,
    args: BTreeMap<String, serde_json::Value>,
    summary: serde_json::Value,
    cases: Vec<serde_json::Value>,
}

fn normalize_database_url(url: String) -> String {
    if let Some(stripped) = url.strip_prefix("postgresql+psycopg://") {
        return format!("postgresql://{stripped}");
    }
    if let Some(stripped) = url.strip_prefix("postgresql+asyncpg://") {
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

fn read_setup_config(path: &Path) -> Result<ConfigFile> {
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let cfg: ConfigFile = serde_json::from_str(&raw).context("failed to parse setup config JSON")?;
    Ok(cfg)
}

fn resolve_data_root(args: &Args, cfg: &ConfigFile) -> PathBuf {
    if let Some(root) = args.data_root.as_ref() {
        return root.clone();
    }
    if let Some(root) = cfg.data_root.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        return PathBuf::from(root);
    }
    PathBuf::from(DEFAULT_DATA_ROOT)
}

fn resolve_database_url(args: &Args, cfg: &ConfigFile) -> Result<String> {
    if let Some(url) = args.database_url.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        return Ok(normalize_database_url(url.to_string()));
    }
    let Some(url) = cfg.database_url.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
        anyhow::bail!("database_url missing from setup config; pass --database-url to override");
    };
    Ok(normalize_database_url(url.to_string()))
}

fn load_cases(path: &Path) -> Result<CaseSet> {
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let set: CaseSet = serde_json::from_str(&raw).context("failed to parse cases JSON")?;
    if set.schema_version != 1 {
        anyhow::bail!(
            "unsupported cases schema_version {}; expected 1",
            set.schema_version
        );
    }
    Ok(set)
}

fn clamp_case_window(start: &str, end: &str) -> Result<(String, String)> {
    let start_ts = DateTime::parse_from_rfc3339(start)
        .with_context(|| format!("invalid case start timestamp: {start}"))?
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339(end)
        .with_context(|| format!("invalid case end timestamp: {end}"))?
        .with_timezone(&Utc);
    if end_ts <= start_ts {
        anyhow::bail!("case window invalid: end must be after start");
    }

    // Keep API-compatible bounds: reuse the same 24*365h guard used elsewhere.
    let max = Duration::hours(24 * 365);
    if end_ts - start_ts > max {
        anyhow::bail!(
            "case window too large (>{}h); narrow the labeled window",
            max.num_hours()
        );
    }

    Ok((start_ts.to_rfc3339(), end_ts.to_rfc3339()))
}

fn compute_case_metrics(expected: &HashSet<String>, predicted: &[String], strong: bool) -> CaseMetrics {
    let precision_at = |k: usize| -> f64 {
        if k == 0 {
            return 0.0;
        }
        let slice = &predicted[..std::cmp::min(k, predicted.len())];
        let hits = slice.iter().filter(|id| expected.contains(*id)).count();
        (hits as f64) / (k as f64)
    };
    let mrr = predicted
        .iter()
        .enumerate()
        .find(|(_, id)| expected.contains(*id))
        .map(|(idx, _)| 1.0 / ((idx + 1) as f64))
        .unwrap_or(0.0);

    CaseMetrics {
        precision_at_5: precision_at(5),
        precision_at_10: precision_at(10),
        precision_at_20: precision_at(20),
        mrr,
        strong_coverage: strong,
    }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let sum: f64 = values.iter().copied().sum();
    sum / (values.len() as f64)
}

fn fmt_pct(value: f64) -> String {
    format!("{:.1}%", value * 100.0)
}

fn build_default_params(case: &LabeledCase, start: String, end: String, args: &Args) -> RelatedSensorsUnifiedJobParamsV2 {
    RelatedSensorsUnifiedJobParamsV2 {
        focus_sensor_id: case.focus_sensor_id.clone(),
        start,
        end,
        focus_events: Vec::new(),
        interval_seconds: Some(args.interval_seconds.max(1)),
        mode: Some(UnifiedRelationshipModeV2::Advanced),
        candidate_source: None, // default: controller-wide scan when candidate list empty
        candidate_sensor_ids: Vec::new(),
        pinned_sensor_ids: Vec::new(),
        evaluate_all_eligible: None,
        candidate_limit: Some(args.candidate_limit.max(10)),
        max_results: Some(args.max_results.max(5)),
        include_low_confidence: Some(true),
        quick_suggest: Some(false),
        stability_enabled: None,
        exclude_system_wide_buckets: Some(false),
        filters: Default::default(),
        weights: None,
        polarity: None,
        z_threshold: None,
        threshold_mode: None,
        adaptive_threshold: None,
        detector_mode: None,
        suppression_mode: None,
        exclude_boundary_events: None,
        sparse_point_events_enabled: None,
        z_cap: None,
        min_separation_buckets: None,
        gap_max_buckets: None,
        max_lag_buckets: None,
        max_events: None,
        max_episodes: None,
        episode_gap_buckets: None,
        tolerance_buckets: None,
        min_sensors: None,
        include_delta_corr_signal: None,
        deseason_mode: None,
        periodic_penalty_enabled: None,
        cooccurrence_score_mode: None,
        cooccurrence_bucket_preference_mode: None,
    }
}

fn read_auth_token(args: &Args) -> Result<String> {
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
        let token = raw.trim();
        if !token.is_empty() {
            return Ok(token.to_string());
        }
    }
    anyhow::bail!("auth token is required for --exec-mode=api (use --auth-token or --auth-token-file)");
}

fn bearer_value(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.to_ascii_lowercase().starts_with("bearer ") {
        trimmed.to_string()
    } else {
        format!("Bearer {}", trimmed)
    }
}

#[derive(Debug, Deserialize)]
struct ApiJobPublic {
    id: String,
    status: String,
    #[serde(default)]
    error: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ApiCreateResponse {
    job: ApiJobPublic,
}

#[derive(Debug, Deserialize)]
struct ApiStatusResponse {
    job: ApiJobPublic,
}

#[derive(Debug, Deserialize)]
struct ApiResultResponse {
    result: serde_json::Value,
}

async fn run_case_via_api(
    client: &reqwest::Client,
    api_base: &Url,
    auth_header: &str,
    case: &LabeledCase,
    params: &RelatedSensorsUnifiedJobParamsV2,
    poll_ms: u64,
    timeout_seconds: u64,
) -> Result<RelatedSensorsUnifiedResultV2> {
    let create_url = api_base
        .join("/api/analysis/jobs")
        .with_context(|| format!("invalid api_base_url: {}", api_base))?;

    let job_key = format!("related_sensors_eval:{}", case.id);
    let payload = serde_json::json!({
        "job_type": "related_sensors_unified_v2",
        "params": serde_json::to_value(params).expect("params json"),
        "job_key": job_key,
        "dedupe": false,
    });

    let created: ApiCreateResponse = client
        .post(create_url)
        .header(reqwest::header::AUTHORIZATION, auth_header)
        .json(&payload)
        .send()
        .await
        .context("failed to call create job")?
        .error_for_status()
        .context("create job returned non-2xx")?
        .json()
        .await
        .context("failed to decode create response")?;

    let job_id = created.job.id;
    let status_url = api_base
        .join(&format!("/api/analysis/jobs/{}", job_id))
        .with_context(|| "invalid status url")?;
    let result_url = api_base
        .join(&format!("/api/analysis/jobs/{}/result", job_id))
        .with_context(|| "invalid result url")?;

    let deadline = Instant::now() + std::time::Duration::from_secs(timeout_seconds.max(1));
    loop {
        let status: ApiStatusResponse = client
            .get(status_url.clone())
            .header(reqwest::header::AUTHORIZATION, auth_header)
            .send()
            .await
            .context("failed to poll job status")?
            .error_for_status()
            .context("poll status returned non-2xx")?
            .json()
            .await
            .context("failed to decode status response")?;

        match status.job.status.as_str() {
            "completed" => break,
            "failed" => {
                anyhow::bail!(
                    "analysis job failed (job_id={}): {}",
                    job_id,
                    status
                        .job
                        .error
                        .as_ref()
                        .and_then(|v| v.get("message"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown error")
                );
            }
            "canceled" => anyhow::bail!("analysis job canceled (job_id={})", job_id),
            _ => {}
        }

        if Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for analysis job {} to complete", job_id);
        }
        tokio::time::sleep(std::time::Duration::from_millis(poll_ms.max(50))).await;
    }

    let result: ApiResultResponse = client
        .get(result_url)
        .header(reqwest::header::AUTHORIZATION, auth_header)
        .send()
        .await
        .context("failed to fetch job result")?
        .error_for_status()
        .context("result returned non-2xx")?
        .json()
        .await
        .context("failed to decode result response")?;

    let decoded: RelatedSensorsUnifiedResultV2 =
        serde_json::from_value(result.result).context("failed to decode unified job result")?;
    Ok(decoded)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let cases = load_cases(&args.cases)?;
    let case_limit = args.case_limit.unwrap_or(cases.cases.len()).min(cases.cases.len());

    eprintln!("Related Sensors eval");
    eprintln!("- Cases: {} (running {})", cases.cases.len(), case_limit);
    eprintln!("- Cases file: {}", args.cases.display());
    let mut results: Vec<CaseRunResult> = Vec::new();

    let (api_client, api_base, api_auth) = match args.exec_mode {
        ExecMode::Api => {
            let api_base =
                Url::parse(args.api_base_url.trim()).context("invalid --api-base-url")?;
            let token = read_auth_token(&args)?;
            let auth = bearer_value(&token);
            (Some(reqwest::Client::new()), Some(api_base), Some(auth))
        }
        ExecMode::Direct => (None, None, None),
    };

    let (db, duckdb, lake, cancel) = match args.exec_mode {
        ExecMode::Api => (None, None, None, None),
        ExecMode::Direct => {
            let cfg = read_setup_config(Path::new(args.config.trim()))?;
            let database_url = resolve_database_url(&args, &cfg)?;
            let data_root = resolve_data_root(&args, &cfg);

            let hot_path = args
                .hot_path
                .clone()
                .unwrap_or_else(|| data_root.join("storage/analysis/lake/hot"));
            let tmp_path = args
                .tmp_path
                .clone()
                .unwrap_or_else(|| data_root.join("storage/analysis/tmp"));
            let cold_path = args
                .cold_path
                .clone()
                .or_else(|| Some(data_root.join("storage/analysis/lake/cold")).filter(|p| p.exists()));

            eprintln!("- DB: {}", redact_database_url(&database_url));
            eprintln!("- Lake hot: {}", hot_path.display());
            eprintln!(
                "- Lake cold: {}",
                cold_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "none".to_string())
            );
            eprintln!("- Tmp: {}", tmp_path.display());

            let db = PgPool::connect(&database_url)
                .await
                .context("failed to connect to Postgres")?;
            let duckdb = DuckDbQueryService::new(tmp_path.clone(), 4);
            let lake = AnalysisLakeConfig {
                hot_path,
                cold_path,
                tmp_path,
                shards: args.shards.max(1),
                hot_retention_days: 60,
                late_window_hours: 24,
                replication_interval: std::time::Duration::from_secs(60),
                replication_lag: std::time::Duration::from_secs(60),
            };

            (Some(db), Some(duckdb), Some(lake), Some(CancellationToken::new()))
        }
    };

    for case in cases.cases.into_iter().take(case_limit) {
        let started = Instant::now();
        let expected_set: HashSet<String> = case
            .expected_related_sensor_ids
            .iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect();

        let (start, end) = clamp_case_window(&case.start, &case.end)
            .with_context(|| format!("case {} has invalid window", case.id))?;

        let params = build_default_params(&case, start, end, &args);

        let (result, error) = match args.exec_mode {
            ExecMode::Api => {
                let client = api_client.as_ref().expect("api_client");
                let base = api_base.as_ref().expect("api_base");
                let auth = api_auth.as_deref().expect("api_auth");
                match run_case_via_api(
                    client,
                    base,
                    auth,
                    &case,
                    &params,
                    args.poll_ms,
                    args.timeout_seconds,
                )
                .await
                {
                    Ok(res) => (Some(res), None),
                    Err(err) => (None, Some(err.to_string())),
                }
            }
            ExecMode::Direct => {
                let job = AnalysisJobRow {
                    id: Uuid::new_v4(),
                    job_type: "related_sensors_unified_v2".to_string(),
                    status: "running".to_string(),
                    job_key: Some(format!("related_sensors_eval:{}", case.id)),
                    created_by: None,
                    params: SqlJson(serde_json::to_value(&params).expect("params json")),
                    progress: SqlJson(AnalysisJobProgress::default()),
                    error: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    started_at: None,
                    completed_at: None,
                    cancel_requested_at: None,
                    canceled_at: None,
                    expires_at: None,
                };

                let run = async {
                    let db = db.as_ref().expect("db");
                    let duckdb = duckdb.as_ref().expect("duckdb");
                    let lake = lake.as_ref().expect("lake");
                    let cancel = cancel.as_ref().expect("cancel").clone();
                    let value =
                        execute_related_sensors_unified_v2(db, duckdb, lake, &job, cancel).await?;
                    let result: RelatedSensorsUnifiedResultV2 =
                        serde_json::from_value(value).context("failed to decode unified result")?;
                    Ok::<_, anyhow::Error>(result)
                };

                match run.await {
                    Ok(result) => (Some(result), None),
                    Err(err) => (None, Some(err.to_string())),
                }
            }
        };

        let duration_ms = started.elapsed().as_millis() as u64;

        let (result_count, strong_count, top_sensor_ids, metrics) = if let Some(ref result) = result {
            let candidates = &result.candidates;
            let top_sensor_ids: Vec<String> = candidates.iter().map(|c| c.sensor_id.clone()).collect();
            let strong_count = candidates
                .iter()
                .filter(|c| matches!(c.confidence_tier, UnifiedConfidenceTierV2::High | UnifiedConfidenceTierV2::Medium))
                .count();
            let strong = strong_count > 0;
            let metrics = compute_case_metrics(&expected_set, &top_sensor_ids, strong);
            (candidates.len(), strong_count, top_sensor_ids, metrics)
        } else {
            let metrics = compute_case_metrics(&expected_set, &[], false);
            (0, 0, Vec::new(), metrics)
        };

        results.push(CaseRunResult {
            case,
            duration_ms,
            result_count,
            strong_count,
            top_sensor_ids: top_sensor_ids.clone(),
            metrics,
            error,
        });
    }

    let succeeded: Vec<&CaseRunResult> = results.iter().filter(|r| r.error.is_none()).collect();
    let total = results.len().max(1);
    let successes = succeeded.len();

    let p5: Vec<f64> = succeeded.iter().map(|r| r.metrics.precision_at_5).collect();
    let p10: Vec<f64> = succeeded.iter().map(|r| r.metrics.precision_at_10).collect();
    let p20: Vec<f64> = succeeded.iter().map(|r| r.metrics.precision_at_20).collect();
    let mrrs: Vec<f64> = succeeded.iter().map(|r| r.metrics.mrr).collect();
    let strong_cov = succeeded
        .iter()
        .filter(|r| r.metrics.strong_coverage)
        .count();

    let mut by_tag_agg: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut by_tag_cov: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in succeeded.iter() {
        for tag in r.case.tags.iter() {
            by_tag_agg
                .entry(format!("p5:{tag}"))
                .or_default()
                .push(r.metrics.precision_at_5);
            by_tag_agg
                .entry(format!("p10:{tag}"))
                .or_default()
                .push(r.metrics.precision_at_10);
            by_tag_agg
                .entry(format!("p20:{tag}"))
                .or_default()
                .push(r.metrics.precision_at_20);
            by_tag_agg
                .entry(format!("mrr:{tag}"))
                .or_default()
                .push(r.metrics.mrr);
            let entry = by_tag_cov.entry(tag.to_string()).or_insert((0, 0));
            entry.0 += 1;
            if r.metrics.strong_coverage {
                entry.1 += 1;
            }
        }
    }

    let mut by_tag: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    for (tag, (count, strong)) in by_tag_cov.into_iter() {
        let p5_key = format!("p5:{tag}");
        let p10_key = format!("p10:{tag}");
        let p20_key = format!("p20:{tag}");
        let mrr_key = format!("mrr:{tag}");
        let p5_vals = by_tag_agg.get(&p5_key).map(|v| v.as_slice()).unwrap_or(&[]);
        let p10_vals = by_tag_agg.get(&p10_key).map(|v| v.as_slice()).unwrap_or(&[]);
        let p20_vals = by_tag_agg.get(&p20_key).map(|v| v.as_slice()).unwrap_or(&[]);
        let mrr_vals = by_tag_agg.get(&mrr_key).map(|v| v.as_slice()).unwrap_or(&[]);
        by_tag.insert(
            tag,
            serde_json::json!({
                "cases": count,
                "precision_at_5_mean": mean(p5_vals),
                "precision_at_10_mean": mean(p10_vals),
                "precision_at_20_mean": mean(p20_vals),
                "mrr_mean": mean(mrr_vals),
                "strong_coverage_rate": if count > 0 { (strong as f64) / (count as f64) } else { 0.0 },
            }),
        );
    }

    let summary = serde_json::json!({
        "cases_total": results.len(),
        "cases_succeeded": successes,
        "cases_failed": results.len().saturating_sub(successes),
        "precision_at_5_mean": mean(&p5),
        "precision_at_10_mean": mean(&p10),
        "precision_at_20_mean": mean(&p20),
        "mrr_mean": mean(&mrrs),
        "strong_coverage_rate": if successes > 0 { (strong_cov as f64) / (successes as f64) } else { 0.0 },
        "by_tag": by_tag,
    });

    let mut args_map: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    args_map.insert(
        "exec_mode".to_string(),
        serde_json::json!(match args.exec_mode {
            ExecMode::Api => "api",
            ExecMode::Direct => "direct",
        }),
    );
    args_map.insert("cases".to_string(), serde_json::json!(args.cases));
    args_map.insert("report_md".to_string(), serde_json::json!(args.report_md));
    args_map.insert("report_json".to_string(), serde_json::json!(args.report_json));
    args_map.insert("interval_seconds".to_string(), serde_json::json!(args.interval_seconds));
    args_map.insert("candidate_limit".to_string(), serde_json::json!(args.candidate_limit));
    args_map.insert("max_results".to_string(), serde_json::json!(args.max_results));
    args_map.insert("case_limit".to_string(), serde_json::json!(args.case_limit));

    match args.exec_mode {
        ExecMode::Api => {
            args_map.insert("api_base_url".to_string(), serde_json::json!(args.api_base_url));
            args_map.insert("poll_ms".to_string(), serde_json::json!(args.poll_ms));
            args_map.insert("timeout_seconds".to_string(), serde_json::json!(args.timeout_seconds));
            args_map.insert(
                "auth_token_source".to_string(),
                serde_json::json!(if args.auth_token.is_some() {
                    "arg"
                } else if args.auth_token_file.is_some() {
                    "file"
                } else {
                    "none"
                }),
            );
        }
        ExecMode::Direct => {
            let cfg = read_setup_config(Path::new(args.config.trim()))?;
            let database_url = resolve_database_url(&args, &cfg)?;
            let data_root = resolve_data_root(&args, &cfg);
            let hot_path = args
                .hot_path
                .clone()
                .unwrap_or_else(|| data_root.join("storage/analysis/lake/hot"));
            let tmp_path = args
                .tmp_path
                .clone()
                .unwrap_or_else(|| data_root.join("storage/analysis/tmp"));
            let cold_path = args
                .cold_path
                .clone()
                .or_else(|| Some(data_root.join("storage/analysis/lake/cold")).filter(|p| p.exists()));

            args_map.insert("config".to_string(), serde_json::json!(args.config));
            args_map.insert("shards".to_string(), serde_json::json!(args.shards));
            args_map.insert(
                "database_url".to_string(),
                serde_json::json!(redact_database_url(&database_url)),
            );
            args_map.insert("data_root".to_string(), serde_json::json!(data_root));
            args_map.insert("hot_path".to_string(), serde_json::json!(hot_path));
            args_map.insert("cold_path".to_string(), serde_json::json!(cold_path));
            args_map.insert("tmp_path".to_string(), serde_json::json!(tmp_path));
        }
    }

    let json_cases: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            let expected: HashSet<&str> = r
                .case
                .expected_related_sensor_ids
                .iter()
                .map(|s| s.as_str())
                .collect();
            let hits_top10 = r
                .top_sensor_ids
                .iter()
                .take(10)
                .filter(|id| expected.contains(id.as_str()))
                .count();
            serde_json::json!({
                "id": r.case.id,
                "focus_sensor_id": r.case.focus_sensor_id,
                "start": r.case.start,
                "end": r.case.end,
                "expected_related_sensor_ids": r.case.expected_related_sensor_ids,
                "tags": r.case.tags,
                "duration_ms": r.duration_ms,
                "result_count": r.result_count,
                "strong_count": r.strong_count,
                "precision_at_5": r.metrics.precision_at_5,
                "precision_at_10": r.metrics.precision_at_10,
                "precision_at_20": r.metrics.precision_at_20,
                "mrr": r.metrics.mrr,
                "strong_coverage": r.metrics.strong_coverage,
                "hits_top10": hits_top10,
                "top_sensor_ids": r.top_sensor_ids.iter().take(20).cloned().collect::<Vec<_>>(),
                "error": r.error,
            })
        })
        .collect();

    let report = JsonReport {
        schema_version: 1,
        args: args_map,
        summary: summary.clone(),
        cases: json_cases.clone(),
    };

    if let Some(parent) = args.report_json.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.report_json, serde_json::to_string_pretty(&report).unwrap())
        .with_context(|| format!("failed to write {}", args.report_json.display()))?;

    let md = {
        let mut out = String::new();
        out.push_str("# Related Sensors Eval Report (Unified v2)\n\n");
        out.push_str("## Summary\n\n");
        out.push_str(&format!("- Cases: {successes}/{total} succeeded\n"));
        out.push_str(&format!(
            "- Precision@5 (mean): {}\n",
            fmt_pct(summary["precision_at_5_mean"].as_f64().unwrap_or(0.0))
        ));
        out.push_str(&format!(
            "- Precision@10 (mean): {}\n",
            fmt_pct(summary["precision_at_10_mean"].as_f64().unwrap_or(0.0))
        ));
        out.push_str(&format!(
            "- Precision@20 (mean): {}\n",
            fmt_pct(summary["precision_at_20_mean"].as_f64().unwrap_or(0.0))
        ));
        out.push_str(&format!(
            "- MRR (mean): {:.3}\n",
            summary["mrr_mean"].as_f64().unwrap_or(0.0)
        ));
        out.push_str(&format!(
            "- Strong coverage rate: {}\n",
            fmt_pct(summary["strong_coverage_rate"].as_f64().unwrap_or(0.0))
        ));

        if let Some(by_tag) = summary.get("by_tag").and_then(|v| v.as_object()) {
            if !by_tag.is_empty() {
                out.push_str("\n## By tag (mean)\n\n");
                out.push_str("| Tag | Cases | p@5 | p@10 | p@20 | MRR | Strong cov |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
                for (tag, val) in by_tag.iter() {
                    let cases = val.get("cases").and_then(|v| v.as_i64()).unwrap_or(0);
                    let p5 = val.get("precision_at_5_mean").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let p10 = val.get("precision_at_10_mean").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let p20 = val.get("precision_at_20_mean").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let mrr = val.get("mrr_mean").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let cov = val.get("strong_coverage_rate").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    out.push_str(&format!(
                        "| `{}` | {} | {} | {} | {} | {:.3} | {} |\n",
                        tag,
                        cases,
                        fmt_pct(p5),
                        fmt_pct(p10),
                        fmt_pct(p20),
                        mrr,
                        fmt_pct(cov),
                    ));
                }
            }
        }

        out.push_str("\n## Cases\n\n");
        for r in results.iter() {
            out.push_str(&format!("### `{}`\n\n", r.case.id));
            out.push_str(&format!(
                "- Focus: `{}`\n- Window: `{}` → `{}`\n",
                r.case.focus_sensor_id, r.case.start, r.case.end
            ));
            if !r.case.tags.is_empty() {
                out.push_str(&format!("- Tags: {}\n", r.case.tags.join(", ")));
            }
            out.push_str(&format!("- Duration: {}ms\n", r.duration_ms));
            if let Some(err) = r.error.as_deref() {
                out.push_str(&format!("- Status: **FAIL** (`{}`)\n\n", err));
                continue;
            }
            out.push_str(&format!("- Result count: {}\n", r.result_count));
            out.push_str(&format!("- Strong candidates: {}\n", r.strong_count));
            out.push_str(&format!(
                "- Metrics: p@5={} p@10={} p@20={} mrr={:.3}\n",
                fmt_pct(r.metrics.precision_at_5),
                fmt_pct(r.metrics.precision_at_10),
                fmt_pct(r.metrics.precision_at_20),
                r.metrics.mrr
            ));

            let expected: HashSet<&str> = r
                .case
                .expected_related_sensor_ids
                .iter()
                .map(|s| s.as_str())
                .collect();
            out.push_str("- Expected:\n");
            for id in r.case.expected_related_sensor_ids.iter() {
                out.push_str(&format!("  - `{}`\n", id));
            }
            out.push_str("- Top 10:\n");
            for (idx, id) in r.top_sensor_ids.iter().take(10).enumerate() {
                let hit = if expected.contains(id.as_str()) { " ✅" } else { "" };
                out.push_str(&format!("  - {}. `{}`{}\n", idx + 1, id, hit));
            }
            out.push_str("\n");
        }

        out
    };

    if let Some(parent) = args.report_md.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.report_md, md)
        .with_context(|| format!("failed to write {}", args.report_md.display()))?;

    eprintln!("Wrote:");
    eprintln!("- {}", args.report_md.display());
    eprintln!("- {}", args.report_json.display());
    Ok(())
}
