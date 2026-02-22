use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Component, Path, PathBuf};

const DEFAULT_SETUP_CONFIG_PATH: &str = "/Users/Shared/FarmDashboard/setup/config.json";

pub(crate) fn setup_config_path() -> PathBuf {
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
    data_root: Option<String>,
    #[serde(default)]
    backup_root: Option<String>,
    #[serde(default)]
    backup_retention_days: Option<u32>,
    #[serde(default)]
    map_storage_path: Option<String>,
    #[serde(default)]
    enable_analytics_feeds: Option<bool>,
    #[serde(default)]
    enable_forecast_ingestion: Option<bool>,
    #[serde(default)]
    analytics_feed_poll_interval_seconds: Option<u64>,
    #[serde(default)]
    forecast_poll_interval_seconds: Option<u64>,
    #[serde(default)]
    schedule_poll_interval_seconds: Option<u64>,
    #[serde(default)]
    enable_external_devices: Option<bool>,
    #[serde(default)]
    external_device_poll_interval_seconds: Option<u64>,
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
                "failed to read setup config; using env defaults"
            );
            return None;
        }
    };
    match serde_json::from_str(&contents) {
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "failed to parse setup config; using env defaults"
            );
            None
        }
    }
}

fn apply_setup_overrides(
    config: &mut CoreConfig,
    overrides: &SetupConfigOverrides,
    allow_mqtt_port_override: bool,
) {
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

    if let Some(path) = overrides
        .backup_root
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        config.backup_storage_path = PathBuf::from(path);
    }
    if let Some(days) = overrides.backup_retention_days.filter(|v| *v != 0) {
        config.backup_retention_days = days;
    }
    if let Some(path) = overrides
        .map_storage_path
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        config.map_storage_path = PathBuf::from(path);
    }

    if let Some(enabled) = overrides.enable_analytics_feeds {
        config.enable_analytics_feeds = enabled;
    }
    if let Some(enabled) = overrides.enable_forecast_ingestion {
        config.enable_forecast_ingestion = enabled;
    }
    if let Some(value) = overrides
        .analytics_feed_poll_interval_seconds
        .filter(|v| *v != 0)
    {
        config.analytics_feed_poll_interval_seconds = value;
    }
    if let Some(value) = overrides.forecast_poll_interval_seconds.filter(|v| *v != 0) {
        config.forecast_poll_interval_seconds = value;
    }
    if let Some(value) = overrides.schedule_poll_interval_seconds.filter(|v| *v != 0) {
        config.schedule_poll_interval_seconds = value;
    }
    if let Some(enabled) = overrides.enable_external_devices {
        config.enable_external_devices = enabled;
    }
    if let Some(value) = overrides
        .external_device_poll_interval_seconds
        .filter(|v| *v != 0)
    {
        config.external_device_poll_interval_seconds = value;
    }
}

#[derive(Debug, Clone)]
pub struct CoreConfig {
    pub database_url: String,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_username: Option<String>,
    pub mqtt_password: Option<String>,
    pub static_root: Option<PathBuf>,
    pub setup_daemon_base_url: Option<String>,
    pub data_root: PathBuf,
    pub backup_storage_path: PathBuf,
    pub backup_retention_days: u32,
    pub map_storage_path: PathBuf,
    pub node_agent_port: u16,
    pub node_agent_overlay_path: PathBuf,
    pub ssh_known_hosts_path: PathBuf,
    pub demo_mode: bool,
    pub enable_analytics_feeds: bool,
    pub enable_forecast_ingestion: bool,
    pub analytics_feed_poll_interval_seconds: u64,
    pub forecast_poll_interval_seconds: u64,
    pub schedule_poll_interval_seconds: u64,
    pub enable_external_devices: bool,
    pub external_device_poll_interval_seconds: u64,
    pub forecast_api_base_url: Option<String>,
    pub forecast_api_path: Option<String>,
    pub rates_api_base_url: Option<String>,
    pub rates_api_path: Option<String>,
    pub analysis_max_concurrent_jobs: usize,
    pub analysis_poll_interval_ms: u64,
    pub analysis_lake_hot_path: PathBuf,
    pub analysis_lake_cold_path: Option<PathBuf>,
    pub analysis_tmp_path: PathBuf,
    pub analysis_lake_shards: u32,
    pub analysis_hot_retention_days: u32,
    pub analysis_late_window_hours: u32,
    pub analysis_replication_interval_seconds: u64,
    pub analysis_replication_lag_seconds: u64,
    pub analysis_max_jobs_per_user: usize,
    pub analysis_preview_max_window_seconds: u64,
    pub analysis_embeddings_refresh_enabled: bool,
    pub analysis_embeddings_refresh_interval_seconds: u64,
    pub analysis_embeddings_refresh_horizon_days: i64,
    pub analysis_embeddings_full_rebuild_interval_hours: u64,
    pub analysis_embeddings_full_rebuild_horizon_days: i64,
    pub analysis_profile_enabled: bool,
    pub analysis_profile_output_path: PathBuf,
    pub qdrant_url: String,
}

