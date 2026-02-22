use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{internal_error, map_db_error};
use crate::state::AppState;

const DEFAULT_UI_REFRESH_SECONDS: i32 = 2;
const DEFAULT_LATENCY_SAMPLE_SECONDS: i32 = 10;
const DEFAULT_LATENCY_WINDOW_SAMPLES: i32 = 12;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DisplayTileType {
    CoreStatus,
    Latency,
    Sensor,
    Sensors,
    Trends,
    Outputs,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DisplayTile {
    #[serde(rename = "type")]
    pub kind: DisplayTileType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensor_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub enum DisplayTrendRange {
    #[serde(rename = "1h")]
    OneHour,
    #[serde(rename = "6h")]
    SixHours,
    #[serde(rename = "24h")]
    TwentyFourHours,
}

fn default_trend_ranges() -> Vec<DisplayTrendRange> {
    vec![
        DisplayTrendRange::OneHour,
        DisplayTrendRange::SixHours,
        DisplayTrendRange::TwentyFourHours,
    ]
}

fn default_trend_range() -> DisplayTrendRange {
    DisplayTrendRange::SixHours
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DisplayTrendConfig {
    pub sensor_id: String,
    #[serde(default = "default_trend_range")]
    pub default_range: DisplayTrendRange,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct NodeDisplayProfile {
    pub schema_version: i32,
    pub enabled: bool,
    pub kiosk_autostart: bool,
    pub ui_refresh_seconds: i32,
    pub latency_sample_seconds: i32,
    pub latency_window_samples: i32,
    pub tiles: Vec<DisplayTile>,
    pub outputs_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_pin_hash: Option<String>,
    #[serde(default = "default_trend_ranges")]
    pub trend_ranges: Vec<DisplayTrendRange>,
    pub trends: Vec<DisplayTrendConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub core_api_base_url: Option<String>,
}

impl Default for NodeDisplayProfile {
    fn default() -> Self {
        Self {
            schema_version: 1,
            enabled: false,
            kiosk_autostart: false,
            ui_refresh_seconds: DEFAULT_UI_REFRESH_SECONDS,
            latency_sample_seconds: DEFAULT_LATENCY_SAMPLE_SECONDS,
            latency_window_samples: DEFAULT_LATENCY_WINDOW_SAMPLES,
            tiles: Vec::new(),
            outputs_enabled: false,
            local_pin_hash: None,
            trend_ranges: default_trend_ranges(),
            trends: Vec::new(),
            core_api_base_url: None,
        }
    }
}

impl NodeDisplayProfile {
    fn normalized(mut self) -> Self {
        if self.schema_version < 1 {
            self.schema_version = 1;
        }
        self.ui_refresh_seconds = self.ui_refresh_seconds.clamp(1, 60);
        self.latency_sample_seconds = self.latency_sample_seconds.clamp(1, 300);
        self.latency_window_samples = self.latency_window_samples.clamp(3, 120);
        if self.trend_ranges.is_empty() {
            self.trend_ranges = default_trend_ranges();
        }
        self.core_api_base_url = self
            .core_api_base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.trim_end_matches('/').to_string());
        self.local_pin_hash = self
            .local_pin_hash
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        self
    }
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct UpdateNodeDisplayProfileResponse {
    pub status: String,
    pub node_id: String,
    pub node_agent_url: Option<String>,
    pub display: NodeDisplayProfile,
    pub warning: Option<String>,
}

fn ensure_object<'a>(
    value: &'a mut JsonValue,
    context: &str,
) -> Result<&'a mut serde_json::Map<String, JsonValue>, String> {
    match value {
        JsonValue::Object(map) => Ok(map),
        JsonValue::Null => {
            *value = JsonValue::Object(serde_json::Map::new());
            match value {
                JsonValue::Object(map) => Ok(map),
                _ => unreachable!("inserted JsonValue::Object but pattern did not match"),
            }
        }
        other => Err(format!(
            "Expected JSON object for {context}, got {}",
            json_type_name(other)
        )),
    }
}

fn json_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "bool",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

async fn load_node_config(
    db: &sqlx::PgPool,
    node_id: Uuid,
) -> Result<Option<(JsonValue, Option<String>)>, sqlx::Error> {
    let row: Option<(SqlJson<JsonValue>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(config, '{}'::jsonb) as config,
            host(ip_last) as ip_last
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(config, ip_last)| (config.0, ip_last)))
}

#[utoipa::path(
    get,
    path = "/api/nodes/{node_id}/display",
    tag = "nodes",
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "Display profile", body = NodeDisplayProfile),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn get_node_display_profile(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(node_id): Path<String>,
) -> Result<Json<NodeDisplayProfile>, (StatusCode, String)> {
    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;
    let Some((config, _ip_last)) = load_node_config(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let fallback_value = serde_json::to_value(NodeDisplayProfile::default())
        .unwrap_or_else(|_| serde_json::json!({}));
    let profile_value = config.get("display").cloned().unwrap_or(fallback_value);
    let profile = serde_json::from_value::<NodeDisplayProfile>(profile_value)
        .unwrap_or_else(|_| NodeDisplayProfile::default());
    Ok(Json(profile.normalized()))
}

#[utoipa::path(
    put,
    path = "/api/nodes/{node_id}/display",
    tag = "nodes",
    request_body = NodeDisplayProfile,
    params(("node_id" = String, Path, description = "Node id")),
    responses(
        (status = 200, description = "Updated display profile", body = UpdateNodeDisplayProfileResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found")
    )
)]
pub(crate) async fn update_node_display_profile(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(node_id): Path<String>,
    Json(payload): Json<NodeDisplayProfile>,
) -> Result<Json<UpdateNodeDisplayProfileResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;
    let node_uuid = Uuid::parse_str(node_id.trim())
        .map_err(|_| (StatusCode::NOT_FOUND, "Node not found".to_string()))?;

    let Some((mut config, _ip_last)) = load_node_config(&state.db, node_uuid)
        .await
        .map_err(map_db_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "Node not found".to_string()));
    };

    let profile = payload.normalized();
    let root = ensure_object(&mut config, "node config root")
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    root.insert(
        "display".to_string(),
        serde_json::to_value(&profile).map_err(internal_error)?,
    );

    sqlx::query(
        r#"
        UPDATE nodes
        SET config = $2,
            last_seen = COALESCE(last_seen, NOW())
        WHERE id = $1
        "#,
    )
    .bind(node_uuid)
    .bind(SqlJson(config))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut status = "stored".to_string();
    let mut warning = None;
    let mut node_agent_url = None;

    let endpoint =
        match crate::services::node_agent_resolver::resolve_node_agent_endpoint(
            &state.db,
            node_uuid,
            state.config.node_agent_port,
            false,
        )
        .await
        {
            Ok(endpoint) => endpoint,
            Err(err) => {
                warning = Some(format!("Failed to locate node-agent endpoint: {err}"));
                None
            }
        };

    if endpoint.is_none() {
        warning = Some(
            "Unable to locate node-agent endpoint (mDNS + last-known IP both missing). Ensure the node is online so the display profile can be pushed."
                .to_string(),
        );
    } else if let Some(token) =
        crate::node_agent_auth::node_agent_bearer_token(&state.db, node_uuid).await
    {
        let config_payload = serde_json::json!({ "display": &profile });

        let mut attempt_urls: Vec<String> = Vec::new();
        let base_url = endpoint.as_ref().unwrap().base_url.clone();
        attempt_urls.push(base_url);
        if let Some(fallback) = endpoint.as_ref().unwrap().ip_fallback.clone() {
            attempt_urls.push(fallback);
        }

        let mut applied = false;
        for (idx, base_url) in attempt_urls.iter().enumerate() {
            node_agent_url = Some(base_url.clone());
            let url = format!("{base_url}/v1/config");
            let response = state
                .http
                .put(url)
                .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
                .timeout(std::time::Duration::from_secs(10))
                .json(&config_payload)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    status = "applied".to_string();
                    warning = None;
                    applied = true;
                    break;
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    warning = Some(format!("Node agent rejected update ({status}): {body}"));
                    break;
                }
                Err(err) => {
                    warning = Some(format!("Node agent sync failed: {err}"));
                    if idx == 0 {
                        if let Ok(Some(refreshed)) =
                            crate::services::node_agent_resolver::resolve_node_agent_endpoint(
                                &state.db,
                                node_uuid,
                                state.config.node_agent_port,
                                true,
                            )
                            .await
                        {
                            if refreshed.base_url != base_url.as_str() {
                                node_agent_url = Some(refreshed.base_url.clone());
                                let url = format!("{}/v1/config", refreshed.base_url);
                                let retry = state
                                    .http
                                    .put(url)
                                    .header(
                                        reqwest::header::AUTHORIZATION,
                                        format!("Bearer {token}"),
                                    )
                                    .timeout(std::time::Duration::from_secs(10))
                                    .json(&config_payload)
                                    .send()
                                    .await;
                                match retry {
                                    Ok(resp) if resp.status().is_success() => {
                                        status = "applied".to_string();
                                        warning = None;
                                        applied = true;
                                        break;
                                    }
                                    Ok(resp) => {
                                        let status = resp.status();
                                        let body = resp.text().await.unwrap_or_default();
                                        warning = Some(format!(
                                            "Node agent rejected update ({status}): {body}"
                                        ));
                                        break;
                                    }
                                    Err(err) => {
                                        warning = Some(format!("Node agent sync failed: {err}"));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if !applied && warning.is_none() {
            warning = Some("Node agent sync failed.".to_string());
        }
    } else {
        warning = Some(
            "Missing node-agent auth token for this node. Re-provision the node with the controller-issued token so the controller can push display profiles."
                .to_string(),
        );
    }

    Ok(Json(UpdateNodeDisplayProfileResponse {
        status,
        node_id: node_uuid.to_string(),
        node_agent_url,
        display: profile,
        warning,
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/nodes/{node_id}/display",
        get(get_node_display_profile).put(update_node_display_profile),
    )
}
