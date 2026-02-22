use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use std::collections::HashMap;
use tracing::warn;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::services::emporia::{EmporiaChannelReading, EmporiaDeviceInfo, EmporiaService};
use crate::services::emporia_preferences::{
    merge_emporia_circuit_preferences, merge_emporia_device_preferences,
    parse_emporia_device_preferences, EmporiaCircuitPreferences, EmporiaDevicePreferences,
    EMPORIA_CIRCUIT_KEY_MAINS,
};
use crate::state::AppState;

const CAP_SETUP_CREDENTIALS_VIEW: &str = "setup.credentials.view";

const SENSITIVE_METADATA_KEYS: &[&str] = &[
    "access_token",
    "api_key",
    "api_token",
    "password",
    "refresh_token",
    "secret",
];

fn redact_credential_metadata(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(mut map) => {
            for (key, entry) in map.iter_mut() {
                if SENSITIVE_METADATA_KEYS
                    .iter()
                    .any(|needle| needle.eq_ignore_ascii_case(key.trim()))
                {
                    *entry = JsonValue::String("<redacted>".to_string());
                    continue;
                }
                let nested = std::mem::take(entry);
                *entry = redact_credential_metadata(nested);
            }
            JsonValue::Object(map)
        }
        JsonValue::Array(items) => {
            JsonValue::Array(items.into_iter().map(redact_credential_metadata).collect())
        }
        other => other,
    }
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct MapillaryTokenResponse {
    configured: bool,
    access_token: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct SetupCredential {
    name: String,
    has_value: bool,
    metadata: JsonValue,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct SetupCredentialsResponse {
    credentials: Vec<SetupCredential>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct UpsertCredentialRequest {
    value: String,
    metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct EmporiaLoginRequest {
    username: String,
    password: String,
    site_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct EmporiaLoginResponse {
    token_present: bool,
    site_ids: Vec<String>,
    devices: Vec<EmporiaDeviceInfo>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct EmporiaCircuitSettings {
    circuit_key: String,
    name: String,
    raw_channel_num: Option<String>,
    nested_device_gid: Option<String>,
    enabled: bool,
    hidden: bool,
    include_in_power_summary: bool,
    is_mains: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct EmporiaDeviceSettings {
    device_gid: String,
    name: Option<String>,
    model: Option<String>,
    firmware: Option<String>,
    address: Option<String>,
    enabled: bool,
    hidden: bool,
    include_in_power_summary: bool,
    group_label: Option<String>,
    circuits: Vec<EmporiaCircuitSettings>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct EmporiaDevicesResponse {
    token_present: bool,
    site_ids: Vec<String>,
    devices: Vec<EmporiaDeviceSettings>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct EmporiaCircuitUpdate {
    circuit_key: String,
    enabled: Option<bool>,
    hidden: Option<bool>,
    include_in_power_summary: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct EmporiaDeviceUpdate {
    device_gid: String,
    enabled: Option<bool>,
    hidden: Option<bool>,
    include_in_power_summary: Option<bool>,
    group_label: Option<String>,
    circuits: Option<Vec<EmporiaCircuitUpdate>>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct EmporiaDevicesUpdateRequest {
    devices: Vec<EmporiaDeviceUpdate>,
}

#[derive(sqlx::FromRow)]
struct SetupCredentialRow {
    name: String,
    value: String,
    metadata: SqlJson<JsonValue>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<SetupCredentialRow> for SetupCredential {
    fn from(row: SetupCredentialRow) -> Self {
        Self {
            name: row.name,
            has_value: !row.value.trim().is_empty(),
            metadata: row.metadata.0,
            created_at: Some(row.created_at.to_rfc3339()),
            updated_at: Some(row.updated_at.to_rfc3339()),
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/setup/credentials",
    tag = "setup",
    responses(
        (status = 200, description = "Setup credentials", body = SetupCredentialsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_credentials(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<SetupCredentialsResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_SETUP_CREDENTIALS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let rows: Vec<SetupCredentialRow> = sqlx::query_as(
        r#"
        SELECT name, value, metadata, created_at, updated_at
        FROM setup_credentials
        ORDER BY name ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(SetupCredentialsResponse {
        credentials: rows
            .into_iter()
            .map(SetupCredential::from)
            .map(|mut credential| {
                credential.metadata = redact_credential_metadata(credential.metadata);
                credential
            })
            .collect(),
    }))
}

#[utoipa::path(
    put,
    path = "/api/setup/credentials/{name}",
    tag = "setup",
    request_body = UpsertCredentialRequest,
    params(("name" = String, Path, description = "Credential name")),
    responses(
        (status = 200, description = "Updated credential", body = SetupCredential),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn upsert_credential(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(name): Path<String>,
    Json(payload): Json<UpsertCredentialRequest>,
) -> Result<Json<SetupCredential>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let name = name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing credential name".to_string(),
        ));
    }
    let metadata = payload.metadata.unwrap_or_else(|| serde_json::json!({}));
    let value = payload.value.trim().to_string();
    if value.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing credential value".to_string(),
        ));
    }

    let row: SetupCredentialRow = sqlx::query_as(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        RETURNING name, value, metadata, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(value)
    .bind(SqlJson(metadata))
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(SetupCredential::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/setup/credentials/{name}",
    tag = "setup",
    params(("name" = String, Path, description = "Credential name")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_credential(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let name = name.trim();
    if name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing credential name".to_string(),
        ));
    }
    let _ = sqlx::query("DELETE FROM setup_credentials WHERE name = $1")
        .bind(name)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/setup/emporia/login",
    tag = "setup",
    request_body = EmporiaLoginRequest,
    responses(
        (status = 200, description = "Emporia tokens stored", body = EmporiaLoginResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 502, description = "Emporia login failed")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn emporia_login(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<EmporiaLoginRequest>,
) -> Result<Json<EmporiaLoginResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let username = payload.username.trim();
    let password = payload.password.trim();
    if username.is_empty() || password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing Emporia username or password".to_string(),
        ));
    }

    let emporia = EmporiaService::new(state.http.clone());
    let tokens = emporia
        .login(username, password)
        .await
        .map_err(upstream_error)?;

    let devices = emporia
        .fetch_devices(&tokens.id_token)
        .await
        .map_err(upstream_error)?;

    let user_provided_allowlist = payload.site_ids.is_some();
    let mut site_ids = payload
        .site_ids
        .clone()
        .unwrap_or_else(|| devices.iter().map(|d| d.device_gid.clone()).collect());
    site_ids.sort();
    site_ids.dedup();

    let include_allowlist: std::collections::HashSet<String> = site_ids.iter().cloned().collect();
    let enabled_device_ids: Vec<String> = devices.iter().map(|d| d.device_gid.clone()).collect();

    let mut metadata = serde_json::json!({
        "username": username,
        // Legacy field: list of enabled/polled device IDs.
        "site_ids": enabled_device_ids,
    });
    if let Some(refresh) = tokens.refresh_token.clone() {
        metadata["refresh_token"] = JsonValue::String(refresh);
    }

    let mut devices_json = serde_json::Map::new();
    for device in &devices {
        let group_label = device
            .address
            .clone()
            .or_else(|| device.name.clone())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        devices_json.insert(
            device.device_gid.clone(),
            serde_json::json!({
                "enabled": true,
                "hidden": false,
                "include_in_power_summary": if user_provided_allowlist {
                    include_allowlist.contains(&device.device_gid)
                } else {
                    true
                },
                "group_label": group_label,
                "circuits": {
                    "mains": {
                        "enabled": true,
                        "hidden": false,
                        "include_in_power_summary": true,
                    }
                },
            }),
        );
    }
    metadata["devices"] = JsonValue::Object(devices_json);

    let _ = sqlx::query(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        "#,
    )
    .bind("emporia")
    .bind(tokens.id_token)
    .bind(SqlJson(metadata))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(EmporiaLoginResponse {
        token_present: true,
        site_ids,
        devices,
    }))
}

#[utoipa::path(
    get,
    path = "/api/setup/emporia/devices",
    tag = "setup",
    responses(
        (status = 200, description = "Emporia devices + preferences", body = EmporiaDevicesResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 502, description = "Emporia API failed")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn emporia_devices(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<EmporiaDevicesResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let row: Option<SetupCredentialRow> = sqlx::query_as(
        r#"
        SELECT name, value, metadata, created_at, updated_at
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind("emporia")
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Ok(Json(EmporiaDevicesResponse {
            token_present: false,
            site_ids: vec![],
            devices: vec![],
        }));
    };

    let mut id_token = row.value.trim().to_string();
    let mut metadata = row.metadata.0.clone();
    let refresh_token = metadata
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let legacy_site_ids: Vec<String> = metadata
        .get("site_ids")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(num) = item.as_i64() {
                        Some(num.to_string())
                    } else {
                        item.as_str().map(|s| s.to_string())
                    }
                })
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let emporia = EmporiaService::new(state.http.clone());
    if id_token.is_empty() {
        if let Some(token) = refresh_token.as_ref() {
            let refreshed = emporia
                .refresh_with_token(token)
                .await
                .map_err(upstream_error)?;
            id_token = refreshed.id_token;
            metadata["refresh_token"] = JsonValue::String(token.clone());
        }
    }
    if id_token.is_empty() {
        return Ok(Json(EmporiaDevicesResponse {
            token_present: false,
            site_ids: vec![],
            devices: vec![],
        }));
    }

    let devices = match emporia.fetch_devices(&id_token).await {
        Ok(value) => value,
        Err(err) => {
            if err.to_string().to_lowercase().contains("401") {
                if let Some(token) = refresh_token.as_ref() {
                    let refreshed = emporia
                        .refresh_with_token(token)
                        .await
                        .map_err(upstream_error)?;
                    id_token = refreshed.id_token;
                    metadata["refresh_token"] = JsonValue::String(token.clone());
                    emporia
                        .fetch_devices(&id_token)
                        .await
                        .map_err(upstream_error)?
                } else {
                    return Err(upstream_error(err));
                }
            } else {
                return Err(upstream_error(err));
            }
        }
    };

    let (mut prefs, enabled_device_gids, _included) =
        merge_emporia_device_preferences(&devices, &mut metadata, &legacy_site_ids);

    let device_gids: Vec<String> = devices.iter().map(|d| d.device_gid.clone()).collect();
    let usage = if device_gids.is_empty() {
        None
    } else {
        match emporia.fetch_usage(&id_token, &device_gids).await {
            Ok(value) => Some(value),
            Err(err) => {
                warn!("Emporia Setup devices: failed to load circuits; continuing with cached preferences: {err:#}");
                None
            }
        }
    };

    let mut circuits_by_device_gid: HashMap<String, Vec<EmporiaCircuitSettings>> = HashMap::new();
    if let Some(usage) = usage.as_ref() {
        merge_emporia_circuit_preferences(usage, &mut prefs, &mut metadata);
        for device in &usage.devices {
            let circuits = build_emporia_circuit_settings(device, prefs.get(&device.device_gid));
            circuits_by_device_gid.insert(device.device_gid.clone(), circuits);
        }
    }

    let devices_with_prefs: Vec<EmporiaDeviceSettings> = devices
        .iter()
        .map(|device| {
            let pref = prefs.get(&device.device_gid);
            let circuits = circuits_by_device_gid
                .get(&device.device_gid)
                .cloned()
                .or_else(|| pref.map(|pref| build_emporia_circuit_settings_fallback(pref)))
                .unwrap_or_default();
            EmporiaDeviceSettings {
                device_gid: device.device_gid.clone(),
                name: device.name.clone(),
                model: device.model.clone(),
                firmware: device.firmware.clone(),
                address: device.address.clone(),
                enabled: pref.map(|p| p.enabled).unwrap_or(true),
                hidden: pref.map(|p| p.hidden).unwrap_or(false),
                include_in_power_summary: pref.map(|p| p.include_in_power_summary).unwrap_or(true),
                group_label: pref.and_then(|p| p.group_label.clone()),
                circuits,
            }
        })
        .collect();

    // Persist metadata normalization + refreshed tokens.
    sqlx::query(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        "#,
    )
    .bind("emporia")
    .bind(&id_token)
    .bind(SqlJson(metadata))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(EmporiaDevicesResponse {
        token_present: true,
        site_ids: enabled_device_gids,
        devices: devices_with_prefs,
    }))
}

#[utoipa::path(
    put,
    path = "/api/setup/emporia/devices",
    tag = "setup",
    request_body = EmporiaDevicesUpdateRequest,
    responses(
        (status = 204, description = "Preferences updated"),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn emporia_update_devices(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<EmporiaDevicesUpdateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let row: Option<(String, SqlJson<JsonValue>)> = sqlx::query_as(
        r#"
        SELECT value, metadata
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind("emporia")
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some((value, metadata_row)) = row else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Emporia credential missing; run Emporia cloud login first.".to_string(),
        ));
    };

    let mut metadata = metadata_row.0;
    if metadata
        .get("devices")
        .and_then(|v| v.as_object())
        .is_none()
    {
        metadata["devices"] = JsonValue::Object(serde_json::Map::new());
    }

    let Some(devices_obj) = metadata.get_mut("devices").and_then(|v| v.as_object_mut()) else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Emporia device metadata corrupted".to_string(),
        ));
    };

    for update in payload.devices {
        let device_gid = update.device_gid.trim();
        if device_gid.is_empty() {
            continue;
        }

        let entry = devices_obj
            .entry(device_gid.to_string())
            .or_insert_with(|| serde_json::json!({}));

        if let Some(enabled) = update.enabled {
            entry["enabled"] = JsonValue::Bool(enabled);
        }
        if let Some(hidden) = update.hidden {
            entry["hidden"] = JsonValue::Bool(hidden);
        }
        if let Some(include) = update.include_in_power_summary {
            entry["include_in_power_summary"] = JsonValue::Bool(include);
        }
        if let Some(group_label) = update.group_label {
            let group_label = group_label.trim().to_string();
            if group_label.is_empty() {
                entry["group_label"] = JsonValue::Null;
            } else {
                entry["group_label"] = JsonValue::String(group_label);
            }
        }

        if let Some(circuits) = update.circuits {
            if entry.get("circuits").and_then(|v| v.as_object()).is_none() {
                entry["circuits"] = JsonValue::Object(serde_json::Map::new());
            }

            let Some(circuits_obj) = entry.get_mut("circuits").and_then(|v| v.as_object_mut())
            else {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Emporia circuits metadata corrupted".to_string(),
                ));
            };

            for circuit_update in circuits {
                let circuit_key = circuit_update.circuit_key.trim();
                if circuit_key.is_empty() {
                    continue;
                }

                let circuit_entry = circuits_obj
                    .entry(circuit_key.to_string())
                    .or_insert_with(|| serde_json::json!({}));

                if let Some(enabled) = circuit_update.enabled {
                    circuit_entry["enabled"] = JsonValue::Bool(enabled);
                }
                if let Some(hidden) = circuit_update.hidden {
                    circuit_entry["hidden"] = JsonValue::Bool(hidden);
                }
                if let Some(include) = circuit_update.include_in_power_summary {
                    circuit_entry["include_in_power_summary"] = JsonValue::Bool(include);
                }
            }
        }
    }

    // Refresh enabled site_ids for legacy consumers.
    let prefs = parse_emporia_device_preferences(&metadata);
    let mut enabled_device_gids: Vec<String> = prefs
        .iter()
        .filter_map(|(gid, entry)| {
            if entry.enabled {
                Some(gid.clone())
            } else {
                None
            }
        })
        .collect();
    enabled_device_gids.sort();
    enabled_device_gids.dedup();
    metadata["site_ids"] = JsonValue::Array(
        enabled_device_gids
            .iter()
            .map(|id| JsonValue::String(id.clone()))
            .collect(),
    );

    sqlx::query(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        "#,
    )
    .bind("emporia")
    .bind(value.trim())
    .bind(SqlJson(metadata))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    // Apply preference changes to already-ingested nodes/sensors so the dashboard updates immediately.
    let mains_metrics = vec![
        "mains_power_w".to_string(),
        "mains_voltage_v".to_string(),
        "mains_current_a".to_string(),
    ];
    for (device_gid, device_pref) in &prefs {
        let node_patch = serde_json::json!({
            "poll_enabled": device_pref.enabled,
            "hidden": device_pref.hidden,
            "include_in_power_summary": device_pref.include_in_power_summary,
            "group_label": device_pref.group_label,
        });
        sqlx::query(
            r#"
            UPDATE nodes
            SET config = COALESCE(config, '{}'::jsonb) || $1
            WHERE external_provider = $2
              AND external_id = $3
            "#,
        )
        .bind(SqlJson(node_patch))
        .bind("emporia")
        .bind(device_gid)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;

        let mains_pref = device_pref
            .circuits
            .get(EMPORIA_CIRCUIT_KEY_MAINS)
            .cloned()
            .unwrap_or(EmporiaCircuitPreferences {
                enabled: true,
                hidden: false,
                include_in_power_summary: true,
            });
        let mains_poll_enabled = device_pref.enabled && mains_pref.enabled;
        let mains_patch = serde_json::json!({
            "poll_enabled": mains_poll_enabled,
            "hidden": device_pref.hidden || mains_pref.hidden,
            "include_in_power_summary": mains_pref.include_in_power_summary,
            "channel_key": EMPORIA_CIRCUIT_KEY_MAINS,
        });
        sqlx::query(
            r#"
            UPDATE sensors
            SET config = COALESCE(config, '{}'::jsonb) || $1
            WHERE deleted_at IS NULL
              AND COALESCE(config->>'external_provider', '') = $2
              AND COALESCE(config->>'external_id', '') = $3
              AND COALESCE(config->>'metric', '') = ANY($4)
            "#,
        )
        .bind(SqlJson(mains_patch))
        .bind("emporia")
        .bind(device_gid)
        .bind(&mains_metrics)
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;

        let summary_patch = serde_json::json!({
            "poll_enabled": device_pref.enabled,
            "hidden": true,
            "channel_key": "summary",
        });
        sqlx::query(
            r#"
            UPDATE sensors
            SET config = COALESCE(config, '{}'::jsonb) || $1
            WHERE deleted_at IS NULL
              AND COALESCE(config->>'external_provider', '') = $2
              AND COALESCE(config->>'external_id', '') = $3
              AND COALESCE(config->>'metric', '') = $4
            "#,
        )
        .bind(SqlJson(summary_patch))
        .bind("emporia")
        .bind(device_gid)
        .bind("power_summary_w")
        .execute(&state.db)
        .await
        .map_err(map_db_error)?;

        for (circuit_key, circuit) in &device_pref.circuits {
            if circuit_key.as_str() == EMPORIA_CIRCUIT_KEY_MAINS {
                continue;
            }

            let poll_enabled = device_pref.enabled && circuit.enabled;
            let patch = serde_json::json!({
                "poll_enabled": poll_enabled,
                "hidden": device_pref.hidden || circuit.hidden,
                "include_in_power_summary": circuit.include_in_power_summary,
            });
            sqlx::query(
                r#"
                UPDATE sensors
                SET config = COALESCE(config, '{}'::jsonb) || $1
                WHERE deleted_at IS NULL
                  AND COALESCE(config->>'external_provider', '') = $2
                  AND COALESCE(config->>'external_id', '') = $3
                  AND COALESCE(config->>'channel_key', '') = $4
                "#,
            )
            .bind(SqlJson(patch))
            .bind("emporia")
            .bind(device_gid)
            .bind(circuit_key)
            .execute(&state.db)
            .await
            .map_err(map_db_error)?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/setup/integrations/mapillary/token",
    tag = "setup",
    responses((status = 200, description = "Mapillary access token (admin-only)", body = MapillaryTokenResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_mapillary_token(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<MapillaryTokenResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let token: Option<String> = sqlx::query_scalar(
        r#"
        SELECT value
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind("mapillary")
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let token = token.unwrap_or_default();
    let token = token.trim().to_string();
    Ok(Json(MapillaryTokenResponse {
        configured: !token.is_empty(),
        access_token: if token.is_empty() { None } else { Some(token) },
    }))
}

fn emporia_channel_display_name(channel: &EmporiaChannelReading) -> String {
    if let Some(name) = channel
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return name.to_string();
    }

    let raw = channel.raw_channel_num.trim();
    if let Some(nested_gid) = channel
        .nested_device_gid
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        if raw.is_empty() {
            return format!("Device {nested_gid} · Channel");
        }
        return format!("Device {nested_gid} · Channel {raw}");
    }

    if raw.is_empty() {
        return format!("Channel {}", channel.channel_num.trim());
    }

    format!("Channel {raw}")
}

fn build_emporia_circuit_settings(
    device: &crate::services::emporia::EmporiaDeviceReading,
    prefs: Option<&EmporiaDevicePreferences>,
) -> Vec<EmporiaCircuitSettings> {
    let default_mains = EmporiaCircuitPreferences {
        enabled: true,
        hidden: false,
        include_in_power_summary: true,
    };
    let default_circuit = EmporiaCircuitPreferences {
        enabled: true,
        hidden: false,
        include_in_power_summary: false,
    };

    let mains_prefs = prefs
        .and_then(|entry| entry.circuits.get(EMPORIA_CIRCUIT_KEY_MAINS))
        .unwrap_or(&default_mains);

    let mut circuits = Vec::new();
    circuits.push(EmporiaCircuitSettings {
        circuit_key: EMPORIA_CIRCUIT_KEY_MAINS.to_string(),
        name: "Mains total".to_string(),
        raw_channel_num: None,
        nested_device_gid: None,
        enabled: mains_prefs.enabled,
        hidden: mains_prefs.hidden,
        include_in_power_summary: mains_prefs.include_in_power_summary,
        is_mains: true,
    });

    let mut channels: Vec<&EmporiaChannelReading> = device.channels.iter().collect();
    channels.sort_by(|a, b| a.channel_num.cmp(&b.channel_num));

    for channel in channels {
        let circuit_key = channel.channel_num.trim();
        if circuit_key.is_empty() {
            continue;
        }

        let circuit_prefs = prefs
            .and_then(|entry| entry.circuits.get(circuit_key))
            .unwrap_or(&default_circuit);

        circuits.push(EmporiaCircuitSettings {
            circuit_key: circuit_key.to_string(),
            name: emporia_channel_display_name(channel),
            raw_channel_num: Some(channel.raw_channel_num.clone()).filter(|v| !v.trim().is_empty()),
            nested_device_gid: channel.nested_device_gid.clone(),
            enabled: circuit_prefs.enabled,
            hidden: circuit_prefs.hidden,
            include_in_power_summary: circuit_prefs.include_in_power_summary,
            is_mains: channel.is_mains,
        });
    }

    circuits
}

fn build_emporia_circuit_settings_fallback(
    prefs: &EmporiaDevicePreferences,
) -> Vec<EmporiaCircuitSettings> {
    let mut circuits = Vec::new();

    let mains = prefs
        .circuits
        .get(EMPORIA_CIRCUIT_KEY_MAINS)
        .cloned()
        .unwrap_or(EmporiaCircuitPreferences {
            enabled: true,
            hidden: false,
            include_in_power_summary: true,
        });
    circuits.push(EmporiaCircuitSettings {
        circuit_key: EMPORIA_CIRCUIT_KEY_MAINS.to_string(),
        name: "Mains total".to_string(),
        raw_channel_num: None,
        nested_device_gid: None,
        enabled: mains.enabled,
        hidden: mains.hidden,
        include_in_power_summary: mains.include_in_power_summary,
        is_mains: true,
    });

    let mut keys: Vec<&String> = prefs
        .circuits
        .keys()
        .filter(|key| key.as_str() != EMPORIA_CIRCUIT_KEY_MAINS)
        .collect();
    keys.sort();

    for key in keys {
        let circuit = prefs
            .circuits
            .get(key)
            .cloned()
            .unwrap_or(EmporiaCircuitPreferences {
                enabled: true,
                hidden: false,
                include_in_power_summary: false,
            });

        circuits.push(EmporiaCircuitSettings {
            circuit_key: key.clone(),
            name: format!("Circuit {key}"),
            raw_channel_num: None,
            nested_device_gid: None,
            enabled: circuit.enabled,
            hidden: circuit.hidden,
            include_in_power_summary: circuit.include_in_power_summary,
            is_mains: false,
        });
    }

    circuits
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/setup/credentials", get(list_credentials))
        .route(
            "/setup/credentials/{name}",
            put(upsert_credential).delete(delete_credential),
        )
        .route("/setup/emporia/login", post(emporia_login))
        .route(
            "/setup/emporia/devices",
            get(emporia_devices).put(emporia_update_devices),
        )
        .route(
            "/setup/integrations/mapillary/token",
            get(get_mapillary_token),
        )
}

fn upstream_error(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_GATEWAY, err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_credential_metadata_redacts_known_sensitive_keys() {
        let input = serde_json::json!({
            "username": "user@example.com",
            "refresh_token": "secret-refresh",
            "nested": { "api_token": "secret-api", "other": 123 },
            "items": [{ "password": "secret-pass" }]
        });

        let redacted = redact_credential_metadata(input);
        assert_eq!(
            redacted.get("username").and_then(|v| v.as_str()),
            Some("user@example.com")
        );
        assert_eq!(
            redacted.get("refresh_token").and_then(|v| v.as_str()),
            Some("<redacted>")
        );
        assert_eq!(
            redacted
                .get("nested")
                .and_then(|v| v.get("api_token"))
                .and_then(|v| v.as_str()),
            Some("<redacted>")
        );
        assert_eq!(
            redacted
                .get("items")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|v| v.get("password"))
                .and_then(|v| v.as_str()),
            Some("<redacted>")
        );
    }
}