impl CoreConfig {
    pub fn from_env(cli_static_root: Option<PathBuf>) -> Result<Self> {
        let setup_overrides = load_setup_config_overrides();

        let database_url = std::env::var("CORE_DATABASE_URL")
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
            .context("CORE_DATABASE_URL must be set for the controller runtime (or present as database_url in the setup config)")?;
        let database_url = normalize_database_url(database_url);
        if database_url.trim().is_empty() {
            anyhow::bail!("CORE_DATABASE_URL resolved to an empty value");
        }
        let mqtt_host = env_string("CORE_MQTT_HOST", "127.0.0.1");
        let mqtt_port = env_u16("CORE_MQTT_PORT", 1883);
        let mqtt_username = env_optional_string("CORE_MQTT_USERNAME");
        let mqtt_password = env_optional_string("CORE_MQTT_PASSWORD");
        let static_root = cli_static_root.or_else(|| env_optional_path("CORE_STATIC_ROOT"));
        let setup_daemon_base_url = env_optional_string("CORE_SETUP_DAEMON_BASE_URL");
        let data_root_value = env_optional_string("CORE_DATA_ROOT")
            .or_else(|| {
                setup_overrides
                    .as_ref()
                    .and_then(|ov| ov.data_root.as_deref())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string())
            })
            .unwrap_or_else(|| "/Users/Shared/FarmDashboard".to_string());
        let data_root = PathBuf::from(data_root_value);
        if data_root.as_os_str().is_empty() {
            anyhow::bail!("CORE_DATA_ROOT resolved to an empty path");
        }
        let backup_default = data_root.join("storage/backups");
        let map_default = data_root.join("storage/map");
        let ssh_default = data_root.join("storage/ssh/known_hosts");
        let analysis_hot_default = data_root.join("storage/analysis/lake/hot");
        let analysis_tmp_default = data_root.join("storage/analysis/tmp");
        let analysis_profile_default = analysis_tmp_default.join("profiles");

        let backup_storage_path = env_path(
            "CORE_BACKUP_STORAGE_PATH",
            &backup_default.to_string_lossy(),
        )?;
        let map_storage_path = env_path("CORE_MAP_STORAGE_PATH", &map_default.to_string_lossy())?;
        let backup_retention_days = env_u32("CORE_BACKUP_RETENTION_DAYS", 30);
        let node_agent_port = env_u16("CORE_NODE_AGENT_PORT", 9000);
        let node_agent_overlay_path = env_path(
            "CORE_NODE_AGENT_OVERLAY_PATH",
            "/usr/local/farm-dashboard/artifacts/node-agent/node-agent-overlay.tar.gz",
        )?;
        let ssh_known_hosts_path =
            env_path("CORE_SSH_KNOWN_HOSTS_PATH", &ssh_default.to_string_lossy())?;

        let demo_mode = env_bool("CORE_DEMO_MODE", false);
        let enable_analytics_feeds = env_bool("CORE_ENABLE_ANALYTICS_FEEDS", true);
        let enable_forecast_ingestion = env_bool("CORE_ENABLE_FORECAST_INGESTION", true);
        let schedule_poll_interval_seconds = env_u64("CORE_SCHEDULE_POLL_INTERVAL_SECONDS", 15);
        let enable_external_devices = env_bool("CORE_ENABLE_EXTERNAL_DEVICES", true);
        let external_device_poll_interval_seconds =
            env_u64("CORE_EXTERNAL_DEVICE_POLL_INTERVAL_SECONDS", 10);
        let analytics_feed_poll_interval_seconds =
            env_u64("CORE_ANALYTICS_FEED_POLL_INTERVAL_SECONDS", 300);
        let forecast_poll_interval_seconds = env_u64("CORE_FORECAST_POLL_INTERVAL_SECONDS", 3600);

