use axum::body::Bytes;
use axum::extract::{Path, RawQuery};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::any;
use axum::Router;

use crate::auth::OptionalAuthUser;
use crate::error::internal_error;
use crate::state::AppState;

async fn proxy(
    axum::extract::State(state): axum::extract::State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    method: Method,
    headers: HeaderMap,
    Path(path): Path<String>,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let user = user.ok_or((
        StatusCode::UNAUTHORIZED,
        "Missing or invalid token".to_string(),
    ))?;
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let Some(base) = state.config.setup_daemon_base_url.clone() else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Setup daemon base URL not configured".to_string(),
        ));
    };

    let trimmed = path.trim_start_matches('/');
    let api_path = if trimmed.starts_with("api/") {
        trimmed.to_string()
    } else {
        format!("api/{trimmed}")
    };

    let mut url = format!("{}/{}", base.trim_end_matches('/'), api_path);
    if let Some(query) = query {
        let query = query.trim();
        if !query.is_empty() {
            url.push('?');
            url.push_str(query);
        }
    }

    let request = state.http.request(method, url).body(body.to_vec());

    let mut request = request;
    if let Some(content_type) = headers.get(axum::http::header::CONTENT_TYPE) {
        request = request.header(axum::http::header::CONTENT_TYPE, content_type);
    }
    if let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) {
        request = request.header(axum::http::header::AUTHORIZATION, auth);
    }

    let response = request.send().await.map_err(internal_error)?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .cloned();
    let bytes = response.bytes().await.map_err(internal_error)?;

    let mut builder = axum::response::Response::builder().status(status);
    if let Some(content_type) = content_type {
        builder = builder.header(axum::http::header::CONTENT_TYPE, content_type);
    }
    Ok(builder
        .body(axum::body::Body::from(bytes))
        .map_err(internal_error)?)
}

pub fn router() -> Router<AppState> {
    Router::new().route("/{*path}", any(proxy))
}
