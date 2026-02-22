use crate::config::Config;
use crate::ingest::TelemetryIngestor;
use crate::telemetry::parse_mqtt_payload;
use anyhow::Result;
use chrono::{DateTime, Utc};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde_json::json;
use simd_json::prelude::ValueAsScalar;
use simd_json::BorrowedValue;
use tokio::time::{sleep, Duration};

pub async fn run_listener(config: Config, ingestor: TelemetryIngestor) -> Result<()> {
    let telemetry_filter = format!("{}/+/+/telemetry", config.mqtt_topic_prefix);
    let status_filter = format!("{}/+/status", config.mqtt_topic_prefix);
    let loss_filter = format!("{}/+/loss", config.mqtt_topic_prefix);
    loop {
        let mut mqttoptions = MqttOptions::new(
            config.mqtt_client_id.clone(),
            config.mqtt_host.clone(),
            config.mqtt_port,
        );
        mqttoptions.set_keep_alive(config.mqtt_keepalive());
        if let Some(username) = &config.mqtt_username {
            mqttoptions.set_credentials(
                username.clone(),
                config.mqtt_password.clone().unwrap_or_default(),
            );
        }

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 32);
        let stats = ingestor.stats();

        match client
            .subscribe(telemetry_filter.clone(), QoS::AtLeastOnce)
            .await
        {
            Ok(_) => {
                tracing::info!(topic=%telemetry_filter, "subscribed to telemetry feed");
                stats.set_mqtt_connected(true);
            }
            Err(err) => {
                tracing::warn!(error=%err, "failed to subscribe to MQTT; retrying");
                sleep(Duration::from_secs(2)).await;
                continue;
            }
        }
        if let Err(err) = client
            .subscribe(status_filter.clone(), QoS::AtLeastOnce)
            .await
        {
            tracing::warn!(error=%err, "failed to subscribe to status feed; retrying");
            stats.set_mqtt_connected(false);
            sleep(Duration::from_secs(2)).await;
            continue;
        }
        if let Err(err) = client
            .subscribe(loss_filter.clone(), QoS::AtLeastOnce)
            .await
        {
            tracing::warn!(error=%err, "failed to subscribe to loss feed; retrying");
            stats.set_mqtt_connected(false);
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    let received_at = Utc::now();
                    let mut payload = publish.payload.to_vec();
                    if publish.topic.ends_with("/status") {
                        let parts: Vec<&str> = publish.topic.split('/').collect();
                        if parts.len() == 3 {
                            if let Some(status) = parse_status_payload(&mut payload) {
                                if let Err(err) = ingestor
                                    .handle_node_status_payload(
                                        parts[1],
                                        &status.status,
                                        received_at,
                                        status.captured_at,
                                        status.uptime_seconds,
                                        status.cpu_percent,
                                        status.storage_used_bytes,
                                        status.heartbeat_interval_seconds,
                                        status.coordinator_ieee.as_deref(),
                                        status.cpu_percent_per_core,
                                        status.memory_percent,
                                        status.memory_used_bytes,
                                        status.ping_ms,
                                        status.ping_p50_30m_ms,
                                        status.ping_jitter_ms,
                                        status.mqtt_broker_rtt_ms,
                                        status.mqtt_broker_rtt_jitter_ms,
                                        status.uptime_percent_24h,
                                        status.analog_backend.as_deref(),
                                        status.analog_health.as_ref(),
                                        status.forwarder.as_ref(),
                                    )
                                    .await
                                {
                                    tracing::warn!(error=%err, "failed to handle node status");
                                }
                            }
                        }
                        continue;
                    }

                    if publish.topic.ends_with("/loss") {
                        let parts: Vec<&str> = publish.topic.split('/').collect();
                        if parts.len() == 3 {
                            if let Some(loss) = parse_loss_payload(&mut payload) {
                                ingestor.handle_loss_range(
                                    parts[1],
                                    loss.stream_id,
                                    loss.start_seq,
                                    loss.end_seq,
                                    loss.dropped_at,
                                    loss.reason,
                                );
                            }
                        }
                        continue;
                    }

                    match parse_mqtt_payload(
                        &config.mqtt_topic_prefix,
                        &publish.topic,
                        &mut payload,
                    ) {
                        Ok(Some(metric)) => {
                            if let Err(err) = ingestor.ingest_metric(metric).await {
                                tracing::warn!(error=%err, "failed to ingest MQTT metric");
                            }
                        }
                        Ok(None) => {}
                        Err(err) => {
                            tracing::warn!(error=%err, topic=%publish.topic, "failed to decode MQTT payload")
                        }
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    stats.set_mqtt_connected(false);
                    tracing::warn!(error=%err, "MQTT connection dropped; reconnecting");
                    break;
                }
            }
        }

        sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Debug)]
