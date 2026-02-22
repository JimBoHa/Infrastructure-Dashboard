use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;
use walkdir::WalkDir;
use zip::write::FileOptions;

use crate::bootstrap_admin;
use crate::bundle::BundleManifest;
use crate::cli::{DiagnosticsArgs, HealthArgs, InstallArgs, RollbackArgs, StatusArgs};
use crate::config::{
    env_flag, load_config, normalize_config, resolve_config_path, save_config_if_changed,
    validate_bundle_path, SaveConfigMode,
};
use crate::constants::{BUNDLE_ROOT_DIR, MANIFEST_NAME, MANIFEST_VERSION};
use crate::health::run_health_checks;
use crate::launchd::{apply_launchd, generate_plan, launchd_results_ok};
use crate::migrations::apply_migrations;
use crate::native::{ensure_database_ready, ensure_postgres_initialized, prepare_native_services};
use crate::paths::state_path;
use crate::processes;
use crate::profile::InstallProfile;
use crate::service_user::{ensure_production_permissions, ensure_production_user};
use crate::utils::{copy_dir, redact_secrets, sha256_file, symlink_force, write_zip_entry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallState {
    pub current_version: Option<String>,
    pub previous_version: Option<String>,
    pub installed_versions: Vec<InstalledRelease>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRelease {
    pub version: String,
    pub installed_at: String,
}

struct DmgMount {
    mount_dir: TempDir,
    _dmg_copy_dir: Option<TempDir>,
}

impl DmgMount {
    fn path(&self) -> &Path {
        self.mount_dir.path()
    }
}

impl Drop for DmgMount {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .arg("detach")
            .arg(self.mount_dir.path())
            .arg("-quiet")
            .status();
    }
}

#[derive(Clone, Copy)]
pub enum InstallMode {
    Install,
    Upgrade,
}

fn load_state(config: &crate::config::SetupConfig) -> Result<InstallState> {
    let path = state_path(config);
    if !path.exists() {
        return Ok(InstallState {
            current_version: None,
            previous_version: None,
            installed_versions: Vec::new(),
        });
    }
    let contents = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&contents)?)
}

fn save_state(config: &crate::config::SetupConfig, state: &InstallState) -> Result<()> {
    let path = state_path(config);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(state)?;
    fs::write(&path, contents)?;
    Ok(())
}

fn mount_dmg(path: &Path) -> Result<DmgMount> {
    validate_bundle_path(path)?;
    if !path.exists() {
        bail!("Bundle DMG not found at {}", path.display());
    }
    let mount_dir = tempfile::tempdir()?;

    fn strip_quarantine_best_effort(path: &Path) {
        let _ = Command::new("xattr")
            .arg("-d")
            .arg("com.apple.quarantine")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    fn looks_like_quarantine_block(err: &str) -> bool {
        let lowered = err.to_lowercase();
        lowered.contains("resource temporarily unavailable")
            || lowered.contains("operation not permitted")
            || lowered.contains("not permitted")
    }

    let attach_with_retries = |dmg_path: &Path| -> Result<Option<String>> {
        let mut last_error: Option<String> = None;
        let mut delay = Duration::from_millis(250);
        for attempt in 1..=5 {
            let output = Command::new("hdiutil")
                .arg("attach")
                .arg(dmg_path)
                .arg("-nobrowse")
                .arg("-readonly")
                .arg("-mountpoint")
                .arg(mount_dir.path())
                .output()
                .with_context(|| "Failed to run hdiutil attach")?;

            if output.status.success() {
                return Ok(None);
            }

            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            last_error = Some(if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("hdiutil attach failed (attempt {attempt})")
            });

            // Best-effort cleanup: detach the mountpoint in case the attach partially succeeded.
            let _ = Command::new("hdiutil")
                .arg("detach")
                .arg(mount_dir.path())
                .arg("-quiet")
                .status();

            if attempt < 5 {
                std::thread::sleep(delay);
                delay = (delay * 2).min(Duration::from_secs(2));
            }
        }
        Ok(last_error.or_else(|| Some("hdiutil attach failed".to_string())))
    };

    // Best-effort: downloaded DMGs often inherit com.apple.quarantine, which can cause `hdiutil attach`
    // to fail in non-interactive contexts. Try stripping quarantine in-place before falling back to a
    // temp copy.
    strip_quarantine_best_effort(path);
    if let Some(err) = attach_with_retries(path)? {
        if looks_like_quarantine_block(&err) {
            let dmg_copy_dir = tempfile::tempdir()?;
            let file_name = path
                .file_name()
                .map(|name| name.to_owned())
                .unwrap_or_else(|| OsStr::new("FarmDashboardController.dmg").to_owned());
            let copied = dmg_copy_dir.path().join(file_name);
            fs::copy(path, &copied)?;
            strip_quarantine_best_effort(&copied);
            if let Some(copy_err) = attach_with_retries(&copied)? {
                bail!(
                    "Failed to mount DMG at {}: {}",
                    path.display(),
                    copy_err.trim()
                );
            }
            return Ok(DmgMount {
                mount_dir,
                _dmg_copy_dir: Some(dmg_copy_dir),
            });
        }
        bail!("Failed to mount DMG at {}: {}", path.display(), err.trim());
    }

    Ok(DmgMount {
        mount_dir,
        _dmg_copy_dir: None,
    })
}

