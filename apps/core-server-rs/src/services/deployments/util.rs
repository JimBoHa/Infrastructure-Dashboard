use anyhow::{anyhow, Context, Result};
use rand::RngCore;
use serde_json::Value as JsonValue;
use ssh2::Session;
use std::time::Duration;

use super::ssh::run_command;
use super::types::{PiDeploymentRequest, MAX_LOG_LINES};

const MANAGED_ENV_KEYS: [&str; 5] = [
    "NODE_NODE_ID",
    "NODE_NODE_NAME",
    "NODE_MQTT_URL",
    "NODE_MQTT_USERNAME",
    "NODE_MQTT_PASSWORD",
];

pub(super) fn trim_logs(logs: Vec<String>) -> Vec<String> {
    if logs.len() <= MAX_LOG_LINES {
        return logs;
    }
    logs[logs.len() - MAX_LOG_LINES..].to_vec()
}

pub(super) fn random_hex(bytes: usize) -> String {
    let mut raw = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut raw);
    raw.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn validate_username(username: &str) -> Result<()> {
    let trimmed = username.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("Username is required"));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(anyhow!("Username contains unsupported characters"));
    }
    Ok(())
}

pub(super) fn read_mac(session: &mut Session, iface: &str) -> Result<Option<String>> {
    let output = run_command(
        session,
        &format!("cat /sys/class/net/{iface}/address"),
        false,
        Some(Duration::from_secs(5)),
    )?;
    let value = output.trim();
    if value.is_empty() {
        Ok(None)
    } else {
        Ok(Some(value.to_string()))
    }
}

pub(super) fn default_node_id(host: &str, mac_eth: Option<&str>, mac_wifi: Option<&str>) -> String {
    let suffix = suffix_from_mac(mac_eth)
        .or_else(|| suffix_from_mac(mac_wifi))
        .unwrap_or_else(|| slugify(host));
    if suffix.is_empty() {
        "pi5-node".to_string()
    } else {
        format!("pi5-{suffix}")
    }
}

pub(super) fn default_node_name(
    host: &str,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
) -> String {
    if let Some(suffix) = suffix_from_mac(mac_eth).or_else(|| suffix_from_mac(mac_wifi)) {
        return format!("Pi 5 Node {}", suffix.to_uppercase());
    }
    let cleaned = host.trim();
    if cleaned.is_empty() {
        "Pi 5 Node".to_string()
    } else {
        format!("Pi 5 Node {cleaned}")
    }
}

fn suffix_from_mac(mac: Option<&str>) -> Option<String> {
    let mac = mac?;
    let cleaned: String = mac.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if cleaned.is_empty() {
        return None;
    }
    let suffix = cleaned
        .chars()
        .rev()
        .take(6)
        .collect::<Vec<char>>()
        .into_iter()
        .rev()
        .collect::<String>();
    if suffix.is_empty() {
        None
    } else {
        Some(suffix)
    }
}

fn slugify(value: &str) -> String {
    let mut out = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            continue;
        }
        if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

pub(super) fn read_node_config_json(session: &mut Session) -> Result<Option<JsonValue>> {
    let output = run_command(
        session,
        "cat /opt/node-agent/storage/node_config.json",
        false,
        Some(Duration::from_secs(5)),
    )?;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let json: JsonValue =
        serde_json::from_str(trimmed).context("node_config.json is invalid JSON")?;
    Ok(Some(json))
}

pub(super) fn is_node_agent_healthy(session: &mut Session, port: u16) -> bool {
    verify_health(session, port).is_ok()
}

fn verify_health(session: &mut Session, port: u16) -> Result<()> {
    let cmd = format!("curl -sf http://127.0.0.1:{port}/healthz");
    let output = run_command(session, &cmd, false, Some(Duration::from_secs(30)))?;
    if output.trim().is_empty() {
        return Err(anyhow!("Health check returned empty response"));
    }
    Ok(())
}

pub(super) fn build_firstboot_json(
    node_id: &str,
    node_name: &str,
    adoption_token: &str,
) -> Result<Vec<u8>> {
    let payload = serde_json::json!({
        "node": {
            "node_id": node_id,
            "node_name": node_name,
            "adoption_token": adoption_token,
        }
    });
    Ok(serde_json::to_vec_pretty(&payload)?)
}

fn env_value_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

pub(super) fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn unquote_env_value(value: &str) -> String {
    let trimmed = value.trim();
    let Some(inner) = trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return trimmed.to_string();
    };

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn parse_existing_env(existing: &str) -> (std::collections::HashMap<String, String>, Vec<String>) {
    let mut values = std::collections::HashMap::new();
    let mut preserved = Vec::new();

    for raw_line in existing.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') {
            preserved.push(line.to_string());
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            preserved.push(line.to_string());
            continue;
        };

        let key = raw_key.trim().strip_prefix("export ").unwrap_or(raw_key).trim();
        if key.is_empty() {
            preserved.push(line.to_string());
            continue;
        }

        if MANAGED_ENV_KEYS.contains(&key) {
            values.insert(key.to_string(), unquote_env_value(raw_value));
        } else {
            preserved.push(line.to_string());
        }
    }

    (values, preserved)
}

