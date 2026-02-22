use crate::config::Config;
use crate::ingest::TelemetryIngestor;
use anyhow::Result;
use chrono::Utc;
use std::collections::VecDeque;
use std::io;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use sysinfo::{Disks, System};
use tokio::time::MissedTickBehavior;

const CORE_NODE_ID: &str = "00000000-0000-0000-0000-000000000001";
const CORE_STATUS_INTERVAL: Duration = Duration::from_secs(5);
const SAMPLE_TIMEOUT: Duration = Duration::from_secs(2);
const PING_WINDOW_SECONDS: f64 = 60.0 * 30.0;
const UPTIME_WINDOW_SECONDS: f64 = 60.0 * 60.0 * 24.0;

pub async fn run(config: Config, ingestor: TelemetryIngestor) -> Result<()> {
    ingestor.ensure_core_node_record().await?;

    let mut ticker = tokio::time::interval(CORE_STATUS_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let mut ping_samples: VecDeque<(f64, f64)> = VecDeque::new();
    let mut ping_outcomes: VecDeque<(f64, bool)> = VecDeque::new();
    let mut mqtt_samples: VecDeque<(f64, f64)> = VecDeque::new();

    let mut system = System::new_all();
    let mut disks = Disks::new_with_refreshed_list();
    let started_at = Instant::now();

    loop {
        ticker.tick().await;
        let now = Utc::now();
        let now_secs = now.timestamp_millis() as f64 / 1000.0;

        system.refresh_cpu_all();
        system.refresh_memory();
        disks.refresh(true);

        let ping_host = config.mqtt_host.clone();
        let ping_ms = tokio::task::spawn_blocking(move || sample_icmp_ping_ms(&ping_host))
            .await
            .ok()
            .flatten();

        let tcp_host = config.mqtt_host.clone();
        let tcp_port = config.mqtt_port;
        let mqtt_rtt_ms = tokio::task::spawn_blocking(move || {
            sample_tcp_rtt_ms(&tcp_host, tcp_port, SAMPLE_TIMEOUT)
        })
        .await
        .ok()
        .flatten();

        ping_outcomes.push_back((now_secs, ping_ms.is_some()));
        if let Some(value) = ping_ms {
            ping_samples.push_back((now_secs, value));
        }
        if let Some(value) = mqtt_rtt_ms {
            mqtt_samples.push_back((now_secs, value));
        }

        trim_samples(&mut ping_outcomes, now_secs, UPTIME_WINDOW_SECONDS);
        trim_samples(&mut ping_samples, now_secs, PING_WINDOW_SECONDS);
        trim_samples(&mut mqtt_samples, now_secs, PING_WINDOW_SECONDS);

        let ping_values: Vec<f64> = ping_samples.iter().map(|(_, value)| *value).collect();
        let mqtt_values: Vec<f64> = mqtt_samples.iter().map(|(_, value)| *value).collect();

        let ping_latest = ping_samples.back().map(|(_, value)| *value);
        let mqtt_latest = mqtt_samples.back().map(|(_, value)| *value);
        let ping_jitter_ms = jitter_ms(&ping_values);
        let mqtt_jitter_ms = jitter_ms(&mqtt_values);
        let ping_p50_30m_ms = percentile(&ping_values, 0.5);
        let uptime_percent_24h = uptime_percent(&ping_outcomes);

        let cpu_percent = {
            let value = system.global_cpu_usage();
            if value.is_finite() && value >= 0.0 {
                Some(value)
            } else {
                None
            }
        };

        let cpu_percent_per_core = {
            let values: Vec<f32> = system
                .cpus()
                .iter()
                .map(|cpu| cpu.cpu_usage())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .collect();
            if values.is_empty() {
                None
            } else {
                Some(values)
            }
        };

        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        let memory_percent = if total_memory > 0 {
            Some(((used_memory as f64 / total_memory as f64) * 100.0) as f32)
        } else {
            None
        };
        let memory_used_bytes = i64::try_from(used_memory).ok();
        let storage_used_bytes = storage_used_bytes(&disks);
        let uptime_seconds = i64::try_from(System::uptime())
            .ok()
            .or_else(|| i64::try_from(started_at.elapsed().as_secs()).ok());

        if let Err(err) = ingestor
            .handle_node_status_payload(
                CORE_NODE_ID,
                "online",
                now,
                Some(now),
                uptime_seconds,
                cpu_percent,
                storage_used_bytes,
                Some(CORE_STATUS_INTERVAL.as_secs_f64()),
                None,
                cpu_percent_per_core,
                memory_percent,
                memory_used_bytes,
                ping_latest,
                ping_p50_30m_ms,
                ping_jitter_ms,
                mqtt_latest,
                mqtt_jitter_ms,
                uptime_percent_24h,
                None,
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %err, "core status heartbeat ingest failed");
        }
    }
}

