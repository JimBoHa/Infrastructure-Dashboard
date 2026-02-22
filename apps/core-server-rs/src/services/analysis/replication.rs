use crate::config::CoreConfig;
use crate::services::analysis::lake::{
    copy_dir_all, count_parquet_files_in_partition, read_manifest, read_replication_state,
    resolve_partition_location, write_manifest, write_replication_state, AnalysisLakeConfig,
    LakeManifest, PartitionLocation, ReplicationState, METRICS_DATASET_V1,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use duckdb::Connection;
use futures::TryStreamExt;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncWriteExt, BufWriter as AsyncBufWriter};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct AnalysisReplicationService {
    db: PgPool,
    lake: AnalysisLakeConfig,
}

impl AnalysisReplicationService {
    pub fn new(db: PgPool, config: &CoreConfig) -> Self {
        let lake = AnalysisLakeConfig {
            hot_path: config.analysis_lake_hot_path.clone(),
            cold_path: config.analysis_lake_cold_path.clone(),
            tmp_path: config.analysis_tmp_path.clone(),
            shards: config.analysis_lake_shards,
            hot_retention_days: config.analysis_hot_retention_days,
            late_window_hours: config.analysis_late_window_hours,
            replication_interval: Duration::from_secs(config.analysis_replication_interval_seconds),
            replication_lag: Duration::from_secs(config.analysis_replication_lag_seconds),
        };
        Self { db, lake }
    }

