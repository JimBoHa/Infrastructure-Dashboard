use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};

use crate::auth::AuthUser;
use crate::services::cloud_sync::{
    self, CloudAccessSettingsPatch, CloudIngestResult, CloudRole, CloudSyncPayload,
};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct CloudAccessResponse {
    role: String,
    local_site_key: Option<String>,
    cloud_server_base_url: Option<String>,
    sync_interval_seconds: u64,
    sync_enabled: bool,
    last_attempt_at: Option<String>,
    last_success_at: Option<String>,
    last_error: Option<String>,
    registered_site_count: u64,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct CloudAccessUpdateRequest {
    cloud_server_base_url: Option<String>,
    sync_interval_seconds: Option<u64>,
    sync_enabled: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct CloudSiteResponse {
    site_id: String,
    site_name: String,
    key_fingerprint: String,
    enabled: bool,
    created_at: Option<String>,
    updated_at: Option<String>,
    last_ingested_at: Option<String>,
    last_payload_bytes: Option<u64>,
    last_metrics_count: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct RegisterCloudSiteRequest {
    site_name: String,
    site_key: String,
}

fn to_cloud_access_response(
    role: CloudRole,
    local_state: Option<cloud_sync::LocalCloudAccessState>,
    registered_site_count: u64,
) -> CloudAccessResponse {
    let local_site_key = local_state
        .as_ref()
        .map(|state| state.local_site_key.clone());
    let cloud_server_base_url = local_state
        .as_ref()
        .and_then(|state| state.settings.cloud_server_base_url.clone());
    let sync_interval_seconds = local_state
        .as_ref()
        .map(|state| state.settings.sync_interval_seconds)
        .unwrap_or(300);
    let sync_enabled = local_state
        .as_ref()
        .map(|state| state.settings.sync_enabled)
        .unwrap_or(false);
    let last_attempt_at = local_state
        .as_ref()
        .and_then(|state| state.status.last_attempt_at.clone());
    let last_success_at = local_state
        .as_ref()
        .and_then(|state| state.status.last_success_at.clone());
    let last_error = local_state
        .as_ref()
        .and_then(|state| state.status.last_error.clone());

    CloudAccessResponse {
        role: role.as_str().to_string(),
        local_site_key,
        cloud_server_base_url,
        sync_interval_seconds,
        sync_enabled,
        last_attempt_at,
        last_success_at,
        last_error,
        registered_site_count,
    }
}

impl From<cloud_sync::CloudSiteSummary> for CloudSiteResponse {
    fn from(value: cloud_sync::CloudSiteSummary) -> Self {
        Self {
            site_id: value.site_id,
            site_name: value.site_name,
            key_fingerprint: value.key_fingerprint,
            enabled: value.enabled,
            created_at: value.created_at,
            updated_at: value.updated_at,
            last_ingested_at: value.last_ingested_at,
            last_payload_bytes: value.last_payload_bytes,
            last_metrics_count: value.last_metrics_count,
        }
    }
}

fn map_internal_error(err: anyhow::Error) -> (StatusCode, String) {
    tracing::error!(error = %err, "cloud access internal error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal server error".to_string(),
    )
}

fn map_bad_request(err: anyhow::Error) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, err.to_string())
}

fn ensure_cloud_role() -> Result<(), (StatusCode, String)> {
    if cloud_sync::runtime_cloud_role() != CloudRole::Cloud {
        return Err((
            StatusCode::BAD_REQUEST,
            "This endpoint is available only when CORE_CLOUD_ROLE=cloud".to_string(),
        ));
    }
    Ok(())
}

fn extract_site_key(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get("x-cloud-site-key") {
        if let Ok(text) = value.to_str() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    let header = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let bearer = header.strip_prefix("Bearer ")?.trim();
    if bearer.is_empty() {
        return None;
    }
    Some(bearer.to_string())
}

#[utoipa::path(
    get,
    path = "/api/cloud/access",
    tag = "connection",
    responses(
        (status = 200, description = "Cloud access status", body = CloudAccessResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_cloud_access(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<CloudAccessResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let role = cloud_sync::runtime_cloud_role();
    let registered_site_count = cloud_sync::count_registered_sites(&state.db)
        .await
        .map_err(map_internal_error)?;

    let response = if role == CloudRole::Local {
        let local = cloud_sync::load_local_cloud_access_state(&state.db)
            .await
            .map_err(map_internal_error)?;
        to_cloud_access_response(role, Some(local), registered_site_count)
    } else {
        to_cloud_access_response(role, None, registered_site_count)
    };

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/cloud/access",
    tag = "connection",
    request_body = CloudAccessUpdateRequest,
    responses(
        (status = 200, description = "Updated cloud access status", body = CloudAccessResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_cloud_access(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<CloudAccessUpdateRequest>,
) -> Result<Json<CloudAccessResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if cloud_sync::runtime_cloud_role() != CloudRole::Local {
        return Err((
            StatusCode::BAD_REQUEST,
            "Cloud access settings are editable only when CORE_CLOUD_ROLE=local".to_string(),
        ));
    }

    let patch = CloudAccessSettingsPatch {
        cloud_server_base_url: payload.cloud_server_base_url,
        sync_interval_seconds: payload.sync_interval_seconds,
        sync_enabled: payload.sync_enabled,
    };

    let local = cloud_sync::update_local_settings(&state.db, patch)
        .await
        .map_err(map_bad_request)?;
    let site_count = cloud_sync::count_registered_sites(&state.db)
        .await
        .map_err(map_internal_error)?;
    Ok(Json(to_cloud_access_response(
        CloudRole::Local,
        Some(local),
        site_count,
    )))
}

#[utoipa::path(
    post,
    path = "/api/cloud/access/key/rotate",
    tag = "connection",
    responses(
        (status = 200, description = "Rotated cloud key", body = CloudAccessResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn rotate_cloud_key(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<CloudAccessResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if cloud_sync::runtime_cloud_role() != CloudRole::Local {
        return Err((
            StatusCode::BAD_REQUEST,
            "Cloud key rotation is available only when CORE_CLOUD_ROLE=local".to_string(),
        ));
    }

    cloud_sync::rotate_local_site_key(&state.db)
        .await
        .map_err(map_internal_error)?;
    let local = cloud_sync::load_local_cloud_access_state(&state.db)
        .await
        .map_err(map_internal_error)?;
    let site_count = cloud_sync::count_registered_sites(&state.db)
        .await
        .map_err(map_internal_error)?;
    Ok(Json(to_cloud_access_response(
        CloudRole::Local,
        Some(local),
        site_count,
    )))
}

#[utoipa::path(
    get,
    path = "/api/cloud/sites",
    tag = "connection",
    responses(
        (status = 200, description = "Registered cloud sites", body = Vec<CloudSiteResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_cloud_sites(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<CloudSiteResponse>>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let sites = cloud_sync::list_registered_sites(&state.db)
        .await
        .map_err(map_internal_error)?
        .into_iter()
        .map(CloudSiteResponse::from)
        .collect::<Vec<_>>();
    Ok(Json(sites))
}

#[utoipa::path(
    post,
    path = "/api/cloud/sites",
    tag = "connection",
    request_body = RegisterCloudSiteRequest,
    responses(
        (status = 200, description = "Registered site", body = CloudSiteResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn register_cloud_site(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<RegisterCloudSiteRequest>,
) -> Result<Json<CloudSiteResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;
    ensure_cloud_role()?;

    let site = cloud_sync::register_cloud_site(&state.db, &payload.site_name, &payload.site_key)
        .await
        .map_err(map_bad_request)?;
    Ok(Json(site.into()))
}

#[utoipa::path(
    delete,
    path = "/api/cloud/sites/{site_id}",
    tag = "connection",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn remove_cloud_site(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(site_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;
    ensure_cloud_role()?;

    cloud_sync::delete_cloud_site(&state.db, &site_id)
        .await
        .map_err(map_bad_request)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/cloud/sites/{site_id}/snapshot",
    tag = "connection",
    params(("site_id" = String, Path, description = "Site ID")),
    responses(
        (status = 200, description = "Latest site snapshot", body = serde_json::Value),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Snapshot not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_cloud_site_snapshot(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(site_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let snapshot = cloud_sync::load_cloud_site_snapshot(&state.config.data_root, &site_id)
        .await
        .map_err(map_internal_error)?;

    let Some(snapshot) = snapshot else {
        return Err((
            StatusCode::NOT_FOUND,
            "No snapshot for this site".to_string(),
        ));
    };

    Ok(Json(snapshot))
}

#[utoipa::path(
    post,
    path = "/api/cloud/ingest",
    tag = "connection",
    request_body = CloudSyncPayload,
    responses(
        (status = 200, description = "Ingest accepted", body = CloudIngestResult),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Invalid site key"),
        (status = 403, description = "Cloud ingest disabled")
    )
)]
pub(crate) async fn ingest_cloud_payload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CloudSyncPayload>,
) -> Result<Json<CloudIngestResult>, (StatusCode, String)> {
    if cloud_sync::runtime_cloud_role() != CloudRole::Cloud {
        return Err((
            StatusCode::FORBIDDEN,
            "Cloud ingest is disabled when CORE_CLOUD_ROLE is not cloud".to_string(),
        ));
    }

    let Some(site_key) = extract_site_key(&headers) else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Missing site key (x-cloud-site-key or Authorization: Bearer <key>)".to_string(),
        ));
    };

    let payload_len = serde_json::to_vec(&payload)
        .map(|bytes| bytes.len())
        .unwrap_or_default();

    let result = cloud_sync::ingest_cloud_payload(&state, &site_key, &payload, payload_len)
        .await
        .map_err(|err| {
            let message = err.to_string();
            if message.to_ascii_lowercase().contains("site key") {
                return (StatusCode::UNAUTHORIZED, "Invalid site key".to_string());
            }
            map_internal_error(err)
        })?;

    Ok(Json(result))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/cloud/access",
            get(get_cloud_access).post(update_cloud_access),
        )
        .route("/cloud/access/key/rotate", post(rotate_cloud_key))
        .route(
            "/cloud/sites",
            get(list_cloud_sites).post(register_cloud_site),
        )
        .route("/cloud/sites/{site_id}", delete(remove_cloud_site))
        .route(
            "/cloud/sites/{site_id}/snapshot",
            get(get_cloud_site_snapshot),
        )
        .route("/cloud/ingest", post(ingest_cloud_payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_site_key_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("x-cloud-site-key", "abc123".parse().unwrap());
        assert_eq!(extract_site_key(&headers).as_deref(), Some("abc123"));

        let mut bearer = HeaderMap::new();
        bearer.insert(AUTHORIZATION, "Bearer testkey".parse().unwrap());
        assert_eq!(extract_site_key(&bearer).as_deref(), Some("testkey"));
    }
}
