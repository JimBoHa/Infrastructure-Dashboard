use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::backup_bundle::{BackupOutputSnapshot, NodeBackupBundle, NODE_BACKUP_SCHEMA_VERSION};
use crate::error::{internal_error, map_db_error};
use crate::routes::node_sensors::{NodeAds1263SettingsDraft, NodeSensorDraft};
use crate::state::AppState;

const CAP_BACKUPS_VIEW: &str = "backups.view";

#[derive(sqlx::FromRow)]
struct NodeRow {
    id: Uuid,
    name: String,
    config: SqlJson<JsonValue>,
}

async fn fetch_default_keep_days(db: &sqlx::PgPool, fallback: i32) -> i32 {
    let stored: Option<String> = sqlx::query_scalar(
        r#"
        SELECT value
        FROM setup_credentials
        WHERE name = 'backup_retention_days'
        LIMIT 1
        "#,
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    stored
        .as_deref()
        .and_then(|value| value.trim().parse::<i32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct BackupFileInfo {
    pub(crate) date: String,
    pub(crate) path: String,
    pub(crate) size_bytes: Option<u64>,
    pub(crate) created_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct BackupNodeMetadata {
    pub(crate) mesh_role: Option<String>,
    pub(crate) pending_buffer_messages: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct BackupSummary {
    pub(crate) node_id: String,
    pub(crate) node_name: String,
    pub(crate) retention_days: i32,
    metadata: Option<BackupNodeMetadata>,
    pub(crate) backups: Vec<BackupFileInfo>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct RestoreRequest {
    backup_node_id: String,
    date: String,
    target_node_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct BackupRunResponse {
    status: String,
    reason: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct RetentionPolicyUpdate {
    node_id: String,
    keep_days: Option<i32>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct RetentionUpdateRequest {
    default_keep_days: Option<i32>,
    policies: Vec<RetentionPolicyUpdate>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RetentionPolicyResponse {
    node_id: String,
    node_name: Option<String>,
    keep_days: Option<i32>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RetentionConfigResponse {
    default_keep_days: i32,
    policies: Vec<RetentionPolicyResponse>,
    last_cleanup_at: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/backups/run",
    tag = "backups",
    responses(
        (status = 200, description = "Triggered backups", body = BackupRunResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn run_backups(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<BackupRunResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let nodes: Vec<NodeRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE NOT (COALESCE(config, '{}'::jsonb) @> '{"deleted": true}')
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let total_nodes = nodes.len();
    if nodes.is_empty() {
        return Ok(Json(BackupRunResponse {
            status: "ok".to_string(),
            reason: Some("No nodes available for backup.".to_string()),
        }));
    }

    let captured_at = Utc::now();
    let date = captured_at.format("%Y-%m-%d").to_string();
    let default_keep_days =
        fetch_default_keep_days(&state.db, state.config.backup_retention_days as i32).await;

    let mut failures: Vec<String> = Vec::new();
    for node in nodes {
        if let Err(err) =
            backup_one_node(&state, &node, &date, captured_at, default_keep_days).await
        {
            tracing::warn!(node_id = %node.id, error = %err, "backup failed");
            failures.push(format!("{}: {}", node.id, err));
        }
    }

    Ok(Json(BackupRunResponse {
        status: if failures.is_empty() {
            "ok".to_string()
        } else {
            "partial".to_string()
        },
        reason: if failures.is_empty() {
            None
        } else {
            Some(format!(
                "Some backups failed ({} of {}). Check logs for details.",
                failures.len(),
                total_nodes
            ))
        },
    }))
}

#[derive(sqlx::FromRow)]
struct OutputSnapshotRow {
    id: String,
    name: String,
    output_type: String,
    supported_states: SqlJson<Vec<String>>,
    config: SqlJson<JsonValue>,
}

async fn backup_one_node(
    state: &AppState,
    node: &NodeRow,
    date: &str,
    captured_at: chrono::DateTime<chrono::Utc>,
    default_keep_days: i32,
) -> anyhow::Result<()> {
    let node_id = node.id;
    let node_name = node.name.trim();
    let config = &node.config.0;

    let desired_sensors: Vec<NodeSensorDraft> = config
        .get("desired_sensors")
        .and_then(|value| serde_json::from_value::<Vec<NodeSensorDraft>>(value.clone()).ok())
        .unwrap_or_else(|| Vec::new());

    let desired_sensors = if desired_sensors.is_empty() {
        crate::routes::node_sensors::fetch_core_node_agent_sensors(&state.db, node_id).await?
    } else {
        desired_sensors
    };

    let desired_ads1263: Option<NodeAds1263SettingsDraft> = config
        .get("desired_ads1263")
        .and_then(|value| serde_json::from_value::<NodeAds1263SettingsDraft>(value.clone()).ok());

    let outputs: Vec<OutputSnapshotRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            type as output_type,
            COALESCE(supported_states, '[]'::jsonb) as supported_states,
            COALESCE(config, '{}'::jsonb) as config
        FROM outputs
        WHERE node_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(node_id)
    .fetch_all(&state.db)
    .await?;

    let output_snapshots: Vec<BackupOutputSnapshot> = outputs
        .into_iter()
        .map(|row| BackupOutputSnapshot {
            id: row.id,
            name: row.name,
            output_type: row.output_type,
            supported_states: row.supported_states.0,
            config: row.config.0,
        })
        .collect();

    let bundle = NodeBackupBundle {
        schema_version: NODE_BACKUP_SCHEMA_VERSION,
        captured_at: captured_at.to_rfc3339(),
        node_id: node_id.to_string(),
        node_name: node_name.to_string(),
        desired_sensors,
        desired_ads1263,
        outputs: output_snapshots,
    };

    let bytes = serde_json::to_vec_pretty(&bundle)?;
    let dir = state.config.backup_storage_path.join(node_id.to_string());
    tokio::fs::create_dir_all(&dir).await?;
    let path = backup_path(&state.config.backup_storage_path, node_id, date);
    let tmp = dir.join(format!(
        "{date}.json.tmp.{}",
        uuid::Uuid::new_v4().to_string()
    ));
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, &path).await?;

    let keep_days: i32 = sqlx::query_scalar(
        r#"
        SELECT keep_days
        FROM backup_retention
        WHERE node_id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(default_keep_days);

    cleanup_node_backups(
        &state.config.backup_storage_path,
        node_id,
        keep_days,
        captured_at,
    )
    .await?;

    Ok(())
}

async fn cleanup_node_backups(
    root: &FsPath,
    node_id: Uuid,
    keep_days: i32,
    now: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<()> {
    let keep_days = keep_days.max(1);
    let cutoff = now.date_naive() - chrono::Duration::days((keep_days - 1) as i64);
    let node_dir = root.join(node_id.to_string());
    if !node_dir.exists() {
        return Ok(());
    }

    let mut entries = tokio::fs::read_dir(&node_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        if !file_type.is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(file_date) = chrono::NaiveDate::parse_from_str(stem, "%Y-%m-%d") else {
            continue;
        };
        if file_date < cutoff {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/backups",
    tag = "backups",
    responses(
        (status = 200, description = "Backups", body = Vec<BackupSummary>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_backups(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<BackupSummary>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_BACKUPS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let root = state.config.backup_storage_path.clone();
    let mut summaries = scan_backup_root(&root).map_err(internal_error)?;
    if summaries.is_empty() {
        return Ok(Json(vec![]));
    }

    #[derive(sqlx::FromRow)]
    struct NodeContextRow {
        node_id: Uuid,
        node_name: String,
        keep_days: Option<i32>,
        config: SqlJson<JsonValue>,
    }

    let node_ids: Vec<Uuid> = summaries
        .iter()
        .filter_map(|summary| Uuid::parse_str(summary.node_id.as_str()).ok())
        .collect();

    let rows: Vec<NodeContextRow> = sqlx::query_as(
        r#"
        SELECT
            n.id as node_id,
            n.name as node_name,
            r.keep_days as keep_days,
            COALESCE(n.config, '{}'::jsonb) as config
        FROM nodes n
        LEFT JOIN backup_retention r ON r.node_id = n.id
        WHERE n.id = ANY($1)
        ORDER BY n.id
        "#,
    )
    .bind(&node_ids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let default_keep_days =
        fetch_default_keep_days(&state.db, state.config.backup_retention_days as i32).await;
    let mut context: BTreeMap<String, (String, i32, Option<BackupNodeMetadata>)> = BTreeMap::new();
    for row in rows {
        let node_id = row.node_id.to_string();
        let keep_days = row.keep_days.unwrap_or(default_keep_days);
        let metadata = derive_backup_metadata(&row.config.0);
        context.insert(node_id, (row.node_name, keep_days, metadata));
    }

    for summary in summaries.iter_mut() {
        if let Some((name, keep_days, metadata)) = context.get(summary.node_id.as_str()) {
            summary.node_name = name.clone();
            summary.retention_days = *keep_days;
            summary.metadata = metadata.clone();
        } else {
            summary.node_name = "Unknown".to_string();
            summary.retention_days = default_keep_days;
            summary.metadata = None;
        }
    }

    Ok(Json(summaries))
}

#[utoipa::path(
    get,
    path = "/api/backups/{node_id}",
    tag = "backups",
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "Backups", body = BackupSummary),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "No backups found for node")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_backups_for_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
) -> Result<Json<BackupSummary>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_BACKUPS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim()).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            "No backups found for node".to_string(),
        )
    })?;

    let root = state.config.backup_storage_path.clone();
    let summaries = scan_backup_root(&root).map_err(internal_error)?;
    let mut summary = summaries
        .into_iter()
        .find(|summary| summary.node_id == node_uuid.to_string())
        .ok_or((
            StatusCode::NOT_FOUND,
            "No backups found for node".to_string(),
        ))?;

    #[derive(sqlx::FromRow)]
    struct NodeContextRow {
        node_name: String,
        keep_days: Option<i32>,
        config: SqlJson<JsonValue>,
    }

    let default_keep_days =
        fetch_default_keep_days(&state.db, state.config.backup_retention_days as i32).await;
    let row: Option<NodeContextRow> = sqlx::query_as(
        r#"
        SELECT
            n.name as node_name,
            r.keep_days as keep_days,
            COALESCE(n.config, '{}'::jsonb) as config
        FROM nodes n
        LEFT JOIN backup_retention r ON r.node_id = n.id
        WHERE n.id = $1
        LIMIT 1
        "#,
    )
    .bind(node_uuid)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    if let Some(row) = row {
        summary.node_name = row.node_name;
        summary.retention_days = row.keep_days.unwrap_or(default_keep_days);
        summary.metadata = derive_backup_metadata(&row.config.0);
    } else {
        summary.node_name = "Unknown".to_string();
        summary.retention_days = default_keep_days;
    }

    Ok(Json(summary))
}

#[utoipa::path(
    get,
    path = "/api/backups/{node_id}/{date}/download",
    tag = "backups",
    params(
        ("node_id" = String, Path, description = "Node id"),
        ("date" = String, Path, description = "Backup date (YYYY-MM-DD)")
    ),
    responses(
        (status = 200, description = "Backup JSON file", content_type = "application/json", body = String),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Backup not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn download_backup(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path((node_id, date)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Invalid node id".to_string()))?;
    validate_date(&date)?;

    let path = backup_path(&state.config.backup_storage_path, node_uuid, &date);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "Backup not found".to_string()))?;
    let filename = format!("{}-{}.json", node_uuid, date);

    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let content_disposition = HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"",
        filename.replace('"', "_")
    ))
    .map_err(internal_error)?;
    response
        .headers_mut()
        .insert(header::CONTENT_DISPOSITION, content_disposition);
    Ok(response)
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct RestoreResponse {
    status: String,
}

#[utoipa::path(
    post,
    path = "/api/restore",
    tag = "backups",
    request_body = RestoreRequest,
    responses(
        (status = 200, description = "Restore queued", body = RestoreResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Backup not found")
    )
)]
pub(crate) async fn restore_backup(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<RestoreRequest>,
) -> Result<Json<RestoreResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let backup_node_id = Uuid::parse_str(payload.backup_node_id.trim()).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid backup_node_id".to_string(),
        )
    })?;
    let target_node_id = payload
        .target_node_id
        .as_deref()
        .unwrap_or(payload.backup_node_id.as_str());
    let target_node_id = Uuid::parse_str(target_node_id.trim()).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid target_node_id".to_string(),
        )
    })?;
    validate_date(payload.date.trim())?;

    let path = backup_path(
        &state.config.backup_storage_path,
        backup_node_id,
        payload.date.trim(),
    );
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, "Backup not found".to_string()));
    }

    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM nodes WHERE id = $1")
        .bind(target_node_id)
        .fetch_optional(&state.db)
        .await
        .map_err(map_db_error)?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, "Target node not found".to_string()));
    }

    let backup_date = NaiveDate::parse_from_str(payload.date.trim(), "%Y-%m-%d").map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Invalid backup date; expected YYYY-MM-DD".to_string(),
        )
    })?;

    let actor_user_id = Uuid::parse_str(user.id.trim()).ok();
    let actor_email = user.email.trim();

    let _ = sqlx::query(
        r#"
        INSERT INTO restore_events (
            id,
            backup_node_id,
            backup_date,
            target_node_id,
            status,
            message,
            actor_user_id,
            actor_email,
            created_at,
            updated_at
        )
        VALUES ($1, $2, $3, $4, 'queued', NULL, $5, $6, NOW(), NOW())
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(backup_node_id)
    .bind(backup_date)
    .bind(target_node_id)
    .bind(actor_user_id)
    .bind(if actor_email.is_empty() {
        None
    } else {
        Some(actor_email)
    })
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(RestoreResponse {
        status: "queued".to_string(),
    }))
}

