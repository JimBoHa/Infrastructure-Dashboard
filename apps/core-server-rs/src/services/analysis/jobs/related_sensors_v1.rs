use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::bucket_reader::{
    read_bucket_series_for_sensors_with_aggregation, BucketAggregationPreference,
};
use crate::services::analysis::lake::read_replication_state;
use crate::services::analysis::parquet_duckdb::MetricsBucketRow;
use crate::services::analysis::stats::fdr::bh_fdr_q_values;
use crate::services::analysis::tsse::candidate_gen::{self, FocusSensorMeta};
use crate::services::analysis::tsse::embeddings::{compute_sensor_embeddings, TsseEmbeddingConfig};
use crate::services::analysis::tsse::scoring::{
    infer_lag_inference, score_related_series_with_timings, LagInferenceSummary, ScoreParams,
};
use crate::services::analysis::tsse::types::{
    BucketAggregationModeV1, RelatedSensorCandidateV1, RelatedSensorsJobParamsV1,
    RelatedSensorsResultV1, TsseWhyRankedV1,
};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

pub async fn execute(
    db: &PgPool,
    duckdb: &crate::services::analysis::parquet_duckdb::DuckDbQueryService,
    lake: &crate::services::analysis::lake::AnalysisLakeConfig,
    qdrant: &crate::services::analysis::qdrant::QdrantService,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: RelatedSensorsJobParamsV1 =
        serde_json::from_value(job.params.0.clone()).map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;

    let start = DateTime::parse_from_rfc3339(params.start.trim())
        .map_err(|_| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: "Invalid start timestamp".to_string(),
                details: None,
            })
        })?
        .with_timezone(&Utc);
    let end_inclusive = DateTime::parse_from_rfc3339(params.end.trim())
        .map_err(|_| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: "Invalid end timestamp".to_string(),
                details: None,
            })
        })?
        .with_timezone(&Utc);
    if end_inclusive <= start {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "end must be after start".to_string(),
            details: None,
        }));
    }
    let end = end_inclusive + Duration::microseconds(1);
    let horizon_seconds = (end_inclusive - start).num_seconds().max(1);

    // TSSE ADR: the engine may increase the base grid interval to keep computation bounded,
    // but must not reject a request due to size.
    let requested_interval_seconds = params.interval_seconds.max(1);
    let max_buckets: i64 = 20_000;
    let expected_buckets =
        (horizon_seconds as f64 / requested_interval_seconds as f64).ceil() as i64;
    let interval_seconds = if expected_buckets > max_buckets {
        ((horizon_seconds as f64) / (max_buckets as f64)).ceil() as i64
    } else {
        requested_interval_seconds
    }
    .max(requested_interval_seconds);

    // Reflect the effective interval in the result params so callers don’t need to infer it.
    let mut params = params;
    params.interval_seconds = interval_seconds;
    let candidate_limit = params.candidate_limit.unwrap_or(250).clamp(10, 2_000);
    let min_pool = params.min_pool.unwrap_or(200).clamp(10, 2_000);
    let lag_max_seconds = params.lag_max_seconds.unwrap_or(0).max(0);
    let min_significant_n = params.min_significant_n.unwrap_or(10).clamp(3, 100_000) as usize;
    let significance_alpha = params
        .significance_alpha
        .unwrap_or(0.05)
        .clamp(0.000_1, 0.5);
    let min_abs_r = params.min_abs_r.unwrap_or(0.2).clamp(0.0, 1.0);
    let bucket_aggregation_mode = params
        .bucket_aggregation_mode
        .unwrap_or(BucketAggregationModeV1::Auto);
    params.candidate_limit = Some(candidate_limit);
    params.min_pool = Some(min_pool);
    params.lag_max_seconds = Some(lag_max_seconds);
    params.min_significant_n = Some(min_significant_n as u32);
    params.significance_alpha = Some(significance_alpha);
    params.min_abs_r = Some(min_abs_r);
    params.bucket_aggregation_mode = Some(bucket_aggregation_mode);

    let replication = read_replication_state(lake).unwrap_or_default();
    let job_started = Instant::now();
    let timing_candidate_gen_ms: u64;
    let timing_duckdb_focus_ms: u64;
    let mut timing_duckdb_candidate_ms: u64 = 0;
    let mut timing_scoring_wall = std::time::Duration::from_millis(0);
    let mut timing_episode_ms: u64 = 0;
    let mut timing_qdrant_search_ms: u64 = 0;

    tracing::info!(
        job_id = %job.id,
        job_type = %job.job_type,
        phase = "start",
        "analysis job started"
    );

    #[derive(sqlx::FromRow)]
    struct FocusSensorRow {
        node_id: uuid::Uuid,
        sensor_type: String,
        unit: String,
    }

    let focus_meta_row: FocusSensorRow = sqlx::query_as(
        r#"
        SELECT node_id, type as sensor_type, unit
        FROM sensors
        WHERE sensor_id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(params.focus_sensor_id.trim())
    .fetch_one(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "focus_sensor_not_found".to_string(),
            message: err.to_string(),
            details: None,
        })
    })?;

    let focus_meta = FocusSensorMeta {
        node_id: Some(focus_meta_row.node_id.to_string()),
        sensor_type: Some(focus_meta_row.sensor_type.clone()),
        unit: Some(focus_meta_row.unit.clone()),
    };

    let mut progress = AnalysisJobProgress {
        phase: "candidates".to_string(),
        completed: 0,
        total: None,
        message: Some("Generating candidates".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    // Fetch focus series once (also used to build embeddings for ANN query).
    let focus_read_start = Instant::now();
    let focus_rows = read_bucket_series_for_sensors_with_aggregation(
        db,
        duckdb,
        lake,
        vec![params.focus_sensor_id.clone()],
        start,
        end,
        interval_seconds,
        to_bucket_aggregation_preference(bucket_aggregation_mode),
    )
    .await
    .map_err(|err| JobFailure::Failed(err.to_job_error()))?;
    let focus_rows = focus_rows
        .into_iter()
        .filter(|r| r.sensor_id == params.focus_sensor_id)
        .collect::<Vec<_>>();
    timing_duckdb_focus_ms = focus_read_start.elapsed().as_millis() as u64;
    tracing::info!(
        job_id = %job.id,
        phase = "duckdb_focus",
        duration_ms = timing_duckdb_focus_ms,
        rows = focus_rows.len(),
        "analysis focus series loaded"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "duckdb_focus",
            "duration_ms": timing_duckdb_focus_ms,
            "rows": focus_rows.len(),
        }),
    )
    .await;

    let candidate_gen_start = Instant::now();
    let embedding_config = TsseEmbeddingConfig::default();
    let focus_embeddings = compute_sensor_embeddings(&focus_rows, &embedding_config);

    let candidates = if let Some(focus_embeddings) = focus_embeddings {
        let (candidates, stats) = candidate_gen::generate_candidates_with_stats(
            qdrant,
            &params.focus_sensor_id,
            &focus_meta,
            &focus_embeddings,
            &params.filters,
            min_pool,
            candidate_limit,
            &embedding_config,
        )
        .await
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "candidate_generation_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;
        timing_qdrant_search_ms = stats.qdrant_search_ms;
        candidates
    } else {
        vec![]
    };
    timing_candidate_gen_ms = candidate_gen_start.elapsed().as_millis() as u64;
    tracing::info!(
        job_id = %job.id,
        phase = "candidate_gen",
        duration_ms = timing_candidate_gen_ms,
        candidate_count = candidates.len(),
        "analysis candidate generation complete"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "candidate_gen",
            "duration_ms": timing_candidate_gen_ms,
            "qdrant_search_ms": timing_qdrant_search_ms,
            "candidate_count": candidates.len(),
        }),
    )
    .await;

    progress.phase = "inference".to_string();
    progress.completed = 0;
    progress.total = Some(candidates.len() as u64);
    progress.message = Some("Computing p/q for candidates".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    let focus_rows = Arc::new(focus_rows);

    const BATCH_SIZE: usize = 25;
    const MAX_BATCH_CONCURRENCY: usize = 4;

    #[derive(Debug)]
    struct InferenceOutcome {
        inferred: Vec<(String, LagInferenceSummary)>,
        processed: usize,
        duckdb_ms: u64,
        inference_ms: u64,
    }

    #[derive(Debug)]
    struct BatchOutcome {
        candidates: Vec<RelatedSensorCandidateV1>,
        processed: usize,
        duckdb_ms: u64,
        scoring_ms: u64,
        episode_ms: u64,
    }

    let mut inference_by_id: HashMap<String, LagInferenceSummary> = HashMap::new();
    let mut timing_inference_wall = std::time::Duration::from_millis(0);

    let duckdb = duckdb.clone();
    let lake = lake.clone();
    let mut futures = FuturesUnordered::new();
    for batch in candidates.chunks(BATCH_SIZE) {
        let batch = batch.to_vec();
        let db_clone = db.clone();
        let duckdb = duckdb.clone();
        let lake = lake.clone();
        let focus_rows = focus_rows.clone();
        let cancel = cancel.clone();
        futures.push(async move {
            if cancel.is_cancelled() {
                return Err(JobFailure::Canceled);
            }

            let sensor_ids = batch
                .iter()
                .map(|c| c.sensor_id.clone())
                .collect::<Vec<_>>();
            let candidate_read_start = Instant::now();
            let rows = read_bucket_series_for_sensors_with_aggregation(
                &db_clone,
                &duckdb,
                &lake,
                sensor_ids,
                start,
                end,
                interval_seconds,
                to_bucket_aggregation_preference(bucket_aggregation_mode),
            )
            .await
            .map_err(|err| JobFailure::Failed(err.to_job_error()))?;
            let duckdb_ms = candidate_read_start.elapsed().as_millis() as u64;

            let mut by_sensor: HashMap<String, Vec<MetricsBucketRow>> = HashMap::new();
            for row in rows {
                by_sensor
                    .entry(row.sensor_id.clone())
                    .or_default()
                    .push(row);
            }

            let mut inferred = Vec::new();
            let inference_wall_start = Instant::now();
            for candidate in batch.iter() {
                if cancel.is_cancelled() {
                    return Err(JobFailure::Canceled);
                }
                let cand_rows = by_sensor.remove(&candidate.sensor_id).unwrap_or_default();
                if let Some(summary) = infer_lag_inference(ScoreParams {
                    focus: (*focus_rows).clone(),
                    candidate: cand_rows,
                    interval_seconds,
                    horizon_seconds,
                    lag_max_seconds,
                    min_significant_n,
                    significance_alpha,
                    min_abs_r,
                    ..ScoreParams::default()
                }) {
                    inferred.push((candidate.sensor_id.clone(), summary));
                }
            }
            let inference_ms = inference_wall_start.elapsed().as_millis() as u64;

            Ok(InferenceOutcome {
                inferred,
                processed: batch.len(),
                duckdb_ms,
                inference_ms,
            })
        });
        if futures.len() >= MAX_BATCH_CONCURRENCY {
            if let Some(result) = futures.next().await {
                let outcome = result?;
                timing_duckdb_candidate_ms += outcome.duckdb_ms;
                timing_inference_wall += std::time::Duration::from_millis(outcome.inference_ms);
                for (sensor_id, summary) in outcome.inferred {
                    inference_by_id.insert(sensor_id, summary);
                }
                progress.completed += outcome.processed as u64;
                if progress.completed % 10 == 0 || progress.completed == candidates.len() as u64 {
                    let _ = store::update_progress(db, job.id, &progress).await;
                }
            }
        }
    }

    while let Some(result) = futures.next().await {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        let outcome = result?;
        timing_duckdb_candidate_ms += outcome.duckdb_ms;
        timing_inference_wall += std::time::Duration::from_millis(outcome.inference_ms);
        for (sensor_id, summary) in outcome.inferred {
            inference_by_id.insert(sensor_id, summary);
        }
        progress.completed += outcome.processed as u64;
        if progress.completed % 10 == 0 || progress.completed == candidates.len() as u64 {
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }

    let timing_inference_ms = timing_inference_wall.as_millis() as u64;

    // BH-FDR across all candidates with a computed lag-corrected p-value.
    let mut p_values_for_fdr: Vec<(usize, f64)> = Vec::new();
    for (idx, candidate) in candidates.iter().enumerate() {
        if let Some(summary) = inference_by_id.get(&candidate.sensor_id) {
            if let Some(p) = summary.p_lag {
                p_values_for_fdr.push((idx, p));
            }
        }
    }
    let q_values = bh_fdr_q_values(&p_values_for_fdr);
    let mut q_by_sensor_id: HashMap<String, f64> = HashMap::new();
    for (idx, q) in q_values {
        if let Some(candidate) = candidates.get(idx) {
            q_by_sensor_id.insert(candidate.sensor_id.clone(), q);
        }
    }

    let selected = candidates
        .iter()
        .filter(|c| q_by_sensor_id.get(&c.sensor_id).copied().unwrap_or(1.0) <= significance_alpha)
        .filter(|c| {
            inference_by_id
                .get(&c.sensor_id)
                .map(|summary| summary.best_lag_abs_r >= min_abs_r)
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();

    progress.phase = "scoring".to_string();
    progress.completed = 0;
    progress.total = Some(selected.len() as u64);
    progress.message = Some("Scoring significant candidates".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    let mut ranked: Vec<RelatedSensorCandidateV1> = Vec::new();
    let mut scoring_futures = FuturesUnordered::new();
    for batch in selected.chunks(BATCH_SIZE) {
        let batch = batch.to_vec();
        let db_clone = db.clone();
        let duckdb = duckdb.clone();
        let lake = lake.clone();
        let focus_rows = focus_rows.clone();
        let cancel = cancel.clone();
        let q_by_sensor_id = q_by_sensor_id.clone();
        scoring_futures.push(async move {
            if cancel.is_cancelled() {
                return Err(JobFailure::Canceled);
            }

            let sensor_ids = batch
                .iter()
                .map(|c| c.sensor_id.clone())
                .collect::<Vec<_>>();
            let candidate_read_start = Instant::now();
            let rows = read_bucket_series_for_sensors_with_aggregation(
                &db_clone,
                &duckdb,
                &lake,
                sensor_ids,
                start,
                end,
                interval_seconds,
                to_bucket_aggregation_preference(bucket_aggregation_mode),
            )
            .await
            .map_err(|err| JobFailure::Failed(err.to_job_error()))?;
            let duckdb_ms = candidate_read_start.elapsed().as_millis() as u64;

            let mut by_sensor: HashMap<String, Vec<MetricsBucketRow>> = HashMap::new();
            for row in rows {
                by_sensor
                    .entry(row.sensor_id.clone())
                    .or_default()
                    .push(row);
            }

            let mut scored_candidates = Vec::new();
            let mut scoring_wall = std::time::Duration::from_millis(0);
            let mut episode_ms = 0_u64;
            for candidate in batch.iter() {
                if cancel.is_cancelled() {
                    return Err(JobFailure::Canceled);
                }
                let cand_rows = by_sensor.remove(&candidate.sensor_id).unwrap_or_default();
                let scoring_wall_start = Instant::now();
                let (scored, scoring_timing) = score_related_series_with_timings(ScoreParams {
                    focus: (*focus_rows).clone(),
                    candidate: cand_rows,
                    interval_seconds,
                    horizon_seconds,
                    lag_max_seconds,
                    min_significant_n,
                    significance_alpha,
                    min_abs_r,
                    ..ScoreParams::default()
                });
                scoring_wall += scoring_wall_start.elapsed();
                episode_ms += scoring_timing.episode_extract_ms;

                if let Some(scored) = scored {
                    let mut score_components = scored.score_components.clone();
                    if let Some(q_value) = q_by_sensor_id.get(&candidate.sensor_id).copied() {
                        score_components.insert("q_value".to_string(), q_value);
                    }
                    scored_candidates.push(RelatedSensorCandidateV1 {
                        sensor_id: candidate.sensor_id.clone(),
                        rank: 0,
                        score: scored.score,
                        ann: candidate.ann.clone(),
                        episodes: scored.episodes,
                        why_ranked: TsseWhyRankedV1 {
                            episode_count: 0,
                            best_window_sec: scored.best_window_sec,
                            best_lag_sec: Some(scored.best_lag_seconds),
                            best_lag_r_ci_low: scored.best_lag_r_ci_low,
                            best_lag_r_ci_high: scored.best_lag_r_ci_high,
                            coverage_pct: scored.coverage_pct,
                            score_components,
                            penalties: scored.penalties.clone(),
                            bonuses: scored.bonuses.clone(),
                        },
                    });
                }
            }

            Ok(BatchOutcome {
                candidates: scored_candidates,
                processed: batch.len(),
                duckdb_ms,
                scoring_ms: scoring_wall.as_millis() as u64,
                episode_ms,
            })
        });

        if scoring_futures.len() >= MAX_BATCH_CONCURRENCY {
            if let Some(result) = scoring_futures.next().await {
                let outcome = result?;
                timing_duckdb_candidate_ms += outcome.duckdb_ms;
                timing_scoring_wall += std::time::Duration::from_millis(outcome.scoring_ms);
                timing_episode_ms += outcome.episode_ms;
                ranked.extend(outcome.candidates);
                progress.completed += outcome.processed as u64;
                if progress.completed % 10 == 0 || progress.completed == selected.len() as u64 {
                    let _ = store::update_progress(db, job.id, &progress).await;
                }
            }
        }
    }

    while let Some(result) = scoring_futures.next().await {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        let outcome = result?;
        timing_duckdb_candidate_ms += outcome.duckdb_ms;
        timing_scoring_wall += std::time::Duration::from_millis(outcome.scoring_ms);
        timing_episode_ms += outcome.episode_ms;
        ranked.extend(outcome.candidates);
        progress.completed += outcome.processed as u64;
        if progress.completed % 10 == 0 || progress.completed == selected.len() as u64 {
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }

    let timing_scoring_ms = timing_scoring_wall.as_millis() as u64;

    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.sensor_id.cmp(&b.sensor_id))
    });
    for (idx, item) in ranked.iter_mut().enumerate() {
        item.rank = (idx + 1) as u32;
        item.why_ranked.episode_count = item.episodes.len() as u32;
        if item.why_ranked.best_window_sec.is_none() {
            item.why_ranked.best_window_sec = item.episodes.first().map(|ep| ep.window_sec);
        }
        if item.why_ranked.coverage_pct.is_none() {
            item.why_ranked.coverage_pct = item.episodes.first().map(|ep| ep.coverage * 100.0);
        }
        item.why_ranked
            .score_components
            .insert("score".to_string(), item.score);
    }

    tracing::info!(
        job_id = %job.id,
        phase = "scoring",
        duration_ms = timing_scoring_ms,
        episode_ms = timing_episode_ms,
        candidate_reads_ms = timing_duckdb_candidate_ms,
        candidates_scored = ranked.len(),
        "analysis scoring complete"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "scoring",
            "duration_ms": timing_scoring_ms,
            "episode_extract_ms": timing_episode_ms,
            "duckdb_candidate_ms": timing_duckdb_candidate_ms,
            "candidates_scored": ranked.len(),
        }),
    )
    .await;

    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("candidate_gen_ms".to_string(), timing_candidate_gen_ms);
    timings_ms.insert("qdrant_search_ms".to_string(), timing_qdrant_search_ms);
    timings_ms.insert("duckdb_focus_ms".to_string(), timing_duckdb_focus_ms);
    timings_ms.insert(
        "duckdb_candidate_ms".to_string(),
        timing_duckdb_candidate_ms,
    );
    timings_ms.insert("inference_ms".to_string(), timing_inference_ms);
    timings_ms.insert(
        "duckdb_load_ms".to_string(),
        timing_duckdb_focus_ms + timing_duckdb_candidate_ms,
    );
    timings_ms.insert("scoring_ms".to_string(), timing_scoring_ms);
    timings_ms.insert("episode_extract_ms".to_string(), timing_episode_ms);
    timings_ms.insert(
        "job_total_ms".to_string(),
        job_started.elapsed().as_millis() as u64,
    );

    let result = RelatedSensorsResultV1 {
        job_type: "related_sensors_v1".to_string(),
        focus_sensor_id: params.focus_sensor_id.clone(),
        computed_through_ts: replication.computed_through_ts.clone(),
        params,
        candidates: ranked,
        timings_ms,
        versions: BTreeMap::from([
            ("scoring".to_string(), "v3".to_string()),
            ("candidate_gen".to_string(), "v1".to_string()),
            ("embeddings".to_string(), embedding_config.version.clone()),
            ("fdr".to_string(), "bh_v1".to_string()),
        ]),
    };

    tracing::info!(
        job_id = %job.id,
        job_type = %job.job_type,
        phase = "complete",
        duration_ms = job_started.elapsed().as_millis() as u64,
        candidates = result.candidates.len(),
        "analysis job completed"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "job_total",
            "duration_ms": job_started.elapsed().as_millis() as u64,
            "candidates": result.candidates.len(),
        }),
    )
    .await;

    serde_json::to_value(&result)
        .context("failed to serialize related_sensors_v1 result")
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "result_encode_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })
}

fn to_bucket_aggregation_preference(mode: BucketAggregationModeV1) -> BucketAggregationPreference {
    match mode {
        BucketAggregationModeV1::Auto => BucketAggregationPreference::Auto,
        BucketAggregationModeV1::Avg => BucketAggregationPreference::Avg,
        BucketAggregationModeV1::Last => BucketAggregationPreference::Last,
        BucketAggregationModeV1::Sum => BucketAggregationPreference::Sum,
        BucketAggregationModeV1::Min => BucketAggregationPreference::Min,
        BucketAggregationModeV1::Max => BucketAggregationPreference::Max,
    }
}
