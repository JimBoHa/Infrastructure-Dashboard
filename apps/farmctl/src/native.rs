use anyhow::{bail, Context, Result};
use postgres::{Client, NoTls};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::config::SetupConfig;
use crate::paths::{
    mosquitto_binary, mosquitto_config_path, mosquitto_dir, postgres_binary, postgres_data_dir,
    postgres_initdb, qdrant_binary, qdrant_data_dir, redis_binary, redis_config_path,
    redis_data_dir, service_root,
};
use crate::profile::InstallProfile;
use crate::service_user::{chown_path, lookup_uid_gid};
use crate::sysv_ipc;
use crate::utils::run_cmd;

pub fn prepare_native_services(config: &SetupConfig) -> Result<()> {
    let data_root = validate_absolute_path(Path::new(&config.data_root), "data_root")?;
    let qdrant_root =
        validate_path_under_root(&qdrant_data_dir(config), &data_root, "qdrant_data_dir")?;
    let analysis_hot_root = validate_path_under_root(
        &data_root.join("storage/analysis/lake/hot"),
        &data_root,
        "analysis_lake_hot_path",
    )?;
    let analysis_tmp_root = validate_path_under_root(
        &data_root.join("storage/analysis/tmp"),
        &data_root,
        "analysis_tmp_path",
    )?;

    let required_bins = [
        ("postgres", postgres_binary(config)),
        ("initdb", postgres_initdb(config)),
        ("redis", redis_binary(config)),
        ("mosquitto", mosquitto_binary(config)),
        ("qdrant", qdrant_binary(config)),
    ];
    for (label, path) in required_bins {
        if !path.exists() {
            bail!("{} binary not found at {}", label, path.display());
        }
    }

    let logs_dir = Path::new(&config.logs_root);
    fs::create_dir_all(logs_dir)?;

    let service_root = service_root(config);
    fs::create_dir_all(&service_root)?;
    fs::create_dir_all(postgres_data_dir(config))?;
    fs::create_dir_all(redis_data_dir(config))?;
    fs::create_dir_all(mosquitto_dir(config).join("data"))?;
    ensure_dir_mode(&qdrant_root, 0o750)?;
    ensure_dir_mode(&analysis_hot_root, 0o750)?;
    ensure_dir_mode(&analysis_tmp_root, 0o700)?;

    let redis_conf = format!(
        "bind 127.0.0.1\nport {}\ndir {}\nprotected-mode yes\n",
        config.redis_port,
        redis_data_dir(config).display()
    );
    fs::write(redis_config_path(config), redis_conf)?;

    let mosquitto_conf = format!(
        "listener {} {}\npersistence true\npersistence_location {}\nlog_dest file {}\nallow_anonymous true\n",
        config.mqtt_port,
        if config.profile == InstallProfile::Prod {
            "0.0.0.0"
        } else {
            "127.0.0.1"
        },
        mosquitto_dir(config).join("data").display(),
        logs_dir.join("mosquitto.log").display()
    );
    fs::write(mosquitto_config_path(config), mosquitto_conf)?;

    ensure_dir_mode(&qdrant_root.join("storage"), 0o750)?;
    ensure_dir_mode(&qdrant_root.join("snapshots"), 0o750)?;
    ensure_dir_mode(&qdrant_root.join("tmp"), 0o750)?;
    let qdrant_conf = format!(
        "log_level: INFO\ntelemetry_disabled: true\n\nstorage:\n  storage_path: \"{}\"\n  snapshots_path: \"{}\"\n  temp_path: \"{}\"\n\nservice:\n  host: 127.0.0.1\n  http_port: {}\n  grpc_port: null\n  enable_cors: false\n",
        qdrant_root.join("storage").display(),
        qdrant_root.join("snapshots").display(),
        qdrant_root.join("tmp").display(),
        config.qdrant_port,
    );
    let qdrant_config = qdrant_root.join("qdrant.yaml");
    fs::write(&qdrant_config, qdrant_conf)?;
    ensure_file_mode(&qdrant_config, 0o600)?;

    Ok(())
}

fn validate_absolute_path(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("{label} must be an absolute path");
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            bail!("{label} must not contain '..' segments");
        }
    }
    canonicalize_with_existing_parent(path)
}

