use anyhow::{anyhow, Context, Result};
use ssh2::Session;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

use super::types::PiDeploymentRequest;
use super::DeploymentManager;

fn resolve_socket_addrs(host: &str, port: u16) -> Result<Vec<SocketAddr>> {
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .with_context(|| format!("Failed to resolve {host}:{port}"))?
        .collect();
    if addrs.is_empty() {
        return Err(anyhow!("Unable to resolve {host}:{port}"));
    }
    Ok(addrs)
}

fn tcp_connect_any(addrs: &[SocketAddr], timeout: Duration) -> bool {
    addrs
        .iter()
        .any(|addr| TcpStream::connect_timeout(addr, timeout).is_ok())
}

fn wait_for_port(
    manager: &DeploymentManager,
    job_id: &str,
    step_name: &str,
    host: &str,
    port: u16,
    want_open: bool,
    timeout: Duration,
) -> Result<()> {
    let addrs = resolve_socket_addrs(host, port)?;
    let start = Instant::now();
    let mut last_log = Instant::now();
    loop {
        let open = tcp_connect_any(&addrs, Duration::from_secs(2));
        if open == want_open {
            return Ok(());
        }

        if start.elapsed() >= timeout {
            return Err(anyhow!(
                "Timed out waiting for {host}:{port} to become {}",
                if want_open {
                    "reachable"
                } else {
                    "unreachable"
                }
            ));
        }

        if last_log.elapsed() >= Duration::from_secs(10) {
            manager.log_step(
                job_id,
                step_name,
                &format!(
                    "Waiting for SSH {} ({:?} elapsed)…",
                    if want_open {
                        "to come back"
                    } else {
                        "to go down"
                    },
                    start.elapsed()
                ),
            );
            last_log = Instant::now();
        }
        thread::sleep(Duration::from_secs(2));
    }
}

impl DeploymentManager {
    pub(super) fn ensure_spi0_enabled(
        &self,
        job_id: &str,
        step_name: &str,
        mut session: Session,
        request: &PiDeploymentRequest,
    ) -> Result<Session> {
        let spidev0 = self.run_logged_command(
            job_id,
            step_name,
            &mut session,
            "if [ -e /dev/spidev0.0 ]; then echo '__SPI0_OK__'; else echo '__SPI0_MISSING__'; fi",
            false,
            Some(Duration::from_secs(5)),
        )?;
        if spidev0.contains("__SPI0_OK__") {
            return Ok(session);
        }

        self.log_step(
            job_id,
            step_name,
            "SPI0 device not detected (/dev/spidev0.0 missing). Enabling SPI in boot config and rebooting (one-time).",
        );

        let cfg_path = self
            .run_logged_command(
                job_id,
                step_name,
                &mut session,
                "if [ -f /boot/firmware/config.txt ]; then echo /boot/firmware/config.txt; elif [ -f /boot/config.txt ]; then echo /boot/config.txt; fi",
                false,
                Some(Duration::from_secs(5)),
            )?
            .trim()
            .to_string();
        if cfg_path.is_empty() {
            return Err(anyhow!(
                "Unable to find Raspberry Pi boot config.txt (tried /boot/firmware/config.txt and /boot/config.txt)"
            ));
        }
        self.log_step(job_id, step_name, &format!("Using boot config: {cfg_path}"));

        let state = self
            .run_logged_command(
                job_id,
                step_name,
                &mut session,
                &format!(
                    "cfg={cfg_path} \
                    && if grep -Eq '^\\\\s*dtparam=spi=on\\\\b' \"$cfg\"; then echo on; \
                    elif grep -Eq '^\\\\s*dtparam=spi=off\\\\b' \"$cfg\"; then echo off; \
                    elif grep -Eq '^\\\\s*#\\\\s*dtparam=spi=on\\\\b' \"$cfg\"; then echo commented; \
                    else echo missing; fi"
                ),
                false,
                Some(Duration::from_secs(5)),
            )?
            .trim()
            .to_string();

        match state.as_str() {
            "on" => {
                self.log_step(
                    job_id,
                    step_name,
                    "SPI is already enabled in boot config, but /dev/spidev0.0 is missing. Rebooting to apply firmware/kernel state.",
                );
            }
            "off" => {
                self.run_logged_sudo(
                    job_id,
                    step_name,
                    &mut session,
                    &request.password,
                    &format!(
                        "sed -i -E 's/^\\\\s*dtparam=spi=off\\\\b.*/dtparam=spi=on/' {cfg_path}"
                    ),
                    Some(Duration::from_secs(20)),
                )?;
            }
            "commented" => {
                self.run_logged_sudo(
                    job_id,
                    step_name,
                    &mut session,
                    &request.password,
                    &format!(
                        "sed -i -E 's/^\\\\s*#\\\\s*dtparam=spi=on\\\\b.*/dtparam=spi=on/' {cfg_path}"
                    ),
                    Some(Duration::from_secs(20)),
                )?;
            }
            "missing" | "" => {
                self.run_logged_sudo(
                    job_id,
                    step_name,
                    &mut session,
                    &request.password,
                    &format!(
                        "printf '\\n# FarmDashboard: enable SPI0 for optional ADS1263 ADC HAT\\ndtparam=spi=on\\n' >> {cfg_path}"
                    ),
                    Some(Duration::from_secs(20)),
                )?;
            }
            other => {
                self.log_step(
                    job_id,
                    step_name,
                    &format!("Unexpected SPI config state '{other}'. Proceeding with reboot."),
                );
            }
        }

        self.run_logged_sudo(
            job_id,
            step_name,
            &mut session,
            &request.password,
            "nohup bash -c \"sleep 1; reboot\" >/dev/null 2>&1 &",
            Some(Duration::from_secs(5)),
        )?;
        drop(session);

        if let Err(err) = wait_for_port(
            self,
            job_id,
            step_name,
            &request.host,
            request.port,
            false,
            Duration::from_secs(60),
        ) {
            self.log_step(job_id, step_name, &format!("Warning: {err:#}"));
        }
        wait_for_port(
            self,
            job_id,
            step_name,
            &request.host,
            request.port,
            true,
            Duration::from_secs(180),
        )
        .context("Node did not come back after reboot")?;

        let reconnect_start = Instant::now();
        let mut session = loop {
            match self.connect_ssh(job_id, request) {
                Ok(session) => break session,
                Err(err) => {
                    if reconnect_start.elapsed() >= Duration::from_secs(120) {
                        return Err(err).context("SSH reconnect after reboot failed");
                    }
                    thread::sleep(Duration::from_secs(2));
                }
            }
        };
        self.log_step(job_id, step_name, "Reconnected over SSH after reboot.");

        let spidev0 = self.run_logged_command(
            job_id,
            step_name,
            &mut session,
            "if [ -e /dev/spidev0.0 ]; then echo '__SPI0_OK__'; else echo '__SPI0_MISSING__'; fi",
            false,
            Some(Duration::from_secs(5)),
        )?;
        if !spidev0.contains("__SPI0_OK__") {
            return Err(anyhow!(
                "SPI0 is still unavailable after enabling dtparam and reboot. Expected /dev/spidev0.0. Verify the Pi 5 OS image supports SPI and that boot config changes are applied."
            ));
        }
        Ok(session)
    }
}
