use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::auth::AuthUser;
use crate::error::internal_error;
use crate::services::deployments::{
    DeploymentJob, DeploymentUserRef, HostKeyScanRequest, HostKeyScanResponse, PiDeploymentRequest,
};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/deployments/pi5", post(start_pi5_deployment))
        .route("/deployments/pi5/{job_id}", get(get_pi5_deployment))
        .route("/deployments/pi5/host-key", post(scan_pi5_host_key))
}

#[utoipa::path(
    post,
    path = "/api/deployments/pi5",
    tag = "deployments",
    request_body = PiDeploymentRequest,
    responses(
        (status = 200, description = "Deployment job", body = DeploymentJob),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn start_pi5_deployment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<PiDeploymentRequest>,
) -> Result<Json<DeploymentJob>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;
    let user_ref = DeploymentUserRef {
        id: user.id,
        email: user.email,
        role: user.role,
    };
    let job = state
        .deployments
        .create_pi5_job(payload, user_ref)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    Ok(Json(job))
}

#[utoipa::path(
    get,
    path = "/api/deployments/pi5/{job_id}",
    tag = "deployments",
    params(("job_id" = String, Path, description = "Deployment job id")),
    responses(
        (status = 200, description = "Deployment job", body = DeploymentJob),
        (status = 404, description = "Job not found"),
        (status = 401, description = "Unauthorized")
    )
)]
pub(crate) async fn get_pi5_deployment(
    State(state): State<AppState>,
    AuthUser(_user): AuthUser,
    Path(job_id): Path<String>,
) -> Result<Json<DeploymentJob>, (StatusCode, String)> {
    let Some(job) = state.deployments.get_job(job_id.trim()) else {
        return Err((
            StatusCode::NOT_FOUND,
            "Deployment job not found".to_string(),
        ));
    };
    Ok(Json(job))
}

#[utoipa::path(
    post,
    path = "/api/deployments/pi5/host-key",
    tag = "deployments",
    request_body = HostKeyScanRequest,
    responses(
        (status = 200, description = "Host key fingerprint", body = HostKeyScanResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn scan_pi5_host_key(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<HostKeyScanRequest>,
) -> Result<Json<HostKeyScanResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let manager = state.deployments.clone();
    let response = tokio::task::spawn_blocking(move || manager.scan_host_key(payload))
        .await
        .map_err(internal_error)?
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    Ok(Json(response))
}
