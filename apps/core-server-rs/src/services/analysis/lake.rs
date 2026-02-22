use anyhow::{Context, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use xxhash_rust::xxh3::xxh3_64;

pub const METRICS_DATASET_V1: &str = "metrics/v1";
pub const LAKE_MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone)]
pub struct AnalysisLakeConfig {
    pub hot_path: PathBuf,
    pub cold_path: Option<PathBuf>,
    pub tmp_path: PathBuf,
    pub shards: u32,
    pub hot_retention_days: u32,
    pub late_window_hours: u32,
    pub replication_interval: std::time::Duration,
    pub replication_lag: std::time::Duration,
}

impl AnalysisLakeConfig {
    pub fn shard_for_sensor_id(&self, sensor_id: &str) -> u32 {
        let shards = self.shards.max(1);
        (xxh3_64(sensor_id.as_bytes()) % shards as u64) as u32
    }

    pub fn dataset_root_hot(&self, dataset: &str) -> PathBuf {
        self.hot_path.join(dataset)
    }

    pub fn dataset_root_cold(&self, dataset: &str) -> Option<PathBuf> {
        self.cold_path.as_ref().map(|root| root.join(dataset))
    }

    pub fn partition_dir_hot(&self, dataset: &str, date: NaiveDate, shard: u32) -> PathBuf {
        self.dataset_root_hot(dataset)
            .join(format!("date={}", date.format("%Y-%m-%d")))
            .join(format!("shard={:02}", shard))
    }

    pub fn partition_dir_cold(
        &self,
        dataset: &str,
        date: NaiveDate,
        shard: u32,
    ) -> Option<PathBuf> {
        self.dataset_root_cold(dataset).map(|root| {
            root.join(format!("date={}", date.format("%Y-%m-%d")))
                .join(format!("shard={:02}", shard))
        })
    }

    pub fn state_dir(&self) -> PathBuf {
        self.hot_path.join("_state")
    }

    pub fn replication_state_path(&self) -> PathBuf {
        self.state_dir().join("replication.json")
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.state_dir().join(LAKE_MANIFEST_FILE)
    }

