use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_HOT_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/lake/hot";
const DEFAULT_TMP_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/tmp";
const METRICS_DATASET_V1: &str = "metrics/v1";

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Mode {
    Direct,
    Api,
}

#[derive(Debug, Parser)]
#[command(about = "Inspect the TSSE Parquet analysis lake (partitions, shards, watermarks).")]
struct Args {
    /// Mode: direct filesystem access (requires lake file permissions) or controller API.
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

    #[arg(long)]
    hot_path: Option<PathBuf>,
    #[arg(long)]
    cold_path: Option<PathBuf>,
    #[arg(long)]
    tmp_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReplicationState {
    schema_version: u32,
    #[serde(default)]
    last_inserted_at: Option<String>,
    #[serde(default)]
    computed_through_ts: Option<String>,
    #[serde(default)]
    backfill_from_ts: Option<String>,
    #[serde(default)]
    backfill_to_ts: Option<String>,
    #[serde(default)]
    backfill_completed_at: Option<String>,
}

impl Default for ReplicationState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            last_inserted_at: None,
            computed_through_ts: None,
            backfill_from_ts: None,
            backfill_to_ts: None,
            backfill_completed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LakeManifest {
    schema_version: u32,
    #[serde(default)]
    datasets: BTreeMap<String, DatasetManifest>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DatasetManifest {
    #[serde(default)]
    partitions: BTreeMap<String, PartitionManifest>,
    #[serde(default)]
    computed_through_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PartitionManifest {
    location: String,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    last_compacted_at: Option<String>,
    #[serde(default)]
    file_count: Option<u32>,
}

impl Default for LakeManifest {
    fn default() -> Self {
        Self {
            schema_version: 1,
            datasets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct LakeInspection {
    hot_path: String,
    cold_path: Option<String>,
    datasets: BTreeMap<String, DatasetInspection>,
    replication: ReplicationState,
}

#[derive(Debug, Clone, Serialize)]
struct DatasetInspection {
    partitions: BTreeMap<String, PartitionInspection>,
    computed_through_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PartitionInspection {
    location: String,
    shards: BTreeMap<String, ShardInspection>,
}

#[derive(Debug, Clone, Serialize)]
struct ShardInspection {
    parquet_files: u64,
    total_bytes: u64,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

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
    Ok(headers)
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

fn inspect_dataset(
    hot_root: PathBuf,
    cold_root: Option<PathBuf>,
    manifest: Option<&DatasetManifest>,
) -> Result<DatasetInspection> {
    let mut partitions: BTreeMap<String, PartitionInspection> = BTreeMap::new();

    if let Some(ds_manifest) = manifest {
        for (date, partition) in &ds_manifest.partitions {
            let location = partition.location.as_str();
            let path = match location {
                "cold" => cold_root
                    .as_ref()
                    .unwrap_or(&hot_root)
                    .join(format!("date={}", date)),
                _ => hot_root.join(format!("date={}", date)),
            };
            partitions.insert(
                date.to_string(),
                inspect_partition(path, location.to_string())?,
            );
        }
    }

    if hot_root.exists() {
        for entry in std::fs::read_dir(&hot_root)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !path.is_dir() || !name.starts_with("date=") {
                continue;
            }
            let date = name.trim_start_matches("date=").to_string();
            if !partitions.contains_key(&date) {
                partitions.insert(date, inspect_partition(path, "hot".to_string())?);
            }
        }
    }

    if let Some(cold_root) = cold_root {
        if cold_root.exists() {
            for entry in std::fs::read_dir(&cold_root)? {
                let entry = entry?;
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if !path.is_dir() || !name.starts_with("date=") {
                    continue;
                }
                let date = name.trim_start_matches("date=").to_string();
                if !partitions.contains_key(&date) {
                    partitions.insert(date, inspect_partition(path, "cold".to_string())?);
                }
            }
        }
    }

    Ok(DatasetInspection {
        partitions,
        computed_through_ts: manifest.and_then(|m| m.computed_through_ts.clone()),
    })
}

fn inspect_partition(path: PathBuf, location: String) -> Result<PartitionInspection> {
    let mut shards: BTreeMap<String, ShardInspection> = BTreeMap::new();
    if path.exists() {
        for entry in std::fs::read_dir(&path)? {
            let entry = entry?;
            let shard_dir = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if !shard_dir.is_dir() || !name.starts_with("shard=") {
                continue;
            }

            let mut parquet_files = 0u64;
            let mut total_bytes = 0u64;
            for file in std::fs::read_dir(&shard_dir)? {
                let file = file?;
                let path = file.path();
                if path.extension().and_then(|v| v.to_str()) != Some("parquet") {
                    continue;
                }
                parquet_files += 1;
                if let Ok(meta) = file.metadata() {
                    total_bytes += meta.len();
                }
            }

            shards.insert(
                name.trim_start_matches("shard=").to_string(),
                ShardInspection {
                    parquet_files,
                    total_bytes,
                },
            );
        }
    }
    Ok(PartitionInspection { location, shards })
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if matches!(args.mode, Mode::Api) {
        let token = read_api_token(&args)?;
        let headers = api_headers(&token)?;
        let base_url = args.api_base_url.trim_end_matches('/').to_string();
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let created: serde_json::Value = http
            .post(format!("{}/api/analysis/jobs", base_url))
            .headers(headers.clone())
            .json(&serde_json::json!({
                "job_type": "lake_inspect_v1",
                "params": {},
                "dedupe": false
            }))
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
        for _ in 0..500 {
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
                "failed" => anyhow::bail!("lake_inspect_v1 job failed (job_id={})", job_id),
                "canceled" => anyhow::bail!("lake_inspect_v1 job canceled (job_id={})", job_id),
                _ => {}
            }
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        }
        anyhow::ensure!(
            completed,
            "lake_inspect_v1 job timed out (job_id={})",
            job_id
        );

        let result: serde_json::Value = http
            .get(format!("{}/api/analysis/jobs/{}/result", base_url, job_id))
            .headers(headers)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("failed to parse get_job_result response")?;

        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    let hot_path = args
        .hot_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_HOT_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HOT_PATH));
    let cold_path = args
        .cold_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_COLD_PATH"));
    let _tmp_path = args
        .tmp_path
        .or_else(|| env_path("CORE_ANALYSIS_TMP_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TMP_PATH));

    let replication = read_replication_state(&hot_path)?;
    let manifest = read_manifest(&hot_path).unwrap_or_default();

    let mut datasets = BTreeMap::new();
    datasets.insert(
        METRICS_DATASET_V1.to_string(),
        inspect_dataset(
            hot_path.join(METRICS_DATASET_V1),
            cold_path.as_ref().map(|root| root.join(METRICS_DATASET_V1)),
            manifest.datasets.get(METRICS_DATASET_V1),
        )?,
    );

    let inspection = LakeInspection {
        hot_path: hot_path.display().to_string(),
        cold_path: cold_path.map(|path| path.display().to_string()),
        datasets,
        replication,
    };
    println!("{}", serde_json::to_string_pretty(&inspection)?);
    Ok(())
}
