use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{require_capabilities, AuthUser};
use crate::error::map_db_error;
use crate::services::analysis::jobs::{
    AnalysisJobCancelResponse, AnalysisJobCreateRequest, AnalysisJobCreateResponse,
    AnalysisJobEventsResponse, AnalysisJobResultResponse, AnalysisJobStatusResponse,
};
use crate::services::analysis::jobs::event_utils::{detect_change_events, EventPoint};
use crate::services::analysis::bucket_reader::{
    read_bucket_series_for_sensors_with_aggregation_and_options, BucketAggregationPreference,
};
use crate::services::analysis::parquet_duckdb::{bucket_coverage_pct, MetricsBucketReadOptions};
use crate::services::analysis::tsse::scoring::{score_related_series, ScoreParams};
use crate::services::analysis::tsse::types::{
    EventPolarityV1, PreviewEventOverlaysV1, TsseEpisodeV1, TssePreviewRequestV1,
    TssePreviewResponseV1, TssePreviewSeriesPointV1, TssePreviewSeriesV1,
};
use crate::state::AppState;

const CAP_ANALYSIS_RUN: &[&str] = &["analysis.run"];
const CAP_ANALYSIS_VIEW: &[&str] = &["analysis.view"];

fn enforce_max_active_jobs(max_jobs: usize, active_jobs: i64) -> Result<(), (StatusCode, String)> {
    let max_jobs = max_jobs.max(1);
    if active_jobs as usize >= max_jobs {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            format!(
                "Too many active analysis jobs (limit {}, active {})",
                max_jobs, active_jobs
            ),
        ));
    }
    Ok(())
}

fn clamp_preview_end_inclusive(
    start: DateTime<Utc>,
    end_inclusive: DateTime<Utc>,
    max_window_seconds: i64,
) -> (DateTime<Utc>, i64) {
    let max_window_seconds = max_window_seconds.max(1);
    let mut end_inclusive = end_inclusive;
    let mut window_seconds = (end_inclusive - start).num_seconds().max(1);
    if window_seconds > max_window_seconds {
        end_inclusive = start + Duration::seconds(max_window_seconds);
        window_seconds = (end_inclusive - start).num_seconds().max(1);
    }
    (end_inclusive, window_seconds)
}

#[derive(Debug, Deserialize)]
pub struct EventsQuery {
    #[serde(default)]
    after: Option<i64>,
    #[serde(default)]
    limit: Option<i64>,
}