    pub fn start(self, cancel: CancellationToken) {
        tokio::spawn(async move {
            if !cancel.is_cancelled() {
                if let Err(err) = self.run_incremental_once().await {
                    tracing::warn!(error = %err, "analysis replication tick failed");
                }
            }
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(self.lake.replication_interval) => {}
                }

                if let Err(err) = self.run_incremental_once().await {
                    tracing::warn!(error = %err, "analysis replication tick failed");
                }
            }
        });
    }

    async fn run_incremental_once(&self) -> Result<()> {
        let run_started = Utc::now();
        let result = self.run_incremental_inner(run_started).await;
        if let Err(err) = &result {
            if let Err(state_err) = record_replication_failure(
                &self.lake,
                run_started,
                err,
                self.lake.late_window_hours,
            ) {
                tracing::warn!(
                    error = %state_err,
                    "analysis replication failure was not recorded"
                );
            }
        }
        result
    }

    async fn run_incremental_inner(&self, run_started: DateTime<Utc>) -> Result<()> {
        crate::services::analysis::security::ensure_dir_mode(&self.lake.hot_path, 0o750).ok();
        crate::services::analysis::security::ensure_dir_mode(&self.lake.tmp_path, 0o700).ok();

        let mut state = read_replication_state(&self.lake)?;
        let now = run_started;

        // Lag guard: only replicate up to a stable inserted_at watermark.
        let target_inserted_at = now
            - chrono::Duration::from_std(self.lake.replication_lag)
                .unwrap_or(chrono::Duration::minutes(5));

        let late_window = late_window_duration(self.lake.late_window_hours);

        let last_inserted_at = state
            .last_inserted_at
            .as_deref()
            .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let backlog_seconds = replication_backlog_seconds(last_inserted_at, target_inserted_at);
        if let Some(backlog) = backlog_seconds {
            if backlog > late_window.num_seconds() {
                tracing::warn!(
                    last_inserted_at = %last_inserted_at.map(|ts| ts.to_rfc3339()).unwrap_or_default(),
                    target_inserted_at = %target_inserted_at.to_rfc3339(),
                    late_window_hours = self.lake.late_window_hours,
                    backlog_seconds = backlog,
                    "analysis replication watermark exceeds late window; run backfill to restore coverage"
                );
            }
        }

        if let Some(last_inserted_at) = last_inserted_at {
            if target_inserted_at <= last_inserted_at {
                update_replication_run_metadata(
                    &mut state,
                    run_started,
                    0,
                    None,
                    None,
                    backlog_seconds,
                    self.lake.late_window_hours,
                );
                mark_replication_ok(&mut state);
                write_replication_state(&self.lake, &state)?;
                return Ok(());
            }
        }

        // Some historical rows may have inserted_at=NULL (migration kept nullable to avoid large
        // hypertable rewrites). Backfill inserted_at for a small recent window so incremental
        // replication can rely on an inserted_at watermark without re-exporting huge ranges.
        let filled_rows = fill_recent_null_inserted_at(
            &self.db,
            target_inserted_at,
            target_inserted_at - late_window,
        )
        .await
        .unwrap_or(0);

        let export_start =
            compute_incremental_start(last_inserted_at, target_inserted_at, late_window);

        tracing::info!(
            start = %export_start.to_rfc3339(),
            end = %target_inserted_at.to_rfc3339(),
            late_window_hours = self.lake.late_window_hours,
            backlog_seconds = backlog_seconds.unwrap_or(0),
            filled_null_inserted_at_rows = filled_rows,
            "analysis replication incremental export window"
        );

        // Bulk-export rows via COPY and fan out into per-date+shard CSVs, then convert to Parquet.
        let run_id = Uuid::new_v4().to_string();
        let run_dir = self.lake.tmp_path.join("replication").join(&run_id);
        let out_dir = run_dir.join("_out");
        crate::services::analysis::security::ensure_dir_mode(&run_dir, 0o700)?;

        let row_count = copy_metrics_incremental_to_segments(
            &self.db,
            &self.lake,
            &run_dir,
            export_start,
            target_inserted_at,
        )
        .await?;

        if row_count == 0 {
            state.last_inserted_at = Some(target_inserted_at.to_rfc3339());
            state.computed_through_ts = Some(target_inserted_at.to_rfc3339());
            update_replication_run_metadata(
                &mut state,
                run_started,
                row_count,
                Some(export_start),
                Some(target_inserted_at),
                backlog_seconds,
                self.lake.late_window_hours,
            );
            mark_replication_ok(&mut state);
            write_replication_state(&self.lake, &state)?;
            let mut manifest: LakeManifest = read_manifest(&self.lake).unwrap_or_default();
            manifest.set_dataset_watermark(METRICS_DATASET_V1, state.computed_through_ts.clone());
            let _ = write_manifest(&self.lake, &manifest);
            return Ok(());
        }

        // Move staged Parquet segments into the lake (atomic rename).
        let mut manifest: LakeManifest = read_manifest(&self.lake).unwrap_or_default();
        let mut touched_dates: std::collections::BTreeSet<chrono::NaiveDate> =
            std::collections::BTreeSet::new();
        let mut compacted_dates: std::collections::BTreeSet<chrono::NaiveDate> =
            std::collections::BTreeSet::new();
        let mut partition_locations: BTreeMap<chrono::NaiveDate, PartitionLocation> =
            BTreeMap::new();

        for date_entry in std::fs::read_dir(&out_dir)? {
            let date_entry = date_entry?;
            let date_dir = date_entry.path();
            if !date_dir.is_dir() {
                continue;
            }
            let date_name = match date_dir.file_name().and_then(|v| v.to_str()) {
                Some(name) if name.starts_with("date=") => name.to_string(),
                _ => continue,
            };
            let date = match date_name
                .strip_prefix("date=")
                .and_then(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").ok())
            {
                Some(value) => value,
                None => continue,
            };
            for shard_entry in std::fs::read_dir(&date_dir)? {
                let shard_entry = shard_entry?;
                let shard_dir = shard_entry.path();
                if !shard_dir.is_dir() {
                    continue;
                }
                let shard_name = match shard_dir.file_name().and_then(|v| v.to_str()) {
                    Some(name) if name.starts_with("shard=") => name.to_string(),
                    _ => continue,
                };
                let shard: u32 = match shard_name
                    .strip_prefix("shard=")
                    .and_then(|v| v.parse::<u32>().ok())
                {
                    Some(value) => value,
                    None => continue,
                };

                let mut parquet_files: Vec<PathBuf> = Vec::new();
                for entry in std::fs::read_dir(&shard_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.extension().and_then(|v| v.to_str()) == Some("parquet") {
                        parquet_files.push(path);
                    }
                }
                if parquet_files.is_empty() {
                    continue;
                }

                let location = resolve_partition_location(
                    &self.lake,
                    &manifest,
                    METRICS_DATASET_V1,
                    date,
                    now,
                );
                let target_dir = match location {
                    PartitionLocation::Hot => {
                        self.lake.partition_dir_hot(METRICS_DATASET_V1, date, shard)
                    }
                    PartitionLocation::Cold => self
                        .lake
                        .partition_dir_cold(METRICS_DATASET_V1, date, shard)
                        .unwrap_or_else(|| {
                            self.lake.partition_dir_hot(METRICS_DATASET_V1, date, shard)
                        }),
                };
                crate::services::analysis::security::ensure_dir_mode(&target_dir, 0o750)?;
                parquet_files.sort();
                for (index, parquet_path) in parquet_files.into_iter().enumerate() {
                    let final_parquet =
                        target_dir.join(format!("part-{}-{}.parquet", run_id, index));
                    let tmp_parquet =
                        target_dir.join(format!("part-{}-{}.parquet.tmp", run_id, index));
                    move_parquet_file(&parquet_path, &tmp_parquet, &final_parquet).with_context(
                        || {
                            format!(
                                "failed to move parquet {} -> {}",
                                parquet_path.display(),
                                final_parquet.display()
                            )
                        },
                    )?;
                }
                touched_dates.insert(date);
                partition_locations.entry(date).or_insert(location);

                match maybe_compact_partition(&target_dir, &self.lake.tmp_path, &run_id) {
                    Ok(true) => {
                        compacted_dates.insert(date);
                    }
                    Ok(false) => {}
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            partition_dir = %target_dir.display(),
                            "failed to compact parquet partition (non-fatal)"
                        );
                    }
                }
            }
        }

        for date in &touched_dates {
            let location = partition_locations
                .get(date)
                .copied()
                .unwrap_or(PartitionLocation::Hot);
            manifest.set_partition_location(METRICS_DATASET_V1, *date, location.as_str());
            let partition_dir =
                partition_root_for_location(&self.lake, METRICS_DATASET_V1, *date, location);
            if let Ok(file_count) = count_parquet_files_in_partition(&partition_dir) {
                manifest.set_partition_file_count(METRICS_DATASET_V1, *date, file_count);
            }
        }
        for date in &compacted_dates {
            manifest.set_partition_compacted_at(METRICS_DATASET_V1, *date, Utc::now().to_rfc3339());
        }

        // Update replication state.
        state.last_inserted_at = Some(target_inserted_at.to_rfc3339());
        state.computed_through_ts = Some(target_inserted_at.to_rfc3339());
        update_replication_run_metadata(
            &mut state,
            run_started,
            row_count,
            Some(export_start),
            Some(target_inserted_at),
            backlog_seconds,
            self.lake.late_window_hours,
        );
        mark_replication_ok(&mut state);
        write_replication_state(&self.lake, &state)?;
        manifest.set_dataset_watermark(METRICS_DATASET_V1, state.computed_through_ts.clone());

        if !touched_dates.is_empty() || !compacted_dates.is_empty() {
            let _ = write_manifest(&self.lake, &manifest);
        }

        // Best-effort hot retention enforcement (move to cold if configured; otherwise delete).
        if let Err(err) = apply_hot_retention(&self.lake, METRICS_DATASET_V1) {
            tracing::warn!(error = %err, "failed to apply analysis lake hot retention (non-fatal)");
        }

        Ok(())
    }
}

