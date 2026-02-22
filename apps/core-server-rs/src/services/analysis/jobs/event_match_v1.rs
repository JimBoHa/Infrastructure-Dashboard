use super::event_utils::{
    detect_change_events_with_options_and_delta_mode, hour_of_day_mean_residual_rows,
    time_of_day_entropy, DetectedEvents, EventDetectOptionsV1, EventPoint,
};
use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::bucket_reader::{
    read_bucket_series_for_sensors_with_aggregation_and_options, BucketAggregationPreference,
};
use crate::services::analysis::lake::read_replication_state;
use crate::services::analysis::parquet_duckdb::{DuckDbQueryService, MetricsBucketReadOptions};
use crate::services::analysis::signal_semantics::{self, DeltaMode};
use crate::services::analysis::tsse::types::{
    DeseasonModeV1, DirectionLabelV1, EventDetectorModeV1, EventEvidenceMonitoringV1,
    EventDirectionV1, EventMatchCandidateV1, EventMatchJobParamsV1, EventMatchLagScoreV1,
    EventMatchResultV1, EventPolarityV1, EventSuppressionModeV1, EventThresholdModeV1,
    ExplicitFocusEventV1, TsseCandidateFiltersV1, TsseEpisodeV1, TsseWhyRankedV1,
};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::cmp::Ordering;
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
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: EventMatchJobParamsV1 =
        serde_json::from_value(job.params.0.clone()).map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;

    let focus_sensor_id = params.focus_sensor_id.trim().to_string();
    if focus_sensor_id.is_empty() {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "focus_sensor_id is required".to_string(),
            details: None,
        }));
    }

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

    let mut interval_seconds = params.interval_seconds.unwrap_or(60).max(1);
    let max_buckets = params.max_buckets.unwrap_or(12_000).clamp(300, 50_000) as i64;
    let horizon_seconds = (end_inclusive - start).num_seconds().max(1);
    let expected_buckets = (horizon_seconds as f64 / interval_seconds as f64).ceil() as i64;
    if expected_buckets > max_buckets {
        interval_seconds = ((horizon_seconds as f64) / (max_buckets as f64)).ceil() as i64;
    }
    let bucket_count = ((horizon_seconds as f64) / interval_seconds as f64)
        .ceil()
        .max(1.0) as u64;

    let polarity = params.polarity.unwrap_or(EventPolarityV1::Both);
    let z_threshold = params.z_threshold.unwrap_or(3.0).max(0.1);
    let min_separation_buckets = params.min_separation_buckets.unwrap_or(2).max(0);
    let gap_max_buckets = params.gap_max_buckets.unwrap_or(5).max(0);
    let max_events = params.max_events.unwrap_or(2_000).clamp(100, 20_000) as usize;
    let suppression_mode = params
        .suppression_mode
        .unwrap_or(EventSuppressionModeV1::NmsWindow);
    let threshold_mode = params
        .threshold_mode
        .unwrap_or(EventThresholdModeV1::FixedZ);
    let adaptive = params.adaptive_threshold.clone();
    let exclude_boundary_events = params.exclude_boundary_events.unwrap_or(false);
    let detector_mode = params.detector_mode.unwrap_or(EventDetectorModeV1::BucketDeltas);
    let sparse_point_events_enabled = params.sparse_point_events_enabled.unwrap_or(false);
    let max_lag_buckets = params.max_lag_buckets.unwrap_or(12).clamp(0, 360);
    let top_k_lags = params.top_k_lags.unwrap_or(0).clamp(0, 3) as usize;
    let tolerance_buckets = params.tolerance_buckets.unwrap_or(0).max(0);
    let tolerance_sec = tolerance_buckets.saturating_mul(interval_seconds.max(1));
    let max_episodes = params.max_episodes.unwrap_or(24).clamp(1, 200) as usize;
    let episode_gap_buckets = params.episode_gap_buckets.unwrap_or(6).max(1);
    let candidate_limit = params.candidate_limit.unwrap_or(50).clamp(5, 10_000) as usize;
    let z_cap = params.z_cap.unwrap_or(15.0).clamp(1.0, 1_000.0);
    let deseason_mode = params.deseason_mode.unwrap_or(DeseasonModeV1::None);
    let periodic_penalty_enabled = params.periodic_penalty_enabled.unwrap_or(false);
    let apply_deseasoning = matches!(deseason_mode, DeseasonModeV1::HourOfDayMean)
        && horizon_seconds >= 2 * 86_400;

    let replication = read_replication_state(lake).unwrap_or_default();
    let job_started = Instant::now();

    let mut progress = AnalysisJobProgress {
        phase: "load_series".to_string(),
        completed: 0,
        total: Some(candidate_limit as u64),
        message: Some("Loading bucketed series".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let focus_meta: SensorMetaRow = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            node_id,
            type as sensor_type,
            unit,
            interval_seconds,
            NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        WHERE sensor_id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(&focus_sensor_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "focus_sensor_not_found".to_string(),
            message: err.to_string(),
            details: None,
        })
    })?;

    let filters = params.filters.clone();
    let mut candidate_sensor_ids: Vec<String> = params
        .candidate_sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    candidate_sensor_ids.retain(|id| id != &focus_sensor_id);
    candidate_sensor_ids.sort();
    candidate_sensor_ids.dedup();

    if candidate_sensor_ids.is_empty() {
        candidate_sensor_ids =
            fetch_candidate_sensors(db, &focus_meta, &filters, candidate_limit).await?;
    }

    let mut truncated_sensor_ids: Vec<String> = Vec::new();
    if candidate_sensor_ids.len() > candidate_limit {
        truncated_sensor_ids = candidate_sensor_ids.split_off(candidate_limit);
    }

    let mut sensor_ids: Vec<String> = Vec::with_capacity(candidate_sensor_ids.len() + 1);
    sensor_ids.push(focus_sensor_id.clone());
    sensor_ids.extend(candidate_sensor_ids.clone());

    let meta_rows: Vec<SensorMetaRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            node_id,
            type as sensor_type,
            unit,
            interval_seconds,
            NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        WHERE sensor_id = ANY($1) AND deleted_at IS NULL
        ORDER BY sensor_id
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "sensor_lookup_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })?;

    let mut delta_mode_by_id: HashMap<String, DeltaMode> = HashMap::new();
    for row in meta_rows {
        delta_mode_by_id.insert(
            row.sensor_id.clone(),
            signal_semantics::auto_delta_mode(Some(&row.sensor_type), row.source.as_deref()),
        );
    }
    let focus_delta_mode = delta_mode_by_id
        .get(&focus_sensor_id)
        .copied()
        .unwrap_or(DeltaMode::Linear);

    tracing::info!(
        phase = "start",
        focus_sensor_id = %focus_sensor_id,
        candidate_count = candidate_sensor_ids.len(),
        interval_seconds,
        "analysis job started"
    );

    let load_started = Instant::now();
    let rows = read_bucket_series_for_sensors_with_aggregation_and_options(
        db,
        duckdb,
        lake,
        sensor_ids.clone(),
        start,
        end,
        interval_seconds,
        BucketAggregationPreference::Auto,
        MetricsBucketReadOptions::analysis_default(),
    )
    .await
    .map_err(|err| JobFailure::Failed(err.to_job_error()))?;
    let load_ms = load_started.elapsed().as_millis() as u64;
    tracing::info!(
        phase = "load_series",
        duration_ms = load_ms,
        sensor_count = sensor_ids.len(),
        "analysis series loaded"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "load_series",
            "duration_ms": load_ms,
            "sensor_count": sensor_ids.len(),
        }),
    )
    .await;

    let mut grouped: HashMap<
        String,
        Vec<crate::services::analysis::parquet_duckdb::MetricsBucketRow>,
    > = HashMap::new();
    for row in rows {
        grouped.entry(row.sensor_id.clone()).or_default().push(row);
    }
    for values in grouped.values_mut() {
        values.sort_by_key(|r| r.bucket);
    }

    progress.phase = "detect_events".to_string();
    progress.completed = 0;
    progress.total = Some(sensor_ids.len() as u64);
    progress.message = Some("Detecting events".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    let detect_started = Instant::now();
    let focus_rows_raw = grouped.get(&focus_sensor_id).cloned().unwrap_or_default();
    let focus_rows = if apply_deseasoning {
        hour_of_day_mean_residual_rows(&focus_rows_raw)
    } else {
        focus_rows_raw.clone()
    };
    let mut gap_skipped_deltas: BTreeMap<String, u64> = BTreeMap::new();
    let mut detect_options = EventDetectOptionsV1::fixed_default(
        interval_seconds,
        z_threshold,
        min_separation_buckets,
        gap_max_buckets,
        polarity,
        max_events,
    );
    detect_options.suppression_mode = suppression_mode;
    detect_options.threshold_mode = threshold_mode;
    detect_options.adaptive = adaptive;
    detect_options.exclude_boundary_events = exclude_boundary_events;
    detect_options.detector_mode = detector_mode;
    detect_options.sparse_point_events_enabled = sparse_point_events_enabled;

    let focus_detected =
        detect_change_events_with_options_and_delta_mode(&focus_rows, &detect_options, focus_delta_mode);
    gap_skipped_deltas.insert(focus_sensor_id.clone(), focus_detected.gap_skipped_deltas);

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let mut candidate_events: HashMap<String, DetectedEvents> = HashMap::new();
    let mut preprocessed_rows_by_id: HashMap<String, Vec<crate::services::analysis::parquet_duckdb::MetricsBucketRow>> =
        HashMap::new();
    preprocessed_rows_by_id.insert(focus_sensor_id.clone(), focus_rows.clone());
    let mut processed: u64 = 0;
    for sensor_id in candidate_sensor_ids.iter() {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        let raw_rows = grouped.get(sensor_id).cloned().unwrap_or_default();
        let rows = if apply_deseasoning {
            hour_of_day_mean_residual_rows(&raw_rows)
        } else {
            raw_rows
        };
        preprocessed_rows_by_id.insert(sensor_id.clone(), rows.clone());
        let delta_mode = delta_mode_by_id
            .get(sensor_id)
            .copied()
            .unwrap_or(DeltaMode::Linear);
        let detected =
            detect_change_events_with_options_and_delta_mode(&rows, &detect_options, delta_mode);
        gap_skipped_deltas.insert(sensor_id.clone(), detected.gap_skipped_deltas);
        candidate_events.insert(sensor_id.clone(), detected);
        processed += 1;
        if processed % 5 == 0 || processed == candidate_sensor_ids.len() as u64 {
            progress.completed = processed + 1;
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }
    let detect_ms = detect_started.elapsed().as_millis() as u64;
    let candidate_events_total: usize = candidate_events.values().map(|v| v.events.len()).sum();
    tracing::info!(
        phase = "detect_events",
        duration_ms = detect_ms,
        focus_events = focus_detected.events.len(),
        candidate_events = candidate_events_total,
        "analysis event detection complete"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "detect_events",
            "duration_ms": detect_ms,
            "focus_events": focus_detected.events.len(),
            "candidate_events": candidate_events_total,
        }),
    )
    .await;

    let monitoring = compute_evidence_monitoring(z_cap, &focus_detected, &candidate_events);

    progress.phase = "match_candidates".to_string();
    progress.completed = 0;
    progress.total = Some(candidate_sensor_ids.len() as u64);
    progress.message = Some("Scoring event matches".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    let match_started = Instant::now();
    let mut candidates: Vec<EventMatchCandidateV1> = Vec::new();
    let explicit_focus_events =
        normalize_explicit_focus_events(&params.focus_events, start, end_inclusive, max_events);
    let focus_events_explicit = !explicit_focus_events.is_empty();
    let focus_events_for_match: Vec<EventPoint> = if focus_events_explicit {
        explicit_focus_events
    } else {
        focus_detected.events.clone()
    };

    let mut focus_sorted: Vec<&EventPoint> = focus_events_for_match.iter().collect();
    focus_sorted.sort_by_key(|e| e.ts_epoch);
    focus_sorted.dedup_by_key(|e| e.ts_epoch);
    let n_focus = focus_sorted.len() as u64;
    let focus_weight_sum = sum_event_weights(&focus_sorted, z_cap);
    let focus_deltas = collect_gap_aware_bucket_deltas_with_delta_mode(
        &focus_rows,
        interval_seconds,
        gap_max_buckets,
        focus_delta_mode,
    );
    let n_focus_up = if focus_events_explicit {
        None
    } else {
        Some(focus_detected.up_events)
    };
    let n_focus_down = if focus_events_explicit {
        None
    } else {
        Some(focus_detected.down_events)
    };

    for (idx, sensor_id) in candidate_sensor_ids.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }

        let detected = candidate_events
            .get(sensor_id)
            .ok_or_else(|| JobFailure::Failed(AnalysisJobError {
                code: "candidate_events_missing".to_string(),
                message: format!("missing detected events for {sensor_id}"),
                details: None,
            }))?;
        let mut candidate_sorted: Vec<&EventPoint> = detected.events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);
        candidate_sorted.dedup_by_key(|e| e.ts_epoch);
        let n_candidate = candidate_sorted.len() as u64;
        let candidate_weight_sum = sum_event_weights(&candidate_sorted, z_cap);
        let n_candidate_up = detected.up_events;
        let n_candidate_down = detected.down_events;

        let (zero_lag, best_lag, top_lags) = if max_lag_buckets > 0 {
            let mut lag_scores = all_lag_scores_weighted(
                &focus_sorted,
                &candidate_sorted,
                max_lag_buckets,
                interval_seconds,
                tolerance_sec,
                focus_weight_sum,
                candidate_weight_sum,
                n_candidate,
                z_cap,
            );
            let zero_lag = lag_scores
                .iter()
                .find(|score| score.lag_sec == 0)
                .cloned()
                .unwrap_or_else(|| {
                    score_lag_weighted(
                        &focus_sorted,
                        &candidate_sorted,
                        0,
                        interval_seconds,
                        tolerance_sec,
                        focus_weight_sum,
                        candidate_weight_sum,
                        n_candidate,
                        z_cap,
                    )
                });
            lag_scores.sort_by(rank_lag_score);
            let best_lag = lag_scores.first().cloned();
            let top_lags = if top_k_lags > 0 {
                lag_scores.into_iter().take(top_k_lags).collect()
            } else {
                Vec::new()
            };
            (zero_lag, best_lag, top_lags)
        } else {
            let zero_lag = score_lag_weighted(
                &focus_sorted,
                &candidate_sorted,
                0,
                interval_seconds,
                tolerance_sec,
                focus_weight_sum,
                candidate_weight_sum,
                n_candidate,
                z_cap,
            );
            let top_lags = if top_k_lags > 0 {
                vec![zero_lag.clone()]
            } else {
                Vec::new()
            };
            (zero_lag, None, top_lags)
        };

        let (score, overlap) = best_lag
            .as_ref()
            .map(|best| (best.score, best.overlap))
            .unwrap_or((zero_lag.score, zero_lag.overlap));

        let best_lag_sec = best_lag.as_ref().map(|b| b.lag_sec).unwrap_or(0);
        let matched_pairs = collect_matched_event_pairs(
            &focus_sorted,
            &candidate_sorted,
            best_lag_sec,
            tolerance_sec,
        );
        let (overlap_weighted, overlap_weighted_sum) =
            weighted_overlap_sum(&focus_sorted, &candidate_sorted, best_lag_sec, tolerance_sec, z_cap);
        let direction_n = matched_pairs.len() as u64;
        let sign_agreement = if focus_events_explicit {
            None
        } else {
            sign_agreement_from_matched_pairs(&matched_pairs)
        };
        let candidate_rows = preprocessed_rows_by_id
            .get(sensor_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let candidate_delta_mode = delta_mode_by_id
            .get(sensor_id)
            .copied()
            .unwrap_or(DeltaMode::Linear);
        let candidate_deltas = collect_gap_aware_bucket_deltas_with_delta_mode(
            candidate_rows,
            interval_seconds,
            gap_max_buckets,
            candidate_delta_mode,
        );
        let delta_corr =
            delta_corr_from_aligned_deltas(&focus_deltas, &candidate_deltas, best_lag_sec, 10);
        let direction_label = compute_direction_label(direction_n, delta_corr, sign_agreement);
        let matched_focus_events: Vec<&EventPoint> =
            matched_pairs.iter().map(|(focus, _)| *focus).collect();
        let mut episodes = build_event_episodes(
            &matched_focus_events,
            best_lag_sec,
            interval_seconds,
            episode_gap_buckets,
            max_episodes,
            z_cap,
            n_focus,
        );
        let entropy = if periodic_penalty_enabled {
            time_of_day_entropy(&detected.events)
        } else {
            None
        };
        if let Some(entropy) = entropy {
            if entropy.weight.is_finite() && entropy.weight > 0.0 && entropy.weight < 1.0 {
                for ep in episodes.iter_mut() {
                    ep.score_mean *= entropy.weight;
                    ep.score_peak *= entropy.weight;
                }
            }
        }

        let best_window_sec = episodes.first().map(|ep| ep.window_sec);
        let coverage_pct = episodes.first().map(|ep| ep.coverage * 100.0);
        let episode_count = episodes.len() as u32;
        let mut score_components = BTreeMap::new();
        score_components.insert("score".to_string(), score.unwrap_or(0.0));
        if let Some(f1) = score {
            // Keep `f1` for back-compat; it now refers to the weighted F1.
            score_components.insert("f1".to_string(), f1);
            score_components.insert("f1_weighted".to_string(), f1);
        }
        score_components.insert("overlap".to_string(), overlap as f64);
        score_components.insert("overlap_weighted".to_string(), overlap_weighted as f64);
        score_components.insert("overlap_weighted_sum".to_string(), overlap_weighted_sum);
        score_components.insert("weight_focus_sum".to_string(), focus_weight_sum);
        score_components.insert("weight_candidate_sum".to_string(), candidate_weight_sum);
        score_components.insert("n_focus".to_string(), n_focus as f64);
        score_components.insert("n_candidate".to_string(), n_candidate as f64);
        score_components.insert("zero_lag_f1".to_string(), zero_lag.score.unwrap_or(0.0));
        if let Some(best) = best_lag.as_ref() {
            score_components.insert("best_lag_f1".to_string(), best.score.unwrap_or(0.0));
        }

        candidates.push(EventMatchCandidateV1 {
            sensor_id: sensor_id.clone(),
            rank: 0,
            score,
            overlap,
            n_focus,
            n_candidate,
            n_focus_up,
            n_focus_down,
            n_candidate_up: Some(n_candidate_up),
            n_candidate_down: Some(n_candidate_down),
            zero_lag,
            best_lag,
            top_lags,
            direction_label: Some(direction_label),
            sign_agreement,
            delta_corr,
            direction_n: Some(direction_n),
            time_of_day_entropy_norm: entropy.map(|x| x.h_norm),
            time_of_day_entropy_weight: entropy.map(|x| x.weight),
            episodes,
            why_ranked: Some(TsseWhyRankedV1 {
                episode_count,
                best_window_sec,
                best_lag_sec: Some(best_lag_sec),
                best_lag_r_ci_low: None,
                best_lag_r_ci_high: None,
                coverage_pct,
                score_components,
                penalties: Vec::new(),
                bonuses: Vec::new(),
            }),
        });

        progress.completed = (idx + 1) as u64;
        if idx % 5 == 0 || idx + 1 == candidate_sensor_ids.len() {
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }

    candidates.sort_by(|a, b| {
        let score_a = a.score.unwrap_or(0.0);
        let score_b = b.score.unwrap_or(0.0);
        score_b
            .total_cmp(&score_a)
            .then_with(|| b.overlap.cmp(&a.overlap))
            .then_with(|| a.sensor_id.cmp(&b.sensor_id))
    });

    for (rank, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = (rank + 1) as u32;
    }

    let match_ms = match_started.elapsed().as_millis() as u64;
    tracing::info!(
        phase = "match_candidates",
        duration_ms = match_ms,
        candidates_scored = candidates.len(),
        "analysis matching complete"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "match_candidates",
            "duration_ms": match_ms,
            "candidates_scored": candidates.len(),
        }),
    )
    .await;

    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("duckdb_load_ms".to_string(), load_ms);
    timings_ms.insert("detect_events_ms".to_string(), detect_ms);
    timings_ms.insert("scoring_ms".to_string(), match_ms);
    timings_ms.insert(
        "job_total_ms".to_string(),
        job_started.elapsed().as_millis() as u64,
    );

    let result = EventMatchResultV1 {
        job_type: "event_match_v1".to_string(),
        focus_sensor_id: focus_sensor_id.clone(),
        computed_through_ts: replication.computed_through_ts.clone(),
        interval_seconds: Some(interval_seconds),
        bucket_count: Some(bucket_count),
        params,
        candidates,
        truncated_sensor_ids,
        gap_skipped_deltas,
        monitoring: Some(monitoring),
        timings_ms,
        versions: BTreeMap::from([("event_match".to_string(), "v1".to_string())]),
    };

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

    serde_json::to_value(&result)
        .context("failed to serialize event_match_v1 result")
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "result_encode_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })
}

