use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::lake::{
    read_manifest, read_replication_state, AnalysisLakeConfig, METRICS_DATASET_V1,
};
use crate::services::analysis::parquet_duckdb::DuckDbQueryService;
use anyhow::Context;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Deserialize)]
struct LakeParityCheckJobParamsV1 {
    start: String,
    end: String,
    #[serde(default)]
    sensor_ids: Vec<String>,
    #[serde(default)]
    sample: Option<u32>,
    #[serde(default)]
    fail_on_mismatch: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
struct LakeParityCheckRowV1 {
    sensor_id: String,
    pg_count: i64,
    lake_count: i64,
    matches: bool,
}

#[derive(Debug, Clone, Serialize)]
struct LakeParityPartitionV1 {
    date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_compacted_at: Option<String>,
    max_shard_files: u32,
}

#[derive(Debug, Clone, Serialize)]
struct LakeParityCheckResultV1 {
    job_type: String,
    window_requested: WindowV1,
    window_checked: WindowV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    computed_through_ts: Option<String>,
    sensor_ids: Vec<String>,
    mismatches: u64,
    rows: Vec<LakeParityCheckRowV1>,
    partitions: Vec<LakeParityPartitionV1>,
    replication: crate::services::analysis::lake::ReplicationState,
    #[serde(default)]
    timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    versions: BTreeMap<String, String>,
    report_markdown: String,
}

#[derive(Debug, Clone, Serialize)]
struct WindowV1 {
    start: String,
    end: String,
}

fn parse_ts(label: &str, value: &str) -> Result<DateTime<Utc>, JobFailure> {
    DateTime::parse_from_rfc3339(value.trim())
        .with_context(|| format!("invalid {} timestamp: {}", label, value))
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err.to_string(),
                details: None,
            })
        })
        .map(|ts| ts.with_timezone(&Utc))
}

fn list_dates_in_range(start: DateTime<Utc>, end_inclusive: DateTime<Utc>) -> Vec<NaiveDate> {
    if end_inclusive < start {
        return Vec::new();
    }
    let mut dates = Vec::new();
    let mut cursor = start.date_naive();
    let end_date = end_inclusive.date_naive();
    while cursor <= end_date {
        dates.push(cursor);
        let Some(next) = cursor.succ_opt() else {
            break;
        };
        if next == cursor {
            break;
        }
        cursor = next;
    }
    dates
}

async fn pick_sensor_ids(
    db: &PgPool,
    params: &LakeParityCheckJobParamsV1,
) -> Result<Vec<String>, JobFailure> {
    let mut ids: Vec<String> = params
        .sensor_ids
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .collect();
    if !ids.is_empty() {
        ids.sort();
        ids.dedup();
        return Ok(ids);
    }

    let limit = params.sample.unwrap_or(5).max(1) as i64;
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE deleted_at IS NULL
        ORDER BY sensor_id
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "db_query_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })?;

    if rows.is_empty() {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "no_sensors".to_string(),
            message: "no sensors available for parity check".to_string(),
            details: None,
        }));
    }
    Ok(rows)
}

async fn pg_count(
    db: &PgPool,
    sensor_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<i64, JobFailure> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)::bigint as count
        FROM metrics
        WHERE sensor_id = $1
          AND ts >= $2
          AND ts <= $3
        "#,
    )
    .bind(sensor_id)
    .bind(start)
    .bind(end)
    .fetch_one(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "db_query_failed".to_string(),
            message: err.to_string(),
            details: Some(serde_json::json!({ "sensor_id": sensor_id })),
        })
    })?;
    Ok(count)
}

fn count_parquet_files_in_shard(partition_dir: &std::path::Path, shard: u32) -> u32 {
    let shard_dir = partition_dir.join(format!("shard={:02}", shard));
    if !shard_dir.exists() {
        return 0;
    }
    match std::fs::read_dir(&shard_dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|v| v.to_str()) == Some("parquet"))
            .count() as u32,
        Err(_) => 0,
    }
}

