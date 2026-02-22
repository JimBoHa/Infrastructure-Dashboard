use anyhow::{bail, Context, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::constants::{
    DEFAULT_QDRANT_PORT, DEFAULT_REDIS_PORT, DEFAULT_SETUP_PORT, DEFAULT_STATE_DIR,
};
use crate::profile::InstallProfile;
use crate::utils::{allocate_local_port, which};

pub fn setup_state_dir() -> PathBuf {
    std::env::var("FARM_SETUP_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_STATE_DIR))
}

pub fn default_config_path() -> PathBuf {
    setup_state_dir().join("config.json")
}

pub fn resolve_config_path(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(default_config_path)
}

fn default_install_root() -> String {
    "/usr/local/farm-dashboard".to_string()
}

fn default_data_root() -> String {
    "/Users/Shared/FarmDashboard".to_string()
}

fn default_logs_root() -> String {
    "/Users/Shared/FarmDashboard/logs".to_string()
}

fn default_core_binary() -> String {
    "/usr/local/farm-dashboard/bin/core-server".to_string()
}

fn default_sidecar_binary() -> String {
    "/usr/local/farm-dashboard/bin/telemetry-sidecar".to_string()
}

fn default_mqtt_host() -> String {
    "127.0.0.1".to_string()
}

fn default_farmctl_path() -> String {
    "farmctl".to_string()
}

fn default_database_url() -> String {
    // Use the psycopg3 driver explicitly so Python tooling (sim-lab, parity harnesses)
    // does not implicitly fall back to psycopg2. Rust services normalize this to a
    // plain `postgresql://` URL where required (sqlx/libpq tooling).
    "postgresql+psycopg://postgres@127.0.0.1:5432/iot".to_string()
}

fn generate_database_password() -> Result<String> {
    let mut bytes = [0u8; 24];
    rand::rngs::OsRng
        .try_fill_bytes(&mut bytes)
        .context("failed to generate random database password")?;
    Ok(hex::encode(bytes))
}

fn parse_database_url(database_url: &str) -> (String, u16, String, Option<String>) {
    let mut host = "127.0.0.1".to_string();
    let mut port = 5432;
    let mut db_name = "iot".to_string();
    let mut password: Option<String> = None;

    let trimmed = database_url
        .split('?')
        .next()
        .unwrap_or(database_url)
        .trim();
    if trimmed.is_empty() {
        return (host, port, db_name, password);
    }

    let after_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);

    let (creds, host_and_path) = after_scheme
        .rsplit_once('@')
        .map(|(lhs, rhs)| (Some(lhs), rhs))
        .unwrap_or((None, after_scheme));

    if let Some(creds) = creds {
        if let Some((_, pass)) = creds.split_once(':') {
            let pass = pass.trim();
            if !pass.is_empty() {
                password = Some(pass.to_string());
            }
        }
    }

    let (host_port, path) = host_and_path
        .split_once('/')
        .map(|(lhs, rhs)| (lhs, Some(rhs)))
        .unwrap_or((host_and_path, None));

    if let Some((maybe_host, maybe_port)) = host_port.rsplit_once(':') {
        if let Ok(parsed) = maybe_port.parse::<u16>() {
            port = parsed;
        }
        if !maybe_host.trim().is_empty() {
            host = maybe_host.trim().to_string();
        }
    } else if !host_port.trim().is_empty() {
        host = host_port.trim().to_string();
    }

    if let Some(path) = path {
        let path = path.trim();
        if !path.is_empty() {
            db_name = path.to_string();
        }
    }

    (host, port, db_name, password)
}

pub fn database_password(database_url: &str) -> Option<String> {
    parse_database_url(database_url).3
}

fn build_database_url(host: &str, port: u16, db_name: &str, password: &str) -> String {
    format!("postgresql+psycopg://postgres:{password}@{host}:{port}/{db_name}")
}

fn ensure_database_url(config: &mut SetupConfig) -> Result<bool> {
    let (host, port, db_name, password) = parse_database_url(&config.database_url);
    let password = match password {
        Some(value) if !value.trim().is_empty() && value.trim() != "postgres" => value,
        _ => generate_database_password()?,
    };

    let rebuilt = build_database_url(&host, port, &db_name, &password);
    if config.database_url != rebuilt {
        config.database_url = rebuilt;
        return Ok(true);
    }
    Ok(false)
}

fn default_backup_root() -> String {
    "/Users/Shared/FarmDashboard/storage/backups".to_string()
}

fn default_enable_analytics_feeds() -> bool {
    true
}

fn default_enable_forecast_ingestion() -> bool {
    true
}

fn default_analytics_feed_poll_interval_seconds() -> u64 {
    300
}

fn default_forecast_poll_interval_seconds() -> u64 {
    3600
}

fn default_schedule_poll_interval_seconds() -> u64 {
    15
}

fn default_qdrant_port() -> u16 {
    DEFAULT_QDRANT_PORT
}

fn default_service_user() -> String {
    "_farmdashboard".to_string()
}

fn default_service_group() -> String {
    "_farmdashboard".to_string()
}

fn default_setup_port() -> u16 {
    DEFAULT_SETUP_PORT
}

fn default_launchd_label_prefix() -> String {
    "com.farmdashboard".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupConfig {
    #[serde(default = "default_install_root")]
    pub install_root: String,
    #[serde(default = "default_data_root")]
    pub data_root: String,
    #[serde(default = "default_logs_root")]
    pub logs_root: String,
    #[serde(default = "default_core_binary")]
    pub core_binary: String,
    #[serde(default = "default_sidecar_binary")]
    pub sidecar_binary: String,
    #[serde(default)]
    pub core_port: u16,
    #[serde(default = "default_mqtt_host")]
    pub mqtt_host: String,
    #[serde(default)]
    pub mqtt_port: u16,
    #[serde(default)]
    pub mqtt_username: Option<String>,
    #[serde(default)]
    pub mqtt_password: Option<String>,
    #[serde(default)]
    pub redis_port: u16,
    #[serde(default = "default_qdrant_port")]
    pub qdrant_port: u16,
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_backup_root")]
    pub backup_root: String,
    #[serde(default)]
    pub backup_retention_days: u32,
    #[serde(default = "default_service_user")]
    pub service_user: String,
    #[serde(default = "default_service_group")]
    pub service_group: String,
    #[serde(default)]
    pub bundle_path: Option<String>,
    #[serde(default = "default_farmctl_path")]
    pub farmctl_path: String,
    #[serde(default)]
    pub profile: InstallProfile,
    #[serde(default = "default_launchd_label_prefix")]
    pub launchd_label_prefix: String,
    #[serde(default = "default_setup_port")]
    pub setup_port: u16,
    #[serde(default = "default_enable_analytics_feeds")]
    pub enable_analytics_feeds: bool,
    #[serde(default = "default_enable_forecast_ingestion")]
    pub enable_forecast_ingestion: bool,
    #[serde(default = "default_analytics_feed_poll_interval_seconds")]
    pub analytics_feed_poll_interval_seconds: u64,
    #[serde(default = "default_forecast_poll_interval_seconds")]
    pub forecast_poll_interval_seconds: u64,
    #[serde(default = "default_schedule_poll_interval_seconds")]
    pub schedule_poll_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SetupConfigPatch {
    pub install_root: Option<String>,
    pub data_root: Option<String>,
    pub logs_root: Option<String>,
    pub core_binary: Option<String>,
    pub sidecar_binary: Option<String>,
    pub core_port: Option<u16>,
    pub mqtt_host: Option<String>,
    pub mqtt_port: Option<u16>,
    pub mqtt_username: Option<String>,
    pub mqtt_password: Option<String>,
    pub redis_port: Option<u16>,
    pub qdrant_port: Option<u16>,
    pub database_url: Option<String>,
    pub backup_root: Option<String>,
    pub backup_retention_days: Option<u32>,
    pub service_user: Option<String>,
    pub service_group: Option<String>,
    pub bundle_path: Option<String>,
    pub farmctl_path: Option<String>,
    pub profile: Option<InstallProfile>,
    pub launchd_label_prefix: Option<String>,
    pub setup_port: Option<u16>,
    pub enable_analytics_feeds: Option<bool>,
    pub enable_forecast_ingestion: Option<bool>,
    pub analytics_feed_poll_interval_seconds: Option<u64>,
    pub forecast_poll_interval_seconds: Option<u64>,
    pub schedule_poll_interval_seconds: Option<u64>,
}

pub fn default_config() -> Result<SetupConfig> {
    let mut config = SetupConfig {
        install_root: default_install_root(),
        data_root: default_data_root(),
        logs_root: default_logs_root(),
        core_binary: default_core_binary(),
        sidecar_binary: default_sidecar_binary(),
        core_port: 8000,
        mqtt_host: default_mqtt_host(),
        mqtt_port: 1883,
        mqtt_username: None,
        mqtt_password: None,
        redis_port: DEFAULT_REDIS_PORT,
        qdrant_port: DEFAULT_QDRANT_PORT,
        database_url: default_database_url(),
        backup_root: default_backup_root(),
        backup_retention_days: 30,
        service_user: default_service_user(),
        service_group: default_service_group(),
        bundle_path: None,
        farmctl_path: default_farmctl_path(),
        profile: InstallProfile::Prod,
        launchd_label_prefix: default_launchd_label_prefix(),
        setup_port: DEFAULT_SETUP_PORT,
        enable_analytics_feeds: default_enable_analytics_feeds(),
        enable_forecast_ingestion: default_enable_forecast_ingestion(),
        analytics_feed_poll_interval_seconds: default_analytics_feed_poll_interval_seconds(),
        forecast_poll_interval_seconds: default_forecast_poll_interval_seconds(),
        schedule_poll_interval_seconds: default_schedule_poll_interval_seconds(),
    };
    let _ = ensure_database_url(&mut config)?;
    Ok(config)
}

pub fn load_config(path: &Path) -> Result<SetupConfig> {
    if !path.exists() {
        let config = default_config()?;
        save_config(path, &config)?;
        return Ok(config);
    }
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config at {}", path.display()))?;
    let mut config: SetupConfig = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse config at {}", path.display()))?;
    if config.core_port == 0 {
        config.core_port = 8000;
    }
    if config.mqtt_port == 0 {
        config.mqtt_port = 1883;
    }
    if config.redis_port == 0 {
        config.redis_port = DEFAULT_REDIS_PORT;
    }
    if config.qdrant_port == 0 {
        config.qdrant_port = DEFAULT_QDRANT_PORT;
    }
    if config.analytics_feed_poll_interval_seconds == 0 {
        config.analytics_feed_poll_interval_seconds =
            default_analytics_feed_poll_interval_seconds();
    }
    if config.forecast_poll_interval_seconds == 0 {
        config.forecast_poll_interval_seconds = default_forecast_poll_interval_seconds();
    }
    if config.schedule_poll_interval_seconds == 0 {
        config.schedule_poll_interval_seconds = default_schedule_poll_interval_seconds();
    }
    if config.backup_retention_days == 0 {
        config.backup_retention_days = 30;
    }
    if config.service_user.trim().is_empty() {
        config.service_user = default_service_user();
    }
    if config.service_group.trim().is_empty() {
        config.service_group = default_service_group();
    }
    if config.farmctl_path.trim().is_empty() {
        config.farmctl_path = default_farmctl_path();
    }
    if config.launchd_label_prefix.trim().is_empty() {
        config.launchd_label_prefix = default_launchd_label_prefix();
    }
    if config.setup_port == 0 {
        config.setup_port = DEFAULT_SETUP_PORT;
    }

    let changed = ensure_database_url(&mut config)?;
    if changed {
        save_config(path, &config)?;
    }
    Ok(config)
}

pub fn save_config(path: &Path, config: &SetupConfig) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create config dir {}", parent.display()))?;
    }
    let contents = serde_json::to_string_pretty(config)?;
    fs::write(path, contents)
        .with_context(|| format!("Failed to write config at {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum SaveConfigMode {
    Strict,
    BestEffort,
}

pub fn save_config_if_changed(
    path: &Path,
    before: &SetupConfig,
    after: &SetupConfig,
    mode: SaveConfigMode,
) -> Result<bool> {
    if before == after {
        return Ok(false);
    }
    match save_config(path, after) {
        Ok(()) => Ok(true),
        Err(err) => {
            if matches!(mode, SaveConfigMode::BestEffort) && is_permission_denied(&err) {
                return Ok(false);
            }
            Err(err)
        }
    }
}

fn is_permission_denied(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|io_err| io_err.kind() == io::ErrorKind::PermissionDenied)
    })
}

