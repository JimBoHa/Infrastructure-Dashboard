//! Materializes historical forecast points as regular metrics.
//!
//! This job queries `forecast_points` for timestamps that have passed and writes
//! them to the `metrics` table with synthetic sensor IDs. This enables querying
//! historical forecasts through the unified `/api/metrics/query` endpoint.
//!
//! Synthetic sensor ID format: `forecast:{provider}:{kind}:{metric}:{subject}`
//!
//! The job tracks a watermark to avoid reprocessing already-materialized data.

use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

/// How far back to look when starting fresh (no watermark).
const DEFAULT_LOOKBACK_DAYS: i64 = 30;

/// Maximum points to process per batch to avoid memory pressure.
const BATCH_SIZE: i64 = 10_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ForecastMaterializeParams {
    /// Optional: Override the lookback period when no watermark exists.
    #[serde(default)]
    lookback_days: Option<i64>,
    /// Optional: Force reprocessing from a specific timestamp.
    #[serde(default)]
    force_from_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ForecastMaterializeResultV1 {
    job_type: String,
    status: String,
    points_materialized: i64,
    start_ts: String,
    end_ts: String,
    new_watermark: String,
}

/// Row from the forecast_points query for materialization.
#[derive(sqlx::FromRow)]
struct ForecastPointRow {
    provider: String,
    kind: String,
    subject_kind: String,
    subject: String,
    metric: String,
    ts: DateTime<Utc>,
    value: f64,
}

pub async fn execute(
    db: &PgPool,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let params: ForecastMaterializeParams =
        serde_json::from_value(job.params.0.clone()).unwrap_or(ForecastMaterializeParams {
            lookback_days: None,
            force_from_ts: None,
        });

    // Determine start timestamp from watermark or params
    let watermark = load_watermark(db).await;
    let start_ts = if let Some(force_from) = params.force_from_ts.as_deref() {
        DateTime::parse_from_rfc3339(force_from)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| {
                JobFailure::Failed(AnalysisJobError {
                    code: "invalid_force_from_ts".to_string(),
                    message: "force_from_ts is not a valid RFC3339 timestamp".to_string(),
                    details: None,
                })
            })?
    } else if let Some(wm) = watermark {
        wm
    } else {
        let lookback = params.lookback_days.unwrap_or(DEFAULT_LOOKBACK_DAYS);
        Utc::now() - Duration::days(lookback)
    };

    // End timestamp is now minus a small lag to ensure data is stable
    let end_ts = Utc::now() - Duration::minutes(5);

    if start_ts >= end_ts {
        // Nothing to do
        let result = ForecastMaterializeResultV1 {
            job_type: "forecast_materialize_v1".to_string(),
            status: "ok".to_string(),
            points_materialized: 0,
            start_ts: start_ts.to_rfc3339(),
            end_ts: end_ts.to_rfc3339(),
            new_watermark: start_ts.to_rfc3339(),
        };
        return serde_json::to_value(result).map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "serialize_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        });
    }

    let mut progress = AnalysisJobProgress {
        phase: "materialize".to_string(),
        completed: 0,
        total: None,
        message: Some("Querying forecast points...".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    // Query distinct forecast point combinations with as-of values
    // For each (provider, kind, subject, metric, ts), get the value from the most recent issued_at <= ts
    let rows: Vec<ForecastPointRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (provider, kind, subject_kind, subject, metric, ts)
            provider,
            kind,
            subject_kind,
            subject,
            metric,
            ts,
            value
        FROM forecast_points
        WHERE ts >= $1
          AND ts < $2
          AND issued_at <= ts
        ORDER BY provider, kind, subject_kind, subject, metric, ts, issued_at DESC
        LIMIT $3
        "#,
    )
    .bind(start_ts)
    .bind(end_ts)
    .bind(BATCH_SIZE)
    .fetch_all(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "query_failed".to_string(),
            message: format!("Failed to query forecast points: {}", err),
            details: None,
        })
    })?;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let total_points = rows.len();
    progress.total = Some(total_points as u64);
    progress.message = Some(format!("Materializing {} points...", total_points));
    let _ = store::update_progress(db, job.id, &progress).await;

    let mut materialized_count: i64 = 0;
    let mut max_ts = start_ts;

    for (idx, row) in rows.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }

        // Build synthetic sensor ID: forecast:{provider}:{kind}:{metric}:{subject}
        let sensor_id = format!(
            "forecast:{}:{}:{}:{}",
            row.provider, row.kind, row.metric, row.subject
        );

        // Ensure sensor exists (create if not)
        ensure_forecast_sensor(db, &sensor_id, &row).await?;

        // Insert metric (ignore conflicts - idempotent)
        let result = sqlx::query(
            r#"
            INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at)
            VALUES ($1, $2, $3, 0, now())
            ON CONFLICT (sensor_id, ts) DO NOTHING
            "#,
        )
        .bind(&sensor_id)
        .bind(row.ts)
        .bind(row.value)
        .execute(db)
        .await
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "insert_failed".to_string(),
                message: format!("Failed to insert metric: {}", err),
                details: None,
            })
        })?;

        if result.rows_affected() > 0 {
            materialized_count += 1;
        }

        if row.ts > max_ts {
            max_ts = row.ts;
        }

        // Update progress every 100 points
        if idx % 100 == 0 || idx + 1 == total_points {
            progress.completed = (idx + 1) as u64;
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }

    // Update watermark to the max ts we processed
    let new_watermark = if total_points == BATCH_SIZE as usize {
        // We hit the batch limit, use max_ts as watermark to continue next time
        max_ts
    } else {
        // We processed everything, use end_ts as watermark
        end_ts
    };

    save_watermark(db, new_watermark).await?;

    let result = ForecastMaterializeResultV1 {
        job_type: "forecast_materialize_v1".to_string(),
        status: "ok".to_string(),
        points_materialized: materialized_count,
        start_ts: start_ts.to_rfc3339(),
        end_ts: end_ts.to_rfc3339(),
        new_watermark: new_watermark.to_rfc3339(),
    };

    serde_json::to_value(result).map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "serialize_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })
}

