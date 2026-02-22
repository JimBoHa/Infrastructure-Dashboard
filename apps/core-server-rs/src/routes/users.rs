use axum::extract::{ConnectInfo, Path};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::net::SocketAddr;
use uuid::Uuid;

use crate::auth::{AuthUser, OptionalAuthUser};
use crate::error::{internal_error, map_db_conflict, map_db_error};
use crate::state::AppState;

fn normalize_capabilities(capabilities: Vec<String>) -> Vec<String> {
    let mut out = capabilities
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}

fn ensure_admin_defaults(role: &str, capabilities: &mut Vec<String>) {
    if !role.trim().eq_ignore_ascii_case("admin") {
        return;
    }
    if !capabilities.iter().any(|cap| cap == "nodes.view") {
        capabilities.push("nodes.view".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "sensors.view") {
        capabilities.push("sensors.view".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "outputs.view") {
        capabilities.push("outputs.view".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "schedules.view") {
        capabilities.push("schedules.view".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "metrics.view") {
        capabilities.push("metrics.view".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "backups.view") {
        capabilities.push("backups.view".to_string());
    }
    if !capabilities
        .iter()
        .any(|cap| cap == "setup.credentials.view")
    {
        capabilities.push("setup.credentials.view".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "config.write") {
        capabilities.push("config.write".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "users.manage") {
        capabilities.push("users.manage".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "analysis.view") {
        capabilities.push("analysis.view".to_string());
    }
    if !capabilities.iter().any(|cap| cap == "analysis.run") {
        capabilities.push("analysis.run".to_string());
    }
}

fn truthy_env(value: Option<&str>) -> bool {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_some_and(|value| {
            value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
        })
}

fn bootstrap_user_create_allowed(peer_ip: std::net::IpAddr) -> bool {
    let allow = truthy_env(
        std::env::var("CORE_ALLOW_BOOTSTRAP_USER_CREATE")
            .ok()
            .as_deref(),
    );
    allow && peer_ip.is_loopback()
}

fn default_capabilities_for_role(role: &str) -> Vec<String> {
    match role.trim().to_lowercase().as_str() {
        "admin" => vec![
            "nodes.view",
            "sensors.view",
            "outputs.view",
            "schedules.view",
            "metrics.view",
            "backups.view",
            "setup.credentials.view",
            "config.write",
            "users.manage",
            "schedules.write",
            "outputs.command",
            "alerts.view",
            "alerts.ack",
            "analytics.view",
            "analysis.view",
            "analysis.run",
        ]
        .into_iter()
        .map(|cap| cap.to_string())
        .collect(),
        "operator" => vec![
            "nodes.view",
            "sensors.view",
            "outputs.view",
            "schedules.view",
            "metrics.view",
            "schedules.write",
            "outputs.command",
            "alerts.view",
            "alerts.ack",
            "analytics.view",
        ]
        .into_iter()
        .map(|cap| cap.to_string())
        .collect(),
        "view" => vec![
            "nodes.view",
            "sensors.view",
            "outputs.view",
            "schedules.view",
            "metrics.view",
            "alerts.view",
            "analytics.view",
        ]
        .into_iter()
        .map(|cap| cap.to_string())
        .collect(),
        _ => vec![],
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct UserCreateRequest {
    name: String,
    email: String,
    role: String,
    capabilities: Vec<String>,
    password: String,
}

#[derive(Debug, Clone, serde::Deserialize, Default, utoipa::ToSchema)]
pub(crate) struct UserUpdateRequest {
    name: Option<String>,
    email: Option<String>,
    role: Option<String>,
    capabilities: Option<Vec<String>>,
    password: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct UserResponse {
    id: String,
    name: String,
    email: String,
    role: String,
    capabilities: Vec<String>,
    last_login: Option<String>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct UserRow {
    id: Uuid,
    name: String,
    email: String,
    role: String,
    capabilities: SqlJson<Vec<String>>,
    last_login: Option<chrono::DateTime<chrono::Utc>>,
}

fn user_row_to_response(row: UserRow) -> UserResponse {
    UserResponse {
        id: row.id.to_string(),
        name: row.name,
        email: row.email,
        role: crate::auth::canonicalize_role(&row.role),
        capabilities: row.capabilities.0,
        last_login: row.last_login.map(|ts| ts.to_rfc3339()),
    }
}

pub(crate) async fn fetch_users(db: &PgPool) -> Result<Vec<UserResponse>, sqlx::Error> {
    let rows: Vec<UserRow> = sqlx::query_as(
        r#"
        SELECT id, name, email, role, capabilities, last_login
        FROM users
        ORDER BY name ASC
        "#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(user_row_to_response).collect())
}

#[utoipa::path(
    get,
    path = "/api/users",
    tag = "users",
    responses(
        (status = 200, description = "Users", body = Vec<UserResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_users(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<UserResponse>>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["users.manage"])
        .map_err(|err| (err.status, err.message))?;
    Ok(Json(fetch_users(&state.db).await.map_err(map_db_error)?))
}

#[utoipa::path(
    post,
    path = "/api/users",
    tag = "users",
    request_body = UserCreateRequest,
    responses(
        (status = 201, description = "Created user", body = UserResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Email already in use")
    )
)]
pub(crate) async fn create_user(
    axum::extract::State(state): axum::extract::State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    OptionalAuthUser(maybe_user): OptionalAuthUser,
    Json(payload): Json<UserCreateRequest>,
) -> Result<(StatusCode, Json<UserResponse>), (StatusCode, String)> {
    let users_exist: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users)")
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?;

    if users_exist {
        let user = maybe_user.ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing or invalid token".to_string(),
        ))?;
        crate::auth::require_capabilities(&user, &["users.manage"])
            .map_err(|err| (err.status, err.message))?;
    } else {
        // Fresh install bootstrap protection:
        // Never allow unauthenticated user creation from the LAN just because the DB is empty.
        //
        // If operators need a bootstrap path (dev/test), it must be explicitly enabled and must
        // originate from localhost.
        if !bootstrap_user_create_allowed(peer.ip()) {
            return Err((
                StatusCode::FORBIDDEN,
                "Bootstrap user creation is disabled. Complete setup via the installer/Setup Center.".to_string(),
            ));
        }
    }

    if payload.password.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Password is required".to_string()));
    }
    let password_hash = crate::auth::hash_password(&payload.password).map_err(internal_error)?;

    let role = crate::auth::canonicalize_role(&payload.role);
    let mut capabilities = normalize_capabilities(payload.capabilities);
    if capabilities.is_empty() {
        capabilities = default_capabilities_for_role(&role);
    }
    ensure_admin_defaults(&role, &mut capabilities);

    let row: UserRow = sqlx::query_as(
        r#"
        INSERT INTO users (name, email, role, capabilities, password_hash)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, name, email, role, capabilities, last_login
        "#,
    )
    .bind(payload.name.trim())
    .bind(payload.email.trim().to_lowercase())
    .bind(&role)
    .bind(SqlJson(capabilities))
    .bind(password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|err| map_db_conflict(err, "Email already in use"))?;

    Ok((StatusCode::CREATED, Json(user_row_to_response(row))))
}

#[utoipa::path(
    put,
    path = "/api/users/{user_id}",
    tag = "users",
    request_body = UserUpdateRequest,
    params(("user_id" = String, Path, description = "User id")),
    responses(
        (status = 200, description = "Updated user", body = UserResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
        (status = 409, description = "Email already in use")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_user(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(user_id): Path<String>,
    Json(payload): Json<UserUpdateRequest>,
) -> Result<Json<UserResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["users.manage"])
        .map_err(|err| (err.status, err.message))?;

    let user_uuid = Uuid::parse_str(user_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    let existing: Option<UserRow> = sqlx::query_as(
        r#"
        SELECT id, name, email, role, capabilities, last_login
        FROM users
        WHERE id = $1
        "#,
    )
    .bind(user_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(existing) = existing else {
        return Err((StatusCode::NOT_FOUND, "User not found".to_string()));
    };

    let mut name = existing.name;
    let mut email = existing.email;
    let mut role = existing.role;
    let mut capabilities = existing.capabilities.0;
    let mut password_hash: Option<String> = None;

    if let Some(updated) = payload.name {
        if !updated.trim().is_empty() {
            name = updated.trim().to_string();
        }
    }
    if let Some(updated) = payload.email {
        if !updated.trim().is_empty() {
            email = updated.trim().to_lowercase();
        }
    }
    if let Some(updated) = payload.role {
        if !updated.trim().is_empty() {
            role = crate::auth::canonicalize_role(updated.trim());
        }
    }
    if let Some(updated) = payload.capabilities {
        capabilities = normalize_capabilities(updated);
    } else {
        capabilities = normalize_capabilities(capabilities);
    }
    if let Some(updated) = payload.password {
        if updated.trim().is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Password cannot be blank".to_string(),
            ));
        }
        password_hash = Some(crate::auth::hash_password(&updated).map_err(internal_error)?);
    }

    ensure_admin_defaults(&role, &mut capabilities);

    let row: UserRow = sqlx::query_as(
        r#"
        UPDATE users
        SET name = $2,
            email = $3,
            role = $4,
            capabilities = $5,
            password_hash = COALESCE($6, password_hash),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, email, role, capabilities, last_login
        "#,
    )
    .bind(user_uuid)
    .bind(&name)
    .bind(&email)
    .bind(&role)
    .bind(SqlJson(capabilities))
    .bind(password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|err| map_db_conflict(err, "Email already in use"))?;

    Ok(Json(user_row_to_response(row)))
}

#[utoipa::path(
    delete,
    path = "/api/users/{user_id}",
    tag = "users",
    params(("user_id" = String, Path, description = "User id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_user(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(user_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["users.manage"])
        .map_err(|err| (err.status, err.message))?;

    let user_uuid = Uuid::parse_str(user_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_uuid)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "User not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users).post(create_user))
        .route("/users/{user_id}", put(update_user).delete(delete_user))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_defaults_include_config_write_and_users_manage() {
        let mut capabilities = normalize_capabilities(vec![
            " users.manage ".to_string(),
            "users.manage".to_string(),
            "".to_string(),
        ]);
        ensure_admin_defaults("admin", &mut capabilities);
        assert!(capabilities.contains(&"config.write".to_string()));
        assert!(capabilities.contains(&"users.manage".to_string()));
    }

    #[test]
    fn role_defaults_are_non_empty_for_known_roles() {
        assert!(!default_capabilities_for_role("admin").is_empty());
        assert!(!default_capabilities_for_role("operator").is_empty());
        assert!(!default_capabilities_for_role("view").is_empty());
    }

    #[test]
    fn bootstrap_user_create_allowed_requires_loopback_and_flag() {
        std::env::remove_var("CORE_ALLOW_BOOTSTRAP_USER_CREATE");
        assert!(!bootstrap_user_create_allowed("127.0.0.1".parse().unwrap()));

        std::env::set_var("CORE_ALLOW_BOOTSTRAP_USER_CREATE", "1");
        assert!(bootstrap_user_create_allowed("127.0.0.1".parse().unwrap()));
        assert!(!bootstrap_user_create_allowed(
            "192.168.1.10".parse().unwrap()
        ));

        std::env::remove_var("CORE_ALLOW_BOOTSTRAP_USER_CREATE");
    }
}
