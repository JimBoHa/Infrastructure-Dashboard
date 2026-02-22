use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{internal_error, map_db_error};
use crate::ids;
use crate::state::AppState;

fn default_interval_seconds() -> f64 {
    30.0
}

fn default_scale() -> f64 {
    1.0
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct NodeAds1263SettingsDraft {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spi_bus: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spi_device: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spi_mode: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spi_speed_hz: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rst_bcm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cs_bcm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drdy_bcm: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vref_volts: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gain: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_rate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_interval_seconds: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct NodeSensorDraft {
    #[serde(default)]
    pub preset: String,
    #[serde(default)]
    pub sensor_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub driver_type: String,
    #[serde(default)]
    pub channel: i32,
    #[serde(default)]
    pub unit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default = "default_interval_seconds")]
    pub interval_seconds: f64,
    #[serde(default)]
    pub rolling_average_seconds: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_max: Option<f64>,
    #[serde(default)]
    pub offset: f64,
    #[serde(default = "default_scale")]
    pub scale: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulses_per_unit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_loop_shunt_ohms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_loop_range_m: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct NodeSensorsConfigResponse {
    pub node_id: String,
    pub sensors: Vec<NodeSensorDraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ads1263: Option<NodeAds1263SettingsDraft>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analog_backend: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analog_health: Option<NodeAnalogHealth>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct NodeAnalogHealth {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chip_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_ok_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct NodeSensorsConfigUpdateRequest {
    pub sensors: Vec<NodeSensorDraft>,
    #[serde(default)]
    pub ads1263: Option<NodeAds1263SettingsDraft>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct NodeSensorsOrderUpdateRequest {
    pub sensor_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ApplyNodeSensorsConfigResponse {
    pub status: String,
    pub node_id: String,
    pub node_agent_url: Option<String>,
    pub sensors: Vec<NodeSensorDraft>,
    pub deleted_sensor_ids: Vec<String>,
    pub warning: Option<String>,
}

#[derive(sqlx::FromRow)]
struct NodeConfigRow {
    config: SqlJson<JsonValue>,
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
}

#[derive(sqlx::FromRow)]
struct CoreSensorRow {
    sensor_id: String,
    name: String,
    sensor_type: String,
    unit: String,
    interval_seconds: i32,
    rolling_avg_seconds: i32,
    config: SqlJson<JsonValue>,
}

fn ensure_object<'a>(
    value: &'a mut JsonValue,
    context: &str,
) -> Result<&'a mut serde_json::Map<String, JsonValue>, String> {
    match value {
        JsonValue::Object(map) => Ok(map),
        JsonValue::Null => {
            *value = JsonValue::Object(serde_json::Map::new());
            match value {
                JsonValue::Object(map) => Ok(map),
                _ => unreachable!("inserted JsonValue::Object but pattern did not match"),
            }
        }
        other => Err(format!(
            "Expected JSON object for {context}, got {}",
            json_type_name(other)
        )),
    }
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

async fn load_node_config(
    db: &PgPool,
    node_id: Uuid,
) -> Result<Option<NodeConfigRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            COALESCE(config, '{}'::jsonb) as config,
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(db)
    .await
}

fn parse_optional_f64(value: &JsonValue) -> Option<f64> {
    match value {
        JsonValue::Number(num) => num.as_f64(),
        JsonValue::String(raw) => raw.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_optional_i32(value: &JsonValue) -> Option<i32> {
    parse_optional_f64(value).map(|v| v.round() as i32)
}

fn parse_optional_string(value: &JsonValue) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
}

fn sensor_config_from_row(row: CoreSensorRow) -> NodeSensorDraft {
    let config = row.config.0;
    let mut driver = config
        .get("driver_type")
        .or_else(|| config.get("driver"))
        .and_then(parse_optional_string)
        .unwrap_or_else(|| "analog".to_string());
    let normalized = driver.trim().to_lowercase();
    if normalized == "ads1115" || normalized == "ads1263" {
        driver = "analog".to_string();
    }
    let channel = config
        .get("channel")
        .and_then(parse_optional_i32)
        .unwrap_or(0);
    let location = config.get("location").and_then(parse_optional_string);

    NodeSensorDraft {
        preset: row.sensor_type,
        sensor_id: row.sensor_id,
        name: row.name,
        driver_type: driver,
        channel,
        unit: row.unit,
        location,
        interval_seconds: row.interval_seconds as f64,
        rolling_average_seconds: row.rolling_avg_seconds as f64,
        input_min: config.get("input_min").and_then(parse_optional_f64),
        input_max: config.get("input_max").and_then(parse_optional_f64),
        output_min: config.get("output_min").and_then(parse_optional_f64),
        output_max: config.get("output_max").and_then(parse_optional_f64),
        offset: config
            .get("offset")
            .and_then(parse_optional_f64)
            .unwrap_or(0.0),
        scale: config
            .get("scale")
            .and_then(parse_optional_f64)
            .unwrap_or(1.0),
        pulses_per_unit: config.get("pulses_per_unit").and_then(parse_optional_f64),
        current_loop_shunt_ohms: config
            .get("current_loop_shunt_ohms")
            .and_then(parse_optional_f64),
        current_loop_range_m: config
            .get("current_loop_range_m")
            .and_then(parse_optional_f64),
    }
}

pub(crate) async fn fetch_core_node_agent_sensors(
    db: &PgPool,
    node_id: Uuid,
) -> Result<Vec<NodeSensorDraft>, sqlx::Error> {
    let rows: Vec<CoreSensorRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            name,
            type as sensor_type,
            unit,
            interval_seconds,
            rolling_avg_seconds,
            COALESCE(config, '{}'::jsonb) as config
        FROM sensors
        WHERE node_id = $1
          AND deleted_at IS NULL
          AND COALESCE(config->>'source', '') = 'node_agent'
        ORDER BY created_at ASC
        "#,
    )
    .bind(node_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(sensor_config_from_row).collect())
}

fn normalize_sensor_draft(mut sensor: NodeSensorDraft) -> Result<NodeSensorDraft, String> {
    sensor.preset = sensor.preset.trim().to_string();
    sensor.sensor_id = sensor.sensor_id.trim().to_string();
    sensor.name = sensor.name.trim().to_string();
    sensor.driver_type = sensor.driver_type.trim().to_string();
    let normalized = sensor.driver_type.to_lowercase();
    if normalized == "ads1115" {
        return Err("driver_type 'ads1115' has been removed; use 'analog'.".to_string());
    }
    if normalized == "ads1263" {
        sensor.driver_type = "analog".to_string();
    }
    sensor.unit = sensor.unit.trim().to_string();
    sensor.location = sensor
        .location
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());

    if sensor.name.is_empty() {
        return Err("Sensor name is required".to_string());
    }
    if sensor.preset.is_empty() {
        return Err("Sensor preset is required".to_string());
    }
    if sensor.driver_type.is_empty() {
        return Err("Sensor driver type is required".to_string());
    }
    if sensor.unit.is_empty() {
        return Err("Sensor unit is required".to_string());
    }
    if !sensor.interval_seconds.is_finite() || sensor.interval_seconds < 0.0 {
        return Err("interval_seconds must be a non-negative number".to_string());
    }
    if !sensor.rolling_average_seconds.is_finite() || sensor.rolling_average_seconds < 0.0 {
        return Err("rolling_average_seconds must be a non-negative number".to_string());
    }

    if sensor.current_loop_shunt_ohms.is_some() || sensor.current_loop_range_m.is_some() {
        if !sensor.preset.starts_with("water_level") {
            return Err(
                "current_loop_* settings are only supported for water_level sensors".to_string(),
            );
        }
        let shunt = sensor.current_loop_shunt_ohms.unwrap_or(0.0);
        let range_m = sensor.current_loop_range_m.unwrap_or(0.0);
        if !shunt.is_finite() || shunt <= 0.0 {
            return Err("current_loop_shunt_ohms must be a positive number".to_string());
        }
        if !range_m.is_finite() || range_m <= 0.0 {
            return Err("current_loop_range_m must be a positive number".to_string());
        }
        sensor.current_loop_shunt_ohms = Some(shunt);
        sensor.current_loop_range_m = Some(range_m);
    }

    sensor.channel = sensor.channel.max(0);
    Ok(sensor)
}

fn build_node_agent_sensor_payload(sensor: &NodeSensorDraft) -> JsonValue {
    serde_json::json!({
        "sensor_id": sensor.sensor_id,
        "name": sensor.name,
        "type": sensor.driver_type,
        "channel": sensor.channel,
        "unit": sensor.unit,
        "location": sensor.location,
        "interval_seconds": sensor.interval_seconds,
        "rolling_average_seconds": sensor.rolling_average_seconds,
        "input_min": sensor.input_min,
        "input_max": sensor.input_max,
        "output_min": sensor.output_min,
        "output_max": sensor.output_max,
        "offset": sensor.offset,
        "scale": sensor.scale,
        "pulses_per_unit": sensor.pulses_per_unit,
        "current_loop_shunt_ohms": sensor.current_loop_shunt_ohms,
        "current_loop_range_m": sensor.current_loop_range_m,
    })
}

fn build_core_sensor_config(sensor: &NodeSensorDraft) -> Result<JsonValue, String> {
    let interval_seconds = sensor.interval_seconds.round();
    let rolling_average_seconds = sensor.rolling_average_seconds.round();
    if interval_seconds < 0.0 || rolling_average_seconds < 0.0 {
        return Err(
            "interval_seconds and rolling_average_seconds must be non-negative".to_string(),
        );
    }
    Ok(serde_json::json!({
        "source": "node_agent",
        "driver_type": sensor.driver_type,
        "channel": sensor.channel,
        "location": sensor.location,
        "interval_seconds": interval_seconds,
        "rolling_average_seconds": rolling_average_seconds,
        "input_min": sensor.input_min,
        "input_max": sensor.input_max,
        "output_min": sensor.output_min,
        "output_max": sensor.output_max,
        "offset": sensor.offset,
        "scale": sensor.scale,
        "pulses_per_unit": sensor.pulses_per_unit,
        "current_loop_shunt_ohms": sensor.current_loop_shunt_ohms,
        "current_loop_range_m": sensor.current_loop_range_m,
    }))
}

async fn sensor_id_exists(db: &PgPool, sensor_id: &str) -> Result<bool, sqlx::Error> {
    let existing: Option<String> = sqlx::query_scalar(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id)
    .fetch_optional(db)
    .await?;
    Ok(existing.is_some())
}

async fn validate_sensor_id_ownership(
    db: &PgPool,
    sensor_id: &str,
    node_id: Uuid,
) -> Result<(), (StatusCode, String)> {
    let row: Option<(Uuid, SqlJson<JsonValue>)> = sqlx::query_as(
        r#"
        SELECT node_id, COALESCE(config, '{}'::jsonb) as config
        FROM sensors
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id)
    .fetch_optional(db)
    .await
    .map_err(map_db_error)?;

    let Some((existing_node_id, config)) = row else {
        return Ok(());
    };
    if existing_node_id != node_id {
        return Err((
            StatusCode::CONFLICT,
            format!("sensor_id {sensor_id} already exists on a different node"),
        ));
    }
    let source = config
        .0
        .get("source")
        .and_then(parse_optional_string)
        .unwrap_or_default();
    if !source.is_empty() && source != "node_agent" {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "sensor_id {sensor_id} is managed by source {source} and cannot be changed via node sensor config"
            ),
        ));
    }
    Ok(())
}