#[derive(sqlx::FromRow)]
struct RetentionRow {
    node_id: Uuid,
    keep_days: i32,
    node_name: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/backups/retention",
    tag = "backups",
    responses(
        (status = 200, description = "Retention config", body = RetentionConfigResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_retention(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<RetentionConfigResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_BACKUPS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let default_keep_days =
        fetch_default_keep_days(&state.db, state.config.backup_retention_days as i32).await;
    Ok(Json(
        fetch_retention(&state.db, default_keep_days)
            .await
            .map_err(map_db_error)?,
    ))
}

pub(crate) async fn fetch_retention(
    db: &sqlx::PgPool,
    default_keep_days: i32,
) -> Result<RetentionConfigResponse, sqlx::Error> {
    let rows: Vec<RetentionRow> = sqlx::query_as(
        r#"
        SELECT
            n.id as node_id,
            COALESCE(r.keep_days, $1) as keep_days,
            n.name as node_name
        FROM nodes n
        LEFT JOIN backup_retention r ON r.node_id = n.id
        ORDER BY n.id
        "#,
    )
    .bind(default_keep_days)
    .fetch_all(db)
    .await?;

    Ok(RetentionConfigResponse {
        default_keep_days,
        policies: rows
            .into_iter()
            .map(|row| RetentionPolicyResponse {
                node_id: row.node_id.to_string(),
                node_name: row.node_name,
                keep_days: Some(row.keep_days),
            })
            .collect(),
        last_cleanup_at: None,
    })
}

#[utoipa::path(
    put,
    path = "/api/backups/retention",
    tag = "backups",
    request_body = RetentionUpdateRequest,
    responses(
        (status = 200, description = "Updated retention config", body = RetentionConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn update_retention(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<RetentionUpdateRequest>,
) -> Result<Json<RetentionConfigResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if let Some(default_keep_days) = payload.default_keep_days {
        if default_keep_days <= 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "default_keep_days must be > 0".to_string(),
            ));
        }
        let _ = sqlx::query(
            r#"
            INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
            VALUES ('backup_retention_days', $1, '{}'::jsonb, NOW(), NOW())
            ON CONFLICT (name)
            DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
            "#,
        )
        .bind(default_keep_days.to_string())
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;
    }

    for policy in payload.policies {
        let node_id = Uuid::parse_str(policy.node_id.trim())
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid node_id".to_string()))?;
        if let Some(keep_days) = policy.keep_days {
            if keep_days <= 0 {
                return Err((StatusCode::BAD_REQUEST, "keep_days must be > 0".to_string()));
            }
            let _ = sqlx::query(
                r#"
                INSERT INTO backup_retention (node_id, keep_days, created_at, updated_at)
                VALUES ($1, $2, NOW(), NOW())
                ON CONFLICT (node_id)
                DO UPDATE SET keep_days = EXCLUDED.keep_days, updated_at = NOW()
                "#,
            )
            .bind(node_id)
            .bind(keep_days)
            .execute(&state.db)
            .await
            .map_err(map_db_error)?;
        } else {
            let _ = sqlx::query("DELETE FROM backup_retention WHERE node_id = $1")
                .bind(node_id)
                .execute(&state.db)
                .await
                .map_err(map_db_error)?;
        }
    }

    get_retention(axum::extract::State(state), AuthUser(user)).await
}

#[utoipa::path(
    get,
    path = "/api/restores/recent",
    tag = "backups",
    responses(
        (status = 200, description = "Recent restore operations", body = Vec<JsonValue>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn recent_restores(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<JsonValue>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_BACKUPS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let rows: Vec<(
        Uuid,
        Uuid,
        NaiveDate,
        Uuid,
        String,
        Option<String>,
        i32,
        chrono::DateTime<Utc>,
        chrono::DateTime<Utc>,
    )> = sqlx::query_as(
        r#"
            SELECT
                id,
                backup_node_id,
                backup_date,
                target_node_id,
                status,
                message,
                attempt_count,
                created_at,
                updated_at
            FROM restore_events
            ORDER BY created_at DESC
            LIMIT 25
            "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let history: Vec<JsonValue> = rows
        .into_iter()
        .map(
            |(
                id,
                backup_node_id,
                backup_date,
                target_node_id,
                status,
                message,
                attempt_count,
                created_at,
                updated_at,
            )| {
                serde_json::json!({
                    "id": id.to_string(),
                    "backup_node_id": backup_node_id.to_string(),
                    "target_node_id": target_node_id.to_string(),
                    "date": backup_date.format("%Y-%m-%d").to_string(),
                    "status": status,
                    "message": message,
                    "attempt_count": attempt_count,
                    "created_at": created_at.to_rfc3339(),
                    "recorded_at": updated_at.to_rfc3339(),
                })
            },
        )
        .collect();

    Ok(Json(history))
}

fn backup_path(root: &FsPath, node_id: Uuid, date: &str) -> PathBuf {
    root.join(node_id.to_string()).join(format!("{date}.json"))
}

fn validate_date(date: &str) -> Result<(), (StatusCode, String)> {
    let date = date.trim();
    if date.len() != 10 {
        return Err((StatusCode::BAD_REQUEST, "Invalid backup date".to_string()));
    }
    if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid backup date; expected YYYY-MM-DD".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn scan_backup_root(root: &FsPath) -> anyhow::Result<Vec<BackupSummary>> {
    let mut summaries: BTreeMap<String, Vec<BackupFileInfo>> = BTreeMap::new();
    if !root.exists() {
        return Ok(vec![]);
    }
    for node_entry in std::fs::read_dir(root)? {
        let node_entry = node_entry?;
        if !node_entry.file_type()?.is_dir() {
            continue;
        }
        let node_id = node_entry.file_name().to_string_lossy().to_string();
        let node_path = node_entry.path();
        let mut backups: Vec<BackupFileInfo> = vec![];
        for file_entry in std::fs::read_dir(node_path)? {
            let file_entry = file_entry?;
            if !file_entry.file_type()?.is_file() {
                continue;
            }
            let path = file_entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let file_name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("");
            if chrono::NaiveDate::parse_from_str(file_name, "%Y-%m-%d").is_err() {
                continue;
            }
            let metadata = file_entry.metadata().ok();
            let size_bytes = metadata.as_ref().map(|m| m.len());
            let created_at = metadata
                .and_then(|m| m.modified().ok())
                .and_then(|mtime| DateTime::<Utc>::from(mtime).to_rfc3339().into());

            backups.push(BackupFileInfo {
                date: file_name.to_string(),
                path: format!("{node_id}/{file_name}.json"),
                size_bytes,
                created_at,
            });
        }
        backups.sort_by(|a, b| b.date.cmp(&a.date));
        if !backups.is_empty() {
            summaries.insert(node_id, backups);
        }
    }

    Ok(summaries
        .into_iter()
        .map(|(node_id, backups)| BackupSummary {
            node_id,
            node_name: "Unknown".to_string(),
            retention_days: 0,
            metadata: None,
            backups,
        })
        .collect())
}

fn derive_backup_metadata(config: &JsonValue) -> Option<BackupNodeMetadata> {
    let mesh_role = config.get("mesh_role").and_then(|value| value.as_str());
    let buffer = config.get("buffer").or_else(|| config.get("buffers"));
    let pending_buffer_messages = buffer
        .and_then(|value| value.as_object())
        .and_then(|map| map.get("pending_messages").or_else(|| map.get("pending")))
        .and_then(|value| value.as_i64())
        .map(|value| value);

    if mesh_role.is_none() && pending_buffer_messages.is_none() {
        return None;
    }

    Some(BackupNodeMetadata {
        mesh_role: mesh_role.map(|value| value.to_string()),
        pending_buffer_messages,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/backups", get(list_backups))
        .route("/backups/{node_id}", get(list_backups_for_node))
        .route("/backups/run", post(run_backups))
        .route(
            "/backups/retention",
            get(get_retention).put(update_retention),
        )
        .route("/backups/{node_id}/{date}/download", get(download_backup))
        .route("/restore", post(restore_backup))
        .route("/restores/recent", get(recent_restores))
}
