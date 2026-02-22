use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use std::path::Path;

use crate::auth::AuthUser;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ControllerRuntimeConfigResponse {
    mqtt_username: Option<String>,
    mqtt_password_configured: bool,
    enable_analytics_feeds: bool,
    enable_forecast_ingestion: bool,
    analytics_feed_poll_interval_seconds: u64,
    forecast_poll_interval_seconds: u64,
    schedule_poll_interval_seconds: u64,
    offline_threshold_seconds: u64,
    sidecar_mqtt_topic_prefix: String,
    sidecar_mqtt_keepalive_secs: u64,
    sidecar_enable_mqtt_listener: bool,
    sidecar_batch_size: u64,
    sidecar_flush_interval_ms: u64,
    sidecar_max_queue: u64,
    sidecar_status_poll_interval_ms: u64,
    config_path: String,
    updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ControllerRuntimeConfigPatchRequest {
    mqtt_username: Option<String>,
    mqtt_password: Option<String>,
    enable_analytics_feeds: Option<bool>,
    enable_forecast_ingestion: Option<bool>,
    analytics_feed_poll_interval_seconds: Option<u64>,
    forecast_poll_interval_seconds: Option<u64>,
    schedule_poll_interval_seconds: Option<u64>,
    offline_threshold_seconds: Option<u64>,
    sidecar_mqtt_topic_prefix: Option<String>,
    sidecar_mqtt_keepalive_secs: Option<u64>,
    sidecar_enable_mqtt_listener: Option<bool>,
    sidecar_batch_size: Option<u64>,
    sidecar_flush_interval_ms: Option<u64>,
    sidecar_max_queue: Option<u64>,
    sidecar_status_poll_interval_ms: Option<u64>,
}

async fn load_setup_config(path: &Path) -> Result<JsonValue, (StatusCode, String)> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => serde_json::from_str(&contents).map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse setup config at {}: {err}", path.display()),
            )
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(JsonValue::Object(Default::default()))
        }
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read setup config at {}: {err}", path.display()),
        )),
    }
}

async fn setup_config_updated_at(path: &Path) -> Option<String> {
    let meta = tokio::fs::metadata(path).await.ok()?;
    let modified = meta.modified().ok()?;
    let ts: chrono::DateTime<chrono::Utc> = modified.into();
    Some(ts.to_rfc3339())
}

