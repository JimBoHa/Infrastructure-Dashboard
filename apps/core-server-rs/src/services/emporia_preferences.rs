use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};

use crate::services::emporia::EmporiaDeviceInfo;
use crate::services::emporia::EmporiaDeviceReading;
use crate::services::emporia::EmporiaUsageAggregate;

pub const EMPORIA_CIRCUIT_KEY_MAINS: &str = "mains";

#[derive(Debug, Clone)]
pub struct EmporiaCircuitPreferences {
    pub enabled: bool,
    pub hidden: bool,
    pub include_in_power_summary: bool,
}

#[derive(Debug, Clone)]
pub struct EmporiaDevicePreferences {
    pub enabled: bool,
    pub hidden: bool,
    pub include_in_power_summary: bool,
    pub group_label: Option<String>,
    pub circuits: HashMap<String, EmporiaCircuitPreferences>,
}

pub fn merge_emporia_device_preferences(
    devices: &[EmporiaDeviceInfo],
    metadata: &mut JsonValue,
    legacy_site_ids: &[String],
) -> (
    HashMap<String, EmporiaDevicePreferences>,
    Vec<String>,
    HashSet<String>,
) {
    let mut prefs: HashMap<String, EmporiaDevicePreferences> =
        parse_emporia_device_preferences(metadata);

    let legacy_allowlist: HashSet<String> = legacy_site_ids.iter().cloned().collect();
    let has_legacy_allowlist = !legacy_allowlist.is_empty();

    for device in devices {
        let entry = prefs.entry(device.device_gid.clone()).or_insert_with(|| {
            let group_label = device
                .address
                .clone()
                .or_else(|| device.name.clone())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            EmporiaDevicePreferences {
                enabled: true,
                hidden: false,
                include_in_power_summary: if has_legacy_allowlist {
                    legacy_allowlist.contains(&device.device_gid)
                } else {
                    true
                },
                group_label,
                circuits: HashMap::new(),
            }
        });

        entry
            .circuits
            .entry(EMPORIA_CIRCUIT_KEY_MAINS.to_string())
            .or_insert_with(|| EmporiaCircuitPreferences {
                enabled: true,
                hidden: false,
                include_in_power_summary: true,
            });

        if entry.group_label.as_deref().unwrap_or("").trim().is_empty() {
            entry.group_label = device
                .address
                .clone()
                .or_else(|| device.name.clone())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
    }

    let mut enabled_device_gids: Vec<String> = devices
        .iter()
        .filter_map(|device| {
            prefs
                .get(&device.device_gid)
                .filter(|entry| entry.enabled)
                .map(|_| device.device_gid.clone())
        })
        .collect();
    enabled_device_gids.sort();
    enabled_device_gids.dedup();

    let included_device_gids: HashSet<String> = devices
        .iter()
        .filter_map(|device| {
            prefs.get(&device.device_gid).and_then(|entry| {
                if entry.enabled && entry.include_in_power_summary {
                    Some(device.device_gid.clone())
                } else {
                    None
                }
            })
        })
        .collect();

    metadata["site_ids"] = JsonValue::Array(
        enabled_device_gids
            .iter()
            .map(|id| JsonValue::String(id.clone()))
            .collect(),
    );

    let mut devices_json = serde_json::Map::new();
    for (gid, entry) in &prefs {
        let mut circuits_json = serde_json::Map::new();
        for (circuit_key, circuit) in &entry.circuits {
            circuits_json.insert(
                circuit_key.clone(),
                json!({
                    "enabled": circuit.enabled,
                    "hidden": circuit.hidden,
                    "include_in_power_summary": circuit.include_in_power_summary,
                }),
            );
        }
        devices_json.insert(
            gid.clone(),
            json!({
                "enabled": entry.enabled,
                "hidden": entry.hidden,
                "include_in_power_summary": entry.include_in_power_summary,
                "group_label": entry.group_label,
                "circuits": JsonValue::Object(circuits_json),
            }),
        );
    }
    metadata["devices"] = JsonValue::Object(devices_json);

    (prefs, enabled_device_gids, included_device_gids)
}

pub fn parse_emporia_device_preferences(
    metadata: &JsonValue,
) -> HashMap<String, EmporiaDevicePreferences> {
    let mut prefs = HashMap::new();
    let Some(devices_obj) = metadata.get("devices").and_then(|v| v.as_object()) else {
        return prefs;
    };

    for (device_gid, entry) in devices_obj {
        let enabled = entry
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let hidden = entry
            .get("hidden")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let include_in_power_summary = entry
            .get("include_in_power_summary")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let group_label = entry
            .get("group_label")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let mut circuits = HashMap::new();
        if let Some(circuits_obj) = entry.get("circuits").and_then(|v| v.as_object()) {
            for (circuit_key, circuit_entry) in circuits_obj {
                let circuit_enabled = circuit_entry
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let circuit_hidden = circuit_entry
                    .get("hidden")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let circuit_include_in_power_summary = circuit_entry
                    .get("include_in_power_summary")
                    .and_then(|v| v.as_bool())
                    .unwrap_or_else(|| circuit_key.as_str() == EMPORIA_CIRCUIT_KEY_MAINS);
                circuits.insert(
                    circuit_key.clone(),
                    EmporiaCircuitPreferences {
                        enabled: circuit_enabled,
                        hidden: circuit_hidden,
                        include_in_power_summary: circuit_include_in_power_summary,
                    },
                );
            }
        }
        circuits
            .entry(EMPORIA_CIRCUIT_KEY_MAINS.to_string())
            .or_insert_with(|| EmporiaCircuitPreferences {
                enabled: true,
                hidden: false,
                include_in_power_summary: true,
            });

        prefs.insert(
            device_gid.clone(),
            EmporiaDevicePreferences {
                enabled,
                hidden,
                include_in_power_summary,
                group_label,
                circuits,
            },
        );
    }

    prefs
}

pub fn derive_emporia_power_summary(
    usage: &EmporiaUsageAggregate,
    included_device_gids: &HashSet<String>,
    prefs: &HashMap<String, EmporiaDevicePreferences>,
) -> (f64, f64) {
    let mut summary_kw = 0.0_f64;
    let mut summary_consumption_kwh = 0.0_f64;

    for device in &usage.devices {
        if !included_device_gids.contains(&device.device_gid) {
            continue;
        }

        let device_prefs = prefs.get(&device.device_gid);
        summary_kw +=
            (compute_emporia_device_summary_power_w(device, device_prefs) / 1000.0).max(0.0);
        summary_consumption_kwh +=
            compute_emporia_device_summary_energy_kwh(device, device_prefs).max(0.0);
    }

    (summary_kw, summary_consumption_kwh)
}

pub fn merge_emporia_circuit_preferences(
    usage: &EmporiaUsageAggregate,
    prefs: &mut HashMap<String, EmporiaDevicePreferences>,
    metadata: &mut JsonValue,
) {
    let Some(devices_obj) = metadata.get_mut("devices").and_then(|v| v.as_object_mut()) else {
        return;
    };

    for device in &usage.devices {
        let Some(device_prefs) = prefs.get_mut(&device.device_gid) else {
            continue;
        };

        for channel in &device.channels {
            let circuit_key = channel.channel_num.trim();
            if circuit_key.is_empty() {
                continue;
            }

            device_prefs
                .circuits
                .entry(circuit_key.to_string())
                .or_insert_with(|| EmporiaCircuitPreferences {
                    enabled: true,
                    hidden: false,
                    include_in_power_summary: false,
                });
        }

        device_prefs
            .circuits
            .entry(EMPORIA_CIRCUIT_KEY_MAINS.to_string())
            .or_insert_with(|| EmporiaCircuitPreferences {
                enabled: true,
                hidden: false,
                include_in_power_summary: true,
            });

        let mut circuits_json = serde_json::Map::new();
        for (circuit_key, circuit) in &device_prefs.circuits {
            circuits_json.insert(
                circuit_key.clone(),
                json!({
                    "enabled": circuit.enabled,
                    "hidden": circuit.hidden,
                    "include_in_power_summary": circuit.include_in_power_summary,
                }),
            );
        }

        if let Some(entry) = devices_obj.get_mut(&device.device_gid) {
            entry["circuits"] = JsonValue::Object(circuits_json);
        }
    }
}

pub fn compute_emporia_device_summary_power_w(
    device: &EmporiaDeviceReading,
    prefs: Option<&EmporiaDevicePreferences>,
) -> f64 {
    let default_mains = EmporiaCircuitPreferences {
        enabled: true,
        hidden: false,
        include_in_power_summary: true,
    };

    let mains_pref = prefs
        .and_then(|entry| entry.circuits.get(EMPORIA_CIRCUIT_KEY_MAINS))
        .unwrap_or(&default_mains);

    if mains_pref.enabled && mains_pref.include_in_power_summary {
        return device.main_power_w.max(0.0);
    }

    let mut total_w = 0.0_f64;
    for channel in &device.channels {
        let power_w = channel
            .power_w
            .or_else(|| match (channel.voltage_v, channel.current_a) {
                (Some(voltage_v), Some(current_a)) => Some(voltage_v * current_a),
                _ => None,
            });
        let Some(power_w) = power_w else {
            continue;
        };
        let Some(entry) = prefs.and_then(|p| p.circuits.get(channel.channel_num.as_str())) else {
            continue;
        };
        if !entry.enabled || !entry.include_in_power_summary {
            continue;
        }
        total_w += power_w.max(0.0);
    }

    total_w.max(0.0)
}

pub fn compute_emporia_device_summary_energy_kwh(
    device: &EmporiaDeviceReading,
    prefs: Option<&EmporiaDevicePreferences>,
) -> f64 {
    let default_mains = EmporiaCircuitPreferences {
        enabled: true,
        hidden: false,
        include_in_power_summary: true,
    };

    let mains_pref = prefs
        .and_then(|entry| entry.circuits.get(EMPORIA_CIRCUIT_KEY_MAINS))
        .unwrap_or(&default_mains);

    if mains_pref.enabled && mains_pref.include_in_power_summary {
        let mut total = 0.0_f64;
        for channel in &device.channels {
            if !channel.is_mains {
                continue;
            }
            if let Some(energy_kwh) = channel.energy_kwh {
                total += energy_kwh.max(0.0);
            }
        }
        return total.max(0.0);
    }

    let mut total = 0.0_f64;
    for channel in &device.channels {
        let Some(entry) = prefs.and_then(|p| p.circuits.get(channel.channel_num.as_str())) else {
            continue;
        };
        if !entry.enabled || !entry.include_in_power_summary {
            continue;
        }
        if let Some(energy_kwh) = channel.energy_kwh {
            total += energy_kwh.max(0.0);
        }
    }

    total.max(0.0)
}