fn sample_icmp_ping_ms(host: &str) -> Option<f64> {
    let host = host.trim();
    if host.is_empty() {
        return None;
    }

    let output = if cfg!(target_os = "macos") {
        let timeout_ms = SAMPLE_TIMEOUT.as_millis().max(250).to_string();
        Command::new("ping")
            .args(["-n", "-q", "-c", "1", "-W", timeout_ms.as_str(), host])
            .output()
            .ok()?
    } else {
        let timeout_secs = SAMPLE_TIMEOUT.as_secs().max(1).to_string();
        Command::new("ping")
            .args(["-n", "-q", "-c", "1", "-W", timeout_secs.as_str(), host])
            .output()
            .ok()?
    };

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_ping_output_ms(&stdout).or_else(|| parse_ping_output_ms(&stderr))
}

fn sample_tcp_rtt_ms(host: &str, port: u16, timeout: Duration) -> Option<f64> {
    let addrs: Vec<SocketAddr> = (host, port).to_socket_addrs().ok()?.collect();
    if addrs.is_empty() {
        return None;
    }

    for addr in addrs {
        let start = Instant::now();
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                let _ = stream.shutdown(std::net::Shutdown::Both);
                return Some(start.elapsed().as_secs_f64() * 1000.0);
            }
            Err(err) => {
                if err.kind() == io::ErrorKind::TimedOut {
                    continue;
                }
            }
        }
    }
    None
}

fn parse_ping_output_ms(output: &str) -> Option<f64> {
    if let Some(idx) = output.find("time=") {
        let rest = &output[idx + 5..];
        let mut raw = String::new();
        for ch in rest.chars() {
            if ch.is_ascii_digit() || ch == '.' {
                raw.push(ch);
            } else {
                break;
            }
        }
        if let Ok(value) = raw.parse::<f64>() {
            if value.is_finite() && value >= 0.0 {
                return Some(value);
            }
        }
    }

    if let Some(idx) = output.find("min/avg/max/") {
        let rest = &output[idx..];
        let eq_idx = rest.find('=')?;
        let stats = rest[eq_idx + 1..].trim();
        let parts: Vec<&str> = stats.split('/').collect();
        if parts.len() >= 2 {
            if let Ok(value) = parts[1].trim().parse::<f64>() {
                if value.is_finite() && value >= 0.0 {
                    return Some(value);
                }
            }
        }
    }

    None
}

fn storage_used_bytes(disks: &Disks) -> Option<i64> {
    let disk = disks
        .list()
        .iter()
        .find(|disk| disk.mount_point() == Path::new("/"))
        .or_else(|| disks.list().first())?;
    let total = disk.total_space();
    let avail = disk.available_space();
    let used = total.saturating_sub(avail);
    i64::try_from(used).ok()
}

fn trim_samples<T>(samples: &mut VecDeque<(f64, T)>, now_secs: f64, window_seconds: f64) {
    while samples
        .front()
        .is_some_and(|(ts, _)| now_secs - *ts > window_seconds)
    {
        samples.pop_front();
    }
}

fn jitter_ms(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    if values.len() < 2 {
        return Some(0.0);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - mean;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64;
    Some(variance.max(0.0).sqrt())
}

fn percentile(values: &[f64], quantile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let q = quantile.clamp(0.0, 1.0);
    if ordered.len() == 1 {
        return ordered.first().copied();
    }
    let idx = (ordered.len() - 1) as f64 * q;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        return ordered.get(lo).copied();
    }
    let fraction = idx - lo as f64;
    let lower = ordered.get(lo).copied()?;
    let upper = ordered.get(hi).copied()?;
    Some(lower + (upper - lower) * fraction)
}

fn uptime_percent(outcomes: &VecDeque<(f64, bool)>) -> Option<f32> {
    if outcomes.is_empty() {
        return None;
    }
    let attempts = outcomes.len() as f64;
    let successes = outcomes.iter().filter(|(_, ok)| *ok).count() as f64;
    Some(((successes / attempts) * 100.0) as f32)
}
