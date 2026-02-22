use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::ids;
use crate::state::AppState;

const CAP_OUTPUTS_VIEW: &str = "outputs.view";

#[derive(sqlx::FromRow)]
pub(crate) struct OutputRow {
    id: String,
    node_id: Uuid,
    name: String,
    output_type: String,
    state: String,
    last_command: Option<chrono::DateTime<chrono::Utc>>,
    supported_states: SqlJson<Vec<String>>,
    config: SqlJson<JsonValue>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct OutputResponse {
    id: String,
    node_id: String,
    name: String,
    #[serde(rename = "type")]
    output_type: String,
    state: String,
    last_command: Option<String>,
    supported_states: Vec<String>,
    command_topic: Option<String>,
    schedule_ids: Vec<String>,
    history: Vec<JsonValue>,
    config: JsonValue,
}

impl From<OutputRow> for OutputResponse {
    fn from(row: OutputRow) -> Self {
        let command_topic = row
            .config
            .0
            .get("command_topic")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
        let schedule_ids: Vec<String> = row
            .config
            .0
            .get("schedule_ids")
            .and_then(|value| value.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str())
                    .map(|item| item.to_string())
                    .collect()
            })
            .unwrap_or_default();
        let history: Vec<JsonValue> = row
            .config
            .0
            .get("history")
            .and_then(|value| value.as_array())
            .map(|items| items.iter().cloned().collect())
            .unwrap_or_default();

