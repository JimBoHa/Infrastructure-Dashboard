use anyhow::Result;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::time::Duration;

pub const SERVICE_TYPE: &str = "_iotnode._tcp.local.";

#[derive(Debug, Clone)]
pub struct IotNodeCandidate {
    pub service_name: String,
    pub hostname: Option<String>,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub mac_eth: Option<String>,
    pub mac_wifi: Option<String>,
    pub properties: HashMap<String, String>,
}

pub fn parse_node_id_from_service_name(service_name: &str) -> Option<String> {
    let trimmed = service_name.trim().trim_end_matches('.');
    let suffix = format!(".{}", SERVICE_TYPE.trim_end_matches('.'));
    trimmed
        .strip_suffix(&suffix)
        .map(|prefix| prefix.to_string())
        .filter(|value| !value.trim().is_empty())
}

#[cfg(target_os = "macos")]
pub async fn scan_iot_nodes(timeout: Duration) -> Result<Vec<IotNodeCandidate>> {
    tokio::task::spawn_blocking(move || scan_iot_nodes_blocking(timeout)).await?
}

#[cfg(not(target_os = "macos"))]
pub async fn scan_iot_nodes(_timeout: Duration) -> Result<Vec<IotNodeCandidate>> {
    Ok(vec![])
}

#[cfg(target_os = "macos")]
fn scan_iot_nodes_blocking(timeout: Duration) -> Result<Vec<IotNodeCandidate>> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};
    use std::time::Instant;

    let mdns = ServiceDaemon::new()?;
    let receiver = mdns.browse(SERVICE_TYPE)?;
    let deadline = Instant::now() + timeout;
    let mut results: BTreeMap<String, IotNodeCandidate> = BTreeMap::new();

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline - now;
        let event = match receiver.recv_timeout(remaining) {
            Ok(event) => event,
            Err(_) => break,
        };
        if let ServiceEvent::ServiceResolved(resolved) = event {
            if !resolved.is_valid() {
                continue;
            }
            let ip = resolved
                .get_addresses_v4()
                .into_iter()
                .next()
                .map(|addr| addr.to_string());
            let props = resolved.txt_properties.clone().into_property_map_str();
            let mac_eth = props.get("mac_eth").cloned().filter(|v| !v.is_empty());
            let mac_wifi = props.get("mac_wifi").cloned().filter(|v| !v.is_empty());
            let candidate = IotNodeCandidate {
                service_name: resolved.fullname.clone(),
                hostname: Some(resolved.host.clone()),
                ip,
                port: Some(resolved.port),
                mac_eth,
                mac_wifi,
                properties: props,
            };
            results.insert(candidate.service_name.clone(), candidate);
        }
    }

    let _ = mdns.shutdown();
    Ok(results.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::parse_node_id_from_service_name;

    #[test]
    fn parses_node_id_from_service_name_with_trailing_dot() {
        assert_eq!(
            parse_node_id_from_service_name("pi-abc123._iotnode._tcp.local."),
            Some("pi-abc123".to_string())
        );
    }

    #[test]
    fn parses_node_id_from_service_name_without_trailing_dot() {
        assert_eq!(
            parse_node_id_from_service_name("pi-abc123._iotnode._tcp.local"),
            Some("pi-abc123".to_string())
        );
    }

    #[test]
    fn rejects_non_matching_service_names() {
        assert_eq!(parse_node_id_from_service_name(""), None);
        assert_eq!(parse_node_id_from_service_name("pi-abc123._http._tcp.local."), None);
    }
}
