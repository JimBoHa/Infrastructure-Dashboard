use crate::services::analysis::lake::{
    list_parquet_files_for_range, shard_set_for_sensor_ids, AnalysisLakeConfig,
};
use crate::services::analysis::security;
use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use duckdb::Connection;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct DuckDbQueryService {
    tmp_path: PathBuf,
    semaphore: Arc<Semaphore>,
}

#[derive(Debug, Clone)]
pub struct MetricsPointRow {
    pub sensor_id: String,
    pub ts: DateTime<Utc>,
    pub value: f64,
    pub quality: i32,
}

#[derive(Debug, Clone)]
pub struct MetricsBucketRow {
    pub sensor_id: String,
    pub bucket: DateTime<Utc>,
    pub value: f64,
    pub samples: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricsQualityFilter {
    All,
    GoodOnly,
}

#[derive(Debug, Clone, Copy)]
pub struct MetricsBucketReadOptions {
    pub min_samples_per_bucket: Option<i64>,
    pub quality_filter: MetricsQualityFilter,
}

impl MetricsBucketReadOptions {
    pub fn analysis_default() -> Self {
        Self {
            min_samples_per_bucket: Some(2),
            quality_filter: MetricsQualityFilter::GoodOnly,
        }
    }
}

impl Default for MetricsBucketReadOptions {
    fn default() -> Self {
        Self {
            min_samples_per_bucket: None,
            quality_filter: MetricsQualityFilter::All,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BucketCoverageStatsRow {
    pub sensor_id: String,
    pub bucket_rows: u64,
    pub delta_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BucketAggregationMode {
    Avg,
    Last,
    Sum,
    Min,
    Max,
}

impl BucketAggregationMode {
    fn sql_expr(self) -> &'static str {
        match self {
            Self::Avg => "avg(value)",
            // Latest value in the bucket by timestamp.
            Self::Last => "arg_max(value, ts)",
            Self::Sum => "sum(value)",
            Self::Min => "min(value)",
            Self::Max => "max(value)",
        }
    }
}

impl DuckDbQueryService {
    pub fn new(tmp_path: PathBuf, max_concurrent: usize) -> Self {
        Self {
            tmp_path,
            semaphore: Arc::new(Semaphore::new(max_concurrent.max(1))),
        }
    }

    pub async fn read_metrics_points(
        &self,
        parquet_files: Vec<PathBuf>,
        sensor_ids: Vec<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<MetricsPointRow>> {
        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .context("duckdb concurrency gate closed")?;

        let tmp_path = self.tmp_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<MetricsPointRow>> {
            let conn = Connection::open_in_memory()?;
            let tmp_dir = tmp_path.join("duckdb");
            security::ensure_dir_mode(&tmp_dir, 0o700).ok();

            // Best-effort safety settings.
            let _ = conn.execute("PRAGMA threads=2", []);
            let _ = conn.execute("PRAGMA enable_progress_bar=false", []);
            let _ = conn.execute(
                &format!(
                    "SET temp_directory='{}'",
                    escape_single_quotes(tmp_dir.display().to_string())
                ),
                [],
            );

            if parquet_files.is_empty() || sensor_ids.is_empty() {
                return Ok(vec![]);
            }

            let files_sql = parquet_files
                .iter()
                .map(|p| format!("'{}'", escape_single_quotes(p.display().to_string())))
                .collect::<Vec<_>>()
                .join(", ");

            let sensors_sql = sensor_ids
                .iter()
                .map(|s| format!("'{}'", escape_single_quotes(s.trim().to_string())))
                .collect::<Vec<_>>()
                .join(", ");

            let start_sql = start.to_rfc3339();
            let end_sql = end.to_rfc3339();

            let sql = format!(
                r#"
                SELECT sensor_id, ts, value, quality
                FROM read_parquet([{files_sql}], hive_partitioning=1, union_by_name=1)
                WHERE sensor_id IN ({sensors_sql})
                  AND ts >= '{start_sql}'::TIMESTAMP
                  AND ts <= '{end_sql}'::TIMESTAMP
                ORDER BY sensor_id, ts
                "#
            );

            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let sensor_id: String = row.get(0)?;
                let ts: NaiveDateTime = row.get(1)?;
                let value: f64 = row.get(2)?;
                let quality: i32 = row.get(3)?;
                out.push(MetricsPointRow {
                    sensor_id,
                    ts: DateTime::<Utc>::from_naive_utc_and_offset(ts, Utc),
                    value,
                    quality,
                });
            }
            Ok(out)
        })
        .await?
    }

    pub async fn read_metrics_points_from_lake(
        &self,
        lake: &AnalysisLakeConfig,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        sensor_ids: Vec<String>,
        shard_set_override: Option<BTreeSet<u32>>,
    ) -> Result<Vec<MetricsPointRow>> {
        let shard_set =
            shard_set_override.unwrap_or_else(|| shard_set_for_sensor_ids(lake, &sensor_ids));
        let parquet_files = list_parquet_files_for_range(
            lake,
            crate::services::analysis::lake::METRICS_DATASET_V1,
            start,
            end,
            &shard_set,
        )?;
        self.read_metrics_points(parquet_files, sensor_ids, start, end)
            .await
    }

    pub async fn read_metrics_buckets_from_lake(
        &self,
        lake: &AnalysisLakeConfig,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        sensor_ids: Vec<String>,
        interval_seconds: i64,
    ) -> Result<Vec<MetricsBucketRow>> {
        self.read_metrics_buckets_from_lake_with_mode(
            lake,
            start,
            end,
            sensor_ids,
            interval_seconds,
            BucketAggregationMode::Avg,
        )
        .await
    }

    pub async fn read_metrics_buckets_from_lake_with_mode(
        &self,
        lake: &AnalysisLakeConfig,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        sensor_ids: Vec<String>,
        interval_seconds: i64,
        aggregation_mode: BucketAggregationMode,
    ) -> Result<Vec<MetricsBucketRow>> {
        self.read_metrics_buckets_from_lake_with_mode_and_options(
            lake,
            start,
            end,
            sensor_ids,
            interval_seconds,
            aggregation_mode,
            MetricsBucketReadOptions::default(),
        )
        .await
    }

    pub async fn read_metrics_buckets_from_lake_with_mode_and_options(
        &self,
        lake: &AnalysisLakeConfig,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        sensor_ids: Vec<String>,
        interval_seconds: i64,
        aggregation_mode: BucketAggregationMode,
        options: MetricsBucketReadOptions,
    ) -> Result<Vec<MetricsBucketRow>> {
        let interval_seconds = interval_seconds.max(1);
        let shard_set = shard_set_for_sensor_ids(lake, &sensor_ids);
        let parquet_files = list_parquet_files_for_range(
            lake,
            crate::services::analysis::lake::METRICS_DATASET_V1,
            start,
            end,
            &shard_set,
        )?;
        self.read_metrics_buckets_with_mode_and_options(
            parquet_files,
            sensor_ids,
            start,
            end,
            interval_seconds,
            aggregation_mode,
            options,
        )
        .await
    }

    pub async fn read_bucket_coverage_stats_from_lake(
        &self,
        lake: &AnalysisLakeConfig,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        sensor_ids: Vec<String>,
        interval_seconds: i64,
        gap_max_buckets: i64,
    ) -> Result<Vec<BucketCoverageStatsRow>> {
        self.read_bucket_coverage_stats_from_lake_with_options(
            lake,
            start,
            end,
            sensor_ids,
            interval_seconds,
            gap_max_buckets,
            MetricsBucketReadOptions::default(),
        )
        .await
    }

    pub async fn read_bucket_coverage_stats_from_lake_with_options(
        &self,
        lake: &AnalysisLakeConfig,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        sensor_ids: Vec<String>,
        interval_seconds: i64,
        gap_max_buckets: i64,
        options: MetricsBucketReadOptions,
    ) -> Result<Vec<BucketCoverageStatsRow>> {
        let shard_set = shard_set_for_sensor_ids(lake, &sensor_ids);
        let parquet_files = list_parquet_files_for_range(
            lake,
            crate::services::analysis::lake::METRICS_DATASET_V1,
            start,
            end,
            &shard_set,
        )?;
        self.read_bucket_coverage_stats(
            parquet_files,
            sensor_ids,
            start,
            end,
            interval_seconds,
            gap_max_buckets,
            options,
        )
        .await
    }

    pub async fn read_bucket_coverage_stats(
        &self,
        parquet_files: Vec<PathBuf>,
        sensor_ids: Vec<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval_seconds: i64,
        gap_max_buckets: i64,
        options: MetricsBucketReadOptions,
    ) -> Result<Vec<BucketCoverageStatsRow>> {
        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .context("duckdb concurrency gate closed")?;

        let tmp_path = self.tmp_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<BucketCoverageStatsRow>> {
            let conn = Connection::open_in_memory()?;
            let tmp_dir = tmp_path.join("duckdb");
            security::ensure_dir_mode(&tmp_dir, 0o700).ok();

            let _ = conn.execute("PRAGMA threads=2", []);
            let _ = conn.execute("PRAGMA enable_progress_bar=false", []);
            let _ = conn.execute(
                &format!(
                    "SET temp_directory='{}'",
                    escape_single_quotes(tmp_dir.display().to_string())
                ),
                [],
            );

            if parquet_files.is_empty() || sensor_ids.is_empty() {
                return Ok(vec![]);
            }

            let files_sql = parquet_files
                .iter()
                .map(|p| format!("'{}'", escape_single_quotes(p.display().to_string())))
                .collect::<Vec<_>>()
                .join(", ");

            let sensors_sql = sensor_ids
                .iter()
                .map(|s| format!("'{}'", escape_single_quotes(s.trim().to_string())))
                .collect::<Vec<_>>()
                .join(", ");

            let start_sql = start.to_rfc3339();
            let end_sql = end.to_rfc3339();

            let interval_seconds = interval_seconds.max(1);
            let gap_max_buckets = gap_max_buckets.max(0);
            let gap_threshold_seconds: i64 = if gap_max_buckets > 0 {
                gap_max_buckets.saturating_mul(interval_seconds)
            } else {
                i64::MAX
            };

            let quality_sql = match options.quality_filter {
                MetricsQualityFilter::All => String::new(),
                MetricsQualityFilter::GoodOnly => "AND COALESCE(quality, 0) = 0".to_string(),
            };
            let min_samples = options.min_samples_per_bucket.unwrap_or(0).max(0);
            let samples_sql = if min_samples > 1 {
                format!("HAVING COUNT(*) >= {min_samples}")
            } else {
                String::new()
            };

            let sql = format!(
                r#"
                WITH buckets AS (
                    SELECT
                        sensor_id,
                        CAST(floor(epoch(ts) / {interval_seconds}) * {interval_seconds} AS BIGINT) AS bucket_epoch,
                        COUNT(*) AS samples
                    FROM read_parquet([{files_sql}], hive_partitioning=1, union_by_name=1)
                    WHERE sensor_id IN ({sensors_sql})
                      AND ts >= '{start_sql}'::TIMESTAMP
                      AND ts < '{end_sql}'::TIMESTAMP
                      {quality_sql}
                    GROUP BY sensor_id, bucket_epoch
                    {samples_sql}
                ),
                lagged AS (
                    SELECT
                        sensor_id,
                        bucket_epoch,
                        LAG(bucket_epoch) OVER (PARTITION BY sensor_id ORDER BY bucket_epoch) AS prev_bucket_epoch
                    FROM buckets
                )
                SELECT
                    sensor_id,
                    CAST(COUNT(*) AS BIGINT) AS bucket_rows,
                    CAST(
                        SUM(
                            CASE
                                WHEN prev_bucket_epoch IS NULL THEN 0
                                WHEN bucket_epoch - prev_bucket_epoch > {gap_threshold_seconds} THEN 0
                                ELSE 1
                            END
                        ) AS BIGINT
                    ) AS delta_count
                FROM lagged
                GROUP BY sensor_id
                ORDER BY sensor_id
                "#
            );

            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let sensor_id: String = row.get(0)?;
                let bucket_rows: i64 = row.get(1)?;
                let delta_count: i64 = row.get(2)?;
                out.push(BucketCoverageStatsRow {
                    sensor_id,
                    bucket_rows: bucket_rows.max(0) as u64,
                    delta_count: delta_count.max(0) as u64,
                });
            }
            Ok(out)
        })
        .await?
    }

