use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::core_node;
use crate::error::map_db_error;
use crate::state::AppState;

const CAP_NODES_VIEW: &str = "nodes.view";

#[derive(sqlx::FromRow)]
pub(crate) struct NodeRow {
    id: Uuid,
    name: String,
    status: String,
    uptime_seconds: i64,
    cpu_percent: f32,
    storage_used_bytes: i64,
    memory_percent: Option<f32>,
    memory_used_bytes: Option<i64>,
    ping_ms: Option<f32>,
    ping_p50_30m_ms: Option<f32>,
    ping_jitter_ms: Option<f32>,
    mqtt_broker_rtt_ms: Option<f32>,
    mqtt_broker_rtt_jitter_ms: Option<f32>,
    network_latency_ms: Option<f32>,
    network_jitter_ms: Option<f32>,
    uptime_percent_24h: Option<f32>,
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    ip_last: Option<String>,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    config: SqlJson<JsonValue>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct NodeResponse {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) uptime_seconds: i64,
    pub(crate) cpu_percent: f32,
    pub(crate) storage_used_bytes: i64,
    pub(crate) memory_percent: Option<f32>,
    pub(crate) memory_used_bytes: Option<i64>,
    pub(crate) ping_ms: Option<f32>,
    pub(crate) ping_p50_30m_ms: Option<f32>,
    pub(crate) ping_jitter_ms: Option<f32>,
    pub(crate) mqtt_broker_rtt_ms: Option<f32>,
    pub(crate) mqtt_broker_rtt_jitter_ms: Option<f32>,
    pub(crate) network_latency_ms: Option<f32>,
    pub(crate) network_jitter_ms: Option<f32>,
    pub(crate) uptime_percent_24h: Option<f32>,
    pub(crate) mac_eth: Option<String>,
    pub(crate) mac_wifi: Option<String>,
    pub(crate) ip_last: Option<String>,
    pub(crate) last_seen: Option<String>,
    pub(crate) created_at: Option<String>,
    pub(crate) config: Option<JsonValue>,
}

