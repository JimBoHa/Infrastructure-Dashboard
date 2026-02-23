use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::{require_capabilities, AuthUser};
use crate::device_catalog;
use crate::device_catalog::DeviceVendor;
use crate::error::map_db_error;
use crate::services::external_devices::ExternalDeviceConfig;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ExternalDeviceCatalogResponse {
    pub version: u32,
    #[schema(value_type = Vec<DeviceVendor>)]
    pub vendors: Vec<DeviceVendor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ExternalDeviceSummary {
    pub node_id: String,
    pub name: String,
    pub external_provider: Option<String>,
    pub external_id: Option<String>,
    pub config: JsonValue,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ExternalDeviceCreateRequest {
    pub name: String,
    pub vendor_id: String,
    pub model_id: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub unit_id: Option<u8>,
    pub poll_interval_seconds: Option<u64>,
    pub snmp_community: Option<String>,
    pub http_base_url: Option<String>,
    pub http_username: Option<String>,
    pub http_password: Option<String>,
    pub lip_username: Option<String>,
    pub lip_password: Option<String>,
    pub lip_integration_report: Option<String>,
    pub leap_client_cert_pem: Option<String>,
    pub leap_client_key_pem: Option<String>,
    pub leap_ca_pem: Option<String>,
    pub leap_verify_ca: Option<bool>,
    pub external_id: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/integrations/devices/catalog",
    tag = "integrations",
    responses(
        (status = 200, description = "Device catalog", body = ExternalDeviceCatalogResponse)
    ),
    security(
        ("HTTPBearer" = [])
    )
)]
pub async fn get_device_catalog(AuthUser(user): AuthUser) -> Result<Json<ExternalDeviceCatalogResponse>, (StatusCode, String)> {
    require_capabilities(&user, &["config.view"]).map_err(|err| (err.status, err.message))?;
    let catalog = device_catalog::catalog();
    Ok(Json(ExternalDeviceCatalogResponse {
        version: catalog.version,
        vendors: catalog.vendors.clone(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/integrations/devices",
    tag = "integrations",
    responses(
        (status = 200, description = "External devices", body = [ExternalDeviceSummary])
    ),
    security(
        ("HTTPBearer" = [])
    )
)]
pub async fn list_devices(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<ExternalDeviceSummary>>, (StatusCode, String)> {
    require_capabilities(&user, &["config.view"]).map_err(|err| (err.status, err.message))?;
    let rows: Vec<(Uuid, String, Option<String>, Option<String>, SqlJson<JsonValue>)> =
        sqlx::query_as(
            r#"
            SELECT id, name, external_provider, external_id, COALESCE(config, '{}'::jsonb) as config
            FROM nodes
            WHERE external_provider IS NOT NULL
            ORDER BY name
            "#,
        )
        .fetch_all(&state.db)
        .await
        .map_err(map_db_error)?;
    let devices = rows
        .into_iter()
        .map(|row| ExternalDeviceSummary {
            node_id: row.0.to_string(),
            name: row.1,
            external_provider: row.2,
            external_id: row.3,
            config: row.4 .0,
        })
        .collect();
    Ok(Json(devices))
}

#[utoipa::path(
    post,
    path = "/api/integrations/devices",
    tag = "integrations",
    request_body = ExternalDeviceCreateRequest,
    responses(
        (status = 200, description = "External device created", body = ExternalDeviceSummary)
    ),
    security(
        ("HTTPBearer" = [])
    )
)]
pub async fn create_device(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    Json(request): Json<ExternalDeviceCreateRequest>,
) -> Result<Json<ExternalDeviceSummary>, (StatusCode, String)> {
    require_capabilities(&user, &["config.write"]).map_err(|err| (err.status, err.message))?;
    let model = device_catalog::find_model(&request.vendor_id, &request.model_id)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Unknown device model".to_string()))?;
    if !model.protocols.iter().any(|entry| entry == &request.protocol) {
        return Err((
            StatusCode::BAD_REQUEST,
            "Protocol not supported for this model".to_string(),
        ));
    }
    let device_config = ExternalDeviceConfig {
        vendor_id: request.vendor_id.clone(),
        model_id: request.model_id.clone(),
        protocol: request.protocol.clone(),
        host: request.host.clone(),
        port: request.port,
        unit_id: request.unit_id,
        poll_interval_seconds: request.poll_interval_seconds,
        snmp_community: request.snmp_community.clone(),
        http_base_url: request.http_base_url.clone(),
        http_username: request.http_username.clone(),
        http_password: request.http_password.clone(),
        lip_username: request.lip_username.clone(),
        lip_password: request.lip_password.clone(),
        lip_integration_report: request.lip_integration_report.clone(),
        leap_client_cert_pem: request.leap_client_cert_pem.clone(),
        leap_client_key_pem: request.leap_client_key_pem.clone(),
        leap_ca_pem: request.leap_ca_pem.clone(),
        leap_verify_ca: request.leap_verify_ca,
        discovered_points: None,
    };
    let external_id = request.external_id.clone().unwrap_or_else(|| {
        let host = request.host.clone().unwrap_or_else(|| "unknown".to_string());
        format!(
            "{}:{}:{}",
            request.vendor_id.trim(),
            request.model_id.trim(),
            host.trim()
        )
    });

    let config = json!({
        "external_device": device_config,
    });

    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO nodes (
            name,
            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            last_seen,
            config,
            external_provider,
            external_id
        )
        VALUES ($1, 'offline', 0, 0, 0, NULL, $2, $3, $4)
        ON CONFLICT (external_provider, external_id)
        DO UPDATE SET
            name = EXCLUDED.name,
            config = EXCLUDED.config
        RETURNING id
        "#,
    )
    .bind(request.name.trim())
    .bind(SqlJson(config))
    .bind(request.vendor_id.trim())
    .bind(external_id.trim())
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(ExternalDeviceSummary {
        node_id: row.0.to_string(),
        name: request.name,
        external_provider: Some(request.vendor_id),
        external_id: Some(external_id),
        config: json!({
            "external_device": device_config
        }),
    }))
}

#[utoipa::path(
    post,
    path = "/api/integrations/devices/{node_id}/sync",
    tag = "integrations",
    responses(
        (status = 200, description = "External device sync kicked")
    ),
    params(
        ("node_id" = String, Path, description = "Device node id")
    ),
    security(
        ("HTTPBearer" = [])
    )
)]
pub async fn sync_device(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    require_capabilities(&user, &["config.write"]).map_err(|err| (err.status, err.message))?;
    let node_id = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;
    let (model_id, points) = crate::services::external_devices::poll_device_by_id(&state, node_id)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    Ok(Json(json!({
        "status": "ok",
        "model_id": model_id,
        "points": points
    })))
}

#[utoipa::path(
    delete,
    path = "/api/integrations/devices/{node_id}",
    tag = "integrations",
    responses(
        (status = 200, description = "External device deleted")
    ),
    params(
        ("node_id" = String, Path, description = "Device node id")
    ),
    security(
        ("HTTPBearer" = [])
    )
)]
pub async fn delete_device(
    AuthUser(user): AuthUser,
    State(state): State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    require_capabilities(&user, &["config.write"]).map_err(|err| (err.status, err.message))?;
    let node_id = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node id".to_string()))?;
    sqlx::query(
        r#"
        DELETE FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;
    Ok(Json(json!({ "status": "ok" })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/integrations/devices/catalog", get(get_device_catalog))
        .route("/integrations/devices", get(list_devices).post(create_device))
        .route("/integrations/devices/{node_id}/sync", post(sync_device))
        .route("/integrations/devices/{node_id}", delete(delete_device))
}