fn partition_root_for_date(
    lake: &AnalysisLakeConfig,
    manifest: &crate::services::analysis::lake::LakeManifest,
    date: NaiveDate,
) -> std::path::PathBuf {
    let key = date.format("%Y-%m-%d").to_string();
    let location = manifest
        .datasets
        .get(METRICS_DATASET_V1)
        .and_then(|ds| ds.partitions.get(&key))
        .map(|p| p.location.as_str())
        .unwrap_or("hot");
    match location {
        "cold" => lake
            .dataset_root_cold(METRICS_DATASET_V1)
            .unwrap_or_else(|| lake.dataset_root_hot(METRICS_DATASET_V1))
            .join(format!("date={}", key)),
        _ => lake
            .dataset_root_hot(METRICS_DATASET_V1)
            .join(format!("date={}", key)),
    }
}

pub async fn execute(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: LakeParityCheckJobParamsV1 =
        serde_json::from_value(job.params.0.clone()).map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;

    let start = parse_ts("start", &params.start)?;
    let end_inclusive = parse_ts("end", &params.end)?;
    if end_inclusive <= start {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "end must be after start".to_string(),
            details: None,
        }));
    }

    let replication = read_replication_state(lake).unwrap_or_default();
    let computed_through = replication
        .computed_through_ts
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts.trim()).ok())
        .map(|ts| ts.with_timezone(&Utc));
    let effective_end = computed_through
        .map(|ct| std::cmp::min(ct, end_inclusive))
        .unwrap_or(end_inclusive);
    if effective_end <= start {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_window".to_string(),
            message: "requested window is newer than lake watermark".to_string(),
            details: Some(serde_json::json!({
                "start": start.to_rfc3339(),
                "end": end_inclusive.to_rfc3339(),
                "computed_through_ts": replication.computed_through_ts,
            })),
        }));
    }
    let end = effective_end + Duration::microseconds(1);

    let sensor_ids = pick_sensor_ids(db, &params).await?;
    let mut progress = AnalysisJobProgress {
        phase: "parity_check".to_string(),
        completed: 0,
        total: Some(sensor_ids.len() as u64),
        message: Some("Checking Postgres ↔ Parquet counts".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    let job_started = Instant::now();
    let pg_started = Instant::now();
    let mut rows: Vec<LakeParityCheckRowV1> = Vec::new();
    let mut mismatches: u64 = 0;

    for (idx, sensor_id) in sensor_ids.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        let pg_count = pg_count(db, sensor_id, start, effective_end).await?;
        let lake_rows = duckdb
            .read_metrics_points_from_lake(lake, start, end, vec![sensor_id.clone()], None)
            .await
            .map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "duckdb_read_failed".to_string(),
                    message: err.to_string(),
                    details: Some(serde_json::json!({ "sensor_id": sensor_id })),
                })
            })?;
        let lake_count = lake_rows.len() as i64;
        let matches = pg_count == lake_count;
        if !matches {
            mismatches += 1;
        }
        rows.push(LakeParityCheckRowV1 {
            sensor_id: sensor_id.clone(),
            pg_count,
            lake_count,
            matches,
        });

        progress.completed = (idx + 1) as u64;
        if idx % 5 == 0 || idx + 1 == sensor_ids.len() {
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }
    let pg_ms = pg_started.elapsed().as_millis() as u64;

    // Partition file-count summary for the relevant shard set and date range.
    let manifest = read_manifest(lake).unwrap_or_default();
    let shard_set: BTreeSet<u32> = sensor_ids
        .iter()
        .map(|id| lake.shard_for_sensor_id(id))
        .collect();
    let dates = list_dates_in_range(start, effective_end);
    let mut partitions: Vec<LakeParityPartitionV1> = Vec::new();
    for date in dates {
        let key = date.format("%Y-%m-%d").to_string();
        let ds = manifest.datasets.get(METRICS_DATASET_V1);
        let part = ds.and_then(|ds| ds.partitions.get(&key));
        let location = part.map(|p| p.location.clone()).filter(|v| !v.is_empty());
        let file_count = part.and_then(|p| p.file_count);
        let last_compacted_at = part.and_then(|p| p.last_compacted_at.clone());
        let partition_root = partition_root_for_date(lake, &manifest, date);
        let mut max_shard_files: u32 = 0;
        for shard in shard_set.iter().copied() {
            max_shard_files =
                max_shard_files.max(count_parquet_files_in_shard(&partition_root, shard));
        }
        partitions.push(LakeParityPartitionV1 {
            date: key,
            location,
            file_count,
            last_compacted_at,
            max_shard_files,
        });
    }

    let fail_on_mismatch = params.fail_on_mismatch.unwrap_or(false);
    if fail_on_mismatch && mismatches > 0 {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "parity_mismatch".to_string(),
            message: "parity mismatches detected".to_string(),
            details: Some(serde_json::json!({ "mismatches": mismatches })),
        }));
    }

    let mut report = String::new();
    report.push_str("# TSSE Lake Parity Check (job)\n\n");
    report.push_str(&format!(
        "- computed_through_ts: `{}`\n",
        replication
            .computed_through_ts
            .clone()
            .unwrap_or_else(|| "—".to_string())
    ));
    report.push_str(&format!(
        "- Window (requested): `{}` → `{}`\n",
        start.to_rfc3339(),
        end_inclusive.to_rfc3339()
    ));
    report.push_str(&format!(
        "- Window (checked): `{}` → `{}`\n\n",
        start.to_rfc3339(),
        effective_end.to_rfc3339()
    ));
    report.push_str(
        "| Sensor | Postgres count | Lake count | Match |\n| --- | ---: | ---: | :---: |\n",
    );
    for row in &rows {
        report.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            row.sensor_id,
            row.pg_count,
            row.lake_count,
            if row.matches { "✅" } else { "❌" }
        ));
    }
    report.push_str("\n## Partition file counts (shard subset)\n\n");
    report.push_str("| Date | location | manifest file_count | max shard parquet files |\n| --- | --- | ---: | ---: |\n");
    for part in &partitions {
        report.push_str(&format!(
            "| `{}` | `{}` | {} | {} |\n",
            part.date,
            part.location.clone().unwrap_or_else(|| "—".to_string()),
            part.file_count
                .map(|v| v.to_string())
                .unwrap_or_else(|| "—".to_string()),
            part.max_shard_files
        ));
    }
    report.push_str("\n## Replication summary\n\n");
    report.push_str(&format!(
        "- last_run_backlog_seconds: `{}`\n- last_run_row_count: `{}`\n- last_run_duration_ms: `{}`\n",
        replication
            .last_run_backlog_seconds
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        replication
            .last_run_row_count
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        replication
            .last_run_duration_ms
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
    ));

    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("pg_counts_ms".to_string(), pg_ms);
    timings_ms.insert(
        "job_total_ms".to_string(),
        job_started.elapsed().as_millis() as u64,
    );

    let result = LakeParityCheckResultV1 {
        job_type: "lake_parity_check_v1".to_string(),
        window_requested: WindowV1 {
            start: start.to_rfc3339(),
            end: end_inclusive.to_rfc3339(),
        },
        window_checked: WindowV1 {
            start: start.to_rfc3339(),
            end: effective_end.to_rfc3339(),
        },
        computed_through_ts: replication.computed_through_ts.clone(),
        sensor_ids,
        mismatches,
        rows,
        partitions,
        replication,
        timings_ms,
        versions: BTreeMap::from([("parity_check".to_string(), "v1".to_string())]),
        report_markdown: report,
    };

    serde_json::to_value(result).map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "serialize_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })
}
