use super::rolling::RollingAverager;
use super::types::{Sample, SensorMeta};
use super::{TelemetryIngestor, COV_TOLERANCE, STATUS_OFFLINE, STATUS_ONLINE};
use crate::pipeline::IngestStats;
use crate::predictive_feed::{PredictiveFeed, PredictiveFeedItem};
use crate::telemetry::MetricRow;
use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::types::Json as SqlJson;
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

const OFFLINE_MULTIPLIER: i64 = 5;

impl TelemetryIngestor {
    pub async fn ensure_core_node_record(&self) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO nodes (id, name, status, last_seen, config, created_at)
            VALUES (
                '00000000-0000-0000-0000-000000000001'::uuid,
                'Core',
                'online',
                NOW(),
                jsonb_build_object('kind', 'core', 'system', true),
                NOW()
            )
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub fn new(
        pool: sqlx::PgPool,
        pipeline: crate::pipeline::PipelineHandle,
        offline_threshold: std::time::Duration,
        predictive_feed: Option<PredictiveFeed>,
        ack_tx: Option<mpsc::UnboundedSender<crate::ack::AckCommand>>,
    ) -> Self {
        let offline_threshold = ChronoDuration::from_std(offline_threshold)
            .unwrap_or_else(|_| ChronoDuration::seconds(5));
        Self {
            pool,
            pipeline,
            state: Arc::new(Mutex::new(super::state::IngestState::new())),
            offline_threshold,
            predictive_feed,
            ack_tx,
        }
    }

    pub fn stats(&self) -> Arc<IngestStats> {
        self.pipeline.stats()
    }

    pub async fn flush(&self) -> Result<()> {
        self.pipeline.flush().await
    }

    pub async fn ingest_metric(&self, metric: MetricRow) -> Result<u64> {
        let meta = match self.get_sensor_meta(&metric.sensor_id).await? {
            Some(meta) => meta,
            None => return Ok(0),
        };

        if meta.interval_seconds == 0 && meta.rolling_avg_seconds <= 0 {
            self.ensure_cov_last(&meta.sensor_id).await?;
        }

        let now = Utc::now();
        self.touch_status(&meta, now).await?;

        {
            let mut state = self.state.lock().await;
            let update_max = |map: &mut std::collections::HashMap<String, DateTime<Utc>>,
                              key: &str,
                              candidate: DateTime<Utc>| {
                if map.get(key).map(|prev| candidate > *prev).unwrap_or(true) {
                    map.insert(key.to_string(), candidate);
                }
            };
            update_max(
                &mut state.sensor_last_sample_ts,
                &meta.sensor_id,
                metric.timestamp,
            );
            update_max(
                &mut state.node_last_sample_ts,
                &meta.node_id,
                metric.timestamp,
            );
        }

        let sample = Sample {
            timestamp: metric.timestamp,
            value: metric.value,
            quality: metric.quality,
            samples: 1,
        };

        let rows = self.process_sample(&meta, sample).await?;
        if rows.is_empty()
            && meta.interval_seconds == 0
            && meta.rolling_avg_seconds <= 0
            && metric.seq.is_some()
            && metric.stream_id.is_some()
        {
            // Change-of-value sensors can drop duplicates without a DB write; still ACK the seq so
            // node-forwarder can advance/truncate.
            if let (Some(tx), Some(node_mqtt_id), Some(stream_id), Some(seq)) = (
                self.ack_tx.as_ref(),
                metric.source.clone(),
                metric.stream_id,
                metric.seq,
            ) {
                let _ = tx.send(crate::ack::AckCommand::Committed {
                    node_mqtt_id,
                    stream_id,
                    seqs: vec![seq],
                });
            }
        }

        let mut accepted = 0u64;
        for row in rows {
            self.pipeline
                .enqueue(MetricRow {
                    sensor_id: meta.sensor_id.clone(),
                    timestamp: row.timestamp,
                    value: row.value,
                    quality: row.quality,
                    source: metric.source.clone(),
                    seq: metric.seq,
                    stream_id: metric.stream_id,
                    backfill: metric.backfill,
                })
                .await?;
            self.enqueue_predictive_feed(&meta.sensor_id, &row);
            accepted += 1;
        }
        Ok(accepted)
    }

