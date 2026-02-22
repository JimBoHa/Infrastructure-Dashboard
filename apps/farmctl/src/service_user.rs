use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use crate::config::{setup_state_dir, SetupConfig};
use crate::profile::InstallProfile;
use crate::utils::{run_cmd, run_cmd_capture, which};

fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

pub fn ensure_production_user(config: &SetupConfig) -> Result<()> {
    if config.profile != InstallProfile::Prod {
        return Ok(());
    }
    if std::env::consts::OS != "macos" {
        bail!("Production installs are macOS-only");
    }

    let user = config.service_user.trim();
    let group = config.service_group.trim();
    if user.is_empty() || group.is_empty() {
        bail!("service_user/service_group must be set for production installs");
    }

    if which("dscl").is_none() {
        bail!("dscl not found; cannot manage service user");
    }

    let user_exists = dscl_exists("Users", user)?;
    let group_exists = dscl_exists("Groups", group)?;

    if !is_root() {
        if user_exists && group_exists {
            return Ok(());
        }
        bail!("Production service user/group missing; re-run install via the installer app (admin prompt) or sudo");
    }

    let gid = if group_exists {
        read_group_gid(group)?
            .with_context(|| format!("Missing PrimaryGroupID for group {group}"))?
    } else {
        let gid = allocate_free_id()?;
        create_group(group, gid)?;
        gid
    };

    if user_exists {
        set_user_primary_gid(user, gid)?;
    } else {
        let uid = if !group_exists {
            gid
        } else {
            allocate_free_id()?
        };
        create_user(user, uid, group, gid)?;
    }

    add_user_to_group(group, user)?;

    Ok(())
}

pub fn ensure_production_permissions(config: &SetupConfig) -> Result<()> {
    if config.profile != InstallProfile::Prod {
        return Ok(());
    }
    if std::env::consts::OS != "macos" {
        bail!("Production installs are macOS-only");
    }
    if !is_root() {
        return Ok(());
    }

    let user = config.service_user.trim();
    let group = config.service_group.trim();
    if user.is_empty() || group.is_empty() {
        bail!("service_user/service_group must be set for production installs");
    }

    for path in [
        Path::new(&config.install_root),
        Path::new(&config.data_root),
        Path::new(&config.logs_root),
        Path::new(&config.backup_root),
        setup_state_dir().as_path(),
    ] {
        if !path.exists() {
            continue;
        }
        chown_recursive(path, user, group)?;
    }
    Ok(())
}

pub fn chown_recursive(path: &Path, user: &str, group: &str) -> Result<()> {
    if !is_root() {
        bail!("chown requires root");
    }
    let spec = format!("{user}:{group}");
    let mut cmd = Command::new("chown");
    cmd.arg("-R").arg(spec).arg(path);
    run_cmd(cmd).with_context(|| format!("Failed to chown {}", path.display()))
}

pub fn chown_path(path: &Path, user: &str, group: &str) -> Result<()> {
    if !is_root() {
        bail!("chown requires root");
    }
    let spec = format!("{user}:{group}");
    let mut cmd = Command::new("chown");
    cmd.arg(spec).arg(path);
    run_cmd(cmd).with_context(|| format!("Failed to chown {}", path.display()))
}

pub fn lookup_uid_gid(user: &str) -> Result<(u32, u32)> {
    let uid = id_lookup("-u", user)?;
    let gid = id_lookup("-g", user)?;
    Ok((uid, gid))
}

fn dscl_exists(kind: &str, name: &str) -> Result<bool> {
    let mut cmd = Command::new("dscl");
    cmd.arg(".")
        .arg("-read")
        .arg(format!("/{kind}/{name}"))
        .arg("RecordName");
    let result = run_cmd_capture(cmd)?;
    Ok(result.ok)
}

fn read_group_gid(group: &str) -> Result<Option<u32>> {
    dscl_read_number("Groups", group, "PrimaryGroupID")
}

fn dscl_read_number(kind: &str, name: &str, attribute: &str) -> Result<Option<u32>> {
    dscl_read_value(kind, name, attribute)?
        .map(|value| {
            value
                .trim()
                .parse::<u32>()
                .with_context(|| format!("Invalid {attribute} for {kind}/{name}: {value}"))
        })
        .transpose()
}

fn dscl_read_value(kind: &str, name: &str, attribute: &str) -> Result<Option<String>> {
    let mut cmd = Command::new("dscl");
    cmd.arg(".")
        .arg("-read")
        .arg(format!("/{kind}/{name}"))
        .arg(attribute);
    let result = run_cmd_capture(cmd)?;
    if !result.ok {
        return Ok(None);
    }
    for line in result.stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(attribute) {
            if let Some((_, value)) = trimmed.split_once(':') {
                return Ok(Some(value.trim().to_string()));
            }
        }
    }
    Ok(None)
}

