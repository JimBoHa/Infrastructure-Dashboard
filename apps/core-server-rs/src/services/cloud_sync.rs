use anyhow::{anyhow, Context, Result};
use axum::extract::State;
use chrono::{DateTime, Utc};
use rand::{rngs::OsRng, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use sha2::{Digest, Sha256};
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use subtle::ConstantTimeEq;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use crate::auth::{AuthUser, AuthenticatedUser};
use crate::core_node::CORE_NODE_ID;
use crate::routes::{backups, dashboard};
use crate::state::AppState;

const LOCAL_KEY_CREDENTIAL: &str = "cloud_access/local_site_key";
const LOCAL_SETTINGS_CREDENTIAL: &str = "cloud_access/local_settings";
const LOCAL_STATUS_CREDENTIAL: &str = "cloud_access/local_status";
const SITE_CREDENTIAL_PREFIX: &str = "cloud_access/site/";
const SITE_KEY_LEN: usize = 32;
const DEFAULT_SYNC_INTERVAL_SECONDS: u64 = 300;
const MIN_SYNC_INTERVAL_SECONDS: u64 = 60;
const MAX_SYNC_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
const METRICS_BATCH_LIMIT: i64 = 10_000;

const KEY_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum CloudRole {
    Local,
    Cloud,
}

impl CloudRole {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Cloud => "cloud",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CloudLocalSettings {
    pub cloud_server_base_url: Option<String>,
    pub sync_interval_seconds: u64,
    pub sync_enabled: bool,
}

impl Default for CloudLocalSettings {
    fn default() -> Self {
        Self {
            cloud_server_base_url: None,
            sync_interval_seconds: DEFAULT_SYNC_INTERVAL_SECONDS,
            sync_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct CloudSyncStatus {
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub last_http_status: Option<u16>,
    pub last_synced_inserted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LocalCloudAccessState {
    pub local_site_key: String,
    pub settings: CloudLocalSettings,
    pub status: CloudSyncStatus,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CloudSiteSummary {
    pub site_id: String,
    pub site_name: String,
    pub key_fingerprint: String,
    pub enabled: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub last_ingested_at: Option<String>,
    pub last_payload_bytes: Option<u64>,
    pub last_metrics_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CloudAccessSettingsPatch {
    pub cloud_server_base_url: Option<String>,
    pub sync_interval_seconds: Option<u64>,
    pub sync_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub(crate) struct CloudMetricPoint {
    pub sensor_id: String,
    pub ts: String,
    pub value: f64,
    pub quality: i16,
    pub inserted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub(crate) struct CloudSyncPayload {
    pub schema_version: u32,
    pub sent_at: String,
    pub source_name: String,
    pub dashboard_snapshot: JsonValue,
    pub backups: JsonValue,
    pub backup_retention: JsonValue,
    #[serde(default)]
    pub metrics: Vec<CloudMetricPoint>,
    pub metrics_max_inserted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub(crate) struct CloudIngestResult {
    pub site_id: String,
    pub site_name: String,
    pub accepted_metrics: usize,
}

#[derive(Debug, sqlx::FromRow)]
struct SetupCredentialRow {
    name: String,
    value: String,
    metadata: SqlJson<JsonValue>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct MetricSyncRow {
    sensor_id: String,
    ts: DateTime<Utc>,
    value: f64,
    quality: i16,
    inserted_at: DateTime<Utc>,
}

pub(crate) fn runtime_cloud_role() -> CloudRole {
    let role = std::env::var("CORE_CLOUD_ROLE")
        .ok()
        .map(|value| value.trim().to_lowercase())
        .unwrap_or_else(|| "local".to_string());

    match role.as_str() {
        "cloud" => CloudRole::Cloud,
        _ => CloudRole::Local,
    }
}

pub(crate) fn generate_site_key() -> String {
    let mut rng = OsRng;
    let mut key = String::with_capacity(SITE_KEY_LEN);
    for _ in 0..SITE_KEY_LEN {
        let idx = rng.gen_range(0..KEY_ALPHABET.len());
        key.push(KEY_ALPHABET[idx] as char);
    }
    key
}

pub(crate) fn validate_site_key(raw: &str) -> Result<String> {
    let key = raw.trim();
    if key.len() != SITE_KEY_LEN {
        anyhow::bail!("site key must be exactly {SITE_KEY_LEN} characters")
    }
    if !key.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        anyhow::bail!("site key must contain only letters and numbers")
    }
    Ok(key.to_string())
}

pub(crate) fn normalize_cloud_server_url(raw: &str) -> Result<Option<String>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = Url::parse(trimmed).context("cloud_server_base_url must be a valid URL")?;
    let scheme = parsed.scheme().to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        anyhow::bail!("cloud_server_base_url must use http or https")
    }
    if parsed.host_str().is_none() {
        anyhow::bail!("cloud_server_base_url must include a host")
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        anyhow::bail!("cloud_server_base_url must not include credentials")
    }
    if parsed.fragment().is_some() {
        anyhow::bail!("cloud_server_base_url must not include a fragment")
    }
    if parsed.query().is_some() {
        anyhow::bail!("cloud_server_base_url must not include query parameters")
    }

    Ok(Some(parsed.to_string().trim_end_matches('/').to_string()))
}

pub(crate) fn hash_site_key(site_key: &str) -> String {
    let digest = Sha256::digest(site_key.as_bytes());
    bytes_to_hex(&digest)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hashes_match(left: &str, right: &str) -> bool {
    let left = left.as_bytes();
    let right = right.as_bytes();
    left.len() == right.len() && bool::from(left.ct_eq(right))
}

async fn fetch_credential(
    db: &PgPool,
    name: &str,
) -> Result<Option<SetupCredentialRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT name, value, metadata, created_at, updated_at
        FROM setup_credentials
        WHERE name = $1
        LIMIT 1
        "#,
    )
    .bind(name)
    .fetch_optional(db)
    .await
}

async fn list_credentials_by_prefix(
    db: &PgPool,
    prefix: &str,
) -> Result<Vec<SetupCredentialRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT name, value, metadata, created_at, updated_at
        FROM setup_credentials
        WHERE name LIKE $1
        ORDER BY name ASC
        "#,
    )
    .bind(format!("{prefix}%"))
    .fetch_all(db)
    .await
}

async fn upsert_credential(
    db: &PgPool,
    name: &str,
    value: &str,
    metadata: JsonValue,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        "#,
    )
    .bind(name)
    .bind(value)
    .bind(SqlJson(metadata))
    .execute(db)
    .await?;
    Ok(())
}

async fn delete_credential(db: &PgPool, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM setup_credentials WHERE name = $1")
        .bind(name)
        .execute(db)
        .await?;
    Ok(())
}

fn metadata_object(value: &JsonValue) -> JsonMap<String, JsonValue> {
    value
        .as_object()
        .cloned()
        .unwrap_or_else(JsonMap::<String, JsonValue>::new)
}

fn metadata_string(value: &JsonMap<String, JsonValue>, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(JsonValue::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

fn metadata_u64(value: &JsonMap<String, JsonValue>, key: &str, default: u64) -> u64 {
    value
        .get(key)
        .and_then(JsonValue::as_u64)
        .unwrap_or(default)
}

fn metadata_bool(value: &JsonMap<String, JsonValue>, key: &str, default: bool) -> bool {
    value
        .get(key)
        .and_then(JsonValue::as_bool)
        .unwrap_or(default)
}

pub(crate) async fn ensure_local_site_key(db: &PgPool) -> Result<String> {
    if let Some(existing) = fetch_credential(db, LOCAL_KEY_CREDENTIAL).await? {
        let key = existing.value.trim().to_string();
        if validate_site_key(&key).is_ok() {
            return Ok(key);
        }
    }

    let key = generate_site_key();
    upsert_credential(
        db,
        LOCAL_KEY_CREDENTIAL,
        &key,
        json!({
            "created_at": Utc::now().to_rfc3339(),
            "rotation": "automatic"
        }),
    )
    .await?;
    Ok(key)
}

pub(crate) async fn rotate_local_site_key(db: &PgPool) -> Result<String> {
    let key = generate_site_key();
    upsert_credential(
        db,
        LOCAL_KEY_CREDENTIAL,
        &key,
        json!({
            "rotated_at": Utc::now().to_rfc3339(),
            "rotation": "manual"
        }),
    )
    .await?;
    Ok(key)
}

async fn load_local_settings(db: &PgPool) -> Result<CloudLocalSettings> {
    let row = fetch_credential(db, LOCAL_SETTINGS_CREDENTIAL).await?;
    let Some(row) = row else {
        return Ok(CloudLocalSettings::default());
    };

    let metadata = metadata_object(&row.metadata.0);
    Ok(CloudLocalSettings {
        cloud_server_base_url: metadata_string(&metadata, "cloud_server_base_url"),
        sync_interval_seconds: metadata_u64(
            &metadata,
            "sync_interval_seconds",
            DEFAULT_SYNC_INTERVAL_SECONDS,
        )
        .clamp(MIN_SYNC_INTERVAL_SECONDS, MAX_SYNC_INTERVAL_SECONDS),
        sync_enabled: metadata_bool(&metadata, "sync_enabled", true),
    })
}

async fn save_local_settings(db: &PgPool, settings: &CloudLocalSettings) -> Result<()> {
    upsert_credential(
        db,
        LOCAL_SETTINGS_CREDENTIAL,
        "v1",
        json!({
            "cloud_server_base_url": settings.cloud_server_base_url,
            "sync_interval_seconds": settings.sync_interval_seconds,
            "sync_enabled": settings.sync_enabled,
            "updated_at": Utc::now().to_rfc3339(),
        }),
    )
    .await?;
    Ok(())
}

pub(crate) async fn load_sync_status(db: &PgPool) -> Result<CloudSyncStatus> {
    let row = fetch_credential(db, LOCAL_STATUS_CREDENTIAL).await?;
    let Some(row) = row else {
        return Ok(CloudSyncStatus::default());
    };

    let metadata = metadata_object(&row.metadata.0);
    Ok(CloudSyncStatus {
        last_attempt_at: metadata_string(&metadata, "last_attempt_at"),
        last_success_at: metadata_string(&metadata, "last_success_at"),
        last_error: metadata_string(&metadata, "last_error"),
        last_http_status: metadata
            .get("last_http_status")
            .and_then(JsonValue::as_u64)
            .map(|value| value as u16),
        last_synced_inserted_at: metadata_string(&metadata, "last_synced_inserted_at"),
    })
}

pub(crate) async fn save_sync_status(db: &PgPool, status: &CloudSyncStatus) -> Result<()> {
    upsert_credential(
        db,
        LOCAL_STATUS_CREDENTIAL,
        "v1",
        json!({
            "last_attempt_at": status.last_attempt_at,
            "last_success_at": status.last_success_at,
            "last_error": status.last_error,
            "last_http_status": status.last_http_status,
            "last_synced_inserted_at": status.last_synced_inserted_at,
            "updated_at": Utc::now().to_rfc3339(),
        }),
    )
    .await?;
    Ok(())
}

pub(crate) async fn load_local_cloud_access_state(db: &PgPool) -> Result<LocalCloudAccessState> {
    let local_site_key = ensure_local_site_key(db).await?;
    let settings = load_local_settings(db).await?;
    let status = load_sync_status(db).await?;
    Ok(LocalCloudAccessState {
        local_site_key,
        settings,
        status,
    })
}

pub(crate) async fn update_local_settings(
    db: &PgPool,
    patch: CloudAccessSettingsPatch,
) -> Result<LocalCloudAccessState> {
    let mut settings = load_local_settings(db).await?;

    if let Some(url) = patch.cloud_server_base_url {
        settings.cloud_server_base_url = normalize_cloud_server_url(&url)?;
    }

    if let Some(interval_seconds) = patch.sync_interval_seconds {
        if !(MIN_SYNC_INTERVAL_SECONDS..=MAX_SYNC_INTERVAL_SECONDS).contains(&interval_seconds) {
            anyhow::bail!(
                "sync_interval_seconds must be between {MIN_SYNC_INTERVAL_SECONDS} and {MAX_SYNC_INTERVAL_SECONDS}"
            );
        }
        settings.sync_interval_seconds = interval_seconds;
    }

    if let Some(sync_enabled) = patch.sync_enabled {
        settings.sync_enabled = sync_enabled;
    }

    save_local_settings(db, &settings).await?;
    load_local_cloud_access_state(db).await
}

fn site_id_from_name(name: &str) -> String {
    name.trim_start_matches(SITE_CREDENTIAL_PREFIX).to_string()
}

fn site_name_from_metadata(metadata: &JsonMap<String, JsonValue>, fallback: &str) -> String {
    metadata_string(metadata, "site_name").unwrap_or_else(|| fallback.to_string())
}

fn site_enabled_from_metadata(metadata: &JsonMap<String, JsonValue>) -> bool {
    metadata_bool(metadata, "enabled", true)
}

fn parse_site_summary(row: SetupCredentialRow) -> CloudSiteSummary {
    let site_id = site_id_from_name(&row.name);
    let metadata = metadata_object(&row.metadata.0);
    let site_name = site_name_from_metadata(&metadata, &site_id);
    let key_fingerprint = row.value.chars().take(12).collect::<String>();

    CloudSiteSummary {
        site_id,
        site_name,
        key_fingerprint,
        enabled: site_enabled_from_metadata(&metadata),
        created_at: Some(row.created_at.to_rfc3339()),
        updated_at: Some(row.updated_at.to_rfc3339()),
        last_ingested_at: metadata_string(&metadata, "last_ingested_at"),
        last_payload_bytes: metadata
            .get("last_payload_bytes")
            .and_then(JsonValue::as_u64),
        last_metrics_count: metadata
            .get("last_metrics_count")
            .and_then(JsonValue::as_u64),
    }
}

pub(crate) async fn count_registered_sites(db: &PgPool) -> Result<u64> {
    let rows = list_credentials_by_prefix(db, SITE_CREDENTIAL_PREFIX).await?;
    Ok(rows.len() as u64)
}

pub(crate) async fn list_registered_sites(db: &PgPool) -> Result<Vec<CloudSiteSummary>> {
    let rows = list_credentials_by_prefix(db, SITE_CREDENTIAL_PREFIX).await?;
    Ok(rows.into_iter().map(parse_site_summary).collect())
}

pub(crate) async fn register_cloud_site(
    db: &PgPool,
    site_name: &str,
    site_key: &str,
) -> Result<CloudSiteSummary> {
    let trimmed_name = site_name.trim();
    if trimmed_name.is_empty() {
        anyhow::bail!("site_name is required")
    }
    if trimmed_name.len() > 120 {
        anyhow::bail!("site_name must be 120 characters or fewer")
    }

    let normalized_key = validate_site_key(site_key)?;
    let hashed_key = hash_site_key(&normalized_key);

    let existing = list_credentials_by_prefix(db, SITE_CREDENTIAL_PREFIX).await?;
    if existing
        .iter()
        .any(|row| hashes_match(row.value.trim(), &hashed_key))
    {
        anyhow::bail!("site_key is already registered")
    }

    let site_id = Uuid::new_v4().to_string();
    let credential_name = format!("{SITE_CREDENTIAL_PREFIX}{site_id}");
    upsert_credential(
        db,
        &credential_name,
        &hashed_key,
        json!({
            "site_name": trimmed_name,
            "enabled": true,
            "created_at": Utc::now().to_rfc3339(),
            "last_ingested_at": null,
            "last_payload_bytes": null,
            "last_metrics_count": null,
        }),
    )
    .await?;

    let row = fetch_credential(db, &credential_name)
        .await?
        .ok_or_else(|| anyhow!("registered site not found after insert"))?;
    Ok(parse_site_summary(row))
}

pub(crate) async fn delete_cloud_site(db: &PgPool, site_id: &str) -> Result<()> {
    let site_id = site_id.trim();
    if site_id.is_empty() {
        anyhow::bail!("site_id is required")
    }
    let name = format!("{SITE_CREDENTIAL_PREFIX}{site_id}");
    delete_credential(db, &name).await?;
    Ok(())
}

async fn find_site_row_for_key(db: &PgPool, site_key: &str) -> Result<Option<SetupCredentialRow>> {
    let normalized = validate_site_key(site_key)?;
    let hashed = hash_site_key(&normalized);

    let rows = list_credentials_by_prefix(db, SITE_CREDENTIAL_PREFIX).await?;
    for row in rows {
        if !hashes_match(row.value.trim(), &hashed) {
            continue;
        }
        let metadata = metadata_object(&row.metadata.0);
        if site_enabled_from_metadata(&metadata) {
            return Ok(Some(row));
        }
    }

    Ok(None)
}

pub(crate) fn cloud_site_directory(data_root: &Path, site_id: &str) -> PathBuf {
    data_root.join("cloud/sites").join(site_id)
}

pub(crate) async fn load_cloud_site_snapshot(
    data_root: &Path,
    site_id: &str,
) -> Result<Option<JsonValue>> {
    let site_dir = cloud_site_directory(data_root, site_id.trim());
    let latest_path = site_dir.join("latest.json");
    let bytes = match tokio::fs::read(&latest_path).await {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", latest_path.display()))
        }
    };

    let value = serde_json::from_slice::<JsonValue>(&bytes)
        .with_context(|| format!("failed to parse {}", latest_path.display()))?;
    Ok(Some(value))
}

pub(crate) async fn ingest_cloud_payload(
    state: &AppState,
    site_key: &str,
    payload: &CloudSyncPayload,
    payload_len_bytes: usize,
) -> Result<CloudIngestResult> {
    let Some(site_row) = find_site_row_for_key(&state.db, site_key).await? else {
        anyhow::bail!("invalid site key")
    };

    let site_id = site_id_from_name(&site_row.name);
    let mut metadata = metadata_object(&site_row.metadata.0);
    let site_name = site_name_from_metadata(&metadata, &site_id);

    let site_dir = cloud_site_directory(&state.config.data_root, &site_id);
    tokio::fs::create_dir_all(&site_dir)
        .await
        .with_context(|| format!("failed to create {}", site_dir.display()))?;

    let latest_path = site_dir.join("latest.json");
    let tmp_path = site_dir.join("latest.json.tmp");
    let payload_bytes =
        serde_json::to_vec_pretty(payload).context("failed to encode sync payload")?;
    tokio::fs::write(&tmp_path, &payload_bytes)
        .await
        .with_context(|| format!("failed to write {}", tmp_path.display()))?;
    tokio::fs::rename(&tmp_path, &latest_path)
        .await
        .with_context(|| format!("failed to move {}", latest_path.display()))?;

    if !payload.metrics.is_empty() {
        let metrics_path = site_dir.join("metrics.jsonl");
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&metrics_path)
            .await
            .with_context(|| format!("failed to open {}", metrics_path.display()))?;

        for metric in &payload.metrics {
            let line = serde_json::to_string(metric).context("failed to encode metric line")?;
            file.write_all(line.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }
        file.flush().await?;
    }

    let history_path = site_dir.join("ingest_history.jsonl");
    let mut history = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_path)
        .await
        .with_context(|| format!("failed to open {}", history_path.display()))?;
    let history_record = json!({
        "received_at": Utc::now().to_rfc3339(),
        "sent_at": payload.sent_at,
        "source_name": payload.source_name,
        "payload_bytes": payload_len_bytes,
        "metrics_count": payload.metrics.len(),
    });
    history
        .write_all(serde_json::to_string(&history_record)?.as_bytes())
        .await?;
    history.write_all(b"\n").await?;
    history.flush().await?;

    metadata.insert(
        "site_name".to_string(),
        JsonValue::String(site_name.clone()),
    );
    metadata.insert(
        "last_ingested_at".to_string(),
        JsonValue::String(Utc::now().to_rfc3339()),
    );
    metadata.insert(
        "last_payload_bytes".to_string(),
        JsonValue::Number(serde_json::Number::from(payload_len_bytes as u64)),
    );
    metadata.insert(
        "last_metrics_count".to_string(),
        JsonValue::Number(serde_json::Number::from(payload.metrics.len() as u64)),
    );
    metadata.insert(
        "last_source_name".to_string(),
        JsonValue::String(payload.source_name.clone()),
    );
    metadata.insert(
        "last_sent_at".to_string(),
        JsonValue::String(payload.sent_at.clone()),
    );

    upsert_credential(
        &state.db,
        &site_row.name,
        site_row.value.trim(),
        JsonValue::Object(metadata),
    )
    .await?;

    Ok(CloudIngestResult {
        site_id,
        site_name,
        accepted_metrics: payload.metrics.len(),
    })
}

fn cloud_sync_service_user() -> AuthenticatedUser {
    let capabilities: HashSet<String> = [
        "nodes.view",
        "sensors.view",
        "outputs.view",
        "schedules.view",
        "alerts.view",
        "analytics.view",
        "backups.view",
        "users.manage",
        "config.write",
    ]
    .iter()
    .map(|value| value.to_string())
    .collect();

    AuthenticatedUser {
        id: CORE_NODE_ID.to_string(),
        email: "cloud-sync@localhost".to_string(),
        role: "admin".to_string(),
        capabilities,
        source: "cloud-sync".to_string(),
    }
}

fn should_attempt(last_attempt_at: Option<&str>, interval_seconds: u64) -> bool {
    let Some(last_attempt_at) = last_attempt_at else {
        return true;
    };

    let parsed = DateTime::parse_from_rfc3339(last_attempt_at)
        .ok()
        .map(|value| value.with_timezone(&Utc));
    let Some(parsed) = parsed else {
        return true;
    };

    let elapsed = Utc::now() - parsed;
    elapsed.num_seconds() >= interval_seconds as i64
}

async fn fetch_sync_source_name(db: &PgPool) -> Result<String> {
    let core_name: Option<String> = sqlx::query_scalar(
        r#"
        SELECT name
        FROM nodes
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(CORE_NODE_ID)
    .fetch_optional(db)
    .await?;

    if let Some(name) = core_name
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
    {
        return Ok(name);
    }

    let hostname = std::env::var("HOSTNAME")
        .ok()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .unwrap_or_else(|| "Core".to_string());
    Ok(hostname)
}

async fn fetch_metrics_batch(
    db: &PgPool,
    since_inserted_at: Option<&str>,
    limit: i64,
) -> Result<(Vec<CloudMetricPoint>, Option<String>)> {
    let since = since_inserted_at
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|| DateTime::<Utc>::from(std::time::SystemTime::UNIX_EPOCH));

    let rows: Vec<MetricSyncRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            ts,
            value,
            COALESCE(quality, 0)::smallint as quality,
            COALESCE(inserted_at, ts) as inserted_at
        FROM metrics
        WHERE COALESCE(inserted_at, ts) > $1
        ORDER BY COALESCE(inserted_at, ts) ASC
        LIMIT $2
        "#,
    )
    .bind(since)
    .bind(limit)
    .fetch_all(db)
    .await?;

    let metrics = rows
        .iter()
        .map(|row| CloudMetricPoint {
            sensor_id: row.sensor_id.clone(),
            ts: row.ts.to_rfc3339(),
            value: row.value,
            quality: row.quality,
            inserted_at: row.inserted_at.to_rfc3339(),
        })
        .collect::<Vec<_>>();

    let watermark = rows.last().map(|row| row.inserted_at.to_rfc3339());
    Ok((metrics, watermark))
}

pub struct CloudSyncService {
    state: AppState,
    tick_interval: Duration,
}

impl CloudSyncService {
    pub fn new(state: AppState, tick_interval: Duration) -> Self {
        Self {
            state,
            tick_interval,
        }
    }

    pub fn start(self, cancel: CancellationToken) {
        tokio::spawn(async move {
            loop {
                if cancel.is_cancelled() {
                    break;
                }

                if let Err(err) = self.tick_once().await {
                    tracing::warn!(error = %err, "cloud sync tick failed");
                }

                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(self.tick_interval) => {}
                }
            }
        });
    }

    async fn tick_once(&self) -> Result<()> {
        if runtime_cloud_role() != CloudRole::Local {
            return Ok(());
        }

        let settings = load_local_settings(&self.state.db).await?;
        if !settings.sync_enabled {
            return Ok(());
        }

        let Some(cloud_server_base_url) = settings.cloud_server_base_url.clone() else {
            return Ok(());
        };

        let mut status = load_sync_status(&self.state.db).await?;
        if !should_attempt(
            status.last_attempt_at.as_deref(),
            settings.sync_interval_seconds,
        ) {
            return Ok(());
        }

        status.last_attempt_at = Some(Utc::now().to_rfc3339());
        save_sync_status(&self.state.db, &status).await?;

        let site_key = ensure_local_site_key(&self.state.db).await?;
        let payload = self
            .build_payload(status.last_synced_inserted_at.as_deref())
            .await?;

        let endpoint = format!("{cloud_server_base_url}/api/cloud/ingest");
        let response = self
            .state
            .http
            .post(&endpoint)
            .header("x-cloud-site-key", site_key)
            .timeout(Duration::from_secs(45))
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("failed to POST {endpoint}"));

        match response {
            Ok(response) if response.status().is_success() => {
                status.last_success_at = Some(Utc::now().to_rfc3339());
                status.last_error = None;
                status.last_http_status = Some(response.status().as_u16());
                status.last_synced_inserted_at = payload
                    .metrics_max_inserted_at
                    .clone()
                    .or_else(|| status.last_synced_inserted_at.clone())
                    .or_else(|| Some(Utc::now().to_rfc3339()));
            }
            Ok(response) => {
                let status_code = response.status();
                let body = response.text().await.unwrap_or_default();
                status.last_http_status = Some(status_code.as_u16());
                status.last_error = Some(format!(
                    "cloud ingest failed: {} {}",
                    status_code,
                    body.trim()
                ));
            }
            Err(err) => {
                status.last_http_status = None;
                status.last_error = Some(format!("cloud ingest request failed: {err}"));
            }
        }

        save_sync_status(&self.state.db, &status).await?;
        Ok(())
    }

    async fn build_payload(
        &self,
        last_synced_inserted_at: Option<&str>,
    ) -> Result<CloudSyncPayload> {
        let dashboard_snapshot = dashboard::build_cloud_sync_snapshot(&self.state)
            .await
            .map_err(|(status, message)| {
                anyhow!(
                    "dashboard snapshot failed ({}): {}",
                    status.as_u16(),
                    message
                )
            })?;
        let dashboard_snapshot = serde_json::to_value(dashboard_snapshot)
            .context("failed to serialize dashboard snapshot")?;

        let user = cloud_sync_service_user();
        let backups = backups::list_backups(State(self.state.clone()), AuthUser(user.clone()))
            .await
            .map_err(|(status, message)| {
                anyhow!("backup listing failed ({}): {}", status.as_u16(), message)
            })?;
        let backups = serde_json::to_value(backups.0).context("failed to encode backups")?;

        let retention = backups::fetch_retention(
            &self.state.db,
            self.state.config.backup_retention_days as i32,
        )
        .await
        .context("failed to fetch backup retention")?;
        let backup_retention =
            serde_json::to_value(retention).context("failed to encode backup retention")?;

        let (metrics, metrics_max_inserted_at) =
            fetch_metrics_batch(&self.state.db, last_synced_inserted_at, METRICS_BATCH_LIMIT)
                .await
                .context("failed to fetch metrics batch")?;

        Ok(CloudSyncPayload {
            schema_version: 1,
            sent_at: Utc::now().to_rfc3339(),
            source_name: fetch_sync_source_name(&self.state.db).await?,
            dashboard_snapshot,
            backups,
            backup_retention,
            metrics,
            metrics_max_inserted_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_site_key_rules() {
        let key = "Abc123Abc123Abc123Abc123Abc123Ab";
        assert!(validate_site_key(key).is_ok());
        assert!(validate_site_key("short").is_err());
        assert!(validate_site_key("********************************").is_err());
    }

    #[test]
    fn generates_expected_site_key_shape() {
        let key = generate_site_key();
        assert_eq!(key.len(), SITE_KEY_LEN);
        assert!(key.chars().all(|ch| ch.is_ascii_alphanumeric()));
    }

    #[test]
    fn normalizes_cloud_server_url() {
        let normalized = normalize_cloud_server_url("https://example.com/")
            .expect("normalize")
            .expect("url");
        assert_eq!(normalized, "https://example.com");

        assert!(normalize_cloud_server_url("ftp://example.com").is_err());
        assert!(normalize_cloud_server_url("https://user:pass@example.com").is_err());
        assert!(normalize_cloud_server_url("https://example.com?x=1").is_err());
    }

    #[test]
    fn hashes_compare_in_constant_length() {
        let key = "Abc123Abc123Abc123Abc123Abc123Ab";
        let hash = hash_site_key(key);
        assert!(hashes_match(&hash, &hash));
        assert!(!hashes_match(&hash, "deadbeef"));
    }

    #[test]
    fn sync_interval_gate_respects_elapsed_time() {
        assert!(should_attempt(None, 300));

        let now = Utc::now();
        let recent = (now - chrono::Duration::seconds(30)).to_rfc3339();
        let old = (now - chrono::Duration::seconds(500)).to_rfc3339();

        assert!(!should_attempt(Some(&recent), 60));
        assert!(should_attempt(Some(&old), 60));
    }
}