    pub async fn ingest_metrics<I>(&self, metrics: I) -> Result<u64>
    where
        I: IntoIterator<Item = MetricRow>,
    {
        let mut accepted = 0u64;
        for metric in metrics {
            accepted += self.ingest_metric(metric).await?;
        }
        Ok(accepted)
    }

    pub fn handle_loss_range(
        &self,
        node_mqtt_id: &str,
        stream_id: uuid::Uuid,
        start_seq: u64,
        end_seq: u64,
        dropped_at: Option<DateTime<Utc>>,
        reason: Option<String>,
    ) {
        let Some(tx) = self.ack_tx.as_ref() else {
            return;
        };
        let node_mqtt_id = node_mqtt_id.trim();
        if node_mqtt_id.is_empty() || start_seq == 0 || end_seq < start_seq {
            return;
        }
        let _ = tx.send(crate::ack::AckCommand::LossRange {
            node_mqtt_id: node_mqtt_id.to_string(),
            stream_id,
            start_seq,
            end_seq,
            dropped_at,
            reason,
        });
    }

    async fn resolve_node_uuid(
        &self,
        node_identifier: &str,
        coordinator_ieee: Option<&str>,
    ) -> Result<Option<String>> {
        let trimmed = node_identifier.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }
        if is_uuid_like(trimmed) {
            return Ok(Some(trimmed.to_string()));
        }

