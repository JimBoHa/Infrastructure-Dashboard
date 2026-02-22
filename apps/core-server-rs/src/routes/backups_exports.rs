use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio_util::io::ReaderStream;
use url::Url;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{internal_error, map_db_error};
use crate::state::AppState;

const POSTGRES_BIN_FALLBACK: &str = "/usr/local/farm-dashboard/native/postgres/bin";

const DB_TABLES_CONFIG: &[&str] = &[
    "users",
    "api_tokens",
    "nodes",
    "sensors",
    "outputs",
    "schedules",
    "alarms",
    "adoption_tokens",
    "setup_credentials",
    "backup_retention",
    "map_layers",
    "map_saves",
    "map_features",
    "map_settings",
    "weather_station_integrations",
];

const DB_TABLES_FULL: &[&str] = &[
    "users",
    "api_tokens",
    "nodes",
    "sensors",
    "outputs",
    "schedules",
    "action_logs",
    "alarms",
    "alarm_events",
    "alarm_rules",
    "alarm_rule_state",
    "incidents",
    "incident_notes",
    "adoption_tokens",
    "setup_credentials",
    "backup_retention",
    "map_layers",
    "map_saves",
    "map_features",
    "map_settings",
    "forecast_data",
    "forecast_points",
    "analytics_integration_status",
    "analytics_indicators",
    "weather_station_integrations",
    "metrics",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct SetupConfigSnapshot {
    path: String,
    updated_at: Option<String>,
    value: JsonValue,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct SetupCredentialSnapshot {
    name: String,
    value: String,
    metadata: JsonValue,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct BackupRetentionSnapshot {
    default_keep_days: i32,
    policies: Vec<BackupRetentionPolicySnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct BackupRetentionPolicySnapshot {
    node_id: String,
    keep_days: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapLayerSnapshot {
    system_key: Option<String>,
    name: String,
    kind: String,
    source_type: String,
    config: JsonValue,
    opacity: f64,
    enabled: bool,
    z_index: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapFeatureSnapshot {
    node_id: Option<String>,
    sensor_id: Option<String>,
    geometry: JsonValue,
    properties: JsonValue,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapSaveSnapshot {
    name: String,
    active_base_layer_system_key: Option<String>,
    active_base_layer_name: Option<String>,
    center_lat: Option<f64>,
    center_lng: Option<f64>,
    zoom: Option<f64>,
    bearing: Option<f64>,
    pitch: Option<f64>,
    features: Vec<MapFeatureSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapSettingsSnapshot {
    active_save_name: Option<String>,
    active_base_layer_system_key: Option<String>,
    active_base_layer_name: Option<String>,
    center_lat: Option<f64>,
    center_lng: Option<f64>,
    zoom: Option<f64>,
    bearing: Option<f64>,
    pitch: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct MapSnapshot {
    layers: Vec<MapLayerSnapshot>,
    saves: Vec<MapSaveSnapshot>,
    settings: Option<MapSettingsSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct AppSettingsBundle {
    schema_version: u32,
    exported_at: String,
    setup_config: SetupConfigSnapshot,
    setup_credentials: Vec<SetupCredentialSnapshot>,
    backup_retention: BackupRetentionSnapshot,
    map: MapSnapshot,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AppSettingsImportApplied {
    setup_credentials: usize,
    backup_retention_policies: usize,
    map_layers: usize,
    map_saves: usize,
    map_features: usize,
    setup_config_written: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AppSettingsImportResponse {
    status: String,
    applied: AppSettingsImportApplied,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct DatabaseExportQuery {
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Clone)]
struct PgConnection {
    host: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    password: Option<String>,
    dbname: String,
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

async fn existing_public_tables(
    db: &sqlx::PgPool,
    tables: &[&str],
) -> Result<Vec<String>, sqlx::Error> {
    let desired: Vec<String> = tables
        .iter()
        .map(|value| value.trim().to_string())
        .collect();
    sqlx::query_scalar(
        r#"
        SELECT tablename
        FROM pg_tables
        WHERE schemaname = 'public'
          AND tablename = ANY($1)
        ORDER BY tablename ASC
        "#,
    )
    .bind(&desired)
    .fetch_all(db)
    .await
}

fn find_postgres_tool(tool: &str) -> PathBuf {
    if let Some(path_var) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&path_var) {
            let candidate = path.join(tool);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    let fallback = PathBuf::from(POSTGRES_BIN_FALLBACK).join(tool);
    if fallback.exists() {
        return fallback;
    }
    PathBuf::from(tool)
}

fn parse_pg_connection(database_url: &str) -> Result<PgConnection, (StatusCode, String)> {
    let url = Url::parse(database_url).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Controller database URL is invalid".to_string(),
        )
    })?;

    let dbname = url.path().trim_start_matches('/').to_string();
    if dbname.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Controller database URL is missing a database name".to_string(),
        ));
    }

    let host = url.host_str().map(|value| value.to_string());
    let port = url.port_or_known_default().map(|value| value as u16);
    let user = {
        let username = url.username().trim();
        if username.is_empty() {
            None
        } else {
            Some(username.to_string())
        }
    };
    let password = url.password().map(|value| value.to_string());

    Ok(PgConnection {
        host,
        port,
        user,
        password,
        dbname,
    })
}

async fn setup_config_updated_at(path: &Path) -> Option<String> {
    let meta = tokio::fs::metadata(path).await.ok()?;
    let modified = meta.modified().ok()?;
    let ts: chrono::DateTime<chrono::Utc> = modified.into();
    Some(ts.to_rfc3339())
}

async fn load_json_file(path: &Path) -> Result<JsonValue, (StatusCode, String)> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => serde_json::from_str(&contents).map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to parse setup config at {}: {err}", path.display()),
            )
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(JsonValue::Object(Default::default()))
        }
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read setup config at {}: {err}", path.display()),
        )),
    }
}

async fn write_json_file_atomic(
    path: &Path,
    value: &JsonValue,
) -> Result<(), (StatusCode, String)> {
    let parent = path.parent().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Invalid setup config path".to_string(),
    ))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(internal_error)?;

    let encoded = serde_json::to_string_pretty(value).map_err(internal_error)?;
    let tmp_name = format!(
        "{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config.json"),
        Uuid::new_v4()
    );
    let tmp_path = parent.join(tmp_name);
    tokio::fs::write(&tmp_path, encoded)
        .await
        .map_err(internal_error)?;
    tokio::fs::rename(&tmp_path, path)
        .await
        .map_err(internal_error)?;
    Ok(())
}

