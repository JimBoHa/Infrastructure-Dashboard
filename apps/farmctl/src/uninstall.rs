use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::UninstallArgs;
use crate::config::{
    load_config, normalize_config, resolve_config_path, save_config, setup_state_dir,
};
use crate::processes;
use crate::profile::InstallProfile;
use crate::sysv_ipc;
use crate::utils::{run_cmd_capture, CommandResult};

const SERVICE_SUFFIXES: &[&str] = &[
    "core-server",
    "telemetry-sidecar",
    "postgres",
    "redis",
    "mosquitto",
    "qdrant",
    "setup-daemon",
];

pub fn uninstall(args: UninstallArgs, profile_override: Option<InstallProfile>) -> Result<()> {
    let config_path = resolve_config_path(args.config);
    if !config_path.exists() {
        bail!("Config not found at {}", config_path.display());
    }

    let mut config = load_config(&config_path)?;
    normalize_config(&mut config, profile_override)?;
    save_config(&config_path, &config)?;

    if args.remove_roots && !args.yes {
        bail!("Refusing to remove install roots without --yes (use --remove-roots --yes)");
    }

    let target = launchctl_target(&config.profile);
    let target_dir = launchd_target_dir(&config.profile)?;
    let use_sudo = target.as_deref() == Some("system") && unsafe { libc::geteuid() } != 0;
    let process_user = process_owner(&config.profile, &config)?;

    // 1) Best-effort stop processes first so bootout doesn't leave behind live services if it fails.
    terminate_service_processes(&config, process_user.as_deref(), use_sudo)?;
    // Best-effort: clean up stale SysV IPC objects (Postgres) after stopping processes.
    // This keeps repeated E2E runs from exhausting macOS SysV IPC limits (initdb shmget ENOSPC).
    if config.profile == InstallProfile::E2e {
        let _ = sysv_ipc::cleanup_stale_postgres_ipc(process_user.as_deref());
    }

    // 2) Unload launchd jobs (bootout) before removing plists/roots.
    let mut results = Vec::new();

    for suffix in SERVICE_SUFFIXES {
        let label = label_for(&config.launchd_label_prefix, suffix);
        let plist_path = target_dir.join(format!("{label}.plist"));
        if let Some(target) = &target {
            results.extend(bootout_best_effort(target, &plist_path, &label)?);
        }
    }

    // 3) Final verification: ensure no leftover launchd labels for this install prefix.
    if let Some(target) = &target {
        let bootout_failures = results
            .iter()
            .filter(|result| result.command.contains("launchctl bootout"))
            .filter(|result| !result.ok && !is_nonfatal_bootout_error(result))
            .collect::<Vec<_>>();

        if launchctl_list_contains(&config.launchd_label_prefix, use_sudo)? {
            // Try one more time via service-targets (handles orphaned jobs where plists were removed).
            for suffix in SERVICE_SUFFIXES {
                let label = label_for(&config.launchd_label_prefix, suffix);
                let _ = bootout_service_target(target, &label, use_sudo);
            }
            if launchctl_list_contains(&config.launchd_label_prefix, use_sudo)? {
                bail!(
                    "Uninstall incomplete: launchd jobs still present for prefix {}{}",
                    config.launchd_label_prefix,
                    if bootout_failures.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "\n\nBootout failures:\n{}",
                            bootout_failures
                                .iter()
                                .map(|result| format!(
                                    "{}: {}",
                                    result.command,
                                    result.stderr.trim()
                                ))
                                .collect::<Vec<_>>()
                                .join("\n")
                        )
                    }
                );
            }
        }
    }

    // 4) Remove plist files after successful bootout + verification.
    for suffix in SERVICE_SUFFIXES {
        let label = label_for(&config.launchd_label_prefix, suffix);
        let plist_path = target_dir.join(format!("{label}.plist"));
        results.push(remove_plist(&plist_path, target.as_deref())?);
    }

    // 5) Optional: remove roots (explicitly requested).
    if args.remove_roots {
        results.push(remove_dir(
            &PathBuf::from(&config.install_root),
            target.as_deref(),
        )?);
        results.push(remove_dir(
            &PathBuf::from(&config.data_root),
            target.as_deref(),
        )?);
        results.push(remove_dir(
            &PathBuf::from(&config.logs_root),
            target.as_deref(),
        )?);
        results.push(remove_dir(&setup_state_dir(), target.as_deref())?);
    }

    println!("Uninstalled {}", config.install_root);
    Ok(())
}

