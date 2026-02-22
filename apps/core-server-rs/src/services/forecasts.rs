use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::core_node;
use crate::services::virtual_sensors;
use crate::state::AppState;

pub const PROVIDER_FORECAST_SOLAR: &str = "forecast_solar";
pub const PROVIDER_OPEN_METEO: &str = "open_meteo";

pub const KIND_PV: &str = "pv";
pub const KIND_WEATHER: &str = "weather";

pub const SUBJECT_KIND_NODE: &str = "node";
pub const SUBJECT_KIND_LOCATION: &str = "location";
pub const SUBJECT_CONTROLLER: &str = "controller";

pub const METRIC_PV_POWER_W: &str = "pv_power_w";
pub const METRIC_PV_ENERGY_DAY_WH: &str = "pv_energy_day_wh";

pub const METRIC_WEATHER_TEMPERATURE_C: &str = "temperature_c";
pub const METRIC_WEATHER_HUMIDITY_PCT: &str = "humidity_pct";
pub const METRIC_WEATHER_PRECIP_MM: &str = "precipitation_mm";
pub const METRIC_WEATHER_WIND_SPEED_MPS: &str = "wind_speed_mps";
pub const METRIC_WEATHER_WIND_DIR_DEG: &str = "wind_direction_deg";
pub const METRIC_WEATHER_CLOUD_COVER_PCT: &str = "cloud_cover_pct";
pub const METRIC_WEATHER_PRESSURE_MSL_KPA: &str = "pressure_msl_kpa";
pub const METRIC_WEATHER_CLOUD_COVER_MEAN_PCT: &str = "cloud_cover_mean_pct";
pub const METRIC_WEATHER_TEMPERATURE_MAX_C: &str = "temperature_max_c";
pub const METRIC_WEATHER_TEMPERATURE_MIN_C: &str = "temperature_min_c";
pub const METRIC_WEATHER_PRECIP_SUM_MM: &str = "precipitation_sum_mm";
pub const METRIC_WEATHER_WIND_GUST_MPS: &str = "wind_gust_mps";

const WEATHER_CONFIG_NAME: &str = "weather_forecast";

#[derive(Debug)]
pub struct ForecastPollResult {
    pub name: String,
    pub status: String,
}

pub struct ForecastService {
    state: AppState,
    interval: Duration,
}

impl ForecastService {
    pub fn new(state: AppState, interval: Duration) -> Self {
        Self { state, interval }
    }

    pub fn start(self, cancel: CancellationToken) {
        let state = self.state.clone();
        let interval = self.interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        if let Err(err) = poll_all_forecasts(&state).await {
                            tracing::warn!("forecast poll failed: {err:#}");
                        }
                    }
                }
            }
        });
    }
}

#[derive(Debug, Clone)]
pub struct WeatherForecastConfig {
    pub enabled: bool,
    pub provider: String,
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(sqlx::FromRow)]
struct SetupCredentialRow {
    value: String,
    metadata: SqlJson<JsonValue>,
}

#[derive(sqlx::FromRow)]
struct NodeConfigRow {
    id: Uuid,
    name: String,
    config: SqlJson<JsonValue>,
}

#[derive(Debug, Deserialize)]
struct ForecastSolarEstimateResponse {
    result: ForecastSolarEstimateResult,
    #[serde(default)]
    message: JsonValue,
}

