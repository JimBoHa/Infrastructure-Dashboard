use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use crate::state::AppState;

const DEFAULT_DEV_ACTIVITY_PATH: &str = "/Users/Shared/FarmDashboard/setup/dev_activity.json";
const DEFAULT_TTL_SECONDS: u64 = 600;
const MAX_TTL_SECONDS: u64 = 24 * 60 * 60;

fn dev_activity_path() -> PathBuf {
    if let Ok(path) = std::env::var("CORE_DEV_ACTIVITY_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(state_dir) = std::env::var("FARM_SETUP_STATE_DIR") {
        let trimmed = state_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("dev_activity.json");
        }
    }

    for key in ["CORE_SETUP_CONFIG_PATH", "FARM_SETUP_CONFIG_PATH"] {
        if let Ok(path) = std::env::var(key) {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                continue;
            }
            let candidate = PathBuf::from(trimmed);
            if let Some(parent) = candidate.parent() {
                return parent.join("dev_activity.json");
            }
        }
    }

    PathBuf::from(DEFAULT_DEV_ACTIVITY_PATH)
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct DevActivityStatusResponse {
    active: bool,
    message: Option<String>,
    updated_at: Option<String>,
    expires_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct DevActivityHeartbeatRequest {
    ttl_seconds: Option<u64>,
    message: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DevActivityRecord {
    message: Option<String>,
    source: Option<String>,
    updated_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

async fn load_record(path: &Path) -> Result<Option<DevActivityRecord>, (StatusCode, String)> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => serde_json::from_str(&contents).map(Some).map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Failed to parse dev activity file at {}: {err}",
                    path.display()
                ),
            )
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to read dev activity file at {}: {err}",
                path.display()
            ),
        )),
    }
}

async fn write_record(path: &Path, record: &DevActivityRecord) -> Result<(), (StatusCode, String)> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Failed to create dev activity directory {}: {err}",
                    parent.display()
                ),
            )
        })?;
    }

    let payload = serde_json::to_string_pretty(record).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to serialize dev activity record: {err}"),
        )
    })?;

    let tmp_path = path.with_extension("tmp");
    tokio::fs::write(&tmp_path, payload).await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to write dev activity temp file at {}: {err}",
                tmp_path.display()
            ),
        )
    })?;

    tokio::fs::rename(&tmp_path, path).await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to replace dev activity file at {}: {err}",
                path.display()
            ),
        )
    })?;

    Ok(())
}

fn status_from_record(record: &DevActivityRecord) -> DevActivityStatusResponse {
    let default_message = "This dashboard is under active development. Automated changes may occur; refresh after updates finish.";
    DevActivityStatusResponse {
        active: true,
        message: Some(
            record
                .message
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(default_message)
                .to_string(),
        ),
        updated_at: Some(record.updated_at.to_rfc3339()),
        expires_at: Some(record.expires_at.to_rfc3339()),
    }
}

fn require_loopback(addr: SocketAddr) -> Result<(), (StatusCode, String)> {
    if addr.ip().is_loopback() {
        return Ok(());
    }
    Err((
        StatusCode::FORBIDDEN,
        "This endpoint is only available from localhost.".to_string(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/dev/activity",
    tag = "dev",
    responses((status = 200, description = "Dev activity status", body = DevActivityStatusResponse))
)]
pub(crate) async fn get_dev_activity_status(
) -> Result<Json<DevActivityStatusResponse>, (StatusCode, String)> {
    let path = dev_activity_path();
    let record = match load_record(&path).await {
        Ok(record) => record,
        Err((_, message)) => {
            tracing::warn!(path = %path.display(), error = %message, "dev activity load failed");
            return Ok(Json(DevActivityStatusResponse {
                active: false,
                message: None,
                updated_at: None,
                expires_at: None,
            }));
        }
    };

    let Some(record) = record else {
        return Ok(Json(DevActivityStatusResponse {
            active: false,
            message: None,
            updated_at: None,
            expires_at: None,
        }));
    };

    if Utc::now() >= record.expires_at {
        if let Err(err) = tokio::fs::remove_file(&path).await {
            tracing::debug!(path = %path.display(), error = %err, "failed to remove expired dev activity file");
        }
        return Ok(Json(DevActivityStatusResponse {
            active: false,
            message: None,
            updated_at: None,
            expires_at: None,
        }));
    }

    Ok(Json(status_from_record(&record)))
}

#[utoipa::path(
    post,
    path = "/api/dev/activity/heartbeat",
    tag = "dev",
    request_body = DevActivityHeartbeatRequest,
    responses((status = 200, description = "Updated dev activity status", body = DevActivityStatusResponse))
)]
pub(crate) async fn heartbeat_dev_activity(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<DevActivityHeartbeatRequest>,
) -> Result<Json<DevActivityStatusResponse>, (StatusCode, String)> {
    require_loopback(addr)?;

    let ttl_seconds = payload.ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS);
    let ttl_seconds = ttl_seconds.clamp(5, MAX_TTL_SECONDS);

    let now = Utc::now();
    let record = DevActivityRecord {
        message: payload.message,
        source: payload.source,
        updated_at: now,
        expires_at: now + chrono::Duration::seconds(ttl_seconds as i64),
    };

    let path = dev_activity_path();
    write_record(&path, &record).await?;

    Ok(Json(status_from_record(&record)))
}

#[utoipa::path(
    delete,
    path = "/api/dev/activity",
    tag = "dev",
    responses((status = 204, description = "Cleared dev activity status"))
)]
pub(crate) async fn clear_dev_activity(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> Result<StatusCode, (StatusCode, String)> {
    require_loopback(addr)?;

    let path = dev_activity_path();
    match tokio::fs::remove_file(&path).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(StatusCode::NO_CONTENT),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to remove dev activity file at {}: {err}",
                path.display()
            ),
        )),
    }
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/dev/activity",
            get(get_dev_activity_status).delete(clear_dev_activity),
        )
        .route("/dev/activity/heartbeat", post(heartbeat_dev_activity))
}