    pub fn hot_retention_cutoff(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        let days = self.hot_retention_days.max(1) as i64;
        now - Duration::days(days)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicationState {
    pub schema_version: u32,
    #[serde(default)]
    pub last_inserted_at: Option<String>,
    #[serde(default)]
    pub computed_through_ts: Option<String>,
    #[serde(default)]
    pub last_run_at: Option<String>,
    #[serde(default)]
    pub last_run_duration_ms: Option<u64>,
    #[serde(default)]
    pub last_run_row_count: Option<u64>,
    #[serde(default)]
    pub last_run_backlog_seconds: Option<i64>,
    #[serde(default)]
    pub last_run_status: Option<String>,
    #[serde(default)]
    pub last_run_error: Option<String>,
    #[serde(default)]
    pub last_export_start: Option<String>,
    #[serde(default)]
    pub last_export_end: Option<String>,
    #[serde(default)]
    pub last_late_window_hours: Option<u32>,
    #[serde(default)]
    pub backfill_from_ts: Option<String>,
    #[serde(default)]
    pub backfill_to_ts: Option<String>,
    #[serde(default)]
    pub backfill_completed_at: Option<String>,
}

impl Default for ReplicationState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            last_inserted_at: None,
            computed_through_ts: None,
            last_run_at: None,
            last_run_duration_ms: None,
            last_run_row_count: None,
            last_run_backlog_seconds: None,
            last_run_status: None,
            last_run_error: None,
            last_export_start: None,
            last_export_end: None,
            last_late_window_hours: None,
            backfill_from_ts: None,
            backfill_to_ts: None,
            backfill_completed_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LakeManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub datasets: BTreeMap<String, DatasetManifest>,
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
pub struct DatasetManifest {
    #[serde(default)]
    pub partitions: BTreeMap<String, PartitionManifest>,
    #[serde(default)]
    pub computed_through_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionManifest {
    pub location: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub last_compacted_at: Option<String>,
    #[serde(default)]
    pub file_count: Option<u32>,
}

impl PartitionManifest {
    fn new(location: &str) -> Self {
        Self {
            location: location.to_string(),
            updated_at: Some(Utc::now().to_rfc3339()),
            last_compacted_at: None,
            file_count: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionLocation {
    Hot,
    Cold,
}

impl PartitionLocation {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartitionLocation::Hot => "hot",
            PartitionLocation::Cold => "cold",
        }
    }
}

impl LakeManifest {
    pub fn partition_location(&self, dataset: &str, date: NaiveDate) -> Option<String> {
        let key = date.format("%Y-%m-%d").to_string();
        self.datasets
            .get(dataset)
            .and_then(|ds| ds.partitions.get(&key))
            .map(|p| p.location.clone())
    }

    pub fn set_partition_location(&mut self, dataset: &str, date: NaiveDate, location: &str) {
        let key = date.format("%Y-%m-%d").to_string();
        let ds = self.datasets.entry(dataset.to_string()).or_default();
        let entry = ds
            .partitions
            .entry(key)
            .or_insert_with(|| PartitionManifest::new(location));
        entry.location = location.to_string();
        entry.updated_at = Some(Utc::now().to_rfc3339());
    }

    pub fn set_partition_file_count(&mut self, dataset: &str, date: NaiveDate, file_count: u32) {
        let key = date.format("%Y-%m-%d").to_string();
        let ds = self.datasets.entry(dataset.to_string()).or_default();
        let entry = ds
            .partitions
            .entry(key)
            .or_insert_with(|| PartitionManifest::new("unknown"));
        entry.file_count = Some(file_count);
    }

    pub fn set_partition_compacted_at(
        &mut self,
        dataset: &str,
        date: NaiveDate,
        compacted_at: String,
    ) {
        let key = date.format("%Y-%m-%d").to_string();
        let ds = self.datasets.entry(dataset.to_string()).or_default();
        let entry = ds
            .partitions
            .entry(key)
            .or_insert_with(|| PartitionManifest::new("unknown"));
        entry.last_compacted_at = Some(compacted_at);
    }

    pub fn set_dataset_watermark(&mut self, dataset: &str, computed_through_ts: Option<String>) {
        let ds = self.datasets.entry(dataset.to_string()).or_default();
        ds.computed_through_ts = computed_through_ts;
    }
}

pub fn resolve_partition_location(
    config: &AnalysisLakeConfig,
    manifest: &LakeManifest,
    dataset: &str,
    date: NaiveDate,
    now: DateTime<Utc>,
) -> PartitionLocation {
    if let Some(location) = manifest.partition_location(dataset, date) {
        match location.as_str() {
            "cold" if config.cold_path.is_some() => return PartitionLocation::Cold,
            "hot" => return PartitionLocation::Hot,
            _ => {}
        }
    }

    if config.cold_path.is_some() && date < config.hot_retention_cutoff(now).date_naive() {
        return PartitionLocation::Cold;
    }
    PartitionLocation::Hot
}

pub fn count_parquet_files_in_partition(partition_dir: &Path) -> Result<u32> {
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

pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
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

pub fn read_replication_state(config: &AnalysisLakeConfig) -> Result<ReplicationState> {
    let path = config.replication_state_path();
    if !path.exists() {
        return Ok(ReplicationState::default());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("failed to read replication state at {}", path.display()))?;
    let parsed: ReplicationState = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "failed to parse replication state JSON at {}",
            path.display()
        )
    })?;
    Ok(parsed)
}

pub fn write_replication_state(
    config: &AnalysisLakeConfig,
    state: &ReplicationState,
) -> Result<()> {
    let path = config.replication_state_path();
    if let Some(parent) = path.parent() {
        crate::services::analysis::security::ensure_dir_mode(parent, 0o750)?;
    }

    let contents = serde_json::to_vec_pretty(state)?;
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new(&config.hot_path));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    use std::io::Write;
    tmp.write_all(&contents)?;
    tmp.flush()?;
    tmp.persist(&path)
        .map_err(|err| anyhow::anyhow!("failed to persist replication state: {err}"))?;
    let _ = crate::services::analysis::security::ensure_file_mode(&path, 0o600);
    Ok(())
}

pub fn read_manifest(config: &AnalysisLakeConfig) -> Result<LakeManifest> {
    let path = config.manifest_path();
    if !path.exists() {
        return Ok(LakeManifest::default());
    }
    let bytes = std::fs::read(&path)
        .with_context(|| format!("failed to read manifest at {}", path.display()))?;
    let parsed: LakeManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(parsed)
}

pub fn write_manifest(config: &AnalysisLakeConfig, manifest: &LakeManifest) -> Result<()> {
    let path = config.manifest_path();
    if let Some(parent) = path.parent() {
        crate::services::analysis::security::ensure_dir_mode(parent, 0o750)?;
    }

    let contents = serde_json::to_vec_pretty(manifest)?;
    let tmp_dir = path.parent().unwrap_or_else(|| Path::new(&config.hot_path));
    let mut tmp = tempfile::NamedTempFile::new_in(tmp_dir)?;
    use std::io::Write;
    tmp.write_all(&contents)?;
    tmp.flush()?;
    tmp.persist(&path)
        .map_err(|err| anyhow::anyhow!("failed to persist manifest: {err}"))?;
    let _ = crate::services::analysis::security::ensure_file_mode(&path, 0o600);
    Ok(())
}

pub fn list_dates_in_range(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<NaiveDate> {
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

pub fn shard_set_for_sensor_ids(
    config: &AnalysisLakeConfig,
    sensor_ids: &[String],
) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for sensor_id in sensor_ids {
        let trimmed = sensor_id.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.insert(config.shard_for_sensor_id(trimmed));
    }
    out
}

pub fn list_parquet_files_for_range(
    config: &AnalysisLakeConfig,
    dataset: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    shard_set: &BTreeSet<u32>,
) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let dates = list_dates_in_range(start, end);
    let manifest = match read_manifest(config) {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!(error = %err, "failed to read analysis lake manifest; falling back to filesystem scan");
            LakeManifest::default()
        }
    };
    for date in dates {
        for shard in shard_set {
            // Prefer manifest location if present; fall back to filesystem checks.
            let location = manifest.partition_location(dataset, date);

            let mut candidate_dirs: Vec<PathBuf> = Vec::new();
            match location.as_deref() {
                Some("cold") => {
                    if let Some(dir) = config.partition_dir_cold(dataset, date, *shard) {
                        candidate_dirs.push(dir);
                    }
                    candidate_dirs.push(config.partition_dir_hot(dataset, date, *shard));
                }
                Some("hot") => {
                    candidate_dirs.push(config.partition_dir_hot(dataset, date, *shard));
                    if let Some(dir) = config.partition_dir_cold(dataset, date, *shard) {
                        candidate_dirs.push(dir);
                    }
                }
                _ => {
                    candidate_dirs.push(config.partition_dir_hot(dataset, date, *shard));
                    if let Some(dir) = config.partition_dir_cold(dataset, date, *shard) {
                        candidate_dirs.push(dir);
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
                // Prefer the first existing location.
                break;
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn shard_is_deterministic() {
        let cfg = AnalysisLakeConfig {
            hot_path: PathBuf::from("/tmp"),
            cold_path: None,
            tmp_path: PathBuf::from("/tmp"),
            shards: 16,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };
        let a = cfg.shard_for_sensor_id("abc123");
        let b = cfg.shard_for_sensor_id("abc123");
        assert_eq!(a, b);
        assert!(a < cfg.shards);
    }

    #[test]
    fn shard_set_matches_expected_range() {
        let cfg = AnalysisLakeConfig {
            hot_path: PathBuf::from("/tmp"),
            cold_path: None,
            tmp_path: PathBuf::from("/tmp"),
            shards: 16,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };
        let shards = shard_set_for_sensor_ids(
            &cfg,
            &vec!["a".to_string(), "b".to_string(), "a".to_string()],
        );
        assert!(shards.len() >= 1);
        for shard in shards {
            assert!(shard < cfg.shards);
        }
    }

    #[test]
    fn list_parquet_files_prunes_partitions_and_shards() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&tmp)?;

        let cfg = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: None,
            tmp_path: tmp,
            shards: 4,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let date1 = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let date2 = chrono::NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();

        let shard0_dir = cfg.partition_dir_hot(METRICS_DATASET_V1, date1, 0);
        let shard1_dir = cfg.partition_dir_hot(METRICS_DATASET_V1, date1, 1);
        let shard0_date2_dir = cfg.partition_dir_hot(METRICS_DATASET_V1, date2, 0);
        std::fs::create_dir_all(&shard0_dir)?;
        std::fs::create_dir_all(&shard1_dir)?;
        std::fs::create_dir_all(&shard0_date2_dir)?;
        std::fs::write(shard0_dir.join("part-a.parquet"), b"")?;
        std::fs::write(shard1_dir.join("part-b.parquet"), b"")?;
        std::fs::write(shard0_date2_dir.join("part-c.parquet"), b"")?;

        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 23, 59, 0).unwrap();
        let shard_set = BTreeSet::from([0u32]);

        let files = list_parquet_files_for_range(&cfg, METRICS_DATASET_V1, start, end, &shard_set)?;
        assert_eq!(files.len(), 1);
        let path_str = files[0].display().to_string();
        assert!(path_str.contains("date=2026-01-01"));
        assert!(path_str.contains("shard=00"));
        Ok(())
    }

    #[test]
    fn list_parquet_files_prefers_manifest_location() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let cold = temp.path().join("cold");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&cold)?;
        std::fs::create_dir_all(&tmp)?;

        let cfg = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: Some(cold.clone()),
            tmp_path: tmp,
            shards: 4,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 3).unwrap();
        let shard = 0u32;