        {
            let state = self.state.lock().await;
            if let Some(mapped) = state.node_aliases.get(trimmed) {
                return Ok(Some(mapped.clone()));
            }
        }

        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT id::text
            FROM nodes
            WHERE config->>'agent_node_id' = $1
            LIMIT 1
            "#,
        )
        .bind(trimmed)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((resolved,)) = row {
            let mut state = self.state.lock().await;
            state
                .node_aliases
                .insert(trimmed.to_string(), resolved.clone());
            return Ok(Some(resolved));
        }

        let Some(coordinator_ieee) = coordinator_ieee
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let coordinator_ieee = coordinator_ieee.to_lowercase();
        let row: Option<(String,)> = sqlx::query_as(
            r#"
            SELECT id::text
            FROM nodes
            WHERE status != 'deleted'
              AND (
                mac_eth::text = $1
                OR mac_wifi::text = $1
              )
            LIMIT 1
            "#,
        )
        .bind(&coordinator_ieee)
        .fetch_optional(&self.pool)
        .await?;
        let Some((resolved,)) = row else {
            return Ok(None);
        };

        {
            let mut state = self.state.lock().await;
            state
                .node_aliases
                .insert(trimmed.to_string(), resolved.clone());
        }

        // Best-effort: persist the mapping so future status topics resolve without MAC hints.
        let _ = sqlx::query(
            r#"
            UPDATE nodes
            SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{agent_node_id}', to_jsonb($2::text), true)
            WHERE id = $1::uuid
              AND COALESCE(config->>'agent_node_id', '') = ''
            "#,
        )
        .bind(&resolved)
        .bind(trimmed)
        .execute(&self.pool)
        .await;

        Ok(Some(resolved))
    }

    pub async fn handle_node_status_payload(
        &self,
        node_id: &str,
        status: &str,
        received_at: DateTime<Utc>,
        captured_at: Option<DateTime<Utc>>,
        uptime_seconds: Option<i64>,
        cpu_percent: Option<f32>,
        storage_used_bytes: Option<i64>,
        heartbeat_interval_seconds: Option<f64>,
        coordinator_ieee: Option<&str>,
        cpu_percent_per_core: Option<Vec<f32>>,
        memory_percent: Option<f32>,
        memory_used_bytes: Option<i64>,
        ping_ms: Option<f64>,
        ping_p50_30m_ms: Option<f64>,
        ping_jitter_ms: Option<f64>,
        mqtt_broker_rtt_ms: Option<f64>,
        mqtt_broker_rtt_jitter_ms: Option<f64>,
        uptime_percent_24h: Option<f32>,
        analog_backend: Option<&str>,
        analog_health: Option<&serde_json::Value>,
        forwarder: Option<&serde_json::Value>,
    ) -> Result<()> {
        let Some(resolved_node_id) = self.resolve_node_uuid(node_id, coordinator_ieee).await?
        else {
            tracing::debug!(node_id = %node_id, "ignoring node status for unknown identifier");
            return Ok(());
        };

        let normalized = if status.eq_ignore_ascii_case(STATUS_OFFLINE) {
            STATUS_OFFLINE
        } else {
            STATUS_ONLINE
        };
        let now = received_at;

        let (status_changed, prev_last_seen) = {
            let mut state = self.state.lock().await;
            let prev_status = state.node_status.get(&resolved_node_id).cloned();
            let prev_last_seen = state.node_last_seen.get(&resolved_node_id).cloned();
            state.node_last_seen.insert(resolved_node_id.clone(), now);
            if let Some(interval) = heartbeat_interval_seconds
                .filter(|value| value.is_finite())
                .map(|value| value.max(0.0))
            {
                if interval > 0.0 {
                    state
                        .node_heartbeat_interval_seconds
                        .insert(resolved_node_id.clone(), interval);
                }
            }
            let status_changed = prev_status.as_deref() != Some(normalized);
            if status_changed {
                state
                    .node_status
                    .insert(resolved_node_id.clone(), normalized.to_string());
            }
            (status_changed, prev_last_seen)
        };

        if status_changed {
            self.set_node_status_db(
                &resolved_node_id,
                normalized,
                Some(now),
                uptime_seconds,
                cpu_percent,
                storage_used_bytes,
                memory_percent,
                memory_used_bytes,
                ping_ms,
                ping_p50_30m_ms,
                ping_jitter_ms,
                mqtt_broker_rtt_ms,
                mqtt_broker_rtt_jitter_ms,
                uptime_percent_24h,
            )
            .await?;
        } else if prev_last_seen.map(|prev| now > prev).unwrap_or(true) {
            self.update_node_last_seen_db(
                &resolved_node_id,
                now,
                uptime_seconds,
                cpu_percent,
                storage_used_bytes,
                memory_percent,
                memory_used_bytes,
                ping_ms,
                ping_p50_30m_ms,
                ping_jitter_ms,
                mqtt_broker_rtt_ms,
                mqtt_broker_rtt_jitter_ms,
                uptime_percent_24h,
            )
            .await?;
        }

        if normalized == STATUS_ONLINE {
            self.persist_node_health_metrics(
                &resolved_node_id,
                now,
                heartbeat_interval_seconds,
                cpu_percent,
                cpu_percent_per_core,
                memory_percent,
                storage_used_bytes,
                memory_used_bytes,
                ping_ms,
                ping_p50_30m_ms,
                ping_jitter_ms,
                mqtt_broker_rtt_ms,
                mqtt_broker_rtt_jitter_ms,
                uptime_percent_24h,
            )
            .await?;
        }

        if analog_backend.is_some() || analog_health.is_some() {
            self.persist_node_analog_state(&resolved_node_id, analog_backend, analog_health)
                .await?;
        }

        if let Some(forwarder) = forwarder {
            let _ = sqlx::query(
                r#"
                UPDATE nodes
                SET config = jsonb_set(
                    COALESCE(config, '{}'::jsonb),
                    '{forwarder}',
                    to_jsonb($2),
                    true
                )
                WHERE id = $1::uuid
                  AND status != 'deleted'
                "#,
            )
            .bind(&resolved_node_id)
            .bind(SqlJson(forwarder.clone()))
            .execute(&self.pool)
            .await;
        }

        if let Some(captured_at) = captured_at {
            // Captured time is not used for liveness; keep it as best-effort metadata for debugging.
            let _ = sqlx::query(
                r#"
                UPDATE nodes
                SET config = jsonb_set(
                    COALESCE(config, '{}'::jsonb),
                    '{agent_captured_at}',
                    to_jsonb($2::text),
                    true
                )
                WHERE id = $1::uuid
                  AND status != 'deleted'
                "#,
            )
            .bind(&resolved_node_id)
            .bind(captured_at.to_rfc3339())
            .execute(&self.pool)
            .await;
        }

        Ok(())
    }

    async fn persist_node_analog_state(
        &self,
        node_id: &str,
        analog_backend: Option<&str>,
        analog_health: Option<&serde_json::Value>,
    ) -> Result<()> {
        match (analog_backend, analog_health) {
            (None, None) => Ok(()),
            (Some(backend), Some(health)) => {
                sqlx::query(
                    r#"
                    UPDATE nodes
                    SET config = jsonb_set(
                        jsonb_set(COALESCE(config, '{}'::jsonb), '{analog_backend}', to_jsonb($2::text), true),
                        '{analog_health}',
                        to_jsonb($3),
                        true
                    )
                    WHERE id = $1::uuid
                    "#,
                )
                .bind(node_id)
                .bind(backend)
                .bind(SqlJson(health.clone()))
                .execute(&self.pool)
                .await?;
                Ok(())
            }
            (Some(backend), None) => {
                sqlx::query(
                    r#"
                    UPDATE nodes
                    SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{analog_backend}', to_jsonb($2::text), true)
                    WHERE id = $1::uuid
                    "#,
                )
                .bind(node_id)
                .bind(backend)
                .execute(&self.pool)
                .await?;
                Ok(())
            }
            (None, Some(health)) => {
                sqlx::query(
                    r#"
                    UPDATE nodes
                    SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{analog_health}', to_jsonb($2), true)
                    WHERE id = $1::uuid
                    "#,
                )
                .bind(node_id)
                .bind(SqlJson(health.clone()))
                .execute(&self.pool)
                .await?;
                Ok(())
            }
        }
    }

    async fn persist_node_health_metrics(
        &self,
        node_id: &str,
        timestamp: DateTime<Utc>,
        heartbeat_interval_seconds: Option<f64>,
        cpu_percent: Option<f32>,
        cpu_percent_per_core: Option<Vec<f32>>,
        memory_percent: Option<f32>,
        storage_used_bytes: Option<i64>,
        memory_used_bytes: Option<i64>,
        ping_ms: Option<f64>,
        ping_p50_30m_ms: Option<f64>,
        ping_jitter_ms: Option<f64>,
        mqtt_broker_rtt_ms: Option<f64>,
        mqtt_broker_rtt_jitter_ms: Option<f64>,
        uptime_percent_24h: Option<f32>,
    ) -> Result<()> {
        let interval_seconds = heartbeat_interval_seconds
            .filter(|value| value.is_finite())
            .map(|value| value.max(1.0))
            .unwrap_or(60.0)
            .round()
            .max(1.0) as i64;
        let mut metrics: Vec<(String, String, String, String, f64)> = Vec::new();
        if let Some(value) = cpu_percent {
            if value.is_finite() && value >= 0.0 {
                metrics.push((
                    "cpu_percent".to_string(),
                    "CPU Usage".to_string(),
                    "cpu_percent".to_string(),
                    "%".to_string(),
                    value as f64,
                ));
            }
        }
        if let Some(values) = cpu_percent_per_core {
            for (idx, value) in values.into_iter().enumerate() {
                if !value.is_finite() {
                    continue;
                }
                metrics.push((
                    format!("cpu_core_{}_percent", idx),
                    format!("CPU Core {} Usage", idx + 1),
                    "cpu_percent".to_string(),
                    "%".to_string(),
                    value as f64,
                ));
            }
        }
        if let Some(value) = memory_percent {
            if value.is_finite() && value >= 0.0 {
                metrics.push((
                    "memory_percent".to_string(),
                    "Memory Usage".to_string(),
                    "memory_percent".to_string(),
                    "%".to_string(),
                    value as f64,
                ));
            }
        }
        if let Some(bytes) = storage_used_bytes {
            if bytes >= 0 {
                metrics.push((
                    "storage_used_bytes".to_string(),
                    "Storage Used".to_string(),
                    "storage".to_string(),
                    "bytes".to_string(),
                    bytes as f64,
                ));
            }
        }
        if let Some(bytes) = memory_used_bytes {
            if bytes >= 0 {
                metrics.push((
                    "memory_used_bytes".to_string(),
                    "Memory Used".to_string(),
                    "memory".to_string(),
                    "bytes".to_string(),
                    bytes as f64,
                ));
            }
        }
        if let Some(latency) = ping_ms {
            if latency.is_finite() && latency >= 0.0 {
                metrics.push((
                    "ping_ms".to_string(),
                    "Ping".to_string(),
                    "ping".to_string(),
                    "ms".to_string(),
                    latency,
                ));
            }
        }
        if let Some(p50) = ping_p50_30m_ms {
            if p50.is_finite() && p50 >= 0.0 {
                metrics.push((
                    "ping_p50_30m_ms".to_string(),
                    "Ping (30m p50)".to_string(),
                    "ping".to_string(),
                    "ms".to_string(),
                    p50,
                ));
            }
        }
        if let Some(jitter) = ping_jitter_ms {
            if jitter.is_finite() && jitter >= 0.0 {
                metrics.push((
                    "ping_jitter_ms".to_string(),
                    "Ping Jitter".to_string(),
                    "ping".to_string(),
                    "ms".to_string(),
                    jitter,
                ));
            }
        }
        if let Some(latency) = mqtt_broker_rtt_ms {
            if latency.is_finite() && latency >= 0.0 {
                metrics.push((
                    "mqtt_broker_rtt_ms".to_string(),
                    "MQTT Broker RTT".to_string(),
                    "latency".to_string(),
                    "ms".to_string(),
                    latency,
                ));
            }
        }
        if let Some(jitter) = mqtt_broker_rtt_jitter_ms {
            if jitter.is_finite() && jitter >= 0.0 {
                metrics.push((
                    "mqtt_broker_rtt_jitter_ms".to_string(),
                    "MQTT Broker RTT Jitter".to_string(),
                    "latency".to_string(),
                    "ms".to_string(),
                    jitter,
                ));
            }
        }
        if let Some(uptime_percent) = uptime_percent_24h {
            if uptime_percent.is_finite() && uptime_percent >= 0.0 {
                metrics.push((
                    "uptime_percent_24h".to_string(),
                    "Uptime (24h)".to_string(),
                    "uptime_percent".to_string(),
                    "%".to_string(),
                    uptime_percent as f64,
                ));
            }
        }

        for (key, name, sensor_type, unit, value) in metrics {
            let sensor_id = self
                .ensure_node_health_sensor(
                    node_id,
                    &key,
                    &name,
                    &sensor_type,
                    &unit,
                    interval_seconds,
                )
                .await?;
            self.pipeline
                .enqueue(MetricRow {
                    sensor_id: sensor_id.clone(),
                    timestamp,
                    value,
                    quality: 0,
                    source: Some("node_health".to_string()),
                    seq: None,
                    stream_id: None,
                    backfill: false,
                })
                .await?;
        }

        Ok(())
    }

    async fn ensure_node_health_sensor(
        &self,
        node_id: &str,
        node_health_key: &str,
        name: &str,
        sensor_type: &str,
        unit: &str,
        interval_seconds: i64,
    ) -> Result<String> {
        let sensor_id = node_health_sensor_id(node_id, node_health_key);
        let interval_seconds = interval_seconds.clamp(1, i64::from(i32::MAX)) as i32;
        let row: (String,) = sqlx::query_as(
            r#"
            INSERT INTO sensors (sensor_id, node_id, name, type, unit, interval_seconds, rolling_avg_seconds, config)
            VALUES ($1, $2::uuid, $3, $4, $5, $6, 0, $7::jsonb)
            ON CONFLICT (sensor_id) DO UPDATE
            SET name = EXCLUDED.name,
                type = EXCLUDED.type,
                unit = EXCLUDED.unit,
                interval_seconds = EXCLUDED.interval_seconds,
                config = jsonb_set(
                    jsonb_set(
                        jsonb_set(COALESCE(sensors.config, '{}'::jsonb), '{source}', to_jsonb('node_health'::text), true),
                        '{node_health_key}',
                        to_jsonb($8::text),
                        true
                    ),
                    '{hidden}',
                    'false',
                    true
                ),
                deleted_at = NULL
            RETURNING sensor_id
            "#,
        )
        .bind(&sensor_id)
        .bind(node_id)
        .bind(name)
        .bind(sensor_type)
        .bind(unit)
        .bind(interval_seconds)
        .bind(json!({
            "source": "node_health",
            "node_health_key": node_health_key,
            "hidden": false,
        }))
        .bind(node_health_key)
        .fetch_one(&self.pool)
        .await?;

        {
            let mut state = self.state.lock().await;
            state.sensor_meta.insert(
                sensor_id.clone(),
                SensorMeta {
                    sensor_id: sensor_id.clone(),
                    node_id: node_id.to_string(),
                    interval_seconds: interval_seconds as i64,
                    rolling_avg_seconds: 0,
                },
            );
        }

        Ok(row.0)
    }

    pub async fn check_offline(&self) -> Result<()> {
        let now = Utc::now();
        let mut offline_sensors = Vec::new();
        let mut offline_nodes = Vec::new();

        {
            let mut state = self.state.lock().await;
            let node_floor = if self.offline_threshold > ChronoDuration::seconds(15) {
                self.offline_threshold
            } else {
                ChronoDuration::seconds(15)
            };
            let mut node_thresholds: std::collections::HashMap<String, ChronoDuration> =
                std::collections::HashMap::new();
            for meta in state.sensor_meta.values() {
                let threshold = derived_offline_threshold(meta, self.offline_threshold);
                node_thresholds
                    .entry(meta.node_id.clone())
                    .and_modify(|existing| {
                        if threshold > *existing {
                            *existing = threshold;
                        }
                    })
                    .or_insert(threshold);
            }
            for (node_id, last_seen) in state.node_last_seen.clone() {
                let is_online = state
                    .node_status
                    .get(&node_id)
                    .map(|status| status == STATUS_ONLINE)
                    .unwrap_or(false);
                let metric_last_seen = state.node_last_metric_seen.get(&node_id).copied();
                let effective_last_seen = match metric_last_seen {
                    Some(m) if m > last_seen => m,
                    _ => last_seen,
                };
                let heartbeat_interval_seconds =
                    state.node_heartbeat_interval_seconds.get(&node_id).copied();
                let sensor_threshold = node_thresholds.get(&node_id).copied();
                let base_threshold = sensor_threshold.unwrap_or(node_floor);
                let heartbeat_threshold = heartbeat_interval_seconds
                    .filter(|value| value.is_finite())
                    .filter(|value| *value > 0.0)
                    .and_then(|value| {
                        let ms = (value * (OFFLINE_MULTIPLIER as f64) * 1000.0).round() as i64;
                        if ms <= 0 {
                            return None;
                        }
                        Some(ChronoDuration::milliseconds(ms))
                    });
                let threshold = match heartbeat_threshold {
                    Some(candidate) if candidate > base_threshold => candidate,
                    _ => base_threshold,
                };
                let threshold = if threshold > node_floor {
                    threshold
                } else {
                    node_floor
                };
                if is_online && now - effective_last_seen > threshold {
                    state
                        .node_status
                        .insert(node_id.clone(), STATUS_OFFLINE.to_string());
                    offline_nodes.push(node_id);
                }
            }

            for (sensor_id, last_seen) in state.sensor_last_seen.clone() {
                let is_online = state
                    .sensor_status
                    .get(&sensor_id)
                    .map(|status| status == STATUS_ONLINE)
                    .unwrap_or(false);
                if !is_online {
                    continue;
                }

                let meta = match state.sensor_meta.get(&sensor_id) {
                    Some(meta) => meta,
                    None => continue,
                };

                let node_is_offline = state
                    .node_status
                    .get(&meta.node_id)
                    .map(|status| status == STATUS_OFFLINE)
                    .unwrap_or(false);
                if node_is_offline {
                    state
                        .sensor_status
                        .insert(sensor_id.clone(), STATUS_OFFLINE.to_string());
                    offline_sensors.push(sensor_id);
                    continue;
                }

                // Change-of-value sensors can legitimately be quiet for long stretches; avoid
                // marking them offline due to inactivity while their node is online.
                if meta.interval_seconds == 0 && meta.rolling_avg_seconds <= 0 {
                    continue;
                }

                let threshold = derived_offline_threshold(meta, self.offline_threshold);
                if now - last_seen > threshold {
                    state
                        .sensor_status
                        .insert(sensor_id.clone(), STATUS_OFFLINE.to_string());
                    offline_sensors.push(sensor_id);
                }
            }
        }

        for sensor_id in offline_sensors {
            self.set_sensor_status_db(&sensor_id, STATUS_OFFLINE)
                .await?;
        }
        for node_id in offline_nodes {
            self.set_node_status_db(
                &node_id,
                STATUS_OFFLINE,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
        }

        Ok(())
    }

    async fn process_sample(&self, meta: &SensorMeta, sample: Sample) -> Result<Vec<Sample>> {
        let mut state = self.state.lock().await;
        let mut processed = if meta.rolling_avg_seconds > 0 {
            let entry = state
                .rolling
                .entry(meta.sensor_id.clone())
                .or_insert_with(|| {
                    RollingAverager::new(meta.interval_seconds, meta.rolling_avg_seconds)
                });
            entry.add_sample(sample)
        } else {
            vec![sample]
        };

        if meta.interval_seconds == 0 && meta.rolling_avg_seconds <= 0 {
            let mut last = state.cov_last.get(&meta.sensor_id).copied();
            let mut rows = Vec::new();
            for entry in processed.drain(..) {
                if let Some((prev_value, prev_quality)) = last {
                    if (entry.value - prev_value).abs() <= COV_TOLERANCE
                        && entry.quality == prev_quality
                    {
                        tracing::debug!(
                            sensor = %meta.sensor_id,
                            value = entry.value,
                            quality = entry.quality,
                            "skipping COV duplicate"
                        );
                        continue;
                    }
                }
                last = Some((entry.value, entry.quality));
                rows.push(entry);
            }
            if let Some(last) = last {
                state.cov_last.insert(meta.sensor_id.clone(), last);
            }
            return Ok(rows);
        }

        Ok(processed)
    }

    fn enqueue_predictive_feed(&self, sensor_id: &str, sample: &Sample) {
        if let Some(feed) = &self.predictive_feed {
            feed.enqueue(PredictiveFeedItem {
                sensor_id: sensor_id.to_string(),
                timestamp: sample.timestamp,
                value: sample.value,
                quality: sample.quality,
            });
        }
    }
}

