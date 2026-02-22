use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct ApiTokenInfo {
    id: String,
    name: Option<String>,
    created_at: String,
    last_used_at: Option<String>,
    expires_at: Option<String>,
    revoked_at: Option<String>,
    capabilities: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct ApiTokenRow {
    id: Uuid,
    name: Option<String>,
    capabilities: SqlJson<Vec<String>>,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    revoked_at: Option<DateTime<Utc>>,
}

impl From<ApiTokenRow> for ApiTokenInfo {
    fn from(row: ApiTokenRow) -> Self {
        Self {
            id: row.id.to_string(),
            name: row.name,
            created_at: row.created_at.to_rfc3339(),
            last_used_at: row.last_used_at.map(|dt| dt.to_rfc3339()),
            expires_at: row.expires_at.map(|dt| dt.to_rfc3339()),
            revoked_at: row.revoked_at.map(|dt| dt.to_rfc3339()),
            capabilities: row.capabilities.0,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/api-tokens",
    tag = "auth",
    responses(
        (status = 200, description = "Known API tokens", body = [ApiTokenInfo]),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn list_api_tokens(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<ApiTokenInfo>>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let rows: Vec<ApiTokenRow> = sqlx::query_as(
        r#"
        SELECT id, name, capabilities, created_at, last_used_at, expires_at, revoked_at
        FROM api_tokens
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(rows.into_iter().map(ApiTokenInfo::from).collect()))
}

#[utoipa::path(
    delete,
    path = "/api/api-tokens/{token_id}",
    tag = "auth",
    params(("token_id" = String, Path, description = "API token id")),
    responses(
        (status = 204, description = "Token revoked"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Token not found")
    )
)]
pub(crate) async fn revoke_api_token(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(token_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let token_id = token_id.trim();
    let id = Uuid::parse_str(token_id)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid token id".to_string()))?;

    let updated = sqlx::query(
        "UPDATE api_tokens SET revoked_at = NOW() WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;
    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Token not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api-tokens", get(list_api_tokens))
        .route("/api-tokens/{token_id}", delete(revoke_api_token))
}
