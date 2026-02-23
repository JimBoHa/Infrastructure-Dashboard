use anyhow::Result;
use libc::geteuid;
use plist::{Dictionary as PlistDictionary, Value as PlistValue};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::{default_config_path, env_flag, setup_state_dir, SetupConfig};
use crate::constants::DEFAULT_SETUP_HOST;
use crate::paths::{
    mosquitto_binary, mosquitto_config_path, postgres_binary, postgres_data_dir, qdrant_binary,
    qdrant_config_path, qdrant_data_dir, redis_binary, redis_config_path,
};
use crate::profile::InstallProfile;
use crate::utils::{port_available, run_cmd_capture, which, writable_dir, CommandResult};

#[derive(Debug, Clone, Serialize)]
pub struct PreflightCheck {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchdPlan {
    pub staging_dir: String,
    pub target_dir: String,
    pub plists: Vec<LaunchdPlistEntry>,
    pub commands: Vec<String>,
    pub load_commands: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LaunchdPlistEntry {
    pub label: String,
    pub staged_path: String,
    pub target_path: String,
}

fn is_root() -> bool {
    unsafe { geteuid() == 0 }
}

fn launchd_target_dir(config: &SetupConfig, staging_dir: &Path) -> PathBuf {
    if let Ok(path) = std::env::var("FARM_SETUP_LAUNCHD_ROOT") {
        return PathBuf::from(path);
    }
    match config.profile {
        InstallProfile::Prod => PathBuf::from("/Library/LaunchDaemons"),
        InstallProfile::E2e => staging_dir.to_path_buf(),
    }
}

fn launchctl_target(config: &SetupConfig) -> Option<String> {
    if env_flag("FARM_SETUP_SKIP_LAUNCHCTL") {
        return None;
    }
    if std::env::var("FARM_SETUP_LAUNCHD_ROOT").is_ok() {
        return None;
    }
    if config.profile == InstallProfile::Prod {
        return Some("system".to_string());
    }
    let uid = unsafe { libc::getuid() };
    Some(format!("gui/{uid}"))
}

fn label_for(config: &SetupConfig, suffix: &str) -> String {
    let prefix = config.launchd_label_prefix.trim_end_matches('.');
    format!("{prefix}.{suffix}")
}

fn launchd_payload(
    label: &str,
    program_args: &[String],
    env: &BTreeMap<String, String>,
    working_dir: &str,
    stdout_path: &str,
    stderr_path: &str,
    run_as: Option<(&str, &str)>,
) -> PlistValue {
    let mut dict = PlistDictionary::new();
    dict.insert("Label".to_string(), PlistValue::String(label.to_string()));
    dict.insert(
        "ProgramArguments".to_string(),
        PlistValue::Array(
            program_args
                .iter()
                .cloned()
                .map(PlistValue::String)
                .collect(),
        ),
    );
    let mut env_dict = PlistDictionary::new();
    for (key, value) in env {
        env_dict.insert(key.clone(), PlistValue::String(value.clone()));
    }
    dict.insert(
        "EnvironmentVariables".to_string(),
        PlistValue::Dictionary(env_dict),
    );
    dict.insert(
        "WorkingDirectory".to_string(),
        PlistValue::String(working_dir.to_string()),
    );
    dict.insert("RunAtLoad".to_string(), PlistValue::Boolean(true));
    dict.insert("KeepAlive".to_string(), PlistValue::Boolean(true));
    dict.insert(
        "StandardOutPath".to_string(),
        PlistValue::String(stdout_path.to_string()),
    );
    dict.insert(
        "StandardErrorPath".to_string(),
        PlistValue::String(stderr_path.to_string()),
    );
    if let Some((user, group)) = run_as {
        dict.insert("UserName".to_string(), PlistValue::String(user.to_string()));
        dict.insert(
            "GroupName".to_string(),
            PlistValue::String(group.to_string()),
        );
    }
    PlistValue::Dictionary(dict)
}

pub fn generate_plan(config: &SetupConfig) -> Result<LaunchdPlan> {
    let staging_dir = setup_state_dir().join("launchd");
    std::fs::create_dir_all(&staging_dir)?;
    let target_dir = launchd_target_dir(config, &staging_dir);

    let logs_dir = Path::new(&config.logs_root);
    std::fs::create_dir_all(logs_dir)?;

    let warnings = Vec::new();

    let mut services: Vec<(
        String,
        Vec<String>,
        BTreeMap<String, String>,
        String,
        String,
        String,
    )> = Vec::new();

    services.push((
        label_for(config, "postgres"),
        vec![
            postgres_binary(config).display().to_string(),
            "-D".to_string(),
            postgres_data_dir(config).display().to_string(),
            "-p".to_string(),
            crate::native::database_port(&config.database_url).to_string(),
            "-h".to_string(),
            "127.0.0.1".to_string(),
            "-c".to_string(),
            "shared_preload_libraries=timescaledb".to_string(),
            "-c".to_string(),
            "timescaledb.telemetry_level=off".to_string(),
        ],
        BTreeMap::new(),
        config.install_root.clone(),
        logs_dir.join("postgres.log").display().to_string(),
        logs_dir.join("postgres.err.log").display().to_string(),
    ));

    services.push((
        label_for(config, "redis"),
        vec![
            redis_binary(config).display().to_string(),
            redis_config_path(config).display().to_string(),
        ],
        BTreeMap::new(),
        config.install_root.clone(),
        logs_dir.join("redis.log").display().to_string(),
        logs_dir.join("redis.err.log").display().to_string(),
    ));

    services.push((
        label_for(config, "mosquitto"),
        vec![
            mosquitto_binary(config).display().to_string(),
            "-c".to_string(),
            mosquitto_config_path(config).display().to_string(),
        ],
        BTreeMap::new(),
        config.install_root.clone(),
        logs_dir.join("mosquitto.log").display().to_string(),
        logs_dir.join("mosquitto.err.log").display().to_string(),
    ));

    services.push((
        label_for(config, "qdrant"),
        vec![
            qdrant_binary(config).display().to_string(),
            "--config-path".to_string(),
            qdrant_config_path(config).display().to_string(),
        ],
        BTreeMap::new(),
        qdrant_data_dir(config).display().to_string(),
        logs_dir.join("qdrant.log").display().to_string(),
        logs_dir.join("qdrant.err.log").display().to_string(),
    ));

    services.push((
        label_for(config, "core-server"),
        vec![
            config.core_binary.clone(),
            "--host".to_string(),
            (if config.profile == InstallProfile::Prod {
                "0.0.0.0"
            } else {
                "127.0.0.1"
            })
            .to_string(),
            "--port".to_string(),
            config.core_port.to_string(),
        ],
        {
            let mut env = BTreeMap::from([
                ("CORE_DATABASE_URL".to_string(), config.database_url.clone()),
                ("CORE_MQTT_HOST".to_string(), config.mqtt_host.clone()),
                ("CORE_MQTT_PORT".to_string(), config.mqtt_port.to_string()),
                (
                    "CORE_SETUP_DAEMON_BASE_URL".to_string(),
                    format!("http://{}:{}", DEFAULT_SETUP_HOST, config.setup_port),
                ),
                (
                    "CORE_STATIC_ROOT".to_string(),
                    PathBuf::from(&config.install_root)
                        .join("static/dashboard-web")
                        .display()
                        .to_string(),
                ),
                ("CORE_DATA_ROOT".to_string(), config.data_root.clone()),
                (
                    "CORE_BACKUP_STORAGE_PATH".to_string(),
                    config.backup_root.clone(),
                ),
                (
                    "CORE_BACKUP_RETENTION_DAYS".to_string(),
                    config.backup_retention_days.to_string(),
                ),
                (
                    "CORE_NODE_AGENT_OVERLAY_PATH".to_string(),
                    PathBuf::from(&config.install_root)
                        .join("artifacts/node-agent/node-agent-overlay.tar.gz")
                        .display()
                        .to_string(),
                ),
                (
                    "CORE_SSH_KNOWN_HOSTS_PATH".to_string(),
                    PathBuf::from(&config.data_root)
                        .join("storage/ssh/known_hosts")
                        .display()
                        .to_string(),
                ),
                (
                    "CORE_QDRANT_URL".to_string(),
                    format!("http://127.0.0.1:{}", config.qdrant_port),
                ),
                (
                    "CORE_ANALYSIS_LAKE_HOT_PATH".to_string(),
                    PathBuf::from(&config.data_root)
                        .join("storage/analysis/lake/hot")
                        .display()
                        .to_string(),
                ),
                (
                    "CORE_ANALYSIS_TMP_PATH".to_string(),
                    PathBuf::from(&config.data_root)
                        .join("storage/analysis/tmp")
                        .display()
                        .to_string(),
                ),
                ("CORE_DEMO_MODE".to_string(), "false".to_string()),
                (
                    "CORE_ENABLE_ANALYTICS_FEEDS".to_string(),
                    config.enable_analytics_feeds.to_string(),
                ),
                (
                    "CORE_ENABLE_FORECAST_INGESTION".to_string(),
                    config.enable_forecast_ingestion.to_string(),
                ),
                (
                    "CORE_ANALYTICS_FEED_POLL_INTERVAL_SECONDS".to_string(),
                    config.analytics_feed_poll_interval_seconds.to_string(),
                ),
                (
                    "CORE_FORECAST_POLL_INTERVAL_SECONDS".to_string(),
                    config.forecast_poll_interval_seconds.to_string(),
                ),
                (
                    "CORE_SCHEDULE_POLL_INTERVAL_SECONDS".to_string(),
                    config.schedule_poll_interval_seconds.to_string(),
                ),
            ]);
            if config.profile == InstallProfile::E2e {
                env.insert(
                    "CORE_ALLOW_BOOTSTRAP_USER_CREATE".to_string(),
                    "1".to_string(),
                );
            }
            if let Some(username) = config
                .mqtt_username
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                env.insert("CORE_MQTT_USERNAME".to_string(), username.to_string());
            }
            if let Some(password) = config
                .mqtt_password
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                env.insert("CORE_MQTT_PASSWORD".to_string(), password.to_string());
            }
            env
        },
        config.install_root.clone(),
        logs_dir.join("core-server.log").display().to_string(),
        logs_dir.join("core-server.err.log").display().to_string(),
    ));

