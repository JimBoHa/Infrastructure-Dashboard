use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::{ArgAction, Parser, ValueEnum};
use core_server_rs::services::analysis::lake::AnalysisLakeConfig;
use core_server_rs::services::analysis::parquet_duckdb::{DuckDbQueryService, MetricsBucketRow};
use core_server_rs::services::analysis::qdrant::QdrantService;
use core_server_rs::services::analysis::tsse::candidate_gen::{self, FocusSensorMeta};
use core_server_rs::services::analysis::tsse::embeddings::{
    compute_sensor_embeddings, TsseEmbeddingConfig,
};
use core_server_rs::services::analysis::tsse::types::TsseCandidateFiltersV1;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::Url;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration as StdDuration, Instant};

const DEFAULT_HOT_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/lake/hot";
const DEFAULT_TMP_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/tmp";
const API_MAX_WINDOW_HOURS: i64 = 24 * 365;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InputMode {
    Lake,
    Api,
}

#[derive(Debug, Parser)]
#[command(about = "Evaluate TSSE candidate generation recall on a curated set of related pairs.")]
struct Args {
    /// Qdrant base URL (controller default: http://127.0.0.1:6333)
    #[arg(long, default_value = "http://127.0.0.1:6333")]
    qdrant_url: String,

    /// Source for focus-series data: direct Parquet lake reads, or core-server `/api/metrics/query`.
    #[arg(long, value_enum, default_value_t = InputMode::Lake)]
    input_mode: InputMode,

    /// Core-server base URL used when `input_mode=api`.
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    api_base_url: String,

    /// API token string (Bearer). Optional; keep auth support for metrics endpoints.
    #[arg(long)]
    auth_token: Option<String>,

    /// File containing a single-line API token string (Bearer).
    #[arg(long)]
    auth_token_file: Option<PathBuf>,

    /// Hot lake root (contains metrics/v1/date=YYYY-MM-DD/shard=NN/*.parquet)
    #[arg(long)]
    hot_path: Option<PathBuf>,

    /// Cold lake root (optional)
    #[arg(long)]
    cold_path: Option<PathBuf>,

    /// DuckDB temp/scratch root
    #[arg(long)]
    tmp_path: Option<PathBuf>,

    /// Number of shards (must match CORE_ANALYSIS_LAKE_SHARDS for the dataset)
    #[arg(long, default_value_t = 16)]
    shards: u32,

    /// RFC3339 start timestamp. Default: now - 90d.
    #[arg(long)]
    start: Option<String>,

    /// RFC3339 end timestamp (inclusive). Default: now.
    #[arg(long)]
    end: Option<String>,

    /// Bucket interval (seconds) used to compute embeddings (matches job path).
    #[arg(long, default_value_t = 60)]
    interval_seconds: i64,

    /// Require candidate interval_seconds to match the focus interval.
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    match_interval: bool,

    /// Candidate limit for ANN union query (higher increases work).
    #[arg(long, default_value_t = 150)]
    candidate_limit: u32,

    /// Minimum ANN pool size before widening stops.
    #[arg(long, default_value_t = 150)]
    min_pool: u32,

    /// Apply same-unit filter (requires --unit or a pairs-file that doesn't need unit filtering).
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    same_unit_only: bool,

    /// Apply same-type filter (requires --sensor-type).
    #[arg(long, default_value_t = false, action = ArgAction::Set)]
    same_type_only: bool,

    /// Unit label used when same_unit_only=true (bench default: "unit").
    #[arg(long)]
    unit: Option<String>,

    /// Sensor type label used when same_type_only=true (bench default: "bench").
    #[arg(long)]
    sensor_type: Option<String>,

    /// Filter candidates by is_derived payload (true/false).
    #[arg(long)]
    is_derived: Option<bool>,

    /// Filter candidates by is_public_provider payload (true/false).
    #[arg(long)]
    is_public_provider: Option<bool>,

    /// Optional file with sensor IDs (one per line).
    /// If omitted, IDs are generated from --sensor-id-prefix + --sensor-count.
    #[arg(long)]
    sensor_ids_file: Option<PathBuf>,

