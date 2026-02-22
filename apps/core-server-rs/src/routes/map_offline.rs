use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::services::map_offline;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct OfflineMapPackResponse {
    id: String,
    name: String,
    bounds: JsonValue,
    min_zoom: i32,
    max_zoom: i32,
    status: String,
    progress: JsonValue,
    error: Option<String>,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct OfflineMapPackRow {
    id: String,
    name: String,
    bounds: SqlJson<JsonValue>,
    min_zoom: i32,
    max_zoom: i32,
    status: String,
    progress: SqlJson<JsonValue>,
    error: Option<String>,
    updated_at: DateTime<Utc>,
}

impl From<OfflineMapPackRow> for OfflineMapPackResponse {
    fn from(row: OfflineMapPackRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            bounds: row.bounds.0,
            min_zoom: row.min_zoom,
            max_zoom: row.max_zoom,
            status: row.status,
            progress: row.progress.0,
            error: row.error,
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

async fn get_pack(db: &PgPool, id: &str) -> Result<Option<OfflineMapPackRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT id, name, bounds, min_zoom, max_zoom, status, progress, error, updated_at
        FROM map_offline_packs
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(db)
    .await
}

#[utoipa::path(
    get,
    path = "/api/map/offline/packs",
    tag = "map",
    responses((status = 200, description = "Offline map packs", body = Vec<OfflineMapPackResponse>))
)]
pub(crate) async fn list_offline_packs(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<OfflineMapPackResponse>>, (StatusCode, String)> {
    let rows: Vec<OfflineMapPackRow> = sqlx::query_as(
        r#"
        SELECT id, name, bounds, min_zoom, max_zoom, status, progress, error, updated_at
        FROM map_offline_packs
        ORDER BY id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(
        rows.into_iter().map(OfflineMapPackResponse::from).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/map/offline/packs/{id}",
    tag = "map",
    params(("id" = String, Path, description = "Pack id")),
    responses(
        (status = 200, description = "Offline map pack", body = OfflineMapPackResponse),
        (status = 404, description = "Pack not found")
    )
)]
pub(crate) async fn get_offline_pack(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<OfflineMapPackResponse>, (StatusCode, String)> {
    let row = get_pack(&state.db, id.trim())
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Pack not found".to_string()))?;
    Ok(Json(row.into()))
}

#[utoipa::path(
    post,
    path = "/api/map/offline/packs/{id}/install",
    tag = "map",
    params(("id" = String, Path, description = "Pack id")),
    responses(
        (status = 202, description = "Install started", body = OfflineMapPackResponse),
        (status = 200, description = "Already installed", body = OfflineMapPackResponse),
        (status = 404, description = "Pack not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn install_offline_pack(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<OfflineMapPackResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let id = id.trim();
    let row = get_pack(&state.db, id)
        .await
        .map_err(map_db_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Pack not found".to_string()))?;

    if row.status == "installed" {
        if let Err(err) = map_offline::prefer_offline_baselayers(&state.db).await {
            tracing::warn!(pack_id = %row.id, error = %err, "failed to switch map saves to offline baselayers");
        }
        return Ok((StatusCode::OK, Json(row.into())));
    }

    if row.status == "installing" {
        map_offline::spawn_install(state.clone(), id.to_string());
        return Ok((StatusCode::ACCEPTED, Json(row.into())));
    }

    let updated: OfflineMapPackRow = sqlx::query_as(
        r#"
        UPDATE map_offline_packs
        SET status = 'installing',
            progress = '{"layers":{}}'::jsonb,
            error = NULL,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, bounds, min_zoom, max_zoom, status, progress, error, updated_at
        "#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    map_offline::spawn_install(state.clone(), id.to_string());

    Ok((StatusCode::ACCEPTED, Json(updated.into())))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/map/offline/packs", get(list_offline_packs))
        .route("/map/offline/packs/{id}", get(get_offline_pack))
        .route(
            "/map/offline/packs/{id}/install",
            post(install_offline_pack),
        )
}