async fn allocate_sensor_id(
    db: &PgPool,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
    created_at: chrono::DateTime<chrono::Utc>,
    used: &mut std::collections::BTreeSet<String>,
) -> Result<String, (StatusCode, String)> {
    for counter in 0..4096u32 {
        let candidate = ids::deterministic_hex_id("sensor", mac_eth, mac_wifi, created_at, counter);
        if used.contains(&candidate) {
            continue;
        }
        if sensor_id_exists(db, &candidate)
            .await
            .map_err(map_db_error)?
        {
            continue;
        }
        used.insert(candidate.clone());
        return Ok(candidate);
    }
    Err((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Unable to allocate unique sensor id".to_string(),
    ))
}

async fn upsert_node_agent_sensors_in_core(
    db: &PgPool,
    node_id: Uuid,
    sensors: &[NodeSensorDraft],
) -> Result<Vec<String>, (StatusCode, String)> {
    let mut tx = db.begin().await.map_err(map_db_error)?;
    let existing: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE node_id = $1
          AND deleted_at IS NULL
          AND COALESCE(config->>'source', '') = 'node_agent'
        "#,
    )
    .bind(node_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let desired_ids: std::collections::BTreeSet<String> =
        sensors.iter().map(|s| s.sensor_id.clone()).collect();
    let deleted_ids: Vec<String> = existing
        .into_iter()
        .filter(|id| !desired_ids.contains(id))
        .collect();

    if !deleted_ids.is_empty() {
        let deleted_at = chrono::Utc::now();
        for sensor_id in &deleted_ids {
            sqlx::query(
                r#"
                UPDATE sensors
                SET deleted_at = $2,
                    name = CASE WHEN name LIKE '%-deleted' THEN name ELSE name || '-deleted' END
                WHERE sensor_id = $1
                "#,
            )
            .bind(sensor_id)
            .bind(deleted_at)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }
    }

    for sensor in sensors {
        let config =
            build_core_sensor_config(sensor).map_err(|err| (StatusCode::BAD_REQUEST, err))?;
        let interval_seconds = sensor.interval_seconds.round().max(0.0) as i32;
        let rolling_avg_seconds = sensor.rolling_average_seconds.round().max(0.0) as i32;

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
                deleted_at,
                config
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8)
            ON CONFLICT (sensor_id) DO UPDATE SET
                node_id = EXCLUDED.node_id,
                name = EXCLUDED.name,
                type = EXCLUDED.type,
                unit = EXCLUDED.unit,
                interval_seconds = EXCLUDED.interval_seconds,
                rolling_avg_seconds = EXCLUDED.rolling_avg_seconds,
                deleted_at = NULL,
                config = COALESCE(sensors.config, '{}'::jsonb) || EXCLUDED.config
            "#,
        )
        .bind(&sensor.sensor_id)
        .bind(node_id)
        .bind(&sensor.name)
        .bind(&sensor.preset)
        .bind(&sensor.unit)
        .bind(interval_seconds)
        .bind(rolling_avg_seconds)
        .bind(SqlJson(config))
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }

    tx.commit().await.map_err(map_db_error)?;
    Ok(deleted_ids)
}