struct ParsedLossPayload {
    stream_id: uuid::Uuid,
    start_seq: u64,
    end_seq: u64,
    dropped_at: Option<DateTime<Utc>>,
    reason: Option<String>,
}

fn parse_loss_payload(payload: &mut [u8]) -> Option<ParsedLossPayload> {
    if payload.is_empty() {
        return None;
    }

    #[derive(serde::Deserialize)]
    struct WireLoss<'a> {
        #[serde(borrow)]
        stream_id: &'a str,
        start_seq: u64,
        end_seq: u64,
        #[serde(default)]
        dropped_at: Option<&'a str>,
        #[serde(default)]
        reason: Option<&'a str>,
    }

    let parsed: WireLoss = serde_json::from_slice(payload).ok()?;
    let stream_id = uuid::Uuid::parse_str(parsed.stream_id.trim()).ok()?;
    let dropped_at = parsed
        .dropped_at
        .and_then(|raw| DateTime::parse_from_rfc3339(raw.trim()).ok())
        .map(|ts| ts.with_timezone(&Utc));
    let reason = parsed
        .reason
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string());
    Some(ParsedLossPayload {
        stream_id,
        start_seq: parsed.start_seq,
        end_seq: parsed.end_seq,
        dropped_at,
        reason,
    })
}

struct ParsedNodeStatus {
    status: String,
    captured_at: Option<DateTime<Utc>>,
    uptime_seconds: Option<i64>,
    cpu_percent: Option<f32>,
    storage_used_bytes: Option<i64>,
    heartbeat_interval_seconds: Option<f64>,
    coordinator_ieee: Option<String>,
    cpu_percent_per_core: Option<Vec<f32>>,
    memory_percent: Option<f32>,
    memory_used_bytes: Option<i64>,
    ping_ms: Option<f64>,
    ping_p50_30m_ms: Option<f64>,
    ping_jitter_ms: Option<f64>,
    mqtt_broker_rtt_ms: Option<f64>,
    mqtt_broker_rtt_jitter_ms: Option<f64>,
    uptime_percent_24h: Option<f32>,
    analog_backend: Option<String>,
    analog_health: Option<serde_json::Value>,
    forwarder: Option<serde_json::Value>,
}