fn apply_hot_retention(lake: &AnalysisLakeConfig, dataset: &str) -> Result<()> {
    let cutoff = lake.hot_retention_cutoff(Utc::now()).date_naive();
    let root = lake.dataset_root_hot(dataset);
    if !root.exists() {
        return Ok(());
    }

    let mut manifest: LakeManifest = read_manifest(lake).unwrap_or_default();

    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(date_str) = name.strip_prefix("date=") else {
            continue;
        };
        let Some(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok() else {
            continue;
        };
        if date >= cutoff {
            continue;
        }

        if let Some(cold_root) = lake.dataset_root_cold(dataset) {
            let _ = crate::services::analysis::security::ensure_dir_mode(&cold_root, 0o750);
            let target = cold_root.join(format!("date={}", date.format("%Y-%m-%d")));
            if target.exists() {
                // Already present in cold; prefer cold and delete hot copy.
                std::fs::remove_dir_all(&path).ok();
                manifest.set_partition_location(dataset, date, "cold");
                if let Ok(file_count) = count_parquet_files_in_partition(&target) {
                    manifest.set_partition_file_count(dataset, date, file_count);
                }
                continue;
            }
            // Try atomic rename first; fall back to copy+delete if crossing filesystems.
            if let Err(err) = std::fs::rename(&path, &target) {
                tracing::info!(
                    error = %err,
                    from = %path.display(),
                    to = %target.display(),
                    "rename failed; falling back to copy+delete for cold partition move"
                );
                copy_dir_all(&path, &target)?;
                std::fs::remove_dir_all(&path).ok();
            }
            manifest.set_partition_location(dataset, date, "cold");
            if let Ok(file_count) = count_parquet_files_in_partition(&target) {
                manifest.set_partition_file_count(dataset, date, file_count);
            }
        } else {
            std::fs::remove_dir_all(&path).ok();
            // Leave manifest entry unset so callers fall back to FS.
        }
    }

    let _ = write_manifest(lake, &manifest);
    Ok(())
}

