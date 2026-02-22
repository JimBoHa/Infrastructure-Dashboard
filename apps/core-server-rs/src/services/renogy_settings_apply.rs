use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::state::AppState;

const DEVICE_TYPE: &str = "renogy_bt2";

#[derive(sqlx::FromRow)]
struct PendingRow {
    node_id: Uuid,
    desired: SqlJson<JsonValue>,
    last_applied: Option<SqlJson<JsonValue>>,
    apply_attempts: i32,
    last_apply_attempt_at: Option<chrono::DateTime<chrono::Utc>>,
    status: String,
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

fn parse_hex_address(value: &str) -> Option<u16> {
    let trimmed = value.trim();
    if let Some(without_prefix) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u16::from_str_radix(without_prefix, 16).ok();
    }
    trimmed.parse::<u16>().ok()
}

#[derive(Debug, Clone)]
struct RegisterField {
    key: String,
    address: u16,
    count: u16,
    scale: f64,
}

fn extract_register_fields(schema: &JsonValue) -> anyhow::Result<Vec<RegisterField>> {
    let fields = schema
        .get("fields")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("schema missing fields[]"))?;

    let mut out = Vec::new();
    for field in fields {
        let key = field
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("register field missing key"))?;
        let addr_str = field
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("register field missing address for {key}"))?;
        let address = parse_hex_address(addr_str)
            .ok_or_else(|| anyhow::anyhow!("invalid address {addr_str}"))?;
        let count = field.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
        if count == 0 || count > 64 {
            return Err(anyhow::anyhow!("invalid register count for {key}: {count}"));
        }
        let scale = field.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
        out.push(RegisterField {
            key: key.to_string(),
            address,
            count: count as u16,
            scale,
        });
    }
    Ok(out)
}

fn build_diff(old: &JsonValue, new: &JsonValue) -> JsonValue {
    let old_map = old.as_object();
    let new_map = new.as_object();

    let mut keys = std::collections::BTreeSet::new();
    if let Some(map) = old_map {
        keys.extend(map.keys().cloned());
    }
    if let Some(map) = new_map {
        keys.extend(map.keys().cloned());
    }

    let mut diff = serde_json::Map::new();
    for key in keys {
        let old_val = old_map.and_then(|m| m.get(&key));
        let new_val = new_map.and_then(|m| m.get(&key));
        if old_val == new_val {
            continue;
        }
        let mut entry = serde_json::Map::new();
        entry.insert(
            "from".to_string(),
            old_val.cloned().unwrap_or(JsonValue::Null),
        );
        entry.insert(
            "to".to_string(),
            new_val.cloned().unwrap_or(JsonValue::Null),
        );
        diff.insert(key, JsonValue::Object(entry));
    }

    JsonValue::Object(diff)
}

fn build_node_agent_writes(
    desired: &JsonValue,
    schema: &JsonValue,
) -> anyhow::Result<Vec<JsonValue>> {
    let desired_obj = desired
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("desired must be a JSON object"))?;

    let fields = schema
        .get("fields")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("schema missing fields[]"))?;

    let mut fields_by_key = std::collections::HashMap::<String, JsonValue>::new();
    for field in fields {
        if let Some(key) = field.get("key").and_then(|v| v.as_str()) {
            fields_by_key.insert(key.to_string(), field.clone());
        }
    }

    let mut writes = Vec::new();
    for (key, value) in desired_obj {
        let field = match fields_by_key.get(key) {
            Some(field) => field,
            None => continue,
        };
        let writable = field
            .get("writable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !writable {
            continue;
        }
        let addr_str = field
            .get("address")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing address for {key}"))?;
        let address = parse_hex_address(addr_str)
            .ok_or_else(|| anyhow::anyhow!("invalid address for {key}"))?;
        let scale = field.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let raw = value
            .as_f64()
            .ok_or_else(|| anyhow::anyhow!("setting value must be numeric: {key}"))?;
        let unscaled = (raw / scale).round();
        let int_val = unscaled as i64;
        if int_val < 0 || int_val > i64::from(u16::MAX) {
            return Err(anyhow::anyhow!("setting out of range for u16: {key}"));
        }
        let description = field
            .get("label")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
        writes.push(serde_json::json!({
            "address": address,
            "values": [int_val as u16],
            "description": description,
        }));
    }

    Ok(writes)
}