impl From<NodeRow> for NodeResponse {
    fn from(row: NodeRow) -> Self {
        let is_core = core_node::is_core_node_id(row.id)
            || row
                .config
                .0
                .get("kind")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v.eq_ignore_ascii_case("core"));
        Self {
            id: row.id.to_string(),
            name: row.name,
            status: if is_core {
                "online".to_string()
            } else {
                row.status
            },
            uptime_seconds: row.uptime_seconds,
            cpu_percent: row.cpu_percent,
            storage_used_bytes: row.storage_used_bytes,
            memory_percent: row.memory_percent,
            memory_used_bytes: row.memory_used_bytes,
            ping_ms: row.ping_ms,
            ping_p50_30m_ms: row.ping_p50_30m_ms,
            ping_jitter_ms: row.ping_jitter_ms,
            mqtt_broker_rtt_ms: row.mqtt_broker_rtt_ms,
            mqtt_broker_rtt_jitter_ms: row.mqtt_broker_rtt_jitter_ms,
            network_latency_ms: row.network_latency_ms,
            network_jitter_ms: row.network_jitter_ms,
            uptime_percent_24h: row.uptime_percent_24h,
            mac_eth: row.mac_eth,
            mac_wifi: row.mac_wifi,
            ip_last: row.ip_last,
            last_seen: if is_core {
                Some(chrono::Utc::now().to_rfc3339())
            } else {
                row.last_seen.map(|ts| ts.to_rfc3339())
            },
            created_at: Some(row.created_at.to_rfc3339()),
            config: Some(row.config.0),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct NodeCreateRequest {
    name: String,
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    ip_last: Option<String>,
    status: Option<String>,
    uptime_seconds: Option<i64>,
    cpu_percent: Option<f32>,
    storage_used_bytes: Option<i64>,
    last_seen: Option<String>,
    config: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize, Default, utoipa::ToSchema)]
pub(crate) struct NodeUpdateRequest {
    name: Option<String>,
    status: Option<String>,
    ip_last: Option<String>,
    config: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct NodeOrderUpdateRequest {
    pub(crate) node_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct NodeDeleteQuery {
    #[serde(default)]
    purge: bool,
}

pub(crate) async fn fetch_nodes(db: &PgPool) -> Result<Vec<NodeResponse>, sqlx::Error> {
    let rows: Vec<NodeRow> = sqlx::query_as(
        r#"
		        SELECT
		            id,
		            name,
		            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            memory_percent,
            memory_used_bytes,
            ping_ms::real as ping_ms,
            ping_p50_30m_ms::real as ping_p50_30m_ms,
            ping_jitter_ms::real as ping_jitter_ms,
            mqtt_broker_rtt_ms::real as mqtt_broker_rtt_ms,
            mqtt_broker_rtt_jitter_ms::real as mqtt_broker_rtt_jitter_ms,
            network_latency_ms,
            network_jitter_ms,
            uptime_percent_24h,
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi,
            host(ip_last) as ip_last,
	            last_seen,
		            created_at,
		            COALESCE(config, '{}'::jsonb) as config
		        FROM nodes
			        WHERE NOT (COALESCE(config, '{}'::jsonb) @> '{"hidden": true}')
			          AND NOT (COALESCE(config, '{}'::jsonb) @> '{"poll_enabled": false}')
			          AND NOT (COALESCE(config, '{}'::jsonb) @> '{"deleted": true}')
			        ORDER BY ui_order NULLS LAST, created_at ASC, id ASC
			        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(NodeResponse::from).collect())
}

#[utoipa::path(
    get,
    path = "/api/nodes",
    tag = "nodes",
    responses(
        (status = 200, description = "Nodes", body = Vec<NodeResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_nodes(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<NodeResponse>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_NODES_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    Ok(Json(fetch_nodes(&state.db).await.map_err(map_db_error)?))
}

#[utoipa::path(
    post,
    path = "/api/nodes",
    tag = "nodes",
    request_body = NodeCreateRequest,
    responses(
        (status = 201, description = "Created node", body = NodeResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<NodeCreateRequest>,
) -> Result<(StatusCode, Json<NodeResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let name = payload.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing name".to_string()));
    }

    let status = payload
        .status
        .as_deref()
        .unwrap_or("unknown")
        .trim()
        .to_string();
    let uptime_seconds = payload.uptime_seconds.unwrap_or(0).max(0);
    let cpu_percent = payload.cpu_percent.unwrap_or(0.0).max(0.0);
    let storage_used_bytes = payload.storage_used_bytes.unwrap_or(0).max(0);
    let mac_eth = payload
        .mac_eth
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let mac_wifi = payload
        .mac_wifi
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let ip_last = payload
        .ip_last
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let last_seen = payload
        .last_seen
        .as_deref()
        .and_then(|raw| chrono::DateTime::parse_from_rfc3339(raw.trim()).ok())
        .map(|ts| ts.with_timezone(&chrono::Utc));
    let config = payload
        .config
        .unwrap_or(JsonValue::Object(Default::default()));

    let row: NodeRow = sqlx::query_as(
        r#"
	        INSERT INTO nodes (
	            name,
	            mac_eth,
	            mac_wifi,
	            ip_last,
	            status,
	            uptime_seconds,
	            cpu_percent,
	            storage_used_bytes,
	            last_seen,
	            config,
	            ui_order
	        )
	        VALUES (
	            $1,
	            $2::macaddr,
	            $3::macaddr,
	            CASE WHEN $4::text IS NULL THEN NULL ELSE $4::inet END,
	            $5,
	            $6,
	            $7,
	            $8,
	            $9,
	            $10,
	            (SELECT COALESCE(MAX(ui_order), 0) + 1 FROM nodes)
	        )
	        RETURNING
	            id,
	            name,
	            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            memory_percent,
            memory_used_bytes,
            ping_ms::real as ping_ms,
            ping_p50_30m_ms::real as ping_p50_30m_ms,
            ping_jitter_ms::real as ping_jitter_ms,
            mqtt_broker_rtt_ms::real as mqtt_broker_rtt_ms,
            mqtt_broker_rtt_jitter_ms::real as mqtt_broker_rtt_jitter_ms,
            network_latency_ms,
            network_jitter_ms,
            uptime_percent_24h,
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi,
            host(ip_last) as ip_last,
            last_seen,
            created_at,
            COALESCE(config, '{}'::jsonb) as config
        "#,
    )
    .bind(name)
    .bind(mac_eth)
    .bind(mac_wifi)
    .bind(ip_last)
    .bind(status)
    .bind(uptime_seconds)
    .bind(cpu_percent)
    .bind(storage_used_bytes)
    .bind(last_seen)
    .bind(SqlJson(config))
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok((StatusCode::CREATED, Json(NodeResponse::from(row))))
}

#[utoipa::path(
    put,
    path = "/api/nodes/order",
    tag = "nodes",
    request_body = NodeOrderUpdateRequest,
    responses(
        (status = 204, description = "Updated node order"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_node_order(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<NodeOrderUpdateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if payload.node_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "node_ids must contain at least one node id".to_string(),
        ));
    }

    let mut seen = HashSet::<Uuid>::new();
    let mut node_ids: Vec<Uuid> = Vec::with_capacity(payload.node_ids.len());
    for raw in payload.node_ids {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let uuid = Uuid::parse_str(trimmed).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid node id: {trimmed}"),
            )
        })?;
        if seen.insert(uuid) {
            node_ids.push(uuid);
        }
    }