        let forecast_api_base_url = env_optional_string("CORE_FORECAST_API_BASE_URL");
        let forecast_api_path = env_optional_string("CORE_FORECAST_API_PATH");
        let rates_api_base_url = env_optional_string("CORE_ANALYTICS_RATES__API_BASE_URL");
        let rates_api_path = env_optional_string("CORE_ANALYTICS_RATES__API_PATH");
        let analysis_max_concurrent_jobs =
            env_u64("CORE_ANALYSIS_MAX_CONCURRENT_JOBS", 2).clamp(1, 16) as usize;
        let analysis_poll_interval_ms =
            env_u64("CORE_ANALYSIS_POLL_INTERVAL_MS", 500).clamp(50, 10_000);
        let analysis_lake_hot_path = env_path(
            "CORE_ANALYSIS_LAKE_HOT_PATH",
            &analysis_hot_default.to_string_lossy(),
        )?;
        let analysis_lake_cold_path = env_optional_path("CORE_ANALYSIS_LAKE_COLD_PATH");
        let analysis_tmp_path = env_path(
            "CORE_ANALYSIS_TMP_PATH",
            &analysis_tmp_default.to_string_lossy(),
        )?;
        let analysis_lake_shards = env_u32("CORE_ANALYSIS_LAKE_SHARDS", 16).max(1);
        let analysis_hot_retention_days = env_u32("CORE_ANALYSIS_HOT_RETENTION_DAYS", 90).max(1);
        let analysis_late_window_hours = env_u32("CORE_ANALYSIS_LATE_WINDOW_HOURS", 48).max(1);
        let analysis_replication_interval_seconds =
            env_u64("CORE_ANALYSIS_REPLICATION_INTERVAL_SECONDS", 60).clamp(10, 3600);
        let analysis_replication_lag_seconds =
            env_u64("CORE_ANALYSIS_REPLICATION_LAG_SECONDS", 60).clamp(0, 3600);
        let analysis_max_jobs_per_user =
            env_u64("CORE_ANALYSIS_MAX_JOBS_PER_USER", 3).clamp(1, 25) as usize;
        let analysis_preview_max_window_seconds =
            env_u64("CORE_ANALYSIS_PREVIEW_MAX_WINDOW_HOURS", 168).saturating_mul(3600);
        let analysis_embeddings_refresh_enabled =
            env_bool("CORE_ANALYSIS_EMBEDDINGS_REFRESH_ENABLED", true);
        let analysis_embeddings_refresh_interval_seconds =
            env_u64("CORE_ANALYSIS_EMBEDDINGS_REFRESH_INTERVAL_SECONDS", 21_600)
                .clamp(300, 7 * 24 * 3600);
        let analysis_embeddings_refresh_horizon_days =
            env_u64("CORE_ANALYSIS_EMBEDDINGS_REFRESH_HORIZON_DAYS", 30).clamp(1, 365) as i64;
        let analysis_embeddings_full_rebuild_interval_hours =
            env_u64("CORE_ANALYSIS_EMBEDDINGS_FULL_REBUILD_INTERVAL_HOURS", 168).clamp(1, 24 * 30);
        let analysis_embeddings_full_rebuild_horizon_days =
            env_u64("CORE_ANALYSIS_EMBEDDINGS_FULL_REBUILD_HORIZON_DAYS", 365).clamp(7, 3650)
                as i64;
        let analysis_profile_enabled = env_bool("CORE_ANALYSIS_PROFILE_ENABLED", false);
        let analysis_profile_output_path = env_path(
            "CORE_ANALYSIS_PROFILE_OUTPUT_PATH",
            &analysis_profile_default.to_string_lossy(),
        )?;
        let qdrant_url = env_string("CORE_QDRANT_URL", "http://127.0.0.1:6333");

