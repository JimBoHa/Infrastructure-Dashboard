use anyhow::Result;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::state::AppState;

const TOPIC_FILTER: &str = "iot/+/status";

pub struct MqttStatusIngestService {
    state: AppState,
}

impl MqttStatusIngestService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn start(self, cancel: CancellationToken) {
        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                if let Err(err) = run_once(&state, cancel.clone()).await {
                    tracing::warn!("mqtt status ingest loop failed: {err:#}");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        });
    }
}

async fn run_once(state: &AppState, cancel: CancellationToken) -> Result<()> {
    let mut options = MqttOptions::new(
        "farmdashboard-core-status-ingest",
        &state.config.mqtt_host,
        state.config.mqtt_port,
    );
    options.set_keep_alive(Duration::from_secs(10));
    if let (Some(username), Some(password)) = (
        state.config.mqtt_username.as_deref(),
        state.config.mqtt_password.as_deref(),
    ) {
        options.set_credentials(username, password);
    }

    let (client, mut eventloop) = AsyncClient::new(options, 10);
    client.subscribe(TOPIC_FILTER, QoS::AtLeastOnce).await?;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            event = eventloop.poll() => {
                match event {
                    Ok(Event::Incoming(Incoming::Publish(publish))) => {
                        handle_publish(&state.db, publish.topic.as_str(), publish.payload.as_ref())
                            .await;
                    }
                    Ok(Event::Incoming(Incoming::Disconnect)) => anyhow::bail!("mqtt disconnected"),
                    Ok(_) => {}
                    Err(err) => {
                        anyhow::bail!(err);
                    }
                }
            }
        }
    }

    Ok(())
}

fn normalize_mac_opt(value: Option<&str>) -> Option<String> {
    value
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
}

fn normalize_ipv4_opt(value: Option<&str>) -> Option<String> {
    let ip = value.map(str::trim).filter(|v| !v.is_empty())?.to_string();
    if ip.starts_with("127.") || ip == "0.0.0.0" {
        return None;
    }
    Some(ip)
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

async fn handle_publish(db: &PgPool, _topic: &str, payload: &[u8]) {
    let parsed: JsonValue = match serde_json::from_slice(payload) {
        Ok(value) => value,
        Err(err) => {
            tracing::debug!("mqtt status ingest: invalid json payload: {err}");
            return;
        }
    };
    let obj = match parsed.as_object() {
        Some(obj) => obj,
        None => return,
    };

    let mac_eth = normalize_mac_opt(obj.get("mac_eth").and_then(|v| v.as_str()));
    let mac_wifi = normalize_mac_opt(obj.get("mac_wifi").and_then(|v| v.as_str()));
    if mac_eth.is_none() && mac_wifi.is_none() {
        return;
    }

    let ip_last = normalize_ipv4_opt(obj.get("ip").and_then(|v| v.as_str()));
    let status = obj
        .get("status")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let uptime_seconds = obj.get("uptime_seconds").and_then(|v| v.as_i64());
    let cpu_percent = obj.get("cpu_percent").and_then(|v| v.as_f64()).map(|v| v as f32);
    let storage_used_bytes = obj
        .get("storage_used_bytes")
        .and_then(|v| v.as_i64())
        .or_else(|| obj.get("storage_used_bytes").and_then(|v| v.as_u64()).map(|v| v as i64));
    let memory_percent = obj.get("memory_percent").and_then(|v| v.as_f64()).map(|v| v as f32);
    let memory_used_bytes = obj
        .get("memory_used_bytes")
        .and_then(|v| v.as_i64())
        .or_else(|| obj.get("memory_used_bytes").and_then(|v| v.as_u64()).map(|v| v as i64));
    let network_latency_ms = obj
        .get("network_latency_ms")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let network_jitter_ms = obj
        .get("network_jitter_ms")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);
    let uptime_percent_24h = obj
        .get("uptime_percent_24h")
        .and_then(|v| v.as_f64())
        .map(|v| v as f32);

    let ping_ms = obj.get("ping_ms").and_then(|v| v.as_f64());
    let ping_p50_30m_ms = obj.get("ping_p50_30m_ms").and_then(|v| v.as_f64());
    let ping_jitter_ms = obj.get("ping_jitter_ms").and_then(|v| v.as_f64());
    let mqtt_broker_rtt_ms = obj.get("mqtt_broker_rtt_ms").and_then(|v| v.as_f64());
    let mqtt_broker_rtt_jitter_ms = obj
        .get("mqtt_broker_rtt_jitter_ms")
        .and_then(|v| v.as_f64());

    let agent_node_id = obj
        .get("node_id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    let agent_host = agent_node_id.as_deref().and_then(host_from_agent_node_id);

    let _ = sqlx::query(
        r#"
        UPDATE nodes
        SET status = COALESCE($3::text, status),
            uptime_seconds = COALESCE($4::bigint, uptime_seconds),
            cpu_percent = COALESCE($5::real, cpu_percent),
            storage_used_bytes = COALESCE($6::bigint, storage_used_bytes),
            memory_percent = COALESCE($7::real, memory_percent),
            memory_used_bytes = COALESCE($8::bigint, memory_used_bytes),
            network_latency_ms = COALESCE($9::real, network_latency_ms),
            network_jitter_ms = COALESCE($10::real, network_jitter_ms),
            uptime_percent_24h = COALESCE($11::real, uptime_percent_24h),
            ping_ms = COALESCE($12::double precision, ping_ms),
            ping_p50_30m_ms = COALESCE($13::double precision, ping_p50_30m_ms),
            ping_jitter_ms = COALESCE($14::double precision, ping_jitter_ms),
            mqtt_broker_rtt_ms = COALESCE($15::double precision, mqtt_broker_rtt_ms),
            mqtt_broker_rtt_jitter_ms = COALESCE($16::double precision, mqtt_broker_rtt_jitter_ms),
            ip_last = CASE WHEN $17::text IS NULL THEN ip_last ELSE $17::inet END,
            config = CASE
                WHEN $18::text IS NULL THEN config
                ELSE jsonb_set(
                    jsonb_set(
                        jsonb_set(
                            COALESCE(config, '{}'::jsonb),
                            '{agent_node_id}',
                            to_jsonb($18::text),
                            true
                        ),
                        '{node_agent,host}',
                        to_jsonb($19::text),
                        true
                    ),
                    '{node_agent,source}',
                    to_jsonb('mqtt_heartbeat'::text),
                    true
                )
            END,
            last_seen = NOW()
        WHERE ($1::macaddr IS NOT NULL AND mac_eth = $1::macaddr)
           OR ($2::macaddr IS NOT NULL AND mac_wifi = $2::macaddr)
        "#,
    )
    .bind(mac_eth.as_deref())
    .bind(mac_wifi.as_deref())
    .bind(status.as_deref())
    .bind(uptime_seconds)
    .bind(cpu_percent)
    .bind(storage_used_bytes)
    .bind(memory_percent)
    .bind(memory_used_bytes)
    .bind(network_latency_ms)
    .bind(network_jitter_ms)
    .bind(uptime_percent_24h)
    .bind(ping_ms)
    .bind(ping_p50_30m_ms)
    .bind(ping_jitter_ms)
    .bind(mqtt_broker_rtt_ms)
    .bind(mqtt_broker_rtt_jitter_ms)
    .bind(ip_last.as_deref())
    .bind(agent_node_id.as_deref())
    .bind(agent_host.as_deref())
    .execute(db)
    .await;
}