fn id_lookup(flag: &str, user: &str) -> Result<u32> {
    if which("id").is_none() {
        bail!("id not found; cannot resolve service user IDs");
    }
    let mut cmd = Command::new("id");
    cmd.arg(flag).arg(user);
    let result = run_cmd_capture(cmd)?;
    if !result.ok {
        bail!("id {flag} {user} failed: {}", result.stderr);
    }
    result
        .stdout
        .trim()
        .parse::<u32>()
        .with_context(|| format!("Invalid id output: {}", result.stdout))
}

fn allocate_free_id() -> Result<u32> {
    let used_uids = dscl_list_ids("Users", "UniqueID")?;
    let used_gids = dscl_list_ids("Groups", "PrimaryGroupID")?;
    for candidate in 220u32..500u32 {
        if used_uids.contains(&candidate) || used_gids.contains(&candidate) {
            continue;
        }
        return Ok(candidate);
    }
    bail!("Unable to allocate a free UID/GID for the service user")
}

fn dscl_list_ids(kind: &str, attribute: &str) -> Result<HashSet<u32>> {
    let mut cmd = Command::new("dscl");
    cmd.arg(".")
        .arg("-list")
        .arg(format!("/{kind}"))
        .arg(attribute);
    let result = run_cmd_capture(cmd)?;
    if !result.ok {
        bail!("dscl list failed: {}", result.stderr);
    }
    let mut ids = HashSet::new();
    for line in result.stdout.lines() {
        let mut parts = line.split_whitespace();
        let _name = parts.next();
        let id = parts.next();
        if let Some(id) = id {
            if let Ok(parsed) = id.parse::<u32>() {
                ids.insert(parsed);
            }
        }
    }
    Ok(ids)
}

fn create_group(group: &str, gid: u32) -> Result<()> {
    run_dscl(&[".", "-create", &format!("/Groups/{group}")])?;
    run_dscl(&[
        ".",
        "-create",
        &format!("/Groups/{group}"),
        "PrimaryGroupID",
        &gid.to_string(),
    ])?;
    run_dscl(&[
        ".",
        "-create",
        &format!("/Groups/{group}"),
        "RealName",
        "Farm Dashboard Service",
    ])?;
    run_dscl(&[".", "-create", &format!("/Groups/{group}"), "Password", "*"])?;
    Ok(())
}

fn create_user(user: &str, uid: u32, group: &str, gid: u32) -> Result<()> {
    run_dscl(&[".", "-create", &format!("/Users/{user}")])?;
    run_dscl(&[
        ".",
        "-create",
        &format!("/Users/{user}"),
        "UniqueID",
        &uid.to_string(),
    ])?;
    run_dscl(&[
        ".",
        "-create",
        &format!("/Users/{user}"),
        "PrimaryGroupID",
        &gid.to_string(),
    ])?;
    run_dscl(&[
        ".",
        "-create",
        &format!("/Users/{user}"),
        "UserShell",
        "/usr/bin/false",
    ])?;
    run_dscl(&[
        ".",
        "-create",
        &format!("/Users/{user}"),
        "NFSHomeDirectory",
        "/var/empty",
    ])?;
    run_dscl(&[
        ".",
        "-create",
        &format!("/Users/{user}"),
        "RealName",
        "Farm Dashboard Service",
    ])?;
    run_dscl(&[".", "-create", &format!("/Users/{user}"), "Password", "*"])?;

    // Ensure the group exists and the user is associated with it.
    add_user_to_group(group, user)?;
    Ok(())
}

fn set_user_primary_gid(user: &str, gid: u32) -> Result<()> {
    run_dscl(&[
        ".",
        "-create",
        &format!("/Users/{user}"),
        "PrimaryGroupID",
        &gid.to_string(),
    ])?;
    Ok(())
}

fn add_user_to_group(group: &str, user: &str) -> Result<()> {
    let mut cmd = Command::new("dscl");
    cmd.arg(".")
        .arg("-append")
        .arg(format!("/Groups/{group}"))
        .arg("GroupMembership")
        .arg(user);
    let _ = run_cmd_capture(cmd)?;
    Ok(())
}

fn run_dscl(args: &[&str]) -> Result<()> {
    let mut cmd = Command::new("dscl");
    for arg in args {
        cmd.arg(arg);
    }
    run_cmd(cmd).context("dscl command failed")
}
