use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::bucket_reader::read_bucket_series_for_sensors;
use crate::services::analysis::lake::read_replication_state;
use crate::services::analysis::parquet_duckdb::DuckDbQueryService;
use crate::services::analysis::tsse::types::{
    MatrixProfileJobParamsV1, MatrixProfileResultV1, MatrixProfileWindowV1,
};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

const MATRIX_PROFILE_DEFAULT_MAX_COMPUTE_MS: u64 = 2_000;
const MATRIX_PROFILE_MAX_COMPUTE_MS_CAP: u64 = 30_000;

pub async fn execute(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &crate::services::analysis::lake::AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: MatrixProfileJobParamsV1 =
        serde_json::from_value(job.params.0.clone()).map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;

    let sensor_id = params.sensor_id.trim().to_string();
    if sensor_id.is_empty() {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "sensor_id is required".to_string(),
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

    let max_points = params.max_points.unwrap_or(512).clamp(64, 4096) as i64;
    let max_windows = params.max_windows.unwrap_or(1024).clamp(64, 4096) as i64;
    let max_compute_ms = params
        .max_compute_ms
        .unwrap_or(MATRIX_PROFILE_DEFAULT_MAX_COMPUTE_MS)
        .min(MATRIX_PROFILE_MAX_COMPUTE_MS_CAP);
    let mut interval_seconds = params.interval_seconds.unwrap_or(60).max(1);
    let horizon_seconds = (end_inclusive - start).num_seconds().max(1);
    let expected_buckets = (horizon_seconds as f64 / interval_seconds as f64).ceil() as i64;
    let max_buckets = max_points.max(max_windows);
    if expected_buckets > max_buckets {
        interval_seconds = ((horizon_seconds as f64) / (max_buckets as f64)).ceil() as i64;
    }

    let replication = read_replication_state(lake).unwrap_or_default();
    let job_started = Instant::now();

    tracing::info!(
        phase = "start",
        sensor_id = %sensor_id,
        interval_seconds,
        "analysis job started"
    );

    #[derive(sqlx::FromRow)]
    struct SensorLabelRow {
        name: String,
        unit: String,
    }
    let sensor_label_row: Option<SensorLabelRow> = sqlx::query_as(
        r#"
        SELECT name, unit
        FROM sensors
        WHERE sensor_id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(&sensor_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "sensor_lookup_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })?;

    let mut progress = AnalysisJobProgress {
        phase: "load_series".to_string(),
        completed: 0,
        total: None,
        message: Some("Loading bucketed series".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let load_started = Instant::now();
    let rows = read_bucket_series_for_sensors(
        db,
        duckdb,
        lake,
        vec![sensor_id.clone()],
        start,
        end,
        interval_seconds,
    )
    .await
    .map_err(|err| JobFailure::Failed(err.to_job_error()))?;
    let load_ms = load_started.elapsed().as_millis() as u64;
    tracing::info!(
        phase = "load_series",
        duration_ms = load_ms,
        "analysis series loaded"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "load_series",
            "duration_ms": load_ms,
        }),
    )
    .await;

    let mut values: Vec<f64> = Vec::new();
    let mut timestamps: Vec<DateTime<Utc>> = Vec::new();
    for row in rows {
        if !row.value.is_finite() {
            continue;
        }
        values.push(row.value);
        timestamps.push(row.bucket);
    }

    let source_points = values.len() as u64;
    let mut warnings: Vec<String> = Vec::new();

    if values.len() < 4 {
        let _ = store::append_event(
            db,
            job.id,
            "phase_timing",
            serde_json::json!({
                "phase": "job_total",
                "duration_ms": job_started.elapsed().as_millis() as u64,
                "reason": "too_few_points",
            }),
        )
        .await;
        let result = MatrixProfileResultV1 {
            job_type: "matrix_profile_v1".to_string(),
            sensor_id,
            sensor_label: sensor_label_row.as_ref().map(|row| row.name.clone()),
            unit: sensor_label_row.as_ref().map(|row| row.unit.clone()),
            computed_through_ts: replication.computed_through_ts.clone(),
            params,
            interval_seconds,
            window_points: 0,
            window: 0,
            exclusion_zone: 0,
            timestamps: timestamps.iter().map(|ts| ts.to_rfc3339()).collect(),
            values: values.clone(),
            window_start_ts: Vec::new(),
            profile: Vec::new(),
            profile_index: Vec::new(),
            step: Some(1),
            effective_interval_seconds: Some(interval_seconds),
            warnings,
            motifs: Vec::new(),
            anomalies: Vec::new(),
            source_points,
            sampled_points: values.len() as u64,
            timings_ms: BTreeMap::from([
                ("duckdb_load_ms".to_string(), load_ms),
                ("compute_ms".to_string(), 0),
                ("compute_budget_ms".to_string(), max_compute_ms),
                (
                    "job_total_ms".to_string(),
                    job_started.elapsed().as_millis() as u64,
                ),
            ]),
            versions: BTreeMap::from([("matrix_profile".to_string(), "v1".to_string())]),
        };
        return serde_json::to_value(&result)
            .context("failed to serialize matrix_profile_v1 result")
            .map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "result_encode_failed".to_string(),
                    message: err.to_string(),
                    details: None,
                })
            });
    }

    let mut sampled_step: u32 = 1;
    let max_points_usize = max_points as usize;
    if values.len() > max_points_usize {
        let step = (values.len() as f64 / max_points_usize as f64)
            .ceil()
            .max(1.0) as usize;
        sampled_step = step as u32;
        let mut sampled_values = Vec::new();
        let mut sampled_ts = Vec::new();
        let mut idx = 0;
        while idx < values.len() {
            sampled_values.push(values[idx]);
            sampled_ts.push(timestamps[idx]);
            idx += step;
        }
        if sampled_values.last() != values.last() {
            sampled_values.push(*values.last().unwrap());
            sampled_ts.push(*timestamps.last().unwrap());
        }
        warnings.push(format!(
            "Downsampled from {} to {} points (step {}).",
            source_points,
            sampled_values.len(),
            sampled_step
        ));
        values = sampled_values;
        timestamps = sampled_ts;
    }

    let sampled_points = values.len() as u64;
    let mut window_points = params
        .window_points
        .unwrap_or(32)
        .clamp(4, values.len() as u32) as usize;
    if window_points >= values.len() {
        window_points = values.len().saturating_sub(1).max(4);
    }
    let exclusion_zone = params
        .exclusion_zone
        .unwrap_or((window_points / 2) as u32)
        .clamp(0, window_points as u32) as usize;
    let top_k = params.top_k.unwrap_or(5).clamp(1, 20) as usize;

    let window_count = values.len().saturating_sub(window_points) + 1;
    let window_step = if window_count as i64 > max_windows {
        ((window_count as f64) / (max_windows as f64))
            .ceil()
            .max(1.0) as usize
    } else {
        1
    };
    if window_step > 1 {
        warnings.push(format!(
            "Sampled windows: computed every {} window(s) ({} → ~{} windows) due to max_windows={}.",
            window_step,
            window_count,
            (window_count as f64 / window_step as f64).ceil() as u64,
            max_windows
        ));
    }

    progress.phase = "compute_profile".to_string();
    progress.completed = 0;
    let windows_computed_target = (window_count as f64 / window_step as f64).ceil().max(1.0) as u64;
    progress.total = Some(windows_computed_target);
    progress.message = Some("Computing matrix profile (bounded)".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    let compute_started = Instant::now();
    let (profile, profile_index, early_stopped, windows_computed) = compute_matrix_profile(
        &values,
        window_points,
        exclusion_zone,
        window_step,
        std::time::Duration::from_millis(max_compute_ms),
        cancel.clone(),
    )?;
    let compute_ms = compute_started.elapsed().as_millis() as u64;
    if early_stopped {
        tracing::warn!(
            phase = "compute_profile",
            duration_ms = compute_ms,
            budget_ms = max_compute_ms,
            windows_computed,
            windows_target = windows_computed_target,
            "analysis matrix profile compute early-stopped"
        );
        warnings.push(format!(
            "Early-stopped matrix profile compute after {} ms (budget {} ms, computed {} of {} windows).",
            compute_ms, max_compute_ms, windows_computed, windows_computed_target
        ));
    }
    tracing::info!(
        phase = "compute_profile",
        duration_ms = compute_ms,
        windows_total = window_count,
        windows_computed_target,
        window_step,
        "analysis matrix profile computed"
    );
    let _ = store::append_event(
        db,
        job.id,
        "phase_timing",
        serde_json::json!({
            "phase": "compute_profile",
            "duration_ms": compute_ms,
            "windows_total": window_count,
            "windows_computed_target": windows_computed_target,
            "windows_computed": windows_computed,
            "window_step": window_step,
            "early_stopped": early_stopped,
            "budget_ms": max_compute_ms,
        }),
    )
    .await;
    if let Some(total) = progress.total {
        progress.completed = total;
        let _ = store::update_progress(db, job.id, &progress).await;
    }

    let window_start_ts = timestamps
        .iter()
        .take(profile.len())
        .map(|ts| ts.to_rfc3339())
        .collect::<Vec<_>>();

    let (motifs, anomalies) = summarize_profile(
        &profile,
        &profile_index,
        &timestamps,
        window_points,
        interval_seconds,
        top_k,
    );

    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("duckdb_load_ms".to_string(), load_ms);
    timings_ms.insert("compute_ms".to_string(), compute_ms);
    timings_ms.insert("compute_budget_ms".to_string(), max_compute_ms);
    timings_ms.insert(
        "job_total_ms".to_string(),
        job_started.elapsed().as_millis() as u64,
    );

    let result = MatrixProfileResultV1 {
        job_type: "matrix_profile_v1".to_string(),
        sensor_id,
        sensor_label: sensor_label_row.as_ref().map(|row| row.name.clone()),
        unit: sensor_label_row.as_ref().map(|row| row.unit.clone()),
        computed_through_ts: replication.computed_through_ts.clone(),
        params,
        interval_seconds,
        window_points: window_points as u32,
        window: window_points as u32,
        exclusion_zone: exclusion_zone as u32,
        timestamps: timestamps.iter().map(|ts| ts.to_rfc3339()).collect(),
        values: values.clone(),
        window_start_ts,
        profile,
        profile_index,
        step: Some(sampled_step),
        effective_interval_seconds: Some(interval_seconds),
        warnings,
        motifs,
        anomalies,
        source_points,
        sampled_points,
        timings_ms,
        versions: BTreeMap::from([("matrix_profile".to_string(), "v1".to_string())]),
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
        .context("failed to serialize matrix_profile_v1 result")
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "result_encode_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })
}

fn compute_matrix_profile(
    values: &[f64],
    window: usize,
    exclusion_zone: usize,
    window_step: usize,
    time_budget: std::time::Duration,
    cancel: CancellationToken,
) -> std::result::Result<(Vec<f64>, Vec<i32>, bool, u64), JobFailure> {
    let n = values.len();
    if n <= window {
        return Ok((Vec::new(), Vec::new(), false, 0));
    }
    let k = n - window + 1;
    if time_budget.is_zero() {
        return Ok((vec![f64::INFINITY; k], vec![-1_i32; k], true, 0));
    }

    let mut prefix = vec![0.0; n + 1];
    let mut prefix_sq = vec![0.0; n + 1];
    for i in 0..n {
        let v = values[i];
        prefix[i + 1] = prefix[i] + v;
        prefix_sq[i + 1] = prefix_sq[i] + v * v;
    }

    let mut normalized = vec![0.0_f32; k * window];
    let mut constant = vec![false; k];

    for start in 0..k {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        let sum = prefix[start + window] - prefix[start];
        let sum_sq = prefix_sq[start + window] - prefix_sq[start];
        let mean = sum / window as f64;
        let variance = (sum_sq / window as f64 - mean * mean).max(0.0);
        let std = variance.sqrt();
        let inv = if std > 1e-12 { 1.0 / std } else { 0.0 };
        if inv == 0.0 {
            constant[start] = true;
        }
        let base = start * window;
        for t in 0..window {
            normalized[base + t] = ((values[start + t] - mean) * inv) as f32;
        }
    }

    let mut profile = vec![f64::INFINITY; k];
    let mut profile_index = vec![-1_i32; k];

    let started = Instant::now();
    let mut early_stopped = false;
    let mut windows_computed: u64 = 0;

    for i in 0..k {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        if window_step > 1 && (i % window_step) != 0 {
            continue;
        }
        if started.elapsed() > time_budget {
            early_stopped = true;
            break;
        }
        windows_computed += 1;
        for j in (i + 1)..k {
            if (j % 256) == 0 {
                if cancel.is_cancelled() {
                    return Err(JobFailure::Canceled);
                }
                if started.elapsed() > time_budget {
                    early_stopped = true;
                    break;
                }
            }
            if window_step > 1 && (j % window_step) != 0 {
                continue;
            }
            if (i as i64 - j as i64).abs() <= exclusion_zone as i64 {
                continue;
            }
            let dist = if constant[i] && constant[j] {
                0.0
            } else if constant[i] != constant[j] {
                (window as f64).sqrt()
            } else {
                let mut dot = 0.0_f32;
                let base_i = i * window;
                let base_j = j * window;
                for t in 0..window {
                    dot += normalized[base_i + t] * normalized[base_j + t];
                }
                let corr = (dot as f64) / window as f64;
                (2.0 * window as f64 * (1.0 - corr)).max(0.0).sqrt()
            };
            if dist < profile[i] {
                profile[i] = dist;
                profile_index[i] = j as i32;
            }
            if dist < profile[j] {
                profile[j] = dist;
                profile_index[j] = i as i32;
            }
        }
        if early_stopped {
            break;
        }

        // progress updates handled by caller to avoid blocking on async writes here.
    }

    Ok((profile, profile_index, early_stopped, windows_computed))
}

fn summarize_profile(
    profile: &[f64],
    profile_index: &[i32],
    timestamps: &[DateTime<Utc>],
    window: usize,
    interval_seconds: i64,
    top_k: usize,
) -> (Vec<MatrixProfileWindowV1>, Vec<MatrixProfileWindowV1>) {
    let mut entries: Vec<(usize, f64)> = profile
        .iter()
        .enumerate()
        .filter(|(_, d)| d.is_finite())
        .map(|(idx, d)| (idx, *d))
        .collect();

    let mut motifs: Vec<MatrixProfileWindowV1> = Vec::new();
    entries.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (idx, dist) in entries.iter().take(top_k) {
        motifs.push(window_summary(
            *idx,
            *dist,
            profile_index,
            timestamps,
            window,
            interval_seconds,
        ));
    }

    let mut anomalies: Vec<MatrixProfileWindowV1> = Vec::new();
    entries.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (idx, dist) in entries.iter().take(top_k) {
        anomalies.push(window_summary(
            *idx,
            *dist,
            profile_index,
            timestamps,
            window,
            interval_seconds,
        ));
    }

    (motifs, anomalies)
}

fn window_summary(
    idx: usize,
    dist: f64,
    profile_index: &[i32],
    timestamps: &[DateTime<Utc>],
    window: usize,
    interval_seconds: i64,
) -> MatrixProfileWindowV1 {
    let start_ts = timestamps
        .get(idx)
        .cloned()
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
    let end_ts = timestamps
        .get(idx + window.saturating_sub(1))
        .cloned()
        .unwrap_or_else(|| start_ts)
        + Duration::seconds(interval_seconds.max(1));
    let match_index = profile_index.get(idx).copied().unwrap_or(-1);
    let (match_start_ts, match_end_ts) = if match_index >= 0 {
        let mi = match_index as usize;
        let ms = timestamps
            .get(mi)
            .cloned()
            .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).unwrap());
        let me = timestamps
            .get(mi + window.saturating_sub(1))
            .cloned()
            .unwrap_or_else(|| ms)
            + Duration::seconds(interval_seconds.max(1));
        (Some(ms), Some(me))
    } else {
        (None, None)
    };

    MatrixProfileWindowV1 {
        window_index: idx as u32,
        start_ts: start_ts.to_rfc3339(),
        end_ts: end_ts.to_rfc3339(),
        distance: dist,
        match_index: if match_index >= 0 {
            Some(match_index as u32)
        } else {
            None
        },
        match_start_ts: match_start_ts.map(|ts| ts.to_rfc3339()),
        match_end_ts: match_end_ts.map(|ts| ts.to_rfc3339()),
    }
}

