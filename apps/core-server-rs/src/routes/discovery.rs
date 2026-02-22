use axum::extract::Query;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::RngCore;
use reqwest::header::AUTHORIZATION;
use reqwest::Client;
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::{Postgres, Transaction};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{internal_error, map_db_error};
use crate::routes::nodes::{NodeResponse, NodeRow};
use crate::state::AppState;

const SERVICE_TYPE: &str = "_iotnode._tcp.local.";
const TOKEN_TTL_SECONDS: i64 = 900;

const RENOGY_SOURCE: &str = "renogy_bt2";

const RENOGY_ALLOWED_METRICS: &[&str] = &[
    "pv_power_w",
    "pv_voltage_v",
    "pv_current_a",
    "battery_soc_percent",
    "battery_voltage_v",
    "battery_current_a",
    "battery_temp_c",
    "controller_temp_c",
    "load_power_w",
    "load_voltage_v",
    "load_current_a",
    "runtime_hours",
];

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AdoptionCandidate {
    pub(crate) service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) port: Option<u16>,
    pub(crate) mac_eth: Option<String>,
    pub(crate) mac_wifi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) adoption_token: Option<String>,
    pub(crate) properties: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct ScanQuery {
    #[param(minimum = 0.5, maximum = 10.0)]
    timeout: Option<f64>,
}

pub(crate) async fn scan_candidates(
    timeout: std::time::Duration,
) -> anyhow::Result<Vec<AdoptionCandidate>> {
    #[cfg(target_os = "macos")]
    {
        let candidates = tokio::task::spawn_blocking(move || browse_mdns(timeout)).await??;
        return Ok(candidates);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = timeout;
        Ok(vec![])
    }
}

