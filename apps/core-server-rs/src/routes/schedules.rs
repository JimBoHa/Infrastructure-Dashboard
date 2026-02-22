use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

const CAP_SCHEDULES_VIEW: &str = "schedules.view";

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ScheduleResponse {
    id: String,
    name: String,
    rrule: String,
    blocks: Vec<JsonValue>,
    conditions: Vec<JsonValue>,
    actions: Vec<JsonValue>,
    next_run: Option<String>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct ScheduleRow {
    id: i64,
    name: String,
    rrule: String,
    blocks: SqlJson<Vec<JsonValue>>,
    conditions: SqlJson<Vec<JsonValue>>,
    actions: SqlJson<Vec<JsonValue>>,
}

impl From<ScheduleRow> for ScheduleResponse {
    fn from(row: ScheduleRow) -> Self {
        Self {
            id: row.id.to_string(),
            name: row.name,
            rrule: row.rrule,
            blocks: row.blocks.0,
            conditions: row.conditions.0,
            actions: row.actions.0,
            next_run: None,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct ScheduleUpsertRequest {
    name: String,
    rrule: String,
    blocks: Vec<JsonValue>,
    conditions: Vec<JsonValue>,
    actions: Vec<JsonValue>,
}

#[utoipa::path(
    get,
    path = "/api/schedules",
    tag = "schedules",
    responses(
        (status = 200, description = "Schedules", body = Vec<ScheduleResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_schedules(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<ScheduleResponse>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(
        &user,
        &[CAP_SCHEDULES_VIEW, "schedules.write", "config.write"],
    )
    .map_err(|err| (err.status, err.message))?;

    Ok(Json(
        fetch_schedules(&state.db).await.map_err(map_db_error)?,
    ))
}

pub(crate) async fn fetch_schedules(
    db: &sqlx::PgPool,
) -> Result<Vec<ScheduleResponse>, sqlx::Error> {
    let rows: Vec<ScheduleRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            rrule,
            COALESCE(blocks, '[]'::jsonb) as blocks,
            COALESCE(conditions, '[]'::jsonb) as conditions,
            COALESCE(actions, '[]'::jsonb) as actions
        FROM schedules
        ORDER BY id ASC
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(ScheduleResponse::from).collect())
}

#[utoipa::path(
    get,
    path = "/api/schedules/{schedule_id}",
    tag = "schedules",
    params(("schedule_id" = String, Path, description = "Schedule id")),
    responses(
        (status = 200, description = "Schedule", body = ScheduleResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Schedule not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_schedule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(schedule_id): Path<String>,
) -> Result<Json<ScheduleResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(
        &user,
        &[CAP_SCHEDULES_VIEW, "schedules.write", "config.write"],
    )
    .map_err(|err| (err.status, err.message))?;

    let schedule_id: i64 = schedule_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Schedule not found".to_string()))?;

    let row: Option<ScheduleRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            rrule,
            COALESCE(blocks, '[]'::jsonb) as blocks,
            COALESCE(conditions, '[]'::jsonb) as conditions,
            COALESCE(actions, '[]'::jsonb) as actions
        FROM schedules
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(schedule_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Schedule not found".to_string()));
    };

    Ok(Json(ScheduleResponse::from(row)))
}

#[utoipa::path(
    post,
    path = "/api/schedules",
    tag = "schedules",
    request_body = ScheduleUpsertRequest,
    responses(
        (status = 201, description = "Created schedule", body = ScheduleResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn create_schedule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<ScheduleUpsertRequest>,
) -> Result<(StatusCode, Json<ScheduleResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["schedules.write"])
        .map_err(|err| (err.status, err.message))?;

    if payload.name.trim().is_empty() || payload.rrule.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing name/rrule".to_string()));
    }
    if payload.actions.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Schedule actions are required".to_string(),
        ));
    }

    let row: ScheduleRow = sqlx::query_as(
        r#"
        INSERT INTO schedules (name, rrule, blocks, conditions, actions)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING
            id,
            name,
            rrule,
            COALESCE(blocks, '[]'::jsonb) as blocks,
            COALESCE(conditions, '[]'::jsonb) as conditions,
            COALESCE(actions, '[]'::jsonb) as actions
        "#,
    )
    .bind(payload.name.trim())
    .bind(payload.rrule.trim())
    .bind(SqlJson(payload.blocks))
    .bind(SqlJson(payload.conditions))
    .bind(SqlJson(payload.actions))
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok((StatusCode::CREATED, Json(ScheduleResponse::from(row))))
}

#[utoipa::path(
    put,
    path = "/api/schedules/{schedule_id}",
    tag = "schedules",
    request_body = ScheduleUpsertRequest,
    params(("schedule_id" = String, Path, description = "Schedule id")),
    responses(
        (status = 200, description = "Updated schedule", body = ScheduleResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Schedule not found")
    )
)]
pub(crate) async fn update_schedule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(schedule_id): Path<String>,
    Json(payload): Json<ScheduleUpsertRequest>,
) -> Result<Json<ScheduleResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["schedules.write"])
        .map_err(|err| (err.status, err.message))?;

    let schedule_id: i64 = schedule_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Schedule not found".to_string()))?;

    let row: Option<ScheduleRow> = sqlx::query_as(
        r#"
        UPDATE schedules
        SET name = $2,
            rrule = $3,
            blocks = $4,
            conditions = $5,
            actions = $6
        WHERE id = $1
        RETURNING
            id,
            name,
            rrule,
            COALESCE(blocks, '[]'::jsonb) as blocks,
            COALESCE(conditions, '[]'::jsonb) as conditions,
            COALESCE(actions, '[]'::jsonb) as actions
        "#,
    )
    .bind(schedule_id)
    .bind(payload.name.trim())
    .bind(payload.rrule.trim())
    .bind(SqlJson(payload.blocks))
    .bind(SqlJson(payload.conditions))
    .bind(SqlJson(payload.actions))
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Schedule not found".to_string()));
    };

    Ok(Json(ScheduleResponse::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/schedules/{schedule_id}",
    tag = "schedules",
    params(("schedule_id" = String, Path, description = "Schedule id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Schedule not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_schedule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(schedule_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["schedules.write"])
        .map_err(|err| (err.status, err.message))?;

    let schedule_id: i64 = schedule_id
        .trim()
        .parse()
        .map_err(|_| (StatusCode::NOT_FOUND, "Schedule not found".to_string()))?;

    let result = sqlx::query("DELETE FROM schedules WHERE id = $1")
        .bind(schedule_id)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Schedule not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct CalendarQuery {
    #[param(format = "date-time")]
    start: String,
    #[param(format = "date-time")]
    end: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ScheduleCalendarEvent {
    schedule_id: String,
    name: String,
    start: String,
    end: String,
    conditions: Vec<JsonValue>,
    actions: Vec<JsonValue>,
}

#[utoipa::path(
    get,
    path = "/api/schedules/calendar",
    tag = "schedules",
    params(CalendarQuery),
    responses(
        (status = 200, description = "Schedule calendar events", body = Vec<ScheduleCalendarEvent>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn calendar(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<CalendarQuery>,
) -> Result<Json<Vec<ScheduleCalendarEvent>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(
        &user,
        &[CAP_SCHEDULES_VIEW, "schedules.write", "config.write"],
    )
    .map_err(|err| (err.status, err.message))?;

    let start = parse_datetime(&query.start, "start")?;
    let end = parse_datetime(&query.end, "end")?;
    if end < start {
        return Err((
            StatusCode::BAD_REQUEST,
            "End must be after start".to_string(),
        ));
    }

    let schedules = fetch_schedules(&state.db).await.map_err(map_db_error)?;
    if schedules.is_empty() {
        return Ok(Json(vec![]));
    }

    let mut events: Vec<ScheduleCalendarEvent> = Vec::new();
    let start_local = start.with_timezone(&Local);
    let end_local = end.with_timezone(&Local);
    let mut cursor = start_local - Duration::days(7);
    let limit = end_local + Duration::days(7);
    while cursor <= limit {
        let weekday = cursor.weekday().num_days_from_monday();
        for schedule in &schedules {
            if schedule.blocks.is_empty() {
                continue;
            }

            for block in &schedule.blocks {
                let Some(block) = block.as_object() else {
                    continue;
                };
                let day_code = block
                    .get("day")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_uppercase();
                let day_index = match day_code.as_str() {
                    "MO" => 0,
                    "TU" => 1,
                    "WE" => 2,
                    "TH" => 3,
                    "FR" => 4,
                    "SA" => 5,
                    "SU" => 6,
                    _ => continue,
                };
                if weekday != day_index {
                    continue;
                }

                let Some(start_raw) = block.get("start").and_then(|value| value.as_str()) else {
                    continue;
                };
                let Some(end_raw) = block.get("end").and_then(|value| value.as_str()) else {
                    continue;
                };
                let Some((start_hour, start_minute)) = parse_hhmm(start_raw) else {
                    continue;
                };
                let Some((end_hour, end_minute)) = parse_hhmm(end_raw) else {
                    continue;
                };

                let date = cursor.date_naive();
                let Some(start_naive) = date.and_hms_opt(start_hour, start_minute, 0) else {
                    continue;
                };
                let Some(mut end_naive) = date.and_hms_opt(end_hour, end_minute, 0) else {
                    continue;
                };
                if end_naive <= start_naive {
                    end_naive += Duration::days(1);
                }

                let resolved = crate::time::resolve_block_interval(&Local, start_naive, end_naive);
                let resolved = match resolved {
                    Ok(resolved) => resolved,
                    Err(err) => {
                        tracing::warn!(
                            schedule_id = %schedule.id,
                            start_local = %start_naive,
                            end_local = %end_naive,
                            error = %err,
                            "schedule calendar block time resolution failed"
                        );
                        continue;
                    }
                };
                if !resolved.warnings.is_empty() {
                    tracing::warn!(
                        schedule_id = %schedule.id,
                        start_local = %start_naive,
                        end_local = %end_naive,
                        warnings = ?resolved.warnings,
                        "schedule calendar block required DST resolution adjustments"
                    );
                }
                let block_start = resolved.start_utc;
                let block_end = resolved.end_utc;
                if block_end < start || block_start > end {
                    continue;
                }

                events.push(ScheduleCalendarEvent {
                    schedule_id: schedule.id.clone(),
                    name: schedule.name.clone(),
                    start: block_start.to_rfc3339(),
                    end: block_end.to_rfc3339(),
                    conditions: schedule.conditions.clone(),
                    actions: schedule.actions.clone(),
                });
            }
        }

        cursor = cursor + Duration::days(1);
    }

    events.sort_by(|a, b| a.start.cmp(&b.start));
    Ok(Json(events))
}

fn parse_datetime(value: &str, field: &'static str) -> Result<DateTime<Utc>, (StatusCode, String)> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, format!("Missing {field}")));
    }

    if let Ok(ts) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(ts.with_timezone(&Utc));
    }

    let naive =
        chrono::NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%dT%H:%M:%S").map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid {field} timestamp"),
            )
        })?;
    Ok(Utc.from_utc_datetime(&naive))
}

fn parse_hhmm(value: &str) -> Option<(u32, u32)> {
    let trimmed = value.trim();
    let (hour_raw, minute_raw) = trimmed.split_once(':')?;
    let hour: u32 = hour_raw.trim().parse().ok()?;
    let minute: u32 = minute_raw.trim().parse().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/schedules", get(list_schedules).post(create_schedule))
        .route(
            "/schedules/{schedule_id}",
            get(get_schedule)
                .put(update_schedule)
                .delete(delete_schedule),
        )
        .route("/schedules/calendar", get(calendar))
}
