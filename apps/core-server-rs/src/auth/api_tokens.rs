use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::auth::AuthenticatedUser;
use crate::error::{internal_error, AppError, AppResult};

#[derive(sqlx::FromRow)]
struct ApiTokenRow {
    id: Uuid,
    name: Option<String>,
    capabilities: SqlJson<Vec<String>>,
}

pub(crate) fn api_token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub(crate) async fn resolve_api_token(
    db: &PgPool,
    token: &str,
) -> AppResult<Option<AuthenticatedUser>> {
    let token = token.trim();
    if token.is_empty() {
        return Ok(None);
    }
    let token_hash = api_token_hash(token);

    let row: Option<ApiTokenRow> = sqlx::query_as(
        r#"
        SELECT id, name, capabilities
        FROM api_tokens
        WHERE token_hash = $1
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > NOW())
        LIMIT 1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(db)
    .await
    .map_err(|err| {
        let (status, message) = internal_error(err);
        AppError::new(status, message)
    })?;

    let Some(row) = row else {
        return Ok(None);
    };

    let _ = sqlx::query("UPDATE api_tokens SET last_used_at = $2 WHERE id = $1")
        .bind(row.id)
        .bind(Utc::now())
        .execute(db)
        .await;

    let display = row
        .name
        .clone()
        .unwrap_or_else(|| format!("api_token_{}", row.id));
    let capabilities: HashSet<String> = row.capabilities.0.into_iter().collect();
    Ok(Some(AuthenticatedUser {
        id: format!("api_token:{}", row.id),
        email: display,
        role: "api_token".to_string(),
        capabilities,
        source: "api_token".to_string(),
    }))
}
