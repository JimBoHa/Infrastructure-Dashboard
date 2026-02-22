use crate::spool::{IncomingSample, SpoolHandle};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct HttpState {
    pub spool: SpoolHandle,
}

#[derive(Debug, Deserialize)]
struct SamplesRequest {
    samples: Vec<IncomingSample>,
}

#[derive(Debug, Serialize)]
struct SamplesResponse {
    accepted: u64,
}

async fn healthz() -> &'static str {
    "ok"
}

async fn get_status(State(state): State<HttpState>) -> Result<Json<crate::spool::SpoolStatus>, (StatusCode, String)> {
    let status = state
        .spool
        .status()
        .await
        .map_err(|err| (StatusCode::SERVICE_UNAVAILABLE, err.to_string()))?;
    Ok(Json(status))
}

async fn post_samples(
    State(state): State<HttpState>,
    Json(payload): Json<SamplesRequest>,
) -> Result<Json<SamplesResponse>, (StatusCode, String)> {
    let result = state
        .spool
        .append_samples(payload.samples)
        .await
        .map_err(|err| (StatusCode::SERVICE_UNAVAILABLE, err.to_string()))?;
    Ok(Json(SamplesResponse {
        accepted: result.accepted,
    }))
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/status", get(get_status))
        .route("/v1/samples", post(post_samples))
        .with_state(state)
}