fn load_manifest(bundle_root: &Path) -> Result<BundleManifest> {
    let manifest_path = bundle_root.join(MANIFEST_NAME);
    let contents = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Failed to read manifest at {}", manifest_path.display()))?;
    let manifest: BundleManifest = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse manifest at {}", manifest_path.display()))?;
    if manifest.format_version == 0 || manifest.format_version > MANIFEST_VERSION {
        bail!(
            "Unsupported manifest format {}; expected 1..={}",
            manifest.format_version,
            MANIFEST_VERSION
        );
    }
    Ok(manifest)
}

fn verify_manifest(manifest: &BundleManifest, bundle_root: &Path) -> Result<()> {
    for entry in &manifest.files {
        let file_path = bundle_root.join(&entry.path);
        if !file_path.exists() {
            bail!("Manifest file missing: {}", entry.path);
        }
        let digest = sha256_file(&file_path)?;
        if digest != entry.sha256 {
            bail!(
                "Checksum mismatch for {} (expected {}, got {})",
                entry.path,
                entry.sha256,
                digest
            );
        }
    }
    Ok(())
}

pub fn install_bundle(
    args: InstallArgs,
    mode: InstallMode,
    profile_override: Option<InstallProfile>,
) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let mut config = load_config(&config_path)?;
    let config_before_normalize = config.clone();
    normalize_config(&mut config, profile_override)?;
    let changed = config_before_normalize != config;
    let saved = save_config_if_changed(
        &config_path,
        &config_before_normalize,
        &config,
        SaveConfigMode::BestEffort,
    )?;
    if changed && !saved {
        eprintln!(
            "Warning: unable to update setup config at {} (permission denied); continuing with in-memory config",
            config_path.display()
        );
    }
    ensure_production_user(&config)?;

    let mut state = load_state(&config)?;
    if config.profile == InstallProfile::Prod && unsafe { libc::geteuid() } != 0 {
        if state.current_version.is_none() {
            bail!("No existing production install found; run install via the installer app (admin prompt) or sudo");
        }
    }
    if matches!(mode, InstallMode::Upgrade)
        && config.profile == InstallProfile::Prod
        && unsafe { libc::geteuid() } != 0
        && !env_flag("FARM_SETUP_SKIP_LAUNCHD")
    {
        ensure_qdrant_launchd_present_for_upgrade(&config)?;
    }

    validate_bundle_path(&args.bundle)?;
    let mount = mount_dmg(&args.bundle)?;
    let bundle_root = mount.path().join(BUNDLE_ROOT_DIR);
    if !bundle_root.exists() {
        bail!("Bundle root not found at {}", bundle_root.display());
    }
    let manifest = load_manifest(&bundle_root)?;
    if let Some(version) = &args.version {
        if version != &manifest.bundle_version {
            bail!(
                "Bundle version {} does not match requested {}",
                manifest.bundle_version,
                version
            );
        }
    }
    verify_manifest(&manifest, &bundle_root)?;

    let install_root = Path::new(&config.install_root);
    let releases_root = install_root.join("releases");
    let release_dir = releases_root.join(&manifest.bundle_version);

    if release_dir.exists() && !args.force {
        if matches!(mode, InstallMode::Install) {
            println!(
                "Release {} already installed; use --force to reinstall",
                manifest.bundle_version
            );
            return Ok(());
        }
    }

    fs::create_dir_all(&release_dir)?;
    copy_dir(
        &bundle_root.join("artifacts"),
        &release_dir.join("artifacts"),
    )?;
    if bundle_root.join("configs").exists() {
        copy_dir(&bundle_root.join("configs"), &release_dir.join("configs"))?;
    }
    copy_dir(
        &bundle_root.join("migrations"),
        &release_dir.join("migrations"),
    )?;
    if bundle_root.join("native").exists() {
        copy_dir(&bundle_root.join("native"), &release_dir.join("native"))?;
    }
    fs::write(
        release_dir.join(MANIFEST_NAME),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    let bin_dir = install_root.join("bin");
    fs::create_dir_all(&bin_dir)?;
    for component in &manifest.components {
        let entry = release_dir.join(&component.entrypoint);
        let name = Path::new(&component.entrypoint)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or(&component.name);
        let link_path = bin_dir.join(name);
        symlink_force(&entry, &link_path)?;
    }
    if release_dir.join("native").exists() {
        let native_link = install_root.join("native");
        symlink_force(&release_dir.join("native"), &native_link)?;
    }
    let artifacts_link = install_root.join("artifacts");
    if artifacts_link.exists() && !artifacts_link.is_symlink() {
        fs::remove_dir_all(&artifacts_link).ok();
    }
    symlink_force(&release_dir.join("artifacts"), &artifacts_link)?;
    link_static_assets(install_root, &release_dir)?;

    let config_before_write = config.clone();
    config.core_binary = bin_dir.join("core-server").display().to_string();
    config.sidecar_binary = bin_dir.join("telemetry-sidecar").display().to_string();
    let farmctl_dest = bin_dir.join("farmctl");
    let current_exe =
        std::env::current_exe().with_context(|| "Failed to resolve farmctl binary")?;
    let source_meta = fs::metadata(&current_exe)
        .with_context(|| format!("Failed to stat farmctl binary at {}", current_exe.display()))?;
    if !source_meta.is_file() || source_meta.len() == 0 {
        bail!(
            "farmctl binary at {} is empty or invalid",
            current_exe.display()
        );
    }
    let same_binary = fs::canonicalize(&current_exe)
        .ok()
        .zip(fs::canonicalize(&farmctl_dest).ok())
        .map(|(src, dest)| src == dest)
        .unwrap_or(false);
    if !same_binary {
        if farmctl_dest.exists() {
            fs::remove_file(&farmctl_dest).with_context(|| {
                format!(
                    "Failed to remove existing farmctl binary at {} before copy",
                    farmctl_dest.display()
                )
            })?;
        }
        let copied = fs::copy(&current_exe, &farmctl_dest).with_context(|| {
            format!(
                "Failed to copy farmctl binary to {}",
                farmctl_dest.display()
            )
        })?;
        if copied == 0 {
            bail!(
                "farmctl copy produced an empty binary at {}",
                farmctl_dest.display()
            );
        }
    }
    fs::set_permissions(&farmctl_dest, fs::Permissions::from_mode(0o755))?;
    let dest_meta = fs::metadata(&farmctl_dest)?;
    if dest_meta.len() == 0 {
        bail!(
            "farmctl binary at {} is empty after copy",
            farmctl_dest.display()
        );
    }
    config.farmctl_path = farmctl_dest.display().to_string();
    config.bundle_path = Some(fs::canonicalize(&args.bundle)?.display().to_string());

    let save_mode = match mode {
        InstallMode::Install => SaveConfigMode::Strict,
        InstallMode::Upgrade => SaveConfigMode::BestEffort,
    };
    let changed = config_before_write != config;
    let saved = save_config_if_changed(&config_path, &config_before_write, &config, save_mode)?;
    if matches!(save_mode, SaveConfigMode::BestEffort) && changed && !saved {
        eprintln!(
            "Warning: unable to update setup config at {} (permission denied); the upgrade may need to be rerun as the service user",
            config_path.display()
        );
    }

    prepare_native_services(&config)?;
    ensure_production_permissions(&config)?;
    if !env_flag("FARM_SETUP_SKIP_DB_INIT") {
        ensure_postgres_initialized(&config)?;
    }
    if !env_flag("FARM_SETUP_SKIP_LAUNCHD") {
        if config.profile == InstallProfile::Prod && unsafe { libc::geteuid() } != 0 {
            restart_application_services(&config)?;
        } else {
            let launchd_results = apply_launchd(&config)?;
            if !launchd_results_ok(&launchd_results) {
                let failures = launchd_results
                    .iter()
                    .filter(|result| !result.ok && !result.command.contains("launchctl bootout"))
                    .map(|result| {
                        let detail = if result.stderr.is_empty() {
                            result.stdout.as_str()
                        } else {
                            result.stderr.as_str()
                        };
                        format!(
                            "{} (exit {}): {}",
                            result.command, result.returncode, detail
                        )
                    })
                    .collect::<Vec<_>>();
                if failures.is_empty() {
                    bail!("Failed to apply launchd services; review setup logs");
                }
                bail!("Failed to apply launchd services:\n{}", failures.join("\n"));
            }
        }
    }
    if !env_flag("FARM_SETUP_SKIP_DB_INIT") {
        ensure_database_ready(&config)?;
        apply_migrations(&config, &release_dir.join("migrations"))?;

        if config.profile == InstallProfile::Prod {
            if let Some(credentials) = bootstrap_admin::ensure_bootstrap_admin(&config)? {
                println!(
                    "Bootstrap admin account created (first login requires credentials below):"
                );
                println!("  Email: {}", credentials.email);
                println!("  Temporary password: {}", credentials.password);
                println!("  Change this password after signing in (Dashboard → Users).");
            }
        }
    }

    let previous = state.current_version.clone();
    if previous.as_deref() != Some(&manifest.bundle_version) {
        state.previous_version = previous;
        state.current_version = Some(manifest.bundle_version.clone());
        state.installed_versions.push(InstalledRelease {
            version: manifest.bundle_version.clone(),
            installed_at: Utc::now().to_rfc3339(),
        });
        save_state(&config, &state)?;
    }

    match mode {
        InstallMode::Install => println!("Installed {}", manifest.bundle_version),
        InstallMode::Upgrade => println!("Upgraded to {}", manifest.bundle_version),
    }
    Ok(())
}

