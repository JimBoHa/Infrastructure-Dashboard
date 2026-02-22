use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Parser, ValueEnum};
use duckdb::Connection;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
use xxhash_rust::xxh3::xxh3_64;

const DEFAULT_HOT_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/lake/hot";
const DEFAULT_TMP_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/tmp";
const METRICS_DATASET_V1: &str = "metrics/v1";

#[derive(Debug, Clone, ValueEnum)]
enum Mode {
    Points,
    Buckets,
}

#[derive(Debug, Parser)]
#[command(about = "Benchmark DuckDB reads over the TSSE Parquet lake.")]
struct Args {
    #[arg(long)]
    hot_path: Option<PathBuf>,
    #[arg(long)]
    cold_path: Option<PathBuf>,
    #[arg(long)]
    tmp_path: Option<PathBuf>,
    #[arg(long, default_value = METRICS_DATASET_V1)]
    dataset: String,
    #[arg(long)]
    sensor_ids: String,
    #[arg(long)]
    start: String,
    #[arg(long)]
    end: String,
    #[arg(long, default_value_t = 16)]
    shards: u32,
    #[arg(long, value_enum, default_value_t = Mode::Points)]
    mode: Mode,
    #[arg(long)]
    interval_seconds: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LakeManifest {
    #[serde(default)]
    datasets: BTreeMap<String, DatasetManifest>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct DatasetManifest {
    #[serde(default)]
    partitions: BTreeMap<String, PartitionManifest>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct PartitionManifest {
    location: String,
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn read_manifest(hot_path: &Path) -> Result<LakeManifest> {
    let path = hot_path.join("_state/manifest.json");
    if !path.exists() {
        return Ok(LakeManifest {
            datasets: BTreeMap::new(),
        });
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("failed to read manifest at {}", path.display()))?;
    let parsed: LakeManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(parsed)
}

fn list_dates_in_range(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<NaiveDate> {
    if end <= start {
        return vec![];
    }
    let mut dates = Vec::new();
    let mut cursor = start.date_naive();
    let end_date = end.date_naive();
    while cursor <= end_date {
        dates.push(cursor);
        cursor = cursor.succ_opt().unwrap_or(cursor);
        if cursor == *dates.last().unwrap() {
            break;
        }
    }
    dates
}

fn list_parquet_files_for_range(
    hot_path: &Path,
    cold_path: Option<&Path>,
    dataset: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    shard_set: &BTreeSet<u32>,
) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let dates = list_dates_in_range(start, end);
    let manifest = read_manifest(hot_path).unwrap_or(LakeManifest {
        datasets: BTreeMap::new(),
    });
    let dataset_manifest = manifest.datasets.get(dataset);

    for date in dates {
        let date_key = date.format("%Y-%m-%d").to_string();
        let location = dataset_manifest
            .and_then(|ds| ds.partitions.get(&date_key))
            .map(|p| p.location.clone());

        for shard in shard_set {
            let mut candidate_dirs: Vec<PathBuf> = Vec::new();
            match location.as_deref() {
                Some("cold") => {
                    if let Some(root) = cold_path {
                        candidate_dirs.push(
                            root.join(dataset)
                                .join(format!("date={}", date_key))
                                .join(format!("shard={:02}", shard)),
                        );
                    }
                    candidate_dirs.push(
                        hot_path
                            .join(dataset)
                            .join(format!("date={}", date_key))
                            .join(format!("shard={:02}", shard)),
                    );
                }
                Some("hot") => {
                    candidate_dirs.push(
                        hot_path
                            .join(dataset)
                            .join(format!("date={}", date_key))
                            .join(format!("shard={:02}", shard)),
                    );
                    if let Some(root) = cold_path {
                        candidate_dirs.push(
                            root.join(dataset)
                                .join(format!("date={}", date_key))
                                .join(format!("shard={:02}", shard)),
                        );
                    }
                }
                _ => {
                    candidate_dirs.push(
                        hot_path
                            .join(dataset)
                            .join(format!("date={}", date_key))
                            .join(format!("shard={:02}", shard)),
                    );
                    if let Some(root) = cold_path {
                        candidate_dirs.push(
                            root.join(dataset)
                                .join(format!("date={}", date_key))
                                .join(format!("shard={:02}", shard)),
                        );
                    }
                }
            }

            for dir in candidate_dirs {
                if !dir.exists() {
                    continue;
                }
                for entry in std::fs::read_dir(&dir)
                    .with_context(|| format!("failed to read dir {}", dir.display()))?
                {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().and_then(|v| v.to_str()) == Some("parquet") {
                        out.push(path);
                    }
                }
                break;
            }
        }
    }
    out.sort();
    Ok(out)
}

fn escape_single_quotes(input: String) -> String {
    input.replace('\'', "''")
}

fn main() -> Result<()> {
    let args = Args::parse();

    let hot_path = args
        .hot_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_HOT_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HOT_PATH));
    let cold_path = args
        .cold_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_COLD_PATH"));
    let tmp_path = args
        .tmp_path
        .or_else(|| env_path("CORE_ANALYSIS_TMP_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TMP_PATH));

    let start = DateTime::parse_from_rfc3339(&args.start)
        .with_context(|| format!("invalid start timestamp: {}", args.start))?
        .with_timezone(&Utc);
    let end = DateTime::parse_from_rfc3339(&args.end)
        .with_context(|| format!("invalid end timestamp: {}", args.end))?
        .with_timezone(&Utc);

    let sensor_ids: Vec<String> = args
        .sensor_ids
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    anyhow::ensure!(!sensor_ids.is_empty(), "sensor_ids cannot be empty");

    let shards = args.shards.max(1);
    let mut shard_set = BTreeSet::new();
    for sensor_id in &sensor_ids {
        let shard = (xxh3_64(sensor_id.as_bytes()) % shards as u64) as u32;
        shard_set.insert(shard);
    }

    let parquet_files = list_parquet_files_for_range(
        &hot_path,
        cold_path.as_deref(),
        &args.dataset,
        start,
        end,
        &shard_set,
    )?;
    if parquet_files.is_empty() {
        println!("No parquet files found for requested range/shards.");
        return Ok(());
    }

    let conn = Connection::open_in_memory()?;
    let tmp_dir = tmp_path.join("duckdb");
    std::fs::create_dir_all(&tmp_dir).ok();
    let _ = conn.execute("PRAGMA threads=2", []);
    let _ = conn.execute("PRAGMA enable_progress_bar=false", []);
    let _ = conn.execute(
        &format!(
            "SET temp_directory='{}'",
            escape_single_quotes(tmp_dir.display().to_string())
        ),
        [],
    );

    let files_sql = parquet_files
        .iter()
        .map(|p| format!("'{}'", escape_single_quotes(p.display().to_string())))
        .collect::<Vec<_>>()
        .join(", ");
    let sensors_sql = sensor_ids
        .iter()
        .map(|s| format!("'{}'", escape_single_quotes(s.to_string())))
        .collect::<Vec<_>>()
        .join(", ");

    let start_sql = start.to_rfc3339();
    let end_sql = end.to_rfc3339();
    let sql = match args.mode {
        Mode::Points => format!(
            r#"
            SELECT sensor_id, ts, value, quality
            FROM read_parquet([{files_sql}], hive_partitioning=1, union_by_name=1)
            WHERE sensor_id IN ({sensors_sql})
              AND ts >= '{start_sql}'::TIMESTAMP
              AND ts <= '{end_sql}'::TIMESTAMP
            ORDER BY sensor_id, ts
            "#
        ),
        Mode::Buckets => {
            let interval_seconds = args.interval_seconds.unwrap_or(60).max(1);
            format!(
                r#"
                SELECT
                    sensor_id,
                    CAST(floor(epoch(ts) / {interval_seconds}) * {interval_seconds} AS BIGINT) AS bucket_epoch,
                    avg(value) AS avg_value,
                    count(*) AS samples
                FROM read_parquet([{files_sql}], hive_partitioning=1, union_by_name=1)
                WHERE sensor_id IN ({sensors_sql})
                  AND ts >= '{start_sql}'::TIMESTAMP
                  AND ts < '{end_sql}'::TIMESTAMP
                GROUP BY sensor_id, bucket_epoch
                ORDER BY sensor_id, bucket_epoch
                "#
            )
        }
    };

    let start_time = Instant::now();
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut row_count: u64 = 0;
    while let Some(_row) = rows.next()? {
        row_count += 1;
    }
    let elapsed = start_time.elapsed();
    let elapsed_ms = elapsed.as_millis().max(1) as u128;
    let rows_per_sec = (row_count as f64) / (elapsed_ms as f64 / 1000.0);

    println!(
        "DuckDB bench: mode={:?}, rows={}, files={}, elapsed_ms={}, rows_per_sec={:.2}",
        args.mode,
        row_count,
        parquet_files.len(),
        elapsed_ms,
        rows_per_sec
    );

    Ok(())
}