fn response_stream_from_file(
    file: tokio::fs::File,
    filename: &str,
    content_type: &str,
) -> Result<Response, (StatusCode, String)> {
    let stream = ReaderStream::new(file);
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type).map_err(internal_error)?,
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

#[utoipa::path(
    get,
    path = "/api/backups/app-settings/export",
    tag = "backups",
    responses(
        (status = 200, description = "Controller settings bundle", content_type = "application/json", body = String),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn export_app_settings(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    #[derive(sqlx::FromRow)]
    struct SetupCredentialRow {
        name: String,
        value: String,
        metadata: SqlJson<JsonValue>,
    }

    #[derive(sqlx::FromRow)]
    struct RetentionRow {
        node_id: Uuid,
        keep_days: i32,
    }

    #[derive(sqlx::FromRow)]
    struct MapLayerRow {
        system_key: Option<String>,
        name: String,
        kind: String,
        source_type: String,
        config: SqlJson<JsonValue>,
        opacity: f64,
        enabled: bool,
        z_index: i32,
    }

    #[derive(sqlx::FromRow)]
    struct MapSaveRow {
        id: i64,
        name: String,
        active_base_layer_system_key: Option<String>,
        active_base_layer_name: Option<String>,
        center_lat: Option<f64>,
        center_lng: Option<f64>,
        zoom: Option<f64>,
        bearing: Option<f64>,
        pitch: Option<f64>,
    }

    #[derive(sqlx::FromRow)]
    struct MapFeatureRow {
        save_id: i64,
        node_id: Option<Uuid>,
        sensor_id: Option<String>,
        geometry: SqlJson<JsonValue>,
        properties: SqlJson<JsonValue>,
    }

    #[derive(sqlx::FromRow)]
    struct MapSettingsRow {
        active_save_name: Option<String>,
        active_base_layer_system_key: Option<String>,
        active_base_layer_name: Option<String>,
        center_lat: Option<f64>,
        center_lng: Option<f64>,
        zoom: Option<f64>,
        bearing: Option<f64>,
        pitch: Option<f64>,
    }

    let setup_config_path = crate::config::setup_config_path();
    let setup_config_value = load_json_file(&setup_config_path).await?;
    let setup_config = SetupConfigSnapshot {
        path: setup_config_path.display().to_string(),
        updated_at: setup_config_updated_at(&setup_config_path).await,
        value: setup_config_value,
    };

    let creds: Vec<SetupCredentialRow> = sqlx::query_as(
        r#"
        SELECT name, value, metadata
        FROM setup_credentials
        ORDER BY name ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;
    let setup_credentials = creds
        .into_iter()
        .map(|row| SetupCredentialSnapshot {
            name: row.name,
            value: row.value,
            metadata: row.metadata.0,
        })
        .collect::<Vec<_>>();

    let default_keep_days =
        fetch_default_keep_days(&state.db, state.config.backup_retention_days as i32).await;

    let retention: Vec<RetentionRow> = sqlx::query_as(
        r#"
        SELECT node_id, keep_days
        FROM backup_retention
        ORDER BY node_id
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;
    let backup_retention = BackupRetentionSnapshot {
        default_keep_days,
        policies: retention
            .into_iter()
            .map(|row| BackupRetentionPolicySnapshot {
                node_id: row.node_id.to_string(),
                keep_days: row.keep_days,
            })
            .collect(),
    };

    let layers: Vec<MapLayerRow> = sqlx::query_as(
        r#"
        SELECT system_key, name, kind, source_type, config, opacity, enabled, z_index
        FROM map_layers
        ORDER BY z_index ASC, id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;
    let layer_snapshots = layers
        .into_iter()
        .map(|row| MapLayerSnapshot {
            system_key: row.system_key,
            name: row.name,
            kind: row.kind,
            source_type: row.source_type,
            config: row.config.0,
            opacity: row.opacity,
            enabled: row.enabled,
            z_index: row.z_index,
        })
        .collect::<Vec<_>>();

    let saves: Vec<MapSaveRow> = sqlx::query_as(
        r#"
        SELECT
            s.id as id,
            s.name as name,
            l.system_key as active_base_layer_system_key,
            l.name as active_base_layer_name,
            s.center_lat as center_lat,
            s.center_lng as center_lng,
            s.zoom as zoom,
            s.bearing as bearing,
            s.pitch as pitch
        FROM map_saves s
        LEFT JOIN map_layers l ON l.id = s.active_base_layer_id
        ORDER BY s.updated_at DESC, s.id DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let features: Vec<MapFeatureRow> = sqlx::query_as(
        r#"
        SELECT save_id, node_id, sensor_id, geometry, properties
        FROM map_features
        ORDER BY save_id ASC, id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut features_by_save: BTreeMap<i64, Vec<MapFeatureSnapshot>> = BTreeMap::new();
    for feature in features {
        features_by_save
            .entry(feature.save_id)
            .or_default()
            .push(MapFeatureSnapshot {
                node_id: feature.node_id.map(|id| id.to_string()),
                sensor_id: feature.sensor_id,
                geometry: feature.geometry.0,
                properties: feature.properties.0,
            });
    }

    let save_snapshots = saves
        .into_iter()
        .map(|row| MapSaveSnapshot {
            name: row.name.clone(),
            active_base_layer_system_key: row.active_base_layer_system_key,
            active_base_layer_name: row.active_base_layer_name,
            center_lat: row.center_lat,
            center_lng: row.center_lng,
            zoom: row.zoom,
            bearing: row.bearing,
            pitch: row.pitch,
            features: features_by_save.remove(&row.id).unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    let settings: Option<MapSettingsRow> = sqlx::query_as(
        r#"
        SELECT
            s.name as active_save_name,
            l.system_key as active_base_layer_system_key,
            l.name as active_base_layer_name,
            ms.center_lat as center_lat,
            ms.center_lng as center_lng,
            ms.zoom as zoom,
            ms.bearing as bearing,
            ms.pitch as pitch
        FROM map_settings ms
        LEFT JOIN map_saves s ON s.id = ms.active_save_id
        LEFT JOIN map_layers l ON l.id = ms.active_base_layer_id
        WHERE ms.singleton = TRUE
        LIMIT 1
        "#,
    )
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let map_settings = settings.map(|row| MapSettingsSnapshot {
        active_save_name: row.active_save_name,
        active_base_layer_system_key: row.active_base_layer_system_key,
        active_base_layer_name: row.active_base_layer_name,
        center_lat: row.center_lat,
        center_lng: row.center_lng,
        zoom: row.zoom,
        bearing: row.bearing,
        pitch: row.pitch,
    });

    let bundle = AppSettingsBundle {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        setup_config,
        setup_credentials,
        backup_retention,
        map: MapSnapshot {
            layers: layer_snapshots,
            saves: save_snapshots,
            settings: map_settings,
        },
    };

    let bytes = serde_json::to_vec_pretty(&bundle).map_err(internal_error)?;
    let filename = format!(
        "farm-dashboard-settings-{}.json",
        Utc::now().format("%Y%m%dT%H%M%SZ")
    );

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

#[utoipa::path(
    post,
    path = "/api/backups/app-settings/import",
    tag = "backups",
    request_body = AppSettingsBundle,
    responses(
        (status = 200, description = "Applied controller settings bundle", body = AppSettingsImportResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn import_app_settings(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(bundle): Json<AppSettingsBundle>,
) -> Result<Json<AppSettingsImportResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if bundle.schema_version != 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Unsupported settings bundle version".to_string(),
        ));
    }

    let mut warnings: Vec<String> = vec![];

    for policy in &bundle.backup_retention.policies {
        if policy.keep_days <= 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                "backup_retention.keep_days must be > 0".to_string(),
            ));
        }
        if Uuid::parse_str(policy.node_id.trim()).is_err() {
            return Err((
                StatusCode::BAD_REQUEST,
                "backup_retention.node_id must be a UUID".to_string(),
            ));
        }
    }

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let _ = sqlx::query("DELETE FROM setup_credentials")
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    for cred in &bundle.setup_credentials {
        let name = cred.name.trim();
        if name.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Credential name is required".to_string(),
            ));
        }
        let _ = sqlx::query(
            r#"
            INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (name)
            DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
            "#,
        )
        .bind(name)
        .bind(cred.value.trim())
        .bind(SqlJson(cred.metadata.clone()))
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }

    let _ = sqlx::query("DELETE FROM backup_retention")
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    for policy in &bundle.backup_retention.policies {
        let node_id = Uuid::parse_str(policy.node_id.trim()).map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                "Invalid backup_retention node_id".to_string(),
            )
        })?;
        let _ = sqlx::query(
            r#"
            INSERT INTO backup_retention (node_id, keep_days, created_at, updated_at)
            VALUES ($1, $2, NOW(), NOW())
            ON CONFLICT (node_id)
            DO UPDATE SET keep_days = EXCLUDED.keep_days, updated_at = NOW()
            "#,
        )
        .bind(node_id)
        .bind(policy.keep_days)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }

    let _ = sqlx::query("DELETE FROM map_settings")
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    let _ = sqlx::query("DELETE FROM map_features")
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    let _ = sqlx::query("DELETE FROM map_saves")
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    let _ = sqlx::query("DELETE FROM map_layers")
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    let mut layer_id_by_system_key: HashMap<String, i64> = HashMap::new();
    let mut layer_id_by_name: HashMap<String, i64> = HashMap::new();
    for layer in &bundle.map.layers {
        let name = layer.name.trim();
        if name.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Map layer name is required".to_string(),
            ));
        }
        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO map_layers (system_key, name, kind, source_type, config, opacity, enabled, z_index, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(layer.system_key.as_deref())
        .bind(name)
        .bind(layer.kind.trim())
        .bind(layer.source_type.trim())
        .bind(SqlJson(layer.config.clone()))
        .bind(layer.opacity)
        .bind(layer.enabled)
        .bind(layer.z_index)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;

        let id = row.0;
        if let Some(system_key) = layer
            .system_key
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            layer_id_by_system_key.insert(system_key.to_string(), id);
        }
        if !layer_id_by_name.contains_key(name) {
            layer_id_by_name.insert(name.to_string(), id);
        }
    }

    let mut save_id_by_name: HashMap<String, i64> = HashMap::new();
    let mut map_features_inserted = 0usize;
    for save in &bundle.map.saves {
        let name = save.name.trim();
        if name.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Map save name is required".to_string(),
            ));
        }
        let base_layer_id = save
            .active_base_layer_system_key
            .as_deref()
            .and_then(|key| layer_id_by_system_key.get(key.trim()))
            .copied()
            .or_else(|| {
                save.active_base_layer_name
                    .as_deref()
                    .and_then(|layer_name| layer_id_by_name.get(layer_name.trim()))
                    .copied()
            });

        let row: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO map_saves (name, active_base_layer_id, center_lat, center_lng, zoom, bearing, pitch, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(base_layer_id)
        .bind(save.center_lat)
        .bind(save.center_lng)
        .bind(save.zoom)
        .bind(save.bearing)
        .bind(save.pitch)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;
        let save_id = row.0;

        if save_id_by_name.contains_key(name) {
            warnings.push(format!(
                "Duplicate map save name \"{name}\" in import bundle; only the first is used as a lookup target."
            ));
        } else {
            save_id_by_name.insert(name.to_string(), save_id);
        }

        for feature in &save.features {
            let node_id = feature
                .node_id
                .as_deref()
                .and_then(|raw| Uuid::parse_str(raw.trim()).ok());
            let _ = sqlx::query(
                r#"
                INSERT INTO map_features (save_id, node_id, sensor_id, geometry, properties, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
                "#,
            )
            .bind(save_id)
            .bind(node_id)
            .bind(feature.sensor_id.as_deref())
            .bind(SqlJson(feature.geometry.clone()))
            .bind(SqlJson(feature.properties.clone()))
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
            map_features_inserted += 1;
        }
    }

    if let Some(settings) = &bundle.map.settings {
        let active_save_id = settings
            .active_save_name
            .as_deref()
            .and_then(|name| save_id_by_name.get(name.trim()))
            .copied();
        let base_layer_id = settings
            .active_base_layer_system_key
            .as_deref()
            .and_then(|key| layer_id_by_system_key.get(key.trim()))
            .copied()
            .or_else(|| {
                settings
                    .active_base_layer_name
                    .as_deref()
                    .and_then(|name| layer_id_by_name.get(name.trim()))
                    .copied()
            });

        let _ = sqlx::query(
            r#"
            INSERT INTO map_settings (singleton, active_save_id, active_base_layer_id, center_lat, center_lng, zoom, bearing, pitch, created_at, updated_at)
            VALUES (TRUE, $1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            ON CONFLICT (singleton)
            DO UPDATE SET active_save_id = EXCLUDED.active_save_id,
                          active_base_layer_id = EXCLUDED.active_base_layer_id,
                          center_lat = EXCLUDED.center_lat,
                          center_lng = EXCLUDED.center_lng,
                          zoom = EXCLUDED.zoom,
                          bearing = EXCLUDED.bearing,
                          pitch = EXCLUDED.pitch,
                          updated_at = NOW()
            "#,
        )
        .bind(active_save_id)
        .bind(base_layer_id)
        .bind(settings.center_lat)
        .bind(settings.center_lng)
        .bind(settings.zoom)
        .bind(settings.bearing)
        .bind(settings.pitch)
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
    }

    tx.commit().await.map_err(map_db_error)?;

    let local_path = crate::config::setup_config_path();
    if bundle.setup_config.path.trim() != local_path.display().to_string() {
        warnings.push(format!(
            "Imported bundle references setup config path {}, applied to local path {} instead.",
            bundle.setup_config.path,
            local_path.display()
        ));
    }
    write_json_file_atomic(&local_path, &bundle.setup_config.value).await?;

    Ok(Json(AppSettingsImportResponse {
        status: "ok".to_string(),
        applied: AppSettingsImportApplied {
            setup_credentials: bundle.setup_credentials.len(),
            backup_retention_policies: bundle.backup_retention.policies.len(),
            map_layers: bundle.map.layers.len(),
            map_saves: bundle.map.saves.len(),
            map_features: map_features_inserted,
            setup_config_written: true,
        },
        warnings,
    }))
}

async fn run_pg_dump_to_file(
    pg_conn: &PgConnection,
    tables: Option<&[String]>,
    format_args: &[&str],
    out_path: &Path,
) -> Result<(), (StatusCode, String)> {
    let pg_dump = find_postgres_tool("pg_dump");
    let mut cmd = Command::new(pg_dump);
    cmd.args(format_args);
    cmd.arg("--no-owner");
    cmd.arg("--no-acl");
    if let Some(host) = pg_conn.host.as_deref() {
        cmd.arg("--host").arg(host);
    }
    if let Some(port) = pg_conn.port {
        cmd.arg("--port").arg(port.to_string());
    }
    if let Some(user) = pg_conn.user.as_deref() {
        cmd.arg("--username").arg(user);
    }
    cmd.arg("--dbname").arg(&pg_conn.dbname);
    if let Some(tables) = tables {
        for table in tables {
            cmd.arg("--table").arg(format!("public.{table}"));
        }
    }
    if let Some(password) = pg_conn.password.as_deref() {
        cmd.env("PGPASSWORD", password);
    }

    let output_file = std::fs::File::create(out_path).map_err(internal_error)?;
    cmd.stdout(std::process::Stdio::from(output_file));
    let status = cmd.status().await.map_err(internal_error)?;
    if !status.success() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to export database (pg_dump)".to_string(),
        ));
    }
    Ok(())
}

