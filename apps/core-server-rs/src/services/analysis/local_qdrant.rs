use crate::config::setup_config_path;
use crate::config::CoreConfig;
use crate::services::analysis::security::{ensure_dir_mode, ensure_file_mode};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
struct SetupInstallRoot {
    #[serde(default)]
    install_root: Option<String>,
}

fn read_install_root_from_setup_config() -> Option<PathBuf> {
    let path = setup_config_path();
    let raw = fs::read_to_string(&path).ok()?;
    let parsed: SetupInstallRoot = serde_json::from_str(&raw).ok()?;
    parsed
        .install_root
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn default_install_root() -> PathBuf {
    PathBuf::from("/usr/local/farm-dashboard")
}

fn qdrant_binary_path() -> PathBuf {
    if let Ok(value) = std::env::var("CORE_QDRANT_BINARY") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    let install_root = read_install_root_from_setup_config().unwrap_or_else(default_install_root);
    install_root.join("native/qdrant/bin/qdrant")
}

fn qdrant_data_root(config: &CoreConfig) -> PathBuf {
    config.data_root.join("storage/qdrant")
}

fn qdrant_config_path(config: &CoreConfig) -> PathBuf {
    qdrant_data_root(config).join("qdrant.yaml")
}

fn is_local_qdrant_url(url: &reqwest::Url) -> bool {
    matches!(
        url.host_str(),
        Some("127.0.0.1") | Some("localhost") | Some("::1")
    )
}

async fn qdrant_healthz(client: &reqwest::Client, base_url: &reqwest::Url) -> bool {
    let url = match base_url.join("healthz") {
        Ok(url) => url,
        Err(_) => return false,
    };
    let resp = client.get(url).timeout(Duration::from_secs(2)).send().await;
    match resp {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

fn ensure_qdrant_config(config: &CoreConfig, base_url: &reqwest::Url) -> Result<()> {
    let root = qdrant_data_root(config);
    ensure_dir_mode(&root, 0o750)?;
    ensure_dir_mode(&root.join("storage"), 0o750)?;
    ensure_dir_mode(&root.join("snapshots"), 0o750)?;
    ensure_dir_mode(&root.join("tmp"), 0o750)?;

    let port = base_url.port_or_known_default().unwrap_or(6333);
    let conf = format!(
        "log_level: INFO\ntelemetry_disabled: true\n\nstorage:\n  storage_path: \"{}\"\n  snapshots_path: \"{}\"\n  temp_path: \"{}\"\n\nservice:\n  host: 127.0.0.1\n  http_port: {}\n  grpc_port: null\n  enable_cors: false\n",
        root.join("storage").display(),
        root.join("snapshots").display(),
        root.join("tmp").display(),
        port
    );
    let path = qdrant_config_path(config);
    fs::write(&path, conf).with_context(|| format!("failed to write {}", path.display()))?;
    ensure_file_mode(&path, 0o600)?;
    Ok(())
}

fn open_append(path: &Path) -> Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        let _ = ensure_dir_mode(parent, 0o750);
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))
}

#[cfg(unix)]
fn maybe_bump_nofile_limit(target: u64) -> Result<()> {
    // Best-effort: increase the soft file-descriptor limit for the qdrant process.
    // Qdrant can hit "Too many open files" while persisting index structures on disk.
    unsafe {
        let mut lim: libc::rlimit = std::mem::zeroed();
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) != 0 {
            return Ok(());
        }
        let desired = std::cmp::min(target as libc::rlim_t, lim.rlim_max);
        if desired <= lim.rlim_cur {
            return Ok(());
        }
        let updated = libc::rlimit {
            rlim_cur: desired,
            rlim_max: lim.rlim_max,
        };
        // Ignore failures; launchd/service-user profiles may restrict raising limits.
        let _ = libc::setrlimit(libc::RLIMIT_NOFILE, &updated);
        Ok(())
    }
}

#[derive(Clone)]
pub struct LocalQdrantSupervisor {
    config: CoreConfig,
    base_url: reqwest::Url,
    client: reqwest::Client,
    start_lock: Arc<Mutex<()>>,
}

impl LocalQdrantSupervisor {
    pub fn maybe_new(config: &CoreConfig, client: reqwest::Client) -> Option<Self> {
        if std::env::consts::OS != "macos" {
            return None;
        }
        let base_url = reqwest::Url::parse(&config.qdrant_url).ok()?;
        if !is_local_qdrant_url(&base_url) {
            return None;
        }
        Some(Self {
            config: config.clone(),
            base_url,
            client,
            start_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn start(self, cancel: CancellationToken) {
        tokio::spawn(async move {
            let qdrant_binary = qdrant_binary_path();
            if !qdrant_binary.exists() {
                tracing::info!(
                    qdrant_binary = %qdrant_binary.display(),
                    "qdrant binary not found; skipping local qdrant supervisor"
                );
                return;
            }

            let mut delay = Duration::from_secs(2);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(delay) => {}
                }

                if qdrant_healthz(&self.client, &self.base_url).await {
                    delay = Duration::from_secs(30);
                    continue;
                }

                let _guard = self.start_lock.lock().await;
                if qdrant_healthz(&self.client, &self.base_url).await {
                    delay = Duration::from_secs(30);
                    continue;
                }

                if let Err(err) = ensure_qdrant_config(&self.config, &self.base_url) {
                    tracing::warn!(error = %err, "failed to ensure qdrant config; will retry");
                    delay = std::cmp::min(delay * 2, Duration::from_secs(30));
                    continue;
                }

                let root = qdrant_data_root(&self.config);
                let cfg = qdrant_config_path(&self.config);
                let logs_dir = self.config.data_root.join("logs");
                let stdout_path = logs_dir.join("qdrant.log");
                let stderr_path = logs_dir.join("qdrant.err.log");
                let stdout = match open_append(&stdout_path) {
                    Ok(file) => file,
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to open qdrant stdout log; will retry");
                        delay = std::cmp::min(delay * 2, Duration::from_secs(30));
                        continue;
                    }
                };
                let stderr = match open_append(&stderr_path) {
                    Ok(file) => file,
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to open qdrant stderr log; will retry");
                        delay = std::cmp::min(delay * 2, Duration::from_secs(30));
                        continue;
                    }
                };

                let mut cmd = Command::new(&qdrant_binary);
                cmd.arg("--config-path")
                    .arg(&cfg)
                    .current_dir(&root)
                    .stdout(Stdio::from(stdout))
                    .stderr(Stdio::from(stderr));
                #[cfg(unix)]
                unsafe {
                    cmd.pre_exec(|| {
                        let _ = maybe_bump_nofile_limit(8192);
                        Ok(())
                    });
                }

                match cmd.spawn() {
                    Ok(mut child) => {
                        tracing::info!(
                            qdrant_binary = %qdrant_binary.display(),
                            qdrant_root = %root.display(),
                            "spawned qdrant (local supervisor)"
                        );
                        tokio::spawn(async move {
                            if let Ok(status) = child.wait().await {
                                tracing::warn!(status = %status, "qdrant exited");
                            }
                        });

                        // Give it a short window to come up before we relax the retry backoff.
                        let mut healthy = false;
                        for _ in 0..20 {
                            if qdrant_healthz(&self.client, &self.base_url).await {
                                healthy = true;
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(250)).await;
                        }
                        delay = if healthy {
                            Duration::from_secs(30)
                        } else {
                            Duration::from_secs(5)
                        };
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "failed to spawn qdrant; will retry");
                        delay = std::cmp::min(delay * 2, Duration::from_secs(30));
                    }
                }
            }
        });
    }
}