fn normalize_explicit_focus_events(
    raw: &[ExplicitFocusEventV1],
    start: DateTime<Utc>,
    end_inclusive: DateTime<Utc>,
    max_events: usize,
) -> Vec<EventPoint> {
    if raw.is_empty() {
        return Vec::new();
    }

    let mut points: Vec<EventPoint> = Vec::new();
    for evt in raw {
        let ts = DateTime::parse_from_rfc3339(evt.ts.trim())
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
        let Some(ts) = ts else {
            continue;
        };
        if ts < start || ts > end_inclusive {
            continue;
        }

        let severity_raw = evt.severity.unwrap_or(1.0);
        let mut z = if severity_raw.is_finite() {
            severity_raw.abs()
        } else {
            1.0
        };
        if !z.is_finite() || z <= 0.0 {
            z = 1.0;
        }

        points.push(EventPoint {
            ts,
            ts_epoch: ts.timestamp(),
            z,
            direction: EventDirectionV1::Up,
            delta: 0.0,
            is_boundary: false,
        });
    }

    // Dedup identical timestamps by keeping the highest severity event for each second.
    points.sort_by(|a, b| {
        a.ts_epoch
            .cmp(&b.ts_epoch)
            .then_with(|| b.z.abs().total_cmp(&a.z.abs()))
    });
    points.dedup_by(|a, b| a.ts_epoch == b.ts_epoch);

    // Bound work: if the caller supplies many focus events, keep the most severe ones
    // while returning them in time order for matching.
    if max_events > 0 && points.len() > max_events {
        points.sort_by(|a, b| b.z.abs().total_cmp(&a.z.abs()).then_with(|| a.ts_epoch.cmp(&b.ts_epoch)));
        points.truncate(max_events);
        points.sort_by_key(|p| p.ts_epoch);
    }

    points
}

