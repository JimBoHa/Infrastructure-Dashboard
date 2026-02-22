use axum::extract::{Path, RawQuery};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration as ChronoDuration, NaiveDateTime, Utc};
use rand::RngCore;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use url::form_urlencoded;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::presets;
use crate::state::AppState;

const WS_2902_KIND: &str = "ws-2902";

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Ws2902Protocol {
    Wunderground,
    Ambient,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct Ws2902CreateRequest {
    pub(crate) nickname: String,
    pub(crate) protocol: Ws2902Protocol,
    pub(crate) interval_seconds: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct Ws2902CreatedSensor {
    pub(crate) sensor_id: String,
    pub(crate) name: String,
    #[serde(rename = "type")]
    pub(crate) sensor_type: String,
    pub(crate) unit: String,
    pub(crate) interval_seconds: i32,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct Ws2902CreateResponse {
    pub(crate) id: String,
    pub(crate) node_id: String,
    pub(crate) nickname: String,
    pub(crate) protocol: Ws2902Protocol,
    pub(crate) enabled: bool,
    pub(crate) ingest_path: String,
    pub(crate) token: String,
    pub(crate) created_at: String,
    pub(crate) sensors: Vec<Ws2902CreatedSensor>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct Ws2902UpdateRequest {
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct Ws2902StatusResponse {
    pub(crate) id: String,
    pub(crate) node_id: String,
    pub(crate) nickname: String,
    pub(crate) protocol: Ws2902Protocol,
    pub(crate) enabled: bool,
    pub(crate) ingest_path_template: String,
    pub(crate) created_at: String,
    pub(crate) rotated_at: Option<String>,
    pub(crate) last_seen: Option<String>,
    pub(crate) last_missing_fields: Vec<String>,
    pub(crate) last_payload: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct Ws2902RotateTokenResponse {
    pub(crate) id: String,
    pub(crate) ingest_path: String,
    pub(crate) token: String,
    pub(crate) rotated_at: String,
}

#[derive(sqlx::FromRow)]
struct IntegrationRow {
    id: Uuid,
    node_id: Uuid,
    nickname: String,
    protocol: String,
    enabled: bool,
    created_at: DateTime<Utc>,
    rotated_at: Option<DateTime<Utc>>,
    last_seen: Option<DateTime<Utc>>,
    last_missing_fields: SqlJson<JsonValue>,
    last_payload: SqlJson<JsonValue>,
}

#[derive(Debug, Clone)]
struct WeatherReadings {
    temp_c: Option<f64>,
    humidity_percent: Option<f64>,
    wind_speed_mps: Option<f64>,
    wind_gust_mps: Option<f64>,
    wind_dir_deg: Option<f64>,
    rain_daily_mm: Option<f64>,
    rain_rate_mm_per_hour: Option<f64>,
    uv_index: Option<f64>,
    solar_radiation_wm2: Option<f64>,
    pressure_relative_kpa: Option<f64>,
    pressure_absolute_kpa: Option<f64>,
}

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    hex_encode(&digest)
}

fn token_hash(token: &str) -> String {
    sha256_hex(token.as_bytes())
}

fn generate_token() -> String {
    let mut bytes = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex_encode(&bytes)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

fn ws_sensor_id(node_id: Uuid, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(node_id.as_bytes());
    hasher.update(b":");
    hasher.update(key.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(24);
    for byte in digest.iter().take(12) {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

fn ws_sensors() -> Vec<Ws2902CreatedSensor> {
    let interval_seconds = presets::ws_2902_default_interval_seconds();
    presets::ws_2902_sensors()
        .iter()
        .map(|sensor| Ws2902CreatedSensor {
            sensor_id: String::new(),
            name: sensor.name.clone(),
            sensor_type: sensor.sensor_type.clone(),
            unit: sensor.unit.clone(),
            interval_seconds,
        })
        .collect()
}

fn ingest_path(token: &str) -> String {
    format!("/api/ws/{token}")
}

fn ingest_path_template() -> String {
    "/api/ws/{token}".to_string()
}

fn parse_params(raw: Option<String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(raw) = raw {
        for (key, value) in form_urlencoded::parse(raw.as_bytes()) {
            out.insert(key.into_owned(), value.into_owned());
        }
    }
    out
}

fn is_sentinel_missing(value: f64) -> bool {
    // Many WS-2902-class uploads use sentinel values (often -9999) for "missing".
    // Treat absurd magnitudes as missing to avoid persisting garbage telemetry.
    value.is_nan() || value.is_infinite() || value <= -9990.0 || value >= 9990.0
}

fn config_f64(config: &JsonValue, key: &str) -> Option<f64> {
    let value = config.get(key)?;
    match value {
        JsonValue::Number(num) => num.as_f64(),
        JsonValue::String(raw) => raw.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_f64(params: &BTreeMap<String, String>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(raw) = params.get(*key) {
            if let Ok(value) = raw.trim().parse::<f64>() {
                if is_sentinel_missing(value) {
                    continue;
                }
                return Some(value);
            }
        }
    }
    None
}

fn normalize_token_and_params(
    token: &str,
    raw: Option<String>,
) -> (String, BTreeMap<String, String>) {
    let token = token.trim();
    let mut embedded_query = String::new();
    let mut token_only = token.to_string();

    // Some station firmwares append WU-style key/value pairs directly to the configured path
    // without inserting a '?' (so the request arrives as `/api/ws/<token>ID=...&tempf=...`).
    // If there is no raw querystring and the captured path segment contains query delimiters,
    // attempt to split a leading hex token from the appended key/value payload.
    let raw_query_empty = raw.as_deref().map(str::trim).unwrap_or("").is_empty();
    if raw_query_empty && (token.contains('=') || token.contains('&')) {
        if let Some(pos) = token.find(|ch: char| !ch.is_ascii_hexdigit()) {
            let (prefix, suffix) = token.split_at(pos);
            if prefix.len() >= 8 && suffix.contains('=') {
                token_only = prefix.to_string();
                embedded_query = suffix
                    .trim_start_matches('?')
                    .trim_start_matches('&')
                    .to_string();
            }
        }
    }

    let combined = match (raw, embedded_query.is_empty()) {
        (Some(raw), false) if !raw.trim().is_empty() => Some(format!("{raw}&{embedded_query}")),
        (Some(raw), _) if !raw.trim().is_empty() => Some(raw),
        (_, false) => Some(embedded_query),
        _ => None,
    };

    (token_only, parse_params(combined))
}

fn f_to_c(value: f64) -> f64 {
    (value - 32.0) * (5.0 / 9.0)
}

fn mph_to_mps(value: f64) -> f64 {
    value * 0.447_04
}

fn inches_to_mm(value: f64) -> f64 {
    value * 25.4
}

fn inhg_to_kpa(value: f64) -> f64 {
    value * 3.386_389
}

fn clamp_opt(value: Option<f64>, min: f64, max: f64) -> Option<f64> {
    match value {
        Some(v) if v >= min && v <= max => Some(v),
        _ => None,
    }
}

fn normalize_readings(params: &BTreeMap<String, String>) -> WeatherReadings {
    let temp_c = parse_f64(params, &["tempc", "temp_c"])
        .or_else(|| parse_f64(params, &["tempf", "temp_f"]).map(f_to_c));

    let humidity_percent = parse_f64(
        params,
        &[
            "humidity",
            "humidityout",
            "humidity_out",
            "hum",
            "humidity_percent",
        ],
    );

    let wind_speed_mps = parse_f64(params, &["windspeedms", "wind_speed_ms"])
        .or_else(|| parse_f64(params, &["windspeedmph", "wind_speed_mph"]).map(mph_to_mps));
    let wind_gust_mps = parse_f64(params, &["windgustms", "wind_gust_ms"])
        .or_else(|| parse_f64(params, &["windgustmph", "wind_gust_mph"]).map(mph_to_mps));
    let wind_dir_deg = parse_f64(params, &["winddir", "wind_dir", "wind_direction"]);

    let rain_daily_mm = parse_f64(params, &["dailyrainmm", "daily_rain_mm"])
        .or_else(|| parse_f64(params, &["dailyrainin", "daily_rain_in"]).map(inches_to_mm));

    let rain_rate_mm_per_hour =
        parse_f64(params, &["rainmm", "rain_mm", "rain_rate_mm"]).or_else(|| {
            parse_f64(params, &["rainin", "rain_in", "rainratein", "rain_rate_in"])
                .map(inches_to_mm)
        });

    let uv_index = parse_f64(params, &["uv", "UV"]);
    let solar_radiation_wm2 = parse_f64(params, &["solarradiation", "solar_radiation"]);

    // Pressure has two distinct interpretations:
    // - Relative (sea-level adjusted): `baromrelin` (inHg) or legacy `baromin` (inHg)
    // - Absolute (station): `baromabsin` (inHg)
    //
    // Avoid mixing these into a single sensor — keep them separate for transparency.
    let pressure_relative_kpa = parse_f64(params, &["baromrelin"])
        .or_else(|| parse_f64(params, &["baromin"]))
        .map(inhg_to_kpa);

    let pressure_absolute_kpa = parse_f64(params, &["baromabsin"]).map(inhg_to_kpa);

    WeatherReadings {
        // Range clamps (fail-open by dropping implausible values).
        temp_c: clamp_opt(temp_c, -80.0, 80.0),
        humidity_percent: clamp_opt(humidity_percent, 0.0, 100.0),
        wind_speed_mps: clamp_opt(wind_speed_mps, 0.0, 200.0),
        wind_gust_mps: clamp_opt(wind_gust_mps, 0.0, 200.0),
        wind_dir_deg: clamp_opt(wind_dir_deg, 0.0, 360.0),
        rain_daily_mm: clamp_opt(rain_daily_mm, 0.0, 10_000.0),
        rain_rate_mm_per_hour: clamp_opt(rain_rate_mm_per_hour, 0.0, 10_000.0),
        uv_index: clamp_opt(uv_index, 0.0, 50.0),
        solar_radiation_wm2: clamp_opt(solar_radiation_wm2, 0.0, 5_000.0),
        pressure_relative_kpa: clamp_opt(pressure_relative_kpa, 50.0, 150.0),
        pressure_absolute_kpa: clamp_opt(pressure_absolute_kpa, 50.0, 150.0),
    }
}

fn required_missing(readings: &WeatherReadings) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if readings.temp_c.is_none() {
        missing.push("temperature");
    }
    if readings.humidity_percent.is_none() {
        missing.push("humidity");
    }
    if readings.wind_speed_mps.is_none() {
        missing.push("wind_speed");
    }
    if readings.wind_dir_deg.is_none() {
        missing.push("wind_direction");
    }
    if readings.rain_daily_mm.is_none() {
        missing.push("rain");
    }
    if readings.uv_index.is_none() {
        missing.push("uv");
    }
    if readings.solar_radiation_wm2.is_none() {
        missing.push("solar_radiation");
    }
    if readings.pressure_relative_kpa.is_none() {
        missing.push("pressure_relative");
    }
    missing
}

fn clamp_metric_ts(metric_ts: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    if metric_ts > now + ChronoDuration::minutes(5) {
        return now;
    }
    if metric_ts < now - ChronoDuration::days(2) {
        return now;
    }
    metric_ts
}

fn parse_metric_ts(params: &BTreeMap<String, String>, now: DateTime<Utc>) -> DateTime<Utc> {
    let Some(raw) = params
        .get("dateutc")
        .or_else(|| params.get("date_utc"))
        .or_else(|| params.get("timestamp"))
    else {
        return now;
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("now") {
        return now;
    }

    let normalized = trimmed.replace('+', " ");

    if let Ok(ts) = DateTime::parse_from_rfc3339(&normalized) {
        return clamp_metric_ts(ts.with_timezone(&Utc), now);
    }

    if let Ok(naive) = NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S") {
        return clamp_metric_ts(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc), now);
    }

    now
}

async fn ensure_ws_sensors(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    node_id: Uuid,
) -> Result<(), sqlx::Error> {
    let interval_seconds: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT interval_seconds
        FROM sensors
        WHERE node_id = $1
          AND deleted_at IS NULL
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .bind(node_id)
    .fetch_optional(&mut **tx)
    .await?;

    let interval_seconds = interval_seconds.unwrap_or(presets::ws_2902_default_interval_seconds());
    let _ = insert_ws_sensors(tx, node_id, interval_seconds).await?;
    ensure_ws_derived_sensors(tx, node_id, interval_seconds).await?;

    // Hide legacy ambiguous pressure sensor (pre-2026-01-18) to prevent mixing/mislabeling.
    let legacy_pressure_id = ws_sensor_id(node_id, "ws2902:pressure");
    let pressure_relative_id = ws_sensor_id(node_id, "ws2902:pressure_relative");
    let pressure_absolute_id = ws_sensor_id(node_id, "ws2902:pressure_absolute");
    let _ = sqlx::query(
        r#"
        UPDATE sensors
        SET config = COALESCE(config, '{}'::jsonb)
            || jsonb_build_object(
                'hidden', true,
                'legacy', true,
                'replaced_by', jsonb_build_array($2::text, $3::text)
            )
        WHERE sensor_id = $1
          AND deleted_at IS NULL
        "#,
    )
    .bind(&legacy_pressure_id)
    .bind(&pressure_relative_id)
    .bind(&pressure_absolute_id)
    .execute(&mut **tx)
    .await?;

    // Ensure WS-managed sensors are explicitly labeled as local weather-station sources.
    let _ = sqlx::query(
        r#"
        UPDATE sensors
        SET config = COALESCE(config, '{}'::jsonb) || jsonb_build_object('source', 'ws_2902')
        WHERE node_id = $1
          AND deleted_at IS NULL
          AND (COALESCE(config, '{}'::jsonb)->>'source') IS NULL
        "#,
    )
    .bind(node_id)
    .execute(&mut **tx)
    .await?;

    let _ = sqlx::query(
        r#"
        UPDATE sensors
        SET config = COALESCE(config, '{}'::jsonb) || jsonb_build_object('pressure_reference', 'relative')
        WHERE sensor_id = $1
          AND deleted_at IS NULL
          AND (COALESCE(config, '{}'::jsonb)->>'pressure_reference') IS NULL
        "#,
    )
    .bind(&pressure_relative_id)
    .execute(&mut **tx)
    .await?;

    let _ = sqlx::query(
        r#"
        UPDATE sensors
        SET config = COALESCE(config, '{}'::jsonb) || jsonb_build_object('pressure_reference', 'absolute')
        WHERE sensor_id = $1
          AND deleted_at IS NULL
          AND (COALESCE(config, '{}'::jsonb)->>'pressure_reference') IS NULL
        "#,
    )
    .bind(&pressure_absolute_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn ensure_ws_derived_sensors(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    node_id: Uuid,
    interval_seconds: i32,
) -> Result<(), sqlx::Error> {
    let wind_direction_id = ws_sensor_id(node_id, "ws2902:wind_direction");
    let rain_daily_id = ws_sensor_id(node_id, "ws2902:rain");

    let derived_sensors = [
        (
            "ws2902:wind_dir_sin",
            "Wind direction (sin)",
            "wind_dir_sin",
            "",
            serde_json::json!({
                "source": "derived",
                "derived": {
                    "expression": "sin(deg2rad(wind_direction))",
                    "inputs": [
                        { "sensor_id": wind_direction_id.as_str(), "var": "wind_direction" }
                    ]
                }
            }),
        ),
        (
            "ws2902:wind_dir_cos",
            "Wind direction (cos)",
            "wind_dir_cos",
            "",
            serde_json::json!({
                "source": "derived",
                "derived": {
                    "expression": "cos(deg2rad(wind_direction))",
                    "inputs": [
                        { "sensor_id": wind_direction_id.as_str(), "var": "wind_direction" }
                    ]
                }
            }),
        ),
        (
            "ws2902:rain_inc",
            "Rain (increment)",
            "rain_inc",
            "mm",
            serde_json::json!({
                "source": "derived",
                "derived": {
                    "expression": "max(0, rain_now - rain_prev)",
                    "inputs": [
                        { "sensor_id": rain_daily_id.as_str(), "var": "rain_now" },
                        { "sensor_id": rain_daily_id.as_str(), "var": "rain_prev", "lag_seconds": interval_seconds as i64 }
                    ]
                }
            }),
        ),
    ];

    for (key, name, sensor_type, unit, config) in derived_sensors {
        let sensor_id = ws_sensor_id(node_id, key);
        sqlx::query(
            r#"
            INSERT INTO sensors (
                sensor_id,
                node_id,
                name,
                type,
                unit,
                interval_seconds,
                rolling_avg_seconds,
                config
            )
            VALUES ($1, $2, $3, $4, $5, $6, 0, $7)
            ON CONFLICT (sensor_id) DO NOTHING
            "#,
        )
        .bind(sensor_id)
        .bind(node_id)
        .bind(name)
        .bind(sensor_type)
        .bind(unit)
        .bind(interval_seconds)
        .bind(SqlJson(config))
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

fn redact_payload(params: &BTreeMap<String, String>) -> JsonValue {
    let mut redacted = serde_json::Map::new();
    for (key, value) in params {
        let upper = key.to_ascii_uppercase();
        if upper.contains("PASS") || upper.contains("TOKEN") || upper.contains("KEY") {
            redacted.insert(key.clone(), JsonValue::String("[REDACTED]".to_string()));
        } else {
            redacted.insert(key.clone(), JsonValue::String(value.clone()));
        }
    }
    JsonValue::Object(redacted)
}

async fn insert_ws_sensors(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    node_id: Uuid,
    interval_seconds: i32,
) -> Result<Vec<Ws2902CreatedSensor>, sqlx::Error> {
    let mut created = Vec::new();
    let mut seen = BTreeSet::new();
    for mut sensor in ws_sensors() {
        let key = format!("ws2902:{}", sensor.sensor_type);
        if !seen.insert(key.clone()) {
            continue;
        }
        let sensor_id = ws_sensor_id(node_id, &key);
        sensor.sensor_id = sensor_id.clone();
        sensor.interval_seconds = interval_seconds;

        sqlx::query(
            r#"
            INSERT INTO sensors (
                sensor_id,
                node_id,
                name,
                type,
                unit,
                interval_seconds,
                rolling_avg_seconds,
                config
            )
            VALUES ($1, $2, $3, $4, $5, $6, 0, '{}'::jsonb)
            ON CONFLICT (sensor_id) DO NOTHING
            "#,
        )
        .bind(&sensor.sensor_id)
        .bind(node_id)
        .bind(&sensor.name)
        .bind(&sensor.sensor_type)
        .bind(&sensor.unit)
        .bind(sensor.interval_seconds)
        .execute(&mut **tx)
        .await?;

        created.push(sensor);
    }
    Ok(created)
}

async fn fetch_integration(db: &PgPool, id: Uuid) -> Result<Option<IntegrationRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            id,
            node_id,
            nickname,
            protocol,
            enabled,
            created_at,
            rotated_at,
            last_seen,
            COALESCE(last_missing_fields, '[]'::jsonb) as last_missing_fields,
            COALESCE(last_payload, '{}'::jsonb) as last_payload
        FROM weather_station_integrations
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

async fn fetch_latest_integration_id_for_node(
    db: &PgPool,
    node_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM weather_station_integrations
        WHERE node_id = $1
          AND kind = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(node_id)
    .bind(WS_2902_KIND)
    .fetch_optional(db)
    .await
}

#[utoipa::path(
    post,
    path = "/api/weather-stations/ws-2902",
    tag = "weather-stations",
    request_body = Ws2902CreateRequest,
    responses(
        (status = 200, description = "Integration created", body = Ws2902CreateResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn create_ws2902(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<Ws2902CreateRequest>,
) -> Result<Json<Ws2902CreateResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let nickname = payload.nickname.trim();
    if nickname.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "nickname is required".to_string()));
    }
    let interval_seconds = payload
        .interval_seconds
        .unwrap_or(presets::ws_2902_default_interval_seconds())
        .clamp(5, 3600);
    let token = generate_token();
    let token_hash = token_hash(&token);

    let mut tx = state.db.begin().await.map_err(map_db_error)?;
    let node_name = format!("Weather station — {nickname}");
    let node_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO nodes (name, status, config)
        VALUES ($1, 'offline', jsonb_build_object('kind', $2, 'protocol', $3))
        RETURNING id
        "#,
    )
    .bind(node_name)
    .bind(WS_2902_KIND)
    .bind(format!("{:?}", payload.protocol).to_ascii_lowercase())
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let created_at = Utc::now();
    let protocol_str = format!("{:?}", payload.protocol).to_ascii_lowercase();
    let integration_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO weather_station_integrations (
            node_id,
            kind,
            nickname,
            protocol,
            token_hash,
            enabled,
            created_at
        )
        VALUES ($1, $2, $3, $4, $5, true, $6)
        RETURNING id
        "#,
    )
    .bind(node_id)
    .bind(WS_2902_KIND)
    .bind(nickname)
    .bind(protocol_str)
    .bind(token_hash)
    .bind(created_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let sensors = insert_ws_sensors(&mut tx, node_id, interval_seconds)
        .await
        .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;

    Ok(Json(Ws2902CreateResponse {
        id: integration_id.to_string(),
        node_id: node_id.to_string(),
        nickname: nickname.to_string(),
        protocol: payload.protocol,
        enabled: true,
        ingest_path: ingest_path(&token),
        token,
        created_at: created_at.to_rfc3339(),
        sensors,
    }))
}

#[utoipa::path(
    get,
    path = "/api/weather-stations/ws-2902/{integration_id}",
    tag = "weather-stations",
    params(("integration_id" = String, Path, description = "Integration id")),
    responses(
        (status = 200, description = "Integration status", body = Ws2902StatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
pub(crate) async fn get_ws2902_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(integration_id): Path<String>,
) -> Result<Json<Ws2902StatusResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let integration_id = Uuid::parse_str(integration_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Integration not found".to_string()))?;
    let row = fetch_integration(&state.db, integration_id)
        .await
        .map_err(map_db_error)?;
    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Integration not found".to_string()));
    };

    let protocol = match row.protocol.as_str() {
        "ambient" => Ws2902Protocol::Ambient,
        _ => Ws2902Protocol::Wunderground,
    };

    let missing = row
        .last_missing_fields
        .0
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let payload = match row.last_payload.0 {
        JsonValue::Object(map) if map.is_empty() => None,
        other => Some(other),
    };

    Ok(Json(Ws2902StatusResponse {
        id: row.id.to_string(),
        node_id: row.node_id.to_string(),
        nickname: row.nickname,
        protocol,
        enabled: row.enabled,
        ingest_path_template: ingest_path_template(),
        created_at: row.created_at.to_rfc3339(),
        rotated_at: row.rotated_at.map(|ts| ts.to_rfc3339()),
        last_seen: row.last_seen.map(|ts| ts.to_rfc3339()),
        last_missing_fields: missing,
        last_payload: payload,
    }))
}

#[utoipa::path(
    put,
    path = "/api/weather-stations/ws-2902/{integration_id}",
    tag = "weather-stations",
    request_body = Ws2902UpdateRequest,
    params(("integration_id" = String, Path, description = "Integration id")),
    responses(
        (status = 200, description = "Integration updated", body = Ws2902StatusResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
pub(crate) async fn update_ws2902(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(integration_id): Path<String>,
    Json(payload): Json<Ws2902UpdateRequest>,
) -> Result<Json<Ws2902StatusResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let integration_id = Uuid::parse_str(integration_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Integration not found".to_string()))?;

    sqlx::query(
        r#"
        UPDATE weather_station_integrations
        SET enabled = $2
        WHERE id = $1
        "#,
    )
    .bind(integration_id)
    .bind(payload.enabled)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    get_ws2902_status(
        axum::extract::State(state),
        AuthUser(user),
        Path(integration_id.to_string()),
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/weather-stations/ws-2902/{integration_id}/rotate-token",
    tag = "weather-stations",
    params(("integration_id" = String, Path, description = "Integration id")),
    responses(
        (status = 200, description = "Token rotated", body = Ws2902RotateTokenResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
pub(crate) async fn rotate_ws2902_token(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(integration_id): Path<String>,
) -> Result<Json<Ws2902RotateTokenResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let integration_id = Uuid::parse_str(integration_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Integration not found".to_string()))?;

    let token = generate_token();
    let token_hash = token_hash(&token);
    let rotated_at = Utc::now();

    let updated = sqlx::query(
        r#"
        UPDATE weather_station_integrations
        SET token_hash = $2,
            rotated_at = $3
        WHERE id = $1
        "#,
    )
    .bind(integration_id)
    .bind(token_hash)
    .bind(rotated_at)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;
    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Integration not found".to_string()));
    }

    Ok(Json(Ws2902RotateTokenResponse {
        id: integration_id.to_string(),
        ingest_path: ingest_path(&token),
        token,
        rotated_at: rotated_at.to_rfc3339(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/weather-stations/ws-2902/node/{node_id}",
    tag = "weather-stations",
    params(("node_id" = String, Path, description = "Weather station node id")),
    responses(
        (status = 200, description = "Integration status", body = Ws2902StatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
pub(crate) async fn get_ws2902_status_for_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<Ws2902StatusResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_id = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Integration not found".to_string()))?;
    let integration_id = fetch_latest_integration_id_for_node(&state.db, node_id)
        .await
        .map_err(map_db_error)?;
    let Some(integration_id) = integration_id else {
        return Err((StatusCode::NOT_FOUND, "Integration not found".to_string()));
    };

    get_ws2902_status(
        axum::extract::State(state),
        AuthUser(user),
        Path(integration_id.to_string()),
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/weather-stations/ws-2902/node/{node_id}/rotate-token",
    tag = "weather-stations",
    params(("node_id" = String, Path, description = "Weather station node id")),
    responses(
        (status = 200, description = "Token rotated", body = Ws2902RotateTokenResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Not found")
    )
)]
pub(crate) async fn rotate_ws2902_token_for_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<Ws2902RotateTokenResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_id = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Integration not found".to_string()))?;
    let integration_id = fetch_latest_integration_id_for_node(&state.db, node_id)
        .await
        .map_err(map_db_error)?;
    let Some(integration_id) = integration_id else {
        return Err((StatusCode::NOT_FOUND, "Integration not found".to_string()));
    };

    rotate_ws2902_token(
        axum::extract::State(state),
        AuthUser(user),
        Path(integration_id.to_string()),
    )
    .await
}

#[utoipa::path(
    get,
    path = "/api/ws/{token}",
    tag = "weather-stations",
    params(("token" = String, Path, description = "Ingest token")),
    responses(
        (status = 200, description = "Upload accepted"),
        (status = 403, description = "Integration disabled"),
        (status = 404, description = "Unknown token")
    )
)]
pub(crate) async fn ingest_ws_short(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(token): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    ingest_ws2902(axum::extract::State(state), Path(token), RawQuery(raw)).await
}

#[utoipa::path(
    get,
    path = "/api/weather-stations/ws-2902/ingest/{token}",
    tag = "weather-stations",
    params(
        ("token" = String, Path, description = "Ingest token"),
    ),
    responses(
        (status = 200, description = "Upload accepted"),
        (status = 403, description = "Integration disabled"),
        (status = 404, description = "Unknown token")
    )
)]
pub(crate) async fn ingest_ws2902(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(token): Path<String>,
    RawQuery(raw): RawQuery,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    let (token, params) = normalize_token_and_params(&token, raw);
    let token_hash = token_hash(token.trim());
    let readings = normalize_readings(&params);
    let missing = required_missing(&readings);
    let payload = redact_payload(&params);
    let now = Utc::now();
    let metric_ts = parse_metric_ts(&params, now);

    let mut tx = state.db.begin().await.map_err(map_db_error)?;
    let row: Option<(Uuid, Uuid, bool)> = sqlx::query_as(
        r#"
        SELECT id, node_id, enabled
        FROM weather_station_integrations
        WHERE token_hash = $1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some((integration_id, node_id, enabled)) = row else {
        return Err((StatusCode::NOT_FOUND, "Unknown token".to_string()));
    };
    if !enabled {
        return Err((StatusCode::FORBIDDEN, "Integration disabled".to_string()));
    }

    let node_allowed: Option<bool> = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM nodes
            WHERE id = $1
              AND status <> 'deleted'
              AND NOT (COALESCE(config, '{}'::jsonb) @> '{"deleted": true}')
              AND NOT (COALESCE(config, '{}'::jsonb) @> '{"poll_enabled": false}')
        )
        "#,
    )
    .bind(node_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    if node_allowed != Some(true) {
        return Err((StatusCode::FORBIDDEN, "Integration disabled".to_string()));
    }

    ensure_ws_sensors(&mut tx, node_id)
        .await
        .map_err(map_db_error)?;

    sqlx::query(
        r#"
        UPDATE weather_station_integrations
        SET last_seen = $2,
            last_missing_fields = $3,
            last_payload = $4
        WHERE id = $1
        "#,
    )
    .bind(integration_id)
    .bind(now)
    .bind(SqlJson(serde_json::json!(missing)))
    .bind(SqlJson(payload))
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    sqlx::query(
        r#"
        UPDATE nodes
        SET status = 'online',
            last_seen = $2
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let metrics = vec![
        ("temperature", readings.temp_c),
        ("humidity", readings.humidity_percent),
        ("wind_speed", readings.wind_speed_mps),
        ("wind_gust", readings.wind_gust_mps),
        ("wind_direction", readings.wind_dir_deg),
        ("rain", readings.rain_daily_mm),
        ("rain_rate", readings.rain_rate_mm_per_hour),
        ("uv", readings.uv_index),
        ("solar_radiation", readings.solar_radiation_wm2),
        ("pressure_relative", readings.pressure_relative_kpa),
        ("pressure_absolute", readings.pressure_absolute_kpa),
    ];

    for (metric_key, value_opt) in metrics {
        let Some(value) = value_opt else {
            continue;
        };
        let sensor_id = ws_sensor_id(node_id, &format!("ws2902:{metric_key}"));
        sqlx::query(
            r#"
            INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at)
            SELECT $1, $2, $3, 0, now()
            WHERE EXISTS (
                SELECT 1
                FROM sensors
                WHERE sensor_id = $1
                  AND deleted_at IS NULL
                  AND COALESCE(config->>'poll_enabled', 'true') <> 'false'
            )
            ON CONFLICT (sensor_id, ts) DO UPDATE SET
                value = EXCLUDED.value,
                inserted_at = EXCLUDED.inserted_at
            "#,
        )
        .bind(sensor_id)
        .bind(metric_ts)
        .bind(value)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }

    let custom_sensors: Vec<(String, SqlJson<JsonValue>)> = sqlx::query_as(
        r#"
        SELECT sensor_id, COALESCE(config, '{}'::jsonb) as config
        FROM sensors
        WHERE node_id = $1
          AND deleted_at IS NULL
          AND COALESCE(config->>'source', '') = 'ws_2902'
          AND COALESCE(config->>'ws_field', '') <> ''
          AND COALESCE(config->>'poll_enabled', 'true') <> 'false'
        "#,
    )
    .bind(node_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(map_db_error)?;

    for (sensor_id, config_json) in custom_sensors {
        let field = config_json
            .0
            .get("ws_field")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if field.is_empty() {
            continue;
        }

        let Some(mut value) = parse_f64(&params, &[field.as_str()]) else {
            continue;
        };

        let scale = config_f64(&config_json.0, "ws_scale").unwrap_or(1.0);
        let offset = config_f64(&config_json.0, "ws_offset").unwrap_or(0.0);
        value = value * scale + offset;

        if let Some(min) = config_f64(&config_json.0, "ws_min") {
            if value < min {
                continue;
            }
        }
        if let Some(max) = config_f64(&config_json.0, "ws_max") {
            if value > max {
                continue;
            }
        }

        sqlx::query(
            r#"
            INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at)
            SELECT $1, $2, $3, 0, now()
            WHERE EXISTS (
                SELECT 1
                FROM sensors
                WHERE sensor_id = $1
                  AND deleted_at IS NULL
                  AND COALESCE(config->>'poll_enabled', 'true') <> 'false'
            )
            ON CONFLICT (sensor_id, ts) DO UPDATE SET
                value = EXCLUDED.value,
                inserted_at = EXCLUDED.inserted_at
            "#,
        )
        .bind(sensor_id)
        .bind(metric_ts)
        .bind(value)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }

    tx.commit().await.map_err(map_db_error)?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "received_at": now.to_rfc3339(),
        "missing_fields": missing,
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ws/{token}", get(ingest_ws_short))
        .route("/weather-stations/ws-2902", post(create_ws2902))
        .route(
            "/weather-stations/ws-2902/node/{node_id}",
            get(get_ws2902_status_for_node),
        )
        .route(
            "/weather-stations/ws-2902/node/{node_id}/rotate-token",
            post(rotate_ws2902_token_for_node),
        )
        .route(
            "/weather-stations/ws-2902/{integration_id}",
            get(get_ws2902_status).put(update_ws2902),
        )
        .route(
            "/weather-stations/ws-2902/{integration_id}/rotate-token",
            post(rotate_ws2902_token),
        )
        .route(
            "/weather-stations/ws-2902/ingest/{token}",
            get(ingest_ws2902),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws2902_token_is_short_hex_and_path_is_short() {
        let token = generate_token();
        assert_eq!(token.len(), 24);
        assert!(
            token.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')),
            "token should be lowercase hex"
        );
        assert_eq!(ingest_path(&token), format!("/api/ws/{token}"));
        assert_eq!(ingest_path_template(), "/api/ws/{token}");
    }

    #[test]
    fn ws2902_ingest_accepts_embedded_query_without_question_mark() {
        let token = "79e028f445193fee54c52cd0";
        let embedded = format!("{token}ID=1111&PASSWORD=2222&tempf=54.5&humidity=55&windspeedmph=3.1&winddir=10&dailyrainin=0&uv=0&solarradiation=0&baromin=-9999&dateutc=now");
        let (normalized_token, params) = normalize_token_and_params(&embedded, None);
        assert_eq!(normalized_token, token);
        assert_eq!(params.get("ID").map(String::as_str), Some("1111"));
        assert_eq!(params.get("tempf").map(String::as_str), Some("54.5"));

        let readings = normalize_readings(&params);
        assert!(readings.pressure_relative_kpa.is_none());
        assert!(readings.pressure_absolute_kpa.is_none());
    }

    #[test]
    fn ws2902_pressure_parses_relative_and_absolute_inhg() {
        let mut params = BTreeMap::new();
        params.insert("baromrelin".to_string(), "29.92".to_string());
        params.insert("baromabsin".to_string(), "29.50".to_string());

        let readings = normalize_readings(&params);
        let relative = readings.pressure_relative_kpa.expect("relative pressure");
        let absolute = readings.pressure_absolute_kpa.expect("absolute pressure");

        assert!((relative - 101.3).abs() < 0.4);
        assert!((absolute - 99.9).abs() < 0.4);
    }

    #[test]
    fn ws2902_pressure_legacy_baromin_is_relative() {
        let mut params = BTreeMap::new();
        params.insert("baromin".to_string(), "29.92".to_string());

        let readings = normalize_readings(&params);
        let relative = readings.pressure_relative_kpa.expect("relative pressure");
        assert!((relative - 101.3).abs() < 0.4);
        assert!(readings.pressure_absolute_kpa.is_none());
    }
}
