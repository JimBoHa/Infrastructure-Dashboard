use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

const DEVICE_TYPE: &str = "renogy_bt2";
const REGISTER_MAP_JSON: &str =
    include_str!("../../../../shared/renogy/register_maps/rng_ctrl_rvr20_us_bt2.json");

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RenogyRegisterMapSchema {
    pub(crate) schema: JsonValue,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RenogyDesiredSettingsResponse {
    pub(crate) node_id: String,
    pub(crate) device_type: String,
    pub(crate) desired: JsonValue,
    pub(crate) pending: bool,
    pub(crate) desired_updated_at: String,
    pub(crate) last_applied: Option<JsonValue>,
    pub(crate) last_applied_at: Option<String>,
    pub(crate) last_apply_status: Option<String>,
    pub(crate) last_apply_result: Option<JsonValue>,
    pub(crate) apply_requested: bool,
    pub(crate) apply_requested_at: Option<String>,
    pub(crate) maintenance_mode: bool,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct RenogyDesiredSettingsUpdateRequest {
    pub(crate) desired: JsonValue,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct RenogyMaintenanceModeRequest {
    pub(crate) enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RenogyValidateResponse {
    pub(crate) ok: bool,
    pub(crate) errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RenogyReadCurrentResponse {
    pub(crate) current: JsonValue,
    pub(crate) provider_status: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RenogyApplyResponse {
    pub(crate) status: String,
    pub(crate) result: JsonValue,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RenogyHistoryEntry {
    pub(crate) id: i64,
    pub(crate) event_type: String,
    pub(crate) created_at: String,
    pub(crate) desired: Option<JsonValue>,
    pub(crate) current: Option<JsonValue>,
    pub(crate) diff: Option<JsonValue>,
    pub(crate) result: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct RenogyRollbackRequest {
    pub(crate) event_id: i64,
}

#[derive(sqlx::FromRow)]
struct DesiredRow {
    desired: JsonValue,
    pending: bool,
    desired_updated_at: chrono::DateTime<chrono::Utc>,
    last_applied: Option<JsonValue>,
    last_applied_at: Option<chrono::DateTime<chrono::Utc>>,
    last_apply_status: Option<String>,
    last_apply_result: Option<JsonValue>,
    apply_requested: bool,
    apply_requested_at: Option<chrono::DateTime<chrono::Utc>>,
    maintenance_mode: bool,
}

#[derive(sqlx::FromRow)]
struct DesiredForApplyRow {
    desired: JsonValue,
    last_applied: Option<JsonValue>,
    maintenance_mode: bool,
}

#[derive(sqlx::FromRow)]
struct NodeStatusRow {
    status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NodeAgentRegisterWrite {
    address: u16,
    values: Vec<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NodeAgentApplyRequest {
    writes: Vec<NodeAgentRegisterWrite>,
    verify: bool,
}

fn advisory_lock_key(namespace: &str, value: &str) -> i64 {
    fn fnv1a_64(input: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in input.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    let combined = format!("{namespace}:{value}");
    fnv1a_64(&combined) as i64
}

fn parse_register_map() -> Result<JsonValue, (StatusCode, String)> {
    serde_json::from_str::<JsonValue>(REGISTER_MAP_JSON).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to parse register-map JSON: {err}"),
        )
    })
}

fn validate_desired_against_schema(desired: &JsonValue, schema: &JsonValue) -> Vec<String> {
    let mut errors = Vec::new();
    let desired_obj = match desired.as_object() {
        Some(obj) => obj,
        None => {
            errors.push("desired must be a JSON object".to_string());
            return errors;
        }
    };

    let mut fields_by_key = std::collections::HashMap::<String, JsonValue>::new();
    if let Some(fields) = schema.get("fields").and_then(|v| v.as_array()) {
        for field in fields {
            if let Some(key) = field.get("key").and_then(|v| v.as_str()) {
                fields_by_key.insert(key.to_string(), field.clone());
            }
        }
    }

    for (key, value) in desired_obj {
        let field = match fields_by_key.get(key) {
            Some(field) => field,
            None => {
                errors.push(format!("Unknown setting key: {key}"));
                continue;
            }
        };
        let writable = field
            .get("writable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !writable {
            errors.push(format!("Setting is read-only: {key}"));
            continue;
        }
        let as_number = value.as_f64();
        if as_number.is_none() {
            errors.push(format!("Setting value must be numeric: {key}"));
            continue;
        }
        let num = as_number.unwrap();
        if let Some(min) = field.get("min").and_then(|v| v.as_f64()) {
            if num < min {
                errors.push(format!("Setting {key} is below min ({num} < {min})"));
            }
        }
        if let Some(max) = field.get("max").and_then(|v| v.as_f64()) {
            if num > max {
                errors.push(format!("Setting {key} is above max ({num} > {max})"));
            }
        }
    }

    errors
}

fn parse_hex_address(value: &str) -> Option<u16> {
    let trimmed = value.trim();
    if let Some(without_prefix) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u16::from_str_radix(without_prefix, 16).ok();
    }
    trimmed.parse::<u16>().ok()
}

#[derive(Debug, Clone)]
struct RegisterField {
    key: String,
    label: String,
    address: u16,
    count: u16,
    scale: f64,
}

fn extract_register_fields(schema: &JsonValue) -> Result<Vec<RegisterField>, (StatusCode, String)> {
    let fields = schema.get("fields").and_then(|v| v.as_array()).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "schema missing fields[]".to_string(),
    ))?;

    let mut out = Vec::new();
    for field in fields {
        let key = field.get("key").and_then(|v| v.as_str()).ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "register field missing key".to_string(),
        ))?;
        let label = field.get("label").and_then(|v| v.as_str()).unwrap_or(key);
        let addr_str = field.get("address").and_then(|v| v.as_str()).ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Missing address for key {key}"),
        ))?;
        let address = parse_hex_address(addr_str).ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Invalid address for key {key}: {addr_str}"),
        ))?;
        let count = field.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
        if count == 0 || count > 64 {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Invalid register count for key {key}: {count}"),
            ));
        }
        let scale = field.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
        out.push(RegisterField {
            key: key.to_string(),
            label: label.to_string(),
            address,
            count: count as u16,
            scale,
        });
    }

    Ok(out)
}

