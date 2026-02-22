use super::event_utils::{
    detect_change_events_with_options_and_delta_mode, hour_of_day_mean_residual_rows,
    time_of_day_entropy, EventDetectOptionsV1, EventPoint,
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
    CooccurrenceBucketPreferenceModeV1, CooccurrenceBucketV1, CooccurrenceEventV1,
    CooccurrenceJobParamsV1, CooccurrenceResultV1, CooccurrenceSensorStatsV1, DeseasonModeV1,
    EventDetectorModeV1, EventPolarityV1, EventSuppressionModeV1, EventThresholdModeV1,
};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(sqlx::FromRow)]
struct SensorTypeSourceRow {
    sensor_id: String,
    sensor_type: String,
    source: Option<String>,
}

pub async fn execute(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &crate::services::analysis::lake::AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: CooccurrenceJobParamsV1 =
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

    let mut sensor_ids: Vec<String> = params
        .sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    sensor_ids.sort();
    sensor_ids.dedup();
    if sensor_ids.len() < 2 {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "At least two sensor_ids are required".to_string(),
            details: None,
        }));
    }

    let max_sensors = params.max_sensors.unwrap_or(20).clamp(2, 10_000) as usize;
    let mut truncated_sensor_ids: Vec<String> = Vec::new();
    if sensor_ids.len() > max_sensors {
        truncated_sensor_ids = sensor_ids.split_off(max_sensors);
    }

    let meta_rows: Vec<SensorTypeSourceRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            type as sensor_type,
            NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        WHERE deleted_at IS NULL
          AND sensor_id = ANY($1)
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
            row.sensor_id,
            signal_semantics::auto_delta_mode(Some(&row.sensor_type), row.source.as_deref()),
        );
    }

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
    let gap_max_buckets = params.gap_max_buckets.unwrap_or(5).max(0);
    let min_separation_buckets = params.min_separation_buckets.unwrap_or(2).max(0);
    let tolerance_buckets = params.tolerance_buckets.unwrap_or(2).clamp(0, 60);
    let min_sensors = params
        .min_sensors
        .unwrap_or(2)
        .clamp(2, sensor_ids.len().max(2) as u32) as usize;
    let max_results = params.max_results.unwrap_or(32).clamp(1, 256) as usize;
    let max_events = params.max_events.unwrap_or(2_000).clamp(100, 20_000) as usize;
    let z_cap = params.z_cap.unwrap_or(15.0).clamp(1.0, 1_000.0);
    let deseason_mode = params.deseason_mode.unwrap_or(DeseasonModeV1::None);
    let periodic_penalty_enabled = params.periodic_penalty_enabled.unwrap_or(false);
    let bucket_preference_mode = params
        .bucket_preference_mode
        .unwrap_or(CooccurrenceBucketPreferenceModeV1::PreferSpecificMatches);
    let apply_deseasoning = matches!(deseason_mode, DeseasonModeV1::HourOfDayMean)
        && horizon_seconds >= 2 * 86_400;
    let focus_sensor_id = params
        .focus_sensor_id
        .as_ref()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());

    let replication = read_replication_state(lake).unwrap_or_default();
    let job_started = Instant::now();

    tracing::info!(
        phase = "start",
        sensor_count = sensor_ids.len(),
        interval_seconds,
        "analysis job started"
    );

    let mut progress = AnalysisJobProgress {
        phase: "load_series".to_string(),
        completed: 0,
        total: Some(sensor_ids.len() as u64),
        message: Some("Loading bucketed series".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

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

    let mut events_by_sensor: HashMap<String, Vec<EventPoint>> = HashMap::new();
    let mut gap_skipped_deltas: BTreeMap<String, u64> = BTreeMap::new();
    let mut entropy_weight_by_sensor: HashMap<String, f64> = HashMap::new();
    for (idx, sensor_id) in sensor_ids.iter().enumerate() {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        let raw_rows = grouped.get(sensor_id).cloned().unwrap_or_default();
        let rows = if apply_deseasoning {
            hour_of_day_mean_residual_rows(&raw_rows)
        } else {
            raw_rows
        };
        let delta_mode = delta_mode_by_id
            .get(sensor_id)
            .copied()
            .unwrap_or(DeltaMode::Linear);
        let detected =
            detect_change_events_with_options_and_delta_mode(&rows, &detect_options, delta_mode);
        let entropy = if periodic_penalty_enabled {
            time_of_day_entropy(&detected.events)
        } else {
            None
        };
        if let Some(entropy) = entropy {
            entropy_weight_by_sensor.insert(sensor_id.clone(), entropy.weight);
        }
        gap_skipped_deltas.insert(sensor_id.clone(), detected.gap_skipped_deltas);
        events_by_sensor.insert(sensor_id.clone(), detected.events);
        progress.completed = (idx + 1) as u64;
        if idx % 5 == 0 || idx + 1 == sensor_ids.len() {
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }
    let detect_ms = detect_started.elapsed().as_millis() as u64;
    tracing::info!(
        phase = "detect_events",
        duration_ms = detect_ms,
        sensor_count = sensor_ids.len(),
        "analysis event detection complete"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "detect_events",
            "duration_ms": detect_ms,
            "sensor_count": sensor_ids.len(),
        }),
    )
    .await;

    progress.phase = "score_buckets".to_string();
    progress.completed = 0;
    progress.total = None;
    progress.message = Some("Scoring co-occurrence buckets".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    let score_started = Instant::now();
    let mut buckets_by_index: HashMap<i64, HashMap<String, EventPoint>> = HashMap::new();
    let mut event_count = 0_u64;

    for (sensor_id, events) in events_by_sensor.iter() {
        for evt in events {
            if cancel.is_cancelled() {
                return Err(JobFailure::Canceled);
            }
            let bucket_index = evt.ts_epoch / interval_seconds.max(1);
            for offset in -tolerance_buckets..=tolerance_buckets {
                let idx = bucket_index + offset;
                let entry = buckets_by_index.entry(idx).or_default();
                let replace = entry
                    .get(sensor_id)
                    .map(|prev| evt.z.abs() > prev.z.abs())
                    .unwrap_or(true);
                if replace {
                    entry.insert(sensor_id.clone(), evt.clone());
                }
            }
            event_count += 1;
        }
    }

    let total_sensors = sensor_ids.len().max(1);
    let mut focus_bucket_strength: HashMap<i64, f64> = HashMap::new();
    if let Some(ref focus) = focus_sensor_id {
        if let Some(events) = events_by_sensor.get(focus) {
            let focus_weight = entropy_weight_by_sensor.get(focus).copied().unwrap_or(1.0);
            for evt in events {
                let bucket_idx = evt.ts_epoch / interval_seconds.max(1);
                let strength = evt.z.abs().min(z_cap) * focus_weight;
                let entry = focus_bucket_strength.entry(bucket_idx).or_insert(0.0);
                if strength > *entry {
                    *entry = strength;
                }
            }
        }
    }

    let mut candidates: Vec<(i64, f64, CooccurrenceBucketV1, usize)> = Vec::new();
    if let Some(ref focus) = focus_sensor_id {
        let mut focus_order: Vec<(i64, f64)> = focus_bucket_strength.iter().map(|(k, v)| (*k, *v)).collect();
        focus_order.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| b.0.cmp(&a.0)));

        for (bucket_idx, focus_strength) in focus_order {
            let Some(entry) = buckets_by_index.get(&bucket_idx) else {
                continue;
            };
            if entry.len() < min_sensors {
                continue;
            }
            if !entry.contains_key(focus) {
                continue;
            }
            let Some((bucket, group_size)) = build_bucket(
                bucket_idx,
                entry,
                interval_seconds,
                z_cap,
                total_sensors,
                &entropy_weight_by_sensor,
                bucket_preference_mode,
            ) else {
                continue;
            };
            candidates.push((bucket_idx, focus_strength, bucket, group_size));
        }

        candidates.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then_with(|| b.2.score.total_cmp(&a.2.score))
                .then_with(|| a.3.cmp(&b.3))
                .then_with(|| b.0.cmp(&a.0))
        });
    } else {
        for (bucket_idx, entry) in buckets_by_index.iter() {
            if entry.len() < min_sensors {
                continue;
            }
            let Some((bucket, group_size)) = build_bucket(
                *bucket_idx,
                entry,
                interval_seconds,
                z_cap,
                total_sensors,
                &entropy_weight_by_sensor,
                bucket_preference_mode,
            ) else {
                continue;
            };
            candidates.push((*bucket_idx, 0.0, bucket, group_size));
        }
        candidates.sort_by(|a, b| {
            b.2.score
                .total_cmp(&a.2.score)
                .then_with(|| a.3.cmp(&b.3))
                .then_with(|| b.0.cmp(&a.0))
        });
    }

    let suppression = tolerance_buckets;
    let mut blocked: HashMap<i64, bool> = HashMap::new();
    let mut selected: Vec<CooccurrenceBucketV1> = Vec::new();

    for (bucket_idx, _focus_strength, bucket, _group_size) in candidates {
        if selected.len() >= max_results {
            break;
        }
        if *blocked.get(&bucket_idx).unwrap_or(&false) {
            continue;
        }
        selected.push(bucket);
        let start = bucket_idx.saturating_sub(suppression);
        let end = bucket_idx.saturating_add(suppression);
        for i in start..=end {
            blocked.insert(i, true);
        }
    }

    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("duckdb_load_ms".to_string(), load_ms);
    timings_ms.insert("detect_events_ms".to_string(), detect_ms);
    let score_ms = score_started.elapsed().as_millis() as u64;
    timings_ms.insert("scoring_ms".to_string(), score_ms);
    timings_ms.insert(
        "job_total_ms".to_string(),
        job_started.elapsed().as_millis() as u64,
    );

    let mut counts = BTreeMap::new();
    counts.insert("event_count".to_string(), event_count);
    counts.insert("buckets".to_string(), bucket_count);

    let mut sensor_stats: BTreeMap<String, CooccurrenceSensorStatsV1> = BTreeMap::new();
    for sensor_id in &sensor_ids {
        let events = events_by_sensor.get(sensor_id).map(|v| v.as_slice()).unwrap_or(&[]);
        let n_events = events.len() as u64;
        let mean_abs_z = if n_events > 0 {
            let sum: f64 = events
                .iter()
                .map(|evt| evt.z.abs().min(z_cap))
                .filter(|v| v.is_finite() && *v > 0.0)
                .sum();
            if sum.is_finite() {
                sum / (n_events as f64)
            } else {
                0.0
            }
        } else {
            0.0
        };
        sensor_stats.insert(
            sensor_id.clone(),
            CooccurrenceSensorStatsV1 {
                n_events,
                mean_abs_z,
            },
        );
    }

    tracing::info!(
        phase = "score_buckets",
        duration_ms = score_ms,
        buckets = selected.len(),
        "analysis scoring complete"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "score_buckets",
            "duration_ms": score_ms,
            "buckets": selected.len(),
        }),
    )
    .await;

    let result = CooccurrenceResultV1 {
        job_type: "cooccurrence_v1".to_string(),
        computed_through_ts: replication.computed_through_ts.clone(),
        interval_seconds: Some(interval_seconds),
        bucket_count: Some(bucket_count),
        params,
        buckets: selected,
        truncated_sensor_ids,
        gap_skipped_deltas,
        timings_ms,
        counts,
        sensor_stats,
        versions: BTreeMap::from([("cooccurrence".to_string(), "v1".to_string())]),
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
        .context("failed to serialize cooccurrence_v1 result")
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "result_encode_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })
}

