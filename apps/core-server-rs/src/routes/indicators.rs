use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsIndicatorRead {
    id: i64,
    key: String,
    value: f64,
    context: JsonValue,
    recorded_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct IndicatorQuery {
    key: Option<String>,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    250
}

#[utoipa::path(
    get,
    path = "/api/indicators",
    tag = "indicators",
    params(IndicatorQuery),
    responses((status = 200, description = "Latest indicators", body = Vec<AnalyticsIndicatorRead>))
)]
pub(crate) async fn latest_indicators(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(query): Query<IndicatorQuery>,
) -> Result<Json<Vec<AnalyticsIndicatorRead>>, (StatusCode, String)> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        key: String,
        value: f64,
        context: SqlJson<JsonValue>,
        recorded_at: DateTime<Utc>,
    }

    if let Some(key) = query
        .key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
    {
        let row: Option<Row> = sqlx::query_as(
            r#"
            SELECT id, key, value, COALESCE(context, '{}'::jsonb) as context, recorded_at
            FROM analytics_indicators
            WHERE key = $1
            ORDER BY recorded_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .map_err(map_db_error)?;
        let Some(row) = row else {
            return Ok(Json(vec![]));
        };
        return Ok(Json(vec![AnalyticsIndicatorRead {
            id: row.id,
            key: row.key,
            value: row.value,
            context: row.context.0,
            recorded_at: row.recorded_at.to_rfc3339(),
        }]));
    }

    let limit = query.limit.clamp(1, 1000);
    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (key)
            id,
            key,
            value,
            COALESCE(context, '{}'::jsonb) as context,
            recorded_at
        FROM analytics_indicators
        ORDER BY key, recorded_at DESC, id DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|row| AnalyticsIndicatorRead {
                id: row.id,
                key: row.key,
                value: row.value,
                context: row.context.0,
                recorded_at: row.recorded_at.to_rfc3339(),
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/indicators/keys",
    tag = "indicators",
    responses((status = 200, description = "Indicator keys", body = Vec<String>))
)]
pub(crate) async fn indicator_keys(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT key
        FROM analytics_indicators
        ORDER BY key ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(rows.into_iter().map(|(key,)| key).collect()))
}

#[utoipa::path(
    get,
    path = "/api/indicators/status",
    tag = "indicators",
    responses((status = 200, description = "Indicator status", body = JsonValue))
)]
pub(crate) async fn indicator_status(
    axum::extract::State(_state): axum::extract::State<AppState>,
) -> Json<JsonValue> {
    Json(serde_json::json!({
        "enabled": false,
        "last_status": "never",
        "last_polled_at": null,
        "last_error": null,
        "meta": {}
    }))
}

#[utoipa::path(
    post,
    path = "/api/indicators/recompute",
    tag = "indicators",
    responses((status = 200, description = "Indicator status", body = JsonValue)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn recompute_indicators(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<JsonValue>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let total_nodes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM nodes")
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?;
    let online_nodes: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM nodes WHERE status = 'online'")
            .fetch_one(&state.db)
            .await
            .map_err(map_db_error)?;
    let offline_nodes: i64 = (total_nodes - online_nodes).max(0);
    let active_alarms: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM alarms WHERE status = 'firing'")
            .fetch_one(&state.db)
            .await
            .map_err(map_db_error)?;

    let now = Utc::now();
    let mut tx = state.db.begin().await.map_err(map_db_error)?;
    for (key, value, context) in [
        (
            "nodes.online",
            online_nodes as f64,
            serde_json::json!({ "total_nodes": total_nodes }),
        ),
        (
            "nodes.offline",
            offline_nodes as f64,
            serde_json::json!({ "total_nodes": total_nodes }),
        ),
        ("alarms.active", active_alarms as f64, serde_json::json!({})),
    ] {
        let _ = sqlx::query(
            r#"
            INSERT INTO analytics_indicators (key, value, context, recorded_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(key)
        .bind(value)
        .bind(SqlJson(context))
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }
    tx.commit().await.map_err(map_db_error)?;

    Ok(Json(serde_json::json!({
        "enabled": true,
        "last_status": "ok",
        "last_polled_at": now.to_rfc3339(),
        "last_error": null,
        "meta": {
            "ingested": 3,
            "keys": ["nodes.online", "nodes.offline", "alarms.active"]
        }
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/indicators", get(latest_indicators))
        .route("/indicators/keys", get(indicator_keys))
        .route("/indicators/status", get(indicator_status))
        .route("/indicators/recompute", post(recompute_indicators))
}