fn label_for(prefix: &str, suffix: &str) -> String {
    let prefix = prefix.trim_end_matches('.');
    format!("{prefix}.{suffix}")
}

fn launchctl_target(profile: &InstallProfile) -> Option<String> {
    if std::env::var("FARM_SETUP_SKIP_LAUNCHCTL").is_ok() {
        return None;
    }
    if std::env::var("FARM_SETUP_LAUNCHD_ROOT").is_ok() {
        return None;
    }
    match profile {
        InstallProfile::Prod => Some("system".to_string()),
        InstallProfile::E2e => {
            let uid = unsafe { libc::getuid() };
            Some(format!("gui/{uid}"))
        }
    }
}

fn launchd_target_dir(profile: &InstallProfile) -> Result<PathBuf> {
    if let Ok(path) = std::env::var("FARM_SETUP_LAUNCHD_ROOT") {
        return Ok(PathBuf::from(path));
    }
    match profile {
        InstallProfile::Prod => Ok(PathBuf::from("/Library/LaunchDaemons")),
        InstallProfile::E2e => Ok(setup_state_dir().join("launchd")),
    }
}

fn bootout(target: &str, plist_path: &Path) -> Result<CommandResult> {
    if target == "system" && unsafe { libc::geteuid() } != 0 {
        let mut cmd = Command::new("sudo");
        cmd.arg("launchctl")
            .arg("bootout")
            .arg(target)
            .arg(plist_path);
        return run_cmd_capture(cmd);
    }
    let mut cmd = Command::new("launchctl");
    cmd.arg("bootout").arg(target).arg(plist_path);
    run_cmd_capture(cmd)
}

fn bootout_service_target(target: &str, label: &str, use_sudo: bool) -> Result<CommandResult> {
    let service_target = format!("{}/{}", target.trim_end_matches('/'), label);
    if use_sudo {
        let mut cmd = Command::new("sudo");
        cmd.arg("launchctl").arg("bootout").arg(&service_target);
        return run_cmd_capture(cmd);
    }
    let mut cmd = Command::new("launchctl");
    cmd.arg("bootout").arg(&service_target);
    run_cmd_capture(cmd)
}

fn bootout_best_effort(target: &str, plist_path: &Path, label: &str) -> Result<Vec<CommandResult>> {
    let mut attempts = Vec::new();
    if plist_path.exists() {
        let by_path = bootout(target, plist_path)?;
        if by_path.ok {
            return Ok(vec![by_path]);
        }
        let use_sudo = target == "system" && unsafe { libc::geteuid() } != 0;
        let by_target = bootout_service_target(target, label, use_sudo)?;
        if by_target.ok {
            return Ok(vec![by_target]);
        }
        attempts.push(by_path);
        attempts.push(by_target);
        return Ok(attempts);
    }

    let use_sudo = target == "system" && unsafe { libc::geteuid() } != 0;
    Ok(vec![bootout_service_target(target, label, use_sudo)?])
}

fn is_nonfatal_bootout_error(result: &CommandResult) -> bool {
    if result.ok {
        return false;
    }
    let combined = format!("{}\n{}", result.stdout, result.stderr).to_lowercase();
    combined.contains("no such process")
        || combined.contains("could not find service")
        || combined.contains("no such file")
}

