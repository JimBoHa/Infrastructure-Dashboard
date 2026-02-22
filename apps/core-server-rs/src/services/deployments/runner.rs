use anyhow::{anyhow, Context, Result};
use std::time::Duration;

use super::db::{find_registered_node, issue_adoption_token};
use super::types::{DeploymentUserRef, JobStatus, PiDeploymentRequest};
use super::util::{
    build_env_file, build_firstboot_json, default_node_id, default_node_name,
    is_node_agent_healthy, read_mac, read_node_config_json,
};
use super::DeploymentManager;

fn parse_os_release_value(contents: &str, key: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        if k.trim() != key {
            continue;
        }
        let mut value = v.trim().to_string();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = value[1..value.len() - 1].to_string();
        }
        return Some(value);
    }
    None
}

fn normalize_python_major_minor(output: &str) -> Option<String> {
    let cleaned = output.trim();
    if cleaned.is_empty() {
        return None;
    }
    // Example outputs:
    // - "Python 3.11.2"
    // - "3.11"
    let cleaned = cleaned.strip_prefix("Python ").unwrap_or(cleaned).trim();
    let mut parts = cleaned.split('.');
    let major = parts.next()?.trim();
    let minor = parts.next()?.trim();
    if major.is_empty() || minor.is_empty() {
        return None;
    }
    Some(format!("{major}.{minor}"))
}

impl DeploymentManager {
    pub(super) fn run_pi5_deployment(
        &self,
        job_id: String,
        request: PiDeploymentRequest,
        user: DeploymentUserRef,
    ) {
        self.set_job_status(&job_id, JobStatus::Running, None);
        let result = self.execute_pi5_deployment(&job_id, &request, &user);
        match result {
            Ok(_) => self.set_job_status(&job_id, JobStatus::Success, None),
            Err(err) => {
                self.fail_running_steps(&job_id, &format!("{err:#}"));
                self.set_job_status(&job_id, JobStatus::Failed, Some(format!("{err:#}")));
            }
        }
    }