fn compute_bucket_score(
    mode: CooccurrenceBucketPreferenceModeV1,
    severity_sum: f64,
    group_size: usize,
    total_sensors: usize,
) -> Option<(f64, Option<f64>, f64)> {
    if !severity_sum.is_finite() || severity_sum <= 0.0 {
        return None;
    }
    let total_sensors = total_sensors.max(1) as f64;
    let group_size = group_size.max(1) as f64;
    let severity_avg = severity_sum / group_size;

    match mode {
        CooccurrenceBucketPreferenceModeV1::PreferSpecificMatches => {
            let pair_weight = 1.0 / (2.0 + group_size).ln();
            let idf = ((total_sensors + 1.0) / (group_size + 1.0)).ln();
            let score = severity_avg * pair_weight * idf;
            if !pair_weight.is_finite() || !idf.is_finite() || !score.is_finite() {
                return None;
            }
            Some((pair_weight, Some(idf), score))
        }
        CooccurrenceBucketPreferenceModeV1::PreferSystemWideMatches => {
            let score = severity_sum;
            if !score.is_finite() || score <= 0.0 {
                return None;
            }
            Some((1.0, None, score))
        }
    }
}

fn build_bucket(
    bucket_idx: i64,
    entry: &HashMap<String, EventPoint>,
    interval_seconds: i64,
    z_cap: f64,
    total_sensors: usize,
    entropy_weight_by_sensor: &HashMap<String, f64>,
    bucket_preference_mode: CooccurrenceBucketPreferenceModeV1,
) -> Option<(CooccurrenceBucketV1, usize)> {
    let mut sensors: Vec<CooccurrenceEventV1> = Vec::new();
    let mut severity_sum = 0.0;
    for (sensor_id, evt) in entry.iter() {
        sensors.push(CooccurrenceEventV1 {
            sensor_id: sensor_id.clone(),
            ts: evt.ts.timestamp_millis(),
            z: evt.z,
            direction: evt.direction,
            delta: evt.delta,
        });
        let weight = entropy_weight_by_sensor.get(sensor_id).copied().unwrap_or(1.0);
        severity_sum += evt.z.abs().min(z_cap) * weight;
    }
    sensors.sort_by(|a, b| b.z.abs().total_cmp(&a.z.abs()));

    let group_size = sensors.len();
    let Some((pair_weight, idf, score)) =
        compute_bucket_score(bucket_preference_mode, severity_sum, group_size, total_sensors)
    else {
        return None;
    };
    if score <= 0.0 {
        return None;
    }

    let ts_epoch = bucket_idx * interval_seconds.max(1);
    let ts = ts_epoch.saturating_mul(1000);

    Some((
        CooccurrenceBucketV1 {
            ts,
            sensors,
            group_size: group_size as u32,
            severity_sum,
            pair_weight,
            idf,
            score,
        },
        group_size,
    ))
}

