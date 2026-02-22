use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::cli::{
    DevActivityArgs, DevActivityCommand, DevActivityStartArgs, DevActivityStatusArgs,
};

#[derive(Debug, Clone, Serialize)]
struct DevActivityHeartbeatRequest {
    ttl_seconds: u64,
    message: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DevActivityStatusResponse {
    active: bool,
    expires_at: Option<String>,
}

fn resolve_base_url(override_url: Option<String>) -> String {
    if let Some(url) = override_url {
        let trimmed = url.trim().trim_end_matches('/').to_string();
        if !trimmed.is_empty() {
            return trimmed;
        }
    }
    let config_path = crate::config::default_config_path();
    if config_path.exists() {
        if let Ok(contents) = fs::read_to_string(&config_path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) {
                let port = value
                    .get("core_port")
                    .and_then(|v| v.as_u64())
                    .filter(|v| *v > 0 && *v <= u16::MAX as u64)
                    .map(|v| v as u16)
                    .unwrap_or(8000);
                return format!("http://127.0.0.1:{port}");
            }
        }
    }
    "http://127.0.0.1:8000".to_string()
}

fn handle_start(base: &str, args: DevActivityStartArgs) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let payload = DevActivityHeartbeatRequest {
        ttl_seconds: args.ttl_seconds.clamp(5, 24 * 60 * 60),
        message: args.message,
        source: args.source,
    };
    let body =
        serde_json::to_vec(&payload).context("failed to serialize dev activity heartbeat")?;
    let response = client
        .post(format!("{base}/api/dev/activity/heartbeat"))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .context("failed to call dev activity heartbeat")?;

    let http_status = response.status();
    let text = response.text().unwrap_or_default();
    if !http_status.is_success() {
        anyhow::bail!("dev activity heartbeat failed: {} {}", http_status, text);
    }
    let status: DevActivityStatusResponse =
        serde_json::from_str(&text).context("failed to parse dev activity response")?;
    if status.active {
        let expires_at = status.expires_at.unwrap_or_else(|| "<unknown>".to_string());
        println!("dev activity: active (expires at {expires_at})");
    } else {
        println!("dev activity: inactive");
    }
    Ok(())
}

fn handle_status(base: &str, args: DevActivityStatusArgs) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(format!("{base}/api/dev/activity"))
        .send()
        .context("failed to fetch dev activity status")?;
    let http_status = response.status();
    let text = response.text().unwrap_or_default();
    if !http_status.is_success() {
        anyhow::bail!("dev activity status failed: {} {}", http_status, text);
    }

    if args.json {
        let value: serde_json::Value =
            serde_json::from_str(&text).context("failed to parse status JSON")?;
        println!(
            "{}",
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
        );
        return Ok(());
    }

    let status: DevActivityStatusResponse =
        serde_json::from_str(&text).context("failed to parse dev activity status")?;
    if status.active {
        let expires_at = status.expires_at.unwrap_or_else(|| "<unknown>".to_string());
        println!("dev activity: active (expires at {expires_at})");
    } else {
        println!("dev activity: inactive");
    }
    Ok(())
}

pub fn handle(args: DevActivityArgs) -> Result<()> {
    let base = resolve_base_url(args.core_url);
    match args.command {
        DevActivityCommand::Start(cmd) => handle_start(&base, cmd),
        DevActivityCommand::Stop => {
            let client = reqwest::blocking::Client::new();
            let response = client
                .delete(format!("{base}/api/dev/activity"))
                .send()
                .context("failed to clear dev activity")?;
            let http_status = response.status();
            let text = response.text().unwrap_or_default();
            if !http_status.is_success() {
                anyhow::bail!("dev activity clear failed: {} {}", http_status, text);
            }
            println!("dev activity: cleared");
            Ok(())
        }
        DevActivityCommand::Status(cmd) => handle_status(&base, cmd),
    }
}