    services.push((
        label_for(config, "telemetry-sidecar"),
        vec![config.sidecar_binary.clone()],
        {
            let mut env = BTreeMap::from([
                (
                    "SIDECAR_DATABASE_URL".to_string(),
                    config.database_url.clone(),
                ),
                ("SIDECAR_MQTT_HOST".to_string(), config.mqtt_host.clone()),
                (
                    "SIDECAR_MQTT_PORT".to_string(),
                    config.mqtt_port.to_string(),
                ),
            ]);
            if let Some(username) = config
                .mqtt_username
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                env.insert("SIDECAR_MQTT_USERNAME".to_string(), username.to_string());
            }
            if let Some(password) = config
                .mqtt_password
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                env.insert("SIDECAR_MQTT_PASSWORD".to_string(), password.to_string());
            }
            env
        },
        config.install_root.clone(),
        logs_dir.join("telemetry-sidecar.log").display().to_string(),
        logs_dir
            .join("telemetry-sidecar.err.log")
            .display()
            .to_string(),
    ));

    services.push((
        label_for(config, "setup-daemon"),
        vec![
            config.farmctl_path.clone(),
            "--profile".to_string(),
            config.profile.to_string(),
            "serve".to_string(),
            "--host".to_string(),
            DEFAULT_SETUP_HOST.to_string(),
            "--port".to_string(),
            config.setup_port.to_string(),
            "--config".to_string(),
            default_config_path().display().to_string(),
            "--no-auto-open".to_string(),
        ],
        BTreeMap::new(),
        config.install_root.clone(),
        logs_dir.join("setup-daemon.log").display().to_string(),
        logs_dir.join("setup-daemon.err.log").display().to_string(),
    ));

    let mut plists = Vec::new();
    let run_as = if config.profile == InstallProfile::Prod {
        Some((config.service_user.as_str(), config.service_group.as_str()))
    } else {
        None
    };
    for (label, args, env, workdir, stdout, stderr) in services {
        let plist_path = staging_dir.join(format!("{label}.plist"));
        let payload = launchd_payload(&label, &args, &env, &workdir, &stdout, &stderr, run_as);
        let mut file = std::fs::File::create(&plist_path)?;
        plist::to_writer_xml(&mut file, &payload)?;
        plists.push(LaunchdPlistEntry {
            label,
            staged_path: plist_path.display().to_string(),
            target_path: target_dir
                .join(plist_path.file_name().unwrap_or_default())
                .display()
                .to_string(),
        });
    }

    let commands = plists
        .iter()
        .map(|entry| {
            if entry.staged_path == entry.target_path {
                return format!("# staged: {}", entry.staged_path);
            }
            if target_dir == Path::new("/Library/LaunchDaemons") && !is_root() {
                format!("sudo cp {} {}", entry.staged_path, entry.target_path)
            } else {
                format!("cp {} {}", entry.staged_path, entry.target_path)
            }
        })
        .collect::<Vec<_>>();

    let load_commands = plists
        .iter()
        .filter_map(|entry| {
            launchctl_target(config).map(|target| {
                if target == "system" {
                    format!("sudo launchctl bootstrap {target} {}", entry.target_path)
                } else {
                    format!("launchctl bootstrap {target} {}", entry.target_path)
                }
            })
        })
        .collect::<Vec<_>>();

    Ok(LaunchdPlan {
        staging_dir: staging_dir.display().to_string(),
        target_dir: target_dir.display().to_string(),
        plists,
        commands,
        load_commands,
        warnings,
    })
}