fn validate_path_under_root(path: &Path, root: &Path, label: &str) -> Result<PathBuf> {
    let canonical_root = canonicalize_with_existing_parent(root)
        .with_context(|| format!("failed to canonicalize root for {label}"))?;
    let canonical_path = canonicalize_with_existing_parent(path)
        .with_context(|| format!("failed to canonicalize {label}"))?;
    if !canonical_path.starts_with(&canonical_root) {
        bail!("{label} must reside under {}", canonical_root.display());
    }
    Ok(canonical_path)
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
        bail!("no existing ancestor found for {}", path.display());
    };
    let base = existing
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", existing.display()))?;
    let suffix = path.strip_prefix(existing).unwrap_or(Path::new(""));
    Ok(base.join(suffix))
}

fn ensure_dir_mode(path: &Path, mode: u32) -> Result<()> {
    fs::create_dir_all(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("Failed to chmod {} to {:o}", path.display(), mode))?;
    Ok(())
}

fn ensure_file_mode(path: &Path, mode: u32) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
        .with_context(|| format!("Failed to chmod {} to {:o}", path.display(), mode))?;
    Ok(())
}

pub fn ensure_postgres_initialized(config: &SetupConfig) -> Result<()> {
    let data_dir = postgres_data_dir(config);
    if data_dir.join("PG_VERSION").exists() {
        ensure_timescaledb_preload(&data_dir)?;
        return Ok(());
    }

    // macOS can accumulate stale SysV IPC objects (shared memory + semaphores) after repeated
    // Postgres bootstrap runs. This is especially common in the E2E profile where we do many
    // ephemeral installs. Clean up best-effort before initdb so we don't fail with shmget ENOSPC.
    if config.profile == InstallProfile::E2e {
        let _ = sysv_ipc::cleanup_stale_postgres_ipc(None);
    }

    let initdb = postgres_initdb(config);
    if !initdb.exists() {
        bail!("initdb not found at {}", initdb.display());
    }
    let pwfile_parent = data_dir
        .parent()
        .unwrap_or_else(|| Path::new(&config.data_root));
    let mut pwfile = tempfile::NamedTempFile::new_in(pwfile_parent)?;
    let Some(password) = crate::config::database_password(&config.database_url)
        .filter(|value| !value.trim().is_empty())
    else {
        bail!("database_url must include a non-empty password");
    };
    pwfile.write_all(format!("{password}\n").as_bytes())?;
    let pwfile_path = pwfile.path().to_path_buf();
    fs::set_permissions(&pwfile_path, fs::Permissions::from_mode(0o600))?;
    if config.profile == InstallProfile::Prod && unsafe { libc::geteuid() } == 0 {
        chown_path(&pwfile_path, &config.service_user, &config.service_group)?;
    }
    let mut cmd = Command::new(initdb);
    cmd.arg("-D")
        .arg(&data_dir)
        .arg("--username=postgres")
        .arg("--pwfile")
        .arg(&pwfile_path)
        .arg("--auth=md5");
    if config.profile == InstallProfile::Prod && unsafe { libc::geteuid() } == 0 {
        let (uid, gid) = lookup_uid_gid(&config.service_user)?;
        cmd.uid(uid).gid(gid);
    }
    run_cmd(cmd)?;
    ensure_timescaledb_preload(&data_dir)?;
    Ok(())
}

