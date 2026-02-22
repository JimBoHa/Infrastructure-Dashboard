use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::lake::{
    count_parquet_files_in_partition, read_manifest, read_replication_state, write_manifest,
    write_replication_state, AnalysisLakeConfig, PartitionLocation, METRICS_DATASET_V1,
};
use crate::services::analysis::replication::copy_metrics_backfill_to_segments;
use crate::services::analysis::replication::move_parquet_file;
use anyhow::Context;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize)]
struct BackfillParams {
    #[serde(default)]
    days: Option<u32>,
    #[serde(default)]
    replace_existing: Option<bool>,
}

pub async fn execute(
    db: &PgPool,
    lake: &AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: BackfillParams =
        serde_json::from_value(job.params.0.clone()).unwrap_or(BackfillParams {
            days: None,
            replace_existing: None,
        });

    let days = params.days.unwrap_or(90).clamp(1, 365);
    let replace_existing = params.replace_existing.unwrap_or(true);

    let mut state =
        read_replication_state(lake).map_err(|err| JobFailure::Failed(error_from_anyhow(err)))?;
    let now = Utc::now();

    // For backfill we pick a stable inserted_at watermark and include all rows with inserted_at NULL.
    // If replication is already ahead, use that watermark so a replace-backfill cannot delete newer points.
    let recommended_target_inserted_at = now
        - chrono::Duration::from_std(lake.replication_lag).unwrap_or(chrono::Duration::minutes(5));
    let current_last_inserted_at = state
        .last_inserted_at
        .as_deref()
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let target_inserted_at = match current_last_inserted_at {
        Some(existing) if existing > recommended_target_inserted_at => existing,
        _ => recommended_target_inserted_at,
    };

    // Backfill coverage by ts through the inserted_at watermark.
    let end_ts = target_inserted_at;
    let start_ts = end_ts - chrono::Duration::days(days as i64);

    let start_day = start_ts.date_naive();
    let end_day = end_ts.date_naive();
    let mut dates: Vec<chrono::NaiveDate> = Vec::new();
    let mut cursor = start_day;
    while cursor <= end_day {
        dates.push(cursor);
        let Some(next) = cursor.succ_opt() else { break };
        if next == cursor {
            break;
        }
        cursor = next;
    }
    let total_days = dates.len().max(1) as u64;

    let mut progress = AnalysisJobProgress {
        phase: "lake_backfill".to_string(),
        completed: 0,
        total: Some(total_days),
        message: Some(format!(
            "Backfilling {} days into the Parquet analysis lake",
            days
        )),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    std::fs::create_dir_all(&lake.hot_path).ok();
    std::fs::create_dir_all(&lake.tmp_path).ok();

    let run_id = Uuid::new_v4().to_string();
    let run_dir = lake.tmp_path.join("backfill").join(&run_id);
    std::fs::create_dir_all(&run_dir).ok();

    let mut touched_dates: BTreeSet<chrono::NaiveDate> = BTreeSet::new();
    let mut partition_locations: BTreeMap<chrono::NaiveDate, PartitionLocation> = BTreeMap::new();
    let mut manifest = read_manifest(lake).unwrap_or_default();
    let mut total_rows: u64 = 0;

    for (idx, date) in dates.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }

        progress.completed = idx as u64;
        progress.message = Some(format!("Backfilling {}", date.format("%Y-%m-%d")));
        let _ = store::update_progress(db, job.id, &progress).await;

        let day_start = Utc
            .with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
            .unwrap();
        let day_end = day_start + chrono::Duration::days(1);

        let segment_start = std::cmp::max(day_start, start_ts);
        let segment_end = std::cmp::min(day_end, end_ts);
        if segment_end <= segment_start {
            progress.completed = (idx + 1) as u64;
            let _ = store::update_progress(db, job.id, &progress).await;
            continue;
        }

        if replace_existing {
            remove_existing_partitions(lake, *date);
        }
        let copied = copy_metrics_backfill_to_segments(
            db,
            lake,
            &run_dir,
            segment_start,
            segment_end,
            target_inserted_at,
            &date.format("%Y-%m-%d").to_string(),
        )
        .await
        .map_err(|err| JobFailure::Failed(error_from_anyhow(err)))?;
        total_rows += copied;

        // Move staged Parquet files into the lake.
        let date_dir = run_dir.join(format!("date={}", date.format("%Y-%m-%d")));
        let mut wrote_day = false;
        if date_dir.exists() {
            for shard_entry in std::fs::read_dir(&date_dir)
                .with_context(|| format!("failed to read {}", date_dir.display()))
                .map_err(|err| JobFailure::Failed(error_from_anyhow(err)))?
            {
                let shard_entry =
                    shard_entry.map_err(|err| JobFailure::Failed(error_from_anyhow(err.into())))?;
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

                let mut parquet_files: Vec<std::path::PathBuf> = Vec::new();
                for entry in std::fs::read_dir(&shard_dir)
                    .with_context(|| format!("failed to read {}", shard_dir.display()))
                    .map_err(|err| JobFailure::Failed(error_from_anyhow(err)))?
                {
                    let entry =
                        entry.map_err(|err| JobFailure::Failed(error_from_anyhow(err.into())))?;
                    let path = entry.path();
                    if path.extension().and_then(|v| v.to_str()) == Some("parquet") {
                        parquet_files.push(path);
                    }
                }
                if parquet_files.is_empty() {
                    continue;
                }

                let location = crate::services::analysis::lake::resolve_partition_location(
                    lake,
                    &manifest,
                    METRICS_DATASET_V1,
                    *date,
                    now,
                );
                let target_dir = match location {
                    PartitionLocation::Hot => {
                        lake.partition_dir_hot(METRICS_DATASET_V1, *date, shard)
                    }
                    PartitionLocation::Cold => lake
                        .partition_dir_cold(METRICS_DATASET_V1, *date, shard)
                        .unwrap_or_else(|| {
                            lake.partition_dir_hot(METRICS_DATASET_V1, *date, shard)
                        }),
                };
                std::fs::create_dir_all(&target_dir).ok();

                parquet_files.sort();
                for (index, parquet_path) in parquet_files.into_iter().enumerate() {
                    let final_parquet =
                        target_dir.join(format!("backfill-{}-{}.parquet", run_id, index));
                    let tmp_parquet =
                        target_dir.join(format!("backfill-{}-{}.parquet.tmp", run_id, index));
                    move_parquet_file(&parquet_path, &tmp_parquet, &final_parquet)
                        .with_context(|| {
                            format!(
                                "failed to move parquet {} -> {}",
                                parquet_path.display(),
                                final_parquet.display()
                            )
                        })
                        .map_err(|err| JobFailure::Failed(error_from_anyhow(err)))?;
                }
                wrote_day = true;
                partition_locations.entry(*date).or_insert(location);
            }
        }
        if wrote_day {
            touched_dates.insert(*date);
        }
        progress.completed = (idx + 1) as u64;
        let _ = store::update_progress(db, job.id, &progress).await;
    }

    // Update manifest with touched hot partitions.
    if !touched_dates.is_empty() {
        for date in &touched_dates {
            let location = partition_locations
                .get(date)
                .copied()
                .unwrap_or(PartitionLocation::Hot);
            manifest.set_partition_location(METRICS_DATASET_V1, *date, location.as_str());
            let partition_dir = partition_root_for_location(lake, *date, location);
            if let Ok(file_count) = count_parquet_files_in_partition(&partition_dir) {
                manifest.set_partition_file_count(METRICS_DATASET_V1, *date, file_count);
            }
        }
        let _ = write_manifest(lake, &manifest);
    }

    // Update replication state so incremental replication can resume without duplicating points.
    state.backfill_from_ts = Some(start_ts.to_rfc3339());
    state.backfill_to_ts = Some(end_ts.to_rfc3339());
    state.backfill_completed_at = Some(Utc::now().to_rfc3339());
    state.last_inserted_at = match current_last_inserted_at {
        Some(existing) if existing > target_inserted_at => Some(existing.to_rfc3339()),
        _ => Some(target_inserted_at.to_rfc3339()),
    };
    state.computed_through_ts = Some(target_inserted_at.to_rfc3339());
    write_replication_state(lake, &state)
        .map_err(|err| JobFailure::Failed(error_from_anyhow(err)))?;
    if !touched_dates.is_empty() {
        manifest.set_dataset_watermark(METRICS_DATASET_V1, state.computed_through_ts.clone());
        let _ = write_manifest(lake, &manifest);
    }

    Ok(serde_json::json!({
        "job_type": "lake_backfill_v1",
        "dataset": METRICS_DATASET_V1,
        "days": days,
        "replace_existing": replace_existing,
        "target_inserted_at": target_inserted_at.to_rfc3339(),
        "backfill_from_ts": start_ts.to_rfc3339(),
        "backfill_to_ts": end_ts.to_rfc3339(),
        "touched_dates": touched_dates.iter().map(|d| d.format("%Y-%m-%d").to_string()).collect::<Vec<_>>(),
        "rows_exported": total_rows,
    }))
}