    /// Sensor id prefix used for generation/parsing (dataset gen default: "bench-sensor").
    #[arg(long, default_value = "bench-sensor")]
    sensor_id_prefix: String,

    /// Sensor count used when generating IDs.
    #[arg(long, default_value_t = 1_000)]
    sensor_count: u32,

    /// Cluster size used to generate ground truth pairs when --pairs-file is not provided.
    #[arg(long, default_value_t = 10)]
    cluster_size: u32,

    /// Optional CSV file of curated pairs: "focus_sensor_id,candidate_sensor_id" per line.
    #[arg(long)]
    pairs_file: Option<PathBuf>,

    /// Number of focus sensors to sample for evaluation.
    #[arg(long, default_value_t = 50)]
    focus_sample: u32,

    /// RNG seed for deterministic sampling.
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Minimum mean recall required to pass (0..1).
    #[arg(long, default_value_t = 0.6)]
    min_mean_recall: f64,

    /// Recall@K values to report (comma-separated). Defaults to: 10,25,50,100,candidate_limit.
    #[arg(long, value_delimiter = ',')]
    recall_k: Vec<u32>,

    /// K used for pass/fail gate (defaults to candidate_limit).
    #[arg(long)]
    min_recall_k: Option<u32>,

    /// Output report path (Markdown). Recommended under reports/.
    #[arg(long)]
    report: PathBuf,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
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

#[derive(Debug, Clone, Deserialize)]
struct MetricsResponse {
    #[serde(default)]
    series: Vec<MetricSeries>,
    #[serde(default)]
    next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetricSeries {
    sensor_id: String,
    #[serde(default)]
    points: Vec<MetricPoint>,
}

#[derive(Debug, Clone, Deserialize)]
struct MetricPoint {
    timestamp: String,
    value: f64,
    samples: i64,
}

fn read_auth_token(args: &Args) -> Result<Option<String>> {
    if let Some(token) = args
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Ok(Some(token.to_string()));
    }
    if let Some(path) = args.auth_token_file.as_ref() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let token = raw.trim();
        if !token.is_empty() {
            return Ok(Some(token.to_string()));
        }
    }
    Ok(None)
}

fn build_auth_headers(token: Option<&str>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    if let Some(token) = token.map(str::trim).filter(|v| !v.is_empty()) {
        let value = if token.to_ascii_lowercase().starts_with("bearer ") {
            token.to_string()
        } else {
            format!("Bearer {}", token)
        };
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&value)?);
    }
    Ok(headers)
}

async fn fetch_bucketed_series_via_api_window(
    client: &reqwest::Client,
    api_base_url: &Url,
    headers: &HeaderMap,
    sensor_id: &str,
    start: DateTime<Utc>,
    end_inclusive: DateTime<Utc>,
    interval_seconds: i64,
) -> Result<Vec<MetricsBucketRow>> {
    let mut cursor: Option<String> = None;
    let mut out: Vec<MetricsBucketRow> = Vec::new();

    for _ in 0..500 {
        let mut url = api_base_url
            .join("/api/metrics/query")
            .with_context(|| format!("invalid api_base_url: {}", api_base_url))?;
        {
            let mut qp = url.query_pairs_mut();
            qp.append_pair("sensor_ids[]", sensor_id);
            qp.append_pair("start", &start.to_rfc3339());
            qp.append_pair("end", &end_inclusive.to_rfc3339());
            qp.append_pair("interval", &interval_seconds.max(1).to_string());
            if let Some(cursor) = cursor.as_deref() {
                qp.append_pair("cursor", cursor);
            }
        }

        let resp = client
            .get(url)
            .headers(headers.clone())
            .send()
            .await
            .with_context(|| format!("metrics query request failed for {}", sensor_id))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "metrics query failed for {}: {} {}",
                sensor_id,
                status,
                body
            );
        }
        let parsed: MetricsResponse = resp.json().await?;

        for series in parsed.series {
            if series.sensor_id != sensor_id {
                continue;
            }
            for point in series.points {
                let bucket = DateTime::parse_from_rfc3339(point.timestamp.trim())
                    .with_context(|| format!("invalid point timestamp: {}", point.timestamp))?
                    .with_timezone(&Utc);
                out.push(MetricsBucketRow {
                    sensor_id: sensor_id.to_string(),
                    bucket,
                    value: point.value,
                    samples: point.samples,
                });
            }
        }

        if let Some(next) = parsed.next_cursor {
            if cursor.as_deref() == Some(next.as_str()) {
                break;
            }
            cursor = Some(next);
        } else {
            break;
        }
    }

    out.sort_by(|a, b| a.bucket.cmp(&b.bucket));
    out.dedup_by(|a, b| a.bucket == b.bucket);
    Ok(out)
}

