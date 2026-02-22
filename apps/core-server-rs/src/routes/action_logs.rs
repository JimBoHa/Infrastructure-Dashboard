use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

const CAP_ALERTS_VIEW: &str = "alerts.view";

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct ActionLogsQuery {
    /// Start timestamp (RFC3339).
    from: String,
    /// End timestamp (RFC3339).
    to: String,
    /// Optional node id (UUID) to scope output actions.
    node_id: Option<String>,
    /// Optional schedule id.
    schedule_id: Option<String>,
    #[param(minimum = 1, maximum = 250)]
    limit: Option<u32>,
}

#[derive(sqlx::FromRow)]
struct ActionLogRow {
    id: i64,
    schedule_id: i64,
    action: sqlx::types::Json<JsonValue>,
    status: String,
    message: Option<String>,
    created_at: DateTime<Utc>,
    output_id: Option<String>,
    output_node_id: Option<Uuid>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ActionLogResponse {
    id: String,
    schedule_id: String,
    action: JsonValue,
    status: String,
    message: Option<String>,
    created_at: String,
    output_id: Option<String>,
    node_id: Option<String>,
}

impl From<ActionLogRow> for ActionLogResponse {
    fn from(row: ActionLogRow) -> Self {
        Self {
            id: row.id.to_string(),
            schedule_id: row.schedule_id.to_string(),
            action: row.action.0,
            status: row.status,
            message: row.message,
            created_at: row.created_at.to_rfc3339(),
            output_id: row.output_id,
            node_id: row.output_node_id.map(|value| value.to_string()),
        }
    }
}

fn parse_rfc3339_required(raw: &str, field: &str) -> Result<DateTime<Utc>, (StatusCode, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, format!("{field} cannot be blank")));
    }
    let parsed = DateTime::parse_from_rfc3339(trimmed)
        .map_err(|_| (StatusCode::BAD_REQUEST, format!("{field} must be RFC3339")))?;
    Ok(parsed.with_timezone(&Utc))
}

#[utoipa::path(
    get,
    path = "/api/action-logs",
    tag = "action_logs",
    params(ActionLogsQuery),
    responses(
        (status = 200, description = "Action logs", body = Vec<ActionLogResponse>),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_action_logs(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<ActionLogsQuery>,
) -> Result<Json<Vec<ActionLogResponse>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let from_ts = parse_rfc3339_required(&query.from, "from")?;
    let to_ts = parse_rfc3339_required(&query.to, "to")?;
    if to_ts < from_ts {
        return Err((StatusCode::BAD_REQUEST, "to must be after from".to_string()));
    }

    let node_id: Option<Uuid> = query
        .node_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "node_id must be a UUID".to_string()))?;
    let schedule_id: Option<i64> = query
        .schedule_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|raw| raw.parse::<i64>())
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "schedule_id must be an integer".to_string()))?;

    let limit = query.limit.unwrap_or(100).clamp(1, 250) as i64;

    let rows: Vec<ActionLogRow> = sqlx::query_as(
        r#"
        SELECT
            l.id,
            l.schedule_id,
            l.action,
            l.status,
            l.message,
            l.created_at,
            (l.action->>'output_id') AS output_id,
            o.node_id AS output_node_id
        FROM action_logs l
        LEFT JOIN outputs o ON o.id = (l.action->>'output_id')
        WHERE l.created_at >= $1
          AND l.created_at <= $2
          AND ($3::uuid IS NULL OR o.node_id = $3)
          AND ($4::bigint IS NULL OR l.schedule_id = $4)
        ORDER BY l.created_at DESC, l.id DESC
        LIMIT $5
        "#,
    )
    .bind(from_ts)
    .bind(to_ts)
    .bind(node_id)
    .bind(schedule_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(rows.into_iter().map(ActionLogResponse::from).collect()))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new().route("/action-logs", get(list_action_logs))
}