pub fn patch_config(path: &Path, patch: SetupConfigPatch) -> Result<SetupConfig> {
    let mut config = load_config(path)?;
    if let Some(value) = patch.install_root {
        config.install_root = value;
    }
    if let Some(value) = patch.data_root {
        config.data_root = value;
    }
    if let Some(value) = patch.logs_root {
        config.logs_root = value;
    }
    if let Some(value) = patch.core_binary {
        config.core_binary = value;
    }
    if let Some(value) = patch.sidecar_binary {
        config.sidecar_binary = value;
    }
    if let Some(value) = patch.core_port {
        config.core_port = value;
    }
    if let Some(value) = patch.mqtt_host {
        config.mqtt_host = value;
    }
    if let Some(value) = patch.mqtt_port {
        config.mqtt_port = value;
    }
    if let Some(value) = patch.mqtt_username {
        let trimmed = value.trim().to_string();
        config.mqtt_username = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }
    if let Some(value) = patch.mqtt_password {
        let trimmed = value.trim().to_string();
        config.mqtt_password = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }
    if let Some(value) = patch.redis_port {
        config.redis_port = value;
    }
    if let Some(value) = patch.qdrant_port {
        config.qdrant_port = value;
    }
    if let Some(value) = patch.database_url {
        config.database_url = value;
    }
    if let Some(value) = patch.backup_root {
        config.backup_root = value;
    }
    if let Some(value) = patch.backup_retention_days {
        config.backup_retention_days = value;
    }
    if let Some(value) = patch.service_user {
        config.service_user = value;
    }
    if let Some(value) = patch.service_group {
        config.service_group = value;
    }
    if let Some(value) = patch.bundle_path {
        if !value.is_empty() {
            validate_bundle_path(Path::new(&value))?;
            config.bundle_path = Some(value);
        }
    }
    if let Some(value) = patch.farmctl_path {
        config.farmctl_path = value;
    }
    if let Some(value) = patch.profile {
        config.profile = value;
    }
    if let Some(value) = patch.launchd_label_prefix {
        config.launchd_label_prefix = value;
    }
    if let Some(value) = patch.setup_port {
        config.setup_port = value;
    }
    if let Some(value) = patch.enable_analytics_feeds {
        config.enable_analytics_feeds = value;
    }
    if let Some(value) = patch.enable_forecast_ingestion {
        config.enable_forecast_ingestion = value;
    }
    if let Some(value) = patch.analytics_feed_poll_interval_seconds {
        config.analytics_feed_poll_interval_seconds = value;
    }
    if let Some(value) = patch.forecast_poll_interval_seconds {
        config.forecast_poll_interval_seconds = value;
    }
    if let Some(value) = patch.schedule_poll_interval_seconds {
        config.schedule_poll_interval_seconds = value;
    }
    normalize_config(&mut config, None)?;
    save_config(path, &config)?;
    Ok(config)
}