pub fn rollback(args: RollbackArgs, profile_override: Option<InstallProfile>) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let mut config = load_config(&config_path)?;
    let config_before_normalize = config.clone();
    normalize_config(&mut config, profile_override)?;
    let changed = config_before_normalize != config;
    let saved = save_config_if_changed(
        &config_path,
        &config_before_normalize,
        &config,
        SaveConfigMode::BestEffort,
    )?;
    if changed && !saved {
        eprintln!(
            "Warning: unable to update setup config at {} (permission denied); continuing with in-memory config",
            config_path.display()
        );
    }
    let install_root = Path::new(&config.install_root);
    let releases_root = install_root.join("releases");
    let mut state = load_state(&config)?;
    if config.profile == InstallProfile::Prod && unsafe { libc::geteuid() } != 0 {
        if state.current_version.is_none() {
            bail!("No existing production install found; rollback requires an existing install (run install via the installer app)");
        }
    }
    let target_version = if let Some(version) = &args.version {
        Some(version.clone())
    } else {
        state.previous_version.clone()
    };
    let target_version = target_version.context("No previous version available for rollback")?;
    let release_dir = releases_root.join(&target_version);
    if !release_dir.exists() {
        bail!(
            "Release {} not found at {}",
            target_version,
            release_dir.display()
        );
    }

    let manifest_path = release_dir.join(MANIFEST_NAME);
    let manifest_contents = fs::read_to_string(&manifest_path)
        .with_context(|| format!("Missing manifest at {}", manifest_path.display()))?;
    let manifest: BundleManifest = serde_json::from_str(&manifest_contents)?;

    let bin_dir = install_root.join("bin");
    fs::create_dir_all(&bin_dir)?;
    for component in &manifest.components {
        let entry = release_dir.join(&component.entrypoint);
        let name = Path::new(&component.entrypoint)
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or(&component.name);
        let link_path = bin_dir.join(name);
        symlink_force(&entry, &link_path)?;
    }
    link_static_assets(install_root, &release_dir)?;
    let artifacts_link = install_root.join("artifacts");
    if artifacts_link.exists() && !artifacts_link.is_symlink() {
        fs::remove_dir_all(&artifacts_link).ok();
    }
    symlink_force(&release_dir.join("artifacts"), &artifacts_link)?;

    prepare_native_services(&config)?;
    if !env_flag("FARM_SETUP_SKIP_LAUNCHD") {
        if config.profile == InstallProfile::Prod && unsafe { libc::geteuid() } != 0 {
            restart_application_services(&config)?;
        } else {
            let launchd_results = apply_launchd(&config)?;
            if !launchd_results_ok(&launchd_results) {
                bail!("Failed to apply launchd services; review setup logs");
            }
        }
    }

    state.previous_version = state.current_version.clone();
    state.current_version = Some(target_version.clone());
    save_state(&config, &state)?;
    println!("Rolled back to {}", target_version);
    Ok(())
}

