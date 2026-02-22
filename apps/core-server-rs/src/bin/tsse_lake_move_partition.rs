use anyhow::{Context, Result};
use chrono::NaiveDate;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DEFAULT_HOT_PATH: &str = "/Users/Shared/FarmDashboard/storage/analysis/lake/hot";
const METRICS_DATASET_V1: &str = "metrics/v1";

#[derive(Debug, Clone, ValueEnum)]
enum TargetLocation {
    Hot,
    Cold,
}

#[derive(Debug, Parser)]
#[command(about = "Move a TSSE lake partition between hot and cold paths safely.")]
struct Args {
    #[arg(long)]
    hot_path: Option<PathBuf>,
    #[arg(long)]
    cold_path: Option<PathBuf>,
    #[arg(long, default_value = METRICS_DATASET_V1)]
    dataset: String,
    #[arg(long)]
    date: String,
    #[arg(long, value_enum)]
    target: TargetLocation,
    #[arg(long, default_value_t = false)]
    apply: bool,
    #[arg(long, default_value_t = false)]
    force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LakeManifest {
    schema_version: u32,
    #[serde(default)]
    datasets: BTreeMap<String, DatasetManifest>,
}

impl Default for LakeManifest {
    fn default() -> Self {
        Self {
            schema_version: 1,
            datasets: BTreeMap::new(),
        }
    }
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
        return Ok(LakeManifest::default());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("failed to read manifest at {}", path.display()))?;
    let parsed: LakeManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(parsed)
}

fn write_manifest(hot_path: &Path, manifest: &LakeManifest) -> Result<()> {
    let path = hot_path.join("_state/manifest.json");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let contents = serde_json::to_vec_pretty(manifest)?;
    let tmp_dir = path.parent().unwrap_or(hot_path);
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    use std::io::Write;
    tmp.write_all(&contents)?;
    tmp.flush()?;
    tmp.persist(&path)
        .map_err(|err| anyhow::anyhow!("failed to persist manifest: {err}"))?;
    Ok(())
}

fn set_partition_location(
    manifest: &mut LakeManifest,
    dataset: &str,
    date: &str,
    location: &str,
    file_count: u32,
) {
    let ds = manifest.datasets.entry(dataset.to_string()).or_default();
    let entry = ds
        .partitions
        .entry(date.to_string())
        .or_insert_with(|| PartitionManifest {
            location: location.to_string(),
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
            last_compacted_at: None,
            file_count: None,
        });
    entry.location = location.to_string();
    entry.updated_at = Some(chrono::Utc::now().to_rfc3339());
    entry.file_count = Some(file_count);
}

fn count_parquet_files_in_partition(partition_dir: &Path) -> Result<u32> {
    if !partition_dir.exists() {
        return Ok(0);
    }
    let mut count: u32 = 0;
    for shard_entry in std::fs::read_dir(partition_dir)? {
        let shard_entry = shard_entry?;
        let shard_dir = shard_entry.path();
        if !shard_dir.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&shard_dir)? {
            let file = file?;
            let path = file.path();
            if path.extension().and_then(|v| v.to_str()) == Some("parquet") {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target)?;
        } else {
            std::fs::copy(&path, &target).with_context(|| {
                format!("failed to copy {} -> {}", path.display(), target.display())
            })?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();
    let date = NaiveDate::parse_from_str(&args.date, "%Y-%m-%d")
        .with_context(|| format!("invalid date (expected YYYY-MM-DD): {}", args.date))?;
    let dataset = args.dataset.trim();
    anyhow::ensure!(!dataset.is_empty(), "dataset cannot be empty");

    let hot_path = args
        .hot_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_HOT_PATH"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_HOT_PATH));
    let cold_path = args
        .cold_path
        .or_else(|| env_path("CORE_ANALYSIS_LAKE_COLD_PATH"));

    let hot_root = hot_path.join(dataset);
    let cold_root = cold_path.as_ref().map(|path| path.join(dataset));

    let date_key = format!("date={}", date.format("%Y-%m-%d"));
    let (source_root, target_root, target_label) = match args.target {
        TargetLocation::Hot => {
            let cold_root = cold_root
                .as_ref()
                .context("cold_path is required when target=hot")?;
            (cold_root, &hot_root, "hot")
        }
        TargetLocation::Cold => {
            let cold_root = cold_root
                .as_ref()
                .context("cold_path is required when target=cold")?;
            (&hot_root, cold_root, "cold")
        }
    };

    let source_partition = source_root.join(&date_key);
    let target_partition = target_root.join(&date_key);

    if !source_partition.exists() {
        anyhow::bail!(
            "source partition does not exist: {}",
            source_partition.display()
        );
    }

    if target_partition.exists() {
        if args.force {
            std::fs::remove_dir_all(&target_partition).with_context(|| {
                format!(
                    "failed to remove existing target {}",
                    target_partition.display()
                )
            })?;
        } else {
            anyhow::bail!(
                "target partition already exists: {} (use --force to overwrite)",
                target_partition.display()
            );
        }
    }

    println!(
        "Plan: move {} -> {} (dataset={}, date={}, target={:?})",
        source_partition.display(),
        target_partition.display(),
        dataset,
        args.date,
        args.target
    );

    if !args.apply {
        println!("Dry-run only (pass --apply to execute).");
        return Ok(());
    }

    std::fs::create_dir_all(target_root).ok();
    if let Err(err) = std::fs::rename(&source_partition, &target_partition) {
        println!("rename failed ({err}); falling back to copy+delete");
        copy_dir_all(&source_partition, &target_partition)?;
        std::fs::remove_dir_all(&source_partition).ok();
    }

    let file_count = count_parquet_files_in_partition(&target_partition)?;
    let mut manifest = read_manifest(&hot_path).unwrap_or_default();
    set_partition_location(
        &mut manifest,
        dataset,
        &date.format("%Y-%m-%d").to_string(),
        target_label,
        file_count,
    );
    write_manifest(&hot_path, &manifest)?;

    println!(
        "Moved partition to {} with {} parquet files; manifest updated.",
        target_label, file_count
    );
    Ok(())
}