fn build_diff(old: &JsonValue, new: &JsonValue) -> JsonValue {
    let old_map = old.as_object();
    let new_map = new.as_object();

    let mut keys = std::collections::BTreeSet::new();
    if let Some(map) = old_map {
        keys.extend(map.keys().cloned());
    }
    if let Some(map) = new_map {
        keys.extend(map.keys().cloned());
    }

    let mut diff = serde_json::Map::new();
    for key in keys {
        let old_val = old_map.and_then(|m| m.get(&key));
        let new_val = new_map.and_then(|m| m.get(&key));
        if old_val == new_val {
            continue;
        }
        let mut entry = serde_json::Map::new();
        entry.insert(
            "from".to_string(),
            old_val.cloned().unwrap_or(JsonValue::Null),
        );
        entry.insert(
            "to".to_string(),
            new_val.cloned().unwrap_or(JsonValue::Null),
        );
        diff.insert(key, JsonValue::Object(entry));
    }

    JsonValue::Object(diff)
}

fn build_node_agent_writes(
    desired: &JsonValue,
    schema: &JsonValue,
) -> Result<Vec<NodeAgentRegisterWrite>, String> {
    let desired_obj = desired
        .as_object()
        .ok_or_else(|| "desired must be a JSON object".to_string())?;

    let fields = schema
        .get("fields")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "schema missing fields[]".to_string())?;

    let mut fields_by_key = std::collections::HashMap::<String, JsonValue>::new();
    for field in fields {
        if let Some(key) = field.get("key").and_then(|v| v.as_str()) {
            fields_by_key.insert(key.to_string(), field.clone());
        }
    }

    let mut writes = Vec::new();
    for (key, value) in desired_obj {
        let field = fields_by_key
            .get(key)
            .ok_or_else(|| format!("Unknown setting key: {key}"))?;
        let writable = field
            .get("writable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !writable {
            continue;
        }
        let addr_str = field
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Missing address for key {key}"))?;
        let address = parse_hex_address(addr_str)
            .ok_or_else(|| format!("Invalid address for key {key}: {addr_str}"))?;
        let scale = field.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let raw = value
            .as_f64()
            .ok_or_else(|| format!("Setting value must be numeric: {key}"))?;
        let unscaled = (raw / scale).round();
        if !unscaled.is_finite() {
            return Err(format!("Setting value is not finite: {key}"));
        }
        let int_val = unscaled as i64;
        if int_val < 0 || int_val > i64::from(u16::MAX) {
            return Err(format!("Setting value out of range for u16: {key}"));
        }
        let label = field
            .get("label")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        writes.push(NodeAgentRegisterWrite {
            address,
            values: vec![int_val as u16],
            description: label,
        });
    }

    Ok(writes)
}

