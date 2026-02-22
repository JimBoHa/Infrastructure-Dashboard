use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::core_node;
use crate::error::map_db_error;
use crate::services::forecasts;
use crate::services::virtual_sensors;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ForecastDatumRead {
    id: i64,
    field: String,
    horizon_hours: i32,
    value: f64,
    recorded_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ForecastIngestItem {
    field: String,
    #[serde(default = "default_horizon_hours")]
    horizon_hours: i32,
    value: f64,
    recorded_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ForecastIngestRequest {
    items: Vec<ForecastIngestItem>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ForecastIngestResponse {
    ingested: i64,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct LatestForecastQuery {
    field: String,
    #[serde(default = "default_horizon_hours")]
    horizon_hours: i32,
}

fn default_horizon_hours() -> i32 {
    24
}

fn default_window_hours() -> i32 {
    72
}

fn default_window_days() -> i32 {
    7
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct CurrentWeatherQuery {
    node_id: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct CurrentWeatherMetric {
    unit: String,
    value: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct CurrentWeatherResponse {
    provider: String,
    latitude: f64,
    longitude: f64,
    observed_at: String,
    fetched_at: String,
    metrics: BTreeMap<String, CurrentWeatherMetric>,
}

#[derive(Clone)]
struct CachedCurrentWeather {
    fetched_at: DateTime<Utc>,
    response: CurrentWeatherResponse,
}

static CURRENT_WEATHER_CACHE: OnceLock<RwLock<HashMap<String, CachedCurrentWeather>>> =
    OnceLock::new();

fn weather_cache() -> &'static RwLock<HashMap<String, CachedCurrentWeather>> {
    CURRENT_WEATHER_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn geojson_point_to_lat_lng(value: &JsonValue) -> Option<(f64, f64)> {
    let obj = value.as_object()?;
    if obj.get("type")?.as_str()? != "Point" {
        return None;
    }
    let coords = obj.get("coordinates")?.as_array()?;
    if coords.len() < 2 {
        return None;
    }
    let lng = coords[0].as_f64()?;
    let lat = coords[1].as_f64()?;
    Some((lat, lng))
}

#[utoipa::path(
    get,
    path = "/api/forecast",
    tag = "forecast",
    responses((status = 200, description = "Latest forecast values", body = Vec<ForecastDatumRead>))
)]
pub(crate) async fn latest_forecast(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<ForecastDatumRead>>, (StatusCode, String)> {
    #[derive(sqlx::FromRow)]
    struct ForecastRow {
        id: i64,
        field: String,
        horizon_hours: i32,
        value: f64,
        recorded_at: DateTime<Utc>,
    }

    let rows: Vec<ForecastRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (field, horizon_hours)
            id,
            field,
            horizon_hours,
            value,
            recorded_at
        FROM forecast_data
        ORDER BY field, horizon_hours, recorded_at DESC, id DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| ForecastDatumRead {
                id: row.id,
                field: row.field,
                horizon_hours: row.horizon_hours,
                value: row.value,
                recorded_at: row.recorded_at.to_rfc3339(),
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/forecast/latest",
    tag = "forecast",
    params(LatestForecastQuery),
    responses(
        (status = 200, description = "Latest forecast datum", body = ForecastDatumRead),
        (status = 404, description = "Forecast data not found")
    )
)]
pub(crate) async fn latest_forecast_value(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(query): Query<LatestForecastQuery>,
) -> Result<Json<ForecastDatumRead>, (StatusCode, String)> {
    let field = query.field.trim();
    if field.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "field is required".to_string()));
    }
    let horizon_hours = query.horizon_hours.clamp(1, 24 * 14);

    #[derive(sqlx::FromRow)]
    struct ForecastRow {
        id: i64,
        field: String,
        horizon_hours: i32,
        value: f64,
        recorded_at: DateTime<Utc>,
    }

    let row: Option<ForecastRow> = sqlx::query_as(
        r#"
        SELECT id, field, horizon_hours, value, recorded_at
        FROM forecast_data
        WHERE field = $1
          AND horizon_hours = $2
        ORDER BY recorded_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(field)
    .bind(horizon_hours)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Forecast data not found".to_string()));
    };

    Ok(Json(ForecastDatumRead {
        id: row.id,
        field: row.field,
        horizon_hours: row.horizon_hours,
        value: row.value,
        recorded_at: row.recorded_at.to_rfc3339(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/forecast/status",
    tag = "forecast",
    responses((status = 200, description = "Forecast ingest status", body = JsonValue))
)]
pub(crate) async fn forecast_status(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    #[derive(sqlx::FromRow)]
    struct StatusRow {
        name: String,
        status: String,
        recorded_at: DateTime<Utc>,
        metadata: SqlJson<JsonValue>,
    }

    let rows: Vec<StatusRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (name) name, status, recorded_at, metadata
        FROM analytics_integration_status
        WHERE category = 'forecast'
        ORDER BY name, recorded_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut providers = BTreeMap::new();
    for row in rows {
        let meta = row.metadata.0;
        let details = meta
            .get("detail")
            .or_else(|| meta.get("reason"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        providers.insert(
            row.name,
            serde_json::json!({
                "status": row.status,
                "last_seen": row.recorded_at.to_rfc3339(),
                "details": details,
                "meta": meta,
            }),
        );
    }

    Ok(Json(serde_json::json!({
        "enabled": state.config.enable_forecast_ingestion,
        "providers": providers,
    })))
}

#[utoipa::path(
    post,
    path = "/api/forecast/poll",
    tag = "forecast",
    responses((status = 200, description = "Triggered poll", body = JsonValue)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn poll_forecast(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let results = forecasts::poll_all_forecasts(&state)
        .await
        .map_err(map_internal_error)?;
    let mut providers = BTreeMap::new();
    for result in results {
        providers.insert(result.name, result.status);
    }
    Ok(Json(
        serde_json::json!({ "status": "ok", "providers": providers }),
    ))
}

fn map_internal_error(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

#[utoipa::path(
    post,
    path = "/api/forecast/ingest",
    tag = "forecast",
    request_body = ForecastIngestRequest,
    responses((status = 201, description = "Ingested forecast data", body = ForecastIngestResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn ingest_forecast(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<ForecastIngestRequest>,
) -> Result<(StatusCode, Json<ForecastIngestResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if payload.items.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No forecast items provided".to_string(),
        ));
    }

    let mut ingested: i64 = 0;
    for item in payload.items {
        let field = item.field.trim();
        if field.is_empty() {
            continue;
        }
        let horizon = item.horizon_hours.clamp(1, 24 * 14);
        let recorded_at = item
            .recorded_at
            .as_deref()
            .and_then(|raw| DateTime::parse_from_rfc3339(raw.trim()).ok())
            .map(|ts| ts.with_timezone(&Utc))
            .unwrap_or_else(Utc::now);

        let result = sqlx::query(
            r#"
            INSERT INTO forecast_data (field, horizon_hours, value, recorded_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(field)
        .bind(horizon)
        .bind(item.value)
        .bind(recorded_at)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;
        ingested += result.rows_affected() as i64;
    }

    Ok((
        StatusCode::CREATED,
        Json(ForecastIngestResponse { ingested }),
    ))
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ForecastSeriesPoint {
    timestamp: String,
    value: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ForecastSeriesMetric {
    unit: String,
    points: Vec<ForecastSeriesPoint>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ForecastSeriesResponse {
    provider: String,
    kind: String,
    subject_kind: String,
    subject: String,
    issued_at: String,
    metrics: BTreeMap<String, ForecastSeriesMetric>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct WindowHoursQuery {
    #[serde(default = "default_window_hours")]
    hours: i32,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct PvWindowHoursQuery {
    #[serde(default = "default_window_hours")]
    hours: i32,
    #[serde(default)]
    history_hours: i32,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct WindowDaysQuery {
    #[serde(default = "default_window_days")]
    days: i32,
}

async fn load_latest_series(
    state: &AppState,
    provider: &str,
    kind: &str,
    subject_kind: &str,
    subject: &str,
    metrics: &[&str],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ForecastSeriesResponse, (StatusCode, String)> {
    let issued_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"
        SELECT issued_at
        FROM forecast_points
        WHERE provider = $1
          AND kind = $2
          AND subject_kind = $3
          AND subject = $4
        ORDER BY issued_at DESC
        LIMIT 1
        "#,
    )
    .bind(provider)
    .bind(kind)
    .bind(subject_kind)
    .bind(subject)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(issued_at) = issued_at else {
        return Err((
            StatusCode::NOT_FOUND,
            "Forecast series not found".to_string(),
        ));
    };

    #[derive(sqlx::FromRow)]
    struct PointRow {
        metric: String,
        unit: String,
        ts: DateTime<Utc>,
        value: f64,
    }

    let metric_list: Vec<String> = metrics.iter().map(|m| m.to_string()).collect();
    let rows: Vec<PointRow> = sqlx::query_as(
        r#"
        SELECT metric, unit, ts, value
        FROM forecast_points
        WHERE provider = $1
          AND kind = $2
          AND subject_kind = $3
          AND subject = $4
          AND issued_at = $5
          AND metric = ANY($6)
          AND ts >= $7
          AND ts <= $8
        ORDER BY metric, ts ASC
        "#,
    )
    .bind(provider)
    .bind(kind)
    .bind(subject_kind)
    .bind(subject)
    .bind(issued_at)
    .bind(metric_list)
    .bind(start)
    .bind(end)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut metrics_map: BTreeMap<String, ForecastSeriesMetric> = BTreeMap::new();
    for row in rows {
        let entry = metrics_map
            .entry(row.metric.clone())
            .or_insert_with(|| ForecastSeriesMetric {
                unit: row.unit.clone(),
                points: Vec::new(),
            });
        entry.points.push(ForecastSeriesPoint {
            timestamp: row.ts.to_rfc3339(),
            value: row.value,
        });
    }

    Ok(ForecastSeriesResponse {
        provider: provider.to_string(),
        kind: kind.to_string(),
        subject_kind: subject_kind.to_string(),
        subject: subject.to_string(),
        issued_at: issued_at.to_rfc3339(),
        metrics: metrics_map,
    })
}

async fn load_latest_asof_series(
    state: &AppState,
    provider: &str,
    kind: &str,
    subject_kind: &str,
    subject: &str,
    metrics: &[&str],
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<ForecastSeriesResponse, (StatusCode, String)> {
    #[derive(sqlx::FromRow)]
    struct PointRow {
        metric: String,
        unit: String,
        ts: DateTime<Utc>,
        value: f64,
        issued_at: DateTime<Utc>,
    }

    let metric_list: Vec<String> = metrics.iter().map(|m| m.to_string()).collect();
    let rows: Vec<PointRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (metric, ts) metric, unit, ts, value, issued_at
        FROM forecast_points
        WHERE provider = $1
          AND kind = $2
          AND subject_kind = $3
          AND subject = $4
          AND metric = ANY($5)
          AND ts >= $6
          AND ts <= $7
          AND issued_at <= ts
        ORDER BY metric, ts ASC, issued_at DESC
        "#,
    )
    .bind(provider)
    .bind(kind)
    .bind(subject_kind)
    .bind(subject)
    .bind(metric_list)
    .bind(start)
    .bind(end)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    if rows.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            "Forecast series not found".to_string(),
        ));
    }

    let mut latest_issue: Option<DateTime<Utc>> = None;
    let mut metrics_map: BTreeMap<String, ForecastSeriesMetric> = BTreeMap::new();
    for row in rows {
        latest_issue = Some(match latest_issue {
            Some(existing) if existing > row.issued_at => existing,
            _ => row.issued_at,
        });
        let entry = metrics_map
            .entry(row.metric.clone())
            .or_insert_with(|| ForecastSeriesMetric {
                unit: row.unit.clone(),
                points: Vec::new(),
            });
        entry.points.push(ForecastSeriesPoint {
            timestamp: row.ts.to_rfc3339(),
            value: row.value,
        });
    }

    Ok(ForecastSeriesResponse {
        provider: provider.to_string(),
        kind: kind.to_string(),
        subject_kind: subject_kind.to_string(),
        subject: subject.to_string(),
        issued_at: latest_issue.unwrap_or_else(Utc::now).to_rfc3339(),
        metrics: metrics_map,
    })
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct WeatherForecastConfigResponse {
    enabled: bool,
    provider: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct WeatherForecastConfigRequest {
    enabled: bool,
    latitude: f64,
    longitude: f64,
    provider: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/forecast/weather/config",
    tag = "forecast",
    responses((status = 200, description = "Weather forecast configuration", body = WeatherForecastConfigResponse))
)]
pub(crate) async fn get_weather_config(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<WeatherForecastConfigResponse>, (StatusCode, String)> {
    #[derive(sqlx::FromRow)]
    struct Row {
        value: String,
        metadata: SqlJson<JsonValue>,
        updated_at: DateTime<Utc>,
    }

    let row: Option<Row> = sqlx::query_as(
        r#"
        SELECT value, metadata, updated_at
        FROM setup_credentials
        WHERE name = 'weather_forecast'
        "#,
    )
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Ok(Json(WeatherForecastConfigResponse {
            enabled: false,
            provider: None,
            latitude: None,
            longitude: None,
            updated_at: None,
        }));
    };

    let meta = row.metadata.0;
    Ok(Json(WeatherForecastConfigResponse {
        enabled: meta
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        provider: Some(row.value),
        latitude: meta.get("latitude").and_then(|v| v.as_f64()),
        longitude: meta.get("longitude").and_then(|v| v.as_f64()),
        updated_at: Some(row.updated_at.to_rfc3339()),
    }))
}

#[utoipa::path(
    put,
    path = "/api/forecast/weather/config",
    tag = "forecast",
    request_body = WeatherForecastConfigRequest,
    responses((status = 200, description = "Updated weather forecast configuration", body = WeatherForecastConfigResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_weather_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<WeatherForecastConfigRequest>,
) -> Result<Json<WeatherForecastConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if !(-90.0..=90.0).contains(&payload.latitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "latitude must be -90..90".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&payload.longitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "longitude must be -180..180".to_string(),
        ));
    }

    let provider = payload
        .provider
        .unwrap_or_else(|| forecasts::PROVIDER_OPEN_METEO.to_string());

    let metadata = serde_json::json!({
        "enabled": payload.enabled,
        "latitude": payload.latitude,
        "longitude": payload.longitude,
    });

    let updated_at: DateTime<Utc> = sqlx::query_scalar(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ('weather_forecast', $1, $2, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        RETURNING updated_at
        "#,
    )
    .bind(&provider)
    .bind(SqlJson(metadata))
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(WeatherForecastConfigResponse {
        enabled: payload.enabled,
        provider: Some(provider),
        latitude: Some(payload.latitude),
        longitude: Some(payload.longitude),
        updated_at: Some(updated_at.to_rfc3339()),
    }))
}

#[utoipa::path(
    get,
    path = "/api/forecast/weather/hourly",
    tag = "forecast",
    params(WindowHoursQuery),
    responses((status = 200, description = "Latest hourly weather forecast series", body = ForecastSeriesResponse))
)]
pub(crate) async fn weather_hourly(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(query): Query<WindowHoursQuery>,
) -> Result<Json<ForecastSeriesResponse>, (StatusCode, String)> {
    let Some(cfg) = forecasts::load_weather_config(&state)
        .await
        .map_err(map_internal_error)?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            "Weather forecast not configured".to_string(),
        ));
    };
    if !cfg.enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            "Weather forecast is disabled".to_string(),
        ));
    }

    let hours = query.hours.clamp(1, 168);
    let start = Utc::now();
    let end = start + chrono::Duration::hours(hours as i64);
    let series = load_latest_series(
        &state,
        forecasts::PROVIDER_OPEN_METEO,
        forecasts::KIND_WEATHER,
        forecasts::SUBJECT_KIND_LOCATION,
        forecasts::SUBJECT_CONTROLLER,
        &[
            forecasts::METRIC_WEATHER_TEMPERATURE_C,
            forecasts::METRIC_WEATHER_HUMIDITY_PCT,
            forecasts::METRIC_WEATHER_PRECIP_MM,
            forecasts::METRIC_WEATHER_WIND_SPEED_MPS,
            forecasts::METRIC_WEATHER_WIND_DIR_DEG,
            forecasts::METRIC_WEATHER_CLOUD_COVER_PCT,
        ],
        start,
        end,
    )
    .await?;
    Ok(Json(series))
}

#[utoipa::path(
    get,
    path = "/api/forecast/weather/daily",
    tag = "forecast",
    params(WindowDaysQuery),
    responses((status = 200, description = "Latest daily weather forecast series", body = ForecastSeriesResponse))
)]
pub(crate) async fn weather_daily(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(query): Query<WindowDaysQuery>,
) -> Result<Json<ForecastSeriesResponse>, (StatusCode, String)> {
    let Some(cfg) = forecasts::load_weather_config(&state)
        .await
        .map_err(map_internal_error)?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            "Weather forecast not configured".to_string(),
        ));
    };
    if !cfg.enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            "Weather forecast is disabled".to_string(),
        ));
    }

    let days = query.days.clamp(1, 14);
    let now = Utc::now();
    let today = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid date".to_string(),
        )
    })?;
    let start = Utc.from_utc_datetime(&today.and_hms_opt(0, 0, 0).unwrap());
    let end = start + chrono::Duration::days(days as i64);
    let series = load_latest_series(
        &state,
        forecasts::PROVIDER_OPEN_METEO,
        forecasts::KIND_WEATHER,
        forecasts::SUBJECT_KIND_LOCATION,
        forecasts::SUBJECT_CONTROLLER,
        &[
            forecasts::METRIC_WEATHER_TEMPERATURE_MAX_C,
            forecasts::METRIC_WEATHER_TEMPERATURE_MIN_C,
            forecasts::METRIC_WEATHER_PRECIP_SUM_MM,
            forecasts::METRIC_WEATHER_CLOUD_COVER_MEAN_PCT,
        ],
        start,
        end,
    )
    .await?;
    Ok(Json(series))
}