#[derive(Debug, Deserialize)]
struct ForecastSolarEstimateResult {
    #[serde(default)]
    watts: BTreeMap<String, f64>,
    #[serde(default)]
    watt_hours_day: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
    #[serde(default)]
    hourly_units: BTreeMap<String, String>,
    #[serde(default)]
    daily_units: BTreeMap<String, String>,
    hourly: Option<OpenMeteoHourly>,
    daily: Option<OpenMeteoDaily>,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoHourly {
    time: Vec<String>,
    #[serde(default)]
    temperature_2m: Vec<f64>,
    #[serde(default)]
    relative_humidity_2m: Vec<f64>,
    #[serde(default)]
    precipitation: Vec<f64>,
    #[serde(default)]
    wind_speed_10m: Vec<f64>,
    #[serde(default)]
    wind_direction_10m: Vec<f64>,
    #[serde(default)]
    cloudcover: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoDaily {
    time: Vec<String>,
    #[serde(default)]
    temperature_2m_max: Vec<f64>,
    #[serde(default)]
    temperature_2m_min: Vec<f64>,
    #[serde(default)]
    precipitation_sum: Vec<f64>,
    #[serde(default)]
    cloudcover_mean: Vec<f64>,
}

pub async fn poll_all_forecasts(state: &AppState) -> Result<Vec<ForecastPollResult>> {
    let mut results = Vec::new();
    results.push(poll_open_meteo(state).await?);
    results.push(poll_forecast_solar(state).await?);
    if let Err(err) = poll_open_meteo_current(state).await {
        tracing::warn!("open-meteo current poll failed: {err:#}");
    }
    Ok(results)
}

#[derive(Debug, Deserialize)]
struct OpenMeteoCurrentPayload {
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    current_units: BTreeMap<String, String>,
    current: Option<OpenMeteoCurrentBlock>,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoCurrentBlock {
    time: String,
    #[serde(default)]
    temperature_2m: Option<f64>,
    #[serde(default)]
    precipitation: Option<f64>,
    #[serde(default)]
    wind_speed_10m: Option<f64>,
    #[serde(default)]
    wind_direction_10m: Option<f64>,
    #[serde(default)]
    wind_gusts_10m: Option<f64>,
    #[serde(default)]
    cloudcover: Option<f64>,
    #[serde(default)]
    relative_humidity_2m: Option<f64>,
    #[serde(default)]
    pressure_msl: Option<f64>,
}

#[derive(Debug, Clone)]
struct WeatherLocationTarget {
    sensor_target_node_id: Uuid,
    subject_kind: &'static str,
    subject: String,
    latitude: f64,
    longitude: f64,
}

fn parse_open_meteo_current_time(time: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(time) {
        return Some(parsed.with_timezone(&Utc));
    }
    if let Ok(parsed) = NaiveDateTime::parse_from_str(time, "%Y-%m-%dT%H:%M") {
        return Some(DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc));
    }
    None
}

fn geojson_point_to_lat_lng(geometry: &JsonValue) -> Option<(f64, f64)> {
    let obj = geometry.as_object()?;
    let geo_type = obj.get("type")?.as_str()?;
    if geo_type != "Point" {
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

async fn load_weather_targets(state: &AppState) -> Result<Vec<WeatherLocationTarget>> {
    let mut targets = Vec::new();

    if let Some(cfg) = load_weather_config(state).await? {
        targets.push(WeatherLocationTarget {
            sensor_target_node_id: core_node::CORE_NODE_ID,
            subject_kind: SUBJECT_KIND_LOCATION,
            subject: SUBJECT_CONTROLLER.to_string(),
            latitude: cfg.latitude,
            longitude: cfg.longitude,
        });
    }

    let rows: Vec<(Uuid, SqlJson<JsonValue>)> = sqlx::query_as(
        r#"
        SELECT mf.node_id, mf.geometry
        FROM map_features mf
        JOIN map_settings ms ON ms.singleton = TRUE
        JOIN nodes n ON n.id = mf.node_id
        WHERE mf.save_id = ms.active_save_id
          AND mf.node_id IS NOT NULL
          AND n.status <> 'deleted'
          AND NOT (COALESCE(n.config, '{}'::jsonb) @> '{"deleted": true}')
          AND NOT (COALESCE(n.config, '{}'::jsonb) @> '{"poll_enabled": false}')
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    for (node_id, geometry) in rows {
        let Some((lat, lng)) = geojson_point_to_lat_lng(&geometry.0) else {
            continue;
        };
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lng) {
            continue;
        }
        targets.push(WeatherLocationTarget {
            sensor_target_node_id: node_id,
            subject_kind: SUBJECT_KIND_NODE,
            subject: node_id.to_string(),
            latitude: lat,
            longitude: lng,
        });
    }

    Ok(targets)
}

async fn poll_open_meteo_current(state: &AppState) -> Result<()> {
    let targets = load_weather_targets(state).await?;
    if targets.is_empty() {
        return Ok(());
    }

    if let Err(err) = core_node::ensure_core_node(&state.db).await {
        tracing::warn!("failed to ensure core node exists for current weather poll: {err:#}");
    }

    for target in targets {
        let response = state
            .http
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude", target.latitude.to_string()),
                ("longitude", target.longitude.to_string()),
                (
                    "current",
                    "temperature_2m,precipitation,wind_speed_10m,wind_direction_10m,wind_gusts_10m,cloudcover,relative_humidity_2m,pressure_msl"
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
            .context("Open-Meteo current request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::warn!("Open-Meteo current HTTP {status}: {body}");
            continue;
        }

        let payload: OpenMeteoCurrentPayload = response
            .json()
            .await
            .context("Open-Meteo current decode failed")?;
        let Some(current) = payload.current else {
            continue;
        };

        let observed_at_dt = parse_open_meteo_current_time(&current.time).unwrap_or_else(Utc::now);

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
        let unit_wind_gust = payload
            .current_units
            .get("wind_gusts_10m")
            .cloned()
            .unwrap_or_else(|| "m/s".to_string());
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

        let mut points: Vec<(&'static str, f64, String)> = Vec::new();
        if let Some(value) = current.temperature_2m {
            points.push((METRIC_WEATHER_TEMPERATURE_C, value, unit_temp));
        }
        if let Some(value) = current.precipitation {
            points.push((METRIC_WEATHER_PRECIP_MM, value.max(0.0), unit_precip));
        }
        if let Some(value) = current.cloudcover {
            points.push((METRIC_WEATHER_CLOUD_COVER_PCT, value, unit_cloud));
        }
        if let Some(value) = current.wind_speed_10m {
            points.push((METRIC_WEATHER_WIND_SPEED_MPS, value, unit_wind_speed));
        }
        if let Some(value) = current.wind_direction_10m {
            points.push((METRIC_WEATHER_WIND_DIR_DEG, value, unit_wind_dir));
        }
        if let Some(value) = current.wind_gusts_10m {
            points.push((METRIC_WEATHER_WIND_GUST_MPS, value, unit_wind_gust));
        }
        if let Some(value) = current.relative_humidity_2m {
            points.push((METRIC_WEATHER_HUMIDITY_PCT, value, unit_humidity));
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
                points.push((
                    METRIC_WEATHER_PRESSURE_MSL_KPA,
                    value_kpa,
                    "kPa".to_string(),
                ));
            }
        }

        if points.is_empty() {
            continue;
        }

        let mut tx = state.db.begin().await?;
        for (metric, value, unit) in points {
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
            .bind(PROVIDER_OPEN_METEO)
            .bind(KIND_WEATHER)
            .bind(target.subject_kind)
            .bind(&target.subject)
            .bind(payload.latitude)
            .bind(payload.longitude)
            .bind(observed_at_dt)
            .bind(observed_at_dt)
            .bind(metric)
            .bind(value)
            .bind(&unit)
            .execute(&mut *tx)
            .await?;

            let (name, sensor_type, sensor_unit) = match metric {
                METRIC_WEATHER_TEMPERATURE_C => ("Weather temperature (°C)", "temperature", "degC"),
                METRIC_WEATHER_HUMIDITY_PCT => ("Weather humidity (%)", "humidity", "%"),
                METRIC_WEATHER_CLOUD_COVER_PCT => ("Weather cloud cover (%)", "cloud_cover", "%"),
                METRIC_WEATHER_WIND_SPEED_MPS => ("Weather wind speed (m/s)", "wind_speed", "m/s"),
                METRIC_WEATHER_WIND_GUST_MPS => ("Weather wind gust (m/s)", "wind_gust", "m/s"),
                METRIC_WEATHER_WIND_DIR_DEG => {
                    ("Weather wind direction (°)", "wind_direction", "deg")
                }
                METRIC_WEATHER_PRECIP_MM => ("Weather precipitation (mm)", "precipitation", "mm"),
                METRIC_WEATHER_PRESSURE_MSL_KPA => ("Weather pressure (kPa)", "pressure", "kPa"),
                _ => continue,
            };

            let sensor_key = format!(
                "weather_current|{}|{}|{metric}",
                target.subject_kind, target.subject
            );
            let _ = virtual_sensors::ensure_forecast_point_sensor(
                &state.db,
                target.sensor_target_node_id,
                &sensor_key,
                name,
                sensor_type,
                sensor_unit,
                300,
                PROVIDER_OPEN_METEO,
                KIND_WEATHER,
                target.subject_kind,
                &target.subject,
                metric,
                "latest",
            )
            .await;
        }
        tx.commit().await?;
    }

    Ok(())
}

pub async fn load_weather_config(state: &AppState) -> Result<Option<WeatherForecastConfig>> {
    let row: Option<SetupCredentialRow> = sqlx::query_as(
        r#"
        SELECT value, metadata
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind(WEATHER_CONFIG_NAME)
    .fetch_optional(&state.db)
    .await
    .context("failed to query weather forecast settings")?;

    let Some(row) = row else {
        return Ok(None);
    };

    let provider = row.value.trim().to_string();
    let meta = row.metadata.0;
    let enabled = meta
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let latitude = meta.get("latitude").and_then(|v| v.as_f64());
    let longitude = meta.get("longitude").and_then(|v| v.as_f64());

    let (Some(latitude), Some(longitude)) = (latitude, longitude) else {
        return Ok(None);
    };
    if provider.is_empty() {
        return Ok(None);
    }

    Ok(Some(WeatherForecastConfig {
        enabled,
        provider,
        latitude,
        longitude,
    }))
}

async fn poll_open_meteo(state: &AppState) -> Result<ForecastPollResult> {
    let Some(config) = load_weather_config(state).await? else {
        return Ok(ForecastPollResult {
            name: "Open-Meteo".to_string(),
            status: "missing".to_string(),
        });
    };

    if !config.enabled {
        persist_status(
            state,
            "Open-Meteo",
            "disabled",
            &json!({ "detail": "weather forecast disabled" }),
        )
        .await?;
        return Ok(ForecastPollResult {
            name: "Open-Meteo".to_string(),
            status: "disabled".to_string(),
        });
    }

    if config.provider != PROVIDER_OPEN_METEO {
        persist_status(
            state,
            "Open-Meteo",
            "error",
            &json!({ "detail": format!("unsupported weather provider {}", config.provider) }),
        )
        .await?;
        return Ok(ForecastPollResult {
            name: "Open-Meteo".to_string(),
            status: "error".to_string(),
        });
    }

    let url = "https://api.open-meteo.com/v1/forecast";
    let response = state
        .http
        .get(url)
        .query(&[
            ("latitude", config.latitude.to_string()),
            ("longitude", config.longitude.to_string()),
            (
                "hourly",
                "temperature_2m,relative_humidity_2m,precipitation,wind_speed_10m,wind_direction_10m,cloudcover"
                    .to_string(),
            ),
            (
                "daily",
                "temperature_2m_max,temperature_2m_min,precipitation_sum,cloudcover_mean".to_string(),
            ),
            ("timezone", "UTC".to_string()),
            ("forecast_days", "14".to_string()),
            ("temperature_unit", "celsius".to_string()),
            ("wind_speed_unit", "ms".to_string()),
            ("precipitation_unit", "mm".to_string()),
        ])
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .context("Open-Meteo request failed")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let meta = json!({ "detail": format!("Open-Meteo HTTP {status}"), "body": body });
        persist_status(state, "Open-Meteo", "error", &meta).await?;
        return Ok(ForecastPollResult {
            name: "Open-Meteo".to_string(),
            status: "error".to_string(),
        });
    }

    let payload: OpenMeteoResponse = response
        .json()
        .await
        .context("failed to decode Open-Meteo response")?;

    let issued_at = Utc::now();
    let unit_temp = payload
        .hourly_units
        .get("temperature_2m")
        .cloned()
        .unwrap_or_else(|| "°C".to_string());
    let unit_humidity = payload
        .hourly_units
        .get("relative_humidity_2m")
        .cloned()
        .unwrap_or_else(|| "%".to_string());
    let unit_precip = payload
        .hourly_units
        .get("precipitation")
        .cloned()
        .unwrap_or_else(|| "mm".to_string());
    let unit_wind_speed = payload
        .hourly_units
        .get("wind_speed_10m")
        .cloned()
        .unwrap_or_else(|| "m/s".to_string());
    let unit_wind_dir = payload
        .hourly_units
        .get("wind_direction_10m")
        .cloned()
        .unwrap_or_else(|| "°".to_string());
    let unit_cloud = payload
        .hourly_units
        .get("cloudcover")
        .cloned()
        .unwrap_or_else(|| "%".to_string());

    let unit_temp_max = payload
        .daily_units
        .get("temperature_2m_max")
        .cloned()
        .unwrap_or_else(|| "°C".to_string());
    let unit_temp_min = payload
        .daily_units
        .get("temperature_2m_min")
        .cloned()
        .unwrap_or_else(|| "°C".to_string());
    let unit_precip_sum = payload
        .daily_units
        .get("precipitation_sum")
        .cloned()
        .unwrap_or_else(|| "mm".to_string());
    let unit_cloud_mean = payload
        .daily_units
        .get("cloudcover_mean")
        .cloned()
        .unwrap_or_else(|| "%".to_string());

    let mut tx = state
        .db
        .begin()
        .await
        .context("failed to begin forecast tx")?;

    if let Some(hourly) = payload.hourly {
        let len = hourly.time.len();
        for idx in 0..len {
            let Some(ts) = parse_open_meteo_hour(&hourly.time[idx]) else {
                continue;
            };
            if let Some(value) = hourly.temperature_2m.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_TEMPERATURE_C,
                    value,
                    &unit_temp,
                )
                .await?;
            }
            if let Some(value) = hourly.relative_humidity_2m.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_HUMIDITY_PCT,
                    value,
                    &unit_humidity,
                )
                .await?;
            }
            if let Some(value) = hourly.precipitation.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_PRECIP_MM,
                    value.max(0.0),
                    &unit_precip,
                )
                .await?;
            }
            if let Some(value) = hourly.wind_speed_10m.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_WIND_SPEED_MPS,
                    value,
                    &unit_wind_speed,
                )
                .await?;
            }
            if let Some(value) = hourly.wind_direction_10m.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_WIND_DIR_DEG,
                    value,
                    &unit_wind_dir,
                )
                .await?;
            }
            if let Some(value) = hourly.cloudcover.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_CLOUD_COVER_PCT,
                    value,
                    &unit_cloud,
                )
                .await?;
            }
        }
    }

    if let Some(daily) = payload.daily {
        let len = daily.time.len();
        for idx in 0..len {
            let Some(ts) = parse_open_meteo_day(&daily.time[idx]) else {
                continue;
            };
            if let Some(value) = daily.temperature_2m_max.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_TEMPERATURE_MAX_C,
                    value,
                    &unit_temp_max,
                )
                .await?;
            }
            if let Some(value) = daily.temperature_2m_min.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_TEMPERATURE_MIN_C,
                    value,
                    &unit_temp_min,
                )
                .await?;
            }
            if let Some(value) = daily.precipitation_sum.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_PRECIP_SUM_MM,
                    value.max(0.0),
                    &unit_precip_sum,
                )
                .await?;
            }
            if let Some(value) = daily.cloudcover_mean.get(idx).copied() {
                insert_point(
                    &mut tx,
                    PROVIDER_OPEN_METEO,
                    KIND_WEATHER,
                    SUBJECT_KIND_LOCATION,
                    SUBJECT_CONTROLLER,
                    config.latitude,
                    config.longitude,
                    issued_at,
                    ts,
                    METRIC_WEATHER_CLOUD_COVER_MEAN_PCT,
                    value,
                    &unit_cloud_mean,
                )
                .await?;
            }
        }
    }

    tx.commit().await.context("failed to commit forecast tx")?;

    persist_status(
        state,
        "Open-Meteo",
        "ok",
        &json!({
            "detail": "weather forecast ingested",
            "latitude": config.latitude,
            "longitude": config.longitude,
            "issued_at": issued_at.to_rfc3339(),
        }),
    )
    .await?;

    Ok(ForecastPollResult {
        name: "Open-Meteo".to_string(),
        status: "ok".to_string(),
    })
}