fn derived_offline_threshold(meta: &SensorMeta, floor: ChronoDuration) -> ChronoDuration {
    let mut threshold = floor;

    if meta.interval_seconds > 0 {
        let candidate =
            ChronoDuration::seconds(meta.interval_seconds.saturating_mul(OFFLINE_MULTIPLIER));
        if candidate > threshold {
            threshold = candidate;
        }
        return threshold;
    }

    if meta.rolling_avg_seconds > 0 {
        let candidate =
            ChronoDuration::seconds(meta.rolling_avg_seconds.saturating_mul(OFFLINE_MULTIPLIER));
        if candidate > threshold {
            threshold = candidate;
        }
        return threshold;
    }

    // Change-of-value sensors can legitimately be quiet for long stretches; use a safer minimum.
    let candidate = ChronoDuration::minutes(5);
    match candidate.cmp(&threshold) {
        Ordering::Greater => candidate,
        _ => threshold,
    }
}

fn is_uuid_like(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (idx, byte) in bytes.iter().copied().enumerate() {
        match idx {
            8 | 13 | 18 | 23 => {
                if byte != b'-' {
                    return false;
                }
            }
            _ => {
                if !byte.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

pub(crate) fn node_health_sensor_id(node_id: &str, key: &str) -> String {
    let payload = format!(
        "node_health|{}|{}",
        node_id.trim().to_lowercase(),
        key.trim().to_lowercase()
    );
    let digest = Sha256::digest(payload.as_bytes());
    let hex = format!("{digest:x}");
    hex.chars().take(24).collect()
}

#[cfg(test)]
mod tests {
    use super::node_health_sensor_id;

    #[test]
    fn node_health_sensor_id_is_stable_and_24_hex() {
        let id1 = node_health_sensor_id("A0E90393-7663-8B5F-ABCD-0123456789AB", "CPU_PERCENT");
        let id2 = node_health_sensor_id("a0e90393-7663-8b5f-abcd-0123456789ab", "cpu_percent");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 24);
        assert!(id1.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