pub fn ensure_database_ready(config: &SetupConfig) -> Result<()> {
    let db_name = database_name(&config.database_url).unwrap_or_else(|| "iot".to_string());
    let admin_url = admin_database_url(
        &crate::config::postgres_connection_string(&config.database_url),
        &db_name,
    );
    let mut attempts = 0;
    loop {
        match Client::connect(&admin_url, NoTls) {
            Ok(mut client) => {
                let exists = client
                    .query("SELECT 1 FROM pg_database WHERE datname = $1", &[&db_name])?
                    .len()
                    > 0;
                if !exists {
                    client.execute(&format!("CREATE DATABASE \"{}\"", db_name), &[])?;
                }
                break;
            }
            Err(err) => {
                attempts += 1;
                if attempts >= 30 {
                    bail!("Database not ready: {err}");
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_config;
    use crate::paths::mosquitto_config_path;
    use std::fs;
    use std::path::Path;

    fn write_empty_executable(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn mosquitto_binds_to_lan_in_prod_profile() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = default_config().unwrap();
        config.profile = InstallProfile::Prod;
        config.install_root = temp.path().join("install").display().to_string();
        config.data_root = temp.path().join("data").display().to_string();
        config.logs_root = temp.path().join("logs").display().to_string();
        config.mqtt_port = 1883;

        let install_root = Path::new(&config.install_root);
        write_empty_executable(&install_root.join("native/postgres/bin/postgres"));
        write_empty_executable(&install_root.join("native/postgres/bin/initdb"));
        write_empty_executable(&install_root.join("native/redis/bin/redis-server"));
        write_empty_executable(&install_root.join("native/mosquitto/bin/mosquitto"));
        write_empty_executable(&install_root.join("native/qdrant/bin/qdrant"));

        prepare_native_services(&config).unwrap();
        let conf = fs::read_to_string(mosquitto_config_path(&config)).unwrap();
        assert!(
            conf.contains("listener 1883 0.0.0.0"),
            "expected mosquitto to bind 0.0.0.0 in prod, got:\n{conf}"
        );
    }

    #[test]
    fn mosquitto_binds_to_loopback_in_e2e_profile() {
        let temp = tempfile::tempdir().unwrap();
        let mut config = default_config().unwrap();
        config.profile = InstallProfile::E2e;
        config.install_root = temp.path().join("install").display().to_string();
        config.data_root = temp.path().join("data").display().to_string();
        config.logs_root = temp.path().join("logs").display().to_string();
        config.mqtt_port = 19999;

        let install_root = Path::new(&config.install_root);
        write_empty_executable(&install_root.join("native/postgres/bin/postgres"));
        write_empty_executable(&install_root.join("native/postgres/bin/initdb"));
        write_empty_executable(&install_root.join("native/redis/bin/redis-server"));
        write_empty_executable(&install_root.join("native/mosquitto/bin/mosquitto"));
        write_empty_executable(&install_root.join("native/qdrant/bin/qdrant"));

        prepare_native_services(&config).unwrap();
        let conf = fs::read_to_string(mosquitto_config_path(&config)).unwrap();
        assert!(
            conf.contains("listener 19999 127.0.0.1"),
            "expected mosquitto to bind 127.0.0.1 in e2e, got:\n{conf}"
        );
    }
}

fn database_name(database_url: &str) -> Option<String> {
    let trimmed = database_url.split('?').next().unwrap_or(database_url);
    trimmed
        .rsplit('/')
        .next()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

pub fn database_port(database_url: &str) -> u16 {
    let trimmed = database_url.split('?').next().unwrap_or(database_url);
    let host_part = trimmed
        .split('@')
        .last()
        .unwrap_or(trimmed)
        .trim_start_matches("postgresql://")
        .trim_start_matches("postgres://");
    let host_port = host_part.split('/').next().unwrap_or(host_part);
    if let Some((_, port)) = host_port.rsplit_once(':') {
        if let Ok(parsed) = port.parse::<u16>() {
            return parsed;
        }
    }
    5432
}

fn admin_database_url(database_url: &str, db_name: &str) -> String {
    let trimmed = database_url.split('?').next().unwrap_or(database_url);
    if let Some((base, _)) = trimmed.rsplit_once('/') {
        format!("{base}/postgres")
    } else {
        database_url.replace(db_name, "postgres")
    }
}

fn ensure_timescaledb_preload(data_dir: &Path) -> Result<()> {
    let config_path = data_dir.join("postgresql.conf");
    if !config_path.exists() {
        return Ok(());
    }
    let contents = fs::read_to_string(&config_path)?;
    let mut lines = Vec::new();
    let mut preload_found = false;
    let mut telemetry_found = false;
    for line in contents.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("shared_preload_libraries") {
            preload_found = true;
            if trimmed.contains("timescaledb") {
                lines.push(line.to_string());
            } else {
                lines.push("shared_preload_libraries = 'timescaledb'".to_string());
            }
            continue;
        }
        if trimmed.starts_with("timescaledb.telemetry_level") {
            telemetry_found = true;
            if trimmed.contains("off") {
                lines.push(line.to_string());
            } else {
                lines.push("timescaledb.telemetry_level = 'off'".to_string());
            }
            continue;
        }
        lines.push(line.to_string());
    }
    if !preload_found {
        lines.push("shared_preload_libraries = 'timescaledb'".to_string());
    }
    if !telemetry_found {
        lines.push("timescaledb.telemetry_level = 'off'".to_string());
    }
    fs::write(&config_path, lines.join("\n") + "\n")?;
    Ok(())
}
