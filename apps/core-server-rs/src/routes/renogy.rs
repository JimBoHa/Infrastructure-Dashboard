use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use rand::RngCore;
use reqwest::header::AUTHORIZATION;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::presets;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RenogyBt2Mode {
    Ble,
    External,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ApplyRenogyBt2PresetRequest {
    pub(crate) bt2_address: String,
    pub(crate) poll_interval_seconds: Option<i32>,
    pub(crate) mode: Option<RenogyBt2Mode>,
    pub(crate) adapter: Option<String>,
    pub(crate) unit_id: Option<i32>,
    pub(crate) device_name: Option<String>,
    pub(crate) request_timeout_seconds: Option<i32>,
    pub(crate) connect_timeout_seconds: Option<i32>,
    pub(crate) service_uuid: Option<String>,
    pub(crate) write_uuid: Option<String>,
    pub(crate) notify_uuid: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RenogyPresetSensor {
    pub(crate) sensor_id: String,
    pub(crate) name: String,
    pub(crate) metric: String,
    #[serde(rename = "type")]
    pub(crate) core_type: String,
    pub(crate) unit: String,
    pub(crate) interval_seconds: i32,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ApplyRenogyBt2PresetResponse {
    pub(crate) status: String,
    pub(crate) node_id: String,
    pub(crate) node_agent_url: Option<String>,
    pub(crate) bt2_address: String,
    pub(crate) mode: RenogyBt2Mode,
    pub(crate) poll_interval_seconds: i32,
    pub(crate) warning: Option<String>,
    pub(crate) sensors: Vec<RenogyPresetSensor>,
    pub(crate) what_to_check: Vec<String>,
}

fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

fn renogy_sensor_id(node_id: Uuid, metric: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(node_id.as_bytes());
    hasher.update(b":renogy_bt2:");
    hasher.update(metric.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(24);
    for byte in digest.iter().take(12) {
        use std::fmt::Write;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

async fn sensor_id_conflicts(
    db: &PgPool,
    sensor_id: &str,
    node_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let existing: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT node_id
        FROM sensors
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id)
    .fetch_optional(db)
    .await?;
    Ok(existing.is_some_and(|existing| existing != node_id))
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

fn ensure_array<'a>(
    value: &'a mut JsonValue,
    context: &str,
) -> Result<&'a mut Vec<JsonValue>, String> {
    match value {
        JsonValue::Array(arr) => Ok(arr),
        JsonValue::Null => {
            *value = JsonValue::Array(Vec::new());
            match value {
                JsonValue::Array(arr) => Ok(arr),
                _ => unreachable!("inserted JsonValue::Array but pattern did not match"),
            }
        }
        other => Err(format!(
            "Expected JSON array for {context}, got {}",
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

fn normalize_address(raw: &str) -> String {
    raw.trim().to_uppercase()
}

fn mode_string(mode: &RenogyBt2Mode) -> &'static str {
    match mode {
        RenogyBt2Mode::Ble => "ble",
        RenogyBt2Mode::External => "external",
    }
}

fn generate_ingest_token() -> String {
    let mut buf = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    sha256_hex(&buf)[..16].to_string()
}

async fn push_node_agent_config(
    client: &reqwest::Client,
    node_agent_url: &str,
    auth_header: &str,
    payload: &JsonValue,
) -> Result<(), String> {
    let url = format!("{node_agent_url}/v1/config");
    let response = client
        .put(url)
        .header(AUTHORIZATION, auth_header)
        .timeout(Duration::from_secs(15))
        .json(payload)
        .send()
        .await
        .map_err(|err| format!("Node agent request failed: {err}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Node agent error ({status}): {body}"));
    }
    Ok(())
}

fn ensure_renogy_config(
    config: &mut JsonValue,
    address: &str,
    interval_seconds: i32,
    mode: &RenogyBt2Mode,
    adapter: Option<&str>,
    unit_id: Option<i32>,
    device_name: Option<&str>,
    request_timeout_seconds: Option<i32>,
    connect_timeout_seconds: Option<i32>,
    service_uuid: Option<&str>,
    write_uuid: Option<&str>,
    notify_uuid: Option<&str>,
) -> Result<bool, String> {
    let mut changed = false;
    let root = ensure_object(config, "node-agent config root")?;
    let renogy = root
        .entry("renogy_bt2".to_string())
        .or_insert(JsonValue::Object(serde_json::Map::new()));
    let renogy_obj = ensure_object(renogy, "node-agent config.renogy_bt2")?;

    let desired_mode = JsonValue::String(mode_string(mode).to_string());
    if renogy_obj.get("mode") != Some(&desired_mode) {
        renogy_obj.insert("mode".to_string(), desired_mode);
        changed = true;
    }
    let enabled = JsonValue::Bool(true);
    if renogy_obj.get("enabled") != Some(&enabled) {
        renogy_obj.insert("enabled".to_string(), enabled);
        changed = true;
    }
    let desired_addr = JsonValue::String(address.to_string());
    if renogy_obj.get("address") != Some(&desired_addr) {
        renogy_obj.insert("address".to_string(), desired_addr);
        changed = true;
    }

    let desired_interval = JsonValue::Number(interval_seconds.into());
    if renogy_obj.get("poll_interval_seconds") != Some(&desired_interval) {
        renogy_obj.insert("poll_interval_seconds".to_string(), desired_interval);
        changed = true;
    }

    if let Some(unit_id) = unit_id {
        let unit_id = unit_id.clamp(1, 255);
        let desired_unit_id = JsonValue::Number(unit_id.into());
        if renogy_obj.get("unit_id") != Some(&desired_unit_id) {
            renogy_obj.insert("unit_id".to_string(), desired_unit_id);
            changed = true;
        }
    }

    if let Some(name) = device_name.filter(|v| !v.trim().is_empty()) {
        let desired = JsonValue::String(name.trim().to_string());
        if renogy_obj.get("device_name") != Some(&desired) {
            renogy_obj.insert("device_name".to_string(), desired);
            changed = true;
        }
    }

    if let Some(timeout) = request_timeout_seconds {
        let timeout = timeout.clamp(1, 60);
        let desired = JsonValue::Number(timeout.into());
        if renogy_obj.get("request_timeout_seconds") != Some(&desired) {
            renogy_obj.insert("request_timeout_seconds".to_string(), desired);
            changed = true;
        }
    }

    if let Some(timeout) = connect_timeout_seconds {
        let timeout = timeout.clamp(1, 120);
        let desired = JsonValue::Number(timeout.into());
        if renogy_obj.get("connect_timeout_seconds") != Some(&desired) {
            renogy_obj.insert("connect_timeout_seconds".to_string(), desired);
            changed = true;
        }
    }

    if let Some(adapter) = adapter.filter(|v| !v.trim().is_empty()) {
        let desired = JsonValue::String(adapter.trim().to_string());
        if renogy_obj.get("adapter") != Some(&desired) {
            renogy_obj.insert("adapter".to_string(), desired);
            changed = true;
        }
    }

    if let Some(uuid) = service_uuid.filter(|v| !v.trim().is_empty()) {
        let desired = JsonValue::String(uuid.trim().to_string());
        if renogy_obj.get("service_uuid") != Some(&desired) {
            renogy_obj.insert("service_uuid".to_string(), desired);
            changed = true;
        }
    }

    if let Some(uuid) = write_uuid.filter(|v| !v.trim().is_empty()) {
        let desired = JsonValue::String(uuid.trim().to_string());
        if renogy_obj.get("write_uuid") != Some(&desired) {
            renogy_obj.insert("write_uuid".to_string(), desired);
            changed = true;
        }
    }

    if let Some(uuid) = notify_uuid.filter(|v| !v.trim().is_empty()) {
        let desired = JsonValue::String(uuid.trim().to_string());
        if renogy_obj.get("notify_uuid") != Some(&desired) {
            renogy_obj.insert("notify_uuid".to_string(), desired);
            changed = true;
        }
    }

    if matches!(mode, RenogyBt2Mode::External) {
        let token = renogy_obj
            .get("ingest_token")
            .and_then(|val| val.as_str())
            .unwrap_or("")
            .trim();
        if token.is_empty() {
            renogy_obj.insert(
                "ingest_token".to_string(),
                JsonValue::String(generate_ingest_token()),
            );
            changed = true;
        }
    }

    Ok(changed)
}

fn ensure_renogy_sensors(
    config: &mut JsonValue,
    node_id: Uuid,
    interval_seconds: i32,
) -> Result<bool, String> {
    let mut changed = false;
    let root = ensure_object(config, "node-agent config root")?;
    let sensors_value = root
        .entry("sensors".to_string())
        .or_insert(JsonValue::Array(Vec::new()));
    let sensors = ensure_array(sensors_value, "node-agent config.sensors")?;

    let mut existing_by_metric: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, sensor) in sensors.iter().enumerate() {
        if let Some(metric) = sensor.get("metric").and_then(|val| val.as_str()) {
            existing_by_metric.insert(metric.trim().to_string(), idx);
        }
    }

    for def in presets::renogy_bt2_sensors() {
        if let Some(idx) = existing_by_metric.get(def.metric.as_str()).copied() {
            let Some(sensor) = sensors.get_mut(idx) else {
                continue;
            };
            let obj = ensure_object(sensor, "node-agent config.sensors[] entry")?;
            if obj.get("type").and_then(|v| v.as_str()) != Some("renogy_bt2") {
                obj.insert(
                    "type".to_string(),
                    JsonValue::String("renogy_bt2".to_string()),
                );
                changed = true;
            }
            if obj.get("name").and_then(|v| v.as_str()) != Some(def.name.as_str()) {
                obj.insert("name".to_string(), JsonValue::String(def.name.clone()));
                changed = true;
            }
            if obj.get("unit").and_then(|v| v.as_str()) != Some(def.unit.as_str()) {
                obj.insert("unit".to_string(), JsonValue::String(def.unit.clone()));
                changed = true;
            }
            let desired_interval = JsonValue::Number(interval_seconds.into());
            if obj.get("interval_seconds") != Some(&desired_interval) {
                obj.insert("interval_seconds".to_string(), desired_interval);
                changed = true;
            }
            if obj.get("rolling_average_seconds").is_none() {
                obj.insert(
                    "rolling_average_seconds".to_string(),
                    JsonValue::Number(0.into()),
                );
                changed = true;
            }
            if obj.get("channel").is_none() {
                obj.insert("channel".to_string(), JsonValue::Number(0.into()));
                changed = true;
            }
            if obj
                .get("sensor_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                obj.insert(
                    "sensor_id".to_string(),
                    JsonValue::String(renogy_sensor_id(node_id, def.metric.as_str())),
                );
                changed = true;
            }
        } else {
            sensors.push(serde_json::json!({
                "sensor_id": renogy_sensor_id(node_id, def.metric.as_str()),
                "name": def.name.as_str(),
                "type": "renogy_bt2",
                "channel": 0,
                "unit": def.unit.as_str(),
                "interval_seconds": interval_seconds,
                "rolling_average_seconds": 0,
                "metric": def.metric.as_str(),
            }));
            changed = true;
        }
    }

    Ok(changed)
}

fn collect_renogy_sensors(config: &JsonValue, interval_seconds: i32) -> Vec<RenogyPresetSensor> {
    let mut ids_by_metric: BTreeMap<&str, String> = BTreeMap::new();
    if let Some(arr) = config.get("sensors").and_then(|v| v.as_array()) {
        for sensor in arr {
            let metric = sensor
                .get("metric")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let sensor_id = sensor
                .get("sensor_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if metric.is_empty() || sensor_id.is_empty() {
                continue;
            }
            ids_by_metric.insert(metric, sensor_id.to_string());
        }
    }

    presets::renogy_bt2_sensors()
        .iter()
        .map(|def| RenogyPresetSensor {
            sensor_id: ids_by_metric
                .get(def.metric.as_str())
                .cloned()
                .unwrap_or_default(),
            name: def.name.clone(),
            metric: def.metric.clone(),
            core_type: def.core_type.clone(),
            unit: def.unit.clone(),
            interval_seconds,
        })
        .collect()
}

async fn ensure_unique_sensor_ids_in_config(
    db: &PgPool,
    config: &mut JsonValue,
    node_id: Uuid,
) -> Result<bool, (StatusCode, String)> {
    let mut changed = false;
    let root = ensure_object(config, "node-agent config root")
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    let sensors_value = root
        .entry("sensors".to_string())
        .or_insert(JsonValue::Array(Vec::new()));
    let sensors = ensure_array(sensors_value, "node-agent config.sensors")
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;

    let mut seen: BTreeSet<String> = BTreeSet::new();
    for sensor in sensors.iter_mut() {
        let obj = match sensor.as_object_mut() {
            Some(obj) => obj,
            None => continue,
        };
        let metric = obj
            .get("metric")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if metric.is_empty() {
            continue;
        }
        let sensor_id_raw = obj
            .get("sensor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if sensor_id_raw.is_empty() {
            continue;
        }
        let mut sensor_id = sensor_id_raw.to_string();

        if !seen.insert(sensor_id.clone())
            || sensor_id_conflicts(db, &sensor_id, node_id)
                .await
                .map_err(map_db_error)?
        {
            let mut counter = 1u32;
            loop {
                let candidate = renogy_sensor_id(node_id, &format!("{metric}:{counter}"));
                if seen.contains(&candidate) {
                    counter += 1;
                    continue;
                }
                if sensor_id_conflicts(db, &candidate, node_id)
                    .await
                    .map_err(map_db_error)?
                {
                    counter += 1;
                    continue;
                }
                sensor_id = candidate;
                break;
            }
            obj.insert(
                "sensor_id".to_string(),
                JsonValue::String(sensor_id.clone()),
            );
            seen.insert(sensor_id.clone());
            changed = true;
        }
    }
    Ok(changed)
}

async fn upsert_renogy_sensors_in_core(
    db: &PgPool,
    node_id: Uuid,
    sensors: &[RenogyPresetSensor],
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    for sensor in sensors {
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
            VALUES ($1, $2, $3, $4, $5, $6, 0, jsonb_build_object('source', 'renogy_bt2', 'metric', $7))
            ON CONFLICT (sensor_id) DO UPDATE SET
                node_id = EXCLUDED.node_id,
                name = EXCLUDED.name,
                type = EXCLUDED.type,
                unit = EXCLUDED.unit,
                interval_seconds = EXCLUDED.interval_seconds,
                config = EXCLUDED.config
            "#,
        )
        .bind(&sensor.sensor_id)
        .bind(node_id)
        .bind(&sensor.name)
        .bind(&sensor.core_type)
        .bind(&sensor.unit)
        .bind(sensor.interval_seconds)
        .bind(&sensor.metric)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/nodes/{node_id}/presets/renogy-bt2",
    tag = "nodes",
    request_body = ApplyRenogyBt2PresetRequest,
    params(("node_id" = String, Path, description = "Node id (UUID)")),
    responses(
        (status = 200, description = "Renogy preset applied", body = ApplyRenogyBt2PresetResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn apply_renogy_bt2_preset(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<ApplyRenogyBt2PresetRequest>,
) -> Result<Json<ApplyRenogyBt2PresetResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    let bt2_address = normalize_address(&payload.bt2_address);
    if bt2_address.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "bt2_address is required".to_string(),
        ));
    }
    let poll_interval_seconds = payload
        .poll_interval_seconds
        .unwrap_or(presets::renogy_bt2_default_interval_seconds())
        .clamp(5, 3600);
    let mode = payload.mode.unwrap_or(RenogyBt2Mode::Ble);

    let row: Option<(sqlx::types::Json<JsonValue>, Option<String>, String)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(config, '{}'::jsonb) as config,
            host(ip_last) as ip_last,
            status
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some((config, _ip_last, node_status)) = row else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };
    let mut config = config.0;

    let mut changed = ensure_renogy_config(
        &mut config,
        &bt2_address,
        poll_interval_seconds,
        &mode,
        payload.adapter.as_deref(),
        payload.unit_id,
        payload.device_name.as_deref(),
        payload.request_timeout_seconds,
        payload.connect_timeout_seconds,
        payload.service_uuid.as_deref(),
        payload.write_uuid.as_deref(),
        payload.notify_uuid.as_deref(),
    )
    .map_err(|err| (StatusCode::BAD_REQUEST, err))?;

    let sensors_changed = ensure_renogy_sensors(&mut config, node_uuid, poll_interval_seconds)
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    changed = changed || sensors_changed;

    let ids_changed = ensure_unique_sensor_ids_in_config(&state.db, &mut config, node_uuid).await?;
    changed = changed || ids_changed;

    sqlx::query(
        r#"
        UPDATE nodes
        SET config = $2,
            last_seen = COALESCE(last_seen, NOW())
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .bind(sqlx::types::Json(config.clone()))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut warning = None;
    let mut node_agent_url = None;
    if node_status == "online" {
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
                "Unable to locate node-agent endpoint (mDNS + last-known IP both missing). Keep it online and refresh."
                    .to_string(),
            );
        } else if let Some(token) =
            crate::node_agent_auth::node_agent_bearer_token(&state.db, node_uuid).await
        {
            let auth_header = format!("Bearer {token}");

            // IMPORTANT: Do not overwrite the node's full `sensors` list with the controller's
            // `nodes.config.sensors` snapshot. The controller tracks hardware sensors separately
            // (`desired_sensors`), so `nodes.config.sensors` may contain only Renogy sensors.
            // Fetch the node-agent config and merge Renogy sensors into it so we never wipe
            // ADS1263/analog sensors.
            async fn fetch_existing_config(
                http: &reqwest::Client,
                base_url: &str,
                auth_header: &str,
            ) -> Result<reqwest::Response, reqwest::Error> {
                http.get(format!("{base_url}/v1/config"))
                    .header(AUTHORIZATION, auth_header)
                    .timeout(Duration::from_secs(4))
                    .send()
                    .await
            }

            let mut existing_config =
                fetch_existing_config(
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
                    if Some(ip_fallback.as_str())
                        != endpoint.as_ref().map(|e| e.base_url.as_str())
                    {
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
                    Ok(resp) if resp.status().is_success() => {
                        match resp.json::<JsonValue>().await {
                            Ok(payload) => payload
                                .get("sensors")
                                .and_then(|value| value.as_array())
                                .map(|arr| arr.iter().cloned().collect()),
                            Err(err) => {
                                warning = Some(format!("Node agent config fetch failed: {err}"));
                                None
                            }
                        }
                    }
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
                    let mut merged_config = serde_json::json!({ "sensors": existing_sensors });
                    if let Err(err) =
                        ensure_renogy_sensors(&mut merged_config, node_uuid, poll_interval_seconds)
                    {
                        warning = Some(format!(
                            "Failed to merge Renogy sensors into node config: {err}"
                        ));
                    } else {
                        let payload = serde_json::json!({
                            "renogy_bt2": config.get("renogy_bt2"),
                            "sensors": merged_config.get("sensors"),
                        });
                        let mut base_url = endpoint.as_ref().unwrap().base_url.clone();
                        node_agent_url = Some(base_url.clone());

                        let mut pushed = push_node_agent_config(
                            &state.http,
                            &base_url,
                            &auth_header,
                            &payload,
                        )
                        .await;
                        if pushed.is_err() {
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
                                    base_url = refreshed.base_url.clone();
                                    node_agent_url = Some(base_url.clone());
                                    pushed = push_node_agent_config(
                                        &state.http,
                                        &base_url,
                                        &auth_header,
                                        &payload,
                                    )
                                    .await;
                                }
                            }
                        }
                        if pushed.is_err() {
                            if let Some(ip_fallback) =
                                endpoint.as_ref().and_then(|e| e.ip_fallback.clone())
                            {
                                if Some(ip_fallback.as_str()) != node_agent_url.as_deref() {
                                    node_agent_url = Some(ip_fallback.clone());
                                    pushed = push_node_agent_config(
                                        &state.http,
                                        &ip_fallback,
                                        &auth_header,
                                        &payload,
                                    )
                                    .await;
                                }
                            }
                        }
                        if let Err(err) = pushed {
                            warning = Some(format!("Node agent sync failed: {err}"));
                        }
                    }
                }
        } else {
            warning = Some(
                "Missing node-agent auth token for this node. Re-provision the node with the controller-issued token so the controller can apply Renogy settings."
                    .to_string(),
            );
        }
    } else {
        warning = Some(
            "Node is offline. Settings are saved and will apply when the node is back online."
                .to_string(),
        );
    }

    let sensors = collect_renogy_sensors(&config, poll_interval_seconds);
    upsert_renogy_sensors_in_core(&state.db, node_uuid, &sensors)
        .await
        .map_err(map_db_error)?;

    let what_to_check = vec![
        "Confirm the BT-2 module is powered and within a few feet of the node.".to_string(),
        "Confirm Bluetooth is enabled on the node (and the correct adapter is selected if multiple exist)."
            .to_string(),
        "If running in BLE mode, ensure the node has the BLE runtime dependencies installed (bleak/BlueZ)."
            .to_string(),
        "If no data arrives, open the node-agent logs and look for Renogy BT-2 connection errors."
            .to_string(),
    ];

    Ok(Json(ApplyRenogyBt2PresetResponse {
        status: if warning.is_some() {
            "stored".to_string()
        } else if changed {
            "applied".to_string()
        } else {
            "already_configured".to_string()
        },
        node_id: node_uuid.to_string(),
        node_agent_url,
        bt2_address,
        mode,
        poll_interval_seconds,
        warning,
        sensors,
        what_to_check,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/nodes/{node_id}/presets/renogy-bt2",
        post(apply_renogy_bt2_preset),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_renogy_sensors_does_not_drop_existing_non_metric_sensors() {
        let node_id = Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap();
        let mut config = serde_json::json!({
            "sensors": [
                {
                    "sensor_id": "deadbeefdeadbeefdeadbeef",
                    "name": "ADC0 Voltage",
                    "type": "analog",
                    "channel": 0,
                    "unit": "V",
                    "interval_seconds": 1.0,
                    "rolling_average_seconds": 0.0
                }
            ]
        });

        ensure_renogy_sensors(&mut config, node_id, 30).expect("renogy merge should succeed");

        let sensors = config
            .get("sensors")
            .and_then(|v| v.as_array())
            .expect("sensors array");

        assert!(
            sensors.iter().any(|sensor| {
                sensor.get("sensor_id").and_then(|v| v.as_str()) == Some("deadbeefdeadbeefdeadbeef")
            }),
            "existing analog sensor should remain in merged list"
        );
        assert!(
            sensors
                .iter()
                .any(|sensor| sensor.get("type").and_then(|v| v.as_str()) == Some("renogy_bt2")),
            "renogy sensors should be present after merge"
        );
    }
}
