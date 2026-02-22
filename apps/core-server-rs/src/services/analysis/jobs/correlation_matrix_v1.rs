use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::bucket_reader::{
    read_bucket_series_for_sensors_with_aggregation_and_options, BucketAggregationPreference,
};
use crate::services::analysis::lake::read_replication_state;
use crate::services::analysis::parquet_duckdb::{DuckDbQueryService, MetricsBucketReadOptions};
use crate::services::analysis::stats::correlation::{
    effective_sample_size_lag1, lag1_autocorr, pearson_confidence_interval_fisher_z,
    pearson_p_value_fisher_z, spearman_confidence_interval_fisher_z_approx,
    spearman_p_value_t_approx, z_value_for_alpha as z_value_for_alpha_from_alpha,
};
use crate::services::analysis::stats::fdr::bh_fdr_q_values;
use crate::services::analysis::tsse::types::{
    BucketAggregationModeV1, CorrelationMatrixCellStatusV1, CorrelationMatrixCellV1,
    CorrelationMatrixJobParamsV1, CorrelationMatrixResultV1, CorrelationMatrixSensorV1,
    CorrelationLagModeV1, CorrelationMethodV1, CorrelationValueModeV1,
};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(sqlx::FromRow, Clone)]
struct SensorMetaRow {
    sensor_id: String,
    name: String,
    unit: String,
    node_id: uuid::Uuid,
    sensor_type: String,
}