#[utoipa::path(
    get,
    path = "/api/scan",
    tag = "discovery",
    params(ScanQuery),
    responses(
        (status = 200, description = "Adoption candidates", body = Vec<AdoptionCandidate>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn scan_nodes(
    AuthUser(user): AuthUser,
    Query(query): Query<ScanQuery>,
) -> Result<Json<Vec<AdoptionCandidate>>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let timeout_seconds = query.timeout.unwrap_or(3.0).clamp(0.5, 10.0);
    let candidates = scan_candidates(std::time::Duration::from_secs_f64(timeout_seconds))
        .await
        .map_err(internal_error)?;
    Ok(Json(candidates))
}

#[cfg(target_os = "macos")]
fn browse_mdns(timeout: std::time::Duration) -> anyhow::Result<Vec<AdoptionCandidate>> {
    use mdns_sd::{ServiceDaemon, ServiceEvent};
    use std::collections::BTreeMap;
    use std::time::Instant;

    let mdns = ServiceDaemon::new()?;
    let receiver = mdns.browse(SERVICE_TYPE)?;
    let deadline = Instant::now() + timeout;
    let mut results: BTreeMap<String, AdoptionCandidate> = BTreeMap::new();

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
            let adoption_token = props
                .get("adoption_token")
                .cloned()
                .filter(|v| !v.trim().is_empty() && v.to_lowercase() != "none");
            let candidate = AdoptionCandidate {
                service_name: resolved.fullname.clone(),
                hostname: Some(resolved.host.clone()),
                ip,
                port: Some(resolved.port),
                mac_eth,
                mac_wifi,
                adoption_token,
                properties: props,
            };
            results.insert(candidate.service_name.clone(), candidate);
        }
    }

    let _ = mdns.shutdown();
    Ok(results.into_values().collect())
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct AdoptionTokenRequest {
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    service_name: Option<String>,
    metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AdoptionTokenResponse {
    token: String,
}

#[derive(sqlx::FromRow)]
struct AdoptionTokenRow {
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    expires_at: DateTime<Utc>,
    used_at: Option<DateTime<Utc>>,
}

#[utoipa::path(
    post,
    path = "/api/adoption/tokens",
    tag = "discovery",
    request_body = AdoptionTokenRequest,
    responses(
        (status = 200, description = "Adoption token", body = AdoptionTokenResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub(crate) async fn issue_adoption_token(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<AdoptionTokenRequest>,
) -> Result<Json<AdoptionTokenResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let mac_eth = normalize_mac_opt(payload.mac_eth.as_deref());
    let mac_wifi = normalize_mac_opt(payload.mac_wifi.as_deref());
    if mac_eth.is_none() && mac_wifi.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing mac_eth/mac_wifi binding".to_string(),
        ));
    }

    let expires_at = Utc::now() + ChronoDuration::seconds(TOKEN_TTL_SECONDS);
    let service_name = payload.service_name.unwrap_or_default().trim().to_string();
    let metadata = payload.metadata.unwrap_or_else(|| serde_json::json!({}));
    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    lock_adoption_token_macs(&mut tx, mac_eth.as_deref(), mac_wifi.as_deref())
        .await
        .map_err(map_db_error)?;

    cleanup_unused_tokens(&mut tx, mac_eth.as_deref(), mac_wifi.as_deref(), None)
        .await
        .map_err(map_db_error)?;

    let token = {
        let mut generated: Option<String> = None;
        for _ in 0..5 {
            let candidate = generate_token();
            let inserted = sqlx::query(
                r#"
                INSERT INTO adoption_tokens (token, mac_eth, mac_wifi, service_name, metadata, expires_at)
                VALUES ($1, $2::macaddr, $3::macaddr, $4, $5, $6)
                "#,
            )
            .bind(&candidate)
            .bind(mac_eth.as_deref())
            .bind(mac_wifi.as_deref())
            .bind(&service_name)
            .bind(SqlJson(metadata.clone()))
            .bind(expires_at)
            .execute(&mut *tx)
            .await;

            match inserted {
                Ok(_) => {
                    generated = Some(candidate);
                    break;
                }
                Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
                    tracing::warn!(token = %candidate, "adoption token collision, regenerating");
                    continue;
                }
                Err(err) => return Err(map_db_error(err)),
            }
        }
        generated
    }
    .ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to issue unique adoption token".to_string(),
        )
    })?;

    cleanup_unused_tokens(
        &mut tx,
        mac_eth.as_deref(),
        mac_wifi.as_deref(),
        Some(token.as_str()),
    )
    .await
    .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;

    Ok(Json(AdoptionTokenResponse { token }))
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct AdoptRequest {
    name: String,
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    ip: Option<String>,
    port: Option<u16>,
    status: Option<String>,
    token: String,
    restore_from_node_id: Option<String>,
}