pub fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

pub fn postgres_connection_string(database_url: &str) -> String {
    let (scheme, rest) = match database_url.split_once("://") {
        Some(parts) => parts,
        None => return database_url.to_string(),
    };
    let (base, _driver) = match scheme.split_once('+') {
        Some(parts) => parts,
        None => return database_url.to_string(),
    };
    if base != "postgresql" {
        return database_url.to_string();
    }
    format!("postgresql://{rest}")
}

pub fn auto_detect_config(config: &mut SetupConfig) -> Result<()> {
    let farmctl_path = Path::new(&config.farmctl_path);
    let farmctl_exists = farmctl_path.exists()
        || (!config.farmctl_path.contains('/') && which(&config.farmctl_path).is_some());
    if config.farmctl_path.trim().is_empty()
        || config.farmctl_path == default_farmctl_path()
        || !farmctl_exists
    {
        if let Ok(exe) = std::env::current_exe() {
            config.farmctl_path = exe.display().to_string();
        }
    }
    if config.bundle_path.is_none() {
        if let Ok(path) = std::env::var("FARM_SETUP_BUNDLE_PATH") {
            if !path.trim().is_empty() {
                config.bundle_path = Some(path);
            }
        } else if let Some(path) = detect_bundle_path() {
            config.bundle_path = Some(path.display().to_string());
        }
    }
    Ok(())
}