        let mut config = Self {
            database_url,
            mqtt_host,
            mqtt_port,
            mqtt_username,
            mqtt_password,
            static_root,
            setup_daemon_base_url,
            data_root,
            backup_storage_path,
            backup_retention_days,
            map_storage_path,
            node_agent_port,
            node_agent_overlay_path,
            ssh_known_hosts_path,
            demo_mode,
            enable_analytics_feeds,
            enable_forecast_ingestion,
            analytics_feed_poll_interval_seconds,
            forecast_poll_interval_seconds,
            schedule_poll_interval_seconds,
            enable_external_devices,
            external_device_poll_interval_seconds,
            forecast_api_base_url,
            forecast_api_path,
            rates_api_base_url,
            rates_api_path,
            analysis_max_concurrent_jobs,
            analysis_poll_interval_ms,
            analysis_lake_hot_path,
            analysis_lake_cold_path,
            analysis_tmp_path,
            analysis_lake_shards,
            analysis_hot_retention_days,
            analysis_late_window_hours,
            analysis_replication_interval_seconds,
            analysis_replication_lag_seconds,
            analysis_max_jobs_per_user,
            analysis_preview_max_window_seconds,
            analysis_embeddings_refresh_enabled,
            analysis_embeddings_refresh_interval_seconds,
            analysis_embeddings_refresh_horizon_days,
            analysis_embeddings_full_rebuild_interval_hours,
            analysis_embeddings_full_rebuild_horizon_days,
            analysis_profile_enabled,
            analysis_profile_output_path,
            qdrant_url,
        };

        if let Some(overrides) = setup_overrides.as_ref() {
            let allow_mqtt_port_override = env_optional_string("CORE_MQTT_PORT").is_none();
            apply_setup_overrides(&mut config, overrides, allow_mqtt_port_override);
        }

        config.validate_security_paths()?;

        Ok(config)
    }
}

impl CoreConfig {
    fn validate_security_paths(&mut self) -> Result<()> {
        self.data_root =
            validate_and_canonicalize_path(self.data_root.clone(), None, "CORE_DATA_ROOT")?;
        self.analysis_lake_hot_path = validate_and_canonicalize_path(
            self.analysis_lake_hot_path.clone(),
            Some(&self.data_root),
            "CORE_ANALYSIS_LAKE_HOT_PATH",
        )?;
        if let Some(cold) = self.analysis_lake_cold_path.clone() {
            self.analysis_lake_cold_path = Some(validate_and_canonicalize_path(
                cold,
                Some(&self.data_root),
                "CORE_ANALYSIS_LAKE_COLD_PATH",
            )?);
        }
        self.analysis_tmp_path = validate_and_canonicalize_path(
            self.analysis_tmp_path.clone(),
            Some(&self.data_root),
            "CORE_ANALYSIS_TMP_PATH",
        )?;
        self.analysis_profile_output_path = validate_and_canonicalize_path(
            self.analysis_profile_output_path.clone(),
            Some(&self.data_root),
            "CORE_ANALYSIS_PROFILE_OUTPUT_PATH",
        )?;
        Ok(())
    }
}

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_optional_string(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key)
        .ok()
        .map(|value| value.trim().to_lowercase())
    {
        Some(value) if value == "1" || value == "true" || value == "yes" => true,
        Some(value) if value == "0" || value == "false" || value == "no" => false,
        _ => default,
    }
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .unwrap_or(default)
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_optional_path(key: &str) -> Option<PathBuf> {
    env_optional_string(key).map(PathBuf::from)
}

fn env_path(key: &str, default: &str) -> Result<PathBuf> {
    let value = env_optional_string(key).unwrap_or_else(|| default.to_string());
    let path = PathBuf::from(value);
    if path.as_os_str().is_empty() {
        anyhow::bail!("{key} resolved to an empty path");
    }
    Ok(path)
}

fn validate_and_canonicalize_path(
    path: PathBuf,
    base: Option<&Path>,
    label: &str,
) -> Result<PathBuf> {
    if !path.is_absolute() {
        anyhow::bail!("{label} must be an absolute path");
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            anyhow::bail!("{label} must not contain '..' segments");
        }
    }
    let canonical = canonicalize_with_existing_parent(&path)
        .with_context(|| format!("failed to canonicalize {label} ({})", path.display()))?;
    if let Some(base) = base {
        let base = canonicalize_with_existing_parent(base)
            .with_context(|| format!("failed to canonicalize base for {label}"))?;
        if !canonical.starts_with(&base) {
            anyhow::bail!("{label} must reside under {}", base.display());
        }
    }
    Ok(canonical)
}