#[utoipa::path(
    post,
    path = "/api/adopt",
    tag = "discovery",
    request_body = AdoptRequest,
    responses(
        (status = 200, description = "Adopted node", body = NodeResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Node not found"),
        (status = 409, description = "Token already used")
    )
)]
pub(crate) async fn adopt_node(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<AdoptRequest>,
) -> Result<Json<NodeResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let token_value = payload.token.trim();
    if token_value.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Missing adoption token".to_string(),
        ));
    }

    let requested_mac_eth = normalize_mac_opt(payload.mac_eth.as_deref());
    let requested_mac_wifi = normalize_mac_opt(payload.mac_wifi.as_deref());
    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let token_row: Option<AdoptionTokenRow> = sqlx::query_as(
        r#"
        SELECT mac_eth::text as mac_eth, mac_wifi::text as mac_wifi, expires_at, used_at
        FROM adoption_tokens
        WHERE token = $1
        FOR UPDATE
        "#,
    )
    .bind(token_value)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(token_row) = token_row else {
        return Err((StatusCode::FORBIDDEN, "Invalid adoption token".to_string()));
    };
    if token_row.expires_at <= Utc::now() {
        return Err((StatusCode::FORBIDDEN, "Adoption token expired".to_string()));
    }
    if token_row.used_at.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "Adoption token already used".to_string(),
        ));
    }

    let (resolved_eth, resolved_wifi) = resolve_mac_binding(
        token_row.mac_eth.as_deref(),
        token_row.mac_wifi.as_deref(),
        requested_mac_eth.as_deref(),
        requested_mac_wifi.as_deref(),
    )?;

    let name = payload.name.trim().to_string();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Missing node name".to_string()));
    }
    let status = payload.status.unwrap_or_else(|| "online".to_string());
    let status = status.trim();
    let status = if status.is_empty() { "online" } else { status };
    let status = status.to_string();
    let ip_last = payload
        .ip
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|v| !v.is_empty());

    let existing_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM nodes
        WHERE ($1::macaddr IS NOT NULL AND mac_eth = $1::macaddr)
           OR ($2::macaddr IS NOT NULL AND mac_wifi = $2::macaddr)
        LIMIT 1
        "#,
    )
    .bind(resolved_eth.as_deref())
    .bind(resolved_wifi.as_deref())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let node_id = if let Some(id) = existing_id {
        let _ = sqlx::query(
            r#"
            UPDATE nodes
            SET name = $2,
                status = $3,
                ip_last = CASE WHEN $4::text IS NULL THEN ip_last ELSE $4::inet END,
                last_seen = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(&name)
        .bind(&status)
        .bind(ip_last.as_deref())
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;
        id
    } else {
        let inserted: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO nodes (name, mac_eth, mac_wifi, ip_last, status, created_at, last_seen)
            VALUES ($1, $2::macaddr, $3::macaddr, $4::inet, $5, NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(name)
        .bind(resolved_eth.as_deref())
        .bind(resolved_wifi.as_deref())
        .bind(ip_last.as_deref())
        .bind(&status)
        .fetch_one(&mut *tx)
        .await
        .map_err(map_db_error)?;
        inserted
    };

    let updated_token = sqlx::query(
        r#"
        UPDATE adoption_tokens
        SET used_at = NOW(), node_id = $2
        WHERE token = $1 AND used_at IS NULL
        "#,
    )
    .bind(token_value)
    .bind(node_id)
    .execute(&mut *tx)
    .await
    .map_err(map_db_error)?;
    if updated_token.rows_affected() == 0 {
        return Err((
            StatusCode::CONFLICT,
            "Adoption token already used".to_string(),
        ));
    }

    tx.commit().await.map_err(map_db_error)?;

    if let Some(ip) = ip_last.clone() {
        let port = payload.port.unwrap_or(9000);
        if let Err(err) = sync_node_agent_profile(&state.db, node_id, &ip, port).await {
            tracing::warn!(error = %err, ip = %ip, port = port, "failed to sync node-agent profile during adoption");
        }
    }

    let _ = payload.restore_from_node_id;
	    let node: Option<NodeRow> = sqlx::query_as(
	        r#"
	        SELECT
	            id,
	            name,
	            status,
	            uptime_seconds,
	            cpu_percent,
	            storage_used_bytes,
	            memory_percent,
	            memory_used_bytes,
	            ping_ms::real as ping_ms,
	            ping_p50_30m_ms::real as ping_p50_30m_ms,
	            ping_jitter_ms::real as ping_jitter_ms,
	            mqtt_broker_rtt_ms::real as mqtt_broker_rtt_ms,
	            mqtt_broker_rtt_jitter_ms::real as mqtt_broker_rtt_jitter_ms,
	            network_latency_ms,
	            network_jitter_ms,
	            uptime_percent_24h,
	            mac_eth::text as mac_eth,
	            mac_wifi::text as mac_wifi,
            host(ip_last) as ip_last,
            last_seen,
            created_at,
            COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(node) = node else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Adopted node missing".to_string(),
        ));
    };

    Ok(Json(NodeResponse::from(node)))
}

