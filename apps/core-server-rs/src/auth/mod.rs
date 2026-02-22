pub(crate) mod api_tokens;
mod password;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::rngs::OsRng;
use rand::RngCore;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub use password::{hash_password, verify_password};

pub fn canonicalize_role(role: &str) -> String {
    let trimmed = role.trim().to_lowercase();
    match trimmed.as_str() {
        "admin" => "admin".to_string(),
        "operator" | "control" => "operator".to_string(),
        "view" | "viewer" | "readonly" | "read-only" | "read_only" => "view".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: String,
    pub email: String,
    pub role: String,
    pub capabilities: HashSet<String>,
    pub source: String,
}

impl AuthenticatedUser {
    pub fn user_id(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.id).ok()
    }
}

#[derive(Debug)]
struct SessionEntry {
    user_id: Uuid,
    source: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct AuthManager {
    sessions: RwLock<HashMap<String, SessionEntry>>,
    ttl: ChronoDuration,
}

impl AuthManager {
    pub fn new(token_ttl_hours: i64) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            ttl: ChronoDuration::hours(token_ttl_hours),
        }
    }

    pub async fn issue_for_user(&self, user_id: Uuid, source: String) -> String {
        let mut buf = [0u8; 32];
        OsRng.fill_bytes(&mut buf);
        let token = URL_SAFE_NO_PAD.encode(buf);
        let expires_at = Utc::now() + self.ttl;
        let mut sessions = self.sessions.write().await;
        sessions.insert(
            token.clone(),
            SessionEntry {
                user_id,
                source,
                expires_at,
            },
        );
        token
    }

    pub async fn resolve(&self, token: &str) -> Option<(Uuid, String)> {
        let mut sessions = self.sessions.write().await;
        let entry = sessions.get(token)?;
        if entry.expires_at <= Utc::now() {
            sessions.remove(token);
            return None;
        }
        Some((entry.user_id, entry.source.clone()))
    }

    pub async fn prune_expired(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let now = Utc::now();
        let expired: Vec<String> = sessions
            .iter()
            .filter_map(|(token, entry)| {
                if entry.expires_at <= now {
                    Some(token.clone())
                } else {
                    None
                }
            })
            .collect();
        for token in &expired {
            sessions.remove(token);
        }
        expired.len()
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser(pub AuthenticatedUser);

impl<S> FromRequestParts<S> for AuthUser
where
    Arc<AuthManager>: FromRef<S>,
    PgPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let manager = Arc::<AuthManager>::from_ref(state);
        let db = PgPool::from_ref(state);
        let token_result: Result<String, AppError> = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| AppError::unauthorized("Missing or invalid token"));

        async move {
            let token = token_result?;
            let user = if let Some((user_id, source)) = manager.resolve(&token).await {
                resolve_user_from_db(&db, user_id, &source).await?
            } else {
                api_tokens::resolve_api_token(&db, &token)
                    .await?
                    .ok_or_else(|| AppError::unauthorized("Missing or invalid token"))?
            };
            Ok(AuthUser(user))
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptionalAuthUser(pub Option<AuthenticatedUser>);

impl<S> FromRequestParts<S> for OptionalAuthUser
where
    Arc<AuthManager>: FromRef<S>,
    PgPool: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let manager = Arc::<AuthManager>::from_ref(state);
        let db = PgPool::from_ref(state);
        let token_result: Result<Option<String>, AppError> =
            if let Some(header_value) = parts.headers.get(AUTHORIZATION) {
                header_value
                    .to_str()
                    .ok()
                    .and_then(|value| value.strip_prefix("Bearer "))
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| Some(value.to_string()))
                    .ok_or_else(|| AppError::unauthorized("Missing or invalid token"))
            } else {
                Ok(None)
            };

        async move {
            let Some(token) = token_result? else {
                return Ok(OptionalAuthUser(None));
            };
            let user = if let Some((user_id, source)) = manager.resolve(&token).await {
                resolve_user_from_db(&db, user_id, &source).await?
            } else {
                api_tokens::resolve_api_token(&db, &token)
                    .await?
                    .ok_or_else(|| AppError::unauthorized("Missing or invalid token"))?
            };
            Ok(OptionalAuthUser(Some(user)))
        }
    }
}