fn restart_application_services(config: &crate::config::SetupConfig) -> Result<()> {
    if std::env::consts::OS != "macos" {
        bail!("Production restart is macOS-only");
    }
    let user = config.service_user.trim();
    if user.is_empty() {
        bail!("service_user must be set to restart services in production mode");
    }

    // Terminate native services first (postgres, redis, qdrant, mosquitto) so they reload
    // with fresh library paths after the native symlink changes. launchd KeepAlive restarts them.
    let postgres_bin = crate::paths::postgres_binary(config);
    for pattern in [
        postgres_bin.to_string_lossy().as_ref(),
        crate::paths::redis_binary(config)
            .to_string_lossy()
            .as_ref(),
        crate::paths::qdrant_binary(config)
            .to_string_lossy()
            .as_ref(),
        crate::paths::mosquitto_binary(config)
            .to_string_lossy()
            .as_ref(),
    ] {
        processes::terminate_processes(pattern, Some(user), false)?;
    }

    // Then terminate application services
    for binary in [config.core_binary.as_str(), config.sidecar_binary.as_str()] {
        processes::terminate_processes(binary, Some(user), false)?;
    }
    Ok(())
}

fn ensure_qdrant_launchd_present_for_upgrade(config: &crate::config::SetupConfig) -> Result<()> {
    let plan = generate_plan(config)?;
    if Path::new(&plan.target_dir) != Path::new("/Library/LaunchDaemons") {
        return Ok(());
    }
    let qdrant_entry = plan
        .plists
        .iter()
        .find(|entry| entry.label.ends_with(".qdrant"));
    let Some(entry) = qdrant_entry else {
        return Ok(());
    };
    if !Path::new(&entry.target_path).exists() {
        eprintln!(
            "Warning: Qdrant LaunchDaemon plist missing at {}. The controller may not have Qdrant managed by launchd until an admin install/repair is performed.",
            entry.target_path
        );
    }
    Ok(())
}

