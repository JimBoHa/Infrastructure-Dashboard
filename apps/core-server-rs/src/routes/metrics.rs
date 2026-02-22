use axum::extract::RawQuery;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, TimeZone, Utc};
use std::collections::BTreeMap;
use url::form_urlencoded;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::services::analysis::bucket_reader;
use crate::services::derived_sensors;
use crate::state::AppState;

const MAX_METRICS_WINDOW_HOURS: i64 = 24 * 365;
const MAX_METRICS_SERIES_SENSORS: usize = 100;
const MAX_METRICS_POINTS: i64 = 10_000_000;
const MAX_INGEST_ITEMS: usize = 50_000;
const CAP_METRICS_VIEW: &str = "metrics.view";
const CAP_METRICS_INGEST: &str = "metrics.ingest";
const SENSOR_CONFIG_SOURCE_FORECAST_POINTS: &str = "forecast_points";
const SENSOR_CONFIG_SOURCE_DERIVED: &str = derived_sensors::SENSOR_CONFIG_SOURCE_DERIVED;

struct MetricsPageWindow {
    end: DateTime<Utc>,
    next_cursor: Option<String>,
}

fn compute_metrics_page_window(
    requested_start: DateTime<Utc>,
    requested_end: DateTime<Utc>,
    cursor_start: DateTime<Utc>,
    interval: i64,
    internal_sensor_count: usize,
) -> MetricsPageWindow {
    let window_seconds = (requested_end - requested_start).num_seconds();
    let interval = interval.max(1);
    let estimated_buckets = (window_seconds / interval).max(0) + 1;

    let internal_sensor_count = std::cmp::max(1, internal_sensor_count) as i64;
    let estimated_points_internal = estimated_buckets.saturating_mul(internal_sensor_count);

    let buckets_per_page = if estimated_points_internal > MAX_METRICS_POINTS {
        let per_sensor = (MAX_METRICS_POINTS / internal_sensor_count).max(1);
        std::cmp::max(25, per_sensor)
    } else {
        estimated_buckets
    };

    let cursor_epoch = cursor_start.timestamp();
    let bucket_start_epoch = cursor_epoch.div_euclid(interval) * interval;
    let bucket_start = Utc
        .timestamp_opt(bucket_start_epoch, 0)
        .single()
        .unwrap_or(cursor_start);

    let page_end_candidate =
        bucket_start + Duration::seconds(interval.saturating_mul(buckets_per_page) as i64);
    let end = std::cmp::min(page_end_candidate, requested_end);
    let next_cursor = if end < requested_end {
        Some(end.to_rfc3339())
    } else {
        None
    };

    MetricsPageWindow { end, next_cursor }
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MetricPoint {
    timestamp: String,
    value: f64,
    samples: i64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MetricSeries {
    sensor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sensor_name: Option<String>,
    points: Vec<MetricPoint>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MetricsResponse {
    series: Vec<MetricSeries>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MetricIngestItem {
    sensor_id: String,
    timestamp: Option<String>,
    value: f64,
    #[serde(default)]
    quality: i32,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MetricIngestRequest {
    items: Vec<MetricIngestItem>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MetricIngestResponse {
    ingested: i64,
}

#[utoipa::path(
    get,
    path = "/api/metrics/query",
    tag = "metrics",
    params(
        ("sensor_ids" = Vec<String>, Query, description = "Sensor ids"),
        ("start" = String, Query, description = "Start timestamp (RFC3339)"),
        ("end" = String, Query, description = "End timestamp (RFC3339)"),
        ("interval" = Option<i64>, Query, description = "Bucket interval (seconds)"),
        ("cursor" = Option<String>, Query, description = "Pagination cursor (RFC3339). When present, returns a page starting at cursor and sets next_cursor for subsequent pages."),
        ("format" = Option<String>, Query, description = "Response format: 'json' (default) or 'binary' (compact binary-v1)")
    ),
    responses(
        (status = 200, description = "Metric series", body = MetricsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid request")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn query_metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    RawQuery(raw): RawQuery,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_METRICS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let mut sensor_ids: Vec<String> = Vec::new();
    let mut start_raw: Option<String> = None;
    let mut end_raw: Option<String> = None;
    let mut interval_raw: Option<i64> = None;
    let mut cursor_raw: Option<String> = None;
    let mut format_raw: Option<String> = None;

    if let Some(raw) = raw {
        for (key, value) in form_urlencoded::parse(raw.as_bytes()) {
            match key.as_ref() {
                "sensor_ids[]" | "sensor_ids" => {
                    let value = value.trim();
                    if !value.is_empty() {
                        sensor_ids.push(value.to_string());
                    }
                }
                "start" => start_raw = Some(value.into_owned()),
                "end" => end_raw = Some(value.into_owned()),
                "interval" => interval_raw = value.parse::<i64>().ok(),
                "cursor" => cursor_raw = Some(value.into_owned()),
                "format" => format_raw = Some(value.into_owned()),
                _ => {}
            }
        }
    }

    let binary_format = format_raw
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("binary"))
        .unwrap_or(false);

    if sensor_ids.is_empty() {
        if binary_format {
            let buf =
                encode_binary_metrics(&[], &BTreeMap::new(), &std::collections::HashMap::new());
            return Ok(binary_response(buf));
        }
        return Ok(json_response(MetricsResponse {
            series: vec![],
            next_cursor: None,
        }));
    }
    if sensor_ids.len() > MAX_METRICS_SERIES_SENSORS {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Too many sensor_ids (max {})", MAX_METRICS_SERIES_SENSORS),
        ));
    }

    let start_raw = start_raw.ok_or((StatusCode::BAD_REQUEST, "Missing start".to_string()))?;
    let end_raw = end_raw.ok_or((StatusCode::BAD_REQUEST, "Missing end".to_string()))?;
    let interval = interval_raw.unwrap_or(1).max(1);

    let requested_start = parse_ts(&start_raw)?;
    let requested_end_inclusive = parse_ts(&end_raw)?;
    if requested_end_inclusive < requested_start {
        return Err((
            StatusCode::BAD_REQUEST,
            "end must be after start".to_string(),
        ));
    }

    // Internally, treat end as inclusive in the API and convert to a paging-safe exclusive bound.
    let requested_end = requested_end_inclusive + Duration::microseconds(1);

    let window_seconds = (requested_end - requested_start).num_seconds();
    if window_seconds <= 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "end must be after start".to_string(),
        ));
    }
    let max_window = Duration::hours(MAX_METRICS_WINDOW_HOURS);
    if requested_end - requested_start > max_window {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Requested window too large (max {} hours)",
                MAX_METRICS_WINDOW_HOURS
            ),
        ));
    }

    let cursor_start = cursor_raw
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(parse_ts)
        .transpose()?
        .unwrap_or(requested_start);
    if cursor_start < requested_start || cursor_start >= requested_end {
        return Err((
            StatusCode::BAD_REQUEST,
            "cursor must be within [start,end]".to_string(),
        ));
    }

    let sensor_name_rows: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT sensor_id, name
        FROM sensors
        WHERE sensor_id = ANY($1)
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;
    let sensor_names: std::collections::HashMap<String, String> =
        sensor_name_rows.into_iter().collect();

    #[derive(sqlx::FromRow)]
    struct SensorConfigRow {
        sensor_id: String,
        node_id: uuid::Uuid,
        config: sqlx::types::Json<serde_json::Value>,
    }

    let sensor_configs: Vec<SensorConfigRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, node_id, COALESCE(config, '{}'::jsonb) as config
        FROM sensors
        WHERE sensor_id = ANY($1)
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    // Collect forecast sensor configs (queried via forecast_points table, not in lake)
    let mut forecast_config_by_id: std::collections::HashMap<
        String,
        (uuid::Uuid, serde_json::Value),
    > = std::collections::HashMap::new();

    for row in sensor_configs {
        let source = row
            .config
            .0
            .get("source")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if source == SENSOR_CONFIG_SOURCE_FORECAST_POINTS {
            forecast_config_by_id.insert(row.sensor_id, (row.node_id, row.config.0));
        }
        // Raw and derived sensors are handled by bucket_reader via lake query
    }

    // Collect non-forecast sensor IDs (raw + derived) for lake query
    let lake_sensor_ids: Vec<String> = sensor_ids
        .iter()
        .filter(|id| !forecast_config_by_id.contains_key(*id))
        .cloned()
        .collect();

    let mut bucketed: BTreeMap<String, Vec<(DateTime<Utc>, f64, i64)>> = BTreeMap::new();

    // Count internal sensors for paging calculation
    let internal_sensor_count = lake_sensor_ids.len() + forecast_config_by_id.len();
    let start;
    let end;
    let next_cursor;

    if binary_format {
        // Binary mode: single response, no pagination. Reject if estimated points exceed cap.
        let window_seconds_est = (requested_end - requested_start).num_seconds();
        let interval_clamped = interval.max(1);
        let estimated_buckets = (window_seconds_est / interval_clamped).max(0) + 1;
        let sensor_count = std::cmp::max(1, internal_sensor_count) as i64;
        let estimated_total = estimated_buckets.saturating_mul(sensor_count);
        if estimated_total > MAX_METRICS_POINTS {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Estimated {} points exceeds binary max of {}. Use a larger interval or shorter window.",
                    estimated_total, MAX_METRICS_POINTS
                ),
            ));
        }
        start = requested_start;
        end = requested_end;
        next_cursor = None;
    } else {
        // JSON mode: paged response with cursor
        start = cursor_start;
        let page = compute_metrics_page_window(
            requested_start,
            requested_end,
            cursor_start,
            interval,
            internal_sensor_count,
        );
        end = page.end;
        next_cursor = page.next_cursor;
    }

    // Query lake for raw + derived sensors via unified bucket reader
    if !lake_sensor_ids.is_empty() {
        let lake_rows = bucket_reader::read_bucket_series_for_sensors(
            &state.db,
            state.analysis_jobs.duckdb(),
            state.analysis_jobs.lake_config(),
            lake_sensor_ids,
            start,
            end,
            interval,
        )
        .await
        .map_err(|err| {
            tracing::warn!(error = %err, "lake bucket read failed");
            (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        })?;

        for row in lake_rows {
            bucketed
                .entry(row.sensor_id)
                .or_default()
                .push((row.bucket, row.value, row.samples));
        }
    }

    // Query forecast_points table for forecast sensors (not in lake)
    #[derive(sqlx::FromRow)]
    struct ForecastBucketRow {
        bucket: DateTime<Utc>,
        avg_value: f64,
        samples: i64,
    }

    for (sensor_id, (node_id, cfg)) in &forecast_config_by_id {
        let provider = cfg
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let kind = cfg
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let metric = cfg
            .get("metric")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let subject_kind = cfg
            .get("subject_kind")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        let subject = cfg
            .get("subject")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
            .unwrap_or_else(|| node_id.to_string());
        let require_asof = cfg
            .get("mode")
            .and_then(|v| v.as_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("asof"));

        if provider.is_empty() || kind.is_empty() || metric.is_empty() || subject_kind.is_empty() {
            continue;
        }

        let rows: Vec<ForecastBucketRow> = sqlx::query_as(
            r#"
            WITH points AS (
              SELECT DISTINCT ON (ts) ts, value, issued_at
              FROM forecast_points
	              WHERE provider = $1
	                AND kind = $2
	                AND subject_kind = $3
	                AND subject = $4
	                AND metric = $5
	                AND ts >= $7
	                AND ts < $8
	                AND ($9::bool = FALSE OR issued_at <= ts)
	              ORDER BY ts ASC, issued_at DESC
	            )
            SELECT
              time_bucket(make_interval(secs => $6), ts) as bucket,
              avg(value) as avg_value,
              count(*) as samples
            FROM points
            GROUP BY bucket
            ORDER BY bucket ASC
            "#,
        )
        .bind(provider)
        .bind(kind)
        .bind(subject_kind)
        .bind(&subject)
        .bind(metric)
        .bind(interval)
        .bind(start)
        .bind(end)
        .bind(require_asof)
        .fetch_all(&state.db)
        .await
        .map_err(map_db_error)?;

        for row in rows {
            bucketed.entry(sensor_id.clone()).or_default().push((
                row.bucket,
                row.avg_value,
                row.samples,
            ));
        }
    }

    if binary_format {
        let buf = encode_binary_metrics(&sensor_ids, &bucketed, &sensor_names);
        return Ok(binary_response(buf));
    }

    let series = sensor_ids
        .iter()
        .map(|sensor_id| {
            let buckets = bucketed.get(sensor_id);
            let mut points: Vec<MetricPoint> = vec![];
            if let Some(entries) = buckets {
                for (bucket_ts, avg_value, count) in entries {
                    if *count <= 0 {
                        continue;
                    }
                    points.push(MetricPoint {
                        timestamp: bucket_ts.to_rfc3339(),
                        value: *avg_value,
                        samples: *count,
                    });
                }
            }
            MetricSeries {
                sensor_id: sensor_id.clone(),
                label: None,
                sensor_name: sensor_names.get(sensor_id).cloned(),
                points,
            }
        })
        .collect();

    Ok(json_response(MetricsResponse {
        series,
        next_cursor,
    }))
}