        Self {
            id: row.id,
            node_id: row.node_id.to_string(),
            name: row.name,
            output_type: row.output_type,
            state: row.state,
            last_command: row.last_command.map(|ts| ts.to_rfc3339()),
            supported_states: row.supported_states.0,
            command_topic,
            schedule_ids,
            history,
            config: row.config.0,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct OutputsQuery {
    node_id: Option<Uuid>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct OutputCreateRequest {
    id: Option<String>,
    node_id: Uuid,
    name: String,
    #[serde(rename = "type")]
    output_type: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    supported_states: Vec<String>,
    #[serde(default)]
    schedule_ids: Vec<String>,
    command_topic: Option<String>,
    #[serde(default)]
    history: Vec<JsonValue>,
    config: Option<JsonValue>,
    last_command: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, Default, utoipa::ToSchema)]
pub(crate) struct OutputUpdateRequest {
    node_id: Option<Uuid>,
    name: Option<String>,
    #[serde(rename = "type")]
    output_type: Option<String>,
    state: Option<String>,
    supported_states: Option<Vec<String>>,
    schedule_ids: Option<Vec<String>>,
    command_topic: Option<String>,
    history: Option<Vec<JsonValue>>,
    config: Option<JsonValue>,
    last_command: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct OutputCommandRequest {
    state: String,
    reason: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/outputs",
    tag = "outputs",
    responses(
        (status = 200, description = "Outputs", body = Vec<OutputResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_outputs(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<OutputsQuery>,
) -> Result<Json<Vec<OutputResponse>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_OUTPUTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    Ok(Json(
        fetch_outputs(&state.db, query.node_id)
            .await
            .map_err(map_db_error)?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/outputs/{output_id}",
    tag = "outputs",
    params(("output_id" = String, Path, description = "Output id")),
    responses(
        (status = 200, description = "Output", body = OutputResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Output not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_output(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(output_id): Path<String>,
) -> Result<Json<OutputResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_OUTPUTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let row: Option<OutputRow> = sqlx::query_as(
        r#"
        SELECT
            outputs.id,
            outputs.node_id,
            name,
            type as output_type,
            state,
            last_command,
            supported_states,
            COALESCE(config, '{}'::jsonb) as config
        FROM outputs
        WHERE id = $1
        "#,
    )
    .bind(output_id.trim())
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Output not found".to_string()));
    };

    Ok(Json(OutputResponse::from(row)))
}

pub(crate) async fn fetch_outputs(
    db: &sqlx::PgPool,
    node_id: Option<Uuid>,
) -> Result<Vec<OutputResponse>, sqlx::Error> {
    let rows: Vec<OutputRow> = sqlx::query_as(
        r#"
        SELECT
            outputs.id,
            outputs.node_id,
            outputs.name,
            outputs.type as output_type,
            outputs.state,
            outputs.last_command,
            outputs.supported_states,
            COALESCE(outputs.config, '{}'::jsonb) as config
        FROM outputs
        JOIN nodes ON nodes.id = outputs.node_id
        WHERE ($1::uuid IS NULL OR outputs.node_id = $1)
          AND NOT (COALESCE(nodes.config, '{}'::jsonb) @> '{"hidden": true}')
          AND NOT (COALESCE(nodes.config, '{}'::jsonb) @> '{"poll_enabled": false}')
          AND NOT (COALESCE(nodes.config, '{}'::jsonb) @> '{"deleted": true}')
        ORDER BY outputs.created_at ASC
        "#,
    )
    .bind(node_id)
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(OutputResponse::from).collect())
}

#[derive(sqlx::FromRow)]
struct NodeIdentityRow {
    id: Uuid,
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    config: SqlJson<JsonValue>,
}

fn ensure_object(value: JsonValue) -> Result<serde_json::Map<String, JsonValue>, String> {
    match value {
        JsonValue::Object(map) => Ok(map),
        other => Err(format!("Expected JSON object, received {}", other)),
    }
}

fn merge_output_config(
    existing: JsonValue,
    config_patch: Option<JsonValue>,
    schedule_ids: Option<Vec<String>>,
    command_topic: Option<String>,
    history: Option<Vec<JsonValue>>,
) -> Result<JsonValue, (StatusCode, String)> {
    let mut config = existing;
    if config.is_null() {
        config = JsonValue::Object(Default::default());
    }

    let mut map = ensure_object(config).map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    if let Some(patch) = config_patch {
        let patch_map = ensure_object(patch).map_err(|err| (StatusCode::BAD_REQUEST, err))?;
        for (key, value) in patch_map {
            map.insert(key, value);
        }
    }
    if let Some(schedule_ids) = schedule_ids {
        let mut normalized: Vec<JsonValue> = Vec::with_capacity(schedule_ids.len());
        for value in schedule_ids {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Schedule ids cannot be blank".to_string(),
                ));
            }
            normalized.push(JsonValue::String(trimmed.to_string()));
        }
        map.insert("schedule_ids".to_string(), JsonValue::Array(normalized));
    }
    if let Some(command_topic) = command_topic {
        map.insert(
            "command_topic".to_string(),
            JsonValue::String(command_topic.trim().to_string()),
        );
    }
    if let Some(history) = history {
        map.insert("history".to_string(), JsonValue::Array(history));
    }

    Ok(JsonValue::Object(map))
}

fn parse_rfc3339(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, (StatusCode, String)> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    chrono::DateTime::parse_from_rfc3339(trimmed)
        .map(|ts| Some(ts.with_timezone(&chrono::Utc)))
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid {field} timestamp"),
            )
        })
}

fn update_output_manifest(config: &mut JsonValue, output: &OutputResponse) {
    let JsonValue::Object(config_map) = config else {
        *config = JsonValue::Object(Default::default());
        return update_output_manifest(config, output);
    };

    let mut manifest = match config_map.remove("output_manifest") {
        Some(JsonValue::Object(map)) => map,
        _ => serde_json::Map::new(),
    };

    manifest.insert(
        output.id.clone(),
        serde_json::json!({
            "id": output.id,
            "name": output.name,
            "type": output.output_type,
            "state": output.state,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        }),
    );

    config_map.insert("output_manifest".to_string(), JsonValue::Object(manifest));
}

fn remove_output_manifest(config: &mut JsonValue, output_id: &str) {
    let JsonValue::Object(config_map) = config else {
        return;
    };

    let JsonValue::Object(mut manifest) = config_map
        .get("output_manifest")
        .cloned()
        .unwrap_or(JsonValue::Object(Default::default()))
    else {
        config_map.remove("output_manifest");
        return;
    };

    manifest.remove(output_id);
    if manifest.is_empty() {
        config_map.remove("output_manifest");
    } else {
        config_map.insert("output_manifest".to_string(), JsonValue::Object(manifest));
    }
}

#[utoipa::path(
    post,
    path = "/api/outputs",
    tag = "outputs",
    request_body = OutputCreateRequest,
    responses(
        (status = 201, description = "Created output", body = OutputResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Parent node not found"),
        (status = 409, description = "Output id already exists")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_output(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<OutputCreateRequest>,
) -> Result<(StatusCode, Json<OutputResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if payload.name.trim().is_empty() || payload.output_type.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing name/type".to_string()));
    }

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let node: Option<NodeIdentityRow> = sqlx::query_as(
        r#"
        SELECT id, mac_eth::text as mac_eth, mac_wifi::text as mac_wifi, COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(payload.node_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(mut node) = node else {
        return Err((StatusCode::NOT_FOUND, "Parent node not found".to_string()));
    };

    let created_at = chrono::Utc::now();
    let output_id = if let Some(id) = payload
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM outputs WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_error)?;
        if exists.is_some() {
            return Err((StatusCode::CONFLICT, "Output id already exists".to_string()));
        }
        id.to_string()
    } else {
        let mac_eth = node.mac_eth.as_deref();
        let mac_wifi = node.mac_wifi.as_deref();
        let mut allocated: Option<String> = None;
        for counter in 0..2048u32 {
            let candidate =
                ids::deterministic_hex_id("output", mac_eth, mac_wifi, created_at, counter);
            let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM outputs WHERE id = $1")
                .bind(&candidate)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db_error)?;
            if exists.is_none() {
                allocated = Some(candidate);
                break;
            }
        }
        allocated.ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to allocate unique output id".to_string(),
        ))?
    };

    let config = merge_output_config(
        payload.config.unwrap_or_else(|| serde_json::json!({})),
        None,
        Some(payload.schedule_ids),
        payload.command_topic.clone(),
        Some(payload.history),
    )?;

    let last_command = parse_rfc3339(payload.last_command.as_deref(), "last_command")?;

    let row: OutputRow = sqlx::query_as(
        r#"
        INSERT INTO outputs (id, node_id, name, type, state, supported_states, last_command, config)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING
            id,
            node_id,
            name,
            type as output_type,
            state,
            last_command,
            supported_states,
            config
        "#,
    )
    .bind(&output_id)
    .bind(payload.node_id)
    .bind(payload.name.trim())
    .bind(payload.output_type.trim())
    .bind(payload.state.as_deref().unwrap_or("unknown").trim())
    .bind(SqlJson(payload.supported_states))
    .bind(last_command)
    .bind(SqlJson(config))
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let output = OutputResponse::from(row);
    update_output_manifest(&mut node.config.0, &output);
    let _ = sqlx::query("UPDATE nodes SET config = $2 WHERE id = $1")
        .bind(node.id)
        .bind(SqlJson(node.config.0))
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;
    Ok((StatusCode::CREATED, Json(output)))
}