fn error_from_anyhow(err: anyhow::Error) -> AnalysisJobError {
    AnalysisJobError {
        code: "internal_error".to_string(),
        message: err.to_string(),
        details: None,
    }
}

fn remove_existing_partitions(lake: &AnalysisLakeConfig, date: chrono::NaiveDate) {
    let date_dir = lake
        .dataset_root_hot(METRICS_DATASET_V1)
        .join(format!("date={}", date.format("%Y-%m-%d")));
    if date_dir.exists() {
        let _ = std::fs::remove_dir_all(&date_dir);
    }
    if let Some(cold_root) = lake.dataset_root_cold(METRICS_DATASET_V1) {
        let cold_dir = cold_root.join(format!("date={}", date.format("%Y-%m-%d")));
        if cold_dir.exists() {
            let _ = std::fs::remove_dir_all(&cold_dir);
        }
    }
}

fn partition_root_for_location(
    lake: &AnalysisLakeConfig,
    date: chrono::NaiveDate,
    location: PartitionLocation,
) -> std::path::PathBuf {
    let root = match location {
        PartitionLocation::Hot => lake.dataset_root_hot(METRICS_DATASET_V1),
        PartitionLocation::Cold => lake
            .dataset_root_cold(METRICS_DATASET_V1)
            .unwrap_or_else(|| lake.dataset_root_hot(METRICS_DATASET_V1)),
    };
    root.join(format!("date={}", date.format("%Y-%m-%d")))
}