fn partition_root_for_location(
    lake: &AnalysisLakeConfig,
    dataset: &str,
    date: chrono::NaiveDate,
    location: PartitionLocation,
) -> PathBuf {
    let root = match location {
        PartitionLocation::Hot => lake.dataset_root_hot(dataset),
        PartitionLocation::Cold => lake
            .dataset_root_cold(dataset)
            .unwrap_or_else(|| lake.dataset_root_hot(dataset)),
    };
    root.join(format!("date={}", date.format("%Y-%m-%d")))
}

pub(crate) fn late_window_duration(late_window_hours: u32) -> chrono::Duration {
    chrono::Duration::hours(late_window_hours.max(1) as i64)
}

fn compute_incremental_start(
    last_inserted_at: Option<DateTime<Utc>>,
    target_inserted_at: DateTime<Utc>,
    late_window: chrono::Duration,
) -> DateTime<Utc> {
    let default_start = target_inserted_at - late_window;
    match last_inserted_at {
        Some(previous) if previous > default_start => previous,
        _ => default_start,
    }
}

fn replication_backlog_seconds(
    last_inserted_at: Option<DateTime<Utc>>,
    target_inserted_at: DateTime<Utc>,
) -> Option<i64> {
    last_inserted_at
        .map(|last| (target_inserted_at - last).num_seconds())
        .filter(|backlog| *backlog >= 0)
}

fn mark_replication_ok(state: &mut ReplicationState) {
    state.last_run_status = Some("ok".to_string());
    state.last_run_error = None;
}

fn record_replication_failure(
    lake: &AnalysisLakeConfig,
    run_started: DateTime<Utc>,
    err: &anyhow::Error,
    late_window_hours: u32,
) -> Result<()> {
    let mut state = read_replication_state(lake)?;
    let backlog_seconds = state.last_run_backlog_seconds;
    update_replication_run_metadata(
        &mut state,
        run_started,
        0,
        None,
        None,
        backlog_seconds,
        late_window_hours,
    );
    state.last_run_status = Some("failed".to_string());
    state.last_run_error = Some(format!("{:#}", err));
    write_replication_state(lake, &state)?;
    Ok(())
}

fn update_replication_run_metadata(
    state: &mut ReplicationState,
    run_started: DateTime<Utc>,
    row_count: u64,
    export_start: Option<DateTime<Utc>>,
    export_end: Option<DateTime<Utc>>,
    backlog_seconds: Option<i64>,
    late_window_hours: u32,
) {
    let completed = Utc::now();
    let duration_ms = completed
        .signed_duration_since(run_started)
        .num_milliseconds()
        .max(0) as u64;
    state.last_run_at = Some(completed.to_rfc3339());
    state.last_run_duration_ms = Some(duration_ms);
    state.last_run_row_count = Some(row_count);
    state.last_run_backlog_seconds = backlog_seconds;
    if let Some(start) = export_start {
        state.last_export_start = Some(start.to_rfc3339());
    }
    if let Some(end) = export_end {
        state.last_export_end = Some(end.to_rfc3339());
    }
    state.last_late_window_hours = Some(late_window_hours);
}

pub(crate) async fn run_replication_tick(
    db: &PgPool,
    lake: &AnalysisLakeConfig,
) -> Result<ReplicationState> {
    let service = AnalysisReplicationService {
        db: db.clone(),
        lake: lake.clone(),
    };
    service.run_incremental_once().await?;
    read_replication_state(lake)
}

async fn fill_recent_null_inserted_at(
    db: &PgPool,
    inserted_at: DateTime<Utc>,
    start_ts: DateTime<Utc>,
) -> Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE metrics
        SET inserted_at = $1
        WHERE inserted_at IS NULL
          AND ts >= $2
          AND ts <= $1
        "#,
    )
    .bind(inserted_at)
    .bind(start_ts)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

pub(crate) async fn copy_metrics_incremental_to_segments(
    db: &PgPool,
    lake: &AnalysisLakeConfig,
    run_dir: &PathBuf,
    export_start: DateTime<Utc>,
    export_end: DateTime<Utc>,
) -> Result<u64> {
    let staging_csv = run_dir.join("_staging").join("incremental.csv");
    let out_dir = run_dir.join("_out");
    // DuckDB COPY errors when the output directory is non-empty. Keep staging files separate.
    if out_dir.exists() {
        let _ = std::fs::remove_dir_all(&out_dir);
    }
    let copy_sql = build_incremental_copy_sql(export_start, export_end);
    let sensor_sql = build_incremental_sensor_ids_sql(export_start, export_end);
    let count_sql = build_incremental_count_sql(export_start, export_end);
    copy_metrics_to_segments(
        db,
        lake,
        &out_dir,
        &staging_csv,
        &copy_sql,
        &sensor_sql,
        &count_sql,
        true,
    )
    .await
}

