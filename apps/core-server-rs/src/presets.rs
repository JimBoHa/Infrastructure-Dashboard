use serde::Deserialize;
use std::sync::OnceLock;
use tracing::error;

const PRESETS_JSON: &str = include_str!("../../../shared/presets/integrations.json");

static PRESETS: OnceLock<IntegrationPresets> = OnceLock::new();

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct IntegrationPresets {
    pub renogy_bt2: RenogyBt2Preset,
    pub ws_2902: Ws2902Preset,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RenogyBt2Preset {
    pub default_interval_seconds: i32,
    pub sensors: Vec<RenogySensorPreset>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RenogySensorPreset {
    pub metric: String,
    pub name: String,
    pub core_type: String,
    pub unit: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Ws2902Preset {
    pub default_interval_seconds: i32,
    pub sensors: Vec<Ws2902SensorPreset>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Ws2902SensorPreset {
    pub sensor_type: String,
    pub name: String,
    pub unit: String,
}

pub fn presets() -> &'static IntegrationPresets {
    PRESETS.get_or_init(|| match serde_json::from_str(PRESETS_JSON) {
        Ok(presets) => presets,
        Err(err) => {
            error!(
                error = %err,
                "Invalid shared/presets/integrations.json; falling back to empty presets"
            );
            IntegrationPresets {
                renogy_bt2: RenogyBt2Preset {
                    default_interval_seconds: 30,
                    sensors: Vec::new(),
                },
                ws_2902: Ws2902Preset {
                    default_interval_seconds: 30,
                    sensors: Vec::new(),
                },
            }
        }
    })
}

pub fn renogy_bt2_default_interval_seconds() -> i32 {
    presets().renogy_bt2.default_interval_seconds
}

pub fn renogy_bt2_sensors() -> &'static [RenogySensorPreset] {
    &presets().renogy_bt2.sensors
}

pub fn ws_2902_default_interval_seconds() -> i32 {
    presets().ws_2902.default_interval_seconds
}

pub fn ws_2902_sensors() -> &'static [Ws2902SensorPreset] {
    &presets().ws_2902.sensors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn preset_file_parses_and_is_stable() {
        let raw: IntegrationPresets =
            serde_json::from_str(PRESETS_JSON).expect("preset file must parse");
        assert_eq!(raw, *presets());
    }

    #[test]
    fn renogy_metrics_are_unique() {
        let mut seen = BTreeSet::new();
        for sensor in renogy_bt2_sensors() {
            assert!(
                seen.insert(sensor.metric.clone()),
                "duplicate renogy metric"
            );
            assert!(!sensor.name.trim().is_empty());
        }
        assert!(renogy_bt2_default_interval_seconds() > 0);
    }

    #[test]
    fn ws_2902_types_are_unique() {
        let mut seen = BTreeSet::new();
        for sensor in ws_2902_sensors() {
            assert!(
                seen.insert(sensor.sensor_type.clone()),
                "duplicate ws_2902 sensor_type"
            );
            assert!(!sensor.name.trim().is_empty());
        }
        assert!(ws_2902_default_interval_seconds() > 0);
    }
}