fn parse_status_payload(payload: &mut [u8]) -> Option<ParsedNodeStatus> {
    if payload.is_empty() {
        return None;
    }

    let mut saw_non_space = None;
    for byte in payload.iter().copied() {
        if !byte.is_ascii_whitespace() {
            saw_non_space = Some(byte);
            break;
        }
    }

    if saw_non_space == Some(b'{') {
        if let Ok(BorrowedValue::Object(obj)) = simd_json::to_borrowed_value(payload) {
            let status = obj
                .get("status")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())?
                .to_string();
            let captured_at = obj
                .get("ts")
                .and_then(|value| value.as_str())
                .and_then(|raw| DateTime::parse_from_rfc3339(raw.trim()).ok())
                .map(|parsed| parsed.with_timezone(&Utc));
            let uptime_seconds = obj
                .get("uptime_seconds")
                .and_then(|value| value.as_i64().or_else(|| value.as_u64().map(|v| v as i64)));
            let cpu_percent = obj
                .get("cpu_percent")
                .and_then(|value| value.as_f64())
                .map(|value| value as f32);
            let storage_used_bytes = obj
                .get("storage_used_bytes")
                .and_then(|value| value.as_i64().or_else(|| value.as_u64().map(|v| v as i64)));
            let heartbeat_interval_seconds = obj.get("heartbeats").and_then(|value| {
                value
                    .as_f64()
                    .or_else(|| value.as_i64().map(|v| v as f64))
                    .or_else(|| value.as_u64().map(|v| v as f64))
            });
            let cpu_percent_per_core = match obj.get("cpu_percent_per_core") {
                Some(BorrowedValue::Array(entries)) => {
                    let mut values = Vec::new();
                    for entry in entries {
                        if let Some(val) = entry
                            .as_f64()
                            .or_else(|| entry.as_i64().map(|v| v as f64))
                            .or_else(|| entry.as_u64().map(|v| v as f64))
                        {
                            if val.is_finite() {
                                values.push(val as f32);
                            }
                        }
                    }
                    if values.is_empty() {
                        None
                    } else {
                        Some(values)
                    }
                }
                _ => None,
            };
            let memory_percent = obj
                .get("memory_percent")
                .and_then(|value| value.as_f64())
                .map(|value| value as f32);
            let memory_used_bytes = obj
                .get("memory_used_bytes")
                .and_then(|value| value.as_i64().or_else(|| value.as_u64().map(|v| v as i64)));
            let ping_ms = obj.get("ping_ms").and_then(|value| value.as_f64());
            let ping_p50_30m_ms = obj.get("ping_p50_30m_ms").and_then(|value| value.as_f64());
            let ping_jitter_ms = obj.get("ping_jitter_ms").and_then(|value| value.as_f64());
            let mqtt_broker_rtt_ms = obj
                .get("mqtt_broker_rtt_ms")
                .and_then(|value| value.as_f64())
                .or_else(|| {
                    obj.get("network_latency_ms")
                        .and_then(|value| value.as_f64())
                });
            let mqtt_broker_rtt_jitter_ms = obj
                .get("mqtt_broker_rtt_jitter_ms")
                .and_then(|value| value.as_f64())
                .or_else(|| {
                    obj.get("network_jitter_ms")
                        .and_then(|value| value.as_f64())
                });
            let uptime_percent_24h = obj
                .get("uptime_percent_24h")
                .and_then(|value| value.as_f64())
                .map(|value| value as f32);
            let coordinator_ieee = match obj.get("mesh") {
                Some(BorrowedValue::Object(mesh)) => mesh
                    .get("coordinator_ieee")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string()),
                _ => None,
            };

            let analog_backend = obj
                .get("analog_backend")
                .and_then(|value| value.as_str())
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_string());
            let analog_health = match obj.get("analog_health") {
                Some(BorrowedValue::Object(health)) => {
                    let ok = health
                        .get("ok")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false);
                    let chip_id = health
                        .get("chip_id")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string());
                    let last_error = health
                        .get("last_error")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string());
                    let last_ok_at = health
                        .get("last_ok_at")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string());
                    Some(json!({
                        "ok": ok,
                        "chip_id": chip_id,
                        "last_error": last_error,
                        "last_ok_at": last_ok_at,
                    }))
                }
                _ => None,
            };

            let forwarder = match obj.get("forwarder") {
                Some(BorrowedValue::Object(forwarder)) => {
                    let queue_len = forwarder.get("queue_len").and_then(|value| {
                        value.as_i64().or_else(|| value.as_u64().map(|v| v as i64))
                    });
                    let dropped_samples = forwarder.get("dropped_samples").and_then(|value| {
                        value.as_i64().or_else(|| value.as_u64().map(|v| v as i64))
                    });
                    let last_error = forwarder
                        .get("last_error")
                        .and_then(|value| value.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(|value| value.to_string());

                    let spool = match forwarder.get("spool") {
                        Some(BorrowedValue::Object(spool)) => {
                            let stream_id = spool
                                .get("stream_id")
                                .and_then(|value| value.as_str())
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(|value| value.to_string());
                            let acked_seq = spool.get("acked_seq").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let next_seq = spool.get("next_seq").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let spool_bytes = spool.get("spool_bytes").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let max_spool_bytes = spool.get("max_spool_bytes").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let keep_free_bytes = spool.get("keep_free_bytes").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let free_bytes = spool.get("free_bytes").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let backlog_samples = spool.get("backlog_samples").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let estimated_drain_seconds =
                                spool.get("estimated_drain_seconds").and_then(|value| {
                                    value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                                });
                            let losses_pending = spool.get("losses_pending").and_then(|value| {
                                value.as_u64().or_else(|| value.as_i64().map(|v| v as u64))
                            });
                            let oldest_unacked_timestamp_ms =
                                spool.get("oldest_unacked_timestamp_ms").and_then(|value| {
                                    value.as_i64().or_else(|| value.as_u64().map(|v| v as i64))
                                });
                            let losses = match spool.get("losses") {
                                Some(BorrowedValue::Array(entries)) => {
                                    let mut out = Vec::new();
                                    for entry in entries {
                                        let BorrowedValue::Object(obj) = entry else {
                                            continue;
                                        };
                                        let start_seq = obj.get("start_seq").and_then(|value| {
                                            value
                                                .as_u64()
                                                .or_else(|| value.as_i64().map(|v| v as u64))
                                        });
                                        let end_seq = obj.get("end_seq").and_then(|value| {
                                            value
                                                .as_u64()
                                                .or_else(|| value.as_i64().map(|v| v as u64))
                                        });
                                        let dropped_at = obj
                                            .get("dropped_at")
                                            .and_then(|value| value.as_str())
                                            .map(str::trim)
                                            .filter(|value| !value.is_empty())
                                            .map(|value| value.to_string());
                                        out.push(json!({
                                            "start_seq": start_seq,
                                            "end_seq": end_seq,
                                            "dropped_at": dropped_at,
                                        }));
                                    }
                                    Some(out)
                                }
                                _ => None,
                            };

                            Some(json!({
                                "stream_id": stream_id,
                                "acked_seq": acked_seq,
                                "next_seq": next_seq,
                                "spool_bytes": spool_bytes,
                                "max_spool_bytes": max_spool_bytes,
                                "keep_free_bytes": keep_free_bytes,
                                "free_bytes": free_bytes,
                                "backlog_samples": backlog_samples,
                                "estimated_drain_seconds": estimated_drain_seconds,
                                "losses_pending": losses_pending,
                                "oldest_unacked_timestamp_ms": oldest_unacked_timestamp_ms,
                                "losses": losses,
                            }))
                        }
                        _ => None,
                    };

                    Some(json!({
                        "queue_len": queue_len,
                        "dropped_samples": dropped_samples,
                        "last_error": last_error,
                        "spool": spool,
                    }))
                }
                _ => None,
            };

            return Some(ParsedNodeStatus {
                status,
                captured_at,
                uptime_seconds,
                cpu_percent,
                storage_used_bytes,
                heartbeat_interval_seconds,
                coordinator_ieee,
                cpu_percent_per_core,
                memory_percent,
                memory_used_bytes,
                ping_ms,
                ping_p50_30m_ms,
                ping_jitter_ms,
                mqtt_broker_rtt_ms,
                mqtt_broker_rtt_jitter_ms,
                uptime_percent_24h,
                analog_backend,
                analog_health,
                forwarder,
            });
        }
    }

    let text = std::str::from_utf8(payload).unwrap_or("").trim();
    if text.is_empty() {
        return None;
    }

    Some(ParsedNodeStatus {
        status: text.to_string(),
        captured_at: None,
        uptime_seconds: None,
        cpu_percent: None,
        storage_used_bytes: None,
        heartbeat_interval_seconds: None,
        coordinator_ieee: None,
        cpu_percent_per_core: None,
        memory_percent: None,
        memory_used_bytes: None,
        ping_ms: None,
        ping_p50_30m_ms: None,
        ping_jitter_ms: None,
        mqtt_broker_rtt_ms: None,
        mqtt_broker_rtt_jitter_ms: None,
        uptime_percent_24h: None,
        analog_backend: None,
        analog_health: None,
        forwarder: None,
    })
}

