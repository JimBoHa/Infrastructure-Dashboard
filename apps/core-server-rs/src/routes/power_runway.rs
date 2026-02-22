use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::services::power_runway::PowerRunwayConfig;
use crate::state::AppState;

const CAP_CONFIG_WRITE: &str = "config.write";
const UNIT_WATTS: &str = "W";

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct PowerRunwayConfigResponse {
    pub(crate) node_id: String,
    pub(crate) power_runway: PowerRunwayConfig,
    #[serde(default)]
    pub(crate) load_sensors_valid: bool,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct PowerRunwayConfigRequest {
    pub(crate) power_runway: PowerRunwayConfig,
}

fn validate_config(cfg: &PowerRunwayConfig) -> Result<(), (StatusCode, String)> {
    if !cfg.enabled {
        return Ok(());
    }
    if cfg.load_sensor_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "load_sensor_ids is required when enabled".to_string(),
        ));
    }
    if cfg.history_days < 1 || cfg.history_days > 30 {
        return Err((
            StatusCode::BAD_REQUEST,
            "history_days must be 1..30".to_string(),
        ));
    }
    if !cfg.pv_derate.is_finite() || cfg.pv_derate < 0.0 || cfg.pv_derate > 1.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "pv_derate must be 0..1".to_string(),
        ));
    }
    if cfg.projection_days < 1 || cfg.projection_days > 14 {
        return Err((
            StatusCode::BAD_REQUEST,
            "projection_days must be 1..14".to_string(),
        ));
    }
    Ok(())
}

async fn validate_load_sensors(db: &sqlx::PgPool, sensor_ids: &[String]) -> Result<bool, (StatusCode, String)> {
    if sensor_ids.is_empty() {
        return Ok(false);
    }
    let cleaned: Vec<String> = sensor_ids
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if cleaned.is_empty() {
        return Ok(false);
    }
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM sensors
        WHERE sensor_id = ANY($1)
          AND deleted_at IS NULL
          AND unit = $2
        "#,
    )
    .bind(&cleaned)
    .bind(UNIT_WATTS)
    .fetch_one(db)
    .await
    .map_err(map_db_error)?;

    Ok(count == cleaned.len() as i64)
}

#[utoipa::path(
    get,
    path = "/api/power/runway/config/{node_id}",
    tag = "power",
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Power runway configuration", body = PowerRunwayConfigResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_power_runway_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<PowerRunwayConfigResponse>, (StatusCode, String)> {
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
        .get("power_runway")
        .cloned()
        .unwrap_or(JsonValue::Null);
    let cfg: PowerRunwayConfig = serde_json::from_value(cfg_value).unwrap_or_default();
    let valid = validate_load_sensors(&state.db, &cfg.load_sensor_ids).await?;

    Ok(Json(PowerRunwayConfigResponse {
        node_id,
        power_runway: cfg,
        load_sensors_valid: valid,
    }))
}

#[utoipa::path(
    put,
    path = "/api/power/runway/config/{node_id}",
    tag = "power",
    request_body = PowerRunwayConfigRequest,
    params(("node_id" = String, Path, description = "Node UUID")),
    responses(
        (status = 200, description = "Updated power runway configuration", body = PowerRunwayConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn put_power_runway_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<PowerRunwayConfigRequest>,
) -> Result<Json<PowerRunwayConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &[CAP_CONFIG_WRITE])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    validate_config(&payload.power_runway)?;
    let valid = validate_load_sensors(&state.db, &payload.power_runway.load_sensor_ids).await?;
    if payload.power_runway.enabled && !valid {
        return Err((
            StatusCode::BAD_REQUEST,
            "load_sensor_ids must reference existing watt sensors".to_string(),
        ));
    }

    let updated: Option<(SqlJson<JsonValue>,)> = sqlx::query_as(
        r#"
        UPDATE nodes
        SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{power_runway}', $2::jsonb, true)
        WHERE id = $1
        RETURNING config
        "#,
    )
    .bind(node_uuid)
    .bind(SqlJson(serde_json::to_value(&payload.power_runway).unwrap_or(JsonValue::Null)))
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some((config,)) = updated else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let cfg_value = config
        .0
        .get("power_runway")
        .cloned()
        .unwrap_or(JsonValue::Null);
    let cfg: PowerRunwayConfig = serde_json::from_value(cfg_value).unwrap_or_default();
    let valid_after = validate_load_sensors(&state.db, &cfg.load_sensor_ids).await?;

    Ok(Json(PowerRunwayConfigResponse {
        node_id,
        power_runway: cfg,
        load_sensors_valid: valid_after,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/power/runway/config/{node_id}",
        get(get_power_runway_config).put(put_power_runway_config),
    )
}