pub fn normalize_config(
    config: &mut SetupConfig,
    profile_override: Option<InstallProfile>,
) -> Result<()> {
    if let Some(profile) = profile_override {
        config.profile = profile;
    }
    auto_detect_config(config)?;
    let _ = ensure_database_url(config)?;
    apply_profile_defaults(config)?;
    Ok(())
}

fn apply_profile_defaults(config: &mut SetupConfig) -> Result<()> {
    if config.launchd_label_prefix.trim().is_empty() {
        config.launchd_label_prefix = default_launchd_label_prefix();
    }
    if config.setup_port == 0 {
        config.setup_port = DEFAULT_SETUP_PORT;
    }

    // In production, nodes and non-controller clients must be able to reach the controller's MQTT
    // broker. Defaulting to localhost breaks remote nodes, so we auto-detect a best-guess LAN IP.
    // E2E stays loopback-only and uses isolated random ports.
    match config.profile {
        InstallProfile::Prod => {
            let host = config.mqtt_host.trim().to_lowercase();
            let loopback = host.is_empty() || host == "127.0.0.1" || host == "localhost";
            if loopback {
                if let Some(ip) = crate::net::recommend_lan_ipv4() {
                    config.mqtt_host = ip;
                }
            }
        }
        InstallProfile::E2e => {
            config.mqtt_host = "127.0.0.1".to_string();
        }
    }

    if config.profile == InstallProfile::E2e {
        // Always derive a namespaced prefix from the install root so multiple E2E installs can
        // coexist and repeated runs don't collide (and so the prefix tracks user-provided roots).
        config.launchd_label_prefix = e2e_label_prefix(&config.install_root);
        assign_e2e_ports(config)?;
    }

    Ok(())
}