    fn execute_pi5_deployment(
        &self,
        job_id: &str,
        request: &PiDeploymentRequest,
        user: &DeploymentUserRef,
    ) -> Result<()> {
        self.start_step(job_id, "Prepare bundle");
        if !self.overlay_path.exists() {
            self.fail_step(job_id, "Prepare bundle", "node-agent overlay missing");
            return Err(anyhow!(
                "Node agent overlay not found at {}",
                self.overlay_path.display()
            ));
        }
        self.log_step(
            job_id,
            "Prepare bundle",
            &format!("Using overlay: {}", self.overlay_path.display()),
        );
        self.finish_step(job_id, "Prepare bundle");

        self.start_step(job_id, "Connect via SSH");
        let mut session = self
            .connect_ssh(job_id, request)
            .context("SSH connection failed")?;
        self.finish_step(job_id, "Connect via SSH");

        self.start_step(job_id, "Inspect node");
        self.log_step(
            job_id,
            "Inspect node",
            &format!("Connected to {} as {}.", request.host, request.username),
        );
        let os_release = self.run_logged_command(
            job_id,
            "Inspect node",
            &mut session,
            "cat /etc/os-release",
            false,
            None,
        );
        let machine = self.run_logged_command(
            job_id,
            "Inspect node",
            &mut session,
            "uname -m",
            false,
            None,
        );
        let python = self.run_logged_command(
            job_id,
            "Inspect node",
            &mut session,
            "python3 --version",
            false,
            None,
        );

        let os_release = os_release.unwrap_or_default();
        let machine = machine.unwrap_or_default();
        let python = python.unwrap_or_default();

        let arch = machine.trim();
        if arch != "aarch64" {
            let msg = format!(
                "Unsupported node architecture '{arch}'. Pi 5 deployments require aarch64 (Raspberry Pi OS Lite 64-bit)."
            );
            self.fail_step(job_id, "Inspect node", &msg);
            return Err(anyhow!(msg));
        }

        let py_mm = normalize_python_major_minor(&python).unwrap_or_default();
        let supported_python = ["3.11", "3.13"];
        if !supported_python.contains(&py_mm.as_str()) {
            let msg = format!(
                "Unsupported node Python version '{py_mm}'. Supported: 3.11 (Bookworm / Raspberry Pi OS Lite) and 3.13 (Trixie / Debian 13)."
            );
            self.fail_step(job_id, "Inspect node", &msg);
            return Err(anyhow!(msg));
        }

        if let Some(codename) = parse_os_release_value(&os_release, "VERSION_CODENAME") {
            let supported_os = ["bookworm", "trixie"];
            if !supported_os.contains(&codename.as_str()) {
                let msg = format!(
                    "Unsupported node OS codename '{codename}'. Supported: bookworm (Raspberry Pi OS Lite 64-bit) and trixie (Debian 13)."
                );
                self.fail_step(job_id, "Inspect node", &msg);
                return Err(anyhow!(msg));
            }
        }

        session = self
            .ensure_spi0_enabled(job_id, "Inspect node", session, request)
            .context("Failed to enable SPI0 on node")?;

        let mac_eth = read_mac(&mut session, "eth0").ok().flatten();
        let mac_wifi = read_mac(&mut session, "wlan0").ok().flatten();
        self.update_node_info(job_id, |node| {
            node.mac_eth = mac_eth.clone();
            node.mac_wifi = mac_wifi.clone();
        });

        let existing_node_config = read_node_config_json(&mut session).ok().flatten();
        let existing_node_id = existing_node_config
            .as_ref()
            .and_then(|v| v.get("node_id"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        let existing_node_name = existing_node_config
            .as_ref()
            .and_then(|v| v.get("node_name"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());

        let node_id = request
            .node_id
            .clone()
            .or(existing_node_id)
            .unwrap_or_else(|| {
                default_node_id(&request.host, mac_eth.as_deref(), mac_wifi.as_deref())
            });
        let node_name = request
            .node_name
            .clone()
            .or(existing_node_name)
            .unwrap_or_else(|| {
                default_node_name(&request.host, mac_eth.as_deref(), mac_wifi.as_deref())
            });
        self.update_node_info(job_id, |node| {
            node.node_id = Some(node_id.clone());
            node.node_name = Some(node_name.clone());
        });

        let already_healthy = is_node_agent_healthy(&mut session, self.node_agent_port);
        if already_healthy {
            self.set_job_outcome(job_id, Some("already_installed/healthy".to_string()));
        }

        self.finish_step(job_id, "Inspect node");

        let handle = tokio::runtime::Handle::current();
        let adoption_token = if let Some(token) = request.adoption_token.clone() {
            Some(token)
        } else {
            let existing_node = handle.block_on(find_registered_node(
                &self.db,
                mac_eth.as_deref(),
                mac_wifi.as_deref(),
            ))?;
            if let Some(existing_node) = existing_node {
                self.log_step(
                    job_id,
                    "Inspect node",
                    &format!(
                        "Node is already registered in the controller database ({}); skipping adoption token.",
                        existing_node
                    ),
                );
                None
            } else {
                Some(handle.block_on(issue_adoption_token(
                    &self.db,
                    mac_eth.as_deref(),
                    mac_wifi.as_deref(),
                    user,
                ))?)
            }
        };
        if let Some(token) = adoption_token.clone() {
            self.update_node_info(job_id, |node| {
                node.adoption_token = Some(token);
            });
        }

        if already_healthy
            && request.node_id.is_none()
            && request.node_name.is_none()
            && request.mqtt_url.is_none()
            && request.mqtt_username.is_none()
            && request.mqtt_password.is_none()
            && request.adoption_token.is_none()
        {
            self.log_step(
                job_id,
                "Upload bundle",
                "Skipping bundle upload; node-agent already healthy.",
            );
            self.finish_step(job_id, "Upload bundle");
            self.log_step(
                job_id,
                "Install node-agent",
                "Skipping install; node-agent already healthy.",
            );
            self.finish_step(job_id, "Install node-agent");
            self.log_step(
                job_id,
                "Start services",
                "Skipping service restart; node-agent already healthy.",
            );
            self.finish_step(job_id, "Start services");
            self.log_step(
                job_id,
                "Verify health",
                "Health check already passed during inspection.",
            );
            self.finish_step(job_id, "Verify health");
            return Ok(());
        }

        self.start_step(job_id, "Upload bundle");
        let firstboot_json = match adoption_token.clone() {
            Some(token) => Some(build_firstboot_json(&node_id, &node_name, &token)?),
            None => None,
        };
        let existing_env = super::ssh::run_command(
            &mut session,
            "cat /etc/node-agent.env 2>/dev/null || true",
            false,
            Some(Duration::from_secs(5)),
        )
        .unwrap_or_default();
        let existing_env = existing_env.trim();
        let env_bytes = build_env_file(
            &node_id,
            &node_name,
            request,
            if existing_env.is_empty() {
                None
            } else {
                Some(existing_env)
            },
            &self.default_mqtt_url,
            self.default_mqtt_username.as_deref(),
            self.default_mqtt_password.as_deref(),
        )?;
        super::ssh::upload_files(
            &mut session,
            &self.overlay_path,
            firstboot_json.as_deref(),
            Some(env_bytes.as_bytes()),
        )
        .context("Failed to upload deployment files")?;
        self.finish_step(job_id, "Upload bundle");

        self.start_step(job_id, "Install node-agent");
        self.run_logged_sudo(
            job_id,
            "Install node-agent",
            &mut session,
            &request.password,
            "rm -rf /opt/node-agent/debs \
              && tar -xzf /tmp/node-agent-overlay.tar.gz -C / \
              && rm -f /tmp/node-agent-overlay.tar.gz",
            Some(Duration::from_secs(300)),
        )
        .context("Failed to extract overlay")?;

        let setup_cmd = "set -euo pipefail \
            && if ! id -u farmnode >/dev/null 2>&1; then adduser --system --group --no-create-home farmnode; fi \
            && for group in bluetooth dialout gpio i2c spi; do if getent group \"$group\" >/dev/null 2>&1; then usermod -aG \"$group\" farmnode || true; fi; done \
            && mkdir -p /opt/node-agent/storage \
            && chown -R farmnode:farmnode /opt/node-agent \
            && if [ -f /tmp/node-agent.env ]; then cp /tmp/node-agent.env /etc/node-agent.env; fi \
            && if [ -f /tmp/node-agent-firstboot.json ]; then cp /tmp/node-agent-firstboot.json /opt/node-agent/storage/node-agent-firstboot.json; fi \
            && rm -f /tmp/node-agent.env /tmp/node-agent-firstboot.json \
            && chown -R farmnode:farmnode /opt/node-agent/storage";
        self.run_logged_sudo(
            job_id,
            "Install node-agent",
            &mut session,
            &request.password,
            setup_cmd,
            Some(Duration::from_secs(300)),
        )
        .context("Failed to configure node-agent paths")?;

        self.run_logged_sudo(
            job_id,
            "Install node-agent",
            &mut session,
            &request.password,
            "dpkg -P pigpio python3-rpi.gpio >/dev/null 2>&1 || true \
              && if [ -d /opt/node-agent/debs ] && ls /opt/node-agent/debs/*.deb >/dev/null 2>&1; then \
                dpkg -i /opt/node-agent/debs/*.deb || true; \
                dpkg --configure -a || true; \
              fi",
            Some(Duration::from_secs(900)),
        )
        .context("Failed to install offline node dependencies")?;

        self.finish_step(job_id, "Install node-agent");

        self.start_step(job_id, "Start services");
        self.run_logged_sudo(
            job_id,
            "Start services",
            &mut session,
            &request.password,
            "systemctl daemon-reload \
              && if systemctl list-unit-files | awk '{print $1}' | grep -qx 'pigpiod.service'; then systemctl enable --now pigpiod.service || true; fi \
              && systemctl enable node-forwarder.service node-agent.service node-agent-logrotate.timer node-agent-backup-verify.timer node-agent-optional-services.path \
              && systemctl restart node-forwarder.service \
              && systemctl restart node-agent.service \
              && (systemctl start node-agent-optional-services.service || true)",
            Some(Duration::from_secs(300)),
        )
        .context("Failed to restart services")?;
        self.finish_step(job_id, "Start services");

        self.start_step(job_id, "Verify health");
        self.run_logged_command(
            job_id,
            "Verify health",
            &mut session,
            &format!("curl -sf http://127.0.0.1:{}/healthz", self.node_agent_port),
            true,
            Some(Duration::from_secs(30)),
        )
        .context("Node agent health check failed")?;
        self.finish_step(job_id, "Verify health");

        if self.job_outcome(job_id).is_none() {
            self.set_job_outcome(job_id, Some("installed".to_string()));
        }
        Ok(())
    }
}
