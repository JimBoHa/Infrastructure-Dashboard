use anyhow::Result;
use chrono::Utc;
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

use crate::services::mdns_iotnode::{parse_node_id_from_service_name, scan_iot_nodes, IotNodeCandidate};

#[derive(Debug, Clone)]
pub struct NodeAgentEndpoint {
    pub base_url: String,
    pub source: String,
    pub host: Option<String>,
    pub ip: Option<String>,
    pub ip_fallback: Option<String>,
}

#[derive(sqlx::FromRow)]
struct NodeLocatorRow {
    config: SqlJson<JsonValue>,
    ip_last: Option<String>,
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
}

fn normalize_mac_opt(value: Option<&str>) -> Option<String> {
    value
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
}

fn normalize_hostname(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Ok(parsed) = url::Url::parse(trimmed) {
            let host = parsed.host_str().unwrap_or("").trim_end_matches('.');
            if host.is_empty() {
                return None;
            }
            return Some(host.to_string());
        }
    }

    let trimmed = trimmed.trim_end_matches('/');
    let trimmed = trimmed.trim_end_matches('.');
    if trimmed.is_empty() {
        return None;
    }
    if let Some((host, port)) = trimmed.rsplit_once(':') {
        let host = host.trim_end_matches('.');
        if !host.contains(':') && !host.is_empty() && port.chars().all(|c| c.is_ascii_digit()) {
            return Some(host.to_string());
        }
    }
    Some(trimmed.to_string())
}

fn host_from_agent_node_id(value: &str) -> Option<String> {
    let node_id = value.trim().trim_end_matches('.').trim_end_matches('/');
    if node_id.is_empty() {
        return None;
    }
    if node_id.ends_with(".local") {
        Some(node_id.to_string())
    } else {
        Some(format!("{node_id}.local"))
    }
}

fn host_from_candidate(candidate: &IotNodeCandidate) -> Option<String> {
    if let Some(host) = candidate.hostname.as_deref().and_then(normalize_hostname) {
        return Some(host);
    }
    let node_id = parse_node_id_from_service_name(&candidate.service_name)?;
    host_from_agent_node_id(&node_id)
}

async fn load_locator(db: &PgPool, node_id: Uuid) -> Result<Option<NodeLocatorRow>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT
            COALESCE(config, '{}'::jsonb) as config,
            host(ip_last) as ip_last,
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(db)
    .await
}