async fn poll_forecast_solar(state: &AppState) -> Result<ForecastPollResult> {
    let nodes: Vec<NodeConfigRow> = sqlx::query_as(
        r#"
        SELECT id, name, COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE COALESCE(config, '{}'::jsonb) ? 'pv_forecast'
          AND status <> 'deleted'
          AND NOT (COALESCE(config, '{}'::jsonb) @> '{"deleted": true}')
          AND NOT (COALESCE(config, '{}'::jsonb) @> '{"poll_enabled": false}')
        ORDER BY name
        "#,
    )
    .fetch_all(&state.db)
    .await
    .context("failed to load nodes for PV forecast")?;

    let mut configured = Vec::new();
    for row in nodes {
        if let Some(cfg) = NodePvForecastConfig::from_node_config(row.id, &row.config.0) {
            if cfg.enabled {
                configured.push((row.id, row.name, cfg));
            }
        }
    }

    if configured.is_empty() {
        return Ok(ForecastPollResult {
            name: "Forecast.Solar".to_string(),
            status: "missing".to_string(),
        });
    }

    let mut total_points: usize = 0;
    let mut failures: Vec<JsonValue> = Vec::new();
    let mut ratelimit: Option<JsonValue> = None;

    for (node_id, node_name, cfg) in configured {
        match poll_one_forecast_solar(state, node_id, &cfg).await {
            Ok(outcome) => {
                total_points += outcome.points;
                if outcome.ratelimit.is_some() {
                    ratelimit = outcome.ratelimit;
                }
            }
            Err(err) => failures.push(json!({ "node": node_name, "error": err.to_string() })),
        }
    }

    let status = if failures.is_empty() { "ok" } else { "error" };
    persist_status(
        state,
        "Forecast.Solar",
        status,
        &json!({
            "detail": if failures.is_empty() { "pv forecast ingested" } else { "pv forecast ingest errors" },
            "points": total_points,
            "ratelimit": ratelimit,
            "failures": failures,
        }),
    )
    .await?;

    Ok(ForecastPollResult {
        name: "Forecast.Solar".to_string(),
        status: status.to_string(),
    })
}

