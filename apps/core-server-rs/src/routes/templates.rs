use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value as JsonValue;

use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct TemplatesResponse {
    templates: Vec<JsonValue>,
}

fn default_sensor_templates() -> Vec<JsonValue> {
    vec![
        serde_json::json!({
            "key": "ambient-temperature",
            "name": "Ambient Temperature",
            "type": "temperature",
            "unit": "degC",
            "interval_seconds": 900,
            "rolling_avg_seconds": 900,
            "config": {
                "input": "rtd",
                "range_min": -40,
                "range_max": 85,
                "offset": 0.0,
            },
        }),
        serde_json::json!({
            "key": "soil-moisture",
            "name": "Soil Moisture",
            "type": "moisture",
            "unit": "%",
            "interval_seconds": 900,
            "rolling_avg_seconds": 900,
            "config": {
                "input": "voltage",
                "range_min": 0,
                "range_max": 3.3,
                "offset": 0.0,
            },
        }),
        serde_json::json!({
            "key": "irrigation-flow",
            "name": "Irrigation Flow",
            "type": "flow",
            "unit": "gpm",
            "interval_seconds": 60,
            "rolling_avg_seconds": 0,
            "config": {
                "mode": "pulse",
                "pulses_per_gallon": 450,
            },
        }),
    ]
}

fn default_output_templates() -> Vec<JsonValue> {
    vec![
        serde_json::json!({
            "key": "irrigation-valve-1",
            "name": "Irrigation Valve 1",
            "type": "relay",
            "state": "off",
            "supported_states": ["on", "off"],
            "config": {
                "channel": 1,
                "fail_safe_state": "off",
            },
        }),
        serde_json::json!({
            "key": "auxiliary-relay",
            "name": "Auxiliary Relay",
            "type": "relay",
            "state": "off",
            "supported_states": ["on", "off"],
            "config": {
                "channel": 2,
                "fail_safe_state": "off",
            },
        }),
    ]
}

#[utoipa::path(
    get,
    path = "/api/templates/sensors",
    tag = "templates",
    responses((status = 200, description = "Sensor templates", body = TemplatesResponse))
)]
pub(crate) async fn sensor_templates() -> Json<TemplatesResponse> {
    Json(TemplatesResponse {
        templates: default_sensor_templates(),
    })
}

#[utoipa::path(
    get,
    path = "/api/templates/outputs",
    tag = "templates",
    responses((status = 200, description = "Output templates", body = TemplatesResponse))
)]
pub(crate) async fn output_templates() -> Json<TemplatesResponse> {
    Json(TemplatesResponse {
        templates: default_output_templates(),
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/templates/sensors", get(sensor_templates))
        .route("/templates/outputs", get(output_templates))
}