    pub async fn read_metrics_buckets(
        &self,
        parquet_files: Vec<PathBuf>,
        sensor_ids: Vec<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval_seconds: i64,
    ) -> Result<Vec<MetricsBucketRow>> {
        self.read_metrics_buckets_with_mode(
            parquet_files,
            sensor_ids,
            start,
            end,
            interval_seconds,
            BucketAggregationMode::Avg,
        )
        .await
    }

    pub async fn read_metrics_buckets_with_mode(
        &self,
        parquet_files: Vec<PathBuf>,
        sensor_ids: Vec<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval_seconds: i64,
        aggregation_mode: BucketAggregationMode,
    ) -> Result<Vec<MetricsBucketRow>> {
        self.read_metrics_buckets_with_mode_and_options(
            parquet_files,
            sensor_ids,
            start,
            end,
            interval_seconds,
            aggregation_mode,
            MetricsBucketReadOptions::default(),
        )
        .await
    }

    pub async fn read_metrics_buckets_with_mode_and_options(
        &self,
        parquet_files: Vec<PathBuf>,
        sensor_ids: Vec<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        interval_seconds: i64,
        aggregation_mode: BucketAggregationMode,
        options: MetricsBucketReadOptions,
    ) -> Result<Vec<MetricsBucketRow>> {
        let _permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .context("duckdb concurrency gate closed")?;

        let tmp_path = self.tmp_path.clone();
        tokio::task::spawn_blocking(move || -> Result<Vec<MetricsBucketRow>> {
            let conn = Connection::open_in_memory()?;
            let tmp_dir = tmp_path.join("duckdb");
            security::ensure_dir_mode(&tmp_dir, 0o700).ok();

            let _ = conn.execute("PRAGMA threads=2", []);
            let _ = conn.execute("PRAGMA enable_progress_bar=false", []);
            let _ = conn.execute(
                &format!(
                    "SET temp_directory='{}'",
                    escape_single_quotes(tmp_dir.display().to_string())
                ),
                [],
            );

            if parquet_files.is_empty() || sensor_ids.is_empty() {
                return Ok(vec![]);
            }

            let files_sql = parquet_files
                .iter()
                .map(|p| format!("'{}'", escape_single_quotes(p.display().to_string())))
                .collect::<Vec<_>>()
                .join(", ");

            let sensors_sql = sensor_ids
                .iter()
                .map(|s| format!("'{}'", escape_single_quotes(s.trim().to_string())))
                .collect::<Vec<_>>()
                .join(", ");

            let start_sql = start.to_rfc3339();
            let end_sql = end.to_rfc3339();

            // NOTE: keep bucket computation as integer epoch seconds so it is unambiguous.
            let interval_seconds = interval_seconds.max(1);
            let aggregation_sql = aggregation_mode.sql_expr();
            let quality_sql = match options.quality_filter {
                MetricsQualityFilter::All => String::new(),
                MetricsQualityFilter::GoodOnly => "AND COALESCE(quality, 0) = 0".to_string(),
            };
            let min_samples = options.min_samples_per_bucket.unwrap_or(0).max(0);
            let having_sql = if min_samples > 1 {
                format!("HAVING count(*) >= {min_samples}")
            } else {
                String::new()
            };
            let sql = format!(
                r#"
                SELECT
                    sensor_id,
                    CAST(floor(epoch(ts) / {interval_seconds}) * {interval_seconds} AS BIGINT) AS bucket_epoch,
                    {aggregation_sql} AS agg_value,
                    count(*) AS samples
                FROM read_parquet([{files_sql}], hive_partitioning=1, union_by_name=1)
                WHERE sensor_id IN ({sensors_sql})
                  AND ts >= '{start_sql}'::TIMESTAMP
                  AND ts < '{end_sql}'::TIMESTAMP
                  {quality_sql}
                GROUP BY sensor_id, bucket_epoch
                {having_sql}
                ORDER BY sensor_id, bucket_epoch
                "#
            );

            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut out = Vec::new();
            while let Some(row) = rows.next()? {
                let sensor_id: String = row.get(0)?;
                let bucket_epoch: i64 = row.get(1)?;
                let agg_value: f64 = row.get(2)?;
                let samples: i64 = row.get(3)?;
                out.push(MetricsBucketRow {
                    sensor_id,
                    bucket: Utc
                        .timestamp_opt(bucket_epoch, 0)
                        .single()
                        .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap()),
                    value: agg_value,
                    samples,
                });
            }
            Ok(out)
        })
        .await?
    }
}