#[cfg(test)]
mod tests {
    use super::{build_bucket, compute_bucket_score};
    use crate::services::analysis::tsse::types::{CooccurrenceBucketPreferenceModeV1, EventDirectionV1};
    use chrono::{DateTime, Utc};
    use std::collections::HashMap;

    #[test]
    fn bucket_score_downweights_large_groups_and_zeroes_global_buckets() {
        let severity_sum = 10.0;

        let score_small = compute_bucket_score(
            CooccurrenceBucketPreferenceModeV1::PreferSpecificMatches,
            severity_sum,
            3,
            100,
        )
            .map(|(_, _, s)| s)
            .expect("score");
        let score_large = compute_bucket_score(
            CooccurrenceBucketPreferenceModeV1::PreferSpecificMatches,
            severity_sum,
            20,
            100,
        )
            .map(|(_, _, s)| s)
            .expect("score");
        assert!(
            score_small > score_large,
            "expected smaller group buckets to score higher when severity is comparable"
        );

        let global = compute_bucket_score(
            CooccurrenceBucketPreferenceModeV1::PreferSpecificMatches,
            severity_sum,
            50,
            50,
        )
            .map(|(_, _, s)| s)
            .expect("score");
        assert!(
            global.abs() < 1e-12,
            "expected group_size==N buckets to have near-zero score (idf==0)"
        );
    }