async fn fetch_bucketed_series_via_api(
    client: &reqwest::Client,
    api_base_url: &Url,
    headers: &HeaderMap,
    sensor_id: &str,
    start: DateTime<Utc>,
    end_inclusive: DateTime<Utc>,
    interval_seconds: i64,
) -> Result<Vec<MetricsBucketRow>> {
    if end_inclusive <= start {
        return Ok(vec![]);
    }
    let max_window = Duration::hours(API_MAX_WINDOW_HOURS);
    if end_inclusive - start <= max_window {
        return fetch_bucketed_series_via_api_window(
            client,
            api_base_url,
            headers,
            sensor_id,
            start,
            end_inclusive,
            interval_seconds,
        )
        .await;
    }

    let mut out: Vec<MetricsBucketRow> = Vec::new();
    let mut window_start = start;
    while window_start <= end_inclusive {
        let window_end = std::cmp::min(window_start + max_window, end_inclusive);
        let mut chunk = fetch_bucketed_series_via_api_window(
            client,
            api_base_url,
            headers,
            sensor_id,
            window_start,
            window_end,
            interval_seconds,
        )
        .await?;
        out.append(&mut chunk);

        let next_start = window_end + Duration::microseconds(1);
        if next_start <= window_start {
            break;
        }
        window_start = next_start;
    }

    out.sort_by(|a, b| a.bucket.cmp(&b.bucket));
    out.dedup_by(|a, b| a.sensor_id == b.sensor_id && a.bucket == b.bucket);
    Ok(out)
}

fn read_sensor_ids(args: &Args) -> Result<Vec<String>> {
    if let Some(path) = args.sensor_ids_file.as_ref() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut ids: Vec<String> = raw
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect();
        ids.sort();
        ids.dedup();
        anyhow::ensure!(!ids.is_empty(), "sensor_ids_file contained no ids");
        return Ok(ids);
    }

    let mut ids = Vec::new();
    for idx in 0..args.sensor_count.max(1) {
        ids.push(format!("{}-{:04}", args.sensor_id_prefix.trim(), idx));
    }
    Ok(ids)
}

fn parse_index(prefix: &str, sensor_id: &str) -> Option<u32> {
    let prefix = prefix.trim();
    let sensor_id = sensor_id.trim();
    let expected = format!("{}-", prefix);
    let suffix = sensor_id.strip_prefix(&expected)?;
    suffix.parse::<u32>().ok()
}

fn build_ground_truth(args: &Args, sensor_ids: &[String]) -> Result<HashMap<String, Vec<String>>> {
    if let Some(path) = args.pairs_file.as_ref() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for (line_idx, line) in raw.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let (left, right) = trimmed.split_once(',').with_context(|| {
                format!("invalid pairs_file line {}: {}", line_idx + 1, trimmed)
            })?;
            let focus = left.trim();
            let cand = right.trim();
            if focus.is_empty() || cand.is_empty() {
                continue;
            }
            out.entry(focus.to_string())
                .or_default()
                .push(cand.to_string());
        }
        for list in out.values_mut() {
            list.sort();
            list.dedup();
        }
        anyhow::ensure!(!out.is_empty(), "pairs_file produced no pairs");
        return Ok(out);
    }

    let cluster_size = args.cluster_size.max(1);
    let mut by_cluster: HashMap<u32, Vec<String>> = HashMap::new();
    for id in sensor_ids {
        if let Some(idx) = parse_index(&args.sensor_id_prefix, id) {
            by_cluster
                .entry(idx / cluster_size)
                .or_default()
                .push(id.clone());
        }
    }
    for list in by_cluster.values_mut() {
        list.sort();
        list.dedup();
    }

    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for (_cluster_id, members) in by_cluster {
        if members.len() <= 1 {
            continue;
        }
        for focus in members.iter() {
            let related: Vec<String> = members.iter().filter(|id| *id != focus).cloned().collect();
            out.insert(focus.clone(), related);
        }
    }

    anyhow::ensure!(
        !out.is_empty(),
        "cluster-based truth generation produced no related pairs (cluster_size too small?)"
    );
    Ok(out)
}