#[utoipa::path(
    put,
    path = "/api/outputs/{output_id}",
    tag = "outputs",
    request_body = OutputUpdateRequest,
    params(("output_id" = String, Path, description = "Output id")),
    responses(
        (status = 200, description = "Updated output", body = OutputResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Output not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_output(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(output_id): Path<String>,
    Json(payload): Json<OutputUpdateRequest>,
) -> Result<Json<OutputResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let existing: Option<OutputRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            node_id,
            name,
            type as output_type,
            state,
            last_command,
            supported_states,
            COALESCE(config, '{}'::jsonb) as config
        FROM outputs
        WHERE id = $1
        "#,
    )
    .bind(output_id.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(mut existing) = existing else {
        return Err((StatusCode::NOT_FOUND, "Output not found".to_string()));
    };

    if let Some(node_id) = payload.node_id {
        let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM nodes WHERE id = $1")
            .bind(node_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_db_error)?;
        if exists.is_none() {
            return Err((StatusCode::NOT_FOUND, "Parent node not found".to_string()));
        }
        existing.node_id = node_id;
    }
    if let Some(name) = payload.name {
        if !name.trim().is_empty() {
            existing.name = name.trim().to_string();
        }
    }
    if let Some(output_type) = payload.output_type {
        if !output_type.trim().is_empty() {
            existing.output_type = output_type.trim().to_string();
        }
    }
    if let Some(state_value) = payload.state {
        if !state_value.trim().is_empty() {
            existing.state = state_value.trim().to_string();
        }
    }
    if let Some(states) = payload.supported_states {
        existing.supported_states = SqlJson(states);
    }
    if let Some(last_command) = payload.last_command {
        existing.last_command = parse_rfc3339(Some(&last_command), "last_command")?;
    }

    if payload.config.is_some()
        || payload.schedule_ids.is_some()
        || payload.command_topic.is_some()
        || payload.history.is_some()
    {
        let merged = merge_output_config(
            existing.config.0,
            payload.config,
            payload.schedule_ids,
            payload.command_topic,
            payload.history,
        )?;
        existing.config = SqlJson(merged);
    }

    let row: OutputRow = sqlx::query_as(
        r#"
        UPDATE outputs
        SET node_id = $2,
            name = $3,
            type = $4,
            state = $5,
            supported_states = $6,
            last_command = $7,
            config = $8
        WHERE id = $1
        RETURNING
            id,
            node_id,
            name,
            type as output_type,
            state,
            last_command,
            supported_states,
            config
        "#,
    )
    .bind(existing.id.trim())
    .bind(existing.node_id)
    .bind(existing.name.trim())
    .bind(existing.output_type.trim())
    .bind(existing.state.trim())
    .bind(existing.supported_states)
    .bind(existing.last_command)
    .bind(existing.config)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let output = OutputResponse::from(row);

    let node: Option<NodeIdentityRow> = sqlx::query_as(
        r#"
        SELECT id, mac_eth::text as mac_eth, mac_wifi::text as mac_wifi, COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(output.node_id.parse::<Uuid>().ok())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    if let Some(mut node) = node {
        update_output_manifest(&mut node.config.0, &output);
        let _ = sqlx::query("UPDATE nodes SET config = $2 WHERE id = $1")
            .bind(node.id)
            .bind(SqlJson(node.config.0))
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
    }

    tx.commit().await.map_err(map_db_error)?;
    Ok(Json(output))
}

#[utoipa::path(
    delete,
    path = "/api/outputs/{output_id}",
    tag = "outputs",
    params(("output_id" = String, Path, description = "Output id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Output not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_output(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(output_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let existing: Option<(Uuid,)> = sqlx::query_as("SELECT node_id FROM outputs WHERE id = $1")
        .bind(output_id.trim())
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;

    let Some((node_id,)) = existing else {
        return Err((StatusCode::NOT_FOUND, "Output not found".to_string()));
    };

    let result = sqlx::query("DELETE FROM outputs WHERE id = $1")
        .bind(output_id.trim())
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Output not found".to_string()));
    }

    let node: Option<NodeIdentityRow> = sqlx::query_as(
        r#"
        SELECT id, mac_eth::text as mac_eth, mac_wifi::text as mac_wifi, COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    if let Some(mut node) = node {
        remove_output_manifest(&mut node.config.0, output_id.trim());
        let _ = sqlx::query("UPDATE nodes SET config = $2 WHERE id = $1")
            .bind(node.id)
            .bind(SqlJson(node.config.0))
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
    }

    tx.commit().await.map_err(map_db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/outputs/{output_id}/command",
    tag = "outputs",
    request_body = OutputCommandRequest,
    params(("output_id" = String, Path, description = "Output id")),
    responses(
        (status = 200, description = "Updated output", body = OutputResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Output not found")
    )
)]
pub(crate) async fn command_output(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(output_id): Path<String>,
    Json(payload): Json<OutputCommandRequest>,
) -> Result<Json<OutputResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["outputs.command"])
        .map_err(|err| (err.status, err.message))?;

    let desired = payload.state.trim();
    if desired.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing state".to_string()));
    }

    let row: Option<OutputRow> = sqlx::query_as(
        r#"
        UPDATE outputs
        SET state = $2,
            last_command = NOW()
        WHERE id = $1
        RETURNING
            id,
            node_id,
            name,
            type as output_type,
            state,
            last_command,
            supported_states,
            config
        "#,
    )
    .bind(output_id.trim())
    .bind(desired)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Output not found".to_string()));
    };

    // Best-effort MQTT publish to match production behavior.
    let topic = format!("iot/broadcast/outputs/{}", row.id);
    let payload = serde_json::json!({
        "state": desired,
        "reason": payload.reason.unwrap_or_else(|| "manual".to_string()),
    });
    let _ = state.mqtt.publish_json(&topic, &payload).await;

    Ok(Json(OutputResponse::from(row)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/outputs", get(list_outputs).post(create_output))
        .route(
            "/outputs/{output_id}",
            get(get_output).put(update_output).delete(delete_output),
        )
        .route("/outputs/{output_id}/command", post(command_output))
}