fn percentile_sorted(values: &[f64], pct: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    if !pct.is_finite() {
        return None;
    }
    let p = pct.clamp(0.0, 1.0);
    if values.len() == 1 {
        return Some(values[0]);
    }
    let pos = p * ((values.len() - 1) as f64);
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    let lo_v = values.get(lo).copied()?;
    let hi_v = values.get(hi).copied()?;
    let frac = pos - (lo as f64);
    let blended = lo_v + (hi_v - lo_v) * frac;
    Some(blended)
}

fn compute_evidence_monitoring(
    z_cap: f64,
    focus: &DetectedEvents,
    candidates: &HashMap<String, DetectedEvents>,
) -> EventEvidenceMonitoringV1 {
    let mut peak_abs_z: Vec<f64> = Vec::with_capacity(candidates.len() + 1);
    let mut delta_points_total: u64 = 0;
    let mut gap_skipped_deltas_total: u64 = 0;
    let mut events_total: u64 = 0;
    let mut z_clipped_events: u64 = 0;

    let mut record = |detected: &DetectedEvents| {
        delta_points_total = delta_points_total.saturating_add(detected.points_total);
        gap_skipped_deltas_total = gap_skipped_deltas_total.saturating_add(detected.gap_skipped_deltas);
        events_total = events_total.saturating_add(detected.events.len() as u64);
        for evt in &detected.events {
            if evt.z.is_finite() && evt.z.abs() > z_cap {
                z_clipped_events = z_clipped_events.saturating_add(1);
            }
        }
        if let Some(peak) = detected.peak_abs_z {
            if peak.is_finite() {
                peak_abs_z.push(peak.max(0.0));
            }
        }
    };

    record(focus);
    for detected in candidates.values() {
        record(detected);
    }

    peak_abs_z.sort_by(|a, b| a.total_cmp(b));
    let denom_events = events_total.max(1) as f64;
    let denom_gap = (delta_points_total + gap_skipped_deltas_total).max(1) as f64;

    EventEvidenceMonitoringV1 {
        peak_abs_dz_p50: percentile_sorted(&peak_abs_z, 0.50),
        peak_abs_dz_p90: percentile_sorted(&peak_abs_z, 0.90),
        peak_abs_dz_p95: percentile_sorted(&peak_abs_z, 0.95),
        peak_abs_dz_p99: percentile_sorted(&peak_abs_z, 0.99),
        z_cap,
        events_total,
        z_clipped_events,
        z_clipped_pct: (z_clipped_events as f64 / denom_events).clamp(0.0, 1.0),
        delta_points_total,
        gap_skipped_deltas_total,
        gap_skipped_pct: (gap_skipped_deltas_total as f64 / denom_gap).clamp(0.0, 1.0),
    }
}

