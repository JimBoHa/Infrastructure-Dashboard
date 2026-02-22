use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::lake::read_replication_state;
use crate::services::analysis::parquet_duckdb::DuckDbQueryService;
use crate::services::analysis::qdrant::QdrantService;
use crate::services::analysis::tsse::embeddings::{
    build_qdrant_point, compute_sensor_embeddings, TsseEmbeddingConfig, TsseEmbeddingMetadata,
};
use crate::services::analysis::tsse::types::{EmbeddingsBuildJobParamsV1, EmbeddingsBuildResultV1};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(sqlx::FromRow, Clone)]
struct SensorMetaRow {
    sensor_id: String,
    node_id: uuid::Uuid,
    sensor_type: String,
    unit: String,
    interval_seconds: i32,
    source: Option<String>,
}

pub async fn execute(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &crate::services::analysis::lake::AnalysisLakeConfig,
    qdrant: &QdrantService,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let job_started = Instant::now();
    let params: EmbeddingsBuildJobParamsV1 =
        serde_json::from_value(job.params.0.clone()).map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;

    let replication = read_replication_state(lake).unwrap_or_default();
    let computed_through = replication
        .computed_through_ts
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts.trim()).ok())
        .map(|ts| ts.with_timezone(&Utc));

    let end_inclusive = params
        .end
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts.trim()).ok())
        .map(|ts| ts.with_timezone(&Utc))
        .or(computed_through)
        .unwrap_or_else(Utc::now);

    let horizon_days = params.horizon_days.unwrap_or(30).clamp(1, 365);
    let start = params
        .start
        .as_deref()
        .and_then(|ts| DateTime::parse_from_rfc3339(ts.trim()).ok())
        .map(|ts| ts.with_timezone(&Utc))
        .unwrap_or_else(|| end_inclusive - Duration::days(horizon_days));

    if end_inclusive <= start {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "end must be after start".to_string(),
            details: None,
        }));
    }
    let end = end_inclusive + Duration::microseconds(1);
    let interval_seconds = params.interval_seconds.unwrap_or(60).max(1);
    let batch_size = params.batch_size.unwrap_or(50).clamp(1, 500) as usize;

    let sensor_ids_filter: Vec<String> = params
        .sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    let sensor_rows: Vec<SensorMetaRow> = if sensor_ids_filter.is_empty() {
        sqlx::query_as(
            r#"
            SELECT
                sensor_id,
                node_id,
                type as sensor_type,
                unit,
                interval_seconds,
                NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
            FROM sensors
            WHERE deleted_at IS NULL
            ORDER BY sensor_id
            "#,
        )
        .fetch_all(db)
        .await
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "sensor_lookup_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?
    } else {
        sqlx::query_as(
            r#"
            SELECT
                sensor_id,
                node_id,
                type as sensor_type,
                unit,
                interval_seconds,
                NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
            FROM sensors
            WHERE deleted_at IS NULL AND sensor_id = ANY($1)
            ORDER BY sensor_id
            "#,
        )
        .bind(&sensor_ids_filter)
        .fetch_all(db)
        .await
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "sensor_lookup_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?
    };

    let sensors_total = sensor_rows.len() as u64;
    let mut sensor_meta: HashMap<String, SensorMetaRow> = HashMap::new();
    let mut sensor_ids: Vec<String> = Vec::new();
    for row in sensor_rows {
        sensor_ids.push(row.sensor_id.clone());
        sensor_meta.insert(row.sensor_id.clone(), row);
    }

    let mut progress = AnalysisJobProgress {
        phase: "embedding".to_string(),
        completed: 0,
        total: Some(sensors_total),
        message: Some("Building embeddings".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    tracing::info!(
        phase = "start",
        sensors_total,
        interval_seconds,
        batch_size,
        "analysis job started"
    );

    let mut embedded: u64 = 0;
    let mut skipped: u64 = 0;
    let mut points_upserted: u64 = 0;
    let mut timings_ms: BTreeMap<String, u64> = BTreeMap::new();
    let mut duckdb_read_ms_total: u64 = 0;
    let mut qdrant_upsert_ms_total: u64 = 0;
    let embedding_config = TsseEmbeddingConfig::default();

    let start_time = Instant::now();
    for chunk in sensor_ids.chunks(batch_size) {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }

        let chunk_ids = chunk.to_vec();
        let read_started = Instant::now();
        let rows = duckdb
            .read_metrics_buckets_from_lake(lake, start, end, chunk_ids.clone(), interval_seconds)
            .await
            .map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "duckdb_read_failed".to_string(),
                    message: err.to_string(),
                    details: None,
                })
            })?;
        let read_ms = read_started.elapsed().as_millis() as u64;
        duckdb_read_ms_total += read_ms;

        let mut grouped: HashMap<
            String,
            Vec<crate::services::analysis::parquet_duckdb::MetricsBucketRow>,
        > = HashMap::new();
        for row in rows {
            grouped.entry(row.sensor_id.clone()).or_default().push(row);
        }

        let mut points: Vec<serde_json::Value> = Vec::new();
        for sensor_id in chunk_ids.iter() {
            let Some(meta) = sensor_meta.get(sensor_id) else {
                skipped += 1;
                continue;
            };
            let rows = grouped.get(sensor_id).cloned().unwrap_or_default();
            if let Some(embeddings) = compute_sensor_embeddings(&rows, &embedding_config) {
                let source = meta.source.as_deref().unwrap_or("").trim();
                let is_derived = source == "derived";
                let is_public_provider = source == "forecast_points";
                let meta = TsseEmbeddingMetadata {
                    sensor_id: meta.sensor_id.clone(),
                    node_id: Some(meta.node_id.to_string()),
                    sensor_type: Some(meta.sensor_type.clone()),
                    unit: Some(meta.unit.clone()),
                    interval_seconds: Some(meta.interval_seconds as i64),
                    computed_through_ts: replication.computed_through_ts.clone(),
                    is_derived: Some(is_derived),
                    is_public_provider: Some(is_public_provider),
                };
                points.push(build_qdrant_point(
                    &embeddings,
                    &meta,
                    &embedding_config,
                    Utc::now(),
                ));
                embedded += 1;
            } else {
                skipped += 1;
            }
        }

        if !points.is_empty() {
            let points_len = points.len() as u64;
            let upsert_started = Instant::now();
            qdrant.upsert_points(points).await.map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "qdrant_upsert_failed".to_string(),
                    message: err.to_string(),
                    details: None,
                })
            })?;
            qdrant_upsert_ms_total += upsert_started.elapsed().as_millis() as u64;
            points_upserted += points_len;
        }

        progress.completed = (embedded + skipped) as u64;
        let _ = store::update_progress(db, job.id, &progress).await;
    }
    let total_ms = start_time.elapsed().as_millis() as u64;
    timings_ms.insert("duckdb_read_ms".to_string(), duckdb_read_ms_total);
    timings_ms.insert("duckdb_load_ms".to_string(), duckdb_read_ms_total);
    timings_ms.insert("qdrant_upsert_ms".to_string(), qdrant_upsert_ms_total);
    timings_ms.insert(
        "job_total_ms".to_string(),
        job_started.elapsed().as_millis() as u64,
    );

    tracing::info!(
        phase = "embedding",
        duration_ms = total_ms,
        duckdb_read_ms = duckdb_read_ms_total,
        qdrant_upsert_ms = qdrant_upsert_ms_total,
        embedded,
        skipped,
        points_upserted,
        "analysis embeddings build complete"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "duckdb_read",
            "duration_ms": duckdb_read_ms_total,
        }),
    )
    .await;
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "qdrant_upsert",
            "duration_ms": qdrant_upsert_ms_total,
            "points_upserted": points_upserted,
        }),
    )
    .await;
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "job_total",
            "duration_ms": job_started.elapsed().as_millis() as u64,
        }),
    )
    .await;

    let result = EmbeddingsBuildResultV1 {
        job_type: "embeddings_build_v1".to_string(),
        embedding_version: embedding_config.version.clone(),
        window_seconds: embedding_config.windows_sec.clone(),
        computed_through_ts: replication.computed_through_ts.clone(),
        sensors_total,
        sensors_embedded: embedded,
        sensors_skipped: skipped,
        points_upserted,
        timings_ms,
        versions: BTreeMap::from([("embeddings".to_string(), embedding_config.version.clone())]),
    };

    serde_json::to_value(&result)
        .context("failed to serialize embeddings_build_v1 result")
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "result_encode_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })
}