        let hot_dir = cfg.partition_dir_hot(METRICS_DATASET_V1, date, shard);
        let cold_dir = cfg
            .partition_dir_cold(METRICS_DATASET_V1, date, shard)
            .unwrap();
        std::fs::create_dir_all(&hot_dir)?;
        std::fs::create_dir_all(&cold_dir)?;
        std::fs::write(hot_dir.join("hot.parquet"), b"")?;
        std::fs::write(cold_dir.join("cold.parquet"), b"")?;

        let mut manifest = LakeManifest::default();
        manifest.set_partition_location(METRICS_DATASET_V1, date, "cold");
        write_manifest(&cfg, &manifest)?;

        let start = Utc.with_ymd_and_hms(2026, 1, 3, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 3, 23, 0, 0).unwrap();
        let shard_set = BTreeSet::from([0u32]);
        let files = list_parquet_files_for_range(&cfg, METRICS_DATASET_V1, start, end, &shard_set)?;
        assert_eq!(files.len(), 1);
        let cold_root = cfg.dataset_root_cold(METRICS_DATASET_V1).unwrap();
        assert!(files[0].starts_with(&cold_root));
        Ok(())
    }

    #[test]
    fn resolve_partition_location_prefers_manifest() {
        let cfg = AnalysisLakeConfig {
            hot_path: PathBuf::from("/tmp/hot"),
            cold_path: Some(PathBuf::from("/tmp/cold")),
            tmp_path: PathBuf::from("/tmp/tmp"),
            shards: 4,
            hot_retention_days: 30,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };
        let mut manifest = LakeManifest::default();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 4).unwrap();
        manifest.set_partition_location(METRICS_DATASET_V1, date, "cold");

        let now = Utc.with_ymd_and_hms(2026, 2, 4, 0, 0, 0).unwrap();
        let location = resolve_partition_location(&cfg, &manifest, METRICS_DATASET_V1, date, now);
        assert_eq!(location, PartitionLocation::Cold);
    }

    #[test]
    fn resolve_partition_location_falls_back_when_cold_missing() {
        let cfg = AnalysisLakeConfig {
            hot_path: PathBuf::from("/tmp/hot"),
            cold_path: None,
            tmp_path: PathBuf::from("/tmp/tmp"),
            shards: 4,
            hot_retention_days: 30,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };
        let mut manifest = LakeManifest::default();
        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 4).unwrap();
        manifest.set_partition_location(METRICS_DATASET_V1, date, "cold");

        let now = Utc.with_ymd_and_hms(2026, 2, 4, 0, 0, 0).unwrap();
        let location = resolve_partition_location(&cfg, &manifest, METRICS_DATASET_V1, date, now);
        assert_eq!(location, PartitionLocation::Hot);
    }

    #[test]
    fn resolve_partition_location_uses_retention_when_manifest_missing() {
        let cfg = AnalysisLakeConfig {
            hot_path: PathBuf::from("/tmp/hot"),
            cold_path: Some(PathBuf::from("/tmp/cold")),
            tmp_path: PathBuf::from("/tmp/tmp"),
            shards: 4,
            hot_retention_days: 10,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };
        let manifest = LakeManifest::default();
        let now = Utc.with_ymd_and_hms(2026, 2, 4, 0, 0, 0).unwrap();
        let date_old = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let date_recent = chrono::NaiveDate::from_ymd_opt(2026, 2, 3).unwrap();

        let location_old =
            resolve_partition_location(&cfg, &manifest, METRICS_DATASET_V1, date_old, now);
        let location_recent =
            resolve_partition_location(&cfg, &manifest, METRICS_DATASET_V1, date_recent, now);
        assert_eq!(location_old, PartitionLocation::Cold);
        assert_eq!(location_recent, PartitionLocation::Hot);
    }
}
