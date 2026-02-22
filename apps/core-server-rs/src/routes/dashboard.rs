use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use serde_json::Value as JsonValue;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::routes::alarms::{AlarmEventResponse, AlarmResponse};
use crate::routes::backups::RetentionConfigResponse;
use crate::routes::connection::ConnectionResponse;
use crate::routes::discovery::AdoptionCandidate;
use crate::routes::nodes::NodeResponse;
use crate::routes::outputs::OutputResponse;
use crate::routes::schedules::ScheduleResponse;
use crate::routes::sensors::SensorResponse;
use crate::routes::users::UserResponse;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct DashboardBackupEntry {
    id: String,
    node_id: String,
    node_name: Option<String>,
    captured_at: Option<String>,
    size_bytes: Option<u64>,
    path: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct DashboardSnapshot {
    timestamp: String,
    nodes: Vec<NodeResponse>,
    sensors: Vec<SensorResponse>,
    outputs: Vec<OutputResponse>,
    users: Vec<UserResponse>,
    schedules: Vec<ScheduleResponse>,
    alarms: Vec<AlarmResponse>,
    alarm_events: Vec<AlarmEventResponse>,
    analytics: JsonValue,
    backups: Vec<DashboardBackupEntry>,
    backup_retention: Option<RetentionConfigResponse>,
    adoption: Vec<AdoptionCandidate>,
    connection: ConnectionResponse,
    trend_series: Vec<JsonValue>,
}

fn has_any_capability(user: &crate::auth::AuthenticatedUser, options: &[&str]) -> bool {
    options.iter().any(|cap| user.capabilities.contains(*cap))
}

async fn build_snapshot(
    state: &AppState,
    headers: &HeaderMap,
    user: &crate::auth::AuthenticatedUser,
) -> Result<DashboardSnapshot, (StatusCode, String)> {
    let can_nodes = has_any_capability(user, &["nodes.view", "config.write"]);
    let can_sensors = has_any_capability(user, &["sensors.view", "config.write"]);
    let can_outputs = has_any_capability(user, &["outputs.view", "config.write"]);
    let can_schedules =
        has_any_capability(user, &["schedules.view", "schedules.write", "config.write"]);
    let can_alerts = has_any_capability(user, &["alerts.view", "config.write"]);
    let can_users = has_any_capability(user, &["users.manage"]);
    let can_analytics = has_any_capability(user, &["analytics.view", "config.write"]);

    let nodes = if can_nodes {
        crate::routes::nodes::fetch_nodes(&state.db)
            .await
            .map_err(map_db_error)?
    } else {
        vec![]
    };
    let sensors = if can_sensors {
        crate::routes::sensors::fetch_sensors(&state.db, None, false)
            .await
            .map_err(map_db_error)?
    } else {
        vec![]
    };
    let outputs = if can_outputs {
        crate::routes::outputs::fetch_outputs(&state.db, None)
            .await
            .map_err(map_db_error)?
    } else {
        vec![]
    };
    let users = if can_users {
        crate::routes::users::fetch_users(&state.db)
            .await
            .map_err(map_db_error)?
    } else {
        vec![]
    };
    let schedules = if can_schedules {
        crate::routes::schedules::fetch_schedules(&state.db)
            .await
            .map_err(map_db_error)?
    } else {
        vec![]
    };
    let alarms = if can_alerts {
        crate::routes::alarms::fetch_alarms(&state.db)
            .await
            .map_err(map_db_error)?
    } else {
        vec![]
    };
    let alarm_events = if can_alerts {
        crate::routes::alarms::fetch_alarm_events(&state.db, 250)
            .await
            .map_err(map_db_error)?
    } else {
        vec![]
    };

    // Expensive scanning is intentionally removed from the default snapshot hot-path.
    // Use dedicated endpoints (`/api/backups`, `/api/scan`) for on-demand refresh.
    let backups: Vec<DashboardBackupEntry> = vec![];
    let adoption: Vec<AdoptionCandidate> = vec![];

    let analytics = if can_analytics {
        build_analytics_bundle(state, user).await
    } else {
        JsonValue::Null
    };

    Ok(DashboardSnapshot {
        timestamp: Utc::now().to_rfc3339(),
        nodes,
        sensors,
        outputs,
        users,
        schedules,
        alarms,
        alarm_events,
        analytics,
        backups,
        backup_retention: None,
        adoption,
        connection: crate::routes::connection::connection_for_request(state, headers).await,
        trend_series: vec![],
    })
}

#[utoipa::path(
    get,
    path = "/api/dashboard/state",
    tag = "dashboard",
    responses(
        (status = 200, description = "Dashboard snapshot", body = DashboardSnapshot),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal error")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn dashboard_state(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    headers: HeaderMap,
) -> Result<Json<DashboardSnapshot>, (StatusCode, String)> {
    let snapshot = build_snapshot(&state, &headers, &user).await?;
    Ok(Json(snapshot))
}

async fn build_analytics_bundle(
    state: &AppState,
    user: &crate::auth::AuthenticatedUser,
) -> JsonValue {
    let now = Utc::now().to_rfc3339();
    let mut errors: serde_json::Map<String, JsonValue> = serde_json::Map::new();

    let user = user.clone();
    let power = match crate::routes::analytics::power(
        axum::extract::State(state.clone()),
        crate::auth::AuthUser(user.clone()),
    )
    .await
    {
        Ok(Json(payload)) => serde_json::to_value(payload).unwrap_or_else(|err| {
            errors.insert(
                "power".to_string(),
                serde_json::json!({ "error": err.to_string() }),
            );
            JsonValue::Null
        }),
        Err((status, message)) => {
            errors.insert(
                "power".to_string(),
                serde_json::json!({ "status": status.as_u16(), "error": message }),
            );
            JsonValue::Null
        }
    };

    let water = match crate::routes::analytics::water(
        axum::extract::State(state.clone()),
        crate::auth::AuthUser(user.clone()),
    )
    .await
    {
        Ok(Json(payload)) => serde_json::to_value(payload).unwrap_or_else(|err| {
            errors.insert(
                "water".to_string(),
                serde_json::json!({ "error": err.to_string() }),
            );
            JsonValue::Null
        }),
        Err((status, message)) => {
            errors.insert(
                "water".to_string(),
                serde_json::json!({ "status": status.as_u16(), "error": message }),
            );
            JsonValue::Null
        }
    };

    let soil = match crate::routes::analytics::soil(
        axum::extract::State(state.clone()),
        crate::auth::AuthUser(user.clone()),
    )
    .await
    {
        Ok(Json(payload)) => serde_json::to_value(payload).unwrap_or_else(|err| {
            errors.insert(
                "soil".to_string(),
                serde_json::json!({ "error": err.to_string() }),
            );
            JsonValue::Null
        }),
        Err((status, message)) => {
            errors.insert(
                "soil".to_string(),
                serde_json::json!({ "status": status.as_u16(), "error": message }),
            );
            JsonValue::Null
        }
    };

    let status = match crate::routes::analytics::status(
        axum::extract::State(state.clone()),
        crate::auth::AuthUser(user.clone()),
    )
    .await
    {
        Ok(Json(payload)) => serde_json::to_value(payload).unwrap_or_else(|err| {
            errors.insert(
                "status".to_string(),
                serde_json::json!({ "error": err.to_string() }),
            );
            JsonValue::Null
        }),
        Err((status, message)) => {
            errors.insert(
                "status".to_string(),
                serde_json::json!({ "status": status.as_u16(), "error": message }),
            );
            JsonValue::Null
        }
    };

    serde_json::json!({
        "generated_at": now,
        "power": power,
        "water": water,
        "soil": soil,
        "status": status,
        "errors": errors,
    })
}

pub fn router() -> Router<AppState> {
    Router::new().route("/dashboard/state", get(dashboard_state))
}