    #[test]
    fn focus_centric_selection_prefers_high_focus_severity_over_high_group_severity() {
        let interval_seconds = 60;
        let z_cap = 15.0;
        let total_sensors = 10;
        let mode = CooccurrenceBucketPreferenceModeV1::PreferSpecificMatches;

        let mk_evt = |bucket_idx: i64, z: f64| super::EventPoint {
            ts: DateTime::<Utc>::from_timestamp(bucket_idx * interval_seconds, 0).expect("ts"),
            ts_epoch: bucket_idx * interval_seconds,
            z,
            direction: if z >= 0.0 {
                EventDirectionV1::Up
            } else {
                EventDirectionV1::Down
            },
            delta: 0.0,
            is_boundary: false,
        };

        // Bucket 10: low focus severity, high group severity.
        let mut entry_10: HashMap<String, super::EventPoint> = HashMap::new();
        entry_10.insert("focus".to_string(), mk_evt(10, 2.0));
        entry_10.insert("a".to_string(), mk_evt(10, 15.0));
        entry_10.insert("b".to_string(), mk_evt(10, 15.0));

        // Bucket 20: high focus severity, lower group severity.
        let mut entry_20: HashMap<String, super::EventPoint> = HashMap::new();
        entry_20.insert("focus".to_string(), mk_evt(20, 10.0));
        entry_20.insert("c".to_string(), mk_evt(20, 3.0));

        let entropy: HashMap<String, f64> = HashMap::new();
        let (bucket_10, _) =
            build_bucket(10, &entry_10, interval_seconds, z_cap, total_sensors, &entropy, mode)
                .expect("bucket 10");
        let (bucket_20, _) =
            build_bucket(20, &entry_20, interval_seconds, z_cap, total_sensors, &entropy, mode)
                .expect("bucket 20");

        // Sanity: global score prefers bucket 10 (bigger group severity).
        assert!(
            bucket_10.score > bucket_20.score,
            "sanity: score-based selection would pick bucket 10"
        );

        // Focus-centric should pick bucket 20 first because focus strength is higher.
        let focus_strength_10: f64 = 2.0;
        let focus_strength_20: f64 = 10.0;
        let mut ordered = vec![
            (10_i64, focus_strength_10, bucket_10),
            (20_i64, focus_strength_20, bucket_20),
        ];
        ordered.sort_by(|a, b| {
            b.1.total_cmp(&a.1)
                .then_with(|| b.2.score.total_cmp(&a.2.score))
        });
        assert_eq!(ordered[0].0, 20);
    }
}
