use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct CreateAnnotationRequest {
    pub chart_state: JsonValue,
    pub sensor_ids: Option<Vec<String>>,
    pub time_start: Option<String>,
    pub time_end: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct UpdateAnnotationRequest {
    pub chart_state: Option<JsonValue>,
    pub sensor_ids: Option<Vec<String>>,
    pub time_start: Option<String>,
    pub time_end: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnnotationResponse {
    pub id: String,
    pub chart_state: JsonValue,
    pub sensor_ids: Option<Vec<String>>,
    pub time_start: Option<String>,
    pub time_end: Option<String>,
    pub label: Option<String>,
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Row mapping
// ---------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct AnnotationRow {
    id: Uuid,
    chart_state: SqlJson<JsonValue>,
    sensor_ids: Option<Vec<String>>,
    time_start: Option<DateTime<Utc>>,
    time_end: Option<DateTime<Utc>>,
    label: Option<String>,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<AnnotationRow> for AnnotationResponse {
    fn from(row: AnnotationRow) -> Self {
        Self {
            id: row.id.to_string(),
            chart_state: row.chart_state.0,
            sensor_ids: row.sensor_ids,
            time_start: row.time_start.map(|t| t.to_rfc3339()),
            time_end: row.time_end.map(|t| t.to_rfc3339()),
            label: row.label,
            created_by: row.created_by.map(|u| u.to_string()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/api/chart-annotations",
    tag = "chart-annotations",
    responses((status = 200, description = "List chart annotations", body = Vec<AnnotationResponse>))
)]
pub(crate) async fn list_annotations(
    State(state): State<AppState>,
) -> Result<Json<Vec<AnnotationResponse>>, (StatusCode, String)> {
    let rows: Vec<AnnotationRow> = sqlx::query_as(
        r#"
        SELECT id, chart_state, sensor_ids, time_start, time_end, label,
               created_by, created_at, updated_at
        FROM chart_annotations
        ORDER BY created_at DESC
        LIMIT 500
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(
        rows.into_iter().map(AnnotationResponse::from).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/chart-annotations",
    tag = "chart-annotations",
    request_body = CreateAnnotationRequest,
    responses(
        (status = 200, description = "Annotation created", body = AnnotationResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_annotation(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<CreateAnnotationRequest>,
) -> Result<Json<AnnotationResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let time_start = payload
        .time_start
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<DateTime<Utc>>()
                .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid time_start".to_string()))
        })
        .transpose()?;
    let time_end = payload
        .time_end
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<DateTime<Utc>>()
                .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid time_end".to_string()))
        })
        .transpose()?;

    let row: AnnotationRow = sqlx::query_as(
        r#"
        INSERT INTO chart_annotations (chart_state, sensor_ids, time_start, time_end, label, created_by)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, chart_state, sensor_ids, time_start, time_end, label,
                  created_by, created_at, updated_at
        "#,
    )
    .bind(SqlJson(&payload.chart_state))
    .bind(&payload.sensor_ids)
    .bind(time_start)
    .bind(time_end)
    .bind(&payload.label)
    .bind(user.user_id())
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(AnnotationResponse::from(row)))
}

#[utoipa::path(
    put,
    path = "/api/chart-annotations/{id}",
    tag = "chart-annotations",
    request_body = UpdateAnnotationRequest,
    params(("id" = String, Path, description = "Annotation id")),
    responses(
        (status = 200, description = "Annotation updated", body = AnnotationResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_annotation(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateAnnotationRequest>,
) -> Result<Json<AnnotationResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let id = Uuid::parse_str(id.trim())
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid ID format".to_string()))?;

    let time_start = payload
        .time_start
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<DateTime<Utc>>()
                .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid time_start".to_string()))
        })
        .transpose()?;
    let time_end = payload
        .time_end
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<DateTime<Utc>>()
                .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid time_end".to_string()))
        })
        .transpose()?;

    let row: AnnotationRow = sqlx::query_as(
        r#"
        UPDATE chart_annotations SET
            chart_state = COALESCE($2, chart_state),
            sensor_ids = COALESCE($3, sensor_ids),
            time_start = COALESCE($4, time_start),
            time_end = COALESCE($5, time_end),
            label = COALESCE($6, label),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, chart_state, sensor_ids, time_start, time_end, label,
                  created_by, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(payload.chart_state.as_ref().map(SqlJson))
    .bind(&payload.sensor_ids)
    .bind(time_start)
    .bind(time_end)
    .bind(&payload.label)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?
    .ok_or((StatusCode::NOT_FOUND, "Annotation not found".to_string()))?;

    Ok(Json(AnnotationResponse::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/chart-annotations/{id}",
    tag = "chart-annotations",
    params(("id" = String, Path, description = "Annotation id")),
    responses(
        (status = 204, description = "Annotation deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_annotation(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let id = Uuid::parse_str(id.trim())
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid ID format".to_string()))?;

    let result = sqlx::query("DELETE FROM chart_annotations WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Annotation not found".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chart-annotations", get(list_annotations))
        .route("/chart-annotations", post(create_annotation))
        .route("/chart-annotations/{id}", put(update_annotation))
        .route("/chart-annotations/{id}", delete(delete_annotation))
}