fn e2e_label_prefix(install_root: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(install_root.as_bytes());
    let digest = hasher.finalize();
    let hex = hex::encode(digest);
    let short = hex.get(0..8).unwrap_or("00000000");
    format!("com.farmdashboard.e2e.{short}")
}

fn assign_e2e_ports(config: &mut SetupConfig) -> Result<()> {
    let mut used = HashSet::new();

    let db_port = database_port(&config.database_url);
    if db_port == 5432 {
        let port = next_free_port(&mut used)?;
        config.database_url = set_database_port(&config.database_url, port);
    } else {
        used.insert(db_port);
    }

    if config.core_port == 8000 {
        config.core_port = next_free_port(&mut used)?;
    } else {
        used.insert(config.core_port);
    }

    if config.mqtt_port == 1883 {
        config.mqtt_port = next_free_port(&mut used)?;
    } else {
        used.insert(config.mqtt_port);
    }

    if config.redis_port == DEFAULT_REDIS_PORT {
        config.redis_port = next_free_port(&mut used)?;
    } else {
        used.insert(config.redis_port);
    }

    if config.qdrant_port == DEFAULT_QDRANT_PORT {
        config.qdrant_port = next_free_port(&mut used)?;
    } else {
        used.insert(config.qdrant_port);
    }

    if config.setup_port == DEFAULT_SETUP_PORT {
        config.setup_port = next_free_port(&mut used)?;
    } else {
        used.insert(config.setup_port);
    }

    Ok(())
}

fn next_free_port(used: &mut HashSet<u16>) -> Result<u16> {
    for _ in 0..64 {
        let port = allocate_local_port()?;
        if port == 0 || used.contains(&port) {
            continue;
        }
        used.insert(port);
        return Ok(port);
    }
    bail!("Failed to allocate a free TCP port for E2E profile")
}

fn database_port(database_url: &str) -> u16 {
    let trimmed = database_url.split('?').next().unwrap_or(database_url);
    let host_part = trimmed
        .split('@')
        .last()
        .unwrap_or(trimmed)
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://")
        .trim_start_matches("postgresql+psycopg://");
    let host_port = host_part.split('/').next().unwrap_or(host_part);
    if let Some((_, port)) = host_port.rsplit_once(':') {
        if let Ok(parsed) = port.parse::<u16>() {
            return parsed;
        }
    }
    5432
}