#[derive(Debug, Clone)]
struct NodePvForecastConfig {
    enabled: bool,
    latitude: f64,
    longitude: f64,
    tilt_deg: f64,
    azimuth_deg: f64,
    kwp: f64,
    time_format: String,
}

impl NodePvForecastConfig {
    fn from_node_config(node_id: Uuid, value: &JsonValue) -> Option<Self> {
        let obj = value.get("pv_forecast")?;
        let enabled = obj
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let provider = obj
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or(PROVIDER_FORECAST_SOLAR);
        if provider != PROVIDER_FORECAST_SOLAR {
            tracing::warn!(%node_id, provider, "unsupported pv_forecast provider");
            return None;
        }
        let time_format = obj
            .get("time_format")
            .and_then(|v| v.as_str())
            .unwrap_or("utc")
            .trim()
            .to_lowercase();
        let time_format = match time_format.as_str() {
            "utc" | "iso8601" => time_format,
            other => {
                tracing::warn!(%node_id, time_format = other, "unsupported pv_forecast time_format, defaulting to utc");
                "utc".to_string()
            }
        };
        Some(Self {
            enabled,
            latitude: obj.get("latitude").and_then(|v| v.as_f64())?,
            longitude: obj.get("longitude").and_then(|v| v.as_f64())?,
            tilt_deg: obj.get("tilt_deg").and_then(|v| v.as_f64())?,
            azimuth_deg: obj.get("azimuth_deg").and_then(|v| v.as_f64())?,
            kwp: obj.get("kwp").and_then(|v| v.as_f64())?,
            time_format,
        })
    }
}