pub(crate) async fn copy_metrics_backfill_to_segments(
    db: &PgPool,
    lake: &AnalysisLakeConfig,
    run_dir: &PathBuf,
    start_ts: DateTime<Utc>,
    end_ts: DateTime<Utc>,
    target_inserted_at: DateTime<Utc>,
    label: &str,
) -> Result<u64> {
    let staging_csv = run_dir
        .join("_staging")
        .join(format!("backfill-{label}.csv"));
    let out_dir = run_dir.join(format!("date={label}"));
    if out_dir.exists() {
        let _ = std::fs::remove_dir_all(&out_dir);
    }
    let copy_sql = build_backfill_copy_sql(start_ts, end_ts, target_inserted_at);
    let sensor_sql = build_backfill_sensor_ids_sql(start_ts, end_ts, target_inserted_at);
    let count_sql = build_backfill_count_sql(start_ts, end_ts, target_inserted_at);
    copy_metrics_to_segments(
        db,
        lake,
        &out_dir,
        &staging_csv,
        &copy_sql,
        &sensor_sql,
        &count_sql,
        false,
    )
    .await
}

fn build_incremental_filter_sql(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let start_sql = escape_single_quotes(&start.to_rfc3339());
    let end_sql = escape_single_quotes(&end.to_rfc3339());
    format!("inserted_at > '{start_sql}'::timestamptz AND inserted_at <= '{end_sql}'::timestamptz")
}

fn build_incremental_copy_sql(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let filter = build_incremental_filter_sql(start, end);
    format!(
        r#"
        COPY (
            SELECT
                sensor_id,
                (EXTRACT(EPOCH FROM ts) * 1000000)::BIGINT as ts_micros,
                value,
                quality,
                (EXTRACT(EPOCH FROM COALESCE(inserted_at, ts)) * 1000000)::BIGINT as inserted_at_micros
            FROM metrics
            WHERE {filter}
            ORDER BY inserted_at ASC
        ) TO STDOUT WITH (FORMAT csv, HEADER true)
        "#
    )
}

fn build_incremental_sensor_ids_sql(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let filter = build_incremental_filter_sql(start, end);
    format!("SELECT DISTINCT sensor_id FROM metrics WHERE {filter}")
}

fn build_incremental_count_sql(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let filter = build_incremental_filter_sql(start, end);
    format!("SELECT COUNT(*) FROM metrics WHERE {filter}")
}

fn build_backfill_filter_sql(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    target_inserted_at: DateTime<Utc>,
) -> String {
    let start_sql = escape_single_quotes(&start.to_rfc3339());
    let end_sql = escape_single_quotes(&end.to_rfc3339());
    let target_sql = escape_single_quotes(&target_inserted_at.to_rfc3339());
    format!(
        "ts >= '{start_sql}'::timestamptz AND ts < '{end_sql}'::timestamptz \
         AND (inserted_at IS NULL OR inserted_at <= '{target_sql}'::timestamptz)"
    )
}

fn build_backfill_copy_sql(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    target_inserted_at: DateTime<Utc>,
) -> String {
    let filter = build_backfill_filter_sql(start, end, target_inserted_at);
    format!(
        r#"
        COPY (
            SELECT
                sensor_id,
                (EXTRACT(EPOCH FROM ts) * 1000000)::BIGINT as ts_micros,
                value,
                quality,
                (EXTRACT(EPOCH FROM COALESCE(inserted_at, ts)) * 1000000)::BIGINT as inserted_at_micros
            FROM metrics
            WHERE {filter}
            ORDER BY ts ASC
        ) TO STDOUT WITH (FORMAT csv, HEADER true)
        "#
    )
}

fn build_backfill_sensor_ids_sql(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    target_inserted_at: DateTime<Utc>,
) -> String {
    let filter = build_backfill_filter_sql(start, end, target_inserted_at);
    format!("SELECT DISTINCT sensor_id FROM metrics WHERE {filter}")
}

fn build_backfill_count_sql(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    target_inserted_at: DateTime<Utc>,
) -> String {
    let filter = build_backfill_filter_sql(start, end, target_inserted_at);
    format!("SELECT COUNT(*) FROM metrics WHERE {filter}")
}