#[utoipa::path(
    post,
    path = "/api/metrics/ingest",
    tag = "metrics",
    request_body = MetricIngestRequest,
    responses(
        (status = 200, description = "Ingest result", body = MetricIngestResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 413, description = "Payload too large"),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Unknown sensors")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn ingest_metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<MetricIngestRequest>,
) -> Result<Json<MetricIngestResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_METRICS_INGEST, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    if payload.items.len() > MAX_INGEST_ITEMS {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "Too many metrics items (max {}, received {})",
                MAX_INGEST_ITEMS,
                payload.items.len()
            ),
        ));
    }
    if payload.items.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No metrics provided".to_string()));
    }

    let sensor_ids: Vec<String> = payload
        .items
        .iter()
        .map(|item| item.sensor_id.trim().to_string())
        .filter(|sensor_id| !sensor_id.is_empty())
        .collect();
    if sensor_ids.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No metrics provided".to_string()));
    }

    #[derive(sqlx::FromRow)]
    struct KnownSensorRow {
        sensor_id: String,
        source: Option<String>,
    }

    let known: Vec<KnownSensorRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        WHERE sensor_id = ANY($1)
          AND deleted_at IS NULL
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut known_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut derived_sensors: Vec<String> = Vec::new();
    let mut forecast_sensors: Vec<String> = Vec::new();
    for row in known {
        known_set.insert(row.sensor_id.clone());
        if row.source.as_deref() == Some(SENSOR_CONFIG_SOURCE_DERIVED) {
            derived_sensors.push(row.sensor_id.clone());
        }
        if row.source.as_deref() == Some(SENSOR_CONFIG_SOURCE_FORECAST_POINTS) {
            forecast_sensors.push(row.sensor_id.clone());
        }
    }

    let missing: Vec<String> = sensor_ids
        .iter()
        .filter(|sensor_id| !known_set.contains(*sensor_id))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Unknown sensors: {}", missing.join(", ")),
        ));
    }

    if !derived_sensors.is_empty() {
        derived_sensors.sort();
        derived_sensors.dedup();
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot ingest metrics for derived sensors: {}",
                derived_sensors.join(", ")
            ),
        ));
    }

    if !forecast_sensors.is_empty() {
        forecast_sensors.sort();
        forecast_sensors.dedup();
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Cannot ingest metrics for forecast sensors: {}",
                forecast_sensors.join(", ")
            ),
        ));
    }

    let now = Utc::now();
    let mut ingested: i64 = 0;
    for item in payload.items {
        let sensor_id = item.sensor_id.trim();
        if sensor_id.is_empty() {
            continue;
        }
        let ts = item
            .timestamp
            .as_deref()
            .and_then(|raw| DateTime::parse_from_rfc3339(raw.trim()).ok())
            .map(|parsed| parsed.with_timezone(&Utc))
            .unwrap_or(now);
        let quality: i16 = item.quality.clamp(i16::MIN as i32, i16::MAX as i32) as i16;

        let result = sqlx::query(
            r#"
            INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at)
            VALUES ($1, $2, $3, $4, now())
            ON CONFLICT (sensor_id, ts) DO NOTHING
            "#,
        )
        .bind(sensor_id)
        .bind(ts)
        .bind(item.value)
        .bind(quality)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;
        ingested += result.rows_affected() as i64;
    }

    if ingested > 0 {
        crate::services::alarm_engine::schedule_evaluate_for_sensors(
            state.db.clone(),
            sensor_ids.clone(),
        );
    }

    Ok(Json(MetricIngestResponse { ingested }))
}