#[derive(Debug)]
struct ForecastSolarPollOutcome {
    points: usize,
    ratelimit: Option<JsonValue>,
}

async fn poll_one_forecast_solar(
    state: &AppState,
    node_id: Uuid,
    cfg: &NodePvForecastConfig,
) -> Result<ForecastSolarPollOutcome> {
    let url = format!(
        "https://api.forecast.solar/estimate/{lat}/{lon}/{dec}/{az}/{kwp}",
        lat = cfg.latitude,
        lon = cfg.longitude,
        dec = cfg.tilt_deg,
        az = cfg.azimuth_deg,
        kwp = cfg.kwp
    );
    let response = state
        .http
        .get(url)
        .query(&[("time", cfg.time_format.as_str())])
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .context("Forecast.Solar request failed")?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Forecast.Solar HTTP {status}: {body}");
    }

    let payload: ForecastSolarEstimateResponse = response
        .json()
        .await
        .context("failed to decode Forecast.Solar response")?;

    let ratelimit = payload
        .message
        .get("ratelimit")
        .cloned()
        .filter(|value| !value.is_null());

    let issued_at = payload
        .message
        .get("info")
        .and_then(|v| v.get("time_utc"))
        .and_then(|v| v.as_str())
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let subject = node_id.to_string();

    let _ = virtual_sensors::ensure_forecast_point_sensor(
        &state.db,
        node_id,
        &format!("pv_forecast_power_w|{node_id}"),
        "PV forecast (W)",
        "power",
        "W",
        900,
        PROVIDER_FORECAST_SOLAR,
        KIND_PV,
        SUBJECT_KIND_NODE,
        &subject,
        METRIC_PV_POWER_W,
        "asof",
    )
    .await;

    let mut points: usize = 0;
    let mut tx = state
        .db
        .begin()
        .await
        .context("failed to begin pv forecast tx")?;

    for (key, value) in payload.result.watts {
        if let Ok(ts) = DateTime::parse_from_rfc3339(&key).map(|dt| dt.with_timezone(&Utc)) {
            insert_point(
                &mut tx,
                PROVIDER_FORECAST_SOLAR,
                KIND_PV,
                SUBJECT_KIND_NODE,
                &subject,
                cfg.latitude,
                cfg.longitude,
                issued_at,
                ts,
                METRIC_PV_POWER_W,
                value,
                "W",
            )
            .await?;
            points += 1;
        }
    }

    for (key, value) in payload.result.watt_hours_day {
        let Ok(date) = NaiveDate::parse_from_str(&key, "%Y-%m-%d") else {
            continue;
        };
        let ts = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap());
        insert_point(
            &mut tx,
            PROVIDER_FORECAST_SOLAR,
            KIND_PV,
            SUBJECT_KIND_NODE,
            &subject,
            cfg.latitude,
            cfg.longitude,
            issued_at,
            ts,
            METRIC_PV_ENERGY_DAY_WH,
            value,
            "Wh",
        )
        .await?;
        points += 1;
    }

    tx.commit()
        .await
        .context("failed to commit pv forecast tx")?;
    Ok(ForecastSolarPollOutcome { points, ratelimit })
}