async fn copy_metrics_to_segments(
    db: &PgPool,
    lake: &AnalysisLakeConfig,
    out_dir: &PathBuf,
    staging_csv: &PathBuf,
    copy_sql: &str,
    sensor_ids_sql: &str,
    count_sql: &str,
    partition_by_date: bool,
) -> Result<u64> {
    let row_count = fetch_row_count(db, count_sql).await?;
    if row_count == 0 {
        return Ok(0);
    }

    let staging_dir = staging_csv
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| out_dir.join("_staging"));
    std::fs::create_dir_all(&staging_dir).ok();

    copy_metrics_to_csv(db, copy_sql, staging_csv).await?;

    let sensor_ids = fetch_sensor_ids(db, sensor_ids_sql).await?;
    if sensor_ids.is_empty() {
        anyhow::bail!("metrics COPY produced rows but no sensor ids for shard mapping");
    }

    let shards_csv = staging_dir.join("sensor_shards.csv");
    write_sensor_shards_csv(lake, &sensor_ids, &shards_csv)?;

    tokio::task::spawn_blocking({
        let staging_csv = staging_csv.clone();
        let shards_csv = shards_csv.clone();
        let out_dir = out_dir.clone();
        let tmp_root = lake.tmp_path.clone();
        move || {
            bulk_export_parquet_from_copy_csv(
                &staging_csv,
                &shards_csv,
                &out_dir,
                &tmp_root,
                partition_by_date,
            )
        }
    })
    .await??;

    let _ = std::fs::remove_file(staging_csv);
    let _ = std::fs::remove_file(&shards_csv);
    Ok(row_count)
}

async fn copy_metrics_to_csv(db: &PgPool, copy_sql: &str, csv_path: &PathBuf) -> Result<()> {
    let mut conn = db.acquire().await?;
    let mut stream = conn.copy_out_raw(copy_sql).await?;
    // Stream Postgres COPY output into a buffered file to avoid row-by-row queries.
    let file = tokio::fs::File::create(csv_path).await?;
    let mut writer = AsyncBufWriter::new(file);
    while let Some(bytes) = stream.try_next().await? {
        writer.write_all(&bytes).await?;
    }
    writer.flush().await?;
    Ok(())
}

async fn fetch_sensor_ids(db: &PgPool, sql: &str) -> Result<Vec<String>> {
    let rows = sqlx::query_scalar::<_, String>(sql).fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect())
}

async fn fetch_row_count(db: &PgPool, sql: &str) -> Result<u64> {
    let count = sqlx::query_scalar::<_, i64>(sql).fetch_one(db).await?;
    Ok(count.max(0) as u64)
}

fn write_sensor_shards_csv(
    lake: &AnalysisLakeConfig,
    sensor_ids: &[String],
    shards_csv: &PathBuf,
) -> Result<()> {
    let mut writer = BufWriter::new(File::create(shards_csv)?);
    writeln!(&mut writer, "sensor_id,shard")?;
    for sensor_id in sensor_ids {
        let shard = lake.shard_for_sensor_id(sensor_id);
        writeln!(
            &mut writer,
            "{},{}",
            escape_csv(sensor_id),
            escape_csv(&format!("{:02}", shard))
        )?;
    }
    writer.flush()?;
    Ok(())
}

fn bulk_export_parquet_from_copy_csv(
    csv_path: &PathBuf,
    shards_path: &PathBuf,
    out_dir: &PathBuf,
    tmp_root: &PathBuf,
    partition_by_date: bool,
) -> Result<()> {
    let conn = Connection::open_in_memory()?;
    let tmp_dir = tmp_root.join("duckdb");
    std::fs::create_dir_all(out_dir).ok();
    let _ = crate::services::analysis::security::ensure_dir_mode(&tmp_dir, 0o700);
    let _ = conn.execute("PRAGMA threads=2", []);
    let _ = conn.execute(
        &format!(
            "SET temp_directory='{}'",
            escape_single_quotes(&tmp_dir.display().to_string())
        ),
        [],
    );

    let csv = escape_single_quotes(&csv_path.display().to_string());
    let shards = escape_single_quotes(&shards_path.display().to_string());
    let out = escape_single_quotes(&out_dir.display().to_string());
    let partition_by = if partition_by_date {
        "PARTITION_BY (date, shard)"
    } else {
        "PARTITION_BY (shard)"
    };
    let sql = format!(
        r#"
        COPY (
            SELECT
                m.sensor_id,
                CAST(to_timestamp(m.ts_micros / 1000000.0) AS TIMESTAMP) as ts,
                m.value::DOUBLE as value,
                COALESCE(m.quality, 0)::INTEGER as quality,
                CAST(
                    to_timestamp(COALESCE(m.inserted_at_micros, m.ts_micros) / 1000000.0)
                    AS TIMESTAMP
                ) as inserted_at,
                s.shard as shard,
                CAST(CAST(to_timestamp(m.ts_micros / 1000000.0) AS TIMESTAMP) AS DATE) as date
            FROM read_csv(
                '{csv}',
                columns={{'sensor_id':'VARCHAR','ts_micros':'BIGINT','value':'DOUBLE','quality':'INTEGER','inserted_at_micros':'BIGINT'}},
                header=true
            ) m
            JOIN read_csv(
                '{shards}',
                columns={{'sensor_id':'VARCHAR','shard':'VARCHAR'}},
                header=true
            ) s USING (sensor_id)
            ORDER BY sensor_id, ts
        ) TO '{out}' (FORMAT PARQUET, {partition_by}, COMPRESSION ZSTD);
        "#
    );
    conn.execute(&sql, [])?;
    Ok(())
}