#[utoipa::path(
    get,
    path = "/api/nodes/{node_id}/sensors/config",
    tag = "nodes",
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "Node sensor config", body = NodeSensorsConfigResponse),
        (status = 400, description = "Hardware sensors are only supported on node-agent nodes"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn get_node_sensors_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<NodeSensorsConfigResponse>, (StatusCode, String)> {
    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    let Some(row) = load_node_config(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let agent_node_id = row
        .config
        .0
        .get("agent_node_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if agent_node_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Hardware sensors can only be configured on Raspberry Pi nodes.".to_string(),
        ));
    }

    let mut sensors: Option<Vec<NodeSensorDraft>> = row
        .config
        .0
        .get("desired_sensors")
        .and_then(|value| serde_json::from_value::<Vec<NodeSensorDraft>>(value.clone()).ok());

    if sensors.is_none() {
        sensors = Some(
            fetch_core_node_agent_sensors(&state.db, node_uuid)
                .await
                .map_err(map_db_error)?,
        );
    }

    let ads1263: Option<NodeAds1263SettingsDraft> =
        row.config.0.get("desired_ads1263").and_then(|value| {
            serde_json::from_value::<NodeAds1263SettingsDraft>(value.clone()).ok()
        });

    let analog_backend = row
        .config
        .0
        .get("analog_backend")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            let normalized = value.to_lowercase();
            if normalized == "ads1115" {
                "disabled".to_string()
            } else {
                value.to_string()
            }
        });
    let analog_health = row
        .config
        .0
        .get("analog_health")
        .and_then(|value| serde_json::from_value::<NodeAnalogHealth>(value.clone()).ok());

    Ok(Json(NodeSensorsConfigResponse {
        node_id: node_uuid.to_string(),
        sensors: sensors.unwrap_or_default(),
        ads1263,
        analog_backend,
        analog_health,
    }))
}