async fn cleanup_unused_tokens(
    tx: &mut Transaction<'_, Postgres>,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
    preserve_token: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM adoption_tokens
        WHERE used_at IS NULL
          AND expires_at <= NOW()
        "#,
    )
    .execute(&mut **tx)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM adoption_tokens
        WHERE used_at IS NULL
          AND (
            ($1::macaddr IS NOT NULL AND mac_eth = $1::macaddr)
            OR ($2::macaddr IS NOT NULL AND mac_wifi = $2::macaddr)
          )
          AND ($3::text IS NULL OR token <> $3::text)
        "#,
    )
    .bind(mac_eth)
    .bind(mac_wifi)
    .bind(preserve_token)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn lock_adoption_token_macs(
    tx: &mut Transaction<'_, Postgres>,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
) -> Result<(), sqlx::Error> {
    let mut macs: Vec<&str> = Vec::new();
    if let Some(mac) = mac_eth {
        macs.push(mac);
    }
    if let Some(mac) = mac_wifi {
        macs.push(mac);
    }
    macs.sort_unstable();
    macs.dedup();

    for mac in macs {
        let key = advisory_lock_key("adoption_tokens", mac);
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(key)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

fn advisory_lock_key(namespace: &str, value: &str) -> i64 {
    fn fnv1a_64(input: &str) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in input.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    let combined = format!("{namespace}:{value}");
    fnv1a_64(&combined) as i64
}

fn normalize_mac_opt(value: Option<&str>) -> Option<String> {
    value.map(normalize_mac).filter(|value| !value.is_empty())
}

fn resolve_mac_binding(
    token_mac_eth: Option<&str>,
    token_mac_wifi: Option<&str>,
    request_mac_eth: Option<&str>,
    request_mac_wifi: Option<&str>,
) -> Result<(Option<String>, Option<String>), (StatusCode, String)> {
    let token_mac_eth = normalize_mac_opt(token_mac_eth);
    let token_mac_wifi = normalize_mac_opt(token_mac_wifi);
    if token_mac_eth.is_none() && token_mac_wifi.is_none() {
        return Err((StatusCode::FORBIDDEN, "Invalid adoption token".to_string()));
    }

    let request_mac_eth = normalize_mac_opt(request_mac_eth);
    let request_mac_wifi = normalize_mac_opt(request_mac_wifi);

    if let (Some(expected), Some(provided)) = (token_mac_eth.as_deref(), request_mac_eth.as_deref())
    {
        if expected != provided {
            return Err((
                StatusCode::BAD_REQUEST,
                "Adoption token does not match provided mac_eth".to_string(),
            ));
        }
    }
    if let (Some(expected), Some(provided)) =
        (token_mac_wifi.as_deref(), request_mac_wifi.as_deref())
    {
        if expected != provided {
            return Err((
                StatusCode::BAD_REQUEST,
                "Adoption token does not match provided mac_wifi".to_string(),
            ));
        }
    }

    let resolved_eth = token_mac_eth.or(request_mac_eth);
    let resolved_wifi = token_mac_wifi.or(request_mac_wifi);
    if resolved_eth.is_none() && resolved_wifi.is_none() {
        return Err((StatusCode::BAD_REQUEST, "Missing MAC binding".to_string()));
    }

    Ok((resolved_eth, resolved_wifi))
}

fn generate_token() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NodeAgentConfig {
    #[serde(default)]
    node: HashMap<String, JsonValue>,
    #[serde(default)]
    sensors: Vec<NodeAgentSensor>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NodeAgentSensor {
    sensor_id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, rename = "type")]
    sensor_type: Option<String>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    metric: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    interval_seconds: Option<f64>,
    #[serde(default, rename = "rolling_average_seconds")]
    rolling_average_seconds: Option<f64>,
}

async fn sync_node_agent_profile(
    db: &sqlx::PgPool,
    node_id: Uuid,
    ip: &str,
    port: u16,
) -> anyhow::Result<()> {
    let Some(token) = crate::node_agent_auth::node_agent_bearer_token(db, node_id).await else {
        anyhow::bail!(
            "Missing node-agent auth token for node {} (re-provision the node with the controller-issued token).",
            node_id
        );
    };
    let client = Client::builder().timeout(Duration::from_secs(2)).build()?;
    let url = format!("http://{ip}:{port}/v1/config");
    let response = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("node-agent /v1/config returned {}", response.status());
    }
    let payload: NodeAgentConfig = response.json().await?;

    let expected_macs: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT mac_eth::text as mac_eth, mac_wifi::text as mac_wifi
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some((expected_eth, expected_wifi)) = expected_macs {
        let reported_eth = payload
            .node
            .get("mac_eth")
            .and_then(|value| value.as_str())
            .map(normalize_mac);
        let reported_wifi = payload
            .node
            .get("mac_wifi")
            .and_then(|value| value.as_str())
            .map(normalize_mac);

        let eth_matches = expected_eth
            .as_deref()
            .map(normalize_mac)
            .filter(|value| !value.is_empty())
            .and_then(|expected| reported_eth.as_deref().map(|reported| expected == reported))
            .unwrap_or(false);
        let wifi_matches = expected_wifi
            .as_deref()
            .map(normalize_mac)
            .filter(|value| !value.is_empty())
            .and_then(|expected| {
                reported_wifi
                    .as_deref()
                    .map(|reported| expected == reported)
            })
            .unwrap_or(false);

        if !(eth_matches || wifi_matches) {
            anyhow::bail!(
                "node-agent identity mismatch (expected mac_eth={:?} mac_wifi={:?}, reported mac_eth={:?} mac_wifi={:?})",
                expected_eth,
                expected_wifi,
                reported_eth,
                reported_wifi
            );
        }
    }

    if let Some(node_agent_id) = payload.node.get("node_id").and_then(|value| value.as_str()) {
        if let Err(err) = sqlx::query(
            r#"
            UPDATE nodes
            SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{agent_node_id}', to_jsonb($2::text), true)
            WHERE id = $1
            "#,
        )
        .bind(node_id)
        .bind(node_agent_id)
        .execute(db)
        .await
        {
            tracing::warn!(error = %err, node_id = %node_id, "failed to persist node-agent id");
        }
    }

    for sensor in payload.sensors {
        let sensor_id = sensor.sensor_id.trim();
        if !is_hex_24(sensor_id) {
            continue;
        }
        let driver = sensor.sensor_type.as_deref().unwrap_or("").trim();
        if driver != RENOGY_SOURCE {
            continue;
        }
        let metric = sensor.metric.as_deref().unwrap_or("").trim();
        if metric.is_empty() || !RENOGY_ALLOWED_METRICS.contains(&metric) {
            continue;
        }

        let unit = sensor.unit.as_deref().unwrap_or("").trim();
        if let Some(expected_unit) = expected_unit_for_metric(metric) {
            if unit != expected_unit {
                continue;
            }
        }
        let name = sensor
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(sensor_id);
        let interval_seconds = sensor.interval_seconds.unwrap_or(30.0).round().max(0.0) as i32;
        let rolling_avg_seconds = sensor
            .rolling_average_seconds
            .unwrap_or(0.0)
            .round()
            .max(0.0) as i32;
        let sensor_type = sensor_kind_from_unit_metric(unit, metric);

        let mut config = serde_json::json!({
            "metric": metric,
            "source": RENOGY_SOURCE,
        });
        if let Some(location) = sensor
            .location
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            config["location"] = serde_json::Value::String(location.to_string());
        }

        if let Err(err) = sqlx::query(
            r#"
            INSERT INTO sensors (sensor_id, node_id, name, type, unit, interval_seconds, rolling_avg_seconds, config, created_at, deleted_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NULL)
            ON CONFLICT (sensor_id) DO UPDATE
            SET node_id = EXCLUDED.node_id,
                name = EXCLUDED.name,
                type = EXCLUDED.type,
                unit = EXCLUDED.unit,
                interval_seconds = EXCLUDED.interval_seconds,
                rolling_avg_seconds = EXCLUDED.rolling_avg_seconds,
                config = COALESCE(sensors.config, '{}'::jsonb) || EXCLUDED.config,
                deleted_at = NULL
            "#,
        )
        .bind(sensor_id)
        .bind(node_id)
        .bind(name)
        .bind(sensor_type)
        .bind(unit)
        .bind(interval_seconds)
        .bind(rolling_avg_seconds)
        .bind(config)
        .execute(db)
        .await
        {
            tracing::warn!(error = %err, sensor_id = %sensor_id, node_id = %node_id, "failed to upsert sensor from node-agent profile");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_bound_mac_when_request_matches() {
        let (mac_eth, mac_wifi) = resolve_mac_binding(
            Some("AA:BB:CC:DD:EE:FF"),
            None,
            Some("aa:bb:cc:dd:ee:ff"),
            None,
        )
        .expect("expected matching MACs to be accepted");

        assert_eq!(mac_eth.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(mac_wifi, None);
    }

    #[test]
    fn rejects_mismatched_bound_eth() {
        let err = resolve_mac_binding(
            Some("aa:bb:cc:dd:ee:ff"),
            None,
            Some("11:22:33:44:55:66"),
            None,
        )
        .expect_err("expected mismatch to fail");

        assert_eq!(err.0, StatusCode::BAD_REQUEST);
        assert!(err.1.contains("mac_eth"));
    }

    #[test]
    fn falls_back_to_token_binding_when_missing_in_request() {
        let (mac_eth, mac_wifi) = resolve_mac_binding(
            Some("AA:BB:CC:DD:EE:FF"),
            Some("11:22:33:44:55:66"),
            None,
            Some("11:22:33:44:55:66"),
        )
        .expect("expected token-bound MACs to populate missing request fields");

        assert_eq!(mac_eth.as_deref(), Some("aa:bb:cc:dd:ee:ff"));
        assert_eq!(mac_wifi.as_deref(), Some("11:22:33:44:55:66"));
    }

    #[test]
    fn rejects_tokens_without_mac_binding() {
        let err = resolve_mac_binding(None, None, None, None).expect_err("missing binding");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }
}

fn is_hex_24(value: &str) -> bool {
    value.len() == 24 && value.as_bytes().iter().all(|byte| byte.is_ascii_hexdigit())
}

fn normalize_mac(value: &str) -> String {
    value.trim().to_lowercase()
}

fn expected_unit_for_metric(metric: &str) -> Option<&'static str> {
    match metric {
        "pv_power_w" | "load_power_w" => Some("W"),
        "pv_voltage_v" | "battery_voltage_v" | "load_voltage_v" => Some("V"),
        "pv_current_a" | "battery_current_a" | "load_current_a" => Some("A"),
        "battery_soc_percent" => Some("%"),
        "battery_temp_c" | "controller_temp_c" => Some("degC"),
        "runtime_hours" => Some("hr"),
        _ => None,
    }
}

fn sensor_kind_from_unit_metric(unit: &str, metric: &str) -> &'static str {
    if metric.eq_ignore_ascii_case("runtime_hours") || unit.eq_ignore_ascii_case("hr") {
        return "runtime";
    }
    match unit {
        "W" => "power",
        "V" => "voltage",
        "A" => "current",
        "%" => "percentage",
        "degC" | "C" | "°C" => "temperature",
        _ => "unknown",
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/scan", get(scan_nodes))
        .route("/adoption/tokens", post(issue_adoption_token))
        .route("/adopt", post(adopt_node))
}
