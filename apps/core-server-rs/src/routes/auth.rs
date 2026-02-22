use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct LoginResponse {
    token: String,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AuthMeResponse {
    id: String,
    email: String,
    role: String,
    source: String,
    capabilities: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AuthBootstrapResponse {
    has_users: bool,
}

#[derive(sqlx::FromRow)]
struct AuthUserRow {
    id: Uuid,
    password_hash: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Auth token", body = LoginResponse),
        (status = 400, description = "Missing email/password"),
        (status = 401, description = "Invalid credentials")
    )
)]
pub(crate) async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let email = payload.email.trim().to_lowercase();
    if email.is_empty() || payload.password.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Email and password are required".to_string(),
        ));
    }

    let row: Option<AuthUserRow> = sqlx::query_as(
        r#"
        SELECT id, password_hash
        FROM users
        WHERE email = $1
        LIMIT 1
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
    };
    let Some(hash) = row.password_hash.as_deref() else {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
    };
    if !crate::auth::verify_password(&payload.password, hash) {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".to_string()));
    }

    let _ = sqlx::query("UPDATE users SET last_login = NOW() WHERE id = $1")
        .bind(row.id)
        .execute(&state.db)
        .await;

    let token = state.auth.issue_for_user(row.id, "db".to_string()).await;
    Ok(Json(LoginResponse { token }))
}

#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "auth",
    responses((status = 200, description = "Current user", body = AuthMeResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn me(AuthUser(user): AuthUser) -> Json<AuthMeResponse> {
    let mut capabilities: Vec<String> = user.capabilities.into_iter().collect();
    capabilities.sort();
    Json(AuthMeResponse {
        id: user.id,
        email: user.email,
        role: user.role,
        source: user.source,
        capabilities,
    })
}

#[utoipa::path(
    get,
    path = "/api/auth/bootstrap",
    operation_id = "auth_bootstrap",
    tag = "auth",
    responses((status = 200, description = "Bootstrap status", body = AuthBootstrapResponse))
)]
pub(crate) async fn bootstrap(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<AuthBootstrapResponse>, (StatusCode, String)> {
    let users_exist: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users)")
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?;
    Ok(Json(AuthBootstrapResponse {
        has_users: users_exist,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/me", get(me))
        .route("/auth/bootstrap", get(bootstrap))
}