fn canonicalize_with_existing_parent(path: &Path) -> Result<PathBuf> {
    let mut existing = None;
    for ancestor in path.ancestors() {
        if ancestor.exists() {
            existing = Some(ancestor);
            break;
        }
    }
    let Some(existing) = existing else {
        anyhow::bail!("no existing ancestor found for path {}", path.display());
    };
    let base = existing
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", existing.display()))?;
    let suffix = path.strip_prefix(existing).unwrap_or(Path::new(""));
    Ok(base.join(suffix))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_config(data_root: PathBuf) -> CoreConfig {
        CoreConfig {
            database_url: "postgresql://postgres@localhost/postgres".to_string(),
            mqtt_host: "127.0.0.1".to_string(),
            mqtt_port: 1883,
            mqtt_username: None,
            mqtt_password: None,
            static_root: None,
            setup_daemon_base_url: None,
            backup_storage_path: data_root.join("storage/backups"),
            backup_retention_days: 30,
            map_storage_path: data_root.join("storage/map"),
            node_agent_port: 9000,
            node_agent_overlay_path: PathBuf::from("/tmp/overlay.tar.gz"),
            ssh_known_hosts_path: data_root.join("storage/ssh/known_hosts"),
            demo_mode: true,
            enable_analytics_feeds: false,
            enable_forecast_ingestion: false,
            analytics_feed_poll_interval_seconds: 300,
            forecast_poll_interval_seconds: 3600,
            schedule_poll_interval_seconds: 15,
            enable_external_devices: false,
            external_device_poll_interval_seconds: 30,
            forecast_api_base_url: None,
            forecast_api_path: None,
            rates_api_base_url: None,
            rates_api_path: None,
            analysis_max_concurrent_jobs: 1,
            analysis_poll_interval_ms: 500,
            analysis_lake_hot_path: data_root.join("storage/analysis/lake/hot"),
            analysis_lake_cold_path: None,
            analysis_tmp_path: data_root.join("storage/analysis/tmp"),
            analysis_lake_shards: 4,
            analysis_hot_retention_days: 90,
            analysis_late_window_hours: 48,
            analysis_replication_interval_seconds: 60,
            analysis_replication_lag_seconds: 300,
            analysis_max_jobs_per_user: 3,
            analysis_preview_max_window_seconds: 7 * 24 * 3600,
            analysis_embeddings_refresh_enabled: true,
            analysis_embeddings_refresh_interval_seconds: 21_600,
            analysis_embeddings_refresh_horizon_days: 30,
            analysis_embeddings_full_rebuild_interval_hours: 168,
            analysis_embeddings_full_rebuild_horizon_days: 365,
            analysis_profile_enabled: false,
            analysis_profile_output_path: data_root.join("storage/analysis/tmp/profiles"),
            qdrant_url: "http://127.0.0.1:6333".to_string(),
            data_root,
        }
    }

    #[test]
    fn rejects_relative_or_parent_paths() {
        let err = validate_and_canonicalize_path(PathBuf::from("relative/path"), None, "TEST");
        assert!(err.is_err());

        let err = validate_and_canonicalize_path(PathBuf::from("/tmp/../etc"), None, "TEST");
        assert!(err.is_err());
    }

    #[test]
    fn rejects_paths_outside_base() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let base = temp.path().join("base");
        let other = temp.path().join("other");
        std::fs::create_dir_all(&base)?;
        std::fs::create_dir_all(&other)?;

        let err = validate_and_canonicalize_path(other.clone(), Some(&base), "TEST");
        assert!(err.is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() -> Result<()> {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir()?;
        let base = temp.path().join("base");
        let external = temp.path().join("external");
        std::fs::create_dir_all(&base)?;
        std::fs::create_dir_all(&external)?;

        let link = base.join("analysis");
        symlink(&external, &link)?;

        let candidate = link.join("lake");
        let err = validate_and_canonicalize_path(candidate, Some(&base), "TEST");
        assert!(err.is_err());
        Ok(())
    }

    #[test]
    fn analysis_paths_must_reside_under_data_root() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let base = temp.path().join("base");
        let other = temp.path().join("other");
        std::fs::create_dir_all(&base)?;
        std::fs::create_dir_all(&other)?;

        let mut config = minimal_config(base.clone());
        config.analysis_lake_hot_path = other.join("lake/hot");

        let err = config.validate_security_paths();
        assert!(err.is_err());
        Ok(())
    }
}