fn link_static_assets(install_root: &Path, release_dir: &Path) -> Result<()> {
    let static_dir = install_root.join("static");
    fs::create_dir_all(&static_dir)?;

    let dashboard_static_src = release_dir.join("artifacts/dashboard-web/static");
    if dashboard_static_src.exists() {
        symlink_force(&dashboard_static_src, &static_dir.join("dashboard-web"))?;
    }

    Ok(())
}

pub fn status(args: StatusArgs, profile_override: Option<InstallProfile>) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let mut config = load_config(&config_path)?;
    let config_before_normalize = config.clone();
    normalize_config(&mut config, profile_override)?;
    let changed = config_before_normalize != config;
    let saved = save_config_if_changed(
        &config_path,
        &config_before_normalize,
        &config,
        SaveConfigMode::BestEffort,
    )?;
    if changed && !saved {
        eprintln!(
            "Warning: unable to update setup config at {} (permission denied); continuing with in-memory config",
            config_path.display()
        );
    }
    let state = load_state(&config)?;
    let payload = serde_json::to_string_pretty(&state)?;
    println!("{}", payload);
    Ok(())
}

pub fn health(args: HealthArgs, profile_override: Option<InstallProfile>) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let mut config = load_config(&config_path)?;
    let config_before_normalize = config.clone();
    normalize_config(&mut config, profile_override)?;
    let changed = config_before_normalize != config;
    let saved = save_config_if_changed(
        &config_path,
        &config_before_normalize,
        &config,
        SaveConfigMode::BestEffort,
    )?;
    if changed && !saved {
        eprintln!(
            "Warning: unable to update setup config at {} (permission denied); continuing with in-memory config",
            config_path.display()
        );
    }
    let report = run_health_checks(&config)?;
    let ok = report.core_api.status == "ok"
        && report.dashboard.status == "ok"
        && report.mqtt.status == "ok"
        && report.database.status == "ok"
        && report.redis.status == "ok";
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "core_api: {} ({})",
            report.core_api.status, report.core_api.message
        );
        println!(
            "dashboard: {} ({})",
            report.dashboard.status, report.dashboard.message
        );
        println!("mqtt: {} ({})", report.mqtt.status, report.mqtt.message);
        println!(
            "database: {} ({})",
            report.database.status, report.database.message
        );
        println!("redis: {} ({})", report.redis.status, report.redis.message);
    }
    if !ok {
        bail!("health check failed");
    }
    Ok(())
}

