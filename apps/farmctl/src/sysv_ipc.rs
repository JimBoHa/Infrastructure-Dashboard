use anyhow::{Context, Result};
use std::collections::HashSet;
use std::process::Command;

#[derive(Debug, Default, Clone)]
pub struct CleanupReport {
    pub removed_shared_memory: usize,
    pub removed_semaphores: usize,
}

fn parse_hex_key(value: &str) -> Option<u32> {
    let trimmed = value.trim();
    let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    u32::from_str_radix(without_prefix, 16).ok()
}

fn current_user() -> Option<String> {
    for key in ["SUDO_USER", "USER"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    let output = Command::new("id").arg("-un").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

/// Best-effort cleanup for stale System V IPC objects created by Postgres.
///
/// On macOS, repeated Postgres bootstrap runs can leave behind unattached SysV shared memory
/// segments and semaphores. These accumulate and eventually cause initdb failures like:
/// "could not create shared memory segment: No space left on device".
///
/// We only remove shared memory with `NATTCH=0` (no attached processes). Semaphore cleanup is
/// limited to keys adjacent to removed shared memory keys (Postgres convention).
pub fn cleanup_stale_postgres_ipc(owner_override: Option<&str>) -> Result<CleanupReport> {
    if std::env::consts::OS != "macos" {
        return Ok(CleanupReport::default());
    }

    let owner = owner_override
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(current_user);
    let Some(owner) = owner else {
        return Ok(CleanupReport::default());
    };

    let mut report = CleanupReport::default();

    let ipcs_shm = Command::new("ipcs")
        .args(["-m", "-o"])
        .output()
        .with_context(|| "Failed to run ipcs -m")?;
    if !ipcs_shm.status.success() {
        return Ok(report);
    }
    let shm_out = String::from_utf8_lossy(&ipcs_shm.stdout);

    let mut removed_shm_keys = HashSet::<u32>::new();

    for line in shm_out.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('m') {
            continue;
        }
        let parts = trimmed.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 7 {
            continue;
        }
        let id = parts[1];
        let key = parts[2];
        let line_owner = parts[4];
        let nattch = parts[6];
        if line_owner != owner {
            continue;
        }
        if nattch != "0" {
            continue;
        }

        let _ = Command::new("ipcrm").args(["-m", id]).status();
        report.removed_shared_memory += 1;
        if let Some(parsed) = parse_hex_key(key) {
            removed_shm_keys.insert(parsed);
        }
    }

    if removed_shm_keys.is_empty() {
        return Ok(report);
    }

    let mut sem_keys = HashSet::<u32>::new();
    for key in &removed_shm_keys {
        for offset in 1u32..=7u32 {
            sem_keys.insert(key.saturating_add(offset));
        }
    }

    let ipcs_sem = Command::new("ipcs")
        .args(["-s"])
        .output()
        .with_context(|| "Failed to run ipcs -s")?;
    if !ipcs_sem.status.success() {
        return Ok(report);
    }
    let sem_out = String::from_utf8_lossy(&ipcs_sem.stdout);
    for line in sem_out.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('s') {
            continue;
        }
        let parts = trimmed.split_whitespace().collect::<Vec<_>>();
        if parts.len() < 6 {
            continue;
        }
        let id = parts[1];
        let key = parts[2];
        let line_owner = parts[4];
        if line_owner != owner {
            continue;
        }
        let Some(parsed_key) = parse_hex_key(key) else {
            continue;
        };
        if !sem_keys.contains(&parsed_key) {
            continue;
        }

        let _ = Command::new("ipcrm").args(["-s", id]).status();
        report.removed_semaphores += 1;
    }

    Ok(report)
}
