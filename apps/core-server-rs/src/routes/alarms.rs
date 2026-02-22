use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

const CAP_ALERTS_VIEW: &str = "alerts.view";

#[derive(sqlx::FromRow)]
pub(crate) struct AlarmRow {
    id: i64,
    name: String,
    rule: SqlJson<JsonValue>,
    status: String,
    sensor_id: Option<String>,
    node_id: Option<Uuid>,
    origin: String,
    anomaly_score: Option<f64>,
    last_fired: Option<chrono::DateTime<chrono::Utc>>,
    rule_id: Option<i64>,
    target_key: Option<String>,
    resolved_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmResponse {
    id: i64,
    name: String,
    rule: JsonValue,
    sensor_id: Option<String>,
    node_id: Option<String>,
    status: String,
    origin: String,
    anomaly_score: Option<f64>,
    last_fired: Option<String>,
    rule_id: Option<i64>,
    target_key: Option<String>,
    resolved_at: Option<String>,
}

impl From<AlarmRow> for AlarmResponse {
    fn from(row: AlarmRow) -> Self {
        let status = match row.status.as_str() {
            "firing" => "active".to_string(),
            other => other.to_string(),
        };
        let last_fired = row.last_fired.map(|ts| ts.to_rfc3339());
        Self {
            id: row.id,
            name: row.name,
            rule: row.rule.0,
            sensor_id: row.sensor_id,
            node_id: row.node_id.map(|id| id.to_string()),
            status,
            origin: row.origin,
            anomaly_score: row.anomaly_score,
            last_fired,
            rule_id: row.rule_id,
            target_key: row.target_key,
            resolved_at: row.resolved_at.map(|ts| ts.to_rfc3339()),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct HistoryQuery {
    #[param(minimum = 1, maximum = 250)]
    limit: Option<u32>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct AlarmEventRow {
    id: i64,
    alarm_id: i64,
    sensor_id: Option<String>,
    node_id: Option<Uuid>,
    status: String,
    message: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    origin: String,
    anomaly_score: Option<f64>,
    rule_id: Option<i64>,
    transition: Option<String>,
    incident_id: Option<i64>,
    target_key: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmEventResponse {
    id: String,
    alarm_id: String,
    sensor_id: Option<String>,
    node_id: Option<String>,
    status: String,
    message: Option<String>,
    created_at: Option<String>,
    origin: String,
    anomaly_score: Option<f64>,
    rule_id: Option<String>,
    transition: Option<String>,
    incident_id: Option<String>,
    target_key: Option<String>,
}

impl From<AlarmEventRow> for AlarmEventResponse {
    fn from(row: AlarmEventRow) -> Self {
        Self {
            id: row.id.to_string(),
            alarm_id: row.alarm_id.to_string(),
            sensor_id: row.sensor_id,
            node_id: row.node_id.map(|id| id.to_string()),
            status: row.status,
            message: row.message,
            created_at: Some(row.created_at.to_rfc3339()),
            origin: row.origin,
            anomaly_score: row.anomaly_score,
            rule_id: row.rule_id.map(|value| value.to_string()),
            transition: row.transition,
            incident_id: row.incident_id.map(|value| value.to_string()),
            target_key: row.target_key,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct BulkAcknowledgeRequest {
    pub event_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct BulkAcknowledgeResponse {
    pub acknowledged: u64,
}

pub(crate) async fn fetch_alarms(db: &sqlx::PgPool) -> Result<Vec<AlarmResponse>, sqlx::Error> {
    let rows: Vec<AlarmRow> = sqlx::query_as(
        r#"
        SELECT id, name, rule, status, sensor_id, node_id, origin, anomaly_score, last_fired, rule_id, target_key, resolved_at
        FROM alarms
        ORDER BY id ASC
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(AlarmResponse::from).collect())
}

pub(crate) async fn fetch_alarm_events(
    db: &sqlx::PgPool,
    limit: i64,
) -> Result<Vec<AlarmEventResponse>, sqlx::Error> {
    let limit = limit.clamp(1, 250);
    let rows: Vec<AlarmEventRow> = sqlx::query_as(
        r#"
        SELECT id, alarm_id, sensor_id, node_id, status, message, created_at, origin, anomaly_score, rule_id, transition, incident_id, target_key
        FROM alarm_events
        WHERE alarm_id IS NOT NULL
        ORDER BY created_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(AlarmEventResponse::from).collect())
}

#[utoipa::path(
    get,
    path = "/api/alarms",
    tag = "alarms",
    responses(
        (status = 200, description = "Alarms", body = Vec<AlarmResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_alarms(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<AlarmResponse>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;
    Ok(Json(fetch_alarms(&state.db).await.map_err(map_db_error)?))
}

#[utoipa::path(
    get,
    path = "/api/alarms/history",
    tag = "alarms",
    params(HistoryQuery),
    responses(
        (status = 200, description = "Alarm history", body = Vec<AlarmEventResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn alarm_history(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Vec<AlarmEventResponse>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;
    let limit = query.limit.unwrap_or(100).clamp(1, 250) as i64;
    Ok(Json(
        fetch_alarm_events(&state.db, limit)
            .await
            .map_err(map_db_error)?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/alarms/events/{event_id}/ack",
    tag = "alarms",
    params(("event_id" = String, Path, description = "Alarm event id")),
    responses(
        (status = 200, description = "Acknowledged event", body = AlarmEventResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Alarm event not found")
    )
)]
pub(crate) async fn acknowledge_event(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(event_id): Path<String>,
) -> Result<Json<AlarmEventResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["alerts.ack"])
        .map_err(|err| (err.status, err.message))?;

    let event_id: i64 = event_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Alarm event not found".to_string()))?;
    let result = sqlx::query(
        "UPDATE alarm_events SET status = 'acknowledged' WHERE id = $1 AND status <> 'ok'",
    )
    .bind(event_id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Alarm event not found".to_string()));
    }

    let row: Option<AlarmEventRow> = sqlx::query_as(
        r#"
        SELECT id, alarm_id, sensor_id, node_id, status, message, created_at, origin, anomaly_score, rule_id, transition, incident_id, target_key
        FROM alarm_events
        WHERE id = $1
        "#,
    )
    .bind(event_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Alarm event not found".to_string()));
    };
    Ok(Json(AlarmEventResponse::from(row)))
}

#[utoipa::path(
    post,
    path = "/api/alarms/events/ack-bulk",
    tag = "alarms",
    request_body = BulkAcknowledgeRequest,
    responses(
        (status = 200, description = "Acknowledged events", body = BulkAcknowledgeResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn acknowledge_events_bulk(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<BulkAcknowledgeRequest>,
) -> Result<Json<BulkAcknowledgeResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["alerts.ack"])
        .map_err(|err| (err.status, err.message))?;

    if payload.event_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "event_ids cannot be empty".to_string(),
        ));
    }
    if payload.event_ids.len() > 250 {
        return Err((
            StatusCode::BAD_REQUEST,
            "event_ids cannot exceed 250 entries".to_string(),
        ));
    }

    let mut ids: Vec<i64> = Vec::with_capacity(payload.event_ids.len());
    for raw in payload.event_ids.iter() {
        let parsed: i64 = raw
            .trim()
            .parse()
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid event id".to_string()))?;
        ids.push(parsed);
    }

    let result = sqlx::query(
        r#"
        UPDATE alarm_events
        SET status = 'acknowledged'
        WHERE id = ANY($1)
          AND status <> 'acknowledged'
          AND status <> 'ok'
        "#,
    )
    .bind(&ids)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(BulkAcknowledgeResponse {
        acknowledged: result.rows_affected(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/alarms", get(list_alarms))
        .route("/alarms/history", get(alarm_history))
        .route("/alarms/events/{event_id}/ack", post(acknowledge_event))
        .route("/alarms/events/ack-bulk", post(acknowledge_events_bulk))
}