pub fn diagnostics(args: DiagnosticsArgs, profile_override: Option<InstallProfile>) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    let mut config = load_config(&config_path)?;
    let config_before_normalize = config.clone();
    normalize_config(&mut config, profile_override)?;
    let changed = config_before_normalize != config;
    let saved = save_config_if_changed(
        &config_path,
        &config_before_normalize,
        &config,
        SaveConfigMode::BestEffort,
    )?;
    if changed && !saved {
        eprintln!(
            "Warning: unable to update setup config at {} (permission denied); continuing with in-memory config",
            config_path.display()
        );
    }
    let report = run_health_checks(&config)?;
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let output = args.output.unwrap_or_else(|| {
        Path::new(&config.data_root).join(format!("support_bundle_{timestamp}.zip"))
    });

    let mut file = fs::File::create(&output).with_context(|| {
        format!(
            "Failed to create diagnostics bundle at {}",
            output.display()
        )
    })?;
    let mut zip = zip::ZipWriter::new(&mut file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    if config_path.exists() {
        let contents = fs::read_to_string(&config_path)?;
        let redacted = if args.include_secrets {
            contents
        } else {
            redact_secrets(&contents)
        };
        write_zip_entry(&mut zip, "config.json", redacted.as_bytes(), options)?;
    }
    let report_json = serde_json::to_string_pretty(&report)?;
    write_zip_entry(&mut zip, "health.json", report_json.as_bytes(), options)?;

    let logs_root = Path::new(&config.logs_root);
    if logs_root.exists() {
        for entry in WalkDir::new(logs_root).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() {
                let rel = entry.path().strip_prefix(logs_root).unwrap_or(entry.path());
                let path = Path::new("logs").join(rel);
                let mut buffer = Vec::new();
                fs::File::open(entry.path())?.read_to_end(&mut buffer)?;
                write_zip_entry(&mut zip, &path.to_string_lossy(), &buffer, options)?;
            }
        }
    }

    zip.finish()?;
    println!("Diagnostics bundle written to {}", output.display());
    Ok(())
}
