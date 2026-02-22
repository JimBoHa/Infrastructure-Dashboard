use super::types::SensorMeta;
use super::{TelemetryIngestor, STATUS_OFFLINE, STATUS_ONLINE};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::Row;

impl TelemetryIngestor {
    pub(in crate::ingest) async fn get_sensor_meta(
        &self,
        sensor_id: &str,
    ) -> Result<Option<SensorMeta>> {
        {
            let state = self.state.lock().await;
            if let Some(meta) = state.sensor_meta.get(sensor_id) {
                return Ok(Some(meta.clone()));
            }
        }

        let row = sqlx::query(
            r#"
            SELECT
                sensor_id,
                node_id::text as node_id,
                interval_seconds,
                rolling_avg_seconds,
                deleted_at
            FROM sensors
            WHERE sensor_id = $1
            "#,
        )
        .bind(sensor_id)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(row) => row,
            None => {
                tracing::warn!(sensor = %sensor_id, "unknown sensor in telemetry");
                return Ok(None);
            }
        };

        let deleted_at = row.try_get::<Option<DateTime<Utc>>, _>("deleted_at")?;
        if deleted_at.is_some() {
            tracing::debug!(sensor = %sensor_id, "ignoring telemetry for deleted sensor");
            return Ok(None);
        }

        let meta = SensorMeta {
            sensor_id: row.try_get::<String, _>("sensor_id")?,
            node_id: row.try_get::<String, _>("node_id")?,
            interval_seconds: row.try_get::<i32, _>("interval_seconds")? as i64,
            rolling_avg_seconds: row
                .try_get::<Option<i32>, _>("rolling_avg_seconds")?
                .unwrap_or(0) as i64,
        };

        let mut state = self.state.lock().await;
        state
            .sensor_meta
            .insert(meta.sensor_id.clone(), meta.clone());