async fn fetch_candidate_sensors(
    db: &PgPool,
    focus: &SensorMetaRow,
    filters: &TsseCandidateFiltersV1,
    limit: usize,
) -> Result<Vec<String>, JobFailure> {
    let rows: Vec<SensorMetaRow> = sqlx::query_as(
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
          AND sensor_id <> $1
        ORDER BY sensor_id
        "#,
    )
    .bind(&focus.sensor_id)
    .fetch_all(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "sensor_lookup_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })?;

    let mut out: Vec<String> = Vec::new();
    for row in rows {
        if filters.same_node_only && row.node_id != focus.node_id {
            continue;
        }
        if filters.same_unit_only && row.unit != focus.unit {
            continue;
        }
        if filters.same_type_only && row.sensor_type != focus.sensor_type {
            continue;
        }
        if let Some(interval_seconds) = filters.interval_seconds {
            if row.interval_seconds as i64 != interval_seconds {
                continue;
            }
        }
        if let Some(is_derived) = filters.is_derived {
            let derived = row.source.as_deref() == Some("derived");
            if derived != is_derived {
                continue;
            }
        }
        if let Some(is_public_provider) = filters.is_public_provider {
            let provider = row.source.as_deref() == Some("forecast_points");
            if provider != is_public_provider {
                continue;
            }
        }
        if filters
            .exclude_sensor_ids
            .iter()
            .any(|id| id == &row.sensor_id)
        {
            continue;
        }
        out.push(row.sensor_id.clone());
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{
        all_lag_scores_weighted, build_event_episodes, collect_gap_aware_bucket_deltas_with_delta_mode,
        collect_matched_event_pairs, compute_direction_label, delta_corr_from_aligned_deltas,
        normalize_explicit_focus_events, pearson_from_aligned_deltas, rank_lag_score,
        sign_agreement_from_matched_pairs, sum_event_weights, weighted_overlap_sum, DeltaMode,
    };
    use crate::services::analysis::tsse::types::{DirectionLabelV1, EventPolarityV1, ExplicitFocusEventV1};
    use crate::services::analysis::jobs::event_utils::EventPoint;
    use crate::services::analysis::parquet_duckdb::MetricsBucketRow;
    use crate::services::analysis::tsse::types::EventDirectionV1;
    use chrono::{DateTime, Utc};
    use std::collections::HashSet;

    fn evt(epoch: i64, z: f64) -> EventPoint {
        EventPoint {
            ts: DateTime::<Utc>::from_timestamp(epoch, 0).expect("ts"),
            ts_epoch: epoch,
            z,
            direction: if z >= 0.0 {
                EventDirectionV1::Up
            } else {
                EventDirectionV1::Down
            },
            delta: 1.0,
            is_boundary: false,
        }
    }

    #[test]
    fn episodes_cap_z_magnitude_for_peak_and_mean_metrics() {
        let focus_events = vec![evt(0, 100.0), evt(60, 5.0), evt(120, -20.0)];
        let matched: Vec<&EventPoint> = focus_events.iter().collect();

        let episodes = build_event_episodes(
            &matched,
            0,
            60,
            2,
            10,
            15.0,
            focus_events.len() as u64,
        );

        assert_eq!(episodes.len(), 1);
        let ep = &episodes[0];
        assert!((ep.score_peak - 15.0).abs() < 1e-6, "peak should be capped");

        let expected_mean = (15.0 + 5.0 + 15.0) / 3.0;
        assert!(
            (ep.score_mean - expected_mean).abs() < 1e-6,
            "mean should be computed from capped magnitudes"
        );
    }

    #[test]
    fn explicit_focus_events_are_sorted_deduped_and_sanitized() {
        let start = DateTime::<Utc>::from_timestamp(0, 0).expect("start");
        let end = DateTime::<Utc>::from_timestamp(300, 0).expect("end");

        let raw = vec![
            ExplicitFocusEventV1 {
                ts: "invalid".to_string(),
                severity: Some(5.0),
            },
            ExplicitFocusEventV1 {
                ts: "1970-01-01T00:00:10Z".to_string(),
                severity: Some(-2.0),
            },
            ExplicitFocusEventV1 {
                ts: "1970-01-01T00:00:10Z".to_string(),
                severity: Some(7.0),
            },
            ExplicitFocusEventV1 {
                ts: "1970-01-01T00:10:00Z".to_string(),
                severity: Some(1.0),
            },
            ExplicitFocusEventV1 {
                ts: "1970-01-01T00:00:20Z".to_string(),
                severity: None,
            },
        ];

        let events = normalize_explicit_focus_events(&raw, start, end, 100);
        let epochs: Vec<i64> = events.iter().map(|e| e.ts_epoch).collect();
        assert_eq!(epochs, vec![10, 20]);

        let z_at_10 = events.iter().find(|e| e.ts_epoch == 10).expect("10").z;
        assert!((z_at_10 - 7.0).abs() < 1e-6);

        let z_at_20 = events.iter().find(|e| e.ts_epoch == 20).expect("20").z;
        assert!((z_at_20 - 1.0).abs() < 1e-6, "default severity should be 1.0");

        let epochs2: Vec<i64> =
            normalize_explicit_focus_events(&raw, start, end, 100)
                .iter()
                .map(|e| e.ts_epoch)
                .collect();
        assert_eq!(epochs, epochs2);
    }

    #[test]
    fn tolerant_overlap_zero_matches_exact_baseline() {
        let focus_times: Vec<i64> = vec![0, 60, 120, 180, 240, 300];
        let candidate_times: Vec<i64> = vec![0, 120, 300, 360];
        let lag_sec = 60;

        let baseline_set: HashSet<i64> = candidate_times.iter().copied().collect();
        let expected: u64 = focus_times
            .iter()
            .filter(|ts| baseline_set.contains(&(*ts + lag_sec)))
            .count() as u64;

        let focus_events: Vec<EventPoint> = focus_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);

        let candidate_events: Vec<EventPoint> =
            candidate_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let (actual, _overlap_sum) =
            weighted_overlap_sum(&focus_sorted, &candidate_sorted, lag_sec, 0, 10.0);
        assert_eq!(actual, expected);
    }

    #[test]
    fn tolerant_overlap_matches_jittered_events_when_enabled() {
        let focus_times: Vec<i64> = vec![0, 10, 20, 30];
        let candidate_times: Vec<i64> = vec![1, 9, 21, 29];

        let focus_events: Vec<EventPoint> = focus_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);

        let candidate_events: Vec<EventPoint> =
            candidate_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        assert_eq!(
            weighted_overlap_sum(&focus_sorted, &candidate_sorted, 0, 0, 10.0).0,
            0
        );
        assert_eq!(
            weighted_overlap_sum(&focus_sorted, &candidate_sorted, 0, 10, 10.0).0,
            4
        );
    }

    #[test]
    fn best_lag_sign_positive_means_candidate_later() {
        let interval_seconds = 60;
        let tolerance_sec = 0;
        let max_lag_buckets = 2;
        let z_cap = 10.0;

        // Candidate later by +60s.
        let focus_times: Vec<i64> = vec![120, 240, 360];
        let candidate_later: Vec<i64> = vec![180, 300, 420];

        let focus_events: Vec<EventPoint> = focus_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);

        let candidate_events: Vec<EventPoint> =
            candidate_later.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let focus_weight_sum = sum_event_weights(&focus_sorted, z_cap);
        let candidate_weight_sum = sum_event_weights(&candidate_sorted, z_cap);
        let mut scores = all_lag_scores_weighted(
            &focus_sorted,
            &candidate_sorted,
            max_lag_buckets,
            interval_seconds,
            tolerance_sec,
            focus_weight_sum,
            candidate_weight_sum,
            candidate_sorted.len() as u64,
            z_cap,
        );
        scores.sort_by(rank_lag_score);
        let best_later = scores.into_iter().next().expect("best lag");
        assert_eq!(best_later.lag_sec, 60);

        // Candidate earlier by -60s.
        let candidate_earlier: Vec<i64> = vec![60, 180, 300];

        let candidate_events: Vec<EventPoint> =
            candidate_earlier.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let candidate_weight_sum = sum_event_weights(&candidate_sorted, z_cap);
        let mut scores = all_lag_scores_weighted(
            &focus_sorted,
            &candidate_sorted,
            max_lag_buckets,
            interval_seconds,
            tolerance_sec,
            focus_weight_sum,
            candidate_weight_sum,
            candidate_sorted.len() as u64,
            z_cap,
        );
        scores.sort_by(rank_lag_score);
        let best_earlier = scores.into_iter().next().expect("best lag");
        assert_eq!(best_earlier.lag_sec, -60);
    }

    #[test]
    fn top_lags_are_sorted_by_score_then_abs_lag() {
        let interval_seconds = 60;
        let tolerance_sec = 0;
        let max_lag_buckets = 1;
        let z_cap = 10.0;

        let focus_times: Vec<i64> = vec![0, 60, 120, 180];
        let candidate_times: Vec<i64> = vec![0, 60, 120, 240];

        let focus_events: Vec<EventPoint> = focus_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);

        let candidate_events: Vec<EventPoint> =
            candidate_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let focus_weight_sum = sum_event_weights(&focus_sorted, z_cap);
        let candidate_weight_sum = sum_event_weights(&candidate_sorted, z_cap);
        let mut scores = all_lag_scores_weighted(
            &focus_sorted,
            &candidate_sorted,
            max_lag_buckets,
            interval_seconds,
            tolerance_sec,
            focus_weight_sum,
            candidate_weight_sum,
            candidate_sorted.len() as u64,
            z_cap,
        );
        scores.sort_by(rank_lag_score);

        let top: Vec<i64> = scores.iter().take(3).map(|score| score.lag_sec).collect();
        assert_eq!(top, vec![0, -60, 60]);

        let best = scores.into_iter().next().expect("best lag");
        assert_eq!(best.lag_sec, 0);
    }

    #[test]
    fn tolerant_overlap_enforces_one_to_one_matching() {
        let focus_times: Vec<i64> = vec![0, 20];
        let candidate_times: Vec<i64> = vec![10];
        let tol_sec = 10;

        let focus_events: Vec<EventPoint> = focus_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);

        let candidate_events: Vec<EventPoint> =
            candidate_times.iter().map(|&ts| evt(ts, 1.0)).collect();
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let one_to_one = weighted_overlap_sum(&focus_sorted, &candidate_sorted, 0, tol_sec, 10.0).0;

        let naive: u64 = focus_times
            .iter()
            .filter(|focus| {
                candidate_times
                    .iter()
                    .any(|cand| (**focus - *cand).abs() <= tol_sec)
            })
            .count() as u64;

        assert_eq!(naive, 2, "sanity: candidate is within tolerance of both focus events");
        assert_eq!(one_to_one, 1);
    }

    #[test]
    fn episodes_use_same_tolerance_semantics_as_overlap() {
        let focus_events = vec![evt(0, 1.0), evt(60, 2.0), evt(120, 3.0)];
        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);
        let focus_times: Vec<i64> = focus_sorted.iter().map(|e| e.ts_epoch).collect();

        let candidate_events = vec![evt(1, 1.0), evt(61, 2.0), evt(121, 3.0)];
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);
        let tol_sec = 60;

        let overlap = weighted_overlap_sum(&focus_sorted, &candidate_sorted, 0, tol_sec, 10.0).0;
        let matched_pairs = collect_matched_event_pairs(&focus_sorted, &candidate_sorted, 0, tol_sec);
        let matched_focus: Vec<&EventPoint> = matched_pairs.iter().map(|(focus, _)| *focus).collect();
        assert_eq!(matched_focus.len() as u64, overlap);

        let episodes = build_event_episodes(
            &matched_focus,
            0,
            60,
            2,
            10,
            15.0,
            focus_times.len() as u64,
        );
        let points_total: u64 = episodes.iter().map(|ep| ep.num_points).sum();
        assert_eq!(points_total, overlap);
    }

    fn row_at(sensor_id: &str, epoch: i64, value: f64) -> MetricsBucketRow {
        MetricsBucketRow {
            sensor_id: sensor_id.to_string(),
            bucket: DateTime::<Utc>::from_timestamp(epoch, 0).expect("ts"),
            value,
            samples: 1,
        }
    }

    fn stepped_series(sensor_id: &str, interval_seconds: i64, buckets: usize, spike_every: usize, spike_delta: f64) -> Vec<MetricsBucketRow> {
        let mut rows: Vec<MetricsBucketRow> = Vec::with_capacity(buckets);
        let mut value = 0.0;
        for idx in 0..buckets {
            if idx > 0 && spike_every > 0 && idx % spike_every == 0 {
                value += spike_delta;
            }
            rows.push(row_at(sensor_id, (idx as i64) * interval_seconds, value));
        }
        rows
    }

    #[test]
    fn directionality_same_when_deltas_and_events_agree() {
        let interval_seconds = 60;
        let focus_rows = stepped_series("focus", interval_seconds, 25, 4, 100.0);
        let candidate_rows = stepped_series("cand", interval_seconds, 25, 4, 100.0);

        let focus_events = crate::services::analysis::jobs::event_utils::detect_change_events(
            &focus_rows,
            interval_seconds,
            0.8,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        )
        .events;
        let candidate_events = crate::services::analysis::jobs::event_utils::detect_change_events(
            &candidate_rows,
            interval_seconds,
            0.8,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        )
        .events;

        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let pairs = collect_matched_event_pairs(&focus_sorted, &candidate_sorted, 0, 0);
        assert!(pairs.len() >= 5, "need enough matched events for delta_corr gating");
        let n = pairs.len() as u64;

        let sign_agreement = sign_agreement_from_matched_pairs(&pairs);
        assert_eq!(sign_agreement, Some(1.0));

        let focus_deltas = collect_gap_aware_bucket_deltas_with_delta_mode(
            &focus_rows,
            interval_seconds,
            0,
            DeltaMode::Linear,
        );
        let candidate_deltas = collect_gap_aware_bucket_deltas_with_delta_mode(
            &candidate_rows,
            interval_seconds,
            0,
            DeltaMode::Linear,
        );
        let delta_corr = pearson_from_aligned_deltas(&focus_deltas, &candidate_deltas, 0);
        assert!(delta_corr.unwrap_or(0.0) > 0.0);

        let label = compute_direction_label(n, delta_corr, sign_agreement);
        assert_eq!(label, DirectionLabelV1::Same);
    }

    #[test]
    fn directionality_opposite_when_deltas_anti_correlate() {
        let interval_seconds = 60;
        let focus_rows = stepped_series("focus", interval_seconds, 25, 4, 100.0);
        let candidate_rows = stepped_series("cand", interval_seconds, 25, 4, -100.0);

        let focus_events = crate::services::analysis::jobs::event_utils::detect_change_events(
            &focus_rows,
            interval_seconds,
            0.8,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        )
        .events;
        let candidate_events = crate::services::analysis::jobs::event_utils::detect_change_events(
            &candidate_rows,
            interval_seconds,
            0.8,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        )
        .events;

        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let pairs = collect_matched_event_pairs(&focus_sorted, &candidate_sorted, 0, 0);
        assert!(pairs.len() >= 5, "need enough matched events for delta_corr gating");
        let n = pairs.len() as u64;

        let sign_agreement = sign_agreement_from_matched_pairs(&pairs);
        assert_eq!(sign_agreement, Some(0.0));

        let focus_deltas = collect_gap_aware_bucket_deltas_with_delta_mode(
            &focus_rows,
            interval_seconds,
            0,
            DeltaMode::Linear,
        );
        let candidate_deltas = collect_gap_aware_bucket_deltas_with_delta_mode(
            &candidate_rows,
            interval_seconds,
            0,
            DeltaMode::Linear,
        );
        let delta_corr = pearson_from_aligned_deltas(&focus_deltas, &candidate_deltas, 0);
        assert!(delta_corr.unwrap_or(0.0) < 0.0);

        let label = compute_direction_label(n, delta_corr, sign_agreement);
        assert_eq!(label, DirectionLabelV1::Opposite);
    }

    #[test]
    fn directionality_unknown_when_too_few_matched_pairs() {
        let interval_seconds = 60;
        let focus_rows = stepped_series("focus", interval_seconds, 10, 4, 100.0);
        let candidate_rows = stepped_series("cand", interval_seconds, 10, 4, 100.0);

        let focus_events = crate::services::analysis::jobs::event_utils::detect_change_events(
            &focus_rows,
            interval_seconds,
            0.8,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        )
        .events;
        let candidate_events = crate::services::analysis::jobs::event_utils::detect_change_events(
            &candidate_rows,
            interval_seconds,
            0.8,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        )
        .events;

        let mut focus_sorted: Vec<&EventPoint> = focus_events.iter().collect();
        focus_sorted.sort_by_key(|e| e.ts_epoch);
        let mut candidate_sorted: Vec<&EventPoint> = candidate_events.iter().collect();
        candidate_sorted.sort_by_key(|e| e.ts_epoch);

        let pairs = collect_matched_event_pairs(&focus_sorted, &candidate_sorted, 0, 0);
        assert!(pairs.len() < 3, "sanity: sparse case should have <3 matches");
        let n = pairs.len() as u64;

        let sign_agreement = sign_agreement_from_matched_pairs(&pairs);
        let label = compute_direction_label(n, None, sign_agreement);
        assert_eq!(label, DirectionLabelV1::Unknown);
    }

    #[test]
    fn delta_corr_is_omitted_when_too_sparse() {
        let focus: Vec<(i64, f64)> = (0..9).map(|i| (i, i as f64)).collect();
        let cand: Vec<(i64, f64)> = (0..9).map(|i| (i, i as f64)).collect();
        assert_eq!(delta_corr_from_aligned_deltas(&focus, &cand, 0, 10), None);

        let focus: Vec<(i64, f64)> = (0..10).map(|i| (i, i as f64)).collect();
        let cand: Vec<(i64, f64)> = (0..10).map(|i| (i, i as f64)).collect();
        let corr = delta_corr_from_aligned_deltas(&focus, &cand, 0, 10).unwrap_or(0.0);
        assert!(corr > 0.95);
    }
}

