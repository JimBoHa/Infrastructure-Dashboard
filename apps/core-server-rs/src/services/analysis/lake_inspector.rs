use crate::services::analysis::lake::{
    read_manifest, read_replication_state, AnalysisLakeConfig, DatasetManifest, METRICS_DATASET_V1,
};
use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct LakeInspection {
    pub hot_path: String,
    pub cold_path: Option<String>,
    pub datasets: BTreeMap<String, DatasetInspection>,
    pub replication: crate::services::analysis::lake::ReplicationState,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatasetInspection {
    pub partitions: BTreeMap<String, PartitionInspection>,
    pub computed_through_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartitionInspection {
    pub location: String,
    pub shards: BTreeMap<String, ShardInspection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShardInspection {
    pub parquet_files: u64,
    pub total_bytes: u64,
}

pub fn inspect(config: &AnalysisLakeConfig) -> Result<LakeInspection> {
    let manifest = read_manifest(config).unwrap_or_default();
    let dataset_manifest = manifest.datasets.get(METRICS_DATASET_V1);
    let mut datasets = BTreeMap::new();
    datasets.insert(
        METRICS_DATASET_V1.to_string(),
        inspect_dataset(config, METRICS_DATASET_V1, dataset_manifest)?,
    );
    Ok(LakeInspection {
        hot_path: config.hot_path.display().to_string(),
        cold_path: config.cold_path.as_ref().map(|p| p.display().to_string()),
        datasets,
        replication: read_replication_state(config)?,
    })
}

fn inspect_dataset(
    config: &AnalysisLakeConfig,
    dataset: &str,
    manifest: Option<&DatasetManifest>,
) -> Result<DatasetInspection> {
    let mut partitions: BTreeMap<String, PartitionInspection> = BTreeMap::new();

    if let Some(ds_manifest) = manifest {
        for (date, partition) in &ds_manifest.partitions {
            let location = partition.location.as_str();
            let path = match location {
                "cold" => config
                    .dataset_root_cold(dataset)
                    .unwrap_or_else(|| config.dataset_root_hot(dataset))
                    .join(format!("date={}", date)),
                _ => config
                    .dataset_root_hot(dataset)
                    .join(format!("date={}", date)),
            };
            partitions.insert(
                date.to_string(),
                inspect_partition(path, location.to_string())?,
            );
        }
    }

    let hot_root = config.dataset_root_hot(dataset);
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

    if let Some(cold_root) = config.dataset_root_cold(dataset) {
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