pub async fn execute(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &crate::services::analysis::lake::AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: CorrelationMatrixJobParamsV1 = serde_json::from_value(job.params.0.clone())
        .map_err(|err| {
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

    let method = params.method.unwrap_or(CorrelationMethodV1::Pearson);
    let max_sensors = params.max_sensors.unwrap_or(20).clamp(2, 100) as usize;
    let max_buckets = params.max_buckets.unwrap_or(10_000).clamp(100, 50_000) as i64;
    let min_overlap = params.min_overlap.unwrap_or(3).clamp(2, 10_000) as usize;
    let min_significant_n = params.min_significant_n.unwrap_or(10).clamp(3, 100_000) as usize;
    let significance_alpha = params
        .significance_alpha
        .unwrap_or(0.05)
        .clamp(0.000_1, 0.5);
    let min_abs_r = params.min_abs_r.unwrap_or(0.2).clamp(0.0, 1.0);
    let bucket_aggregation_mode = params
        .bucket_aggregation_mode
        .unwrap_or(BucketAggregationModeV1::Auto);
    let value_mode = params
        .value_mode
        .unwrap_or(CorrelationValueModeV1::Levels);
    let lag_mode = params.lag_mode.unwrap_or(CorrelationLagModeV1::Aligned);
    let max_lag_buckets = params.max_lag_buckets.unwrap_or(12).clamp(0, 360);

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

    let mut truncated_sensor_ids: Vec<String> = Vec::new();
    if sensor_ids.len() > max_sensors {
        truncated_sensor_ids = sensor_ids.split_off(max_sensors);
    }

    let replication = read_replication_state(lake).unwrap_or_default();
    let job_started = Instant::now();

    let horizon_seconds = (end_inclusive - start).num_seconds().max(1);
    let mut interval_seconds = params.interval_seconds.unwrap_or(60).max(1);
    let expected_buckets = (horizon_seconds as f64 / interval_seconds as f64).ceil() as i64;
    if expected_buckets > max_buckets {
        interval_seconds = ((horizon_seconds as f64) / (max_buckets as f64)).ceil() as i64;
    }
    let bucket_count = (horizon_seconds as f64 / interval_seconds as f64)
        .ceil()
        .max(1.0) as u64;

    let mut params = params;
    params.method = Some(method);
    params.interval_seconds = Some(interval_seconds);
    params.max_sensors = Some(max_sensors as u32);
    params.max_buckets = Some(max_buckets as u32);
    params.min_overlap = Some(min_overlap as u32);
    params.min_significant_n = Some(min_significant_n as u32);
    params.significance_alpha = Some(significance_alpha);
    params.min_abs_r = Some(min_abs_r);
    params.bucket_aggregation_mode = Some(bucket_aggregation_mode);
    params.value_mode = Some(value_mode);
    params.lag_mode = Some(lag_mode);
    params.max_lag_buckets = Some(max_lag_buckets);

    tracing::info!(
        phase = "start",
        sensor_count = sensor_ids.len(),
        interval_seconds,
        bucket_count,
        method = ?method,
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

    let meta_rows: Vec<SensorMetaRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, name, unit, node_id, type as sensor_type
        FROM sensors
        WHERE sensor_id = ANY($1) AND deleted_at IS NULL
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

    let mut meta_map: HashMap<String, SensorMetaRow> = HashMap::new();
    for row in meta_rows {
        meta_map.insert(row.sensor_id.clone(), row);
    }

    let mut resolved_ids: Vec<String> = Vec::new();
    for id in sensor_ids.iter() {
        if meta_map.contains_key(id) {
            resolved_ids.push(id.clone());
        }
    }
    if resolved_ids.len() < 2 {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "Not enough sensors found".to_string(),
            details: None,
        }));
    }
    sensor_ids = resolved_ids;

    let load_started = Instant::now();
    let rows = read_bucket_series_for_sensors_with_aggregation_and_options(
        db,
        duckdb,
        lake,
        sensor_ids.clone(),
        start,
        end,
        interval_seconds,
        to_bucket_aggregation_preference(bucket_aggregation_mode),
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

    let mut series_map: HashMap<String, Vec<(i64, f64)>> = HashMap::new();
    for row in rows {
        if !row.value.is_finite() {
            continue;
        }
        series_map
            .entry(row.sensor_id.clone())
            .or_default()
            .push((row.bucket.timestamp(), row.value));
    }
    for values in series_map.values_mut() {
        values.sort_by_key(|(ts, _)| *ts);
    }

    if matches!(value_mode, CorrelationValueModeV1::Deltas) {
        let gap_threshold_seconds = 5_i64.saturating_mul(interval_seconds.max(1));
        for values in series_map.values_mut() {
            let mut deltas: Vec<(i64, f64)> = Vec::new();
            for window in values.windows(2) {
                let (prev_ts, prev) = window[0];
                let (curr_ts, curr) = window[1];
                let dt = curr_ts - prev_ts;
                if dt > gap_threshold_seconds {
                    continue;
                }
                let delta = curr - prev;
                if !delta.is_finite() {
                    continue;
                }
                deltas.push((curr_ts, delta));
            }
            *values = deltas;
        }
    }

    let mut rho1_map: HashMap<String, Option<f64>> = HashMap::new();
    for sensor_id in sensor_ids.iter() {
        let series = series_map
            .get(sensor_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);
        let values: Vec<f64> = series.iter().map(|(_, v)| *v).collect();
        rho1_map.insert(sensor_id.clone(), lag1_autocorr(&values));
    }

    progress.phase = "correlate".to_string();
    let total_pairs = (sensor_ids.len() * (sensor_ids.len() + 1) / 2) as u64;
    progress.completed = 0;
    progress.total = Some(total_pairs);
    progress.message = Some(format!("Computing {} correlations", total_pairs));
    let _ = store::update_progress(db, job.id, &progress).await;

    let mut matrix: Vec<Vec<CorrelationMatrixCellV1>> = vec![
        vec![
            CorrelationMatrixCellV1 {
                r: None,
                r_ci_low: None,
                r_ci_high: None,
                p_value: None,
                q_value: None,
                n_eff: None,
                status: None,
                lag_sec: None,
                n: 0,
            };
            sensor_ids.len()
        ];
        sensor_ids.len()
    ];

    let compute_started = Instant::now();
    let mut processed_pairs: u64 = 0;
    let mut p_values_for_fdr: Vec<(usize, f64)> = Vec::new();
    let mut r_for_key: HashMap<usize, f64> = HashMap::new();
    for row_idx in 0..sensor_ids.len() {
        for col_idx in row_idx..sensor_ids.len() {
            if cancel.is_cancelled() {
                return Err(JobFailure::Canceled);
            }

            let a_id = &sensor_ids[row_idx];
            let b_id = &sensor_ids[col_idx];

            if row_idx == col_idx {
                let count = series_map.get(a_id).map(|v| v.len()).unwrap_or(0) as u64;
                matrix[row_idx][col_idx] = CorrelationMatrixCellV1 {
                    r: Some(1.0),
                    r_ci_low: None,
                    r_ci_high: None,
                    p_value: None,
                    q_value: None,
                    n_eff: None,
                    status: Some(CorrelationMatrixCellStatusV1::NotComputed),
                    lag_sec: None,
                    n: count,
                };
            } else {
                let a_series = series_map.get(a_id).map(|v| v.as_slice()).unwrap_or(&[]);
                let b_series = series_map.get(b_id).map(|v| v.as_slice()).unwrap_or(&[]);
                let (r, n, lag_sec) = match lag_mode {
                    CorrelationLagModeV1::Aligned => {
                        let (r, n) = match method {
                            CorrelationMethodV1::Pearson => {
                                pearson_from_series_with_lag(a_series, b_series, 0, min_overlap)
                            }
                            CorrelationMethodV1::Spearman => {
                                spearman_from_series_with_lag(a_series, b_series, 0, min_overlap)
                            }
                        };
                        (r, n, None)
                    }
                    CorrelationLagModeV1::BestWithinMax => {
                        let (r, n, best_lag_sec) = best_corr_within_lag(
                            a_series,
                            b_series,
                            method,
                            min_overlap,
                            interval_seconds,
                            max_lag_buckets,
                        );
                        (r, n, Some(best_lag_sec))
                    }
                };
                let mut cell = CorrelationMatrixCellV1 {
                    r: None,
                    r_ci_low: None,
                    r_ci_high: None,
                    p_value: None,
                    q_value: None,
                    n_eff: None,
                    status: None,
                    lag_sec,
                    n: n as u64,
                };
                if let Some(r) = r {
                    cell.r = Some(r);
                    if n < min_significant_n {
                        cell.status = Some(CorrelationMatrixCellStatusV1::InsufficientOverlap);
                    } else {
                        let rho_a = rho1_map.get(a_id).copied().flatten();
                        let rho_b = rho1_map.get(b_id).copied().flatten();
                        let n_eff = effective_sample_size_lag1(n, rho_a, rho_b);
                        cell.n_eff = Some(n_eff as u64);
                        if n_eff < min_significant_n {
                            cell.status = Some(CorrelationMatrixCellStatusV1::InsufficientOverlap);
                        } else {
                            let p_value = correlation_p_value(r, n_eff, method);
                            cell.p_value = p_value;
                            if let Some(p_value) = p_value {
                                let key = row_idx * sensor_ids.len() + col_idx;
                                p_values_for_fdr.push((key, p_value));
                                r_for_key.insert(key, r);
                                cell.status = None;
                            } else {
                                cell.status = Some(CorrelationMatrixCellStatusV1::NotComputed);
                            }
                        }
                    }
                } else if n < min_overlap {
                    cell.status = Some(CorrelationMatrixCellStatusV1::InsufficientOverlap);
                } else {
                    cell.status = Some(CorrelationMatrixCellStatusV1::NotComputed);
                }
                matrix[row_idx][col_idx] = cell.clone();
                let mut mirror = cell;
                if let Some(lag_sec) = mirror.lag_sec {
                    mirror.lag_sec = Some(-lag_sec);
                }
                matrix[col_idx][row_idx] = mirror;
            }

            processed_pairs += 1;
            if processed_pairs % 10 == 0 || processed_pairs == total_pairs {
                progress.completed = processed_pairs;
                let _ = store::update_progress(db, job.id, &progress).await;
            }
        }
    }
    let compute_ms = compute_started.elapsed().as_millis() as u64;
    tracing::info!(
        phase = "correlate",
        duration_ms = compute_ms,
        sensor_count = sensor_ids.len(),
        pair_count = total_pairs,
        "analysis correlation computed"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "correlate",
            "duration_ms": compute_ms,
            "sensor_count": sensor_ids.len(),
            "pair_count": total_pairs,
        }),
    )
    .await;

    let q_values = bh_fdr_q_values(&p_values_for_fdr);
    for (key, q_value) in q_values {
        let row_idx = key / sensor_ids.len();
        let col_idx = key % sensor_ids.len();

        let mut cell = matrix[row_idx][col_idx].clone();
        cell.q_value = Some(q_value);
        if let Some(r) = r_for_key.get(&key).copied() {
            if passes_fdr_and_effect_size(q_value, r, significance_alpha, min_abs_r) {
                let n_eff = cell.n_eff.map(|v| v as usize).unwrap_or(cell.n as usize);
                let ci = correlation_confidence_interval(r, n_eff, significance_alpha, method);
                cell.r = Some(r);
                cell.r_ci_low = ci.map(|v| v.0);
                cell.r_ci_high = ci.map(|v| v.1);
                cell.status = Some(CorrelationMatrixCellStatusV1::Ok);
            } else {
                cell.status = Some(CorrelationMatrixCellStatusV1::NotSignificant);
            }
        } else {
            cell.status = Some(CorrelationMatrixCellStatusV1::NotComputed);
        }

        matrix[row_idx][col_idx] = cell.clone();
        matrix[col_idx][row_idx] = cell;
    }

    let sensors = sensor_ids
        .iter()
        .map(|id| {
            let meta = meta_map.get(id);
            CorrelationMatrixSensorV1 {
                sensor_id: id.clone(),
                name: meta.map(|m| m.name.clone()),
                unit: meta.map(|m| m.unit.clone()),
                node_id: meta.map(|m| m.node_id.to_string()),
                sensor_type: meta.map(|m| m.sensor_type.clone()),
            }
        })
        .collect::<Vec<_>>();

    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("duckdb_load_ms".to_string(), load_ms);
    timings_ms.insert("correlation_ms".to_string(), compute_ms);
    timings_ms.insert(
        "job_total_ms".to_string(),
        job_started.elapsed().as_millis() as u64,
    );

    let result = CorrelationMatrixResultV1 {
        job_type: "correlation_matrix_v1".to_string(),
        computed_through_ts: replication.computed_through_ts.clone(),
        params,
        sensor_ids: sensor_ids.clone(),
        sensors,
        matrix,
        interval_seconds,
        bucket_count,
        truncated_sensor_ids,
        timings_ms,
        versions: BTreeMap::from([
            (
                "correlation".to_string(),
                match method {
                    CorrelationMethodV1::Pearson => "pearson_v1".to_string(),
                    CorrelationMethodV1::Spearman => "spearman_v1".to_string(),
                },
            ),
            ("fdr".to_string(), "bh_v1".to_string()),
            ("n_eff".to_string(), "lag1_v1".to_string()),
        ]),
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
        .context("failed to serialize correlation_matrix_v1 result")
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

fn pearson_from_series_with_lag(
    a: &[(i64, f64)],
    b: &[(i64, f64)],
    lag_sec: i64,
    min_overlap: usize,
) -> (Option<f64>, usize) {
    let mut i = 0;
    let mut j = 0;
    let mut n: usize = 0;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_x2 = 0.0;
    let mut sum_y2 = 0.0;
    let mut sum_xy = 0.0;

    while i < a.len() && j < b.len() {
        let (ts_a, val_a) = a[i];
        let (ts_b, val_b) = b[j];
        let ts_b_shifted = ts_b.saturating_sub(lag_sec);
        if ts_a == ts_b_shifted {
            if val_a.is_finite() && val_b.is_finite() {
                n += 1;
                sum_x += val_a;
                sum_y += val_b;
                sum_x2 += val_a * val_a;
                sum_y2 += val_b * val_b;
                sum_xy += val_a * val_b;
            }
            i += 1;
            j += 1;
        } else if ts_a < ts_b_shifted {
            i += 1;
        } else {
            j += 1;
        }
    }

    if n < min_overlap {
        return (None, n);
    }
    let n_f = n as f64;
    let denom_x = n_f * sum_x2 - sum_x * sum_x;
    let denom_y = n_f * sum_y2 - sum_y * sum_y;
    let denom = (denom_x * denom_y).sqrt();
    if denom <= 0.0 || !denom.is_finite() {
        return (None, n);
    }
    let r = (n_f * sum_xy - sum_x * sum_y) / denom;
    (Some(r.max(-1.0).min(1.0)), n)
}

fn spearman_from_series_with_lag(
    a: &[(i64, f64)],
    b: &[(i64, f64)],
    lag_sec: i64,
    min_overlap: usize,
) -> (Option<f64>, usize) {
    let mut i = 0;
    let mut j = 0;
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();

    while i < a.len() && j < b.len() {
        let (ts_a, val_a) = a[i];
        let (ts_b, val_b) = b[j];
        let ts_b_shifted = ts_b.saturating_sub(lag_sec);
        if ts_a == ts_b_shifted {
            if val_a.is_finite() && val_b.is_finite() {
                xs.push(val_a);
                ys.push(val_b);
            }
            i += 1;
            j += 1;
        } else if ts_a < ts_b_shifted {
            i += 1;
        } else {
            j += 1;
        }
    }

    if xs.len() < min_overlap {
        return (None, xs.len());
    }

    let rx = ranks(&xs);
    let ry = ranks(&ys);
    pearson_from_aligned(&rx, &ry)
        .map(|r| (Some(r), xs.len()))
        .unwrap_or((None, xs.len()))
}

fn best_corr_within_lag(
    a: &[(i64, f64)],
    b: &[(i64, f64)],
    method: CorrelationMethodV1,
    min_overlap: usize,
    interval_seconds: i64,
    max_lag_buckets: i64,
) -> (Option<f64>, usize, i64) {
    let max_lag_buckets = max_lag_buckets.max(0);
    let interval_seconds = interval_seconds.max(1);

    let mut best_r: Option<f64> = None;
    let mut best_n: usize = 0;
    let mut best_lag_sec: i64 = 0;

    for lag_buckets in -max_lag_buckets..=max_lag_buckets {
        let lag_sec = lag_buckets.saturating_mul(interval_seconds);
        let (r, n) = match method {
            CorrelationMethodV1::Pearson => {
                pearson_from_series_with_lag(a, b, lag_sec, min_overlap)
            }
            CorrelationMethodV1::Spearman => {
                spearman_from_series_with_lag(a, b, lag_sec, min_overlap)
            }
        };
        let Some(r) = r else {
            continue;
        };

        let replace = match best_r {
            None => true,
            Some(prev) => {
                let abs = r.abs();
                let prev_abs = prev.abs();
                abs > prev_abs
                    || (abs == prev_abs && n > best_n)
                    || (abs == prev_abs && n == best_n && lag_sec.abs() < best_lag_sec.abs())
                    || (abs == prev_abs
                        && n == best_n
                        && lag_sec.abs() == best_lag_sec.abs()
                        && lag_sec < best_lag_sec)
            }
        };

        if replace {
            best_r = Some(r);
            best_n = n;
            best_lag_sec = lag_sec;
        }
    }

    (best_r, best_n, best_lag_sec)
}

fn pearson_from_aligned(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() != y.len() || x.len() < 2 {
        return None;
    }
    let n = x.len() as f64;
    let mut sum_x = 0.0;
    let mut sum_y = 0.0;
    let mut sum_x2 = 0.0;
    let mut sum_y2 = 0.0;
    let mut sum_xy = 0.0;
    for (&xi, &yi) in x.iter().zip(y.iter()) {
        if !xi.is_finite() || !yi.is_finite() {
            continue;
        }
        sum_x += xi;
        sum_y += yi;
        sum_x2 += xi * xi;
        sum_y2 += yi * yi;
        sum_xy += xi * yi;
    }
    let denom_x = n * sum_x2 - sum_x * sum_x;
    let denom_y = n * sum_y2 - sum_y * sum_y;
    let denom = (denom_x * denom_y).sqrt();
    if denom <= 0.0 || !denom.is_finite() {
        return None;
    }
    let r = (n * sum_xy - sum_x * sum_y) / denom;
    Some(r.max(-1.0).min(1.0))
}

fn ranks(values: &[f64]) -> Vec<f64> {
    let mut indices: Vec<usize> = (0..values.len()).collect();
    indices.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut ranks = vec![0.0; values.len()];
    let mut i = 0;
    while i < indices.len() {
        let start = i;
        let value = values[indices[i]];
        i += 1;
        while i < indices.len() && values[indices[i]] == value {
            i += 1;
        }
        let end = i;
        let avg_rank = (start + end - 1) as f64 / 2.0 + 1.0;
        for idx in &indices[start..end] {
            ranks[*idx] = avg_rank;
        }
    }
    ranks
}

fn pearson_p_value(r: f64, n: usize) -> Option<f64> {
    pearson_p_value_fisher_z(r, n)
}

fn pearson_confidence_interval(r: f64, n: usize, z_value: f64) -> Option<(f64, f64)> {
    pearson_confidence_interval_fisher_z(r, n, z_value)
}

fn correlation_p_value(r: f64, n: usize, method: CorrelationMethodV1) -> Option<f64> {
    match method {
        CorrelationMethodV1::Pearson => pearson_p_value(r, n),
        CorrelationMethodV1::Spearman => spearman_p_value(r, n),
    }
}

fn correlation_confidence_interval(
    r: f64,
    n: usize,
    alpha: f64,
    method: CorrelationMethodV1,
) -> Option<(f64, f64)> {
    let z_value = z_value_for_alpha_from_alpha(alpha)?;
    match method {
        CorrelationMethodV1::Pearson => pearson_confidence_interval(r, n, z_value),
        CorrelationMethodV1::Spearman => spearman_confidence_interval(r, n, z_value),
    }
}

fn spearman_p_value(r: f64, n: usize) -> Option<f64> {
    spearman_p_value_t_approx(r, n)
}

fn spearman_confidence_interval(r: f64, n: usize, z_value: f64) -> Option<(f64, f64)> {
    spearman_confidence_interval_fisher_z_approx(r, n, z_value)
}

fn passes_fdr_and_effect_size(q_value: f64, r: f64, alpha: f64, min_abs_r: f64) -> bool {
    q_value <= alpha && r.abs() >= min_abs_r
}

#[cfg(test)]
mod tests {
    use super::{best_corr_within_lag, passes_fdr_and_effect_size};
    use crate::services::analysis::tsse::types::CorrelationMethodV1;

    #[test]
    fn effect_size_floor_blocks_tiny_correlation_even_with_good_q() {
        assert!(!passes_fdr_and_effect_size(0.01, 0.05, 0.05, 0.2));
    }

    #[test]
    fn passes_when_q_and_effect_size_both_meet_thresholds() {
        assert!(passes_fdr_and_effect_size(0.01, -0.42, 0.05, 0.2));
    }

    #[test]
    fn best_corr_within_lag_finds_shifted_alignment_and_reports_lag() {
        let interval_seconds = 60;
        let a: Vec<(i64, f64)> = (0..20)
            .map(|idx| (idx * interval_seconds, idx as f64))
            .collect();
        let b: Vec<(i64, f64)> = (0..20)
            .map(|idx| ((idx + 1) * interval_seconds, idx as f64))
            .collect();

        let (r, n, lag_sec) = best_corr_within_lag(
            &a,
            &b,
            CorrelationMethodV1::Pearson,
            3,
            interval_seconds,
            2,
        );

        assert_eq!(lag_sec, interval_seconds);
        assert_eq!(n, 20);
        assert!(r.unwrap_or(0.0) > 0.999);
    }
}