    if node_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "node_ids must contain at least one valid node id".to_string(),
        ));
    }

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nodes WHERE id = ANY($1)")
        .bind(&node_ids)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;
    if count != node_ids.len() as i64 {
        return Err((
            StatusCode::BAD_REQUEST,
            "One or more node ids were not found".to_string(),
        ));
    }

    sqlx::query(
        r#"
        WITH ordered AS (
            SELECT id, ord::integer AS ui_order
            FROM unnest($1::uuid[]) WITH ORDINALITY AS t(id, ord)
        ),
        remaining AS (
            SELECT
                nodes.id,
                (SELECT COALESCE(MAX(ui_order), 0) FROM ordered)
                  + row_number() OVER (
                      ORDER BY nodes.ui_order NULLS LAST, nodes.created_at ASC, nodes.id ASC
                    ) AS ui_order
            FROM nodes
            WHERE nodes.id NOT IN (SELECT id FROM ordered)
        ),
        combined AS (
            SELECT id, ui_order FROM ordered
            UNION ALL
            SELECT id, ui_order FROM remaining
        )
        UPDATE nodes
        SET ui_order = combined.ui_order
        FROM combined
        WHERE nodes.id = combined.id
        "#,
    )
    .bind(&node_ids)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/nodes/{node_id}",
    tag = "nodes",
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "Node", body = NodeResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<NodeResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_NODES_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;
    let row: Option<NodeRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            memory_percent,
            memory_used_bytes,
            ping_ms::real as ping_ms,
            ping_p50_30m_ms::real as ping_p50_30m_ms,
            ping_jitter_ms::real as ping_jitter_ms,
            mqtt_broker_rtt_ms::real as mqtt_broker_rtt_ms,
            mqtt_broker_rtt_jitter_ms::real as mqtt_broker_rtt_jitter_ms,
            network_latency_ms,
            network_jitter_ms,
            uptime_percent_24h,
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi,
            host(ip_last) as ip_last,
            last_seen,
            created_at,
            COALESCE(config, '{}'::jsonb) as config
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
    Ok(Json(NodeResponse::from(row)))
}

