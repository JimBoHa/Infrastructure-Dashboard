use anyhow::{Context, Result};
use dotenvy::dotenv;
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_SETUP_CONFIG_PATH: &str = "/Users/Shared/FarmDashboard/setup/config.json";

fn setup_config_path() -> PathBuf {
    if let Ok(path) = std::env::var("SIDECAR_SETUP_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(path) = std::env::var("CORE_SETUP_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(path) = std::env::var("FARM_SETUP_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(state_dir) = std::env::var("FARM_SETUP_STATE_DIR") {
        let trimmed = state_dir.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("config.json");
        }
    }
    PathBuf::from(DEFAULT_SETUP_CONFIG_PATH)
}

#[derive(Debug, Clone, Deserialize)]
struct SetupConfigOverrides {
    #[serde(default)]
    database_url: Option<String>,
    #[serde(default)]
    mqtt_host: Option<String>,
    #[serde(default)]
    mqtt_port: Option<u16>,
    #[serde(default)]
    mqtt_username: Option<String>,
    #[serde(default)]
    mqtt_password: Option<String>,
    #[serde(default)]
    offline_threshold_seconds: Option<u64>,
    #[serde(default)]
    sidecar_mqtt_topic_prefix: Option<String>,
    #[serde(default)]
    sidecar_mqtt_keepalive_secs: Option<u64>,
    #[serde(default)]
    sidecar_enable_mqtt_listener: Option<bool>,
    #[serde(default)]
    sidecar_batch_size: Option<usize>,
    #[serde(default)]
    sidecar_flush_interval_ms: Option<u64>,
    #[serde(default)]
    sidecar_max_queue: Option<usize>,
    #[serde(default)]
    sidecar_status_poll_interval_ms: Option<u64>,
}

fn load_setup_config_overrides() -> Option<SetupConfigOverrides> {
    let path = setup_config_path();
    if !path.exists() {
        return None;
    }
    let contents = match std::fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "telemetry-sidecar failed to read setup config; using env defaults"
            );
            return None;
        }
    };
    let mut bytes = contents.into_bytes();
    match simd_json::serde::from_slice(&mut bytes) {
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "telemetry-sidecar failed to parse setup config; using env defaults"
            );
            None
        }
    }
}