async fn run_psql_copy(pg_conn: &PgConnection, sql: &str) -> Result<(), (StatusCode, String)> {
    let psql = find_postgres_tool("psql");
    let mut cmd = Command::new(psql);
    cmd.arg("-X");
    cmd.arg("-v");
    cmd.arg("ON_ERROR_STOP=1");
    cmd.arg("-q");
    if let Some(host) = pg_conn.host.as_deref() {
        cmd.arg("--host").arg(host);
    }
    if let Some(port) = pg_conn.port {
        cmd.arg("--port").arg(port.to_string());
    }
    if let Some(user) = pg_conn.user.as_deref() {
        cmd.arg("--username").arg(user);
    }
    cmd.arg("--dbname").arg(&pg_conn.dbname);
    cmd.arg("-c").arg(sql);
    if let Some(password) = pg_conn.password.as_deref() {
        cmd.env("PGPASSWORD", password);
    }
    let status = cmd.status().await.map_err(internal_error)?;
    if !status.success() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to export database (psql)".to_string(),
        ));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/backups/database/export",
    tag = "backups",
    params(DatabaseExportQuery),
    responses(
        (status = 200, description = "Database export", content_type = "application/octet-stream", body = String),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn export_database(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<DatabaseExportQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let format = query
        .format
        .as_deref()
        .unwrap_or("raw")
        .trim()
        .to_lowercase();
    let scope = query
        .scope
        .as_deref()
        .unwrap_or("full")
        .trim()
        .to_lowercase();

    let pg_conn = parse_pg_connection(&state.config.database_url)?;

    let export_tables: Option<Vec<String>> = match scope.as_str() {
        "config" => Some(
            existing_public_tables(&state.db, DB_TABLES_CONFIG)
                .await
                .map_err(map_db_error)?,
        ),
        "full" => None,
        "app" => Some(
            existing_public_tables(&state.db, DB_TABLES_FULL)
                .await
                .map_err(map_db_error)?,
        ),
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "scope must be one of: full, config, app".to_string(),
            ));
        }
    };

    let now = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();

    match format.as_str() {
        "raw" => {
            let tmp_dir = tempfile::tempdir().map_err(internal_error)?;
            let filename = match scope.as_str() {
                "config" => format!("farm-dashboard-db-config-{now}.dump"),
                "app" => format!("farm-dashboard-db-app-{now}.dump"),
                _ => format!("farm-dashboard-db-full-{now}.dump"),
            };
            let out_path = tmp_dir.path().join(&filename);
            run_pg_dump_to_file(
                &pg_conn,
                export_tables.as_deref(),
                &["--format=custom"],
                &out_path,
            )
            .await?;
            let file = tokio::fs::File::open(&out_path)
                .await
                .map_err(internal_error)?;
            Ok(response_stream_from_file(
                file,
                &filename,
                "application/octet-stream",
            )?)
        }
        "sql" => {
            let tmp_dir = tempfile::tempdir().map_err(internal_error)?;
            let filename = match scope.as_str() {
                "config" => format!("farm-dashboard-db-config-{now}.sql"),
                "app" => format!("farm-dashboard-db-app-{now}.sql"),
                _ => format!("farm-dashboard-db-full-{now}.sql"),
            };
            let out_path = tmp_dir.path().join(&filename);
            run_pg_dump_to_file(
                &pg_conn,
                export_tables.as_deref(),
                &["--format=plain"],
                &out_path,
            )
            .await?;
            let file = tokio::fs::File::open(&out_path)
                .await
                .map_err(internal_error)?;
            Ok(response_stream_from_file(
                file,
                &filename,
                "application/sql",
            )?)
        }
        "csv" | "json" => {
            if scope == "full" {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "csv/json export requires scope=config or scope=app (full DB exports are available via raw/sql)".to_string(),
                ));
            }

            let tables: Vec<String> = export_tables.unwrap_or_default();
            if tables.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "No matching tables were found for this export scope".to_string(),
                ));
            }

            let tmp_dir = tempfile::tempdir().map_err(internal_error)?;
            let export_dir = tmp_dir.path().join("export");
            tokio::fs::create_dir_all(&export_dir)
                .await
                .map_err(internal_error)?;

            let manifest = serde_json::json!({
                "schema_version": 1,
                "exported_at": Utc::now().to_rfc3339(),
                "scope": scope,
                "format": format,
                "tables": tables,
            });
            tokio::fs::write(
                export_dir.join("manifest.json"),
                serde_json::to_vec_pretty(&manifest).map_err(internal_error)?,
            )
            .await
            .map_err(internal_error)?;

            for table in &tables {
                let safe_table = table.trim();
                if safe_table.is_empty() {
                    continue;
                }
                let out_name = if format == "csv" {
                    format!("{safe_table}.csv")
                } else {
                    format!("{safe_table}.jsonl")
                };
                let out_path = export_dir.join(out_name);
                let out_path_str = out_path.to_string_lossy().replace('\'', "''");

                let table_ref = format!("public.\"{}\"", safe_table.replace('"', "\"\""));
                let copy_sql = if format == "csv" {
                    format!("\\copy {table_ref} TO '{out_path_str}' WITH (FORMAT csv, HEADER true)")
                } else {
                    format!(
                        "\\copy (SELECT row_to_json(t) FROM (SELECT * FROM {table_ref}) t) TO '{out_path_str}'"
                    )
                };
                run_psql_copy(&pg_conn, &copy_sql).await?;
            }

            let archive_name = match format.as_str() {
                "csv" => format!("farm-dashboard-db-{scope}-{now}.csv.tar.gz"),
                _ => format!("farm-dashboard-db-{scope}-{now}.json.tar.gz"),
            };
            let archive_path = tmp_dir.path().join(&archive_name);
            let tar_status = Command::new("tar")
                .arg("-czf")
                .arg(&archive_path)
                .arg("-C")
                .arg(&export_dir)
                .arg(".")
                .status()
                .await
                .map_err(internal_error)?;
            if !tar_status.success() {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to archive database export".to_string(),
                ));
            }

            let file = tokio::fs::File::open(&archive_path)
                .await
                .map_err(internal_error)?;
            Ok(response_stream_from_file(
                file,
                &archive_name,
                "application/gzip",
            )?)
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            "format must be one of: raw, sql, csv, json".to_string(),
        )),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/backups/app-settings/export", get(export_app_settings))
        .route("/backups/app-settings/import", post(import_app_settings))
        .route("/backups/database/export", get(export_database))
}
