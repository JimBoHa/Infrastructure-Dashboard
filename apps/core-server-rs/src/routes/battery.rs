use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::services::battery_model::BatteryModelConfig;
use crate::state::AppState;

const CAP_CONFIG_WRITE: &str = "config.write";
const DEVICE_TYPE_RENOGY_BT2: &str = "renogy_bt2";

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct BatteryConfigResponse {
    pub(crate) node_id: String,
    pub(crate) battery_model: BatteryModelConfig,
    #[serde(default)]
    pub(crate) resolved_sticker_capacity_ah: Option<f64>,
    #[serde(default)]
    pub(crate) resolved_sticker_capacity_source: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct BatteryConfigRequest {
    pub(crate) battery_model: BatteryModelConfig,
}

fn validate_battery_model(cfg: &BatteryModelConfig) -> Result<(), (StatusCode, String)> {
    if !cfg.enabled {
        return Ok(());
    }
    if let Some(value) = cfg.sticker_capacity_ah {
        if !value.is_finite() || value <= 0.0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "sticker_capacity_ah must be > 0".to_string(),
            ));
        }
    }
    if !(0.0..=100.0).contains(&cfg.soc_cutoff_percent) {
        return Err((
            StatusCode::BAD_REQUEST,
            "soc_cutoff_percent must be 0..100".to_string(),
        ));
    }
    if cfg.rest_current_abs_a < 0.0 || !cfg.rest_current_abs_a.is_finite() {
        return Err((
            StatusCode::BAD_REQUEST,
            "rest_current_abs_a must be >= 0".to_string(),
        ));
    }
    if cfg.rest_minutes_required < 0 || cfg.rest_minutes_required > 24 * 60 {
        return Err((
            StatusCode::BAD_REQUEST,
            "rest_minutes_required must be 0..1440".to_string(),
        ));
    }
    if cfg.soc_anchor_max_step_percent < 0.0
        || cfg.soc_anchor_max_step_percent > 100.0
        || !cfg.soc_anchor_max_step_percent.is_finite()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "soc_anchor_max_step_percent must be 0..100".to_string(),
        ));
    }
    if cfg.capacity_estimation.min_soc_span_percent < 1.0
        || cfg.capacity_estimation.min_soc_span_percent > 100.0
        || !cfg.capacity_estimation.min_soc_span_percent.is_finite()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "capacity_estimation.min_soc_span_percent must be 1..100".to_string(),
        ));
    }
    if cfg.capacity_estimation.ema_alpha < 0.0
        || cfg.capacity_estimation.ema_alpha > 1.0
        || !cfg.capacity_estimation.ema_alpha.is_finite()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "capacity_estimation.ema_alpha must be 0..1".to_string(),
        ));
    }
    if cfg.capacity_estimation.clamp_min_ah <= 0.0
        || !cfg.capacity_estimation.clamp_min_ah.is_finite()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "capacity_estimation.clamp_min_ah must be > 0".to_string(),
        ));
    }
    if cfg.capacity_estimation.clamp_max_ah <= cfg.capacity_estimation.clamp_min_ah
        || !cfg.capacity_estimation.clamp_max_ah.is_finite()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "capacity_estimation.clamp_max_ah must be > clamp_min_ah".to_string(),
        ));
    }

    Ok(())
}

async fn resolve_sticker_capacity(
    state: &AppState,
    node_uuid: Uuid,
    cfg: &BatteryModelConfig,
) -> Result<(Option<f64>, Option<String>), (StatusCode, String)> {
    if let Some(value) = cfg.sticker_capacity_ah {
        if value.is_finite() && value > 0.0 {
            return Ok((Some(value), Some("battery_model".to_string())));
        }
    }

    let raw: Option<String> = sqlx::query_scalar(
        r#"
        SELECT desired->>'battery_capacity_ah'
        FROM device_settings
        WHERE node_id = $1 AND device_type = $2
        "#,
    )
    .bind(node_uuid)
    .bind(DEVICE_TYPE_RENOGY_BT2)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(raw) = raw else {
        return Ok((None, None));
    };
    let parsed = raw.trim().parse::<f64>().ok();
    if let Some(value) = parsed.filter(|v| v.is_finite() && *v > 0.0) {
        return Ok((Some(value), Some("renogy_desired_settings".to_string())));
    }
    Ok((None, None))
}

#[utoipa::path(
    get,
    path = "/api/battery/config/{node_id}",
    tag = "battery",
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Battery model configuration", body = BatteryConfigResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_battery_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<BatteryConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &[CAP_CONFIG_WRITE])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        config: SqlJson<JsonValue>,
    }
    let row: Option<Row> = sqlx::query_as(
        r#"
        SELECT COALESCE(config, '{}'::jsonb) as config
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

    let cfg_value = row
        .config
        .0
        .get("battery_model")
        .cloned()
        .unwrap_or(JsonValue::Null);
    let cfg: BatteryModelConfig = serde_json::from_value(cfg_value).unwrap_or_default();
    let (resolved, source) = resolve_sticker_capacity(&state, node_uuid, &cfg).await?;

    Ok(Json(BatteryConfigResponse {
        node_id,
        battery_model: cfg,
        resolved_sticker_capacity_ah: resolved,
        resolved_sticker_capacity_source: source,
    }))
}

#[utoipa::path(
    put,
    path = "/api/battery/config/{node_id}",
    tag = "battery",
    request_body = BatteryConfigRequest,
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Updated battery model configuration", body = BatteryConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn put_battery_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<BatteryConfigRequest>,
) -> Result<Json<BatteryConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &[CAP_CONFIG_WRITE])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    validate_battery_model(&payload.battery_model)?;

    let updated: Option<(SqlJson<JsonValue>,)> = sqlx::query_as(
        r#"
        UPDATE nodes
        SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{battery_model}', $2::jsonb, true)
        WHERE id = $1
        RETURNING config
        "#,
    )
    .bind(node_uuid)
    .bind(SqlJson(serde_json::to_value(&payload.battery_model).unwrap_or(JsonValue::Null)))
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some((config,)) = updated else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let cfg_value = config
        .0
        .get("battery_model")
        .cloned()
        .unwrap_or(JsonValue::Null);
    let cfg: BatteryModelConfig = serde_json::from_value(cfg_value).unwrap_or_default();
    let (resolved, source) = resolve_sticker_capacity(&state, node_uuid, &cfg).await?;

    Ok(Json(BatteryConfigResponse {
        node_id,
        battery_model: cfg,
        resolved_sticker_capacity_ah: resolved,
        resolved_sticker_capacity_source: source,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/battery/config/{node_id}",
        get(get_battery_config).put(put_battery_config),
    )
}