#[cfg(test)]
mod tests {
    use super::compute_matrix_profile;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn compute_matrix_profile_early_stops_when_budget_zero() {
        let values: Vec<f64> = (0..128).map(|i| i as f64).collect();
        let (profile, profile_index, early_stopped, windows_computed) = match compute_matrix_profile(
            &values,
            16,
            4,
            1,
            std::time::Duration::from_millis(0),
            CancellationToken::new(),
        ) {
            Ok(result) => result,
            Err(_) => panic!("compute_matrix_profile should succeed"),
        };

        assert!(early_stopped);
        assert_eq!(windows_computed, 0);
        assert_eq!(profile.len(), values.len() - 16 + 1);
        assert_eq!(profile_index.len(), profile.len());
        assert!(profile.iter().all(|v| v.is_infinite()));
    }

    #[test]
    fn compute_matrix_profile_completes_with_reasonable_budget() {
        let values: Vec<f64> = (0..96)
            .map(|i| (i as f64).sin() * 10.0 + i as f64)
            .collect();
        let (profile, profile_index, early_stopped, windows_computed) = match compute_matrix_profile(
            &values,
            12,
            3,
            1,
            std::time::Duration::from_secs(1),
            CancellationToken::new(),
        ) {
            Ok(result) => result,
            Err(_) => panic!("compute_matrix_profile should succeed"),
        };

        assert!(!early_stopped);
        assert_eq!(profile.len(), values.len() - 12 + 1);
        assert_eq!(profile_index.len(), profile.len());
        assert!(windows_computed > 0);
        assert!(profile.iter().any(|v| v.is_finite()));
    }
}