#[utoipa::path(
    post,
    path = "/api/analysis/jobs",
    tag = "analysis",
    request_body = AnalysisJobCreateRequest,
    responses(
        (status = 200, description = "Job created (or deduped)", body = AnalysisJobCreateResponse),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid request")
    )
)]
pub async fn create_job(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<AnalysisJobCreateRequest>,
) -> Result<Json<AnalysisJobCreateResponse>, (StatusCode, String)> {
    require_capabilities(&user, CAP_ANALYSIS_RUN).map_err(|err| (err.status, err.message))?;

    if request.job_type.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "job_type is required".to_string()));
    }

    let job_type = request.job_type.trim();
    let user_id = user.user_id();
    if user_id.is_none() && user.source != "api_token" {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid user id".to_string(),
        ));
    }

    if request.dedupe {
        if let Some(job_key) = request
            .job_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some(existing) = state
                .analysis_jobs
                .get_job_by_key(job_type, job_key)
                .await
                .map_err(map_db_error)?
            {
                return Ok(Json(AnalysisJobCreateResponse {
                    job: existing.to_public(),
                }));
            }
        }
    }

    if let Some(user_id) = user_id {
        let max_jobs = state.config.analysis_max_jobs_per_user.max(1);
        let active_jobs = state
            .analysis_jobs
            .count_active_jobs_for_user(user_id)
            .await
            .map_err(map_db_error)?;
        enforce_max_active_jobs(max_jobs, active_jobs)?;
    }
    let (job, _created) = state
        .analysis_jobs
        .create_job(&request, user_id)
        .await
        .map_err(map_db_error)?;
    Ok(Json(AnalysisJobCreateResponse {
        job: job.to_public(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/analysis/jobs/{id}",
    tag = "analysis",
    params(
        ("id" = String, Path, description = "Job id (uuid)")
    ),
    responses(
        (status = 200, description = "Job status", body = AnalysisJobStatusResponse),
        (status = 404, description = "Not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_job(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AnalysisJobStatusResponse>, (StatusCode, String)> {
    require_capabilities(&user, CAP_ANALYSIS_VIEW).map_err(|err| (err.status, err.message))?;
    let job_id = Uuid::parse_str(&id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid job id".to_string()))?;
    let job = state
        .analysis_jobs
        .get_job(job_id)
        .await
        .map_err(map_db_error)?
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;
    Ok(Json(AnalysisJobStatusResponse {
        job: job.to_public(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/analysis/jobs/{id}/events",
    tag = "analysis",
    params(
        ("id" = String, Path, description = "Job id (uuid)"),
        ("after" = Option<i64>, Query, description = "Return events with id > after"),
        ("limit" = Option<i64>, Query, description = "Max events (<=500)")
    ),
    responses(
        (status = 200, description = "Job events", body = AnalysisJobEventsResponse),
        (status = 404, description = "Not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_job_events(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Result<Json<AnalysisJobEventsResponse>, (StatusCode, String)> {
    require_capabilities(&user, CAP_ANALYSIS_VIEW).map_err(|err| (err.status, err.message))?;
    let job_id = Uuid::parse_str(&id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid job id".to_string()))?;
    let after = query.after.unwrap_or(0).max(0);
    let limit = query.limit.unwrap_or(200).clamp(1, 500);

    let job_exists = state
        .analysis_jobs
        .get_job(job_id)
        .await
        .map_err(map_db_error)?
        .is_some();
    if !job_exists {
        return Err((StatusCode::NOT_FOUND, "Job not found".to_string()));
    }

    let events = state
        .analysis_jobs
        .list_events(job_id, after, limit)
        .await
        .map_err(map_db_error)?;
    let next_after = events.last().map(|evt| evt.id);
    Ok(Json(AnalysisJobEventsResponse { events, next_after }))
}

#[utoipa::path(
    post,
    path = "/api/analysis/jobs/{id}/cancel",
    tag = "analysis",
    params(
        ("id" = String, Path, description = "Job id (uuid)")
    ),
    responses(
        (status = 200, description = "Cancellation requested", body = AnalysisJobCancelResponse),
        (status = 404, description = "Not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn cancel_job(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AnalysisJobCancelResponse>, (StatusCode, String)> {
    require_capabilities(&user, CAP_ANALYSIS_RUN).map_err(|err| (err.status, err.message))?;
    let job_id = Uuid::parse_str(&id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid job id".to_string()))?;

    let updated = state
        .analysis_jobs
        .request_cancel(job_id)
        .await
        .map_err(map_db_error)?;

    let job = if let Some(job) = updated {
        job
    } else {
        state
            .analysis_jobs
            .get_job(job_id)
            .await
            .map_err(map_db_error)?
            .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?
    };

    Ok(Json(AnalysisJobCancelResponse {
        job: job.to_public(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/analysis/jobs/{id}/result",
    tag = "analysis",
    params(
        ("id" = String, Path, description = "Job id (uuid)")
    ),
    responses(
        (status = 200, description = "Job result", body = AnalysisJobResultResponse),
        (status = 404, description = "Not found"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_job_result(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<AnalysisJobResultResponse>, (StatusCode, String)> {
    require_capabilities(&user, CAP_ANALYSIS_VIEW).map_err(|err| (err.status, err.message))?;
    let job_id = Uuid::parse_str(&id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid job id".to_string()))?;
    let job = state
        .analysis_jobs
        .get_job(job_id)
        .await
        .map_err(map_db_error)?
        .ok_or((StatusCode::NOT_FOUND, "Job not found".to_string()))?;

    if job.status_enum() != crate::services::analysis::jobs::AnalysisJobStatus::Completed {
        return Err((StatusCode::BAD_REQUEST, "Job is not completed".to_string()));
    }

    let result = state
        .analysis_jobs
        .get_result(job_id)
        .await
        .map_err(map_db_error)?
        .ok_or((StatusCode::NOT_FOUND, "Result not found".to_string()))?;

    Ok(Json(AnalysisJobResultResponse {
        job_id: job_id.to_string(),
        result,
    }))
}

#[utoipa::path(
    post,
    path = "/api/analysis/preview",
    tag = "analysis",
    request_body = TssePreviewRequestV1,
    responses(
        (status = 200, description = "Preview series around an episode", body = TssePreviewResponseV1),
        (status = 400, description = "Invalid request"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn preview(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(request): Json<TssePreviewRequestV1>,
) -> Result<Json<TssePreviewResponseV1>, (StatusCode, String)> {
    require_capabilities(&user, CAP_ANALYSIS_VIEW).map_err(|err| (err.status, err.message))?;

    let focus_sensor_id = request.focus_sensor_id.trim();
    let candidate_sensor_id = request.candidate_sensor_id.trim();
    if focus_sensor_id.is_empty() || candidate_sensor_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "sensor ids are required".to_string(),
        ));
    }

    #[derive(sqlx::FromRow)]
    struct SensorMetaRow {
        sensor_id: String,
        name: String,
        unit: String,
    }

    let meta_rows: Vec<SensorMetaRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, name, unit
        FROM sensors
        WHERE sensor_id = ANY($1) AND deleted_at IS NULL
        "#,
    )
    .bind(&vec![
        focus_sensor_id.to_string(),
        candidate_sensor_id.to_string(),
    ])
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut meta_map: std::collections::HashMap<String, SensorMetaRow> =
        std::collections::HashMap::new();
    for row in meta_rows {
        meta_map.insert(row.sensor_id.clone(), row);
    }
    if !meta_map.contains_key(focus_sensor_id) || !meta_map.contains_key(candidate_sensor_id) {
        return Err((StatusCode::NOT_FOUND, "sensor not found".to_string()));
    }

    let max_points = request.max_points.unwrap_or(5_000).clamp(100, 50_000) as i64;
    let mut selected_episode: Option<TsseEpisodeV1> = None;

    let mut start = if let Some(raw) = request.episode_start_ts.as_deref() {
        Some(
            DateTime::parse_from_rfc3339(raw.trim())
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        "Invalid episode_start_ts".to_string(),
                    )
                })?
                .with_timezone(&Utc),
        )
    } else {
        None
    };
    let mut end_inclusive = if let Some(raw) = request.episode_end_ts.as_deref() {
        Some(
            DateTime::parse_from_rfc3339(raw.trim())
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        "Invalid episode_end_ts".to_string(),
                    )
                })?
                .with_timezone(&Utc),
        )
    } else {
        None
    };

    if start.is_none() || end_inclusive.is_none() {
        let auto_horizon_seconds = 7 * 24 * 3600_i64;
        let end_guess = Utc::now();
        let start_guess = end_guess - Duration::seconds(auto_horizon_seconds);
        let scoring_bucket_seconds =
            ((auto_horizon_seconds as f64) / (max_points as f64)).ceil() as i64;
        let scoring_bucket_seconds = scoring_bucket_seconds.clamp(1, 3600);
        let end_exclusive = end_guess + Duration::microseconds(1);

        let rows = read_bucket_series_for_sensors_with_aggregation_and_options(
            &state.db,
            state.analysis_jobs.duckdb(),
            state.analysis_jobs.lake_config(),
            vec![focus_sensor_id.to_string(), candidate_sensor_id.to_string()],
            start_guess,
            end_exclusive,
            scoring_bucket_seconds,
            BucketAggregationPreference::Auto,
            MetricsBucketReadOptions::analysis_default(),
        )
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

        let mut focus_rows = Vec::new();
        let mut candidate_rows = Vec::new();
        for row in rows {
            if row.sensor_id == focus_sensor_id {
                focus_rows.push(row);
            } else if row.sensor_id == candidate_sensor_id {
                candidate_rows.push(row);
            }
        }

        let lag_max_seconds = request.lag_seconds.unwrap_or(0).abs();
        if let Some(scored) = score_related_series(ScoreParams {
            focus: focus_rows,
            candidate: candidate_rows,
            interval_seconds: scoring_bucket_seconds,
            horizon_seconds: auto_horizon_seconds,
            lag_max_seconds,
            ..ScoreParams::default()
        }) {
            if let Some(best) = scored.episodes.first() {
                selected_episode = Some(best.clone());
                start = DateTime::parse_from_rfc3339(&best.start_ts)
                    .ok()
                    .map(|ts| ts.with_timezone(&Utc));
                end_inclusive = DateTime::parse_from_rfc3339(&best.end_ts)
                    .ok()
                    .map(|ts| ts.with_timezone(&Utc));
            }
        }

        if start.is_none() || end_inclusive.is_none() {
            start = Some(start_guess);
            end_inclusive = Some(end_guess);
        }
    }

    let start = start.ok_or((
        StatusCode::BAD_REQUEST,
        "episode_start_ts is invalid".to_string(),
    ))?;
    let end_inclusive = end_inclusive.ok_or((
        StatusCode::BAD_REQUEST,
        "episode_end_ts is invalid".to_string(),
    ))?;

    if end_inclusive <= start {
        return Err((
            StatusCode::BAD_REQUEST,
            "episode_end_ts must be after start".to_string(),
        ));
    }

    let max_window_seconds = state.config.analysis_preview_max_window_seconds.max(1) as i64;
    let (end_inclusive, window_seconds) =
        clamp_preview_end_inclusive(start, end_inclusive, max_window_seconds);

    let bucket_seconds = ((window_seconds as f64) / (max_points as f64)).ceil() as i64;
    let bucket_seconds = bucket_seconds.clamp(1, 3600);

    let end = end_inclusive + Duration::microseconds(1);

    let rows = read_bucket_series_for_sensors_with_aggregation_and_options(
        &state.db,
        state.analysis_jobs.duckdb(),
        state.analysis_jobs.lake_config(),
        vec![focus_sensor_id.to_string(), candidate_sensor_id.to_string()],
        start,
        end,
        bucket_seconds,
        BucketAggregationPreference::Auto,
        MetricsBucketReadOptions::analysis_default(),
    )
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let mut focus_rows: Vec<crate::services::analysis::parquet_duckdb::MetricsBucketRow> = Vec::new();
    let mut candidate_rows: Vec<crate::services::analysis::parquet_duckdb::MetricsBucketRow> = Vec::new();
    let mut focus_points: Vec<TssePreviewSeriesPointV1> = Vec::new();
    let mut candidate_points: Vec<TssePreviewSeriesPointV1> = Vec::new();
    for row in rows {
        let point = TssePreviewSeriesPointV1 {
            timestamp: row.bucket.to_rfc3339(),
            value: row.value,
            samples: row.samples,
        };
        if row.sensor_id == focus_sensor_id {
            focus_points.push(point);
            focus_rows.push(row);
        } else if row.sensor_id == candidate_sensor_id {
            candidate_points.push(point);
            candidate_rows.push(row);
        }
    }

    focus_rows.sort_by_key(|r| r.bucket);
    candidate_rows.sort_by_key(|r| r.bucket);

    let focus_meta = meta_map.get(focus_sensor_id);
    let candidate_meta = meta_map.get(candidate_sensor_id);

    let focus_bucket_coverage_pct =
        bucket_coverage_pct(focus_points.len() as u64, start, end, bucket_seconds);
    let candidate_bucket_coverage_pct =
        bucket_coverage_pct(candidate_points.len() as u64, start, end, bucket_seconds);

    let focus_series = TssePreviewSeriesV1 {
        sensor_id: focus_sensor_id.to_string(),
        sensor_name: focus_meta.map(|m| m.name.clone()),
        unit: focus_meta.map(|m| m.unit.clone()),
        bucket_coverage_pct: focus_bucket_coverage_pct,
        points: focus_points,
    };
    let candidate_series = TssePreviewSeriesV1 {
        sensor_id: candidate_sensor_id.to_string(),
        sensor_name: candidate_meta.map(|m| m.name.clone()),
        unit: candidate_meta.map(|m| m.unit.clone()),
        bucket_coverage_pct: candidate_bucket_coverage_pct,
        points: candidate_points.clone(),
    };

    let lag_seconds = request
        .lag_seconds
        .or_else(|| selected_episode.as_ref().map(|ep| ep.lag_sec))
        .filter(|v| *v != 0);

    let candidate_aligned = lag_seconds.map(|lag| {
        let ms = lag * 1000;
        let points = candidate_points
            .into_iter()
            .filter_map(|p| {
                let parsed = DateTime::parse_from_rfc3339(&p.timestamp).ok()?;
                let shifted = parsed - Duration::milliseconds(ms);
                Some(TssePreviewSeriesPointV1 {
                    timestamp: shifted.with_timezone(&Utc).to_rfc3339(),
                    ..p
                })
            })
            .collect::<Vec<_>>();
        TssePreviewSeriesV1 {
            sensor_id: candidate_sensor_id.to_string(),
            sensor_name: candidate_meta.map(|m| m.name.clone()),
            unit: candidate_meta.map(|m| m.unit.clone()),
            bucket_coverage_pct: candidate_bucket_coverage_pct,
            points,
        }
    });

    let event_overlays = {
        let overlay_params = request.event_overlay.as_ref();
        let z_threshold = overlay_params
            .and_then(|p| p.z_threshold)
            .unwrap_or(3.0)
            .max(0.1);
        let min_separation_buckets = overlay_params
            .and_then(|p| p.min_separation_buckets)
            .unwrap_or(2)
            .max(0);
        let gap_max_buckets = overlay_params
            .and_then(|p| p.gap_max_buckets)
            .unwrap_or(5)
            .max(0);
        let polarity = overlay_params
            .and_then(|p| p.polarity)
            .unwrap_or(EventPolarityV1::Both);
        let max_events = overlay_params
            .and_then(|p| p.max_events)
            .unwrap_or(2_000)
            .clamp(100, 20_000) as usize;
        let tol_sec = overlay_params
            .and_then(|p| p.tolerance_seconds)
            .unwrap_or(bucket_seconds)
            .max(0);

        let focus_detected = detect_change_events(
            &focus_rows,
            bucket_seconds,
            z_threshold,
            min_separation_buckets,
            gap_max_buckets,
            polarity,
            max_events,
        );
        let candidate_detected = detect_change_events(
            &candidate_rows,
            bucket_seconds,
            z_threshold,
            min_separation_buckets,
            gap_max_buckets,
            polarity,
            max_events,
        );

        let lag_sec_for_match = request
            .lag_seconds
            .or_else(|| selected_episode.as_ref().map(|ep| ep.lag_sec))
            .unwrap_or(0);

        let matched = collect_matched_preview_events(
            &focus_detected.events,
            &candidate_detected.events,
            lag_sec_for_match,
            tol_sec,
        );

        Some(PreviewEventOverlaysV1 {
            focus_event_ts_ms: focus_detected
                .events
                .iter()
                .map(|evt| evt.ts.timestamp_millis())
                .collect(),
            candidate_event_ts_ms: candidate_detected
                .events
                .iter()
                .map(|evt| evt.ts.timestamp_millis())
                .collect(),
            matched_focus_event_ts_ms: matched
                .iter()
                .map(|(focus, _)| focus.ts.timestamp_millis())
                .collect(),
            matched_candidate_event_ts_ms: matched
                .iter()
                .map(|(_, candidate)| candidate.ts.timestamp_millis())
                .collect(),
            tolerance_seconds: if tol_sec > 0 { Some(tol_sec) } else { None },
        })
    };

    Ok(Json(TssePreviewResponseV1 {
        focus: focus_series,
        candidate: candidate_series,
        candidate_aligned,
        selected_episode,
        bucket_seconds,
        event_overlays,
    }))
}

fn collect_matched_preview_events<'a, 'b>(
    focus_events_sorted: &'a [EventPoint],
    candidate_events_sorted: &'b [EventPoint],
    lag_sec: i64,
    tolerance_sec: i64,
) -> Vec<(&'a EventPoint, &'b EventPoint)> {
    if focus_events_sorted.is_empty() || candidate_events_sorted.is_empty() {
        return Vec::new();
    }
    let tol = tolerance_sec.max(0);
    let mut matched: Vec<(&EventPoint, &EventPoint)> = Vec::new();
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
            matched.push((&focus_events_sorted[focus_idx], &candidate_events_sorted[candidate_idx]));
            focus_idx += 1;
            candidate_idx += 1;
        }
    }
    matched
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/analysis/jobs", post(create_job))
        .route("/analysis/jobs/{id}", get(get_job))
        .route("/analysis/jobs/{id}/events", get(get_job_events))
        .route("/analysis/jobs/{id}/result", get(get_job_result))
        .route("/analysis/jobs/{id}/cancel", post(cancel_job))
        .route("/analysis/preview", post(preview))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthManager, AuthenticatedUser};
    use crate::services::analysis::jobs::AnalysisJobService;
    use crate::services::analysis::qdrant::QdrantService;
    use crate::services::deployments::DeploymentManager;
    use crate::services::mqtt::MqttPublisher;
    use chrono::TimeZone;
    use reqwest::Client;
    use sqlx::postgres::PgPoolOptions;
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn enforce_max_active_jobs_returns_429() {
        let err = enforce_max_active_jobs(3, 3).unwrap_err();
        assert_eq!(err.0, StatusCode::TOO_MANY_REQUESTS);
        assert!(err.1.contains("limit 3"));
    }

    #[test]
    fn clamp_preview_end_inclusive_clamps_large_windows() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
        let (clamped, window_seconds) = clamp_preview_end_inclusive(start, end, 3600);
        assert_eq!(clamped, start + Duration::seconds(3600));
        assert_eq!(window_seconds, 3600);
    }

    fn make_user_with_caps(caps: &[&str]) -> AuthUser {
        let mut set = HashSet::new();
        for cap in caps {
            set.insert((*cap).to_string());
        }
        AuthUser(AuthenticatedUser {
            id: "00000000-0000-0000-0000-000000000000".to_string(),
            email: "user@example.com".to_string(),
            role: "view".to_string(),
            capabilities: set,
            source: "test".to_string(),
        })
    }

    async fn make_min_state() -> AppState {
        let tmp = tempfile::tempdir().expect("tempdir");
        let data_root = tmp.path().join("data_root");
        let backup_storage_path = data_root.join("storage/backups");
        let map_storage_path = data_root.join("storage/map");
        let ssh_known_hosts_path = data_root.join("storage/ssh/known_hosts");
        let analysis_lake_hot_path = data_root.join("storage/analysis/lake/hot");
        let analysis_tmp_path = data_root.join("storage/analysis/tmp");

        std::fs::create_dir_all(&backup_storage_path).ok();
        std::fs::create_dir_all(&map_storage_path).ok();
        std::fs::create_dir_all(ssh_known_hosts_path.parent().unwrap()).ok();
        std::fs::create_dir_all(&analysis_lake_hot_path).ok();
        std::fs::create_dir_all(&analysis_tmp_path).ok();

        let config = crate::config::CoreConfig {
            database_url: "postgresql://postgres@localhost/postgres".to_string(),
            mqtt_host: "127.0.0.1".to_string(),
            mqtt_port: 1883,
            mqtt_username: None,
            mqtt_password: None,
            static_root: None,
            setup_daemon_base_url: None,
            data_root,
            backup_storage_path,
            backup_retention_days: 1,
            map_storage_path,
            node_agent_port: 9000,
            node_agent_overlay_path: PathBuf::from("/tmp/overlay.tar.gz"),
            ssh_known_hosts_path,
            demo_mode: true,
            enable_analytics_feeds: false,
            enable_forecast_ingestion: false,
            analytics_feed_poll_interval_seconds: 300,
            forecast_poll_interval_seconds: 3600,
            schedule_poll_interval_seconds: 15,
            enable_external_devices: false,
            external_device_poll_interval_seconds: 30,
            forecast_api_base_url: None,
            forecast_api_path: None,
            rates_api_base_url: None,
            rates_api_path: None,
            analysis_max_concurrent_jobs: 1,
            analysis_poll_interval_ms: 500,
            analysis_lake_hot_path: analysis_lake_hot_path.clone(),
            analysis_lake_cold_path: None,
            analysis_tmp_path: analysis_tmp_path.clone(),
            analysis_lake_shards: 4,
            analysis_hot_retention_days: 90,
            analysis_late_window_hours: 48,
            analysis_replication_interval_seconds: 60,
            analysis_replication_lag_seconds: 300,
            analysis_max_jobs_per_user: 3,
            analysis_preview_max_window_seconds: 3600,
            analysis_embeddings_refresh_enabled: false,
            analysis_embeddings_refresh_interval_seconds: 21_600,
            analysis_embeddings_refresh_horizon_days: 30,
            analysis_embeddings_full_rebuild_interval_hours: 168,
            analysis_embeddings_full_rebuild_horizon_days: 365,
            analysis_profile_enabled: false,
            analysis_profile_output_path: analysis_tmp_path.join("profiles"),
            qdrant_url: "http://127.0.0.1:6333".to_string(),
        };

        let db = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://postgres@localhost/postgres")
            .expect("lazy db");
        let http = Client::new();
        let auth = Arc::new(AuthManager::new(1));

        let (mqtt, handle) =
            MqttPublisher::new("test", "127.0.0.1", 1883, None, None).expect("mqtt");
        handle.abort();
        let mqtt = Arc::new(mqtt);

        let deployments = Arc::new(DeploymentManager::new(
            db.clone(),
            PathBuf::from("/tmp/overlay.tar.gz"),
            analysis_tmp_path.join("known_hosts"),
            9000,
            "mqtt://127.0.0.1:1883".to_string(),
            None,
            None,
        ));

        let qdrant = Arc::new(QdrantService::new(config.qdrant_url.clone(), http.clone()));
        let analysis_jobs = Arc::new(AnalysisJobService::new(db.clone(), &config, qdrant.clone()));

        AppState {
            config,
            db,
            auth,
            mqtt,
            deployments,
            analysis_jobs,
            qdrant,
            http,
        }
    }

    #[tokio::test]
    async fn create_job_requires_analysis_run() {
        let state = make_min_state().await;
        let request = AnalysisJobCreateRequest {
            job_type: "noop_v1".to_string(),
            params: serde_json::json!({}),
            job_key: None,
            dedupe: false,
        };

        let err = create_job(
            State(state),
            make_user_with_caps(&["analysis.view"]),
            Json(request),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert!(err.1.contains("analysis.run"));
    }

    #[tokio::test]
    async fn preview_requires_analysis_view() {
        let state = make_min_state().await;
        let request = TssePreviewRequestV1 {
            focus_sensor_id: "s1".to_string(),
            candidate_sensor_id: "s2".to_string(),
            episode_start_ts: None,
            episode_end_ts: None,
            max_points: None,
            lag_seconds: None,
            event_overlay: None,
        };

        let err = preview(
            State(state),
            make_user_with_caps(&["analysis.run"]),
            Json(request),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert!(err.1.contains("analysis.view"));
    }

    #[tokio::test]
    async fn job_read_endpoints_require_analysis_view() {
        let state = make_min_state().await;

        let err = get_job(
            State(state.clone()),
            make_user_with_caps(&[]),
            Path("not-a-uuid".to_string()),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);

        let err = get_job_events(
            State(state.clone()),
            make_user_with_caps(&[]),
            Path("not-a-uuid".to_string()),
            Query(EventsQuery {
                after: None,
                limit: None,
            }),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);

        let err = get_job_result(
            State(state),
            make_user_with_caps(&[]),
            Path("not-a-uuid".to_string()),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn cancel_requires_analysis_run() {
        let state = make_min_state().await;
        let err = cancel_job(
            State(state),
            make_user_with_caps(&[]),
            Path("not-a-uuid".to_string()),
        )
        .await
        .unwrap_err();
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }
}
