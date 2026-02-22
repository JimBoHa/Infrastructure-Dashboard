use serde_json::Value as JsonValue;

pub(crate) const VISIBILITY_OVERRIDE_KEY: &str = "visibility_override";

pub(crate) const REASON_VISIBLE: &str = "visible";
pub(crate) const REASON_SENSOR_HIDDEN: &str = "sensor.hidden";
pub(crate) const REASON_SENSOR_OVERRIDE_HIDDEN: &str = "sensor.override_hidden";
pub(crate) const REASON_SENSOR_OVERRIDE_VISIBLE: &str = "sensor.override_visible";
pub(crate) const REASON_SENSOR_POLL_DISABLED: &str = "sensor.poll_disabled";
pub(crate) const REASON_NODE_HIDDEN: &str = "node.hidden";
pub(crate) const REASON_NODE_POLL_DISABLED: &str = "node.poll_disabled";
pub(crate) const REASON_NODE_DELETED: &str = "node.deleted";
pub(crate) const REASON_NODE_HIDE_LIVE_WEATHER: &str = "node.hide_live_weather";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VisibilityOverride {
    Inherit,
    Visible,
    Hidden,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct SensorVisibilityInfo {
    pub(crate) visible: bool,
    pub(crate) reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) override_mode: Option<String>,
}

fn config_str<'a>(config: &'a JsonValue, key: &str) -> Option<&'a str> {
    config.get(key).and_then(|value| value.as_str())
}

fn config_bool(config: &JsonValue, key: &str) -> Option<bool> {
    config.get(key).and_then(|value| value.as_bool())
}

pub(crate) fn is_open_meteo_weather_sensor(config: &JsonValue) -> bool {
    config_str(config, "source") == Some("forecast_points")
        && config_str(config, "provider") == Some("open_meteo")
        && config_str(config, "kind") == Some("weather")
}

pub(crate) fn node_hides_live_weather(node_config: &JsonValue) -> bool {
    config_bool(node_config, "hide_live_weather").unwrap_or(false)
}

fn is_legacy_node_hide_hidden(sensor_config: &JsonValue) -> bool {
    let hidden = config_bool(sensor_config, "hidden").unwrap_or(false);
    if !hidden {
        return false;
    }
    let reason = config_str(sensor_config, "hidden_reason").unwrap_or("");
    reason == REASON_NODE_HIDE_LIVE_WEATHER
}

pub(crate) fn parse_visibility_override(sensor_config: &JsonValue) -> VisibilityOverride {
    let raw = config_str(sensor_config, VISIBILITY_OVERRIDE_KEY)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    match raw.as_str() {
        "" | "inherit" | "default" | "auto" => VisibilityOverride::Inherit,
        "visible" | "show" | "unhide" | "force_visible" | "force-visible" | "forcevisible" => {
            VisibilityOverride::Visible
        }
        "hidden" | "hide" | "force_hidden" | "force-hidden" | "forcehidden" => {
            VisibilityOverride::Hidden
        }
        _ => VisibilityOverride::Inherit,
    }
}