fn parse_open_meteo_current_time(raw: &str) -> Option<DateTime<Utc>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Some(dt.with_timezone(&Utc));
    }
    let naive = chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M").ok()?;
    Some(Utc.from_utc_datetime(&naive))
}

#[derive(Debug, serde::Deserialize)]
struct OpenMeteoCurrentPayload {
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    current_units: BTreeMap<String, String>,
    current: Option<OpenMeteoCurrentBlock>,
}

#[derive(Debug, serde::Deserialize)]
struct OpenMeteoCurrentBlock {
    time: String,
    #[serde(default)]
    temperature_2m: Option<f64>,
    #[serde(default)]
    precipitation: Option<f64>,
    #[serde(default)]
    cloudcover: Option<f64>,
    #[serde(default)]
    wind_speed_10m: Option<f64>,
    #[serde(default)]
    wind_direction_10m: Option<f64>,
    #[serde(default)]
    relative_humidity_2m: Option<f64>,
    #[serde(default)]
    pressure_msl: Option<f64>,
}

#[utoipa::path(
    get,
    path = "/api/forecast/weather/current",
    tag = "forecast",
    params(CurrentWeatherQuery),
    responses(
        (status = 200, description = "Current weather at a coordinate", body = CurrentWeatherResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Location not found")
    )
)]
pub(crate) async fn weather_current(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(query): Query<CurrentWeatherQuery>,
) -> Result<Json<CurrentWeatherResponse>, (StatusCode, String)> {
    let mut sensor_target_node_id: uuid::Uuid = core_node::CORE_NODE_ID;
    let mut subject_kind: &str = forecasts::SUBJECT_KIND_LOCATION;
    let mut subject: String = forecasts::SUBJECT_CONTROLLER.to_string();

    let (latitude, longitude) = if let Some(node_id) = query.node_id.as_deref() {
        let node_uuid = Uuid::parse_str(node_id.trim())
            .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;
        sensor_target_node_id = node_uuid;
        subject_kind = forecasts::SUBJECT_KIND_NODE;
        subject = node_uuid.to_string();

        let geometry: Option<SqlJson<JsonValue>> = sqlx::query_scalar(
            r#"
            SELECT mf.geometry
            FROM map_features mf
            JOIN map_settings ms ON ms.singleton = TRUE
            WHERE mf.save_id = ms.active_save_id
              AND mf.node_id = $1
            LIMIT 1
            "#,
        )
        .bind(node_uuid)
        .fetch_optional(&state.db)
        .await
        .map_err(map_db_error)?;

        let geometry = match geometry {
            Some(geometry) => Some(geometry),
            None if core_node::is_core_node_id(node_uuid) => None,
            None => {
                return Err((
                    StatusCode::NOT_FOUND,
                    "Node is not placed on the active map".to_string(),
                ));
            }
        };

        if let Some(geometry) = geometry {
            let Some((lat, lng)) = geojson_point_to_lat_lng(&geometry.0) else {
                return Err((
                    StatusCode::NOT_FOUND,
                    "Node map placement is not a point".to_string(),
                ));
            };
            (lat, lng)
        } else {
            let Some(cfg) = forecasts::load_weather_config(&state)
                .await
                .map_err(map_internal_error)?
            else {
                return Err((
                    StatusCode::NOT_FOUND,
                    "Weather forecast is not configured for controller location".to_string(),
                ));
            };
            (cfg.latitude, cfg.longitude)
        }
    } else {
        let Some(latitude) = query.latitude else {
            return Err((StatusCode::BAD_REQUEST, "latitude is required".to_string()));
        };
        let Some(longitude) = query.longitude else {
            return Err((StatusCode::BAD_REQUEST, "longitude is required".to_string()));
        };
        (latitude, longitude)
    };

    if !(-90.0..=90.0).contains(&latitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "latitude must be -90..90".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&longitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "longitude must be -180..180".to_string(),
        ));
    }

    let key = format!("{latitude:.4},{longitude:.4}");
    let now = Utc::now();
    {
        let cache = weather_cache().read().await;
        if let Some(hit) = cache.get(&key) {
            if now.signed_duration_since(hit.fetched_at) < chrono::Duration::seconds(60) {
                return Ok(Json(hit.response.clone()));
            }
        }
    }

    let url = "https://api.open-meteo.com/v1/forecast";
    let response = state
        .http
        .get(url)
        .query(&[
            ("latitude", latitude.to_string()),
            ("longitude", longitude.to_string()),
            (
                "current",
                "temperature_2m,precipitation,wind_speed_10m,wind_direction_10m,cloudcover,relative_humidity_2m,pressure_msl"
                    .to_string(),
            ),
            ("timezone", "UTC".to_string()),
            ("temperature_unit", "celsius".to_string()),
            ("wind_speed_unit", "ms".to_string()),
            ("precipitation_unit", "mm".to_string()),
        ])
        .timeout(Duration::from_secs(12))
        .send()
        .await
        .map_err(|err| (StatusCode::BAD_GATEWAY, format!("Open-Meteo request failed: {err}")))?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Open-Meteo HTTP {status}: {body}"),
        ));
    }

    let payload: OpenMeteoCurrentPayload = response.json().await.map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Open-Meteo decode failed: {err}"),
        )
    })?;

    let Some(current) = payload.current else {
        return Err((
            StatusCode::BAD_GATEWAY,
            "Open-Meteo missing current block".to_string(),
        ));
    };

    let observed_at_dt = parse_open_meteo_current_time(&current.time).unwrap_or_else(Utc::now);
    let observed_at = observed_at_dt.to_rfc3339();
    let unit_temp = payload
        .current_units
        .get("temperature_2m")
        .cloned()
        .unwrap_or_else(|| "°C".to_string());
    let unit_precip = payload
        .current_units
        .get("precipitation")
        .cloned()
        .unwrap_or_else(|| "mm".to_string());
    let unit_cloud = payload
        .current_units
        .get("cloudcover")
        .cloned()
        .unwrap_or_else(|| "%".to_string());
    let unit_wind_speed = payload
        .current_units
        .get("wind_speed_10m")
        .cloned()
        .unwrap_or_else(|| "m/s".to_string());
    let unit_wind_dir = payload
        .current_units
        .get("wind_direction_10m")
        .cloned()
        .unwrap_or_else(|| "°".to_string());
    let unit_humidity = payload
        .current_units
        .get("relative_humidity_2m")
        .cloned()
        .unwrap_or_else(|| "%".to_string());
    let unit_pressure = payload
        .current_units
        .get("pressure_msl")
        .cloned()
        .unwrap_or_else(|| "hPa".to_string());

    let mut metrics = BTreeMap::new();
    if let Some(value) = current.temperature_2m {
        metrics.insert(
            forecasts::METRIC_WEATHER_TEMPERATURE_C.to_string(),
            CurrentWeatherMetric {
                unit: unit_temp,
                value,
            },
        );
    }
    if let Some(value) = current.precipitation {
        metrics.insert(
            forecasts::METRIC_WEATHER_PRECIP_MM.to_string(),
            CurrentWeatherMetric {
                unit: unit_precip,
                value: value.max(0.0),
            },
        );
    }
    if let Some(value) = current.cloudcover {
        metrics.insert(
            forecasts::METRIC_WEATHER_CLOUD_COVER_PCT.to_string(),
            CurrentWeatherMetric {
                unit: unit_cloud,
                value,
            },
        );
    }
    if let Some(value) = current.wind_speed_10m {
        metrics.insert(
            forecasts::METRIC_WEATHER_WIND_SPEED_MPS.to_string(),
            CurrentWeatherMetric {
                unit: unit_wind_speed,
                value,
            },
        );
    }
    if let Some(value) = current.wind_direction_10m {
        metrics.insert(
            forecasts::METRIC_WEATHER_WIND_DIR_DEG.to_string(),
            CurrentWeatherMetric {
                unit: unit_wind_dir,
                value,
            },
        );
    }
    if let Some(value) = current.relative_humidity_2m {
        metrics.insert(
            forecasts::METRIC_WEATHER_HUMIDITY_PCT.to_string(),
            CurrentWeatherMetric {
                unit: unit_humidity,
                value,
            },
        );
    }
    if let Some(value) = current.pressure_msl {
        let value_kpa = if unit_pressure.trim().eq_ignore_ascii_case("hPa") {
            value / 10.0
        } else if unit_pressure.trim().eq_ignore_ascii_case("Pa") {
            value / 1000.0
        } else {
            value
        };
        if value_kpa.is_finite() && value_kpa >= 50.0 && value_kpa <= 150.0 {
            metrics.insert(
                forecasts::METRIC_WEATHER_PRESSURE_MSL_KPA.to_string(),
                CurrentWeatherMetric {
                    unit: "kPa".to_string(),
                    value: value_kpa,
                },
            );
        }
    }

    let response = CurrentWeatherResponse {
        provider: "Open-Meteo".to_string(),
        latitude: payload.latitude,
        longitude: payload.longitude,
        observed_at,
        fetched_at: now.to_rfc3339(),
        metrics,
    };

    if let Err(err) = core_node::ensure_core_node(&state.db).await {
        tracing::warn!("failed to ensure core node exists for current weather: {err:#}");
    }

    if !response.metrics.is_empty() {
        let mut tx = state.db.begin().await.map_err(map_db_error)?;
        for (metric, entry) in &response.metrics {
            let _ = sqlx::query(
                r#"
                INSERT INTO forecast_points (
                    provider, kind, subject_kind, subject, latitude, longitude, issued_at, ts, metric, value, unit
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                ON CONFLICT (provider, kind, subject_kind, subject, metric, issued_at, ts)
                DO NOTHING
                "#,
            )
            .bind(forecasts::PROVIDER_OPEN_METEO)
            .bind(forecasts::KIND_WEATHER)
            .bind(subject_kind)
            .bind(&subject)
            .bind(response.latitude)
            .bind(response.longitude)
            .bind(observed_at_dt)
            .bind(observed_at_dt)
            .bind(metric)
            .bind(entry.value)
            .bind(&entry.unit)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

            let (name, sensor_type, unit, interval_seconds) = match metric.as_str() {
                forecasts::METRIC_WEATHER_TEMPERATURE_C => {
                    ("Weather temperature (°C)", "temperature", "degC", 300)
                }
                forecasts::METRIC_WEATHER_HUMIDITY_PCT => {
                    ("Weather humidity (%)", "humidity", "%", 300)
                }
                forecasts::METRIC_WEATHER_CLOUD_COVER_PCT => {
                    ("Weather cloud cover (%)", "cloud_cover", "%", 300)
                }
                forecasts::METRIC_WEATHER_WIND_SPEED_MPS => {
                    ("Weather wind speed (m/s)", "wind_speed", "m/s", 300)
                }
                forecasts::METRIC_WEATHER_WIND_DIR_DEG => {
                    ("Weather wind direction (°)", "wind_direction", "deg", 300)
                }
                forecasts::METRIC_WEATHER_PRECIP_MM => {
                    ("Weather precipitation (mm)", "precipitation", "mm", 300)
                }
                forecasts::METRIC_WEATHER_PRESSURE_MSL_KPA => {
                    ("Weather pressure (kPa)", "pressure", "kPa", 300)
                }
                _ => continue,
            };

            let sensor_key = format!("weather_current|{subject_kind}|{subject}|{metric}");
            let _ = virtual_sensors::ensure_forecast_point_sensor(
                &state.db,
                sensor_target_node_id,
                &sensor_key,
                name,
                sensor_type,
                unit,
                interval_seconds,
                forecasts::PROVIDER_OPEN_METEO,
                forecasts::KIND_WEATHER,
                subject_kind,
                &subject,
                metric,
                "latest",
            )
            .await;
        }
        tx.commit().await.map_err(map_db_error)?;
    }

    {
        let mut cache = weather_cache().write().await;
        cache.insert(
            key,
            CachedCurrentWeather {
                fetched_at: now,
                response: response.clone(),
            },
        );
    }

    Ok(Json(response))
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct PvForecastConfigResponse {
    enabled: bool,
    provider: String,
    latitude: f64,
    longitude: f64,
    tilt_deg: f64,
    azimuth_deg: f64,
    kwp: f64,
    time_format: String,
    updated_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct PvForecastConfigRequest {
    enabled: bool,
    latitude: f64,
    longitude: f64,
    tilt_deg: f64,
    azimuth_deg: f64,
    kwp: f64,
    #[serde(default)]
    time_format: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/forecast/pv/config/{node_id}",
    tag = "forecast",
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "PV forecast configuration", body = PvForecastConfigResponse),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn get_pv_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<PvForecastConfigResponse>, (StatusCode, String)> {
    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        config: SqlJson<JsonValue>,
    }
    let row: Option<Row> = sqlx::query_as(
        r#"
        SELECT COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let obj = row
        .config
        .0
        .get("pv_forecast")
        .and_then(|v| v.as_object())
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "PV forecast not configured".to_string(),
            )
        })?;

    let updated_at = obj.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

    Ok(Json(PvForecastConfigResponse {
        enabled: obj
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        provider: obj
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or(forecasts::PROVIDER_FORECAST_SOLAR)
            .to_string(),
        latitude: obj.get("latitude").and_then(|v| v.as_f64()).unwrap_or(0.0),
        longitude: obj.get("longitude").and_then(|v| v.as_f64()).unwrap_or(0.0),
        tilt_deg: obj.get("tilt_deg").and_then(|v| v.as_f64()).unwrap_or(0.0),
        azimuth_deg: obj
            .get("azimuth_deg")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        kwp: obj.get("kwp").and_then(|v| v.as_f64()).unwrap_or(0.0),
        time_format: obj
            .get("time_format")
            .and_then(|v| v.as_str())
            .unwrap_or("utc")
            .to_string(),
        updated_at: updated_at.to_string(),
    }))
}