async fn persist_node_agent_locator(
    db: &PgPool,
    node_id: Uuid,
    host: &str,
    source: &str,
    ip: Option<&str>,
) {
    let now = Utc::now().to_rfc3339();
    let _ = sqlx::query(
        r#"
        UPDATE nodes
        SET config = jsonb_set(
            jsonb_set(
                jsonb_set(
                    COALESCE(config, '{}'::jsonb),
                    '{node_agent,host}',
                    to_jsonb($2::text),
                    true
                ),
                '{node_agent,source}',
                to_jsonb($3::text),
                true
            ),
            '{node_agent,last_resolved_at}',
            to_jsonb($4::text),
            true
        ),
            ip_last = CASE WHEN $5::text IS NULL THEN ip_last ELSE $5::inet END,
            last_seen = COALESCE(last_seen, NOW())
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .bind(host)
    .bind(source)
    .bind(&now)
    .bind(ip)
    .execute(db)
    .await;
}

pub async fn resolve_node_agent_endpoint(
    db: &PgPool,
    node_id: Uuid,
    port: u16,
    force_rescan: bool,
) -> Result<Option<NodeAgentEndpoint>> {
    let row = load_locator(db, node_id).await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let ip_last = row.ip_last.as_deref().and_then(normalize_hostname);
    let ip_fallback = ip_last.as_ref().map(|ip| format!("http://{ip}:{port}"));

    let config = row.config.0;
    let cfg_host = config
        .get("node_agent")
        .and_then(|value| value.get("host"))
        .and_then(|value| value.as_str())
        .and_then(normalize_hostname);
    if let Some(ref host) = cfg_host {
        if !force_rescan {
            return Ok(Some(NodeAgentEndpoint {
                base_url: format!("http://{host}:{port}"),
                source: "config.node_agent.host".to_string(),
                host: Some(host.clone()),
                ip: row.ip_last.clone(),
                ip_fallback,
            }));
        }
    }

    let agent_node_id = config
        .get("agent_node_id")
        .and_then(|value| value.as_str())
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
    if let Some(agent_node_id) = agent_node_id {
        let host = host_from_agent_node_id(&agent_node_id);
        if let Some(host) = host {
            if cfg_host.as_deref() != Some(&host) {
                persist_node_agent_locator(db, node_id, &host, "agent_node_id", None).await;
            }
            if !force_rescan {
                return Ok(Some(NodeAgentEndpoint {
                    base_url: format!("http://{host}:{port}"),
                    source: "config.agent_node_id".to_string(),
                    host: Some(host),
                    ip: row.ip_last.clone(),
                    ip_fallback,
                }));
            }
        }
    }

    let mac_eth = normalize_mac_opt(row.mac_eth.as_deref());
    let mac_wifi = normalize_mac_opt(row.mac_wifi.as_deref());
    if mac_eth.is_some() || mac_wifi.is_some() {
        if let Ok(candidates) = scan_iot_nodes(Duration::from_secs(1)).await {
            let matched = candidates.into_iter().find(|candidate| {
                let candidate_eth = normalize_mac_opt(candidate.mac_eth.as_deref());
                let candidate_wifi = normalize_mac_opt(candidate.mac_wifi.as_deref());
                mac_eth
                    .as_deref()
                    .and_then(|expected| candidate_eth.as_deref().map(|seen| expected == seen))
                    .unwrap_or(false)
                    || mac_wifi
                        .as_deref()
                        .and_then(|expected| {
                            candidate_wifi.as_deref().map(|seen| expected == seen)
                        })
                        .unwrap_or(false)
            });

            if let Some(candidate) = matched {
                if let Some(host) = host_from_candidate(&candidate) {
                    persist_node_agent_locator(
                        db,
                        node_id,
                        &host,
                        "mdns",
                        candidate.ip.as_deref(),
                    )
                    .await;
                    return Ok(Some(NodeAgentEndpoint {
                        base_url: format!("http://{host}:{port}"),
                        source: "mdns".to_string(),
                        host: Some(host),
                        ip: candidate.ip.clone(),
                        ip_fallback,
                    }));
                }
            }
        }
    }

    if let Some(ip_last) = ip_last {
        return Ok(Some(NodeAgentEndpoint {
            base_url: format!("http://{ip_last}:{port}"),
            source: "nodes.ip_last".to_string(),
            host: None,
            ip: Some(ip_last),
            ip_fallback: None,
        }));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{host_from_agent_node_id, normalize_hostname};

    #[test]
    fn normalize_hostname_strips_scheme_and_port() {
        assert_eq!(
            normalize_hostname("http://pi-abc123.local:9000/"),
            Some("pi-abc123.local".to_string())
        );
        assert_eq!(
            normalize_hostname("https://pi-abc123.local:9000"),
            Some("pi-abc123.local".to_string())
        );
    }

    #[test]
    fn normalize_hostname_strips_trailing_dot_and_port_without_scheme() {
        assert_eq!(
            normalize_hostname("pi-abc123.local."),
            Some("pi-abc123.local".to_string())
        );
        assert_eq!(
            normalize_hostname("10.0.0.10:9000"),
            Some("10.0.0.10".to_string())
        );
    }

    #[test]
    fn host_from_agent_node_id_appends_local() {
        assert_eq!(
            host_from_agent_node_id("pi-abc123"),
            Some("pi-abc123.local".to_string())
        );
        assert_eq!(
            host_from_agent_node_id("pi-abc123.local"),
            Some("pi-abc123.local".to_string())
        );
    }
}