async fn ensure_device_settings_row(db: &PgPool, node_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO device_settings (node_id, device_type)
        VALUES ($1, $2)
        ON CONFLICT (node_id, device_type) DO NOTHING
        "#,
    )
    .bind(node_id)
    .bind(DEVICE_TYPE)
    .execute(db)
    .await?;
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/schema",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Register-map schema", body = RenogyRegisterMapSchema),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn get_register_map_schema(
    Path(_node_id): Path<String>,
    AuthUser(_user): AuthUser,
) -> Result<Json<RenogyRegisterMapSchema>, (StatusCode, String)> {
    let schema = parse_register_map()?;
    Ok(Json(RenogyRegisterMapSchema { schema }))
}

#[utoipa::path(
    get,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/desired",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Desired settings", body = RenogyDesiredSettingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn get_desired_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(_user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<RenogyDesiredSettingsResponse>, (StatusCode, String)> {
    let node_uuid = Uuid::parse_str(&node_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;

    let exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT 1
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    }

    ensure_device_settings_row(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?;
    let row: DesiredRow = sqlx::query_as(
        r#"
        SELECT desired,
               pending,
               desired_updated_at,
               last_applied,
               last_applied_at,
               last_apply_status,
               last_apply_result,
               COALESCE(apply_requested, false) as apply_requested,
               apply_requested_at,
               COALESCE(maintenance_mode, false) as maintenance_mode
        FROM device_settings
        WHERE node_id = $1 AND device_type = $2
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(RenogyDesiredSettingsResponse {
        node_id,
        device_type: DEVICE_TYPE.to_string(),
        desired: row.desired,
        pending: row.pending,
        desired_updated_at: row.desired_updated_at.to_rfc3339(),
        last_applied: row.last_applied,
        last_applied_at: row.last_applied_at.map(|v| v.to_rfc3339()),
        last_apply_status: row.last_apply_status,
        last_apply_result: row.last_apply_result,
        apply_requested: row.apply_requested,
        apply_requested_at: row.apply_requested_at.map(|v| v.to_rfc3339()),
        maintenance_mode: row.maintenance_mode,
    }))
}

#[utoipa::path(
    put,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/desired",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    request_body = RenogyDesiredSettingsUpdateRequest,
    responses(
        (status = 200, description = "Updated desired settings", body = RenogyDesiredSettingsResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn put_desired_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<RenogyDesiredSettingsUpdateRequest>,
) -> Result<Json<RenogyDesiredSettingsResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(&node_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;

    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM nodes WHERE id = $1")
        .bind(node_uuid)
        .fetch_optional(&state.db)
        .await
        .map_err(map_db_error)?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    }

    let schema = parse_register_map()?;
    let errors = validate_desired_against_schema(&payload.desired, &schema);
    if !errors.is_empty() {
        return Err((StatusCode::BAD_REQUEST, errors.join("; ")));
    }

    let actor_user_id = user.id.clone();
    ensure_device_settings_row(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?;
    let previous: Option<(JsonValue,)> = sqlx::query_as(
        r#"
        SELECT desired
        FROM device_settings
        WHERE node_id = $1 AND device_type = $2
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;
    let previous_desired = previous
        .map(|row| row.0)
        .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));
    let diff = build_diff(&previous_desired, &payload.desired);

    let updated: DesiredRow = sqlx::query_as(
        r#"
        UPDATE device_settings
        SET desired = $3,
            desired_updated_at = now(),
            desired_updated_by = $4,
            pending = true
        WHERE node_id = $1 AND device_type = $2
        RETURNING desired,
                  pending,
                  desired_updated_at,
                  last_applied,
                  last_applied_at,
                  last_apply_status,
                  last_apply_result,
                  COALESCE(apply_requested, false) as apply_requested,
                  apply_requested_at,
                  COALESCE(maintenance_mode, false) as maintenance_mode
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(&payload.desired)
    .bind(&actor_user_id)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
        INSERT INTO device_settings_events (node_id, device_type, event_type, actor_user_id, desired, diff)
        VALUES ($1, $2, 'set_desired', $3, $4, $5)
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(&actor_user_id)
    .bind(&payload.desired)
    .bind(diff)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(RenogyDesiredSettingsResponse {
        node_id,
        device_type: DEVICE_TYPE.to_string(),
        desired: updated.desired,
        pending: updated.pending,
        desired_updated_at: updated.desired_updated_at.to_rfc3339(),
        last_applied: updated.last_applied,
        last_applied_at: updated.last_applied_at.map(|v| v.to_rfc3339()),
        last_apply_status: updated.last_apply_status,
        last_apply_result: updated.last_apply_result,
        apply_requested: updated.apply_requested,
        apply_requested_at: updated.apply_requested_at.map(|v| v.to_rfc3339()),
        maintenance_mode: updated.maintenance_mode,
    }))
}

#[utoipa::path(
    post,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/validate",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    request_body = RenogyDesiredSettingsUpdateRequest,
    responses(
        (status = 200, description = "Validation result", body = RenogyValidateResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn validate_settings(
    Path(_node_id): Path<String>,
    AuthUser(user): AuthUser,
    Json(payload): Json<RenogyDesiredSettingsUpdateRequest>,
) -> Result<Json<RenogyValidateResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let schema = parse_register_map()?;
    let errors = validate_desired_against_schema(&payload.desired, &schema);
    Ok(Json(RenogyValidateResponse {
        ok: errors.is_empty(),
        errors,
    }))
}

#[utoipa::path(
    post,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/read",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Current settings read from controller", body = RenogyReadCurrentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found"),
        (status = 409, description = "Node offline")
    )
)]
pub(crate) async fn read_current_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<RenogyReadCurrentResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(&node_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;

    let node: Option<NodeStatusRow> =
        sqlx::query_as("SELECT status FROM nodes WHERE id = $1")
    .bind(node_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;
    let Some(node) = node else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };
    if node.status != "online" {
        return Err((StatusCode::CONFLICT, "Node is offline".to_string()));
    }
    let mut endpoint = crate::services::node_agent_resolver::resolve_node_agent_endpoint(
        &state.db,
        node_uuid,
        state.config.node_agent_port,
        false,
    )
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to locate node-agent endpoint: {err}"),
        )
    })?;
    let Some(endpoint) = endpoint.as_mut() else {
        return Err((
            StatusCode::CONFLICT,
            "Unable to locate node-agent endpoint (mDNS + last-known IP both missing)."
                .to_string(),
        ));
    };

    let schema = parse_register_map()?;
    let fields = extract_register_fields(&schema)?;
    if fields.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "register map has no fields".to_string(),
        ));
    }

    let min_addr = fields.iter().map(|f| f.address).min().unwrap_or(0);
    let max_addr = fields
        .iter()
        .map(|f| f.address.saturating_add(f.count.saturating_sub(1)))
        .max()
        .unwrap_or(min_addr);
    let span_count = max_addr.saturating_sub(min_addr).saturating_add(1);
    if span_count > 64 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("register map span too large ({span_count}); split reads"),
        ));
    }

    let build_url = |base_url: &str| {
        format!("{base_url}/v1/renogy-bt/settings?start_address={min_addr}&count={span_count}")
    };
    let mut response = state
        .http
        .get(build_url(&endpoint.base_url))
        .timeout(Duration::from_secs(20))
        .send()
        .await;
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
            if refreshed.base_url != endpoint.base_url {
                *endpoint = refreshed;
                response = state
                    .http
                    .get(build_url(&endpoint.base_url))
                    .timeout(Duration::from_secs(20))
                    .send()
                    .await;
            }
        }
    }
    if response.is_err() {
        if let Some(ip_fallback) = endpoint.ip_fallback.clone() {
            if ip_fallback != endpoint.base_url {
                response = state
                    .http
                    .get(build_url(&ip_fallback))
                    .timeout(Duration::from_secs(20))
                    .send()
                    .await;
            }
        }
    }
    let response = response.map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Node-agent request failed: {err}"),
        )
    })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|_| "".to_string());
    if !status.is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Node-agent error ({status}): {body}"),
        ));
    }
    let raw: JsonValue = serde_json::from_str(&body).map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Invalid JSON from node-agent: {err}"),
        )
    })?;

    let status_text = raw
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("error");
    let provider_status = if status_text == "ok" {
        "ok".to_string()
    } else {
        let detail = raw
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        format!("error: {detail}")
    };

    let registers = raw
        .get("registers")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|v| v.as_u64())
        .filter_map(|v| u16::try_from(v).ok())
        .collect::<Vec<u16>>();

    let mut current_map = serde_json::Map::new();
    if status_text == "ok" {
        for field in &fields {
            if field.count != 1 {
                continue;
            }
            let offset = field.address.saturating_sub(min_addr) as usize;
            if offset >= registers.len() {
                continue;
            }
            let raw_val = registers[offset];
            let scaled = (raw_val as f64) * field.scale;
            if (field.scale - 1.0).abs() < f64::EPSILON {
                current_map.insert(field.key.clone(), JsonValue::from(raw_val as i64));
            } else {
                current_map.insert(field.key.clone(), JsonValue::from(scaled));
            }
        }
    }

    Ok(Json(RenogyReadCurrentResponse {
        current: JsonValue::Object(current_map),
        provider_status,
    }))
}