#[derive(sqlx::FromRow)]
struct UserAuthRow {
    id: Uuid,
    email: String,
    role: String,
    capabilities: SqlJson<Vec<String>>,
}

async fn resolve_user_from_db(
    db: &PgPool,
    user_id: Uuid,
    source: &str,
) -> AppResult<AuthenticatedUser> {
    let row: Option<UserAuthRow> = sqlx::query_as(
        r#"
        SELECT id, email, role, capabilities
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        tracing::error!(error = %err, "database error");
        AppError::internal("Internal server error")
    })?;

    let row = row.ok_or_else(|| AppError::unauthorized("Missing or invalid token"))?;

    Ok(AuthenticatedUser {
        id: row.id.to_string(),
        email: row.email,
        role: canonicalize_role(&row.role),
        capabilities: row.capabilities.0.into_iter().collect(),
        source: source.to_string(),
    })
}

pub fn require_capabilities(user: &AuthenticatedUser, required: &[&str]) -> AppResult<()> {
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|cap| !user.capabilities.contains(*cap))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(AppError::forbidden(format!(
        "Missing capabilities: {}",
        missing.join(", ")
    )))
}

pub fn require_any_capabilities(user: &AuthenticatedUser, options: &[&str]) -> AppResult<()> {
    if options.is_empty() {
        return Ok(());
    }
    for cap in options {
        if user.capabilities.contains(*cap) {
            return Ok(());
        }
    }
    Err(AppError::forbidden(format!(
        "Missing capabilities: one of {}",
        options.join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn require_capabilities_enforces_missing_caps() {
        let user = AuthenticatedUser {
            id: "user-1".to_string(),
            email: "user@example.com".to_string(),
            role: "view".to_string(),
            capabilities: HashSet::new(),
            source: "test".to_string(),
        };

        let err = require_capabilities(&user, &["analysis.run"]).unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::FORBIDDEN);
        assert!(err.message.contains("analysis.run"));
    }

    #[test]
    fn require_capabilities_allows_present_caps() {
        let mut caps = HashSet::new();
        caps.insert("analysis.run".to_string());
        let user = AuthenticatedUser {
            id: "user-1".to_string(),
            email: "user@example.com".to_string(),
            role: "operator".to_string(),
            capabilities: caps,
            source: "test".to_string(),
        };

        assert!(require_capabilities(&user, &["analysis.run"]).is_ok());
    }

    #[test]
    fn authenticated_user_user_id_parses_uuid() {
        let user = AuthenticatedUser {
            id: Uuid::new_v4().to_string(),
            email: "user@example.com".to_string(),
            role: "operator".to_string(),
            capabilities: HashSet::new(),
            source: "test".to_string(),
        };

        assert!(user.user_id().is_some());
    }

    #[test]
    fn authenticated_user_user_id_none_for_api_token() {
        let user = AuthenticatedUser {
            id: "api_token:11111111-2222-3333-4444-555555555555".to_string(),
            email: "token".to_string(),
            role: "api_token".to_string(),
            capabilities: HashSet::new(),
            source: "api_token".to_string(),
        };

        assert!(user.user_id().is_none());
    }

    #[test]
    fn require_any_capabilities_allows_any_present_cap() {
        let mut caps = HashSet::new();
        caps.insert("a".to_string());
        let user = AuthenticatedUser {
            id: "user-1".to_string(),
            email: "user@example.com".to_string(),
            role: "view".to_string(),
            capabilities: caps,
            source: "test".to_string(),
        };

        assert!(require_any_capabilities(&user, &["b", "a"]).is_ok());
    }

    #[test]
    fn require_any_capabilities_rejects_when_none_present() {
        let user = AuthenticatedUser {
            id: "user-1".to_string(),
            email: "user@example.com".to_string(),
            role: "view".to_string(),
            capabilities: HashSet::new(),
            source: "test".to_string(),
        };

        let err = require_any_capabilities(&user, &["metrics.view", "config.write"]).unwrap_err();
        assert_eq!(err.status, axum::http::StatusCode::FORBIDDEN);
        assert!(err.message.contains("metrics.view"));
    }
}
