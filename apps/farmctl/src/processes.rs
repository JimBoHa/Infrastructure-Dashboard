use anyhow::{bail, Result};
use std::process::Command;
use std::time::Duration;

pub fn terminate_processes(pattern: &str, user: Option<&str>, use_sudo: bool) -> Result<()> {
    let self_pid = std::process::id() as i32;
    let pids = pgrep(pattern, user)?
        .into_iter()
        .filter(|pid| *pid != self_pid)
        .collect::<Vec<_>>();
    if pids.is_empty() {
        return Ok(());
    }

    for pid in &pids {
        let _ = run_kill(*pid, "TERM", use_sudo);
    }
    std::thread::sleep(Duration::from_millis(900));

    let still_running = pgrep(pattern, user)?
        .into_iter()
        .filter(|pid| *pid != self_pid)
        .collect::<Vec<_>>();
    for pid in &still_running {
        let _ = run_kill(*pid, "KILL", use_sudo);
    }
    Ok(())
}

pub fn pgrep(pattern: &str, user: Option<&str>) -> Result<Vec<i32>> {
    let mut cmd = Command::new("pgrep");
    if let Some(user) = user {
        if !user.trim().is_empty() {
            cmd.arg("-u").arg(user);
        }
    }
    let output = cmd.arg("-f").arg(pattern).output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut pids = Vec::new();
        for line in stdout.lines() {
            if let Ok(pid) = line.trim().parse::<i32>() {
                pids.push(pid);
            }
        }
        return Ok(pids);
    }

    if output.status.code() == Some(1) {
        return Ok(Vec::new());
    }

    // macOS pgrep returns exit code 2 when the specified user does not exist
    // (e.g., after a partial uninstall that removed the service account).
    // Treat that as "no processes" so `farmctl uninstall` can recover.
    if output.status.code() == Some(2) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Unknown user") {
            return Ok(Vec::new());
        }
    }

    bail!(
        "pgrep failed ({}): {}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

fn run_kill(pid: i32, signal: &str, use_sudo: bool) -> std::io::Result<()> {
    let mut cmd = if use_sudo {
        let mut cmd = Command::new("sudo");
        cmd.arg("kill");
        cmd
    } else {
        Command::new("kill")
    };
    cmd.arg(format!("-{signal}")).arg(pid.to_string());
    cmd.status().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pgrep_missing_user_is_not_fatal() {
        let result = pgrep(
            "farmdashboard-processes-test-pattern",
            Some("farmdashboard_test_no_such_user_123456789"),
        );
        assert!(result.is_ok(), "expected Ok(..), got {result:?}");
        assert_eq!(result.unwrap(), Vec::<i32>::new());
    }
}