#[utoipa::path(
    post,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/apply",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Apply result", body = RenogyApplyResponse),
        (status = 400, description = "Invalid desired settings"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found"),
        (status = 409, description = "Node offline")
    )
)]
pub(crate) async fn apply_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<RenogyApplyResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(&node_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;

    ensure_device_settings_row(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?;

    let mut tx = state.db.begin().await.map_err(map_db_error)?;
    let lock_value = node_uuid.to_string();
    let lock_key = advisory_lock_key("renogy_settings", &lock_value);
    let _ = sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    let desired: DesiredForApplyRow = sqlx::query_as(
        r#"
        SELECT
            desired,
            last_applied,
            COALESCE(maintenance_mode, false) as maintenance_mode
        FROM device_settings
        WHERE node_id = $1 AND device_type = $2
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    if desired.maintenance_mode {
        return Err((
            StatusCode::CONFLICT,
            "Renogy maintenance mode is enabled for this node. Disable it to apply settings."
                .to_string(),
        ));
    }

    let schema = parse_register_map()?;
    let errors = validate_desired_against_schema(&desired.desired, &schema);
    if !errors.is_empty() {
        return Err((StatusCode::BAD_REQUEST, errors.join("; ")));
    }
    let writes = build_node_agent_writes(&desired.desired, &schema)
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    if writes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No writable settings in desired config".to_string(),
        ));
    }

    let node: Option<NodeStatusRow> = sqlx::query_as("SELECT status FROM nodes WHERE id = $1")
    .bind(node_uuid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;
    let Some(node) = node else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };
    let endpoint =
        crate::services::node_agent_resolver::resolve_node_agent_endpoint(
            &state.db,
            node_uuid,
            state.config.node_agent_port,
            false,
        )
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to locate node-agent endpoint: {err}"),
            )
        })?;
    if node.status != "online" || endpoint.is_none() {
        let actor_user_id = user.id.clone();
        let _ = sqlx::query(
            r#"
            UPDATE device_settings
            SET apply_requested = true,
                apply_requested_at = now(),
                apply_requested_by = $3
            WHERE node_id = $1 AND device_type = $2
            "#,
        )
        .bind(node_uuid)
        .bind(DEVICE_TYPE)
        .bind(&actor_user_id)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let prior_applied = desired
            .last_applied
            .clone()
            .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));
        let diff = build_diff(&prior_applied, &desired.desired);

        let _ = sqlx::query(
            r#"
            INSERT INTO device_settings_events (node_id, device_type, event_type, actor_user_id, desired, diff, result)
            VALUES ($1, $2, 'apply_queued', $3, $4, $5, jsonb_build_object('message', $6))
            "#,
        )
        .bind(node_uuid)
        .bind(DEVICE_TYPE)
        .bind(&actor_user_id)
        .bind(&desired.desired)
        .bind(diff)
        .bind("Apply queued (node offline or missing endpoint). The controller will retry automatically when the node is online.")
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;

        return Ok(Json(RenogyApplyResponse {
            status: "queued".to_string(),
            result: serde_json::json!({
                "message": "Apply queued. The controller will retry automatically when the node is online."
            }),
        }));
    }
    let endpoint = endpoint.unwrap();

    let actor_user_id = user.id.clone();
    let request_body = NodeAgentApplyRequest {
        writes: writes.clone(),
        verify: true,
    };
    let build_url = |base_url: &str| format!("{base_url}/v1/renogy-bt/settings/apply");
    let mut response = state
        .http
        .post(build_url(&endpoint.base_url))
        .timeout(Duration::from_secs(30))
        .json(&request_body)
        .send()
        .await;
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
            if refreshed.base_url != endpoint.base_url {
                response = state
                    .http
                    .post(build_url(&refreshed.base_url))
                    .timeout(Duration::from_secs(30))
                    .json(&request_body)
                    .send()
                    .await;
            }
        }
    }
    if response.is_err() {
        if let Some(ip_fallback) = endpoint.ip_fallback.as_deref() {
            if ip_fallback != endpoint.base_url.as_str() {
                response = state
                    .http
                    .post(build_url(ip_fallback))
                    .timeout(Duration::from_secs(30))
                    .json(&request_body)
                    .send()
                    .await;
            }
        }
    }
    let response = response.map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Node-agent request failed: {err}"),
        )
    })?;
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|_| "".to_string());
    if !status.is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            format!("Node-agent error ({status}): {body}"),
        ));
    }
    let result: JsonValue = serde_json::from_str(&body).map_err(|err| {
        (
            StatusCode::BAD_GATEWAY,
            format!("Invalid JSON from node-agent: {err}"),
        )
    })?;

    let apply_status = result
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("error")
        .to_string();

    let fields = extract_register_fields(&schema)?;

    let field_by_address: std::collections::HashMap<u16, (&str, &str)> = fields
        .iter()
        .map(|f| (f.address, (f.key.as_str(), f.label.as_str())))
        .collect();
    let mut applied_by_address: std::collections::HashMap<u16, Vec<u16>> =
        std::collections::HashMap::new();
    if apply_status == "ok" {
        if let Some(applied) = result.get("applied").and_then(|v| v.as_array()) {
            for item in applied {
                let address = item
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u16::try_from(v).ok());
                let read_back = item.get("read_back").and_then(|v| v.as_array());
                let Some(address) = address else { continue };
                let Some(read_back) = read_back else { continue };
                let values = read_back
                    .iter()
                    .filter_map(|v| v.as_u64())
                    .filter_map(|v| u16::try_from(v).ok())
                    .collect::<Vec<u16>>();
                if values.is_empty() {
                    continue;
                }
                applied_by_address.insert(address, values);
            }
        }
    }

    let field_results = writes
        .iter()
        .map(|write| {
            let (key, label) = field_by_address
                .get(&write.address)
                .copied()
                .unwrap_or(("", ""));
            let read_back = applied_by_address.get(&write.address).cloned();
            let ok = read_back
                .as_ref()
                .and_then(|vals| vals.first().copied())
                .zip(write.values.first().copied())
                .map(|(a, b)| a == b)
                .unwrap_or(false);
            serde_json::json!({
                "address": write.address,
                "key": if key.is_empty() { JsonValue::Null } else { JsonValue::String(key.to_string()) },
                "label": if label.is_empty() { JsonValue::Null } else { JsonValue::String(label.to_string()) },
                "expected": write.values,
                "read_back": read_back,
                "ok": ok,
            })
        })
        .collect::<Vec<JsonValue>>();

    let mut enriched_result = match result.clone() {
        JsonValue::Object(map) => JsonValue::Object(map),
        other => serde_json::json!({ "raw": other }),
    };
    if let JsonValue::Object(map) = &mut enriched_result {
        map.insert("field_results".to_string(), JsonValue::Array(field_results));
    }

    let mut current_map = serde_json::Map::new();
    if apply_status == "ok" {
        if let Some(applied) = result.get("applied").and_then(|v| v.as_array()) {
            for item in applied {
                let address = item
                    .get("address")
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u16::try_from(v).ok());
                let read_back = item.get("read_back").and_then(|v| v.as_array());
                let Some(address) = address else { continue };
                let Some(read_back) = read_back else { continue };
                let read_val = read_back
                    .first()
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u16::try_from(v).ok());
                let Some(read_val) = read_val else { continue };

                for field in &fields {
                    if field.count == 1 && field.address == address {
                        let scaled = (read_val as f64) * field.scale;
                        if (field.scale - 1.0).abs() < f64::EPSILON {
                            current_map.insert(field.key.clone(), JsonValue::from(read_val as i64));
                        } else {
                            current_map.insert(field.key.clone(), JsonValue::from(scaled));
                        }
                    }
                }
            }
        }
    }

    let prior_applied = desired
        .last_applied
        .clone()
        .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));
    let diff = build_diff(&prior_applied, &desired.desired);

    let _ = sqlx::query(
        r#"
        UPDATE device_settings
        SET pending = CASE WHEN $4 = 'ok' THEN false ELSE pending END,
            last_applied = CASE WHEN $4 = 'ok' THEN desired ELSE last_applied END,
            last_applied_at = CASE WHEN $4 = 'ok' THEN now() ELSE last_applied_at END,
            last_applied_by = CASE WHEN $4 = 'ok' THEN $3 ELSE last_applied_by END,
            last_apply_status = $4,
            last_apply_result = $5,
            apply_requested = false,
            apply_requested_at = NULL,
            apply_requested_by = NULL,
            last_apply_attempt_at = now(),
            apply_attempts = CASE WHEN $4 = 'ok' THEN 0 ELSE apply_attempts + 1 END
        WHERE node_id = $1 AND device_type = $2
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(&actor_user_id)
    .bind(&apply_status)
    .bind(&enriched_result)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
        INSERT INTO device_settings_events (node_id, device_type, event_type, actor_user_id, desired, current, diff, result)
        VALUES ($1, $2, 'apply', $3, $4, $5, $6, $7)
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(&actor_user_id)
    .bind(&desired.desired)
    .bind(JsonValue::Object(current_map))
    .bind(diff)
    .bind(&enriched_result)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;

    Ok(Json(RenogyApplyResponse {
        status: apply_status,
        result: enriched_result,
    }))
}