#[utoipa::path(
    put,
    path = "/api/nodes/{node_id}/sensors/config",
    tag = "nodes",
    request_body = NodeSensorsConfigUpdateRequest,
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "Updated node sensor config", body = ApplyNodeSensorsConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_node_sensors_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<NodeSensorsConfigUpdateRequest>,
) -> Result<Json<ApplyNodeSensorsConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    let Some(mut row) = load_node_config(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let agent_node_id = row
        .config
        .0
        .get("agent_node_id")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if agent_node_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Hardware sensors can only be configured on Raspberry Pi nodes.".to_string(),
        ));
    }

    let mut used_ids = std::collections::BTreeSet::new();
    let created_at = chrono::Utc::now();
    let mac_eth = row.mac_eth.as_deref();
    let mac_wifi = row.mac_wifi.as_deref();

    let desired_ads1263 = payload.ads1263.clone().or_else(|| {
        row.config.0.get("desired_ads1263").and_then(|value| {
            serde_json::from_value::<NodeAds1263SettingsDraft>(value.clone()).ok()
        })
    });

    let mut normalized: Vec<NodeSensorDraft> = Vec::with_capacity(payload.sensors.len());
    for sensor in payload.sensors {
        let sensor =
            normalize_sensor_draft(sensor).map_err(|err| (StatusCode::BAD_REQUEST, err))?;
        normalized.push(sensor);
    }

    let mut by_id = std::collections::BTreeSet::new();
    for sensor in &normalized {
        if sensor.sensor_id.is_empty() {
            continue;
        }
        if !by_id.insert(sensor.sensor_id.clone()) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Duplicate sensor_id {}", sensor.sensor_id),
            ));
        }
        validate_sensor_id_ownership(&state.db, &sensor.sensor_id, node_uuid).await?;
        used_ids.insert(sensor.sensor_id.clone());
    }

    for sensor in &mut normalized {
        if sensor.sensor_id.is_empty() {
            sensor.sensor_id =
                allocate_sensor_id(&state.db, mac_eth, mac_wifi, created_at, &mut used_ids).await?;
        }
    }

    let deleted_sensor_ids =
        upsert_node_agent_sensors_in_core(&state.db, node_uuid, &normalized).await?;

    let root = ensure_object(&mut row.config.0, "node config root")
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    root.insert(
        "desired_sensors".to_string(),
        serde_json::to_value(&normalized).map_err(internal_error)?,
    );
    root.insert(
        "desired_sensors_updated_at".to_string(),
        JsonValue::String(created_at.to_rfc3339()),
    );
    if let Some(ads1263) = &desired_ads1263 {
        root.insert(
            "desired_ads1263".to_string(),
            serde_json::to_value(ads1263).map_err(internal_error)?,
        );
    }

    sqlx::query(
        r#"
        UPDATE nodes
        SET config = $2,
            last_seen = COALESCE(last_seen, NOW())
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .bind(SqlJson(row.config.0.clone()))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut status = "stored".to_string();
    let mut warning = None;
    let mut node_agent_url = None;

    let mut endpoint =
        match crate::services::node_agent_resolver::resolve_node_agent_endpoint(
            &state.db,
            node_uuid,
            state.config.node_agent_port,
            false,
        )
        .await
        {
            Ok(endpoint) => endpoint,
            Err(err) => {
                warning = Some(format!("Failed to locate node-agent endpoint: {err}"));
                None
            }
        };

    if endpoint.is_none() {
        warning = Some(
            "Unable to locate node-agent endpoint (mDNS + last-known IP both missing). Ensure the node is online so sensor config can be pushed."
                .to_string(),
        );
    } else if let Some(token) =
        crate::node_agent_auth::node_agent_bearer_token(&state.db, node_uuid).await
    {
        let auth_header = format!("Bearer {token}");

        async fn fetch_existing_config(
            http: &reqwest::Client,
            base_url: &str,
            auth_header: &str,
        ) -> Result<reqwest::Response, reqwest::Error> {
            let url = format!("{base_url}/v1/config");
            http.get(&url)
                .header(reqwest::header::AUTHORIZATION, auth_header)
                .timeout(std::time::Duration::from_secs(4))
                .send()
                .await
        }

        let mut existing_config = fetch_existing_config(
            &state.http,
            &endpoint.as_ref().unwrap().base_url,
            &auth_header,
        )
        .await;
        if existing_config.is_err() {
            if let Ok(Some(refreshed)) =
                crate::services::node_agent_resolver::resolve_node_agent_endpoint(
                    &state.db,
                    node_uuid,
                    state.config.node_agent_port,
                    true,
                )
                .await
            {
                if refreshed.base_url != endpoint.as_ref().unwrap().base_url {
                    endpoint = Some(refreshed);
                    existing_config = fetch_existing_config(
                        &state.http,
                        &endpoint.as_ref().unwrap().base_url,
                        &auth_header,
                    )
                    .await;
                }
            }
        }
        if existing_config.is_err() {
            let ip_fallback = endpoint.as_ref().and_then(|e| e.ip_fallback.clone());
            if let Some(ip_fallback) = ip_fallback {
                if Some(ip_fallback.as_str()) != endpoint.as_ref().map(|e| e.base_url.as_str()) {
                    existing_config =
                        fetch_existing_config(&state.http, &ip_fallback, &auth_header).await;
                    if existing_config.is_ok() {
                        endpoint.as_mut().unwrap().base_url = ip_fallback;
                        endpoint.as_mut().unwrap().source = "ip_fallback".to_string();
                    }
                }
            }
        }

        let existing_sensors: Option<Vec<JsonValue>> = match existing_config {
            Ok(resp) if resp.status().is_success() => match resp.json::<JsonValue>().await {
                Ok(payload) => payload
                    .get("sensors")
                    .and_then(|value| value.as_array())
                    .map(|arr| arr.iter().cloned().collect()),
                Err(err) => {
                    warning = Some(format!("Node agent config fetch failed: {err}"));
                    None
                }
            },
            Ok(resp) => {
                warning = Some(format!(
                    "Node agent config fetch failed ({}): {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                ));
                None
            }
            Err(err) => {
                warning = Some(format!("Node agent config fetch failed: {err}"));
                None
            }
        };

        if let Some(existing_sensors) = existing_sensors {
            let deleted_ids: std::collections::BTreeSet<&str> =
                deleted_sensor_ids.iter().map(|v| v.as_str()).collect();
            let desired_ids: std::collections::BTreeSet<&str> =
                normalized.iter().map(|v| v.sensor_id.as_str()).collect();
            let mut merged: Vec<JsonValue> = Vec::new();
            for sensor in existing_sensors {
                let Some(sensor_id) = sensor.get("sensor_id").and_then(|v| v.as_str()) else {
                    continue;
                };
                if deleted_ids.contains(sensor_id) || desired_ids.contains(sensor_id) {
                    continue;
                }
                merged.push(sensor);
            }
            for sensor in &normalized {
                merged.push(build_node_agent_sensor_payload(sensor));
            }

            let mut config_payload = serde_json::json!({ "sensors": merged });
            if let Some(ads1263) = &desired_ads1263 {
                let map = config_payload.as_object_mut().unwrap();
                map.insert(
                    "ads1263".to_string(),
                    serde_json::to_value(ads1263).map_err(internal_error)?,
                );
            }

            let base_url = endpoint.as_ref().unwrap().base_url.clone();
            node_agent_url = Some(base_url.clone());

            async fn apply_config(
                http: &reqwest::Client,
                base_url: &str,
                auth_header: &str,
                config_payload: &JsonValue,
            ) -> Result<reqwest::Response, reqwest::Error> {
                let url = format!("{base_url}/v1/config");
                http.put(url)
                    .header(reqwest::header::AUTHORIZATION, auth_header)
                    .timeout(std::time::Duration::from_secs(10))
                    .json(config_payload)
                    .send()
                    .await
            }

            let mut response =
                apply_config(&state.http, &base_url, &auth_header, &config_payload).await;
            if response.is_err() {
                if let Ok(Some(refreshed)) =
                    crate::services::node_agent_resolver::resolve_node_agent_endpoint(
                        &state.db,
                        node_uuid,
                        state.config.node_agent_port,
                        true,
                    )
                    .await
                {
                    if refreshed.base_url != base_url {
                        node_agent_url = Some(refreshed.base_url.clone());
                        response = apply_config(
                            &state.http,
                            &refreshed.base_url,
                            &auth_header,
                            &config_payload,
                        )
                        .await;
                    }
                }
            }
            if response.is_err() {
                if let Some(ip_fallback) = endpoint.as_ref().and_then(|e| e.ip_fallback.as_deref()) {
                    if Some(ip_fallback) != node_agent_url.as_deref() {
                        node_agent_url = Some(ip_fallback.to_string());
                        response =
                            apply_config(&state.http, ip_fallback, &auth_header, &config_payload)
                                .await;
                    }
                }
            }

            match response {
                Ok(resp) if resp.status().is_success() => {
                    status = "applied".to_string();
                    warning = None;
                }
                Ok(resp) => {
                    let status_code = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warning =
                        Some(format!("Node agent rejected update ({status_code}): {body}"));
                }
                Err(err) => {
                    warning = Some(format!("Node agent sync failed: {err}"));
                }
            }
        }
    } else {
        warning = Some(
            "Missing node-agent auth token for this node. Re-provision the node with the controller-issued token so the controller can push sensor config."
                .to_string(),
        );
    }

    let last_apply_at = chrono::Utc::now();
    let root = ensure_object(&mut row.config.0, "node config root")
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    root.insert(
        "node_sensors_last_apply_status".to_string(),
        JsonValue::String(status.clone()),
    );
    root.insert(
        "node_sensors_last_apply_at".to_string(),
        JsonValue::String(last_apply_at.to_rfc3339()),
    );
    root.insert(
        "node_sensors_last_apply_warning".to_string(),
        warning
            .clone()
            .map(JsonValue::String)
            .unwrap_or(JsonValue::Null),
    );

    sqlx::query(
        r#"
        UPDATE nodes
        SET config = $2
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .bind(SqlJson(row.config.0))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(ApplyNodeSensorsConfigResponse {
        status,
        node_id: node_uuid.to_string(),
        node_agent_url,
        sensors: normalized,
        deleted_sensor_ids,
        warning,
    }))
}

#[utoipa::path(
    put,
    path = "/api/nodes/{node_id}/sensors/order",
    tag = "nodes",
    request_body = NodeSensorsOrderUpdateRequest,
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 204, description = "Updated sensor order"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub async fn update_node_sensors_order(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<NodeSensorsOrderUpdateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    if payload.sensor_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "sensor_ids must contain at least one sensor id".to_string(),
        ));
    }

    let mut seen = HashSet::<String>::new();
    let mut sensor_ids: Vec<String> = Vec::with_capacity(payload.sensor_ids.len());
    for raw in payload.sensor_ids {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string()) {
            sensor_ids.push(trimmed.to_string());
        }
    }

    if sensor_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "sensor_ids must contain at least one valid sensor id".to_string(),
        ));
    }

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let node_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM nodes WHERE id = $1)")
        .bind(node_uuid)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;
    if !node_exists {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    }

    let found: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE node_id = $1
          AND deleted_at IS NULL
          AND sensor_id = ANY($2)
        "#,
    )
    .bind(node_uuid)
    .bind(&sensor_ids)
    .fetch_all(&mut *tx)
    .await
    .map_err(map_db_error)?;
    let found_set: HashSet<String> = found.into_iter().collect();
    let missing: Vec<String> = sensor_ids
        .iter()
        .filter(|sensor_id| !found_set.contains(*sensor_id))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Sensors not found on node: {}", missing.join(", ")),
        ));
    }

    sqlx::query(
        r#"
        WITH ordered AS (
            SELECT sensor_id, ord::integer AS ui_order
            FROM unnest($2::text[]) WITH ORDINALITY AS t(sensor_id, ord)
        ),
        remaining AS (
            SELECT
                sensors.sensor_id,
                (SELECT COALESCE(MAX(ui_order), 0) FROM ordered)
                  + row_number() OVER (
                      ORDER BY sensors.ui_order NULLS LAST, sensors.created_at ASC, sensors.sensor_id ASC
                    ) AS ui_order
            FROM sensors
            WHERE sensors.node_id = $1
              AND sensors.deleted_at IS NULL
              AND sensors.sensor_id NOT IN (SELECT sensor_id FROM ordered)
        ),
        combined AS (
            SELECT sensor_id, ui_order FROM ordered
            UNION ALL
            SELECT sensor_id, ui_order FROM remaining
        )
        UPDATE sensors
        SET ui_order = combined.ui_order
        FROM combined
        WHERE sensors.node_id = $1
          AND sensors.sensor_id = combined.sensor_id
        "#,
    )
    .bind(node_uuid)
    .bind(&sensor_ids)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/nodes/{node_id}/sensors/config",
            get(get_node_sensors_config).put(update_node_sensors_config),
        )
        .route(
            "/nodes/{node_id}/sensors/order",
            put(update_node_sensors_order),
        )
}