#[utoipa::path(
    put,
    path = "/api/forecast/pv/config/{node_id}",
    tag = "forecast",
    request_body = PvForecastConfigRequest,
    params(("node_id" = String, Path, description = "Node id")),
    responses((status = 200, description = "Updated PV forecast configuration", body = PvForecastConfigResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_pv_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<PvForecastConfigRequest>,
) -> Result<Json<PvForecastConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    if !(-90.0..=90.0).contains(&payload.latitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "latitude must be -90..90".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&payload.longitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "longitude must be -180..180".to_string(),
        ));
    }
    if !(0.0..=90.0).contains(&payload.tilt_deg) {
        return Err((
            StatusCode::BAD_REQUEST,
            "tilt_deg must be 0..90".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&payload.azimuth_deg) {
        return Err((
            StatusCode::BAD_REQUEST,
            "azimuth_deg must be -180..180".to_string(),
        ));
    }
    if payload.kwp <= 0.0 {
        return Err((StatusCode::BAD_REQUEST, "kwp must be > 0".to_string()));
    }

    let time_format = payload
        .time_format
        .as_deref()
        .unwrap_or("utc")
        .trim()
        .to_lowercase();
    if time_format != "utc" && time_format != "iso8601" {
        return Err((
            StatusCode::BAD_REQUEST,
            "time_format must be 'utc' or 'iso8601'".to_string(),
        ));
    }

    let updated_at = Utc::now().to_rfc3339();
    let pv_config = serde_json::json!({
        "enabled": payload.enabled,
        "provider": forecasts::PROVIDER_FORECAST_SOLAR,
        "latitude": payload.latitude,
        "longitude": payload.longitude,
        "tilt_deg": payload.tilt_deg,
        "azimuth_deg": payload.azimuth_deg,
        "kwp": payload.kwp,
        "time_format": time_format.clone(),
        "updated_at": updated_at,
    });

    let updated: Option<(SqlJson<JsonValue>,)> = sqlx::query_as(
        r#"
        UPDATE nodes
        SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{pv_forecast}', $2::jsonb, true)
        WHERE id = $1
        RETURNING config
        "#,
    )
    .bind(node_uuid)
    .bind(SqlJson(pv_config.clone()))
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(_) = updated else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    Ok(Json(PvForecastConfigResponse {
        enabled: payload.enabled,
        provider: forecasts::PROVIDER_FORECAST_SOLAR.to_string(),
        latitude: payload.latitude,
        longitude: payload.longitude,
        tilt_deg: payload.tilt_deg,
        azimuth_deg: payload.azimuth_deg,
        kwp: payload.kwp,
        time_format,
        updated_at,
    }))
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct PvForecastCheckRequest {
    latitude: f64,
    longitude: f64,
    tilt_deg: f64,
    azimuth_deg: f64,
    kwp: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct PvForecastCheckResponse {
    status: String,
    place: Option<String>,
    timezone: Option<String>,
    checked_at: String,
}

#[derive(Debug, serde::Deserialize)]
struct ForecastSolarCheckResult {
    #[serde(default)]
    place: Option<String>,
    #[serde(default)]
    timezone: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ForecastSolarCheckMessage {
    code: i64,
    #[serde(default)]
    text: String,
}

#[derive(Debug, serde::Deserialize)]
struct ForecastSolarCheckPayload {
    #[serde(default)]
    result: Option<ForecastSolarCheckResult>,
    message: ForecastSolarCheckMessage,
}

#[utoipa::path(
    post,
    path = "/api/forecast/pv/check",
    tag = "forecast",
    request_body = PvForecastCheckRequest,
    responses(
        (status = 200, description = "Checked PV plane parameters", body = PvForecastCheckResponse),
        (status = 400, description = "Invalid parameters")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn check_pv_plane(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<PvForecastCheckRequest>,
) -> Result<Json<PvForecastCheckResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if !(-90.0..=90.0).contains(&payload.latitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "latitude must be -90..90".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&payload.longitude) {
        return Err((
            StatusCode::BAD_REQUEST,
            "longitude must be -180..180".to_string(),
        ));
    }
    if !(0.0..=90.0).contains(&payload.tilt_deg) {
        return Err((
            StatusCode::BAD_REQUEST,
            "tilt_deg must be 0..90".to_string(),
        ));
    }
    if !(-180.0..=180.0).contains(&payload.azimuth_deg) {
        return Err((
            StatusCode::BAD_REQUEST,
            "azimuth_deg must be -180..180".to_string(),
        ));
    }
    if payload.kwp <= 0.0 {
        return Err((StatusCode::BAD_REQUEST, "kwp must be > 0".to_string()));
    }

    let url = format!(
        "https://api.forecast.solar/check/{lat}/{lon}/{dec}/{az}/{kwp}",
        lat = payload.latitude,
        lon = payload.longitude,
        dec = payload.tilt_deg,
        az = payload.azimuth_deg,
        kwp = payload.kwp
    );
    let response = state
        .http
        .get(url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(map_internal_error)?;

    let status = response.status();
    let body = response.text().await.map_err(map_internal_error)?;
    if !status.is_success() {
        if let Ok(decoded) = serde_json::from_str::<ForecastSolarCheckPayload>(&body) {
            if decoded.message.text.trim().is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Forecast.Solar check failed ({status})"),
                ));
            }
            return Err((StatusCode::BAD_REQUEST, decoded.message.text));
        }
        return Err((StatusCode::BAD_REQUEST, body));
    }

    let decoded: ForecastSolarCheckPayload =
        serde_json::from_str(&body).map_err(map_internal_error)?;
    if decoded.message.code != 0 {
        let text = decoded.message.text.trim();
        return Err((
            StatusCode::BAD_REQUEST,
            if text.is_empty() {
                "Forecast.Solar check failed".to_string()
            } else {
                text.to_string()
            },
        ));
    }

    let result = decoded.result;
    Ok(Json(PvForecastCheckResponse {
        status: "ok".to_string(),
        place: result.as_ref().and_then(|r| r.place.clone()),
        timezone: result.as_ref().and_then(|r| r.timezone.clone()),
        checked_at: Utc::now().to_rfc3339(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/forecast/pv/{node_id}/hourly",
    tag = "forecast",
    params(("node_id" = String, Path, description = "Node id"), PvWindowHoursQuery),
    responses((status = 200, description = "Latest hourly PV forecast series", body = ForecastSeriesResponse))
)]
pub(crate) async fn pv_hourly(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(node_id): Path<String>,
    Query(query): Query<PvWindowHoursQuery>,
) -> Result<Json<ForecastSeriesResponse>, (StatusCode, String)> {
    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    let hours = query.hours.clamp(1, 168);
    let history_hours = query.history_hours.clamp(0, 168);
    let now = Utc::now();
    let start = now - chrono::Duration::hours(history_hours as i64);
    let end = now + chrono::Duration::hours(hours as i64);

    let series = if history_hours > 0 {
        load_latest_asof_series(
            &state,
            forecasts::PROVIDER_FORECAST_SOLAR,
            forecasts::KIND_PV,
            forecasts::SUBJECT_KIND_NODE,
            &node_uuid.to_string(),
            &[forecasts::METRIC_PV_POWER_W],
            start,
            end,
        )
        .await?
    } else {
        load_latest_series(
            &state,
            forecasts::PROVIDER_FORECAST_SOLAR,
            forecasts::KIND_PV,
            forecasts::SUBJECT_KIND_NODE,
            &node_uuid.to_string(),
            &[forecasts::METRIC_PV_POWER_W],
            start,
            end,
        )
        .await?
    };
    Ok(Json(series))
}

#[utoipa::path(
    get,
    path = "/api/forecast/pv/{node_id}/daily",
    tag = "forecast",
    params(("node_id" = String, Path, description = "Node id"), WindowDaysQuery),
    responses((status = 200, description = "Latest daily PV forecast series", body = ForecastSeriesResponse))
)]
pub(crate) async fn pv_daily(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(node_id): Path<String>,
    Query(query): Query<WindowDaysQuery>,
) -> Result<Json<ForecastSeriesResponse>, (StatusCode, String)> {
    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    let days = query.days.clamp(1, 14);
    let now = Utc::now();
    let today = NaiveDate::from_ymd_opt(now.year(), now.month(), now.day()).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "invalid date".to_string(),
        )
    })?;
    let start = Utc.from_utc_datetime(&today.and_hms_opt(0, 0, 0).unwrap());
    let end = start + chrono::Duration::days(days as i64);
    let series = load_latest_series(
        &state,
        forecasts::PROVIDER_FORECAST_SOLAR,
        forecasts::KIND_PV,
        forecasts::SUBJECT_KIND_NODE,
        &node_uuid.to_string(),
        &[forecasts::METRIC_PV_ENERGY_DAY_WH],
        start,
        end,
    )
    .await?;
    Ok(Json(series))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/forecast", get(latest_forecast))
        .route("/forecast/latest", get(latest_forecast_value))
        .route("/forecast/status", get(forecast_status))
        .route("/forecast/poll", post(poll_forecast))
        .route("/forecast/ingest", post(ingest_forecast))
        .route(
            "/forecast/weather/config",
            get(get_weather_config).put(update_weather_config),
        )
        .route("/forecast/weather/current", get(weather_current))
        .route("/forecast/weather/hourly", get(weather_hourly))
        .route("/forecast/weather/daily", get(weather_daily))
        .route("/forecast/pv/check", post(check_pv_plane))
        .route(
            "/forecast/pv/config/{node_id}",
            get(get_pv_config).put(update_pv_config),
        )
        .route("/forecast/pv/{node_id}/hourly", get(pv_hourly))
        .route("/forecast/pv/{node_id}/daily", get(pv_daily))
}