fn set_database_port(database_url: &str, port: u16) -> String {
    let (base, query) = database_url
        .split_once('?')
        .map(|(lhs, rhs)| (lhs, Some(rhs)))
        .unwrap_or((database_url, None));

    let (creds, host_and_path) = base
        .rsplit_once('@')
        .map(|(lhs, rhs)| (Some(lhs), rhs))
        .unwrap_or((None, base));
    let (host_port, path) = host_and_path
        .split_once('/')
        .map(|(lhs, rhs)| (lhs, Some(rhs)))
        .unwrap_or((host_and_path, None));

    let host = host_port
        .rsplit_once(':')
        .map(|(lhs, _)| lhs)
        .unwrap_or(host_port);

    let mut rebuilt = String::new();
    if let Some(creds) = creds {
        rebuilt.push_str(creds);
        rebuilt.push('@');
    }
    rebuilt.push_str(host);
    rebuilt.push(':');
    rebuilt.push_str(&port.to_string());
    if let Some(path) = path {
        rebuilt.push('/');
        rebuilt.push_str(path);
    }
    if let Some(query) = query {
        rebuilt.push('?');
        rebuilt.push_str(query);
    }
    rebuilt
}

fn detect_bundle_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let app_path = detect_installer_app(&exe)?;

    // Prefer the embedded controller bundle inside the installer launcher app bundle.
    if let Some(found) = find_bundle_dmg_in_dir(&app_path.join("Contents/Resources")) {
        return Some(found);
    }

    // Legacy fallback: older installer layouts placed the controller DMG next to the app on the
    // mounted installer disk image.
    let dmg_root = app_path.parent()?.to_path_buf();
    find_bundle_dmg_in_dir(&dmg_root)
}

fn find_bundle_dmg_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut fallback = None;
    for entry in fs::read_dir(dir).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.extension().and_then(OsStr::to_str) != Some("dmg") {
            continue;
        }
        if let Some(name) = path.file_name().and_then(OsStr::to_str) {
            if name.contains("FarmDashboardController") {
                return Some(path);
            }
        }
        if fallback.is_none() {
            fallback = Some(path);
        }
    }
    fallback
}

fn detect_installer_app(exe: &Path) -> Option<PathBuf> {
    let mut current = exe.to_path_buf();
    while let Some(parent) = current.parent() {
        if current.extension().and_then(OsStr::to_str) == Some("app") {
            return Some(current.to_path_buf());
        }
        current = parent.to_path_buf();
    }
    None
}

pub fn validate_bundle_path(path: &Path) -> Result<()> {
    let value = path.to_string_lossy();
    if value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("file://")
        || value.starts_with("s3://")
    {
        bail!("Bundle path must be a local DMG path, not a remote URL");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e2e_profile_assigns_non_default_ports_and_label_prefix() {
        let mut config = default_config().unwrap();
        config.install_root = "/tmp/farm-dashboard-e2e-test-root".to_string();

        normalize_config(&mut config, Some(InstallProfile::E2e)).unwrap();

        assert_eq!(config.profile, InstallProfile::E2e);
        assert!(
            config
                .launchd_label_prefix
                .starts_with("com.farmdashboard.e2e."),
            "unexpected prefix: {}",
            config.launchd_label_prefix
        );
        assert_ne!(config.core_port, 8000);
        assert_ne!(config.mqtt_port, 1883);
        assert_ne!(config.redis_port, DEFAULT_REDIS_PORT);
        assert_ne!(config.qdrant_port, DEFAULT_QDRANT_PORT);
        assert_ne!(config.setup_port, DEFAULT_SETUP_PORT);

        let db_port = super::database_port(&config.database_url);
        assert_ne!(db_port, 5432);

        let ports = [
            config.core_port,
            config.mqtt_port,
            config.redis_port,
            config.qdrant_port,
            config.setup_port,
            db_port,
        ];
        let mut seen = std::collections::HashSet::new();
        for port in ports {
            assert!(
                seen.insert(port),
                "expected unique ports; saw duplicate {} in {:?}",
                port,
                ports
            );
        }
    }
}
