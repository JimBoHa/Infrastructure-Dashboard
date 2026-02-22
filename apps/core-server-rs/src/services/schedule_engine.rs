use crate::config::CoreConfig;
use anyhow::Result;
use chrono::{DateTime, Datelike, Local, Utc};
use rrule::RRuleSet;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use super::mqtt::MqttPublisher;

#[derive(Debug, Clone)]
pub struct ScheduleEngine {
    pool: PgPool,
    mqtt: Arc<MqttPublisher>,
    config: CoreConfig,
}

#[derive(sqlx::FromRow)]
struct ScheduleEngineRow {
    id: i64,
    rrule: String,
    blocks: JsonValue,
    conditions: JsonValue,
    actions: JsonValue,
}

#[derive(Debug, Clone, Copy)]
struct ParsedBlock {
    day_index: u32,
    start_hour: u32,
    start_minute: u32,
    end_hour: u32,
    end_minute: u32,
}

impl ScheduleEngine {
    pub fn new(pool: PgPool, mqtt: Arc<MqttPublisher>, config: CoreConfig) -> Self {
        Self { pool, mqtt, config }
    }

    pub fn start(self, cancel: CancellationToken) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(
                self.config.schedule_poll_interval_seconds.max(5),
            ));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {
                        if let Err(err) = self.tick().await {
                            tracing::warn!(error = %err, "schedule engine tick failed");
                        }
                    }
                }
            }
        });
    }

    async fn tick(&self) -> Result<()> {
        let schedules: Vec<ScheduleEngineRow> = sqlx::query_as(
            r#"
            SELECT
                id,
                rrule,
                COALESCE(blocks, '[]'::jsonb) as blocks,
                COALESCE(conditions, '[]'::jsonb) as conditions,
                COALESCE(actions, '[]'::jsonb) as actions
            FROM schedules
            ORDER BY id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let lookback =
            chrono::Duration::seconds(self.config.schedule_poll_interval_seconds as i64 + 5);
        let window_start = now - lookback;

        for schedule in schedules {
            if Self::has_blocks(&schedule.blocks) {
                self.evaluate_blocks(
                    schedule.id,
                    &schedule.blocks,
                    &schedule.conditions,
                    &schedule.actions,
                    window_start,
                    now,
                )
                .await?;
                continue;
            }

            if !self.is_due(&schedule.rrule, window_start, now) {
                continue;
            }
            if !self.conditions_met(&schedule.conditions).await? {
                continue;
            }
            self.execute_actions(schedule.id, &schedule.actions).await?;
        }

        Ok(())
    }

    fn has_blocks(blocks: &JsonValue) -> bool {
        matches!(blocks, JsonValue::Array(items) if !items.is_empty())
    }

    async fn evaluate_blocks(
        &self,
        schedule_id: i64,
        blocks: &JsonValue,
        conditions: &JsonValue,
        actions: &JsonValue,
        window_start: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let parsed_blocks = Self::parse_blocks(blocks);
        if parsed_blocks.is_empty() {
            return Ok(());
        }

        let window_start_local = window_start.with_timezone(&Local);
        let now_local = now.with_timezone(&Local);
        let mut date = window_start_local.date_naive();
        let end_date = now_local.date_naive();

        while date <= end_date {
            let weekday = date.weekday().num_days_from_monday();
            for block in &parsed_blocks {
                if block.day_index != weekday {
                    continue;
                }

                let Some(start_naive) = date.and_hms_opt(block.start_hour, block.start_minute, 0)
                else {
                    continue;
                };
                let Some(mut end_naive) = date.and_hms_opt(block.end_hour, block.end_minute, 0)
                else {
                    continue;
                };
                if end_naive <= start_naive {
                    end_naive += chrono::Duration::days(1);
                }

                let resolved = crate::time::resolve_block_interval(&Local, start_naive, end_naive);
                let resolved = match resolved {
                    Ok(resolved) => resolved,
                    Err(err) => {
                        tracing::warn!(
                            schedule_id,
                            start_local = %start_naive,
                            end_local = %end_naive,
                            error = %err,
                            "schedule block time resolution failed"
                        );
                        continue;
                    }
                };

                let block_start = resolved.start_utc;
                let block_end = resolved.end_utc;
                let should_log_resolution = (block_start > window_start && block_start <= now)
                    || (block_end > window_start && block_end <= now);
                if should_log_resolution && !resolved.warnings.is_empty() {
                    tracing::warn!(
                        schedule_id,
                        start_local = %start_naive,
                        end_local = %end_naive,
                        warnings = ?resolved.warnings,
                        "schedule block required DST resolution adjustments"
                    );
                }

                if block_start > window_start && block_start <= now {
                    if self.conditions_met(conditions).await? {
                        self.execute_actions(schedule_id, actions).await?;
                    }
                }

                if block_end > window_start && block_end <= now {
                    let end_actions = Self::derive_block_end_actions(actions);
                    if Self::has_blocks(&end_actions) {
                        self.execute_actions(schedule_id, &end_actions).await?;
                    }
                }
            }
            date += chrono::Duration::days(1);
        }

        Ok(())
    }

    fn parse_blocks(blocks: &JsonValue) -> Vec<ParsedBlock> {
        let JsonValue::Array(items) = blocks else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|block| {
                let JsonValue::Object(map) = block else {
                    return None;
                };
                let day_code = map
                    .get("day")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_uppercase();
                let day_index = match day_code.as_str() {
                    "MO" => 0,
                    "TU" => 1,
                    "WE" => 2,
                    "TH" => 3,
                    "FR" => 4,
                    "SA" => 5,
                    "SU" => 6,
                    _ => return None,
                };
                let start_raw = map.get("start").and_then(|v| v.as_str())?;
                let end_raw = map.get("end").and_then(|v| v.as_str())?;
                let (start_hour, start_minute) = Self::parse_hhmm(start_raw)?;
                let (end_hour, end_minute) = Self::parse_hhmm(end_raw)?;
                Some(ParsedBlock {
                    day_index,
                    start_hour,
                    start_minute,
                    end_hour,
                    end_minute,
                })
            })
            .collect()
    }

    fn parse_hhmm(value: &str) -> Option<(u32, u32)> {
        let trimmed = value.trim();
        let (hour_raw, minute_raw) = trimmed.split_once(':')?;
        let hour: u32 = hour_raw.trim().parse().ok()?;
        let minute: u32 = minute_raw.trim().parse().ok()?;
        if hour > 23 || minute > 59 {
            return None;
        }
        Some((hour, minute))
    }

    fn derive_block_end_actions(actions: &JsonValue) -> JsonValue {
        let action_list: Vec<JsonValue> = match actions {
            JsonValue::Array(items) => items.clone(),
            JsonValue::Object(_) => vec![actions.clone()],
            _ => Vec::new(),
        };

        let mut end_actions: Vec<JsonValue> = Vec::new();
        for action in action_list {
            let JsonValue::Object(map) = &action else {
                continue;
            };
            let action_type = map.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if action_type != "output" {
                continue;
            }
            let output_id = map
                .get("output_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if output_id.is_empty() {
                continue;
            }
            let state = map
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let Some(end_state) = Self::default_end_state(state) else {
                continue;
            };
            end_actions.push(serde_json::json!({
                "type": "output",
                "output_id": output_id,
                "state": end_state,
                "duration_seconds": null,
            }));
        }

        JsonValue::Array(end_actions)
    }

    fn default_end_state(state: &str) -> Option<&'static str> {
        let normalized = state.trim().to_lowercase();
        match normalized.as_str() {
            "on" | "off" => Some("off"),
            "open" | "close" => Some("close"),
            "start" | "stop" => Some("stop"),
            _ => None,
        }
    }

    fn is_due(&self, rrule_text: &str, window_start: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        let parsed: Result<RRuleSet, _> = rrule_text.parse();
        let Ok(rule) = parsed else {
            return false;
        };

        let window_start_tz = window_start.with_timezone(&rrule::Tz::UTC);
        let now_tz = now.with_timezone(&rrule::Tz::UTC);
        !rule
            .after(window_start_tz)
            .before(now_tz)
            .all(1)
            .dates
            .is_empty()
    }

    async fn conditions_met(&self, raw: &JsonValue) -> Result<bool> {
        let JsonValue::Array(items) = raw else {
            return Ok(true);
        };
        for cond in items {
            if !self.evaluate_condition(cond).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn evaluate_condition(&self, cond: &JsonValue) -> Result<bool> {
        let JsonValue::Object(map) = cond else {
            return Ok(false);
        };
        let cond_type = map.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if cond_type == "forecast" {
            let field = map.get("field").and_then(|v| v.as_str()).unwrap_or("");
            let horizon_hours = map
                .get("horizon_hours")
                .and_then(|v| v.as_i64())
                .unwrap_or(24);
            let operator = map.get("operator").and_then(|v| v.as_str()).unwrap_or("");
            let threshold = map.get("threshold").and_then(|v| v.as_f64());
            let value = self.latest_forecast_value(field, horizon_hours).await?;
            return Ok(compare(value, operator, threshold));
        }
        Ok(false)
    }

    async fn latest_forecast_value(&self, field: &str, horizon_hours: i64) -> Result<Option<f64>> {
        if field.trim().is_empty() {
            return Ok(None);
        }
        let row: Option<(f64,)> = sqlx::query_as(
            r#"
            SELECT value
            FROM forecast_data
            WHERE field = $1 AND horizon_hours = $2
            ORDER BY recorded_at DESC
            LIMIT 1
            "#,
        )
        .bind(field)
        .bind(horizon_hours as i32)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(value,)| value))
    }

    async fn execute_actions(&self, schedule_id: i64, actions: &JsonValue) -> Result<()> {
        let action_list: Vec<JsonValue> = match actions {
            JsonValue::Array(items) => items.clone(),
            JsonValue::Object(_) => vec![actions.clone()],
            _ => Vec::new(),
        };
        for action in action_list {
            if let Err(err) = self.execute_action(schedule_id, &action).await {
                tracing::warn!(error = %err, schedule_id, "schedule action failed");
                self.log_action(schedule_id, &action, "failed", Some(&err.to_string()))
                    .await?;
            } else {
                self.log_action(schedule_id, &action, "success", None)
                    .await?;
            }
        }
        Ok(())
    }

    async fn execute_action(&self, schedule_id: i64, action: &JsonValue) -> Result<()> {
        let JsonValue::Object(map) = action else {
            anyhow::bail!("action must be an object");
        };
        let action_type = map.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if action_type == "alarm" {
            let message = map
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Schedule triggered")
                .to_string();
            let severity = map
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("warning");
            let name = map
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Schedule Alarm");
            self.raise_schedule_alarm(schedule_id, name, severity, &message)
                .await?;
            return Ok(());
        }
        if action_type == "mqtt_publish" {
            let topic = map
                .get("topic")
                .and_then(|v| v.as_str())
                .unwrap_or("iot/broadcast/outputs/command");
            let payload = map
                .get("payload")
                .cloned()
                .unwrap_or(JsonValue::Object(Default::default()));
            self.mqtt.publish_json(topic, &payload).await?;
            return Ok(());
        }
        // Output actions are handled by the schedule creator (core-server) publishing to MQTT.
        if action_type == "output" {
            let output_id = map.get("output_id").and_then(|v| v.as_str()).unwrap_or("");
            let state = map.get("state").and_then(|v| v.as_str()).unwrap_or("");
            if output_id.is_empty() || state.is_empty() {
                anyhow::bail!("output action missing output_id/state");
            }
            let payload =
                serde_json::json!({ "state": state, "reason": format!("schedule:{schedule_id}") });
            let topic = format!("iot/broadcast/outputs/{output_id}");
            self.mqtt.publish_json(&topic, &payload).await?;
            return Ok(());
        }

        anyhow::bail!("unsupported action type {action_type}");
    }

    async fn raise_schedule_alarm(
        &self,
        schedule_id: i64,
        alarm_name: &str,
        _severity: &str,
        message: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now();
        let mut tx = self.pool.begin().await?;
        let alarm_row: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT id FROM alarms
            WHERE name = $1 AND origin = 'schedule'
            LIMIT 1
            "#,
        )
        .bind(alarm_name)
        .fetch_optional(&mut *tx)
        .await?;

        let alarm_id = if let Some((alarm_id,)) = alarm_row {
            sqlx::query(
                r#"
                UPDATE alarms SET status = 'firing', last_fired = NOW(), rule = $2, origin = 'schedule'
                WHERE id = $1
                "#,
            )
            .bind(alarm_id)
            .bind(serde_json::json!({"type":"schedule","severity":_severity,"schedule_id":schedule_id}))
            .execute(&mut *tx)
            .await?;
            alarm_id
        } else {
            let inserted: (i64,) = sqlx::query_as(
                r#"
                INSERT INTO alarms (name, rule, status, origin, last_fired)
                VALUES ($1, $2, 'firing', 'schedule', NOW())
                RETURNING id
                "#,
            )
            .bind(alarm_name)
            .bind(serde_json::json!({"type":"schedule","severity":_severity,"schedule_id":schedule_id}))
            .fetch_one(&mut *tx)
            .await?;
            inserted.0
        };

        sqlx::query(
            r#"
            INSERT INTO alarm_events (
                alarm_id,
                status,
                message,
                origin,
                transition,
                incident_id,
                target_key
            )
            VALUES ($1, 'firing', $2, 'schedule', 'fired', $3, $4)
            "#,
        )
        .bind(alarm_id)
        .bind(message)
        .bind(
            crate::services::incidents::get_or_create_incident(
                &mut tx,
                now,
                &crate::services::incidents::IncidentKey {
                    rule_id: None,
                    target_key: Some(format!("schedule:{schedule_id}")),
                },
                _severity,
                alarm_name,
                "fired",
            )
            .await?,
        )
        .bind(format!("schedule:{schedule_id}"))
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn log_action(
        &self,
        schedule_id: i64,
        action: &JsonValue,
        status: &str,
        message: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO action_logs (schedule_id, action, status, message)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(schedule_id)
        .bind(action)
        .bind(status)
        .bind(message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn compare(value: Option<f64>, operator: &str, threshold: Option<f64>) -> bool {
    let Some(value) = value else {
        return false;
    };
    let Some(threshold) = threshold else {
        return false;
    };
    match operator {
        "<" => value < threshold,
        "<=" => value <= threshold,
        ">" => value > threshold,
        ">=" => value >= threshold,
        "==" => (value - threshold).abs() < f64::EPSILON,
        "!=" => (value - threshold).abs() >= f64::EPSILON,
        _ => false,
    }
}