        Ok(Some(meta))
    }

    pub(in crate::ingest) async fn ensure_cov_last(&self, sensor_id: &str) -> Result<()> {
        {
            let state = self.state.lock().await;
            if state.cov_last.contains_key(sensor_id) || state.cov_initialized.contains(sensor_id) {
                return Ok(());
            }
        }

        {
            let mut state = self.state.lock().await;
            state.cov_initialized.insert(sensor_id.to_string());
        }

        let row = sqlx::query(
            r#"
            SELECT value, quality
            FROM metrics
            WHERE sensor_id = $1
            ORDER BY ts DESC
            LIMIT 1
            "#,
        )
        .bind(sensor_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let value = row.try_get::<f64, _>("value")?;
            let quality = row.try_get::<i16, _>("quality")? as i32;
            let mut state = self.state.lock().await;
            state
                .cov_last
                .insert(sensor_id.to_string(), (value, quality));
        }

        Ok(())
    }

    pub(in crate::ingest) async fn touch_status(
        &self,
        meta: &SensorMeta,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let (sensor_changed, node_changed, prev_node_last_seen) = {
            let mut state = self.state.lock().await;
            let prev_sensor = state.sensor_status.get(&meta.sensor_id).cloned();
            let prev_node = state.node_status.get(&meta.node_id).cloned();
            let prev_node_last_seen = state.node_last_seen.get(&meta.node_id).cloned();

            state
                .sensor_last_seen
                .insert(meta.sensor_id.clone(), timestamp);
            state.node_last_seen.insert(meta.node_id.clone(), timestamp);
            state
                .node_last_metric_seen
                .insert(meta.node_id.clone(), timestamp);

            let sensor_changed = prev_sensor.as_deref() != Some(STATUS_ONLINE);
            if sensor_changed {
                state
                    .sensor_status
                    .insert(meta.sensor_id.clone(), STATUS_ONLINE.to_string());
            }

            let node_changed = prev_node.as_deref() != Some(STATUS_ONLINE);
            if node_changed {
                state
                    .node_status
                    .insert(meta.node_id.clone(), STATUS_ONLINE.to_string());
            }

            (sensor_changed, node_changed, prev_node_last_seen)
        };

        if sensor_changed {
            self.set_sensor_status_db(&meta.sensor_id, STATUS_ONLINE)
                .await?;
        }

        if node_changed {
            self.set_node_status_db(
                &meta.node_id,
                STATUS_ONLINE,
                Some(timestamp),
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
        } else if prev_node_last_seen
            .map(|prev| timestamp > prev)
            .unwrap_or(true)
        {
            self.update_node_last_seen_db(
                &meta.node_id,
                timestamp,
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

    pub(in crate::ingest) async fn set_sensor_status_db(
        &self,
        sensor_id: &str,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sensors
            SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{status}', to_jsonb($2::text), true)
            WHERE sensor_id = $1
            "#,
        )
        .bind(sensor_id)
        .bind(status)
        .execute(&self.pool)
        .await?;

        let message = if status == STATUS_OFFLINE {
            format!("Sensor {} offline", sensor_id)
        } else {
            format!("Sensor {} restored", sensor_id)
        };
        let alarm_status = if status == STATUS_OFFLINE {
            "firing"
        } else {
            "ok"
        };
        self.update_alarm(
            "Sensor Offline",
            alarm_status,
            Some(sensor_id),
            None,
            &message,
        )
        .await?;
        Ok(())
    }

    pub(in crate::ingest) async fn set_node_status_db(
        &self,
        node_id: &str,
        status: &str,
        last_seen: Option<DateTime<Utc>>,
        uptime_seconds: Option<i64>,
        cpu_percent: Option<f32>,
        storage_used_bytes: Option<i64>,
        memory_percent: Option<f32>,
        memory_used_bytes: Option<i64>,
        ping_ms: Option<f64>,
        ping_p50_30m_ms: Option<f64>,
        ping_jitter_ms: Option<f64>,
        mqtt_broker_rtt_ms: Option<f64>,
        mqtt_broker_rtt_jitter_ms: Option<f64>,
        uptime_percent_24h: Option<f32>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE nodes
            SET status = $2,
                last_seen = CASE
                    WHEN $3 IS NULL THEN last_seen
                    WHEN last_seen IS NULL OR $3 > last_seen THEN $3
                    ELSE last_seen
                END,
                uptime_seconds = CASE
                    WHEN $3 IS NULL THEN uptime_seconds
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($4, uptime_seconds)
                    ELSE uptime_seconds
                END,
                cpu_percent = CASE
                    WHEN $3 IS NULL THEN cpu_percent
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($5, cpu_percent)
                    ELSE cpu_percent
                END,
                storage_used_bytes = CASE
                    WHEN $3 IS NULL THEN storage_used_bytes
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($6, storage_used_bytes)
                    ELSE storage_used_bytes
                END,
                memory_percent = CASE
                    WHEN $3 IS NULL THEN memory_percent
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($7, memory_percent)
                    ELSE memory_percent
                END,
                memory_used_bytes = CASE
                    WHEN $3 IS NULL THEN memory_used_bytes
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($8, memory_used_bytes)
                    ELSE memory_used_bytes
                END,
                ping_ms = CASE
                    WHEN $3 IS NULL THEN ping_ms
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($9, ping_ms)
                    ELSE ping_ms
                END,
                ping_p50_30m_ms = CASE
                    WHEN $3 IS NULL THEN ping_p50_30m_ms
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($10, ping_p50_30m_ms)
                    ELSE ping_p50_30m_ms
                END,
                ping_jitter_ms = CASE
                    WHEN $3 IS NULL THEN ping_jitter_ms
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($11, ping_jitter_ms)
                    ELSE ping_jitter_ms
                END,
                mqtt_broker_rtt_ms = CASE
                    WHEN $3 IS NULL THEN mqtt_broker_rtt_ms
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($12, mqtt_broker_rtt_ms)
                    ELSE mqtt_broker_rtt_ms
                END,
                mqtt_broker_rtt_jitter_ms = CASE
                    WHEN $3 IS NULL THEN mqtt_broker_rtt_jitter_ms
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($13, mqtt_broker_rtt_jitter_ms)
                    ELSE mqtt_broker_rtt_jitter_ms
                END,
                network_latency_ms = CASE
                    WHEN $3 IS NULL THEN network_latency_ms
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($12, network_latency_ms)
                    ELSE network_latency_ms
                END,
                network_jitter_ms = CASE
                    WHEN $3 IS NULL THEN network_jitter_ms
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($13, network_jitter_ms)
                    ELSE network_jitter_ms
                END,
                uptime_percent_24h = CASE
                    WHEN $3 IS NULL THEN uptime_percent_24h
                    WHEN last_seen IS NULL OR $3 > last_seen THEN COALESCE($14, uptime_percent_24h)
                    ELSE uptime_percent_24h
                END
            WHERE id = $1::uuid
              AND status != 'deleted'
            "#,
        )
        .bind(node_id)
        .bind(status)
        .bind(last_seen)
        .bind(uptime_seconds)
        .bind(cpu_percent)
        .bind(storage_used_bytes)
        .bind(memory_percent)
        .bind(memory_used_bytes)
        .bind(ping_ms)
        .bind(ping_p50_30m_ms)
        .bind(ping_jitter_ms)
        .bind(mqtt_broker_rtt_ms)
        .bind(mqtt_broker_rtt_jitter_ms)
        .bind(uptime_percent_24h)
        .execute(&self.pool)
        .await?;

        let message = if status == STATUS_OFFLINE {
            format!("Node {} offline", node_id)
        } else {
            format!("Node {} restored", node_id)
        };
        let alarm_status = if status == STATUS_OFFLINE {
            "firing"
        } else {
            "ok"
        };
        self.update_alarm("Node Offline", alarm_status, None, Some(node_id), &message)
            .await?;
        Ok(())
    }

    pub(in crate::ingest) async fn update_node_last_seen_db(
        &self,
        node_id: &str,
        last_seen: DateTime<Utc>,
        uptime_seconds: Option<i64>,
        cpu_percent: Option<f32>,
        storage_used_bytes: Option<i64>,
        memory_percent: Option<f32>,
        memory_used_bytes: Option<i64>,
        ping_ms: Option<f64>,
        ping_p50_30m_ms: Option<f64>,
        ping_jitter_ms: Option<f64>,
        mqtt_broker_rtt_ms: Option<f64>,
        mqtt_broker_rtt_jitter_ms: Option<f64>,
        uptime_percent_24h: Option<f32>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE nodes
            SET last_seen = $2
              , uptime_seconds = COALESCE($3, uptime_seconds)
              , cpu_percent = COALESCE($4, cpu_percent)
              , storage_used_bytes = COALESCE($5, storage_used_bytes)
              , memory_percent = COALESCE($6, memory_percent)
              , memory_used_bytes = COALESCE($7, memory_used_bytes)
              , ping_ms = COALESCE($8, ping_ms)
              , ping_p50_30m_ms = COALESCE($9, ping_p50_30m_ms)
              , ping_jitter_ms = COALESCE($10, ping_jitter_ms)
              , mqtt_broker_rtt_ms = COALESCE($11, mqtt_broker_rtt_ms)
              , mqtt_broker_rtt_jitter_ms = COALESCE($12, mqtt_broker_rtt_jitter_ms)
              , network_latency_ms = COALESCE($11, network_latency_ms)
              , network_jitter_ms = COALESCE($12, network_jitter_ms)
              , uptime_percent_24h = COALESCE($13, uptime_percent_24h)
            WHERE id = $1::uuid
              AND status != 'deleted'
              AND (last_seen IS NULL OR $2 > last_seen)
            "#,
        )
        .bind(node_id)
        .bind(last_seen)
        .bind(uptime_seconds)
        .bind(cpu_percent)
        .bind(storage_used_bytes)
        .bind(memory_percent)
        .bind(memory_used_bytes)
        .bind(ping_ms)
        .bind(ping_p50_30m_ms)
        .bind(ping_jitter_ms)
        .bind(mqtt_broker_rtt_ms)
        .bind(mqtt_broker_rtt_jitter_ms)
        .bind(uptime_percent_24h)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_alarm(
        &self,
        name: &str,
        status: &str,
        sensor_id: Option<&str>,
        node_id: Option<&str>,
        message: &str,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let (alarm_id, previous_status) = if let Some(sensor_id) = sensor_id {
            if let Some(row) =
                sqlx::query("SELECT id, status FROM alarms WHERE name = $1 AND sensor_id = $2")
                    .bind(name)
                    .bind(sensor_id)
                    .fetch_optional(&mut *tx)
                    .await?
            {
                let alarm_id: i64 = row.try_get("id")?;
                let current_status: String = row.try_get("status")?;
                (alarm_id, current_status)
            } else {
                let rule_json = format!(r#"{{"type":"{}"}}"#, name);
                let row = sqlx::query(
                    "INSERT INTO alarms (name, sensor_id, rule, status) VALUES ($1, $2, $3::jsonb, 'ok') RETURNING id, status",
                )
                .bind(name)
                .bind(sensor_id)
                .bind(rule_json)
                .fetch_one(&mut *tx)
                .await?;
                let alarm_id: i64 = row.try_get("id")?;
                let current_status: String = row.try_get("status")?;
                (alarm_id, current_status)
            }
        } else if let Some(node_id) = node_id {
            if let Some(row) =
                sqlx::query("SELECT id, status FROM alarms WHERE name = $1 AND node_id = $2::uuid")
                    .bind(name)
                    .bind(node_id)
                    .fetch_optional(&mut *tx)
                    .await?
            {
                let alarm_id: i64 = row.try_get("id")?;
                let current_status: String = row.try_get("status")?;
                (alarm_id, current_status)
            } else {
                let rule_json = format!(r#"{{"type":"{}"}}"#, name);
                let row = sqlx::query(
                    "INSERT INTO alarms (name, node_id, rule, status) VALUES ($1, $2::uuid, $3::jsonb, 'ok') RETURNING id, status",
                )
                .bind(name)
                .bind(node_id)
                .bind(rule_json)
                .fetch_one(&mut *tx)
                .await?;
                let alarm_id: i64 = row.try_get("id")?;
                let current_status: String = row.try_get("status")?;
                (alarm_id, current_status)
            }
        } else {
            return Ok(());
        };

        if previous_status == "ok" && status == "ok" {
            tx.commit().await?;
            return Ok(());
        }

        if status == "firing" {
            sqlx::query("UPDATE alarms SET status = $1, last_fired = $2 WHERE id = $3")
                .bind(status)
                .bind(Utc::now())
                .bind(alarm_id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE alarms SET status = $1 WHERE id = $2")
                .bind(status)
                .bind(alarm_id)
                .execute(&mut *tx)
                .await?;
        }

        if let Some(sensor_id) = sensor_id {
            sqlx::query(
                "INSERT INTO alarm_events (alarm_id, sensor_id, status, message, origin) VALUES ($1, $2, $3, $4, 'threshold')",
            )
            .bind(alarm_id)
            .bind(sensor_id)
            .bind(status)
            .bind(message)
            .execute(&mut *tx)
            .await?;
        } else if let Some(node_id) = node_id {
            sqlx::query(
                "INSERT INTO alarm_events (alarm_id, node_id, status, message, origin) VALUES ($1, $2::uuid, $3, $4, 'threshold')",
            )
            .bind(alarm_id)
            .bind(node_id)
            .bind(status)
            .bind(message)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
