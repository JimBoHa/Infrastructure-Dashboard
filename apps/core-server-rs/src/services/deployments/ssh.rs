use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine;
use sha2::{Digest, Sha256};
use ssh2::{CheckResult, HostKeyType, KnownHostFileKind, KnownHostKeyFormat, Session};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use super::types::PiDeploymentRequest;
use super::util::shell_quote;
use super::DeploymentManager;

pub(super) fn handshake_ssh(host: &str, port: u16) -> Result<Session> {
    let addr = format!("{host}:{port}");
    let tcp = TcpStream::connect(addr).context("Failed to open TCP connection")?;
    tcp.set_read_timeout(Some(Duration::from_secs(20))).ok();
    tcp.set_write_timeout(Some(Duration::from_secs(20))).ok();
    let mut session = Session::new().context("Failed to create SSH session")?;
    session.set_tcp_stream(tcp);
    session.handshake().context("SSH handshake failed")?;
    Ok(session)
}

pub(super) fn fingerprint_sha256(key: &[u8]) -> String {
    let digest = Sha256::digest(key);
    let b64 = STANDARD_NO_PAD.encode(digest);
    format!("SHA256:{b64}")
}

pub(super) fn host_key_type_to_name(key_type: HostKeyType) -> String {
    match key_type {
        HostKeyType::Rsa => "ssh-rsa",
        HostKeyType::Dss => "ssh-dss",
        HostKeyType::Ecdsa256 => "ecdsa-sha2-nistp256",
        HostKeyType::Ecdsa384 => "ecdsa-sha2-nistp384",
        HostKeyType::Ecdsa521 => "ecdsa-sha2-nistp521",
        HostKeyType::Ed25519 => "ssh-ed25519",
        HostKeyType::Unknown => "unknown",
    }
    .to_string()
}

pub(super) fn known_hosts_entry(
    host: &str,
    port: u16,
    key_type: HostKeyType,
    key: &[u8],
) -> String {
    let host_part = if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    };
    let algo = host_key_type_to_name(key_type);
    let key_b64 = STANDARD.encode(key);
    format!("{host_part} {algo} {key_b64}")
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }
    Ok(())
}

pub(super) fn upload_files(
    session: &mut Session,
    overlay_path: &Path,
    firstboot_json: Option<&[u8]>,
    env_bytes: Option<&[u8]>,
) -> Result<()> {
    let sftp = session.sftp().context("SFTP unavailable")?;

    {
        let mut local = fs::File::open(overlay_path)
            .with_context(|| format!("Failed to open {}", overlay_path.display()))?;
        let mut remote = sftp
            .create(Path::new("/tmp/node-agent-overlay.tar.gz"))
            .context("Failed to create remote overlay archive")?;
        std::io::copy(&mut local, &mut remote).context("Failed to upload overlay archive")?;
    }

    if let Some(firstboot_json) = firstboot_json {
        let mut remote = sftp
            .create(Path::new("/tmp/node-agent-firstboot.json"))
            .context("Failed to create remote firstboot file")?;
        remote.write_all(firstboot_json)?;
    }

    if let Some(env_bytes) = env_bytes {
        let mut remote = sftp
            .create(Path::new("/tmp/node-agent.env"))
            .context("Failed to create remote env file")?;
        remote.write_all(env_bytes)?;
    }
    Ok(())
}

pub(super) fn run_command(
    session: &mut Session,
    command: &str,
    check: bool,
    timeout: Option<Duration>,
) -> Result<String> {
    let mut channel = session
        .channel_session()
        .context("Failed to open SSH channel")?;
    if let Some(timeout) = timeout {
        session.set_timeout(timeout.as_millis().min(u128::from(u32::MAX)) as u32);
    }
    channel
        .exec(command)
        .with_context(|| format!("Failed to exec {command}"))?;
    let mut stdout = String::new();
    channel.read_to_string(&mut stdout).ok();
    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr).ok();
    channel.wait_close().ok();
    let exit = channel.exit_status().unwrap_or(-1);
    if !stderr.trim().is_empty() {
        stdout.push_str(&format!("\n{stderr}"));
    }
    if check && exit != 0 {
        return Err(anyhow!("Command failed ({exit}): {command}"));
    }
    Ok(stdout)
}

pub(super) fn run_sudo(
    session: &mut Session,
    password: &str,
    command: &str,
    timeout: Option<Duration>,
) -> Result<String> {
    let sudo_cmd = format!("sudo -S -p '' bash -c {}", shell_quote(command));
    let mut channel = session
        .channel_session()
        .context("Failed to open SSH channel")?;
    if let Some(timeout) = timeout {
        session.set_timeout(timeout.as_millis().min(u128::from(u32::MAX)) as u32);
    }
    channel
        .exec(&sudo_cmd)
        .context("Failed to exec sudo command")?;
    channel.write_all(format!("{password}\n").as_bytes()).ok();
    channel.send_eof().ok();
    let mut stdout = String::new();
    channel.read_to_string(&mut stdout).ok();
    let mut stderr = String::new();
    channel.stderr().read_to_string(&mut stderr).ok();
    channel.wait_close().ok();
    let exit = channel.exit_status().unwrap_or(-1);
    if exit != 0 {
        return Err(anyhow!("Sudo command failed ({exit})"));
    }
    if !stderr.trim().is_empty() {
        stdout.push_str(&format!("\n{stderr}"));
    }
    Ok(stdout)
}