#[utoipa::path(
    get,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/history",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Apply history", body = Vec<RenogyHistoryEntry>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn list_history(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(_user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<Vec<RenogyHistoryEntry>>, (StatusCode, String)> {
    let node_uuid = Uuid::parse_str(&node_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;

    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM nodes WHERE id = $1")
        .bind(node_uuid)
        .fetch_optional(&state.db)
        .await
        .map_err(map_db_error)?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    }

    let rows = sqlx::query_as::<
        _,
        (
            i64,
            String,
            chrono::DateTime<chrono::Utc>,
            Option<JsonValue>,
            Option<JsonValue>,
            Option<JsonValue>,
            Option<JsonValue>,
        ),
    >(
        r#"
        SELECT id,
               event_type,
               created_at,
               desired,
               current,
               diff,
               result
        FROM device_settings_events
        WHERE node_id = $1 AND device_type = $2
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let entries = rows
        .into_iter()
        .map(|row| RenogyHistoryEntry {
            id: row.0,
            event_type: row.1,
            created_at: row.2.to_rfc3339(),
            desired: row.3,
            current: row.4,
            diff: row.5,
            result: row.6,
        })
        .collect();

    Ok(Json(entries))
}

#[utoipa::path(
    post,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/rollback",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    request_body = RenogyRollbackRequest,
    responses(
        (status = 200, description = "Rollback scheduled/applied", body = RenogyDesiredSettingsResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn rollback(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<RenogyRollbackRequest>,
) -> Result<Json<RenogyDesiredSettingsResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(&node_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;

    let row: Option<(JsonValue,)> = sqlx::query_as(
        r#"
        SELECT desired
        FROM device_settings_events
        WHERE id = $1 AND node_id = $2 AND device_type = $3
        "#,
    )
    .bind(payload.event_id)
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;
    let Some((desired,)) = row else {
        return Err((StatusCode::NOT_FOUND, "Event not found".to_string()));
    };

    let schema = parse_register_map()?;
    let errors = validate_desired_against_schema(&desired, &schema);
    if !errors.is_empty() {
        return Err((StatusCode::BAD_REQUEST, errors.join("; ")));
    }

    let actor_user_id = user.id.clone();
    ensure_device_settings_row(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?;
    let updated: DesiredRow = sqlx::query_as(
        r#"
        UPDATE device_settings
        SET desired = $3,
            desired_updated_at = now(),
            desired_updated_by = $4,
            pending = true
        WHERE node_id = $1 AND device_type = $2
        RETURNING desired,
                  pending,
                  desired_updated_at,
                  last_applied,
                  last_applied_at,
                  last_apply_status,
                  last_apply_result,
                  COALESCE(apply_requested, false) as apply_requested,
                  apply_requested_at,
                  COALESCE(maintenance_mode, false) as maintenance_mode
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(&desired)
    .bind(&actor_user_id)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
        INSERT INTO device_settings_events (node_id, device_type, event_type, actor_user_id, desired)
        VALUES ($1, $2, 'rollback_set_desired', $3, $4)
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(&actor_user_id)
    .bind(&desired)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(RenogyDesiredSettingsResponse {
        node_id,
        device_type: DEVICE_TYPE.to_string(),
        desired: updated.desired,
        pending: updated.pending,
        desired_updated_at: updated.desired_updated_at.to_rfc3339(),
        last_applied: updated.last_applied,
        last_applied_at: updated.last_applied_at.map(|v| v.to_rfc3339()),
        last_apply_status: updated.last_apply_status,
        last_apply_result: updated.last_apply_result,
        apply_requested: updated.apply_requested,
        apply_requested_at: updated.apply_requested_at.map(|v| v.to_rfc3339()),
        maintenance_mode: updated.maintenance_mode,
    }))
}

#[utoipa::path(
    put,
    path = "/api/nodes/{node_id}/renogy-bt2/settings/maintenance",
    tag = "renogy",
    params(("node_id" = String, Path, description = "Node UUID")),
    request_body = RenogyMaintenanceModeRequest,
    responses(
        (status = 200, description = "Updated settings", body = RenogyDesiredSettingsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn set_maintenance_mode(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<RenogyMaintenanceModeRequest>,
) -> Result<Json<RenogyDesiredSettingsResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(&node_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM nodes WHERE id = $1")
        .bind(node_uuid)
        .fetch_optional(&state.db)
        .await
        .map_err(map_db_error)?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    }

    ensure_device_settings_row(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?;
    let actor_user_id = user.id.clone();

    let updated: DesiredRow = sqlx::query_as(
        r#"
        UPDATE device_settings
        SET maintenance_mode = $3
        WHERE node_id = $1 AND device_type = $2
        RETURNING desired,
                  pending,
                  desired_updated_at,
                  last_applied,
                  last_applied_at,
                  last_apply_status,
                  last_apply_result,
                  COALESCE(apply_requested, false) as apply_requested,
                  apply_requested_at,
                  COALESCE(maintenance_mode, false) as maintenance_mode
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(payload.enabled)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    let event_type = if payload.enabled {
        "maintenance_on"
    } else {
        "maintenance_off"
    };
    let _ = sqlx::query(
        r#"
        INSERT INTO device_settings_events (node_id, device_type, event_type, actor_user_id, result)
        VALUES ($1, $2, $3, $4, jsonb_build_object('enabled', $5))
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE)
    .bind(event_type)
    .bind(&actor_user_id)
    .bind(payload.enabled)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(RenogyDesiredSettingsResponse {
        node_id,
        device_type: DEVICE_TYPE.to_string(),
        desired: updated.desired,
        pending: updated.pending,
        desired_updated_at: updated.desired_updated_at.to_rfc3339(),
        last_applied: updated.last_applied,
        last_applied_at: updated.last_applied_at.map(|v| v.to_rfc3339()),
        last_apply_status: updated.last_apply_status,
        last_apply_result: updated.last_apply_result,
        apply_requested: updated.apply_requested,
        apply_requested_at: updated.apply_requested_at.map(|v| v.to_rfc3339()),
        maintenance_mode: updated.maintenance_mode,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/schema",
            get(get_register_map_schema),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/desired",
            get(get_desired_settings),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/desired",
            put(put_desired_settings),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/validate",
            post(validate_settings),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/read",
            post(read_current_settings),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/apply",
            post(apply_settings),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/history",
            get(list_history),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/rollback",
            post(rollback),
        )
        .route(
            "/nodes/{node_id}/renogy-bt2/settings/maintenance",
            put(set_maintenance_mode),
        )
}