fn percentile(samples: &[f64], pct: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let pct = pct.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted.get(idx).copied()
}

fn percentile_ms(samples: &[u64], pct: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let pct = pct.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted.get(idx).copied()
}

fn normalize_recall_ks(candidate_limit: u32, ks: &[u32]) -> Vec<u32> {
    let mut out: Vec<u32> = if ks.is_empty() {
        vec![10, 25, 50, 100, candidate_limit.max(1)]
    } else {
        ks.to_vec()
    };
    let limit = candidate_limit.max(1);
    for k in out.iter_mut() {
        if *k < 1 {
            *k = 1;
        }
        if *k > limit {
            *k = limit;
        }
    }
    out.push(limit);
    out.sort_unstable();
    out.dedup();
    out
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_recall_ks_clamps_and_dedups() {
        let ks = normalize_recall_ks(5, &[0, 3, 10, 5, 5]);
        assert_eq!(ks, vec![1, 3, 5]);
    }

    #[test]
    fn mean_handles_empty() {
        assert_eq!(mean(&[]), 0.0);
        assert_eq!(mean(&[0.5, 1.0]), 0.75);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

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

    let end_inclusive = args
        .end
        .as_deref()
        .map(|v| parse_ts("end", v))
        .transpose()?
        .unwrap_or_else(Utc::now);
    let start = args
        .start
        .as_deref()
        .map(|v| parse_ts("start", v))
        .transpose()?
        .unwrap_or_else(|| end_inclusive - Duration::days(90));
    anyhow::ensure!(end_inclusive > start, "end must be after start");
    let end = end_inclusive + Duration::microseconds(1);

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

    let http = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()
        .context("failed to build http client")?;

    let api_base_url = Url::parse(args.api_base_url.trim())
        .with_context(|| format!("invalid api_base_url: {}", args.api_base_url))?;
    let token = read_auth_token(&args)?;
    let auth_headers = build_auth_headers(token.as_deref())?;

    let qdrant = QdrantService::new(args.qdrant_url.clone(), http.clone());
    qdrant
        .ensure_schema()
        .await
        .context("qdrant ensure_schema failed")?;

    let svc = DuckDbQueryService::new(tmp_path, 1);
    let embedding_config = TsseEmbeddingConfig::default();

    let mut focus_meta = FocusSensorMeta {
        node_id: None,
        sensor_type: None,
        unit: None,
    };

    if args.same_unit_only {
        let unit = args
            .unit
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .context("same_unit_only=true requires --unit")?;
        focus_meta.unit = Some(unit.to_string());
    }
    if args.same_type_only {
        let sensor_type = args
            .sensor_type
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .context("same_type_only=true requires --sensor-type")?;
        focus_meta.sensor_type = Some(sensor_type.to_string());
    }

    let sensor_ids = read_sensor_ids(&args)?;
    let truth = build_ground_truth(&args, &sensor_ids)?;
    let truth_focuses_total = truth.len() as u64;
    let truth_pairs_total: u64 = truth.values().map(|v| v.len() as u64).sum();
    let (truth_mode, pairs_file_display) = match args.pairs_file.as_ref() {
        Some(path) => ("pairs_file", Some(path.display().to_string())),
        None => ("cluster_generated", None),
    };

    let mut focus_ids: Vec<String> = truth.keys().cloned().collect();
    focus_ids.sort();
    let target_focuses = (args.focus_sample as usize).max(1).min(focus_ids.len());
    let mut rng = StdRng::seed_from_u64(args.seed);
    focus_ids.shuffle(&mut rng);
    focus_ids.truncate(target_focuses);
    focus_ids.sort();

    let filters = TsseCandidateFiltersV1 {
        same_node_only: false,
        same_unit_only: args.same_unit_only,
        same_type_only: args.same_type_only,
        interval_seconds: if args.match_interval {
            Some(args.interval_seconds)
        } else {
            None
        },
        is_derived: args.is_derived,
        is_public_provider: args.is_public_provider,
        exclude_sensor_ids: Vec::new(),
    };

    let mut recall_ks = normalize_recall_ks(args.candidate_limit.max(1), &args.recall_k);
    let mut primary_k = args.min_recall_k.unwrap_or(args.candidate_limit);
    if primary_k < 1 {
        primary_k = 1;
    }
    if primary_k > args.candidate_limit.max(1) {
        primary_k = args.candidate_limit.max(1);
    }
    if !recall_ks.contains(&primary_k) {
        recall_ks.push(primary_k);
        recall_ks.sort_unstable();
        recall_ks.dedup();
    }

    let mut per_focus_recall: Vec<(String, f64)> = Vec::new();
    let mut per_focus_cand_ms: Vec<u64> = Vec::new();
    let mut recall_by_k: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    let mut skipped: u64 = 0;

    for focus in focus_ids.iter() {
        let related = truth.get(focus).cloned().unwrap_or_default();
        if related.is_empty() {
            skipped += 1;
            continue;
        }
        let related_set: HashSet<String> = related.iter().cloned().collect();

        let focus_rows = match args.input_mode {
            InputMode::Lake => svc
                .read_metrics_buckets_from_lake(
                    &lake,
                    start,
                    end,
                    vec![focus.clone()],
                    args.interval_seconds.max(1),
                )
                .await
                .with_context(|| format!("duckdb read failed for focus {}", focus))?
                .into_iter()
                .filter(|r| r.sensor_id == *focus)
                .collect::<Vec<_>>(),
            InputMode::Api => fetch_bucketed_series_via_api(
                &http,
                &api_base_url,
                &auth_headers,
                focus,
                start,
                end_inclusive,
                args.interval_seconds.max(1),
            )
            .await
            .with_context(|| format!("api metrics query failed for focus {}", focus))?,
        };
        let Some(focus_embeddings) = compute_sensor_embeddings(&focus_rows, &embedding_config)
        else {
            skipped += 1;
            continue;
        };

        let cand_started = Instant::now();
        let candidates = candidate_gen::generate_candidates(
            &qdrant,
            focus,
            &focus_meta,
            &focus_embeddings,
            &filters,
            args.min_pool.max(1),
            args.candidate_limit.max(1),
            &embedding_config,
        )
        .await
        .with_context(|| format!("candidate gen failed for focus {}", focus))?;
        per_focus_cand_ms.push(cand_started.elapsed().as_millis() as u64);

        let candidate_ids: Vec<String> = candidates.into_iter().map(|c| c.sensor_id).collect();
        let denom = related_set.len() as f64;
        for k in recall_ks.iter().copied() {
            let found = candidate_ids
                .iter()
                .take(k as usize)
                .filter(|id| related_set.contains(*id))
                .count() as f64;
            let recall = if denom > 0.0 { found / denom } else { 0.0 };
            recall_by_k.entry(k).or_default().push(recall);
            if k == primary_k {
                per_focus_recall.push((focus.clone(), recall));
            }
        }
    }

    let primary_recalls = recall_by_k.get(&primary_k).cloned().unwrap_or_default();
    let mean_recall = mean(&primary_recalls);
    let p50_recall = percentile(&primary_recalls, 0.50).unwrap_or(0.0);
    let p10_recall = percentile(&primary_recalls, 0.10).unwrap_or(0.0);
    let p90_recall = percentile(&primary_recalls, 0.90).unwrap_or(0.0);

    let cand_p50 = percentile_ms(&per_focus_cand_ms, 0.50);
    let cand_p95 = percentile_ms(&per_focus_cand_ms, 0.95);

    let pass = mean_recall >= args.min_mean_recall.clamp(0.0, 1.0);

    let mut report = String::new();
    report.push_str("# TSSE Candidate Recall Eval\n\n");
    report.push_str(&format!(
        "- Date: {}\n- input_mode: `{:?}`\n- Qdrant: `{}`\n- api_base_url: `{}`\n- Window: `{}` → `{}`\n- Interval: `{}` seconds\n- match_interval: `{}`\n- candidate_limit: `{}` (min_pool `{}`)\n- same_unit_only: `{}`\n- same_type_only: `{}`\n- is_derived: `{}`\n- is_public_provider: `{}`\n- ground_truth_mode: `{}`\n- pairs_file: `{}`\n- truth_pairs: `{}` (focuses `{}`)\n- focus_sample: `{}`\n- skipped_focuses: `{}`\n\n",
        Utc::now().to_rfc3339(),
        args.input_mode,
        args.qdrant_url,
        args.api_base_url,
        start.to_rfc3339(),
        end_inclusive.to_rfc3339(),
        args.interval_seconds,
        args.match_interval,
        args.candidate_limit,
        args.min_pool,
        args.same_unit_only,
        args.same_type_only,
        args.is_derived
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        args.is_public_provider
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        truth_mode,
        pairs_file_display.unwrap_or_else(|| "—".to_string()),
        truth_pairs_total,
        truth_focuses_total,
        target_focuses,
        skipped,
    ));

    report.push_str("## Recall@K summary\n\n");
    report.push_str("| K | Mean | P10 | P50 | P90 |\n| ---: | ---: | ---: | ---: | ---: |\n");
    for (k, values) in recall_by_k.iter() {
        let mean_val = mean(values);
        let p10 = percentile(values, 0.10).unwrap_or(0.0);
        let p50 = percentile(values, 0.50).unwrap_or(0.0);
        let p90 = percentile(values, 0.90).unwrap_or(0.0);
        report.push_str(&format!(
            "| {} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            k, mean_val, p10, p50, p90
        ));
    }
    report.push('\n');

    report.push_str("## Pass/Fail gate\n\n");
    report.push_str(&format!(
        "- mean recall@K={} : **{:.3}** (min required {:.3}) → **{}**\n- p10/p50/p90: `{:.3}` / `{:.3}` / `{:.3}`\n\n",
        primary_k,
        mean_recall,
        args.min_mean_recall,
        if pass { "PASS" } else { "FAIL" },
        p10_recall,
        p50_recall,
        p90_recall,
    ));

    report.push_str("## Candidate generation wall time (client-side)\n\n");
    report.push_str(&format!(
        "- p50: `{}` ms\n- p95: `{}` ms\n\n",
        cand_p50
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        cand_p95
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
    ));

    report.push_str("## Sampled focus recalls\n\n");
    report.push_str("| Focus sensor | Recall |\n| --- | ---: |\n");
    let mut sample = per_focus_recall.clone();
    sample.sort_by(|a, b| a.0.cmp(&b.0));
    for (focus, recall) in sample.into_iter().take(30) {
        report.push_str(&format!("| `{}` | `{:.3}` |\n", focus, recall));
    }
    report.push_str("\n## Notes\n\n");
    report.push_str("- Ground truth is derived from `--pairs-file` when provided; otherwise it is generated by grouping `--sensor-id-prefix-####` into clusters of size `--cluster-size`.\n");
    report.push_str("- This harness evaluates ANN candidate recall only; episodic scoring is validated separately via `tsse_bench`.\n");

    if let Some(parent) = args.report.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.report, report)
        .with_context(|| format!("failed to write {}", args.report.display()))?;

    println!(
        "tsse_recall_eval: wrote report to {} (mean_recall={:.3})",
        args.report.display(),
        mean_recall
    );

    if !pass {
        anyhow::bail!("candidate recall below threshold (see report)");
    }

    Ok(())
}