fn apply_setup_overrides(
    config: &mut Config,
    overrides: &SetupConfigOverrides,
    allow_mqtt_port_override: bool,
    allow_offline_threshold_override: bool,
) {
    let env_allows = |key: &str| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .is_none()
    };

    if let Some(host) = overrides
        .mqtt_host
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        config.mqtt_host = host.to_string();
    }
    if allow_mqtt_port_override {
        if let Some(port) = overrides.mqtt_port.filter(|v| *v != 0) {
            config.mqtt_port = port;
        }
    }
    if let Some(username) = overrides.mqtt_username.as_deref() {
        let trimmed = username.trim();
        config.mqtt_username = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(password) = overrides.mqtt_password.as_deref() {
        let trimmed = password.trim();
        config.mqtt_password = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }

    if allow_offline_threshold_override {
        if let Some(value) = overrides.offline_threshold_seconds.filter(|v| *v != 0) {
            config.offline_threshold_seconds = value;
        }
    }

    if env_allows("SIDECAR_MQTT_TOPIC_PREFIX") {
        if let Some(prefix) = overrides
            .sidecar_mqtt_topic_prefix
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            config.mqtt_topic_prefix = prefix.to_string();
        }
    }
    if env_allows("SIDECAR_MQTT_KEEPALIVE_SECS") {
        if let Some(value) = overrides.sidecar_mqtt_keepalive_secs.filter(|v| *v != 0) {
            config.mqtt_keepalive_secs = value;
        }
    }
    if env_allows("SIDECAR_ENABLE_MQTT") {
        if let Some(value) = overrides.sidecar_enable_mqtt_listener {
            config.enable_mqtt_listener = value;
        }
    }

    let mut batch_overridden = false;
    if env_allows("SIDECAR_BATCH_SIZE") {
        if let Some(value) = overrides.sidecar_batch_size.filter(|v| *v != 0) {
            config.batch_size = value;
            batch_overridden = true;
        }
    }
    if env_allows("SIDECAR_FLUSH_INTERVAL_MS") {
        if let Some(value) = overrides.sidecar_flush_interval_ms.filter(|v| *v != 0) {
            config.flush_interval_ms = value;
        }
    }
    if env_allows("SIDECAR_STATUS_POLL_INTERVAL_MS") {
        if let Some(value) = overrides
            .sidecar_status_poll_interval_ms
            .filter(|v| *v != 0)
        {
            config.status_poll_interval_ms = value;
        }
    }

    if env_allows("SIDECAR_MAX_QUEUE") {
        if let Some(value) = overrides.sidecar_max_queue.filter(|v| *v != 0) {
            config.max_queue = value;
        } else if batch_overridden {
            config.max_queue = config.batch_size.saturating_mul(10);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub db_pool_size: u32,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_username: Option<String>,
    pub mqtt_password: Option<String>,
    pub mqtt_topic_prefix: String,
    pub mqtt_keepalive_secs: u64,
    pub mqtt_client_id: String,
    pub batch_size: usize,
    pub flush_interval_ms: u64,
    pub max_queue: usize,
    pub grpc_socket_path: String,
    pub enable_mqtt_listener: bool,
    pub offline_threshold_seconds: u64,
    pub status_poll_interval_ms: u64,
    pub predictive_feed_url: Option<String>,
    pub predictive_feed_token: Option<String>,
    pub predictive_feed_batch_size: usize,
    pub predictive_feed_flush_ms: u64,
    pub predictive_feed_queue: usize,
    pub otlp_endpoint: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv().ok();

        let setup_overrides = load_setup_config_overrides();

        let database_url = env::var("SIDECAR_DATABASE_URL")
            .or_else(|_| env::var("DATABASE_URL"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                setup_overrides
                    .as_ref()
                    .and_then(|ov| ov.database_url.as_deref())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
            })
            .context("SIDECAR_DATABASE_URL or DATABASE_URL is required (or present as database_url in the setup config)")?;
        let database_url = normalize_database_url(database_url);

        let mqtt_host = env::var("SIDECAR_MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let mqtt_port = env::var("SIDECAR_MQTT_PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(1883);
        let mqtt_username = env::var("SIDECAR_MQTT_USERNAME").ok();
        let mqtt_password = env::var("SIDECAR_MQTT_PASSWORD").ok();
        let mqtt_topic_prefix =
            env::var("SIDECAR_MQTT_TOPIC_PREFIX").unwrap_or_else(|_| "iot".to_string());
        let mqtt_keepalive_secs = env::var("SIDECAR_MQTT_KEEPALIVE_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);
        let mqtt_client_id = env::var("SIDECAR_MQTT_CLIENT_ID")
            .unwrap_or_else(|_| format!("telemetry-sidecar-{}", std::process::id()));

        let batch_size = env::var("SIDECAR_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(500);
        let flush_interval_ms = env::var("SIDECAR_FLUSH_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(750);
        let max_queue = env::var("SIDECAR_MAX_QUEUE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(batch_size * 10);
        let db_pool_size = env::var("SIDECAR_DB_POOL_SIZE")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(10);

        let grpc_socket_path = env::var("SIDECAR_GRPC_SOCKET")
            .unwrap_or_else(|_| "/tmp/telemetry_ingest.sock".to_string());
        let enable_mqtt_listener = env::var("SIDECAR_ENABLE_MQTT")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(true);
        let offline_threshold_seconds = env::var("SIDECAR_OFFLINE_THRESHOLD_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(30);
        let status_poll_interval_ms = env::var("SIDECAR_STATUS_POLL_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(1000);
        let predictive_feed_url = env::var("SIDECAR_PREDICTIVE_FEED_URL").ok();
        let predictive_feed_token = env::var("SIDECAR_PREDICTIVE_FEED_TOKEN").ok();
        let predictive_feed_batch_size = env::var("SIDECAR_PREDICTIVE_FEED_BATCH_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(200);
        let predictive_feed_flush_ms = env::var("SIDECAR_PREDICTIVE_FEED_FLUSH_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(500);
        let predictive_feed_queue = env::var("SIDECAR_PREDICTIVE_FEED_QUEUE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(predictive_feed_batch_size.saturating_mul(4));
        let otlp_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

        let mut config = Self {
            database_url,
            db_pool_size,
            mqtt_host,
            mqtt_port,
            mqtt_username,
            mqtt_password,
            mqtt_topic_prefix,
            mqtt_keepalive_secs,
            mqtt_client_id,
            batch_size,
            flush_interval_ms,
            max_queue,
            grpc_socket_path,
            enable_mqtt_listener,
            offline_threshold_seconds,
            status_poll_interval_ms,
            predictive_feed_url,
            predictive_feed_token,
            predictive_feed_batch_size,
            predictive_feed_flush_ms,
            predictive_feed_queue,
            otlp_endpoint,
        };

        if let Some(overrides) = setup_overrides.as_ref() {
            let allow_mqtt_port_override = std::env::var("SIDECAR_MQTT_PORT")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .is_none();
            let allow_offline_threshold_override =
                std::env::var("SIDECAR_OFFLINE_THRESHOLD_SECONDS")
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .is_none();
            apply_setup_overrides(
                &mut config,
                overrides,
                allow_mqtt_port_override,
                allow_offline_threshold_override,
            );
        }

        Ok(config)
    }

    pub fn flush_interval(&self) -> Duration {
        Duration::from_millis(self.flush_interval_ms)
    }

    pub fn mqtt_keepalive(&self) -> Duration {
        Duration::from_secs(self.mqtt_keepalive_secs)
    }

    pub fn offline_threshold(&self) -> Duration {
        Duration::from_secs(self.offline_threshold_seconds)
    }

    pub fn status_poll_interval(&self) -> Duration {
        Duration::from_millis(self.status_poll_interval_ms)
    }

    pub fn predictive_feed_flush_interval(&self) -> Duration {
        Duration::from_millis(self.predictive_feed_flush_ms)
    }
}

fn normalize_database_url(url: String) -> String {
    if let Some(stripped) = url.strip_prefix("postgresql+psycopg://") {
        return format!("postgresql://{stripped}");
    }
    if let Some(stripped) = url.strip_prefix("postgresql+asyncpg://") {
        return format!("postgresql://{stripped}");
    }
    url
}
