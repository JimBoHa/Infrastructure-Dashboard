use sqlx::PgPool;
use uuid::Uuid;

/// Best-effort lookup of a node-agent bearer token for a given node.
///
/// Today this reuses the controller-issued adoption token that was used to
/// adopt the node (stored in `adoption_tokens`). The node-agent accepts this
/// token as a bearer token for its config/provisioning endpoints.
pub(crate) async fn node_agent_bearer_token(db: &PgPool, node_id: Uuid) -> Option<String> {
    sqlx::query_scalar(
        r#"
        SELECT token
        FROM adoption_tokens
        WHERE node_id = $1
          AND used_at IS NOT NULL
        ORDER BY used_at DESC NULLS LAST, created_at DESC
        LIMIT 1
        "#,
    )
    .bind(node_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}
