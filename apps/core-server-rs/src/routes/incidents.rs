use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

const CAP_ALERTS_VIEW: &str = "alerts.view";

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct IncidentsQuery {
    status: Option<String>,
    severity: Option<String>,
    assigned_to: Option<String>,
    unassigned: Option<bool>,
    from: Option<String>,
    to: Option<String>,
    search: Option<String>,
    #[param(minimum = 1, maximum = 250)]
    limit: Option<u32>,
    cursor: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct NotesQuery {
    #[param(minimum = 1, maximum = 250)]
    limit: Option<u32>,
    cursor: Option<String>,
}

#[derive(sqlx::FromRow)]
struct IncidentRow {
    id: i64,
    rule_id: Option<i64>,
    target_key: Option<String>,
    severity: String,
    status: String,
    title: String,
    assigned_to: Option<Uuid>,
    snoozed_until: Option<DateTime<Utc>>,
    first_event_at: DateTime<Utc>,
    last_event_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    total_event_count: i64,
    active_event_count: i64,
    note_count: i64,
    last_message: Option<String>,
    last_origin: Option<String>,
    last_sensor_id: Option<String>,
    last_node_id: Option<Uuid>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct IncidentResponse {
    id: String,
    rule_id: Option<String>,
    target_key: Option<String>,
    severity: String,
    status: String,
    title: String,
    assigned_to: Option<String>,
    snoozed_until: Option<String>,
    first_event_at: String,
    last_event_at: String,
    closed_at: Option<String>,
    created_at: String,
    updated_at: String,
    total_event_count: i64,
    active_event_count: i64,
    note_count: i64,
    last_message: Option<String>,
    last_origin: Option<String>,
    last_sensor_id: Option<String>,
    last_node_id: Option<String>,
}

impl From<IncidentRow> for IncidentResponse {
    fn from(row: IncidentRow) -> Self {
        Self {
            id: row.id.to_string(),
            rule_id: row.rule_id.map(|value| value.to_string()),
            target_key: row.target_key,
            severity: row.severity,
            status: row.status,
            title: row.title,
            assigned_to: row.assigned_to.map(|value| value.to_string()),
            snoozed_until: row.snoozed_until.map(|ts| ts.to_rfc3339()),
            first_event_at: row.first_event_at.to_rfc3339(),
            last_event_at: row.last_event_at.to_rfc3339(),
            closed_at: row.closed_at.map(|ts| ts.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
            total_event_count: row.total_event_count,
            active_event_count: row.active_event_count,
            note_count: row.note_count,
            last_message: row.last_message,
            last_origin: row.last_origin,
            last_sensor_id: row.last_sensor_id,
            last_node_id: row.last_node_id.map(|value| value.to_string()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct IncidentsListResponse {
    incidents: Vec<IncidentResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct IncidentDetailResponse {
    incident: IncidentResponse,
    events: Vec<crate::routes::alarms::AlarmEventResponse>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct IncidentAssignRequest {
    user_id: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct IncidentSnoozeRequest {
    until: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct IncidentCloseRequest {
    closed: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct IncidentNoteResponse {
    id: String,
    incident_id: String,
    created_by: Option<String>,
    body: String,
    created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct IncidentNotesListResponse {
    notes: Vec<IncidentNoteResponse>,
    next_cursor: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct IncidentNoteCreateRequest {
    body: String,
}

#[derive(sqlx::FromRow)]
struct IncidentNoteRow {
    id: i64,
    incident_id: i64,
    created_by: Option<Uuid>,
    body: String,
    created_at: DateTime<Utc>,
}

impl From<IncidentNoteRow> for IncidentNoteResponse {
    fn from(row: IncidentNoteRow) -> Self {
        Self {
            id: row.id.to_string(),
            incident_id: row.incident_id.to_string(),
            created_by: row.created_by.map(|value| value.to_string()),
            body: row.body,
            created_at: row.created_at.to_rfc3339(),
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

fn parse_rfc3339_optional(raw: Option<&str>, field: &str) -> Result<Option<DateTime<Utc>>, (StatusCode, String)> {
    let Some(raw) = raw else { return Ok(None) };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(parse_rfc3339_required(trimmed, field)?))
}

fn parse_incidents_cursor(raw: &str) -> Result<(DateTime<Utc>, i64), (StatusCode, String)> {
    let trimmed = raw.trim();
    let Some((ts_raw, id_raw)) = trimmed.split_once('|') else {
        return Err((StatusCode::BAD_REQUEST, "cursor must be <rfc3339>|<id>".to_string()));
    };
    let ts = parse_rfc3339_required(ts_raw, "cursor")?;
    let id: i64 = id_raw
        .trim()
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "cursor id must be an integer".to_string()))?;
    Ok((ts, id))
}

fn parse_notes_cursor(raw: &str) -> Result<(DateTime<Utc>, i64), (StatusCode, String)> {
    let trimmed = raw.trim();
    let Some((ts_raw, id_raw)) = trimmed.split_once('|') else {
        return Err((StatusCode::BAD_REQUEST, "cursor must be <rfc3339>|<id>".to_string()));
    };
    let ts = parse_rfc3339_required(ts_raw, "cursor")?;
    let id: i64 = id_raw
        .trim()
        .parse()
        .map_err(|_| (StatusCode::BAD_REQUEST, "cursor id must be an integer".to_string()))?;
    Ok((ts, id))
}

async fn fetch_incident_row(
    db: &sqlx::PgPool,
    incident_id: i64,
) -> Result<IncidentRow, (StatusCode, String)> {
    let row: Option<IncidentRow> = sqlx::query_as(
        r#"
        SELECT
            i.id,
            i.rule_id,
            i.target_key,
            i.severity,
            i.status,
            i.title,
            i.assigned_to,
            i.snoozed_until,
            i.first_event_at,
            i.last_event_at,
            i.closed_at,
            i.created_at,
            i.updated_at,
            (SELECT COUNT(*) FROM alarm_events e WHERE e.incident_id = i.id) AS total_event_count,
            (SELECT COUNT(*) FROM alarm_events e WHERE e.incident_id = i.id AND e.status <> 'acknowledged' AND e.status <> 'ok') AS active_event_count,
            (SELECT COUNT(*) FROM incident_notes n WHERE n.incident_id = i.id) AS note_count,
            (SELECT e.message FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_message,
            (SELECT e.origin FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_origin,
            (SELECT e.sensor_id FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_sensor_id,
            (SELECT e.node_id FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_node_id
        FROM incidents i
        WHERE i.id = $1
        "#,
    )
    .bind(incident_id)
    .fetch_optional(db)
    .await
    .map_err(map_db_error)?;

    row.ok_or((StatusCode::NOT_FOUND, "Incident not found".to_string()))
}

fn normalize_status(raw: Option<String>) -> Result<Option<String>, (StatusCode, String)> {
    let Some(raw) = raw else { return Ok(None) };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let normalized = trimmed.to_lowercase();
    if crate::services::incidents::parse_incident_status(&normalized).is_none() {
        return Err((StatusCode::BAD_REQUEST, "Invalid status filter".to_string()));
    }
    Ok(Some(normalized))
}

fn normalize_severity(raw: Option<String>) -> Result<Option<String>, (StatusCode, String)> {
    let Some(raw) = raw else { return Ok(None) };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let normalized = trimmed.to_lowercase();
    if crate::services::incidents::parse_severity(&normalized).is_none() {
        return Err((StatusCode::BAD_REQUEST, "Invalid severity filter".to_string()));
    }
    Ok(Some(normalized))
}

#[utoipa::path(
    get,
    path = "/api/incidents",
    tag = "incidents",
    params(IncidentsQuery),
    responses(
        (status = 200, description = "Incidents", body = IncidentsListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_incidents(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<IncidentsQuery>,
) -> Result<Json<IncidentsListResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let status = normalize_status(query.status)?;
    let severity = normalize_severity(query.severity)?;
    let assigned_to: Option<Uuid> = query
        .assigned_to
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "assigned_to must be a UUID".to_string()))?;
    let unassigned = query.unassigned.unwrap_or(false);
    let unassigned_filter: Option<bool> = if unassigned && assigned_to.is_none() {
        Some(true)
    } else {
        None
    };

    let from_ts = parse_rfc3339_optional(query.from.as_deref(), "from")?;
    let to_ts = parse_rfc3339_optional(query.to.as_deref(), "to")?;

    let search = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let (cursor_ts, cursor_id) = if let Some(raw) = query.cursor.as_deref().filter(|value| !value.trim().is_empty()) {
        let (ts, id) = parse_incidents_cursor(raw)?;
        (Some(ts), Some(id))
    } else {
        (None, None)
    };

    let limit = query.limit.unwrap_or(100).clamp(1, 250) as i64;

    let rows: Vec<IncidentRow> = sqlx::query_as(
        r#"
        SELECT
            i.id,
            i.rule_id,
            i.target_key,
            i.severity,
            i.status,
            i.title,
            i.assigned_to,
            i.snoozed_until,
            i.first_event_at,
            i.last_event_at,
            i.closed_at,
            i.created_at,
            i.updated_at,
            (SELECT COUNT(*) FROM alarm_events e WHERE e.incident_id = i.id) AS total_event_count,
            (SELECT COUNT(*) FROM alarm_events e WHERE e.incident_id = i.id AND e.status <> 'acknowledged' AND e.status <> 'ok') AS active_event_count,
            (SELECT COUNT(*) FROM incident_notes n WHERE n.incident_id = i.id) AS note_count,
            (SELECT e.message FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_message,
            (SELECT e.origin FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_origin,
            (SELECT e.sensor_id FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_sensor_id,
            (SELECT e.node_id FROM alarm_events e WHERE e.incident_id = i.id ORDER BY e.created_at DESC, e.id DESC LIMIT 1) AS last_node_id
        FROM incidents i
        WHERE ($1::text IS NULL OR i.status = $1)
          AND ($2::text IS NULL OR i.severity = $2)
          AND ($3::uuid IS NULL OR i.assigned_to = $3)
          AND ($4::bool IS NULL OR i.assigned_to IS NULL)
          AND ($5::timestamptz IS NULL OR i.last_event_at >= $5)
          AND ($6::timestamptz IS NULL OR i.last_event_at <= $6)
          AND (
              $7::text IS NULL
              OR i.title ILIKE ('%' || $7 || '%')
              OR i.target_key ILIKE ('%' || $7 || '%')
              OR EXISTS (SELECT 1 FROM alarm_rules r WHERE r.id = i.rule_id AND r.name ILIKE ('%' || $7 || '%'))
              OR EXISTS (SELECT 1 FROM alarm_events e WHERE e.incident_id = i.id AND e.message ILIKE ('%' || $7 || '%'))
              OR EXISTS (
                    SELECT 1
                    FROM alarm_events e
                    JOIN sensors s ON s.sensor_id = e.sensor_id
                    WHERE e.incident_id = i.id
                      AND (
                        s.name ILIKE ('%' || $7 || '%')
                        OR s.sensor_id ILIKE ('%' || $7 || '%')
                        OR s.type ILIKE ('%' || $7 || '%')
                        OR s.unit ILIKE ('%' || $7 || '%')
                      )
                )
              OR EXISTS (
                    SELECT 1
                    FROM alarm_events e
                    JOIN nodes n ON n.id = e.node_id
                    WHERE e.incident_id = i.id
                      AND n.name ILIKE ('%' || $7 || '%')
                )
          )
          AND (
            $8::timestamptz IS NULL
            OR $9::bigint IS NULL
            OR (i.last_event_at, i.id) < ($8, $9)
          )
        ORDER BY i.last_event_at DESC, i.id DESC
        LIMIT $10
        "#,
    )
    .bind(status)
    .bind(severity)
    .bind(assigned_to)
    .bind(unassigned_filter)
    .bind(from_ts)
    .bind(to_ts)
    .bind(search)
    .bind(cursor_ts)
    .bind(cursor_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let incidents: Vec<IncidentResponse> = rows.into_iter().map(IncidentResponse::from).collect();
    let next_cursor = incidents.last().map(|row| format!("{}|{}", row.last_event_at, row.id));
    Ok(Json(IncidentsListResponse { incidents, next_cursor }))
}

#[utoipa::path(
    get,
    path = "/api/incidents/{incident_id}",
    tag = "incidents",
    params(("incident_id" = String, Path, description = "Incident id")),
    responses(
        (status = 200, description = "Incident detail", body = IncidentDetailResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Incident not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_incident(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(incident_id): Path<String>,
) -> Result<Json<IncidentDetailResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let incident_id: i64 = incident_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Incident not found".to_string()))?;
    let incident = fetch_incident_row(&state.db, incident_id).await?;

    let rows: Vec<crate::routes::alarms::AlarmEventRow> = sqlx::query_as(
        r#"
        SELECT id, alarm_id, sensor_id, node_id, status, message, created_at, origin, anomaly_score, rule_id, transition, incident_id, target_key
        FROM alarm_events
        WHERE incident_id = $1
          AND alarm_id IS NOT NULL
        ORDER BY created_at DESC
        LIMIT 250
        "#,
    )
    .bind(incident_id)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;
    let events = rows.into_iter().map(crate::routes::alarms::AlarmEventResponse::from).collect();

    Ok(Json(IncidentDetailResponse {
        incident: IncidentResponse::from(incident),
        events,
    }))
}

#[utoipa::path(
    post,
    path = "/api/incidents/{incident_id}/assign",
    tag = "incidents",
    params(("incident_id" = String, Path, description = "Incident id")),
    request_body = IncidentAssignRequest,
    responses(
        (status = 200, description = "Updated incident", body = IncidentResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Incident not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn assign_incident(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(incident_id): Path<String>,
    Json(payload): Json<IncidentAssignRequest>,
) -> Result<Json<IncidentResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let incident_id: i64 = incident_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Incident not found".to_string()))?;
    let next_assigned_to: Option<Uuid> = payload
        .user_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "user_id must be a UUID".to_string()))?;

    let result = sqlx::query(
        r#"
        UPDATE incidents
        SET assigned_to = $2, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(incident_id)
    .bind(next_assigned_to)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Incident not found".to_string()));
    }

    let row = fetch_incident_row(&state.db, incident_id).await?;
    Ok(Json(IncidentResponse::from(row)))
}

#[utoipa::path(
    post,
    path = "/api/incidents/{incident_id}/snooze",
    tag = "incidents",
    params(("incident_id" = String, Path, description = "Incident id")),
    request_body = IncidentSnoozeRequest,
    responses(
        (status = 200, description = "Updated incident", body = IncidentResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Incident not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn snooze_incident(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(incident_id): Path<String>,
    Json(payload): Json<IncidentSnoozeRequest>,
) -> Result<Json<IncidentResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let incident_id: i64 = incident_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Incident not found".to_string()))?;
    let now = Utc::now();
    let until = parse_rfc3339_optional(payload.until.as_deref(), "until")?;
    let (next_status, next_until) = match until {
        None => ("open".to_string(), None),
        Some(ts) if ts <= now => ("open".to_string(), None),
        Some(ts) => ("snoozed".to_string(), Some(ts)),
    };

    let result = sqlx::query(
        r#"
        UPDATE incidents
        SET status = $2, snoozed_until = $3, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(incident_id)
    .bind(next_status)
    .bind(next_until)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Incident not found".to_string()));
    }

    let row = fetch_incident_row(&state.db, incident_id).await?;
    Ok(Json(IncidentResponse::from(row)))
}

#[utoipa::path(
    post,
    path = "/api/incidents/{incident_id}/close",
    tag = "incidents",
    params(("incident_id" = String, Path, description = "Incident id")),
    request_body = IncidentCloseRequest,
    responses(
        (status = 200, description = "Updated incident", body = IncidentResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Incident not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn close_incident(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(incident_id): Path<String>,
    Json(payload): Json<IncidentCloseRequest>,
) -> Result<Json<IncidentResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let incident_id: i64 = incident_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Incident not found".to_string()))?;
    let now = Utc::now();
    let (next_status, next_closed_at, next_snoozed_until): (String, Option<DateTime<Utc>>, Option<DateTime<Utc>>) =
        if payload.closed {
            ("closed".to_string(), Some(now), None)
        } else {
            ("open".to_string(), None, None)
        };

    let result = sqlx::query(
        r#"
        UPDATE incidents
        SET status = $2, closed_at = $3, snoozed_until = $4, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(incident_id)
    .bind(next_status)
    .bind(next_closed_at)
    .bind(next_snoozed_until)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;
    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Incident not found".to_string()));
    }

    let row = fetch_incident_row(&state.db, incident_id).await?;
    Ok(Json(IncidentResponse::from(row)))
}

#[utoipa::path(
    get,
    path = "/api/incidents/{incident_id}/notes",
    tag = "incidents",
    params(("incident_id" = String, Path, description = "Incident id"), NotesQuery),
    responses(
        (status = 200, description = "Incident notes", body = IncidentNotesListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Incident not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_incident_notes(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(incident_id): Path<String>,
    Query(query): Query<NotesQuery>,
) -> Result<Json<IncidentNotesListResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let incident_id: i64 = incident_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Incident not found".to_string()))?;

    let _ = fetch_incident_row(&state.db, incident_id).await?;

    let (cursor_ts, cursor_id) = if let Some(raw) = query.cursor.as_deref().filter(|value| !value.trim().is_empty()) {
        let (ts, id) = parse_notes_cursor(raw)?;
        (Some(ts), Some(id))
    } else {
        (None, None)
    };
    let limit = query.limit.unwrap_or(100).clamp(1, 250) as i64;

    let rows: Vec<IncidentNoteRow> = sqlx::query_as(
        r#"
        SELECT id, incident_id, created_by, body, created_at
        FROM incident_notes
        WHERE incident_id = $1
          AND (
            $2::timestamptz IS NULL
            OR $3::bigint IS NULL
            OR (created_at, id) < ($2, $3)
          )
        ORDER BY created_at DESC, id DESC
        LIMIT $4
        "#,
    )
    .bind(incident_id)
    .bind(cursor_ts)
    .bind(cursor_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let notes: Vec<IncidentNoteResponse> = rows.into_iter().map(IncidentNoteResponse::from).collect();
    let next_cursor = notes.last().map(|row| format!("{}|{}", row.created_at, row.id));
    Ok(Json(IncidentNotesListResponse { notes, next_cursor }))
}

#[utoipa::path(
    post,
    path = "/api/incidents/{incident_id}/notes",
    tag = "incidents",
    params(("incident_id" = String, Path, description = "Incident id")),
    request_body = IncidentNoteCreateRequest,
    responses(
        (status = 200, description = "Created note", body = IncidentNoteResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Incident not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_incident_note(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(incident_id): Path<String>,
    Json(payload): Json<IncidentNoteCreateRequest>,
) -> Result<Json<IncidentNoteResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let incident_id: i64 = incident_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Incident not found".to_string()))?;
    let body = payload.body.trim();
    if body.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "body cannot be blank".to_string()));
    }

    let created_by = user
        .user_id()
        .ok_or((StatusCode::BAD_REQUEST, "Invalid user id".to_string()))?;

    let _ = fetch_incident_row(&state.db, incident_id).await?;

    let inserted: IncidentNoteRow = sqlx::query_as(
        r#"
        INSERT INTO incident_notes (incident_id, created_by, body)
        VALUES ($1, $2, $3)
        RETURNING id, incident_id, created_by, body, created_at
        "#,
    )
    .bind(incident_id)
    .bind(created_by)
    .bind(body)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(IncidentNoteResponse::from(inserted)))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/incidents", get(list_incidents))
        .route("/incidents/{incident_id}", get(get_incident))
        .route("/incidents/{incident_id}/assign", post(assign_incident))
        .route("/incidents/{incident_id}/snooze", post(snooze_incident))
        .route("/incidents/{incident_id}/close", post(close_incident))
        .route(
            "/incidents/{incident_id}/notes",
            get(list_incident_notes).post(create_incident_note),
        )
}