type MetricsResult = axum::response::Response;

fn json_response(body: MetricsResponse) -> MetricsResult {
    Json(body).into_response()
}

fn binary_response(buf: Vec<u8>) -> MetricsResult {
    (
        [
            (axum::http::header::CONTENT_TYPE, "application/octet-stream"),
            (
                axum::http::header::HeaderName::from_static("x-metrics-format"),
                "binary-v1",
            ),
        ],
        buf,
    )
        .into_response()
}

fn encode_binary_metrics(
    sensor_ids: &[String],
    bucketed: &BTreeMap<String, Vec<(DateTime<Utc>, f64, i64)>>,
    sensor_names: &std::collections::HashMap<String, String>,
) -> Vec<u8> {
    // Filter to series that have points (count > 0)
    let mut series_info: Vec<(&str, &str, &[(DateTime<Utc>, f64, i64)])> = Vec::new();
    let mut total_points: u32 = 0;

    for sensor_id in sensor_ids {
        if let Some(entries) = bucketed.get(sensor_id) {
            let valid: Vec<usize> = entries
                .iter()
                .enumerate()
                .filter(|(_, (_, _, count))| *count > 0)
                .map(|(i, _)| i)
                .collect();
            if valid.is_empty() {
                continue;
            }
            let name = sensor_names
                .get(sensor_id)
                .map(|s| s.as_str())
                .unwrap_or("");
            // We'll store all entries and filter during write
            series_info.push((sensor_id.as_str(), name, entries.as_slice()));
            total_points = total_points.saturating_add(valid.len() as u32);
        }
    }

    let series_count = series_info.len() as u16;

    // Estimate capacity: header(10) + per-series headers + data
    let mut buf = Vec::with_capacity(10 + series_info.len() * 32 + total_points as usize * 8);

    // Magic
    buf.extend_from_slice(b"FDB1");
    // series_count (u16 LE)
    buf.extend_from_slice(&series_count.to_le_bytes());
    // total_point_count (u32 LE)
    buf.extend_from_slice(&total_points.to_le_bytes());

    // Collect filtered points per series for consistent counts in header and data
    let mut series_points: Vec<Vec<(DateTime<Utc>, f64)>> = Vec::with_capacity(series_info.len());

    for (sensor_id, sensor_name, entries) in &series_info {
        let points: Vec<(DateTime<Utc>, f64)> = entries
            .iter()
            .filter(|(_, _, count)| *count > 0)
            .map(|(ts, val, _)| (*ts, *val))
            .collect();
        let point_count = points.len() as u32;
        let base_timestamp_ms: f64 = points
            .first()
            .map(|(ts, _)| ts.timestamp_millis() as f64)
            .unwrap_or(0.0);

        // sensor_id
        let id_bytes = sensor_id.as_bytes();
        buf.extend_from_slice(&(id_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(id_bytes);

        // sensor_name
        let name_bytes = sensor_name.as_bytes();
        if name_bytes.is_empty() {
            buf.extend_from_slice(&0u16.to_le_bytes());
        } else {
            buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            buf.extend_from_slice(name_bytes);
        }

        // point_count (u32 LE)
        buf.extend_from_slice(&point_count.to_le_bytes());
        // base_timestamp_ms (f64 LE)
        buf.extend_from_slice(&base_timestamp_ms.to_le_bytes());

        series_points.push(points);
    }

    // Bulk data
    for points in &series_points {
        if points.is_empty() {
            continue;
        }
        let base_ms = points[0].0.timestamp_millis();
        for (ts, value) in points {
            let offset_seconds = ((ts.timestamp_millis() - base_ms) / 1000) as u32;
            buf.extend_from_slice(&offset_seconds.to_le_bytes());
            buf.extend_from_slice(&(*value as f32).to_le_bytes());
        }
    }

    buf
}

fn parse_ts(raw: &str) -> Result<DateTime<Utc>, (StatusCode, String)> {
    let parsed = DateTime::parse_from_rfc3339(raw.trim())
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid timestamp".to_string()))?;
    Ok(parsed.with_timezone(&Utc))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/metrics/query", get(query_metrics))
        .route("/metrics/ingest", post(ingest_metrics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn paging_returns_full_window_when_under_cap() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 10).unwrap();
        let requested_end = end + Duration::microseconds(1);
        let interval = 1;
        let internal_sensors = 2;

        let page =
            compute_metrics_page_window(start, requested_end, start, interval, internal_sensors);
        assert_eq!(page.end, requested_end);
        assert!(page.next_cursor.is_none());
    }

    #[test]
    fn paging_slices_large_windows_and_advances_cursor() {
        // With 10M cap, we need a window large enough to exceed it.
        // 120 days at 1s interval with 1 sensor = ~10.4M points.
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let end_inclusive = Utc.with_ymd_and_hms(2026, 5, 1, 0, 0, 0).unwrap();
        let requested_end = end_inclusive + Duration::microseconds(1);
        let interval = 1;
        let internal_sensors = 1;

        let first =
            compute_metrics_page_window(start, requested_end, start, interval, internal_sensors);
        assert!(first.end > start);
        assert!(first.next_cursor.is_some());

        let next_cursor = first.next_cursor.clone().unwrap();
        let cursor = parse_ts(&next_cursor).unwrap();
        let second =
            compute_metrics_page_window(start, requested_end, cursor, interval, internal_sensors);
        assert!(second.end > cursor);
    }

    #[test]
    fn binary_encoding_roundtrip() {
        let ts1 = Utc.with_ymd_and_hms(2026, 1, 15, 10, 0, 0).unwrap();
        let ts2 = Utc.with_ymd_and_hms(2026, 1, 15, 10, 0, 5).unwrap();
        let ts3 = Utc.with_ymd_and_hms(2026, 1, 15, 10, 0, 10).unwrap();

        let sensor_ids = vec!["temp_1".to_string()];
        let mut bucketed: BTreeMap<String, Vec<(DateTime<Utc>, f64, i64)>> = BTreeMap::new();
        bucketed.insert(
            "temp_1".to_string(),
            vec![(ts1, 23.5, 1), (ts2, 24.0, 1), (ts3, 24.5, 1)],
        );
        let mut sensor_names = std::collections::HashMap::new();
        sensor_names.insert("temp_1".to_string(), "Temperature".to_string());

        let buf = encode_binary_metrics(&sensor_ids, &bucketed, &sensor_names);

        // Verify magic
        assert_eq!(&buf[0..4], b"FDB1");

        // series_count = 1
        let series_count = u16::from_le_bytes([buf[4], buf[5]]);
        assert_eq!(series_count, 1);

        // total_point_count = 3
        let total_points = u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]);
        assert_eq!(total_points, 3);

        // Parse series header
        let mut pos = 10;
        // sensor_id
        let id_len = u16::from_le_bytes([buf[pos], buf[pos + 1]]) as usize;
        pos += 2;
        let sensor_id = std::str::from_utf8(&buf[pos..pos + id_len]).unwrap();
        assert_eq!(sensor_id, "temp_1");
        pos += id_len;

        // sensor_name
        let name_len = u16::from_le_bytes([buf[pos], buf[pos + 1]]) as usize;
        pos += 2;
        let sensor_name = std::str::from_utf8(&buf[pos..pos + name_len]).unwrap();
        assert_eq!(sensor_name, "Temperature");
        pos += name_len;

        // point_count
        let point_count = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        assert_eq!(point_count, 3);
        pos += 4;

        // base_timestamp_ms
        let base_ts_ms = f64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        assert_eq!(base_ts_ms, ts1.timestamp_millis() as f64);
        pos += 8;

        // Verify bulk data points
        for i in 0..3 {
            let offset = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
            pos += 4;
            let value = f32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
            pos += 4;

            assert_eq!(offset, i * 5); // 0, 5, 10 seconds
            let expected_value = [23.5_f32, 24.0, 24.5][i as usize];
            assert!((value - expected_value).abs() < 0.01);
        }

        assert_eq!(pos, buf.len());
    }

    #[test]
    fn binary_base_timestamp_and_offsets() {
        let base = Utc.with_ymd_and_hms(2026, 6, 1, 0, 0, 0).unwrap();
        let sensor_ids = vec!["flow_1".to_string()];
        let mut bucketed: BTreeMap<String, Vec<(DateTime<Utc>, f64, i64)>> = BTreeMap::new();
        bucketed.insert(
            "flow_1".to_string(),
            vec![
                (base, 100.0, 1),
                (base + Duration::seconds(3600), 200.0, 1),
                (base + Duration::seconds(86400), 300.0, 1),
            ],
        );
        let sensor_names = std::collections::HashMap::new();

        let buf = encode_binary_metrics(&sensor_ids, &bucketed, &sensor_names);

        // Skip to data section
        let mut pos = 10;
        // sensor_id "flow_1" = 6 bytes
        let id_len = u16::from_le_bytes([buf[pos], buf[pos + 1]]) as usize;
        pos += 2 + id_len;
        // sensor_name "" = 0 bytes
        let name_len = u16::from_le_bytes([buf[pos], buf[pos + 1]]) as usize;
        pos += 2 + name_len;
        // point_count + base_ts
        pos += 4 + 8;

        // Check offsets: 0, 3600, 86400
        let offset0 = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        assert_eq!(offset0, 0);
        pos += 8;
        let offset1 = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        assert_eq!(offset1, 3600);
        pos += 8;
        let offset2 = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        assert_eq!(offset2, 86400);
    }

    #[test]
    fn binary_empty_series() {
        let sensor_ids: Vec<String> = vec![];
        let bucketed: BTreeMap<String, Vec<(DateTime<Utc>, f64, i64)>> = BTreeMap::new();
        let sensor_names = std::collections::HashMap::new();

        let buf = encode_binary_metrics(&sensor_ids, &bucketed, &sensor_names);
        assert_eq!(&buf[0..4], b"FDB1");
        let series_count = u16::from_le_bytes([buf[4], buf[5]]);
        assert_eq!(series_count, 0);
        let total_points = u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]);
        assert_eq!(total_points, 0);
        assert_eq!(buf.len(), 10);
    }

    #[test]
    fn paging_aligns_end_to_interval_boundary() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 30).unwrap();
        let end_inclusive = Utc.with_ymd_and_hms(2026, 1, 1, 6, 0, 0).unwrap();
        let requested_end = end_inclusive + Duration::microseconds(1);
        let interval = 60;
        let internal_sensors = 1;

        let page =
            compute_metrics_page_window(start, requested_end, start, interval, internal_sensors);
        assert!(page.end > start);
        assert_eq!(page.end.timestamp() % interval, 0);
    }
}
