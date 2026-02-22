use chrono::{DateTime, Datelike, Timelike, Utc};
use sha2::{Digest, Sha256};

pub(crate) fn normalize_mac_hex(mac: Option<&str>) -> String {
    let Some(mac) = mac else {
        return "000000000000".to_string();
    };

    let cleaned: String = mac
        .chars()
        .filter(|ch| matches!(ch, '0'..='9' | 'a'..='f' | 'A'..='F'))
        .flat_map(|ch| ch.to_lowercase())
        .collect();

    if cleaned.is_empty() {
        return "000000000000".to_string();
    }

    let padded = format!("{:0>12}", cleaned);
    padded
        .chars()
        .rev()
        .take(12)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

pub(crate) fn deterministic_hex_id(
    kind: &str,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
    created_at: DateTime<Utc>,
    counter: u32,
) -> String {
    let timestamp = format!(
        "{:04}{:02}{:02}{:02}{:02}{:02}{:06}",
        created_at.year(),
        created_at.month(),
        created_at.day(),
        created_at.hour(),
        created_at.minute(),
        created_at.second(),
        created_at.timestamp_subsec_micros(),
    );

    let payload = [
        kind,
        &normalize_mac_hex(mac_eth),
        &normalize_mac_hex(mac_wifi),
        &timestamp,
        &format!("{counter:08x}"),
    ]
    .join("|");

    let digest = Sha256::digest(payload.as_bytes());
    let hex = format!("{digest:x}");
    hex.chars().take(24).collect()
}

pub(crate) fn stable_hex_id(namespace: &str, key: &str) -> String {
    let payload = [namespace.trim(), key.trim()].join("|");
    let digest = Sha256::digest(payload.as_bytes());
    let hex = format!("{digest:x}");
    hex.chars().take(24).collect()
}