async fn insert_point(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    provider: &str,
    kind: &str,
    subject_kind: &str,
    subject: &str,
    latitude: f64,
    longitude: f64,
    issued_at: DateTime<Utc>,
    ts: DateTime<Utc>,
    metric: &str,
    value: f64,
    unit: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO forecast_points (
            provider, kind, subject_kind, subject, latitude, longitude, issued_at, ts, metric, value, unit
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        ON CONFLICT (provider, kind, subject_kind, subject, metric, issued_at, ts)
        DO NOTHING
        "#,
    )
    .bind(provider)
    .bind(kind)
    .bind(subject_kind)
    .bind(subject)
    .bind(latitude)
    .bind(longitude)
    .bind(issued_at)
    .bind(ts)
    .bind(metric)
    .bind(value)
    .bind(unit)
    .execute(&mut **tx)
    .await
    .context("failed to insert forecast point")?;
    Ok(())
}

fn parse_open_meteo_hour(raw: &str) -> Option<DateTime<Utc>> {
    let naive = NaiveDateTime::parse_from_str(raw.trim(), "%Y-%m-%dT%H:%M").ok()?;
    Some(Utc.from_utc_datetime(&naive))
}

fn parse_open_meteo_day(raw: &str) -> Option<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()?;
    Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0)?))
}

async fn persist_status(
    state: &AppState,
    name: &str,
    status: &str,
    meta: &JsonValue,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO analytics_integration_status (category, name, status, recorded_at, metadata)
        VALUES ($1, $2, $3, NOW(), $4)
        "#,
    )
    .bind("forecast")
    .bind(name)
    .bind(status)
    .bind(SqlJson(meta.clone()))
    .execute(&state.db)
    .await
    .context("failed to persist forecast integration status")?;
    Ok(())
}