fn escape_single_quotes(input: String) -> String {
    input.replace('\'', "''")
}

pub fn expected_bucket_count(start: DateTime<Utc>, end_exclusive: DateTime<Utc>, interval_seconds: i64) -> u64 {
    let interval_seconds = interval_seconds.max(1);
    if end_exclusive <= start {
        return 0;
    }

    let interval_micros = interval_seconds.saturating_mul(1_000_000);
    let start_micros = start.timestamp_micros();
    let end_micros = end_exclusive.timestamp_micros();
    if end_micros <= start_micros {
        return 0;
    }

    // `ts < end_exclusive` semantics: the last included point is at `end_exclusive - ε`.
    let last_inclusive_micros = end_micros - 1;
    let bucket_start_micros = start_micros.div_euclid(interval_micros) * interval_micros;
    let bucket_end_micros = last_inclusive_micros.div_euclid(interval_micros) * interval_micros;
    if bucket_end_micros < bucket_start_micros {
        return 0;
    }

    let bucket_span_micros = bucket_end_micros - bucket_start_micros;
    (bucket_span_micros.div_euclid(interval_micros) + 1) as u64
}

pub fn bucket_coverage_pct(
    bucket_rows: u64,
    start: DateTime<Utc>,
    end_exclusive: DateTime<Utc>,
    interval_seconds: i64,
) -> Option<f64> {
    let expected = expected_bucket_count(start, end_exclusive, interval_seconds);
    if expected == 0 {
        return None;
    }
    let safe_rows = bucket_rows.min(expected);
    Some((safe_rows as f64) / (expected as f64) * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::analysis::lake::{write_manifest, LakeManifest, METRICS_DATASET_V1};
    use chrono::TimeZone;

    #[test]
    fn bucket_aggregation_mode_sql_expr_is_stable() {
        assert_eq!(BucketAggregationMode::Avg.sql_expr(), "avg(value)");
        assert_eq!(BucketAggregationMode::Last.sql_expr(), "arg_max(value, ts)");
        assert_eq!(BucketAggregationMode::Sum.sql_expr(), "sum(value)");
        assert_eq!(BucketAggregationMode::Min.sql_expr(), "min(value)");
        assert_eq!(BucketAggregationMode::Max.sql_expr(), "max(value)");
    }

    #[test]
    fn missingness_is_deterministic_for_bucket_windows() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 1, 0, 0).unwrap();
        assert_eq!(expected_bucket_count(start, end, 60), 60);
        assert_eq!(bucket_coverage_pct(60, start, end, 60), Some(100.0));
        assert_eq!(bucket_coverage_pct(30, start, end, 60), Some(50.0));
    }

    async fn write_parquet_rows(
        parquet_path: PathBuf,
        rows: Vec<(&str, &str, f64, i32)>,
    ) -> Result<()> {
        let rows: Vec<(String, String, f64, i32)> = rows
            .into_iter()
            .map(|(sensor_id, ts, value, quality)| {
                (sensor_id.to_string(), ts.to_string(), value, quality)
            })
            .collect();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open_in_memory()?;
            conn.execute(
                "CREATE TABLE t(sensor_id VARCHAR, ts TIMESTAMP, value DOUBLE, quality INTEGER)",
                [],
            )?;
            for (sensor_id, ts, value, quality) in rows {
                conn.execute(
                    &format!(
                        "INSERT INTO t VALUES ('{}', '{}', {}, {})",
                        escape_single_quotes(sensor_id.to_string()),
                        escape_single_quotes(ts.to_string()),
                        value,
                        quality
                    ),
                    [],
                )?;
            }
            conn.execute(
                &format!(
                    "COPY t TO '{}' (FORMAT PARQUET)",
                    escape_single_quotes(parquet_path.display().to_string())
                ),
                [],
            )?;
            Ok(())
        })
        .await?
    }

    #[tokio::test]
    async fn reads_points_from_a_small_parquet_file() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&tmp)?;

        let lake = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: None,
            tmp_path: tmp.clone(),
            shards: 16,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let shard = lake.shard_for_sensor_id("sensor-a");
        let dir = lake.partition_dir_hot(METRICS_DATASET_V1, date, shard);
        std::fs::create_dir_all(&dir)?;
        let parquet_path = dir.join("part-test.parquet");

        // Write a tiny parquet file using DuckDB itself.
        tokio::task::spawn_blocking({
            let parquet_path = parquet_path.clone();
            move || -> Result<()> {
                let conn = Connection::open_in_memory()?;
                conn.execute(
                    "CREATE TABLE t(sensor_id VARCHAR, ts TIMESTAMP, value DOUBLE, quality INTEGER)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO t VALUES ('sensor-a', '2026-01-01 00:00:00', 1.0, 0)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO t VALUES ('sensor-a', '2026-01-01 00:00:01', 2.0, 0)",
                    [],
                )?;
                conn.execute(
                    &format!(
                        "COPY t TO '{}' (FORMAT PARQUET)",
                        escape_single_quotes(parquet_path.display().to_string())
                    ),
                    [],
                )?;
                Ok(())
            }
        })
        .await??;

        let svc = DuckDbQueryService::new(tmp.clone(), 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap();
        let rows = svc
            .read_metrics_points_from_lake(&lake, start, end, vec!["sensor-a".to_string()], None)
            .await?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].value, 1.0);
        assert_eq!(rows[1].value, 2.0);
        Ok(())
    }

    #[tokio::test]
    async fn reads_bucket_coverage_stats_and_respects_gap_threshold() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&tmp)?;

        let lake = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: None,
            tmp_path: tmp.clone(),
            shards: 1,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let shard = lake.shard_for_sensor_id("sensor-a");
        let dir = lake.partition_dir_hot(METRICS_DATASET_V1, date, shard);
        std::fs::create_dir_all(&dir)?;
        let parquet_path = dir.join("part-coverage.parquet");

        write_parquet_rows(
            parquet_path,
            vec![
                ("sensor-a", "2026-01-01T00:00:10Z", 1.0, 0),
                ("sensor-a", "2026-01-01T00:01:10Z", 2.0, 0),
                ("sensor-a", "2026-01-01T00:02:10Z", 3.0, 0),
                ("sensor-a", "2026-01-01T00:03:10Z", 4.0, 0),
                ("sensor-b", "2026-01-01T00:00:10Z", 1.0, 0),
                ("sensor-b", "2026-01-01T00:05:10Z", 2.0, 0),
            ],
        )
        .await?;

        let mut manifest = LakeManifest::default();
        manifest.set_partition_location(METRICS_DATASET_V1, date, "hot");
        write_manifest(&lake, &manifest)?;

        let service = DuckDbQueryService::new(lake.tmp_path.clone(), 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 0, 10, 0).unwrap();

        let stats = service
            .read_bucket_coverage_stats_from_lake(
                &lake,
                start,
                end,
                vec!["sensor-a".to_string(), "sensor-b".to_string()],
                60,
                2,
            )
            .await?;
        let by_id: std::collections::HashMap<String, BucketCoverageStatsRow> = stats
            .into_iter()
            .map(|row| (row.sensor_id.clone(), row))
            .collect();

        assert_eq!(by_id.get("sensor-a").unwrap().bucket_rows, 4);
        assert_eq!(by_id.get("sensor-a").unwrap().delta_count, 3);
        assert_eq!(by_id.get("sensor-b").unwrap().bucket_rows, 2);
        assert_eq!(by_id.get("sensor-b").unwrap().delta_count, 0);

        let stats_no_gap = service
            .read_bucket_coverage_stats_from_lake(
                &lake,
                start,
                end,
                vec!["sensor-b".to_string()],
                60,
                0,
            )
            .await?;
        assert_eq!(stats_no_gap[0].bucket_rows, 2);
        assert_eq!(stats_no_gap[0].delta_count, 1);

        Ok(())
    }

    #[tokio::test]
    async fn reads_bucketed_series_from_a_small_parquet_file() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&tmp)?;

        let lake = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: None,
            tmp_path: tmp.clone(),
            shards: 16,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let shard = lake.shard_for_sensor_id("sensor-a");
        let dir = lake.partition_dir_hot(METRICS_DATASET_V1, date, shard);
        std::fs::create_dir_all(&dir)?;
        let parquet_path = dir.join("part-test.parquet");

        tokio::task::spawn_blocking({
            let parquet_path = parquet_path.clone();
            move || -> Result<()> {
                let conn = Connection::open_in_memory()?;
                conn.execute(
                    "CREATE TABLE t(sensor_id VARCHAR, ts TIMESTAMP, value DOUBLE, quality INTEGER)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO t VALUES ('sensor-a', '2026-01-01 00:00:00', 1.0, 0)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO t VALUES ('sensor-a', '2026-01-01 00:00:01', 2.0, 0)",
                    [],
                )?;
                conn.execute(
                    &format!(
                        "COPY t TO '{}' (FORMAT PARQUET)",
                        escape_single_quotes(parquet_path.display().to_string())
                    ),
                    [],
                )?;
                Ok(())
            }
        })
        .await??;

        let svc = DuckDbQueryService::new(tmp.clone(), 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 2).unwrap();
        let rows = svc
            .read_metrics_buckets_from_lake(&lake, start, end, vec!["sensor-a".to_string()], 1)
            .await?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].value, 1.0);
        assert_eq!(rows[1].value, 2.0);
        Ok(())
    }

    #[tokio::test]
    async fn bucket_aggregation_modes_match_state_and_counter_semantics() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&tmp)?;

        let parquet_path = temp.path().join("part-test.parquet");
        write_parquet_rows(
            parquet_path.clone(),
            vec![
                // State-like sensor: duty-cycle artifact under Avg, transition under Last.
                ("sensor-state", "2026-01-01 00:00:00", 0.0, 0),
                ("sensor-state", "2026-01-01 00:00:30", 1.0, 0),
                // Counter-like sensor: increments sum naturally; avg hides pulse totals.
                ("sensor-counter", "2026-01-01 00:00:05", 2.0, 0),
                ("sensor-counter", "2026-01-01 00:00:55", 3.0, 0),
            ],
        )
        .await?;

        let svc = DuckDbQueryService::new(tmp, 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 0, 1, 0).unwrap();
        let interval_seconds = 60;
        let sensor_ids = vec!["sensor-counter".to_string(), "sensor-state".to_string()];

        let avg_rows = svc
            .read_metrics_buckets_with_mode(
                vec![parquet_path.clone()],
                sensor_ids.clone(),
                start,
                end,
                interval_seconds,
                BucketAggregationMode::Avg,
            )
            .await?;
        let last_rows = svc
            .read_metrics_buckets_with_mode(
                vec![parquet_path.clone()],
                sensor_ids.clone(),
                start,
                end,
                interval_seconds,
                BucketAggregationMode::Last,
            )
            .await?;
        let sum_rows = svc
            .read_metrics_buckets_with_mode(
                vec![parquet_path.clone()],
                sensor_ids.clone(),
                start,
                end,
                interval_seconds,
                BucketAggregationMode::Sum,
            )
            .await?;

        let avg_state = avg_rows
            .iter()
            .find(|r| r.sensor_id == "sensor-state")
            .map(|r| r.value)
            .unwrap();
        let last_state = last_rows
            .iter()
            .find(|r| r.sensor_id == "sensor-state")
            .map(|r| r.value)
            .unwrap();
        assert!((avg_state - 0.5).abs() < 1e-6);
        assert!((last_state - 1.0).abs() < 1e-6);

        let avg_counter = avg_rows
            .iter()
            .find(|r| r.sensor_id == "sensor-counter")
            .map(|r| r.value)
            .unwrap();
        let sum_counter = sum_rows
            .iter()
            .find(|r| r.sensor_id == "sensor-counter")
            .map(|r| r.value)
            .unwrap();
        assert!((avg_counter - 2.5).abs() < 1e-6);
        assert!((sum_counter - 5.0).abs() < 1e-6);

        Ok(())
    }

    #[tokio::test]
    async fn bucketing_respects_min_samples_and_quality_filter() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&tmp)?;

        let lake = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: None,
            tmp_path: tmp.clone(),
            shards: 1,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let shard = lake.shard_for_sensor_id("sensor-a");
        let dir = lake.partition_dir_hot(METRICS_DATASET_V1, date, shard);
        std::fs::create_dir_all(&dir)?;
        let parquet_path = dir.join("part-min-samples.parquet");

        write_parquet_rows(
            parquet_path,
            vec![
                // Bucket 00:00 has 1 sample → should be dropped when min_samples=2.
                ("sensor-a", "2026-01-01T00:00:10Z", 1.0, 0),
                // Bucket 00:01 has 2 samples → should remain.
                ("sensor-a", "2026-01-01T00:01:10Z", 2.0, 0),
                ("sensor-a", "2026-01-01T00:01:20Z", 3.0, 0),
                // Bucket 00:02 has 2 samples but bad quality → should be dropped when GoodOnly.
                ("sensor-a", "2026-01-01T00:02:10Z", 99.0, 1),
                ("sensor-a", "2026-01-01T00:02:20Z", 100.0, 1),
            ],
        )
        .await?;

        let mut manifest = LakeManifest::default();
        manifest.set_partition_location(METRICS_DATASET_V1, date, "hot");
        write_manifest(&lake, &manifest)?;

        let service = DuckDbQueryService::new(lake.tmp_path.clone(), 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 0, 3, 0).unwrap();

        let rows = service
            .read_metrics_buckets_from_lake_with_mode_and_options(
                &lake,
                start,
                end,
                vec!["sensor-a".to_string()],
                60,
                BucketAggregationMode::Avg,
                MetricsBucketReadOptions {
                    min_samples_per_bucket: Some(2),
                    quality_filter: MetricsQualityFilter::GoodOnly,
                },
            )
            .await?;

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].bucket, Utc.with_ymd_and_hms(2026, 1, 1, 0, 1, 0).unwrap());
        assert_eq!(rows[0].samples, 2);
        assert!((rows[0].value - 2.5).abs() < 1e-6);
        Ok(())
    }

    #[tokio::test]
    async fn reads_points_from_cold_partition() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let cold = temp.path().join("cold");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&cold)?;
        std::fs::create_dir_all(&tmp)?;

        let lake = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: Some(cold.clone()),
            tmp_path: tmp.clone(),
            shards: 16,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 4).unwrap();
        let shard = lake.shard_for_sensor_id("sensor-cold");
        let dir = lake
            .partition_dir_cold(METRICS_DATASET_V1, date, shard)
            .unwrap();
        std::fs::create_dir_all(&dir)?;
        let parquet_path = dir.join("part-cold.parquet");

        tokio::task::spawn_blocking({
            let parquet_path = parquet_path.clone();
            move || -> Result<()> {
                let conn = Connection::open_in_memory()?;
                conn.execute(
                    "CREATE TABLE t(sensor_id VARCHAR, ts TIMESTAMP, value DOUBLE, quality INTEGER)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO t VALUES ('sensor-cold', '2026-01-04 00:00:00', 7.0, 0)",
                    [],
                )?;
                conn.execute(
                    "INSERT INTO t VALUES ('sensor-cold', '2026-01-04 00:00:01', 8.0, 0)",
                    [],
                )?;
                conn.execute(
                    &format!(
                        "COPY t TO '{}' (FORMAT PARQUET)",
                        escape_single_quotes(parquet_path.display().to_string())
                    ),
                    [],
                )?;
                Ok(())
            }
        })
        .await??;

        let mut manifest = LakeManifest::default();
        manifest.set_partition_location(METRICS_DATASET_V1, date, "cold");
        write_manifest(&lake, &manifest)?;

        let svc = DuckDbQueryService::new(tmp.clone(), 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 4, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 4, 0, 0, 2).unwrap();
        let rows = svc
            .read_metrics_points_from_lake(&lake, start, end, vec!["sensor-cold".to_string()], None)
            .await?;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].value, 7.0);
        assert_eq!(rows[1].value, 8.0);
        Ok(())
    }

    #[tokio::test]
    async fn reads_points_for_multiple_sensors_across_partitions_and_files() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&tmp)?;

        let lake = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: None,
            tmp_path: tmp.clone(),
            shards: 16,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let sensor_a = "sensor-a";
        let sensor_b = "sensor-b";
        let shard_a = lake.shard_for_sensor_id(sensor_a);
        let shard_b = lake.shard_for_sensor_id(sensor_b);

        let date1 = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let date2 = chrono::NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();

        let dir_a_1 = lake.partition_dir_hot(METRICS_DATASET_V1, date1, shard_a);
        let dir_a_2 = lake.partition_dir_hot(METRICS_DATASET_V1, date2, shard_a);
        let dir_b_1 = lake.partition_dir_hot(METRICS_DATASET_V1, date1, shard_b);
        let dir_b_2 = lake.partition_dir_hot(METRICS_DATASET_V1, date2, shard_b);
        std::fs::create_dir_all(&dir_a_1)?;
        std::fs::create_dir_all(&dir_a_2)?;
        std::fs::create_dir_all(&dir_b_1)?;
        std::fs::create_dir_all(&dir_b_2)?;

        write_parquet_rows(
            dir_a_1.join("part-a-1.parquet"),
            vec![(sensor_a, "2026-01-01 00:00:00", 1.0, 0)],
        )
        .await?;
        write_parquet_rows(
            dir_a_1.join("part-a-2.parquet"),
            vec![(sensor_a, "2026-01-01 00:00:01", 2.0, 0)],
        )
        .await?;
        write_parquet_rows(
            dir_a_2.join("part-a-3.parquet"),
            vec![(sensor_a, "2026-01-02 00:00:00", 3.0, 0)],
        )
        .await?;
        write_parquet_rows(
            dir_a_2.join("part-a-4.parquet"),
            vec![
                (sensor_a, "2026-01-02 00:00:01", 4.0, 0),
                (sensor_a, "2026-01-02 00:00:02", 999.0, 0),
            ],
        )
        .await?;

        write_parquet_rows(
            dir_b_1.join("part-b-1.parquet"),
            vec![(sensor_b, "2026-01-01 00:00:00", 10.0, 0)],
        )
        .await?;
        write_parquet_rows(
            dir_b_1.join("part-b-2.parquet"),
            vec![(sensor_b, "2026-01-01 00:00:01", 11.0, 0)],
        )
        .await?;
        write_parquet_rows(
            dir_b_2.join("part-b-3.parquet"),
            vec![(sensor_b, "2026-01-02 00:00:00", 12.0, 0)],
        )
        .await?;
        write_parquet_rows(
            dir_b_2.join("part-b-4.parquet"),
            vec![(sensor_b, "2026-01-02 00:00:01", 13.0, 0)],
        )
        .await?;

        let svc = DuckDbQueryService::new(tmp.clone(), 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end_inclusive = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 1).unwrap();
        let rows = svc
            .read_metrics_points_from_lake(
                &lake,
                start,
                end_inclusive,
                vec![sensor_b.to_string(), sensor_a.to_string()],
                None,
            )
            .await?;

        assert_eq!(rows.len(), 8);
        assert_eq!(rows.first().map(|r| r.sensor_id.as_str()), Some(sensor_a));
        assert_eq!(rows.last().map(|r| r.sensor_id.as_str()), Some(sensor_b));
        for window in rows.windows(2) {
            let a = &window[0];
            let b = &window[1];
            assert!(
                a.sensor_id < b.sensor_id || (a.sensor_id == b.sensor_id && a.ts <= b.ts),
                "rows not ordered: {:?} then {:?}",
                a,
                b
            );
        }

        let max_a = rows
            .iter()
            .filter(|r| r.sensor_id == sensor_a)
            .map(|r| r.ts)
            .max()
            .unwrap();
        assert_eq!(max_a, Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 1).unwrap());

        Ok(())
    }

    #[tokio::test]
    async fn bucketed_reads_align_to_interval_and_count_samples() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let hot = temp.path().join("hot");
        let tmp = temp.path().join("tmp");
        std::fs::create_dir_all(&hot)?;
        std::fs::create_dir_all(&tmp)?;

        let lake = AnalysisLakeConfig {
            hot_path: hot.clone(),
            cold_path: None,
            tmp_path: tmp.clone(),
            shards: 16,
            hot_retention_days: 90,
            late_window_hours: 48,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let sensor_a = "sensor-a";
        let sensor_b = "sensor-b";
        let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let shard_a = lake.shard_for_sensor_id(sensor_a);
        let shard_b = lake.shard_for_sensor_id(sensor_b);

        let dir_a = lake.partition_dir_hot(METRICS_DATASET_V1, date, shard_a);
        let dir_b = lake.partition_dir_hot(METRICS_DATASET_V1, date, shard_b);
        std::fs::create_dir_all(&dir_a)?;
        std::fs::create_dir_all(&dir_b)?;

        write_parquet_rows(
            dir_a.join("part-a.parquet"),
            vec![
                (sensor_a, "2026-01-01 00:00:00", 1.0, 0),
                (sensor_a, "2026-01-01 00:00:30", 3.0, 0),
                (sensor_a, "2026-01-01 00:01:00", 5.0, 0),
                (sensor_a, "2026-01-01 00:01:30", 7.0, 0),
                (sensor_a, "2026-01-01 00:02:00", 100.0, 0),
            ],
        )
        .await?;
        write_parquet_rows(
            dir_b.join("part-b.parquet"),
            vec![
                (sensor_b, "2026-01-01 00:00:10", 10.0, 0),
                (sensor_b, "2026-01-01 00:01:10", 20.0, 0),
            ],
        )
        .await?;

        let svc = DuckDbQueryService::new(tmp.clone(), 1);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 0, 2, 0).unwrap();
        let rows = svc
            .read_metrics_buckets_from_lake(
                &lake,
                start,
                end,
                vec![sensor_b.to_string(), sensor_a.to_string()],
                60,
            )
            .await?;

        assert_eq!(rows.len(), 4);

        assert_eq!(rows[0].sensor_id, sensor_a);
        assert_eq!(
            rows[0].bucket,
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
        );
        assert_eq!(rows[0].samples, 2);
        assert!((rows[0].value - 2.0).abs() < 1e-9);

        assert_eq!(rows[1].sensor_id, sensor_a);
        assert_eq!(
            rows[1].bucket,
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 1, 0).unwrap()
        );
        assert_eq!(rows[1].samples, 2);
        assert!((rows[1].value - 6.0).abs() < 1e-9);

        assert_eq!(rows[2].sensor_id, sensor_b);
        assert_eq!(
            rows[2].bucket,
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
        );
        assert_eq!(rows[2].samples, 1);
        assert!((rows[2].value - 10.0).abs() < 1e-9);

        assert_eq!(rows[3].sensor_id, sensor_b);
        assert_eq!(
            rows[3].bucket,
            Utc.with_ymd_and_hms(2026, 1, 1, 0, 1, 0).unwrap()
        );
        assert_eq!(rows[3].samples, 1);
        assert!((rows[3].value - 20.0).abs() < 1e-9);

        Ok(())
    }
}