fn parse_register_map() -> anyhow::Result<JsonValue> {
    let raw = include_str!("../../../../shared/renogy/register_maps/rng_ctrl_rvr20_us_bt2.json");
    serde_json::from_str::<JsonValue>(raw)
        .map_err(|err| anyhow::anyhow!("Failed to parse register-map JSON: {err}"))
}

pub struct RenogySettingsApplyService {
    state: AppState,
    interval: Duration,
}

impl RenogySettingsApplyService {
    pub fn new(state: AppState, interval: Duration) -> Self {
        Self { state, interval }
    }

    pub fn start(self, cancel: CancellationToken) {
        let state = self.state.clone();
        let interval = self.interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        if let Err(err) = poll_and_apply(&state).await {
                            tracing::warn!("renogy apply worker failed: {err:#}");
                        }
                    }
                }
            }
        });
    }
}

async fn poll_and_apply(state: &AppState) -> anyhow::Result<()> {
    let rows: Vec<PendingRow> = sqlx::query_as(
        r#"
        SELECT
            ds.node_id,
            ds.desired,
            ds.last_applied,
            ds.apply_attempts,
            ds.last_apply_attempt_at,
            n.status
        FROM device_settings ds
        JOIN nodes n ON n.id = ds.node_id
        WHERE ds.device_type = $1
          AND COALESCE(ds.apply_requested, false) = true
          AND COALESCE(ds.maintenance_mode, false) = false
        ORDER BY COALESCE(ds.apply_requested_at, ds.desired_updated_at) ASC
        LIMIT 10
        "#,
    )
    .bind(DEVICE_TYPE)
    .fetch_all(&state.db)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    for row in rows {
        if row.status != "online" {
            continue;
        }
        if row.apply_attempts >= 50 {
            continue;
        }
        if let Some(last_attempt) = row.last_apply_attempt_at {
            let age = chrono::Utc::now().signed_duration_since(last_attempt);
            if age.num_seconds() < 20 {
                continue;
            }
        }

        let mut endpoint =
            crate::services::node_agent_resolver::resolve_node_agent_endpoint(
                &state.db,
                row.node_id,
                state.config.node_agent_port,
                false,
            )
            .await
            .ok()
            .flatten();
        if endpoint.is_none() {
            continue;
        }

        let lock_key = advisory_lock_key("renogy_settings", &row.node_id.to_string());
        let mut tx = state.db.begin().await?;
        let _ = sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;

        let schema = parse_register_map()?;
        let writes = build_node_agent_writes(&row.desired.0, &schema)?;
        if writes.is_empty() {
            let _ = sqlx::query(
                r#"
                UPDATE device_settings
                SET apply_requested = false,
                    apply_requested_at = NULL,
                    apply_requested_by = NULL,
                    last_apply_attempt_at = now()
                WHERE node_id = $1 AND device_type = $2
                "#,
            )
            .bind(row.node_id)
            .bind(DEVICE_TYPE)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            continue;
        }

        let url = format!(
            "{}/v1/renogy-bt/settings/apply",
            endpoint.as_ref().unwrap().base_url
        );
        let request_body = serde_json::json!({ "writes": writes, "verify": true });
        let mut response = state
            .http
            .post(url)
            .timeout(Duration::from_secs(30))
            .json(&request_body)
            .send()
            .await;
        if response.is_err() {
            if let Ok(Some(refreshed)) =
                crate::services::node_agent_resolver::resolve_node_agent_endpoint(
                    &state.db,
                    row.node_id,
                    state.config.node_agent_port,
                    true,
                )
                .await
            {
                endpoint = Some(refreshed);
                let url = format!(
                    "{}/v1/renogy-bt/settings/apply",
                    endpoint.as_ref().unwrap().base_url
                );
                response = state
                    .http
                    .post(url)
                    .timeout(Duration::from_secs(30))
                    .json(&request_body)
                    .send()
                    .await;
            }
        }
        if response.is_err() {
            if let Some(ip_fallback) = endpoint.as_ref().and_then(|e| e.ip_fallback.as_deref()) {
                let url = format!("{ip_fallback}/v1/renogy-bt/settings/apply");
                response = state
                    .http
                    .post(url)
                    .timeout(Duration::from_secs(30))
                    .json(&request_body)
                    .send()
                    .await;
            }
        }

        let (apply_status, result) = match response {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    (
                        "error".to_string(),
                        serde_json::json!({"message": format!("Node-agent error ({status}): {body}")}),
                    )
                } else {
                    let json: JsonValue = serde_json::from_str(&body).unwrap_or_else(
                        |_| serde_json::json!({"status":"error","message":"invalid json"}),
                    );
                    let st = json
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("error")
                        .to_string();
                    (st, json)
                }
            }
            Err(err) => (
                "error".to_string(),
                serde_json::json!({"message": format!("Node-agent request failed: {err}")}),
            ),
        };

        let fields = extract_register_fields(&schema)?;
        let mut current_map = serde_json::Map::new();
        if apply_status == "ok" {
            if let Some(applied) = result.get("applied").and_then(|v| v.as_array()) {
                for item in applied {
                    let address = item
                        .get("address")
                        .and_then(|v| v.as_u64())
                        .and_then(|v| u16::try_from(v).ok());
                    let read_back = item.get("read_back").and_then(|v| v.as_array());
                    let Some(address) = address else { continue };
                    let Some(read_back) = read_back else { continue };
                    let read_val = read_back
                        .first()
                        .and_then(|v| v.as_u64())
                        .and_then(|v| u16::try_from(v).ok());
                    let Some(read_val) = read_val else { continue };

                    for field in &fields {
                        if field.count == 1 && field.address == address {
                            let scaled = (read_val as f64) * field.scale;
                            if (field.scale - 1.0).abs() < f64::EPSILON {
                                current_map
                                    .insert(field.key.clone(), JsonValue::from(read_val as i64));
                            } else {
                                current_map.insert(field.key.clone(), JsonValue::from(scaled));
                            }
                        }
                    }
                }
            }
        }

        let prior_applied = row
            .last_applied
            .clone()
            .map(|v| v.0)
            .unwrap_or_else(|| JsonValue::Object(serde_json::Map::new()));
        let diff = build_diff(&prior_applied, &row.desired.0);

        let _ = sqlx::query(
            r#"
            UPDATE device_settings
            SET pending = CASE WHEN $3 = 'ok' THEN false ELSE pending END,
                last_applied = CASE WHEN $3 = 'ok' THEN desired ELSE last_applied END,
                last_applied_at = CASE WHEN $3 = 'ok' THEN now() ELSE last_applied_at END,
                last_apply_status = $3,
                last_apply_result = $4,
                apply_requested = CASE WHEN $3 = 'ok' THEN false ELSE apply_requested END,
                apply_requested_at = CASE WHEN $3 = 'ok' THEN NULL ELSE apply_requested_at END,
                apply_requested_by = CASE WHEN $3 = 'ok' THEN NULL ELSE apply_requested_by END,
                last_apply_attempt_at = now(),
                apply_attempts = CASE WHEN $3 = 'ok' THEN 0 ELSE apply_attempts + 1 END
            WHERE node_id = $1 AND device_type = $2
            "#,
        )
        .bind(row.node_id)
        .bind(DEVICE_TYPE)
        .bind(&apply_status)
        .bind(&result)
        .execute(&mut *tx)
        .await?;

        let _ = sqlx::query(
            r#"
            INSERT INTO device_settings_events (node_id, device_type, event_type, desired, current, diff, result)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(row.node_id)
        .bind(DEVICE_TYPE)
        .bind(if apply_status == "ok" { "apply_auto" } else { "apply_auto_failed" })
        .bind(&row.desired.0)
        .bind(JsonValue::Object(current_map))
        .bind(diff)
        .bind(&result)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
    }

    Ok(())
}