fn launchctl_list_contains(prefix: &str, use_sudo: bool) -> Result<bool> {
    if prefix.trim().is_empty() {
        return Ok(false);
    }
    let output = if use_sudo {
        Command::new("sudo")
            .arg("launchctl")
            .arg("list")
            .output()
            .with_context(|| "Failed to run sudo launchctl list")?
    } else {
        Command::new("launchctl")
            .arg("list")
            .output()
            .with_context(|| "Failed to run launchctl list")?
    };
    if !output.status.success() {
        bail!(
            "launchctl list failed ({}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains(prefix))
}

fn process_owner(
    profile: &InstallProfile,
    config: &crate::config::SetupConfig,
) -> Result<Option<String>> {
    match profile {
        InstallProfile::Prod => {
            let user = config.service_user.trim();
            if user.is_empty() {
                return Ok(None);
            }
            Ok(Some(user.to_string()))
        }
        InstallProfile::E2e => Ok(Some(effective_user()?)),
    }
}

fn effective_user() -> Result<String> {
    for key in ["SUDO_USER", "USER"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim().to_string();
            if !trimmed.is_empty() {
                return Ok(trimmed);
            }
        }
    }
    let output = Command::new("id").arg("-un").output()?;
    if !output.status.success() {
        bail!(
            "id -un failed ({}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn terminate_service_processes(
    config: &crate::config::SetupConfig,
    user: Option<&str>,
    use_sudo: bool,
) -> Result<()> {
    // Note: the setup-daemon is `farmctl serve`; match on " serve " to avoid killing the uninstall command itself.
    let farmctl_serve_pattern = format!("{}.* serve ", config.farmctl_path);

    for pattern in [config.core_binary.as_str(), config.sidecar_binary.as_str()] {
        processes::terminate_processes(pattern, user, use_sudo)?;
    }

    // Native deps (paths may differ by distribution layout; match a couple stable patterns).
    let postgres = format!("{}/native/postgres/bin/postgres", config.install_root);
    let redis = format!("{}/native/redis/bin/redis-server", config.install_root);
    let mosquitto_bin = format!("{}/native/mosquitto/bin/mosquitto", config.install_root);
    let mosquitto_sbin = format!("{}/native/mosquitto/sbin/mosquitto", config.install_root);
    let qdrant = format!("{}/native/qdrant/bin/qdrant", config.install_root);

    for pattern in [
        postgres,
        redis,
        mosquitto_bin,
        mosquitto_sbin,
        qdrant,
        farmctl_serve_pattern,
    ] {
        processes::terminate_processes(&pattern, user, use_sudo)?;
    }

    Ok(())
}

fn remove_plist(plist_path: &Path, target: Option<&str>) -> Result<CommandResult> {
    if !plist_path.exists() {
        return Ok(CommandResult {
            command: format!("rm {}", plist_path.display()),
            ok: true,
            stdout: String::new(),
            stderr: String::new(),
            returncode: 0,
        });
    }
    if target == Some("system") && unsafe { libc::geteuid() } != 0 {
        let mut cmd = Command::new("sudo");
        cmd.arg("rm").arg("-f").arg(plist_path);
        return run_cmd_capture(cmd);
    }
    let mut cmd = Command::new("rm");
    cmd.arg("-f").arg(plist_path);
    run_cmd_capture(cmd)
}

fn remove_dir(path: &Path, target: Option<&str>) -> Result<CommandResult> {
    if !path.exists() {
        return Ok(CommandResult {
            command: format!("rm -rf {}", path.display()),
            ok: true,
            stdout: String::new(),
            stderr: String::new(),
            returncode: 0,
        });
    }
    if target == Some("system") && unsafe { libc::geteuid() } != 0 {
        let mut cmd = Command::new("sudo");
        cmd.arg("rm").arg("-rf").arg(path);
        return run_cmd_capture(cmd);
    }
    let mut cmd = Command::new("rm");
    cmd.arg("-rf").arg(path);
    run_cmd_capture(cmd)
}