fn collect_matched_event_pairs<'a, 'b>(
    focus_events_sorted: &[&'a EventPoint],
    candidate_events_sorted: &[&'b EventPoint],
    lag_sec: i64,
    tolerance_sec: i64,
) -> Vec<(&'a EventPoint, &'b EventPoint)> {
    if focus_events_sorted.is_empty() || candidate_events_sorted.is_empty() {
        return Vec::new();
    }
    let tol = tolerance_sec.max(0);
    let mut matched: Vec<(&'a EventPoint, &'b EventPoint)> = Vec::new();
    let mut focus_idx: usize = 0;
    let mut candidate_idx: usize = 0;
    while focus_idx < focus_events_sorted.len() && candidate_idx < candidate_events_sorted.len() {
        let target = focus_events_sorted[focus_idx].ts_epoch + lag_sec;
        let candidate_ts = candidate_events_sorted[candidate_idx].ts_epoch;
        if candidate_ts < target - tol {
            candidate_idx += 1;
        } else if candidate_ts > target + tol {
            focus_idx += 1;
        } else {
            matched.push((
                focus_events_sorted[focus_idx],
                candidate_events_sorted[candidate_idx],
            ));
            focus_idx += 1;
            candidate_idx += 1;
        }
    }
    matched
}

fn sign_agreement_from_matched_pairs(matched_pairs: &[(&EventPoint, &EventPoint)]) -> Option<f64> {
    if matched_pairs.is_empty() {
        return None;
    }
    let mut same: u64 = 0;
    for (focus, candidate) in matched_pairs {
        if focus.direction == candidate.direction {
            same = same.saturating_add(1);
        }
    }
    Some((same as f64) / (matched_pairs.len() as f64))
}

fn collect_gap_aware_bucket_deltas_with_delta_mode(
    rows: &[crate::services::analysis::parquet_duckdb::MetricsBucketRow],
    interval_seconds: i64,
    gap_max_buckets: i64,
    delta_mode: DeltaMode,
) -> Vec<(i64, f64)> {
    if rows.len() < 2 {
        return Vec::new();
    }
    let interval_seconds = interval_seconds.max(1);
    let gap_max_buckets = gap_max_buckets.max(0);
    let gap_threshold_seconds = if gap_max_buckets > 0 {
        gap_max_buckets.saturating_mul(interval_seconds)
    } else {
        i64::MAX
    };

    let mut deltas: Vec<(i64, f64)> = Vec::new();
    for window in rows.windows(2) {
        let prev = &window[0];
        let curr = &window[1];
        if !prev.value.is_finite() || !curr.value.is_finite() {
            continue;
        }
        let dt_seconds = (curr.bucket - prev.bucket).num_seconds();
        if dt_seconds > gap_threshold_seconds {
            continue;
        }
        let delta = signal_semantics::delta(prev.value, curr.value, delta_mode);
        if !delta.is_finite() {
            continue;
        }
        deltas.push((curr.bucket.timestamp(), delta));
    }
    deltas
}

#[cfg(test)]
fn pearson_from_aligned_deltas(
    focus_deltas: &[(i64, f64)],
    candidate_deltas: &[(i64, f64)],
    lag_sec: i64,
) -> Option<f64> {
    pearson_from_aligned_deltas_with_n(focus_deltas, candidate_deltas, lag_sec).0
}

fn pearson_from_aligned_deltas_with_n(
    focus_deltas: &[(i64, f64)],
    candidate_deltas: &[(i64, f64)],
    lag_sec: i64,
) -> (Option<f64>, usize) {
    if focus_deltas.is_empty() || candidate_deltas.is_empty() {
        return (None, 0);
    }
    let mut x: Vec<f64> = Vec::new();
    let mut y: Vec<f64> = Vec::new();

    let mut focus_idx: usize = 0;
    let mut cand_idx: usize = 0;
    while focus_idx < focus_deltas.len() && cand_idx < candidate_deltas.len() {
        let (ts_focus, delta_focus) = focus_deltas[focus_idx];
        let target = ts_focus + lag_sec;
        let (ts_cand, delta_cand) = candidate_deltas[cand_idx];
        if ts_cand < target {
            cand_idx += 1;
        } else if ts_cand > target {
            focus_idx += 1;
        } else {
            x.push(delta_focus);
            y.push(delta_cand);
            focus_idx += 1;
            cand_idx += 1;
        }
    }

    let n = x.len();
    (pearson(&x, &y), n)
}

fn delta_corr_from_aligned_deltas(
    focus_deltas: &[(i64, f64)],
    candidate_deltas: &[(i64, f64)],
    lag_sec: i64,
    min_pairs: usize,
) -> Option<f64> {
    let (raw, n) = pearson_from_aligned_deltas_with_n(focus_deltas, candidate_deltas, lag_sec);
    if n >= min_pairs {
        raw
    } else {
        None
    }
}

fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 3 {
        return None;
    }
    let n = x.len() as f64;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_yy = 0.0;
    let mut sum_xy = 0.0;
    for (xv, yv) in x.iter().zip(y.iter()) {
        sum_x += *xv;
        sum_y += *yv;
        sum_xx += xv * xv;
        sum_yy += yv * yv;
        sum_xy += xv * yv;
    }
    let denom_x = n * sum_xx - sum_x * sum_x;
    let denom_y = n * sum_yy - sum_y * sum_y;
    if denom_x <= 0.0 || denom_y <= 0.0 {
        return None;
    }
    let r = (n * sum_xy - sum_x * sum_y) / (denom_x * denom_y).sqrt();
    if !r.is_finite() {
        return None;
    }
    Some(r.max(-1.0).min(1.0))
}

fn compute_direction_label(
    matched_pairs: u64,
    delta_corr: Option<f64>,
    sign_agreement: Option<f64>,
) -> DirectionLabelV1 {
    if matched_pairs < 3 {
        return DirectionLabelV1::Unknown;
    }
    if let Some(delta_corr) = delta_corr {
        return if delta_corr >= 0.0 {
            DirectionLabelV1::Same
        } else {
            DirectionLabelV1::Opposite
        };
    }
    if let Some(sign_agreement) = sign_agreement {
        if sign_agreement >= 0.5 {
            DirectionLabelV1::Same
        } else {
            DirectionLabelV1::Opposite
        }
    } else {
        DirectionLabelV1::Unknown
    }
}

fn event_weight(evt: &EventPoint, z_cap: f64) -> f64 {
    if !evt.z.is_finite() {
        return 0.0;
    }
    let abs = evt.z.abs();
    if z_cap.is_finite() && z_cap > 0.0 {
        abs.min(z_cap)
    } else {
        abs
    }
}

fn sum_event_weights(events_sorted: &[&EventPoint], z_cap: f64) -> f64 {
    events_sorted
        .iter()
        .map(|evt| event_weight(evt, z_cap))
        .filter(|v| v.is_finite() && *v > 0.0)
        .sum()
}

fn weighted_overlap_sum(
    focus_events_sorted: &[&EventPoint],
    candidate_events_sorted: &[&EventPoint],
    lag_sec: i64,
    tolerance_sec: i64,
    z_cap: f64,
) -> (u64, f64) {
    if focus_events_sorted.is_empty() || candidate_events_sorted.is_empty() {
        return (0, 0.0);
    }
    let tol = tolerance_sec.max(0);
    let mut overlap: u64 = 0;
    let mut overlap_sum: f64 = 0.0;
    let mut focus_idx: usize = 0;
    let mut candidate_idx: usize = 0;
    while focus_idx < focus_events_sorted.len() && candidate_idx < candidate_events_sorted.len() {
        let focus = focus_events_sorted[focus_idx];
        let candidate = candidate_events_sorted[candidate_idx];
        let target = focus.ts_epoch + lag_sec;
        let candidate_ts = candidate.ts_epoch;
        if candidate_ts < target - tol {
            candidate_idx += 1;
        } else if candidate_ts > target + tol {
            focus_idx += 1;
        } else {
            overlap = overlap.saturating_add(1);
            let wf = event_weight(focus, z_cap);
            let wc = event_weight(candidate, z_cap);
            if wf.is_finite() && wc.is_finite() && wf > 0.0 && wc > 0.0 {
                overlap_sum += wf.min(wc);
            }
            focus_idx += 1;
            candidate_idx += 1;
        }
    }
    (overlap, overlap_sum)
}

fn weighted_f1_score(
    overlap_sum: f64,
    focus_sum: f64,
    candidate_sum: f64,
) -> Option<f64> {
    if (!focus_sum.is_finite() || focus_sum <= 0.0) && (!candidate_sum.is_finite() || candidate_sum <= 0.0) {
        return None;
    }
    if !focus_sum.is_finite() || !candidate_sum.is_finite() || focus_sum <= 0.0 || candidate_sum <= 0.0 {
        return Some(0.0);
    }
    if !overlap_sum.is_finite() || overlap_sum <= 0.0 {
        return Some(0.0);
    }
    let denom = focus_sum + candidate_sum;
    if !denom.is_finite() || denom <= 0.0 {
        return Some(0.0);
    }
    Some(((2.0 * overlap_sum) / denom).clamp(0.0, 1.0))
}

fn score_lag_weighted(
    focus_events_sorted: &[&EventPoint],
    candidate_events_sorted: &[&EventPoint],
    lag_buckets: i64,
    interval_seconds: i64,
    tolerance_sec: i64,
    focus_weight_sum: f64,
    candidate_weight_sum: f64,
    n_candidate: u64,
    z_cap: f64,
) -> EventMatchLagScoreV1 {
    let lag_sec = lag_buckets.saturating_mul(interval_seconds);
    let (overlap, overlap_sum) = weighted_overlap_sum(
        focus_events_sorted,
        candidate_events_sorted,
        lag_sec,
        tolerance_sec,
        z_cap,
    );
    let score = weighted_f1_score(overlap_sum, focus_weight_sum, candidate_weight_sum);
    EventMatchLagScoreV1 {
        lag_sec,
        score,
        overlap,
        n_candidate,
    }
}

fn rank_lag_score(a: &EventMatchLagScoreV1, b: &EventMatchLagScoreV1) -> Ordering {
    let score_a = a.score.unwrap_or(-1.0);
    let score_b = b.score.unwrap_or(-1.0);
    score_b
        .total_cmp(&score_a)
        .then_with(|| b.overlap.cmp(&a.overlap))
        .then_with(|| a.lag_sec.abs().cmp(&b.lag_sec.abs()))
        .then_with(|| a.lag_sec.cmp(&b.lag_sec))
}

fn all_lag_scores_weighted(
    focus_events_sorted: &[&EventPoint],
    candidate_events_sorted: &[&EventPoint],
    max_lag_buckets: i64,
    interval_seconds: i64,
    tolerance_sec: i64,
    focus_weight_sum: f64,
    candidate_weight_sum: f64,
    n_candidate: u64,
    z_cap: f64,
) -> Vec<EventMatchLagScoreV1> {
    if max_lag_buckets <= 0 {
        return vec![score_lag_weighted(
            focus_events_sorted,
            candidate_events_sorted,
            0,
            interval_seconds,
            tolerance_sec,
            focus_weight_sum,
            candidate_weight_sum,
            n_candidate,
            z_cap,
        )];
    }

    (-max_lag_buckets..=max_lag_buckets)
        .map(|lag| {
            score_lag_weighted(
                focus_events_sorted,
                candidate_events_sorted,
                lag,
                interval_seconds,
                tolerance_sec,
                focus_weight_sum,
                candidate_weight_sum,
                n_candidate,
                z_cap,
            )
        })
        .collect()
}

fn build_event_episodes(
    matched_focus_events: &[&EventPoint],
    lag_sec: i64,
    interval_seconds: i64,
    gap_buckets: i64,
    max_episodes: usize,
    z_cap: f64,
    focus_total: u64,
) -> Vec<TsseEpisodeV1> {
    if matched_focus_events.is_empty() {
        return Vec::new();
    }

    let mut matched: Vec<&EventPoint> = matched_focus_events.to_vec();
    matched.sort_by_key(|e| e.ts_epoch);
    let gap_seconds = gap_buckets.max(1) * interval_seconds.max(1);
    let mut episodes: Vec<TsseEpisodeV1> = Vec::new();

    let mut current_start = matched[0].ts_epoch;
    let mut current_end = matched[0].ts_epoch;
    let z_cap = z_cap.max(0.0);
    let first_z = if z_cap > 0.0 {
        matched[0].z.abs().min(z_cap)
    } else {
        matched[0].z.abs()
    };
    let mut score_sum = first_z;
    let mut score_peak = first_z;
    let mut count = 1_u64;

    for evt in matched.iter().skip(1) {
        let z_used = if z_cap > 0.0 {
            evt.z.abs().min(z_cap)
        } else {
            evt.z.abs()
        };
        if evt.ts_epoch - current_end > gap_seconds {
            episodes.push(build_episode(
                current_start,
                current_end,
                lag_sec,
                interval_seconds,
                count,
                score_sum,
                score_peak,
                focus_total,
            ));
            current_start = evt.ts_epoch;
            current_end = evt.ts_epoch;
            score_sum = z_used;
            score_peak = z_used;
            count = 1;
        } else {
            current_end = evt.ts_epoch;
            score_sum += z_used;
            score_peak = score_peak.max(z_used);
            count += 1;
        }
    }
    episodes.push(build_episode(
        current_start,
        current_end,
        lag_sec,
        interval_seconds,
        count,
        score_sum,
        score_peak,
        focus_total,
    ));

    episodes.sort_by(|a, b| b.score_peak.total_cmp(&a.score_peak));
    if episodes.len() > max_episodes {
        episodes.truncate(max_episodes);
    }
    episodes
}

fn build_episode(
    start_epoch: i64,
    end_epoch: i64,
    lag_sec: i64,
    interval_seconds: i64,
    count: u64,
    score_sum: f64,
    score_peak: f64,
    focus_total: u64,
) -> TsseEpisodeV1 {
    // Ensure end is at least 1 interval after start for single-event episodes
    // to satisfy API validation that episode_end_ts must be after start_ts
    let effective_end_epoch = if end_epoch <= start_epoch {
        start_epoch + interval_seconds.max(1)
    } else {
        end_epoch
    };
    let start_ts = DateTime::<Utc>::from_timestamp(start_epoch, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
        .to_rfc3339();
    let end_ts = DateTime::<Utc>::from_timestamp(effective_end_epoch, 0)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap())
        .to_rfc3339();
    let window_sec = ((effective_end_epoch - start_epoch).max(0) + interval_seconds.max(1)) as i64;
    let score_mean = if count > 0 {
        score_sum / count as f64
    } else {
        0.0
    };
    let coverage = if focus_total > 0 {
        (count as f64) / (focus_total as f64)
    } else {
        0.0
    };
    TsseEpisodeV1 {
        start_ts,
        end_ts,
        window_sec,
        lag_sec,
        lag_iqr_sec: 0,
        score_mean,
        score_peak,
        coverage,
        num_points: count,
    }
}