#[utoipa::path(
    put,
    path = "/api/nodes/{node_id}",
    tag = "nodes",
    request_body = NodeUpdateRequest,
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "Updated node", body = NodeResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn update_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<NodeUpdateRequest>,
) -> Result<Json<NodeResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let row: Option<NodeRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            memory_percent,
            memory_used_bytes,
            ping_ms::real as ping_ms,
            ping_p50_30m_ms::real as ping_p50_30m_ms,
            ping_jitter_ms::real as ping_jitter_ms,
            mqtt_broker_rtt_ms::real as mqtt_broker_rtt_ms,
            mqtt_broker_rtt_jitter_ms::real as mqtt_broker_rtt_jitter_ms,
            network_latency_ms,
            network_jitter_ms,
            uptime_percent_24h,
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi,
            host(ip_last) as ip_last,
            last_seen,
            created_at,
            COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(mut row) = row else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    if let Some(name) = payload.name {
        if !name.trim().is_empty() {
            row.name = name.trim().to_string();
        }
    }
    if let Some(status) = payload.status {
        if !status.trim().is_empty() {
            row.status = status.trim().to_string();
        }
    }
    if let Some(ip_last) = payload.ip_last {
        let trimmed = ip_last.trim();
        row.ip_last = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(config) = payload.config {
        row.config = SqlJson(config);
    }

    let updated: NodeRow = sqlx::query_as(
        r#"
        UPDATE nodes
        SET name = $2,
            status = $3,
            ip_last = CASE WHEN $4::text IS NULL THEN NULL ELSE $4::inet END,
            config = $5,
            last_seen = COALESCE(last_seen, NOW())
        WHERE id = $1
        RETURNING
            id,
            name,
            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            memory_percent,
            memory_used_bytes,
            ping_ms::real as ping_ms,
            ping_p50_30m_ms::real as ping_p50_30m_ms,
            ping_jitter_ms::real as ping_jitter_ms,
            mqtt_broker_rtt_ms::real as mqtt_broker_rtt_ms,
            mqtt_broker_rtt_jitter_ms::real as mqtt_broker_rtt_jitter_ms,
            network_latency_ms,
            network_jitter_ms,
            uptime_percent_24h,
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi,
            host(ip_last) as ip_last,
            last_seen,
            created_at,
            COALESCE(config, '{}'::jsonb) as config
        "#,
    )
    .bind(node_uuid)
    .bind(&row.name)
    .bind(&row.status)
    .bind(row.ip_last.as_deref())
    .bind(row.config)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;

    Ok(Json(NodeResponse::from(updated)))
}