pub(crate) fn move_parquet_file(source: &PathBuf, tmp: &PathBuf, dest: &PathBuf) -> Result<()> {
    if let Err(err) = std::fs::rename(source, tmp) {
        tracing::debug!(
            error = %err,
            from = %source.display(),
            to = %tmp.display(),
            "parquet rename failed; falling back to copy"
        );
        std::fs::copy(source, tmp)?;
        let _ = std::fs::remove_file(source);
    }
    std::fs::rename(tmp, dest)?;
    Ok(())
}

fn escape_csv(value: &str) -> String {
    // Values we emit are sensor_id hex strings; still escape conservatively.
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

pub(crate) fn escape_single_quotes(input: &str) -> String {
    input.replace('\'', "''")
}

fn maybe_compact_partition(
    partition_dir: &PathBuf,
    tmp_root: &PathBuf,
    run_id: &str,
) -> Result<bool> {
    let mut parquet_files: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(partition_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|v| v.to_str()) == Some("parquet") {
            parquet_files.push(path);
        }
    }

    // Keep it simple: only compact when a shard-day accumulates too many segments.
    // This keeps file counts bounded without trying to be clever about incremental merges.
    const COMPACT_TRIGGER_FILES: usize = 10;
    if parquet_files.len() <= COMPACT_TRIGGER_FILES {
        return Ok(false);
    }
    parquet_files.sort();

    let conn = Connection::open_in_memory()?;
    let tmp_dir = tmp_root.join("duckdb");
    let _ = crate::services::analysis::security::ensure_dir_mode(&tmp_dir, 0o700);
    let _ = conn.execute("PRAGMA threads=2", []);
    let _ = conn.execute(
        &format!(
            "SET temp_directory='{}'",
            escape_single_quotes(&tmp_dir.display().to_string())
        ),
        [],
    );

    let files_sql = parquet_files
        .iter()
        .map(|p| format!("'{}'", escape_single_quotes(&p.display().to_string())))
        .collect::<Vec<_>>()
        .join(", ");

    let tmp_out = partition_dir.join(format!("compact-{}.parquet.tmp", run_id));
    let final_out = partition_dir.join(format!("compact-{}.parquet", run_id));

    let sql = format!(
        r#"
        COPY (
            SELECT
                sensor_id,
                ts,
                arg_max(value, COALESCE(inserted_at, ts)) as value,
                arg_max(quality, COALESCE(inserted_at, ts)) as quality,
                max(COALESCE(inserted_at, ts)) as inserted_at
            FROM read_parquet([{files_sql}], hive_partitioning=1, union_by_name=1)
            GROUP BY sensor_id, ts
            ORDER BY sensor_id, ts
        ) TO '{}' (FORMAT PARQUET, COMPRESSION ZSTD);
        "#,
        escape_single_quotes(&tmp_out.display().to_string())
    );
    conn.execute(&sql, [])?;

    std::fs::rename(&tmp_out, &final_out).with_context(|| {
        format!(
            "failed to finalize compacted parquet {} -> {}",
            tmp_out.display(),
            final_out.display()
        )
    })?;

    for path in parquet_files {
        if path == final_out {
            continue;
        }
        let _ = std::fs::remove_file(&path);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tempfile::tempdir;

    #[test]
    fn late_window_duration_has_minimum() {
        assert_eq!(late_window_duration(0), chrono::Duration::hours(1));
        assert_eq!(late_window_duration(1), chrono::Duration::hours(1));
        assert_eq!(late_window_duration(48), chrono::Duration::hours(48));
    }

    #[test]
    fn bulk_export_parquet_writes_partitioned_files() -> Result<()> {
        let dir = tempdir()?;
        let run_dir = dir.path().join("run");
        let tmp_root = dir.path().join("tmp");
        std::fs::create_dir_all(&run_dir)?;
        std::fs::create_dir_all(&tmp_root)?;

        let lake = AnalysisLakeConfig {
            hot_path: dir.path().join("hot"),
            cold_path: None,
            tmp_path: tmp_root.clone(),
            shards: 16,
            hot_retention_days: 7,
            late_window_hours: 24,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let sensor_id = "sensor-a";
        let shard = lake.shard_for_sensor_id(sensor_id);
        let shard_str = format!("{:02}", shard);

        let ts = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let ts_micros = ts.timestamp() * 1_000_000 + i64::from(ts.timestamp_subsec_micros());

        let csv_path = dir.path().join("metrics.csv");
        let mut csv_writer = BufWriter::new(File::create(&csv_path)?);
        writeln!(
            &mut csv_writer,
            "sensor_id,ts_micros,value,quality,inserted_at_micros"
        )?;
        writeln!(
            &mut csv_writer,
            "{},{},{},{},{}",
            sensor_id, ts_micros, 1.25, 4, ts_micros
        )?;
        csv_writer.flush()?;

        let shards_path = dir.path().join("shards.csv");
        let mut shard_writer = BufWriter::new(File::create(&shards_path)?);
        writeln!(&mut shard_writer, "sensor_id,shard")?;
        writeln!(&mut shard_writer, "{},{}", sensor_id, shard_str)?;
        shard_writer.flush()?;

        bulk_export_parquet_from_copy_csv(&csv_path, &shards_path, &run_dir, &tmp_root, true)?;

        let date_dir = run_dir.join(format!("date={}", ts.date_naive().format("%Y-%m-%d")));
        let shard_dir = date_dir.join(format!("shard={}", shard_str));
        let parquet_count = std::fs::read_dir(&shard_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|v| v.to_str()) == Some("parquet"))
            .count();

        assert!(parquet_count > 0, "expected parquet output for shard");
        Ok(())
    }

    #[test]
    fn bulk_export_parquet_can_partition_by_shard_only_into_date_dir() -> Result<()> {
        let dir = tempdir()?;
        let run_dir = dir.path().join("run");
        let tmp_root = dir.path().join("tmp");
        std::fs::create_dir_all(&run_dir)?;
        std::fs::create_dir_all(&tmp_root)?;

        let lake = AnalysisLakeConfig {
            hot_path: dir.path().join("hot"),
            cold_path: None,
            tmp_path: tmp_root.clone(),
            shards: 16,
            hot_retention_days: 7,
            late_window_hours: 24,
            replication_interval: std::time::Duration::from_secs(60),
            replication_lag: std::time::Duration::from_secs(300),
        };

        let sensor_id = "sensor-a";
        let shard = lake.shard_for_sensor_id(sensor_id);
        let shard_str = format!("{:02}", shard);

        let ts = Utc.with_ymd_and_hms(2025, 1, 2, 3, 4, 5).unwrap();
        let ts_micros = ts.timestamp() * 1_000_000 + i64::from(ts.timestamp_subsec_micros());

        let csv_path = dir.path().join("metrics.csv");
        let mut csv_writer = BufWriter::new(File::create(&csv_path)?);
        writeln!(
            &mut csv_writer,
            "sensor_id,ts_micros,value,quality,inserted_at_micros"
        )?;
        writeln!(
            &mut csv_writer,
            "{},{},{},{},{}",
            sensor_id, ts_micros, 1.25, 4, ts_micros
        )?;
        csv_writer.flush()?;

        let shards_path = dir.path().join("shards.csv");
        let mut shard_writer = BufWriter::new(File::create(&shards_path)?);
        writeln!(&mut shard_writer, "sensor_id,shard")?;
        writeln!(&mut shard_writer, "{},{}", sensor_id, shard_str)?;
        shard_writer.flush()?;

        let date_dir = run_dir.join(format!("date={}", ts.date_naive().format("%Y-%m-%d")));
        std::fs::create_dir_all(&date_dir)?;
        bulk_export_parquet_from_copy_csv(&csv_path, &shards_path, &date_dir, &tmp_root, false)?;

        let shard_dir = date_dir.join(format!("shard={}", shard_str));
        let parquet_count = std::fs::read_dir(&shard_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|v| v.to_str()) == Some("parquet"))
            .count();

        assert!(
            parquet_count > 0,
            "expected parquet output for shard-only partition"
        );
        Ok(())
    }
}