pub fn apply_launchd(config: &SetupConfig) -> Result<Vec<CommandResult>> {
    let plan = generate_plan(config)?;
    let target_dir = PathBuf::from(&plan.target_dir);
    let target = launchctl_target(config);

    let mut results = Vec::new();
    for entry in plan.plists {
        if entry.staged_path != entry.target_path {
            if target_dir == Path::new("/Library/LaunchDaemons") && !is_root() {
                let mut cmd = Command::new("sudo");
                cmd.arg("cp")
                    .arg(&entry.staged_path)
                    .arg(&entry.target_path);
                results.push(run_cmd_capture(cmd)?);
            } else {
                let mut cmd = Command::new("cp");
                cmd.arg(&entry.staged_path).arg(&entry.target_path);
                results.push(run_cmd_capture(cmd)?);
            }
        }

        if let Some(target_dir) = &target {
            let use_sudo = target_dir == "system" && !is_root();
            let mut bootout_cmd = if use_sudo {
                let mut cmd = Command::new("sudo");
                cmd.arg("launchctl");
                cmd
            } else {
                Command::new("launchctl")
            };
            bootout_cmd
                .arg("bootout")
                .arg(target_dir)
                .arg(&entry.target_path);
            let bootout = run_cmd_capture(bootout_cmd)?;
            results.push(bootout);
            let mut bootstrap_cmd = if use_sudo {
                let mut cmd = Command::new("sudo");
                cmd.arg("launchctl");
                cmd
            } else {
                Command::new("launchctl")
            };
            bootstrap_cmd
                .arg("bootstrap")
                .arg(target_dir)
                .arg(&entry.target_path);
            let bootstrap = run_cmd_capture(bootstrap_cmd)?;
            results.push(bootstrap);
            if config.profile == InstallProfile::Prod {
                let mut enable_cmd = if use_sudo {
                    let mut cmd = Command::new("sudo");
                    cmd.arg("launchctl");
                    cmd
                } else {
                    Command::new("launchctl")
                };
                enable_cmd
                    .arg("enable")
                    .arg(format!("{}/{}", target_dir, entry.label));
                let enable = run_cmd_capture(enable_cmd)?;
                results.push(enable);
            }
        }
    }
    Ok(results)
}

