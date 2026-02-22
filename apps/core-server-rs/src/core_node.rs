use serde_json::json;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;

use uuid::Uuid;

pub const CORE_NODE_ID: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000001");

pub fn is_core_node_id(id: Uuid) -> bool {
    id == CORE_NODE_ID
}

pub async fn ensure_core_node(db: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO nodes (id, name, status, last_seen, config, created_at)
        VALUES ($1, 'Core', 'online', NOW(), $2, NOW())
        ON CONFLICT (id) DO NOTHING
        "#,
    )
    .bind(CORE_NODE_ID)
    .bind(SqlJson(json!({ "kind": "core", "system": true })))
    .execute(db)
    .await?;
    Ok(())
}
