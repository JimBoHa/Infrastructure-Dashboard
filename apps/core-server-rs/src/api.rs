use axum::routing::get;
use axum::{Json, Router};

use crate::openapi::{openapi_json, router as openapi_router};
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct HealthResponse {
    pub status: String,
}

#[utoipa::path(
    get,
    path = "/healthz",
    responses(
        (status = 200, description = "OK", body = HealthResponse)
    )
)]
pub(crate) async fn healthz_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(healthz_handler))
        .nest("/api", openapi_router())
}

pub fn openapi_document() -> serde_json::Value {
    openapi_json()
}