impl DeploymentManager {
    pub(super) fn connect_ssh(
        &self,
        job_id: &str,
        request: &PiDeploymentRequest,
    ) -> Result<Session> {
        let session = handshake_ssh(&request.host, request.port)?;
        let (host_key, host_key_type) = session
            .host_key()
            .ok_or_else(|| anyhow!("SSH host key unavailable"))?;
        let fingerprint = fingerprint_sha256(host_key);

        let mut known_hosts = session.known_hosts()?;
        let known_hosts_path = &self.ssh_known_hosts_path;
        if known_hosts_path.exists() {
            known_hosts.read_file(known_hosts_path, KnownHostFileKind::OpenSSH)?;
        }
        let check = known_hosts.check_port(&request.host, request.port, host_key);
        match check {
            CheckResult::Match => {}
            CheckResult::NotFound => {
                let Some(approved) = request.host_key_fingerprint.as_ref() else {
                    return Err(anyhow!(
                        "SSH host key not trusted yet. Fetch the fingerprint first: {fingerprint}"
                    ));
                };
                if approved.trim() != fingerprint {
                    return Err(anyhow!(
                        "SSH host key fingerprint mismatch (expected {}, got {}).",
                        approved.trim(),
                        fingerprint
                    ));
                }
                ensure_parent_dir(known_hosts_path)?;
                known_hosts.add(
                    &if request.port == 22 {
                        request.host.clone()
                    } else {
                        format!("[{}]:{}", request.host, request.port)
                    },
                    host_key,
                    "farmdashboard trusted",
                    KnownHostKeyFormat::from(host_key_type),
                )?;
                known_hosts.write_file(known_hosts_path, KnownHostFileKind::OpenSSH)?;
                self.log_step(
                    job_id,
                    "Connect via SSH",
                    &format!("Trusted SSH host key: {fingerprint}"),
                );
            }
            CheckResult::Mismatch => {
                return Err(anyhow!(
                    "SSH host key mismatch for {}:{} (expected a different key). Refusing to connect. If you reflashed the Pi, remove its entry from {} and fetch the host key again.",
                    request.host,
                    request.port,
                    known_hosts_path.display()
                ));
            }
            CheckResult::Failure => {
                return Err(anyhow!(
                    "Unable to verify SSH host key for {}:{} (known_hosts: {}).",
                    request.host,
                    request.port,
                    known_hosts_path.display()
                ));
            }
        }

        if let Some(key) = request
            .ssh_private_key_pem
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            session
                .userauth_pubkey_memory(
                    &request.username,
                    None,
                    &key,
                    request.ssh_private_key_passphrase.as_deref(),
                )
                .context("SSH key authentication failed")?;
        } else {
            if request.password.trim().is_empty() {
                return Err(anyhow!(
                    "SSH password is required when no private key is provided"
                ));
            }
            session
                .userauth_password(&request.username, &request.password)
                .context("SSH authentication failed")?;
        }
        if !session.authenticated() {
            return Err(anyhow!("SSH authentication failed"));
        }
        Ok(session)
    }

    pub(super) fn run_logged_command(
        &self,
        job_id: &str,
        step_name: &str,
        session: &mut Session,
        command: &str,
        check: bool,
        timeout: Option<Duration>,
    ) -> Result<String> {
        self.log_step(job_id, step_name, &format!("$ {command}"));
        let output = run_command(session, command, check, timeout)?;
        for line in output.lines() {
            let line = line.trim_end();
            if !line.is_empty() {
                self.log_step(job_id, step_name, line);
            }
        }
        Ok(output)
    }

    pub(super) fn run_logged_sudo(
        &self,
        job_id: &str,
        step_name: &str,
        session: &mut Session,
        password: &str,
        command: &str,
        timeout: Option<Duration>,
    ) -> Result<()> {
        self.log_step(job_id, step_name, &format!("$ {command} (sudo)"));
        let output = if password.trim().is_empty() {
            run_command(
                session,
                &format!("sudo -n bash -c {}", shell_quote(command)),
                true,
                timeout,
            )?
        } else {
            run_sudo(session, password, command, timeout)?
        };
        for line in output.lines() {
            let line = line.trim_end();
            if !line.is_empty() {
                self.log_step(job_id, step_name, line);
            }
        }
        Ok(())
    }
}