#[cfg(test)]
mod tests {
    use super::parse_status_payload;

    #[test]
    fn parse_status_payload_extracts_health_fields() {
        let mut payload = br#"{
            "status":"online",
            "ts":"2026-01-11T00:00:00Z",
            "uptime_seconds": 123,
            "cpu_percent": 12.5,
            "cpu_percent_per_core":[10,11.5,12.0,16],
            "memory_percent": 42.0,
            "memory_used_bytes": 1000,
            "storage_used_bytes": 2000,
            "ping_ms": 12.1,
            "ping_p50_30m_ms": 11.4,
            "ping_jitter_ms": 1.5,
            "mqtt_broker_rtt_ms": 15.2,
            "mqtt_broker_rtt_jitter_ms": 3.1,
            "uptime_percent_24h": 99.5,
            "heartbeats": 30
        }"#
        .to_vec();
        let parsed = parse_status_payload(&mut payload).expect("parsed");
        assert_eq!(parsed.status, "online");
        assert_eq!(parsed.uptime_seconds, Some(123));
        assert!(parsed.cpu_percent.unwrap() > 12.0);
        assert_eq!(
            parsed.cpu_percent_per_core.as_ref().map(|v| v.len()),
            Some(4)
        );
        assert_eq!(parsed.memory_percent, Some(42.0));
        assert_eq!(parsed.memory_used_bytes, Some(1000));
        assert_eq!(parsed.storage_used_bytes, Some(2000));
        assert!(parsed.ping_ms.unwrap() > 12.0);
        assert!(parsed.ping_p50_30m_ms.unwrap() > 11.0);
        assert!(parsed.ping_jitter_ms.unwrap() > 1.0);
        assert!(parsed.mqtt_broker_rtt_ms.unwrap() > 15.0);
        assert!(parsed.mqtt_broker_rtt_jitter_ms.unwrap() > 3.0);
        assert_eq!(parsed.uptime_percent_24h, Some(99.5));
        assert_eq!(parsed.heartbeat_interval_seconds, Some(30.0));
    }
}