pub(crate) fn evaluate_sensor_visibility(
    sensor_config: &JsonValue,
    node_config: &JsonValue,
) -> SensorVisibilityInfo {
    let override_mode = parse_visibility_override(sensor_config);
    let override_label = match override_mode {
        VisibilityOverride::Inherit => None,
        VisibilityOverride::Visible => Some("visible".to_string()),
        VisibilityOverride::Hidden => Some("hidden".to_string()),
    };

    let node_deleted = config_bool(node_config, "deleted").unwrap_or(false);
    if node_deleted {
        return SensorVisibilityInfo {
            visible: false,
            reason: REASON_NODE_DELETED.to_string(),
            override_mode: override_label,
        };
    }

    let node_poll_enabled = config_bool(node_config, "poll_enabled").unwrap_or(true);
    if !node_poll_enabled {
        return SensorVisibilityInfo {
            visible: false,
            reason: REASON_NODE_POLL_DISABLED.to_string(),
            override_mode: override_label,
        };
    }

    let node_hidden = config_bool(node_config, "hidden").unwrap_or(false);
    if node_hidden {
        return SensorVisibilityInfo {
            visible: false,
            reason: REASON_NODE_HIDDEN.to_string(),
            override_mode: override_label,
        };
    }

    let sensor_poll_enabled = config_bool(sensor_config, "poll_enabled").unwrap_or(true);
    if !sensor_poll_enabled {
        return SensorVisibilityInfo {
            visible: false,
            reason: REASON_SENSOR_POLL_DISABLED.to_string(),
            override_mode: override_label,
        };
    }

    if override_mode == VisibilityOverride::Hidden {
        return SensorVisibilityInfo {
            visible: false,
            reason: REASON_SENSOR_OVERRIDE_HIDDEN.to_string(),
            override_mode: override_label,
        };
    }

    let explicit_hidden = config_bool(sensor_config, "hidden").unwrap_or(false);
    if explicit_hidden && !is_legacy_node_hide_hidden(sensor_config) {
        return SensorVisibilityInfo {
            visible: false,
            reason: REASON_SENSOR_HIDDEN.to_string(),
            override_mode: override_label,
        };
    }

    if node_hides_live_weather(node_config) && is_open_meteo_weather_sensor(sensor_config) {
        if override_mode == VisibilityOverride::Visible {
            return SensorVisibilityInfo {
                visible: true,
                reason: REASON_SENSOR_OVERRIDE_VISIBLE.to_string(),
                override_mode: override_label,
            };
        }
        return SensorVisibilityInfo {
            visible: false,
            reason: REASON_NODE_HIDE_LIVE_WEATHER.to_string(),
            override_mode: override_label,
        };
    }

    SensorVisibilityInfo {
        visible: true,
        reason: if override_mode == VisibilityOverride::Visible {
            REASON_SENSOR_OVERRIDE_VISIBLE.to_string()
        } else {
            REASON_VISIBLE.to_string()
        },
        override_mode: override_label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_meteo_weather_sensor_matches_expected() {
        let config = serde_json::json!({
            "source": "forecast_points",
            "provider": "open_meteo",
            "kind": "weather",
        });
        assert!(is_open_meteo_weather_sensor(&config));
    }

    #[test]
    fn node_hide_live_weather_hides_open_meteo_weather() {
        let node_config = serde_json::json!({ "hide_live_weather": true });
        let sensor_config = serde_json::json!({
            "source": "forecast_points",
            "provider": "open_meteo",
            "kind": "weather",
        });
        let vis = evaluate_sensor_visibility(&sensor_config, &node_config);
        assert!(!vis.visible);
        assert_eq!(vis.reason, REASON_NODE_HIDE_LIVE_WEATHER);
    }

    #[test]
    fn per_sensor_override_visible_beats_node_hide_rule() {
        let node_config = serde_json::json!({ "hide_live_weather": true });
        let sensor_config = serde_json::json!({
            "source": "forecast_points",
            "provider": "open_meteo",
            "kind": "weather",
            "visibility_override": "visible",
        });
        let vis = evaluate_sensor_visibility(&sensor_config, &node_config);
        assert!(vis.visible);
        assert_eq!(vis.reason, REASON_SENSOR_OVERRIDE_VISIBLE);
    }

    #[test]
    fn explicit_hidden_beats_override_visible() {
        let node_config = serde_json::json!({});
        let sensor_config = serde_json::json!({
            "hidden": true,
            "visibility_override": "visible",
        });
        let vis = evaluate_sensor_visibility(&sensor_config, &node_config);
        assert!(!vis.visible);
        assert_eq!(vis.reason, REASON_SENSOR_HIDDEN);
    }

    #[test]
    fn legacy_node_hide_hidden_reason_is_ignored_when_node_rule_off() {
        let node_config = serde_json::json!({ "hide_live_weather": false });
        let sensor_config = serde_json::json!({
            "hidden": true,
            "hidden_reason": "node.hide_live_weather",
            "source": "forecast_points",
            "provider": "open_meteo",
            "kind": "weather",
        });
        let vis = evaluate_sensor_visibility(&sensor_config, &node_config);
        assert!(vis.visible);
        assert_eq!(vis.reason, REASON_VISIBLE);
    }
}