fn opt_trimmed_string(value: Option<&JsonValue>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn string_value(value: Option<&JsonValue>, fallback: &str) -> String {
    opt_trimmed_string(value).unwrap_or_else(|| fallback.to_string())
}

fn bool_value(value: Option<&JsonValue>, fallback: bool) -> bool {
    value.and_then(|v| v.as_bool()).unwrap_or(fallback)
}

fn u64_value(value: Option<&JsonValue>, fallback: u64) -> u64 {
    value.and_then(|v| v.as_u64()).unwrap_or(fallback)
}

fn response_from_value(
    state: &AppState,
    path: &Path,
    value: &JsonValue,
) -> ControllerRuntimeConfigResponse {
    let empty = serde_json::Map::new();
    let obj = value.as_object().unwrap_or(&empty);

    let mqtt_username =
        opt_trimmed_string(obj.get("mqtt_username")).or_else(|| state.config.mqtt_username.clone());
    let mqtt_password_configured = opt_trimmed_string(obj.get("mqtt_password")).is_some()
        || state
            .config
            .mqtt_password
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some();

    ControllerRuntimeConfigResponse {
        mqtt_username,
        mqtt_password_configured,
        enable_analytics_feeds: bool_value(
            obj.get("enable_analytics_feeds"),
            state.config.enable_analytics_feeds,
        ),
        enable_forecast_ingestion: bool_value(
            obj.get("enable_forecast_ingestion"),
            state.config.enable_forecast_ingestion,
        ),
        analytics_feed_poll_interval_seconds: u64_value(
            obj.get("analytics_feed_poll_interval_seconds"),
            state.config.analytics_feed_poll_interval_seconds,
        ),
        forecast_poll_interval_seconds: u64_value(
            obj.get("forecast_poll_interval_seconds"),
            state.config.forecast_poll_interval_seconds,
        ),
        schedule_poll_interval_seconds: u64_value(
            obj.get("schedule_poll_interval_seconds"),
            state.config.schedule_poll_interval_seconds,
        ),
        offline_threshold_seconds: u64_value(obj.get("offline_threshold_seconds"), 5),
        sidecar_mqtt_topic_prefix: string_value(obj.get("sidecar_mqtt_topic_prefix"), "iot"),
        sidecar_mqtt_keepalive_secs: u64_value(obj.get("sidecar_mqtt_keepalive_secs"), 30),
        sidecar_enable_mqtt_listener: bool_value(obj.get("sidecar_enable_mqtt_listener"), true),
        sidecar_batch_size: u64_value(obj.get("sidecar_batch_size"), 500),
        sidecar_flush_interval_ms: u64_value(obj.get("sidecar_flush_interval_ms"), 750),
        sidecar_max_queue: u64_value(
            obj.get("sidecar_max_queue"),
            u64_value(obj.get("sidecar_batch_size"), 500).saturating_mul(10),
        ),
        sidecar_status_poll_interval_ms: u64_value(
            obj.get("sidecar_status_poll_interval_ms"),
            1000,
        ),
        config_path: path.display().to_string(),
        updated_at: None,
    }
}

#[utoipa::path(
    get,
    path = "/api/setup/controller/runtime-config",
    tag = "setup",
    responses((status = 200, description = "Controller runtime config", body = ControllerRuntimeConfigResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_controller_runtime_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<ControllerRuntimeConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let path = crate::config::setup_config_path();
    let value = load_setup_config(&path).await?;
    let mut response = response_from_value(&state, &path, &value);
    response.updated_at = setup_config_updated_at(&path).await;
    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/api/setup/controller/runtime-config",
    tag = "setup",
    request_body = ControllerRuntimeConfigPatchRequest,
    responses((status = 200, description = "Updated controller runtime config", body = ControllerRuntimeConfigResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn patch_controller_runtime_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<ControllerRuntimeConfigPatchRequest>,
) -> Result<Json<ControllerRuntimeConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if let Some(value) = payload.analytics_feed_poll_interval_seconds {
        if value < 60 {
            return Err((
                StatusCode::BAD_REQUEST,
                "analytics_feed_poll_interval_seconds must be at least 60".to_string(),
            ));
        }
    }
    if let Some(value) = payload.forecast_poll_interval_seconds {
        if value < 300 {
            return Err((
                StatusCode::BAD_REQUEST,
                "forecast_poll_interval_seconds must be at least 300".to_string(),
            ));
        }
    }
    if let Some(value) = payload.schedule_poll_interval_seconds {
        if value < 5 {
            return Err((
                StatusCode::BAD_REQUEST,
                "schedule_poll_interval_seconds must be at least 5".to_string(),
            ));
        }
    }
    if let Some(value) = payload.offline_threshold_seconds {
        if value < 1 {
            return Err((
                StatusCode::BAD_REQUEST,
                "offline_threshold_seconds must be at least 1".to_string(),
            ));
        }
    }
    if let Some(value) = payload.sidecar_mqtt_keepalive_secs {
        if value < 5 {
            return Err((
                StatusCode::BAD_REQUEST,
                "sidecar_mqtt_keepalive_secs must be at least 5".to_string(),
            ));
        }
    }
    if let Some(value) = payload.sidecar_batch_size {
        if value < 10 {
            return Err((
                StatusCode::BAD_REQUEST,
                "sidecar_batch_size must be at least 10".to_string(),
            ));
        }
    }
    if let Some(value) = payload.sidecar_flush_interval_ms {
        if value < 50 {
            return Err((
                StatusCode::BAD_REQUEST,
                "sidecar_flush_interval_ms must be at least 50".to_string(),
            ));
        }
    }
    if let Some(value) = payload.sidecar_max_queue {
        if value < 10 {
            return Err((
                StatusCode::BAD_REQUEST,
                "sidecar_max_queue must be at least 10".to_string(),
            ));
        }
    }
    if let Some(value) = payload.sidecar_status_poll_interval_ms {
        if value < 100 {
            return Err((
                StatusCode::BAD_REQUEST,
                "sidecar_status_poll_interval_ms must be at least 100".to_string(),
            ));
        }
    }

    let path = crate::config::setup_config_path();
    let mut value = load_setup_config(&path).await?;
    let obj = value.as_object_mut().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Setup config at {} must be a JSON object", path.display()),
        )
    })?;

    if let Some(username) = payload.mqtt_username {
        let trimmed = username.trim().to_string();
        if trimmed.is_empty() {
            obj.remove("mqtt_username");
        } else {
            obj.insert("mqtt_username".to_string(), JsonValue::String(trimmed));
        }
    }
    if let Some(password) = payload.mqtt_password {
        let trimmed = password.trim().to_string();
        if trimmed.is_empty() {
            obj.remove("mqtt_password");
        } else {
            obj.insert("mqtt_password".to_string(), JsonValue::String(trimmed));
        }
    }
    if let Some(enabled) = payload.enable_analytics_feeds {
        obj.insert(
            "enable_analytics_feeds".to_string(),
            JsonValue::Bool(enabled),
        );
    }
    if let Some(enabled) = payload.enable_forecast_ingestion {
        obj.insert(
            "enable_forecast_ingestion".to_string(),
            JsonValue::Bool(enabled),
        );
    }
    if let Some(value) = payload.analytics_feed_poll_interval_seconds {
        obj.insert(
            "analytics_feed_poll_interval_seconds".to_string(),
            JsonValue::Number(value.into()),
        );
    }
    if let Some(value) = payload.forecast_poll_interval_seconds {
        obj.insert(
            "forecast_poll_interval_seconds".to_string(),
            JsonValue::Number(value.into()),
        );
    }
    if let Some(value) = payload.schedule_poll_interval_seconds {
        obj.insert(
            "schedule_poll_interval_seconds".to_string(),
            JsonValue::Number(value.into()),
        );
    }
    if let Some(value) = payload.offline_threshold_seconds {
        obj.insert(
            "offline_threshold_seconds".to_string(),
            JsonValue::Number(value.into()),
        );
    }

    if let Some(prefix) = payload.sidecar_mqtt_topic_prefix {
        let trimmed = prefix.trim().to_string();
        if trimmed.is_empty() {
            obj.remove("sidecar_mqtt_topic_prefix");
        } else {
            obj.insert(
                "sidecar_mqtt_topic_prefix".to_string(),
                JsonValue::String(trimmed),
            );
        }
    }
    if let Some(value) = payload.sidecar_mqtt_keepalive_secs {
        obj.insert(
            "sidecar_mqtt_keepalive_secs".to_string(),
            JsonValue::Number(value.into()),
        );
    }
    if let Some(enabled) = payload.sidecar_enable_mqtt_listener {
        obj.insert(
            "sidecar_enable_mqtt_listener".to_string(),
            JsonValue::Bool(enabled),
        );
    }
    if let Some(value) = payload.sidecar_batch_size {
        obj.insert(
            "sidecar_batch_size".to_string(),
            JsonValue::Number(value.into()),
        );
    }
    if let Some(value) = payload.sidecar_flush_interval_ms {
        obj.insert(
            "sidecar_flush_interval_ms".to_string(),
            JsonValue::Number(value.into()),
        );
    }
    if let Some(value) = payload.sidecar_max_queue {
        obj.insert(
            "sidecar_max_queue".to_string(),
            JsonValue::Number(value.into()),
        );
    }
    if let Some(value) = payload.sidecar_status_poll_interval_ms {
        obj.insert(
            "sidecar_status_poll_interval_ms".to_string(),
            JsonValue::Number(value.into()),
        );
    }

    let Some(parent) = path.parent() else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Invalid setup config path {}", path.display()),
        ));
    };
    tokio::fs::create_dir_all(parent).await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to create setup config dir {}: {err}",
                parent.display()
            ),
        )
    })?;

    let contents = serde_json::to_string_pretty(&value).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode setup config: {err}"),
        )
    })?;

    let tmp_path = path.with_extension("json.tmp");
    tokio::fs::write(&tmp_path, contents).await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to write setup config at {}: {err}",
                tmp_path.display()
            ),
        )
    })?;
    tokio::fs::rename(&tmp_path, &path).await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to atomically replace setup config at {}: {err}",
                path.display()
            ),
        )
    })?;

    let mut response = response_from_value(&state, &path, &value);
    response.updated_at = setup_config_updated_at(&path).await;
    Ok(Json(response))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new().route(
        "/setup/controller/runtime-config",
        get(get_controller_runtime_config).post(patch_controller_runtime_config),
    )
}