pub(super) fn build_env_file(
    node_id: &str,
    node_name: &str,
    request: &PiDeploymentRequest,
    existing_env: Option<&str>,
    default_mqtt_url: &str,
    default_mqtt_username: Option<&str>,
    default_mqtt_password: Option<&str>,
) -> Result<String> {
    let (existing_values, preserved_lines) = existing_env
        .map(parse_existing_env)
        .unwrap_or_else(|| (Default::default(), Vec::new()));

    let mqtt_url = request
        .mqtt_url
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .or_else(|| existing_values.get("NODE_MQTT_URL").cloned())
        .or_else(|| {
            let trimmed = default_mqtt_url.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
    let mqtt_username = request
        .mqtt_username
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .or_else(|| existing_values.get("NODE_MQTT_USERNAME").cloned())
        .or_else(|| default_mqtt_username.map(|v| v.trim()).filter(|v| !v.is_empty()).map(|v| v.to_string()));
    let mqtt_password = request
        .mqtt_password
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .or_else(|| existing_values.get("NODE_MQTT_PASSWORD").cloned())
        .or_else(|| default_mqtt_password.map(|v| v.trim()).filter(|v| !v.is_empty()).map(|v| v.to_string()));

    let mut lines = Vec::new();
    lines.push(format!("NODE_NODE_ID={}", env_value_quote(node_id)));
    lines.push(format!("NODE_NODE_NAME={}", env_value_quote(node_name)));
    if let Some(url) = mqtt_url {
        lines.push(format!("NODE_MQTT_URL={}", env_value_quote(url.trim())));
    }
    if let Some(username) = mqtt_username {
        lines.push(format!(
            "NODE_MQTT_USERNAME={}",
            env_value_quote(username.trim())
        ));
    }
    if let Some(password) = mqtt_password {
        lines.push(format!(
            "NODE_MQTT_PASSWORD={}",
            env_value_quote(password.trim())
        ));
    }

    if !preserved_lines.is_empty() {
        lines.push(String::new());
        lines.extend(preserved_lines);
    }

    Ok(format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_request() -> PiDeploymentRequest {
        PiDeploymentRequest {
            host: "127.0.0.1".to_string(),
            port: 22,
            username: "pi".to_string(),
            password: "pw".to_string(),
            ssh_private_key_pem: None,
            ssh_private_key_passphrase: None,
            node_name: None,
            node_id: None,
            mqtt_url: None,
            mqtt_username: None,
            mqtt_password: None,
            adoption_token: None,
            host_key_fingerprint: None,
        }
    }

    #[test]
    fn build_env_file_uses_defaults_when_missing_everywhere() {
        let request = base_request();
        let env = build_env_file(
            "node-123",
            "Test Node",
            &request,
            None,
            "mqtt://core.local:1883",
            None,
            None,
        )
        .expect("build env");
        assert!(env.contains("NODE_NODE_ID=\"node-123\""));
        assert!(env.contains("NODE_NODE_NAME=\"Test Node\""));
        assert!(env.contains("NODE_MQTT_URL=\"mqtt://core.local:1883\""));
    }

    #[test]
    fn build_env_file_preserves_existing_mqtt_url_when_request_omits_it() {
        let request = base_request();
        let existing = r#"
NODE_NODE_ID="old"
NODE_MQTT_URL="mqtt://10.0.0.99:1883"
CUSTOM_FLAG=1
"#;
        let env = build_env_file(
            "node-123",
            "Test Node",
            &request,
            Some(existing),
            "mqtt://core.local:1883",
            None,
            None,
        )
        .expect("build env");
        assert!(env.contains("NODE_MQTT_URL=\"mqtt://10.0.0.99:1883\""));
        assert!(env.contains("CUSTOM_FLAG=1"));
    }

    #[test]
    fn build_env_file_request_overrides_existing_values() {
        let mut request = base_request();
        request.mqtt_url = Some("mqtt://override.local:1884".to_string());
        request.mqtt_username = Some("user".to_string());
        request.mqtt_password = Some("pass".to_string());

        let existing = r#"
NODE_MQTT_URL="mqtt://10.0.0.99:1883"
NODE_MQTT_USERNAME="old"
NODE_MQTT_PASSWORD="oldpass"
"#;

        let env = build_env_file(
            "node-123",
            "Test Node",
            &request,
            Some(existing),
            "mqtt://core.local:1883",
            Some("default-user"),
            Some("default-pass"),
        )
        .expect("build env");

        assert!(env.contains("NODE_MQTT_URL=\"mqtt://override.local:1884\""));
        assert!(env.contains("NODE_MQTT_USERNAME=\"user\""));
        assert!(env.contains("NODE_MQTT_PASSWORD=\"pass\""));
    }
}