/// Load the watermark from forecast_materialize_state table.
async fn load_watermark(db: &PgPool) -> Option<DateTime<Utc>> {
    #[derive(sqlx::FromRow)]
    struct WatermarkRow {
        watermark_ts: Option<DateTime<Utc>>,
    }

    let row: Option<WatermarkRow> = sqlx::query_as(
        r#"
        SELECT watermark_ts FROM forecast_materialize_state WHERE id = 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    row.and_then(|r| r.watermark_ts)
}

/// Save the watermark to forecast_materialize_state table.
async fn save_watermark(db: &PgPool, ts: DateTime<Utc>) -> Result<(), JobFailure> {
    sqlx::query(
        r#"
        UPDATE forecast_materialize_state
        SET watermark_ts = $1, updated_at = now()
        WHERE id = 1
        "#,
    )
    .bind(ts)
    .execute(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "watermark_save_failed".to_string(),
            message: format!("Failed to save watermark: {}", err),
            details: None,
        })
    })?;

    Ok(())
}

/// Ensure a sensor exists for the materialized forecast data.
async fn ensure_forecast_sensor(
    db: &PgPool,
    sensor_id: &str,
    row: &ForecastPointRow,
) -> Result<(), JobFailure> {
    // Check if sensor already exists
    let exists: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT sensor_id FROM sensors WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "sensor_check_failed".to_string(),
            message: format!("Failed to check sensor: {}", err),
            details: None,
        })
    })?;

    if exists.is_some() {
        return Ok(());
    }

    // Get the node_id for this subject (if it's a node-type subject)
    let node_id: Option<uuid::Uuid> = if row.subject_kind == "node" {
        sqlx::query_scalar(
            r#"
            SELECT id FROM nodes WHERE id::text = $1 OR hostname = $1 LIMIT 1
            "#,
        )
        .bind(&row.subject)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
    } else {
        // For controller-level forecasts, use a placeholder node or the first node
        sqlx::query_scalar(
            r#"
            SELECT id FROM nodes ORDER BY created_at ASC LIMIT 1
            "#,
        )
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
    };

    let Some(node_id) = node_id else {
        // No nodes exist, can't create sensor
        return Ok(());
    };

    // Create sensor with forecast metadata
    let sensor_name = format!("Forecast: {} {} ({})", row.kind, row.metric, row.subject);

    let config = serde_json::json!({
        "source": "forecast_materialized",
        "provider": row.provider,
        "kind": row.kind,
        "subject_kind": row.subject_kind,
        "subject": row.subject,
        "metric": row.metric
    });

    sqlx::query(
        r#"
        INSERT INTO sensors (sensor_id, node_id, name, unit, config, created_at)
        VALUES ($1, $2, $3, '', $4, now())
        ON CONFLICT (sensor_id) DO NOTHING
        "#,
    )
    .bind(sensor_id)
    .bind(node_id)
    .bind(&sensor_name)
    .bind(config)
    .execute(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "sensor_create_failed".to_string(),
            message: format!("Failed to create sensor: {}", err),
            details: None,
        })
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_synthetic_sensor_id_format() {
        let sensor_id = format!(
            "forecast:{}:{}:{}:{}",
            "solcast", "pv", "pv_power_w", "node-123"
        );
        assert_eq!(sensor_id, "forecast:solcast:pv:pv_power_w:node-123");
    }
}