pub fn launchd_results_ok(results: &[CommandResult]) -> bool {
    results.iter().all(|result| {
        if result.ok {
            return true;
        }
        result.command.contains("launchctl bootout")
    })
}

pub fn run_preflight(config: &SetupConfig) -> Result<Vec<PreflightCheck>> {
    let mut checks = Vec::new();

    let os_name = std::env::consts::OS.to_string();
    checks.push(PreflightCheck {
        id: "os".to_string(),
        status: if os_name == "macos" { "ok" } else { "error" }.to_string(),
        message: format!("Detected OS: {os_name}"),
    });

    checks.push(PreflightCheck {
        id: "launchctl".to_string(),
        status: if which("launchctl").is_some() {
            "ok"
        } else {
            "error"
        }
        .to_string(),
        message: "launchctl is available".to_string(),
    });

    let farmctl_path = PathBuf::from(&config.farmctl_path);
    let farmctl_exists = farmctl_path.exists()
        || (!config.farmctl_path.contains('/') && which(&config.farmctl_path).is_some());
    checks.push(PreflightCheck {
        id: "farmctl".to_string(),
        status: if farmctl_exists { "ok" } else { "error" }.to_string(),
        message: format!("farmctl is available at {}", config.farmctl_path),
    });

    let install_root = PathBuf::from(&config.install_root);
    let existing_install = [
        PathBuf::from(&config.core_binary),
        PathBuf::from(&config.sidecar_binary),
        postgres_binary(config),
        redis_binary(config),
        mosquitto_binary(config),
        qdrant_binary(config),
    ]
    .iter()
    .any(|path| path.exists());

    let host = config.mqtt_host.trim().to_lowercase();
    let loopback = host.is_empty() || host == "127.0.0.1" || host == "localhost";
    let (status, message) = match config.profile {
        InstallProfile::Prod => {
            if loopback {
                (
                    "warn",
                    "MQTT host is set to localhost; remote nodes cannot connect. Set mqtt_host to this Mac’s LAN IP (Configure → “Use this Mac’s IP”).".to_string(),
                )
            } else {
                (
                    "ok",
                    format!("MQTT host is configured for nodes: {}", config.mqtt_host),
                )
            }
        }
        InstallProfile::E2e => (
            "ok",
            format!("MQTT host (E2E profile): {}", config.mqtt_host),
        ),
    };
    checks.push(PreflightCheck {
        id: "mqtt-host".to_string(),
        status: status.to_string(),
        message,
    });

    for port in [
        config.setup_port,
        config.core_port,
        config.mqtt_port,
        config.redis_port,
        config.qdrant_port,
        crate::native::database_port(&config.database_url),
    ] {
        let available = port_available(port);
        let (status, message) = if available {
            ("ok", format!("Port {port} is available"))
        } else if port == config.setup_port {
            (
                "ok",
                format!("Port {port} is in use by the setup wizard (expected while running)"),
            )
        } else if existing_install && config.profile == InstallProfile::Prod {
            (
                "info",
                format!("Port {port} is in use (expected while an existing install is running)"),
            )
        } else {
            ("warn", format!("Port {port} is in use"))
        };
        checks.push(PreflightCheck {
            id: format!("port-{port}"),
            status: status.to_string(),
            message,
        });
    }

    checks.push(PreflightCheck {
        id: "state-dir".to_string(),
        status: if writable_dir(&setup_state_dir()) {
            "ok"
        } else {
            "error"
        }
        .to_string(),
        message: format!(
            "State directory is writable: {}",
            setup_state_dir().display()
        ),
    });

    checks.push(PreflightCheck {
        id: "existing-install".to_string(),
        status: if existing_install { "info" } else { "ok" }.to_string(),
        message: if existing_install {
            format!(
                "Existing install detected under {} (use Upgrade to update, or uninstall/reset for a clean reinstall)",
                install_root.display()
            )
        } else {
            format!("No existing install detected under {}", install_root.display())
        },
    });

    checks.push(PreflightCheck {
        id: "data-root".to_string(),
        status: if writable_dir(&PathBuf::from(&config.data_root)) {
            "ok"
        } else {
            "error"
        }
        .to_string(),
        message: format!("Data root is writable: {}", config.data_root),
    });

    checks.push(PreflightCheck {
        id: "backup-root".to_string(),
        status: if writable_dir(&PathBuf::from(&config.backup_root)) {
            "ok"
        } else {
            "error"
        }
        .to_string(),
        message: format!("Backup path is writable: {}", config.backup_root),
    });

    if let Some(bundle_path) = config.bundle_path.as_ref() {
        let exists = Path::new(bundle_path).exists();
        checks.push(PreflightCheck {
            id: "bundle-path".to_string(),
            status: if exists { "ok" } else { "error" }.to_string(),
            message: format!("Bundle DMG found at {}", bundle_path),
        });
    }

    checks.push(PreflightCheck {
        id: "root".to_string(),
        status: "ok".to_string(),
        message: if config.profile == InstallProfile::Prod && !is_root() {
            "Wizard is running as your user (expected). You'll be prompted for admin only when installing LaunchDaemons.".to_string()
        } else if config.profile == InstallProfile::Prod {
            "Running as root (install actions can install LaunchDaemons without prompting).".to_string()
        } else {
            "E2E profile uses LaunchAgents (no admin required).".to_string()
        },
    });

    Ok(checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_state_dir<T>(state_dir: &Path, f: impl FnOnce() -> T) -> T {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let key = "FARM_SETUP_STATE_DIR";
        let previous = std::env::var(key).ok();
        std::env::set_var(key, state_dir);
        let result = f();
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        result
    }

    #[test]
    fn prod_launchd_plan_sets_user_and_group() {
        let temp = tempfile::tempdir().unwrap();
        let plan = with_state_dir(temp.path(), || {
            let mut config = default_config().unwrap();
            config.profile = InstallProfile::Prod;
            config.install_root = temp.path().join("install").display().to_string();
            config.data_root = temp.path().join("data").display().to_string();
            config.logs_root = temp.path().join("logs").display().to_string();
            config.backup_root = temp.path().join("backups").display().to_string();
            config.service_user = "_farmdashboard".to_string();
            config.service_group = "_farmdashboard".to_string();
            generate_plan(&config).unwrap()
        });

        assert_eq!(plan.target_dir, "/Library/LaunchDaemons");
        for entry in plan.plists {
            let value = plist::Value::from_file(Path::new(&entry.staged_path)).unwrap();
            let dict = value.as_dictionary().unwrap();
            assert!(
                dict.contains_key("UserName"),
                "missing UserName in {}",
                entry.staged_path
            );
            assert!(
                dict.contains_key("GroupName"),
                "missing GroupName in {}",
                entry.staged_path
            );
        }
    }

    #[test]
    fn e2e_launchd_plan_stays_self_contained_without_run_as() {
        let temp = tempfile::tempdir().unwrap();
        let plan = with_state_dir(temp.path(), || {
            let mut config = default_config().unwrap();
            config.profile = InstallProfile::E2e;
            config.install_root = temp.path().join("install").display().to_string();
            config.data_root = temp.path().join("data").display().to_string();
            config.logs_root = temp.path().join("logs").display().to_string();
            config.backup_root = temp.path().join("backups").display().to_string();
            generate_plan(&config).unwrap()
        });

        assert_eq!(plan.target_dir, plan.staging_dir);
        for entry in plan.plists {
            let value = plist::Value::from_file(Path::new(&entry.staged_path)).unwrap();
            let dict = value.as_dictionary().unwrap();
            assert!(
                !dict.contains_key("UserName"),
                "unexpected UserName in {}",
                entry.staged_path
            );
            assert!(
                !dict.contains_key("GroupName"),
                "unexpected GroupName in {}",
                entry.staged_path
            );
        }
    }
}