#[utoipa::path(
    delete,
    path = "/api/nodes/{node_id}",
    tag = "nodes",
    params(("node_id" = String, Path, description = "Node id"), NodeDeleteQuery),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Query(query): Query<NodeDeleteQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    if core_node::is_core_node_id(node_uuid) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Core node cannot be deleted".to_string(),
        ));
    }

    if query.purge {
        crate::auth::require_capabilities(&user, &["ops.purge"])
            .map_err(|err| (err.status, err.message))?;

        let mut tx = state.db.begin().await.map_err(map_db_error)?;

        let _ = sqlx::query("DELETE FROM adoption_tokens WHERE node_id = $1")
            .bind(node_uuid)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        let _ = sqlx::query("DELETE FROM alarm_events WHERE node_id = $1")
            .bind(node_uuid)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        let _ = sqlx::query("DELETE FROM alarms WHERE node_id = $1")
            .bind(node_uuid)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        let _ = sqlx::query("DELETE FROM nodes WHERE id = $1")
            .bind(node_uuid)
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        tx.commit().await.map_err(map_db_error)?;
        return Ok(StatusCode::NO_CONTENT);
    }

    let deleted_at = chrono::Utc::now();
    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    #[derive(sqlx::FromRow)]
    struct NodeDeleteMeta {
        name: String,
        external_provider: Option<String>,
        external_id: Option<String>,
    }

    let existing: Option<NodeDeleteMeta> = sqlx::query_as(
        r#"
        SELECT name, external_provider, external_id
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(existing) = existing else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let deleted_stamp = deleted_at.format("%Y%m%dT%H%M%SZ").to_string();
    let deleted_name = if existing.name.contains("-deleted") {
        existing.name
    } else {
        format!("{}-deleted-{}", existing.name, deleted_stamp)
    };

    let _ = sqlx::query(
        r#"
		        UPDATE nodes
		        SET name = $2,
		            status = 'deleted',
		            mac_eth = NULL,
		            mac_wifi = NULL,
		            ip_last = NULL,
		            last_seen = $3,
		            config = jsonb_set(
		              jsonb_set(
		                jsonb_set(
		                jsonb_set(COALESCE(config, '{}'::jsonb), '{deleted}', 'true'::jsonb, true),
		                '{hidden}',
		                'true'::jsonb,
		                true
		              ),
		              '{poll_enabled}',
		              'false'::jsonb,
		              true
		            ),
		              '{deleted_at}',
		              to_jsonb($3),
		              true
		            )
		        WHERE id = $1
	        "#,
    )
    .bind(node_uuid)
    .bind(deleted_name.trim())
    .bind(deleted_at)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query("DELETE FROM map_features WHERE node_id = $1")
        .bind(node_uuid)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
        UPDATE weather_station_integrations
        SET enabled = FALSE
        WHERE node_id = $1
        "#,
    )
    .bind(node_uuid)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let _ = sqlx::query(
        r#"
		        UPDATE sensors
	        SET name = CASE
	                WHEN name LIKE '%-deleted%' THEN name
	                ELSE name || '-deleted-' || $3
	            END,
	            deleted_at = $2,
	            config = COALESCE(config, '{}'::jsonb) || '{"poll_enabled": false, "hidden": true}'::jsonb
	        WHERE node_id = $1
	          AND deleted_at IS NULL
	        "#,
	    )
	    .bind(node_uuid)
	    .bind(deleted_at)
	    .bind(deleted_stamp)
	    .execute(&mut *tx)
	    .await
    .map_err(map_db_error)?;

    // If this is an Emporia-backed node, disable the device in Emporia preferences so the controller
    // stops polling/ingesting it (but retains history).
    if existing
        .external_provider
        .as_deref()
        .is_some_and(|provider| provider.eq_ignore_ascii_case("emporia"))
    {
        let Some(device_gid) = existing
            .external_id
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        else {
            tx.commit().await.map_err(map_db_error)?;
            return Ok(StatusCode::NO_CONTENT);
        };

        #[derive(sqlx::FromRow)]
        struct CredentialRow {
            metadata: SqlJson<JsonValue>,
        }

        let row: Option<CredentialRow> = sqlx::query_as(
            r#"
            SELECT metadata
            FROM setup_credentials
            WHERE name = 'emporia'
            "#,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_db_error)?;

        if let Some(row) = row {
            let mut metadata = row.metadata.0;
            let devices = metadata
                .as_object_mut()
                .and_then(|obj| obj.get_mut("devices"))
                .and_then(|v| v.as_object_mut());

            if devices.is_none() {
                if let Some(obj) = metadata.as_object_mut() {
                    obj.insert(
                        "devices".to_string(),
                        JsonValue::Object(serde_json::Map::new()),
                    );
                }
            }

            let Some(devices) = metadata
                .as_object_mut()
                .and_then(|obj| obj.get_mut("devices"))
                .and_then(|v| v.as_object_mut())
            else {
                tx.commit().await.map_err(map_db_error)?;
                return Ok(StatusCode::NO_CONTENT);
            };

            match devices.get_mut(device_gid) {
                Some(JsonValue::Object(obj)) => {
                    obj.insert("enabled".to_string(), JsonValue::Bool(false));
                    obj.insert("hidden".to_string(), JsonValue::Bool(true));
                    obj.insert(
                        "include_in_power_summary".to_string(),
                        JsonValue::Bool(false),
                    );
                }
                _ => {
                    devices.insert(
                        device_gid.to_string(),
                        serde_json::json!({
                            "enabled": false,
                            "hidden": true,
                            "include_in_power_summary": false
                        }),
                    );
                }
            }

            let _ = sqlx::query(
                r#"
                UPDATE setup_credentials
                SET metadata = $1,
                    updated_at = NOW()
                WHERE name = 'emporia'
                "#,
            )
            .bind(SqlJson(metadata))
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }
    }

    tx.commit().await.map_err(map_db_error)?;

    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/nodes", get(list_nodes).post(create_node))
        .route("/nodes/order", put(update_node_order))
        .route(
            "/nodes/{node_id}",
            get(get_node).put(update_node).delete(delete_node),
        )
}
