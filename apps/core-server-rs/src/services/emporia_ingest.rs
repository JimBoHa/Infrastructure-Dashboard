use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::ids;
use crate::services::emporia::{EmporiaChannelReading, EmporiaDeviceInfo, EmporiaUsageAggregate};
use crate::services::emporia_preferences::{
    compute_emporia_device_summary_power_w, EmporiaCircuitPreferences, EmporiaDevicePreferences,
    EMPORIA_CIRCUIT_KEY_MAINS,
};
use crate::state::AppState;

const EMPORIA_EXTERNAL_PROVIDER: &str = "emporia";
const SENSOR_TYPE_POWER: &str = "power";
const UNIT_WATTS: &str = "W";
const SENSOR_TYPE_VOLTAGE: &str = "voltage";
const UNIT_VOLTS: &str = "V";
const SENSOR_TYPE_CURRENT: &str = "current";
const UNIT_AMPS: &str = "A";
const EMPORIA_METRIC_SUMMARY_POWER_W: &str = "power_summary_w";
const EMPORIA_MAINS_LEG_A: &str = "mains_a";
const EMPORIA_MAINS_LEG_B: &str = "mains_b";

pub async fn persist_emporia_usage(
    state: &AppState,
    devices: &[EmporiaDeviceInfo],
    usage: &EmporiaUsageAggregate,
    device_prefs: &std::collections::HashMap<String, EmporiaDevicePreferences>,
) -> Result<()> {
    let poll_interval_seconds = state
        .config
        .analytics_feed_poll_interval_seconds
        .min(u64::MAX / 2);
    let timestamp = usage.timestamp;
    for device in &usage.devices {
        let info = devices
            .iter()
            .find(|entry| entry.device_gid == device.device_gid);
        let prefs = device_prefs.get(&device.device_gid);
        let node_id =
            upsert_emporia_node(state, info, prefs, &device.device_gid, timestamp).await?;

        let device_hidden = prefs.map(|entry| entry.hidden).unwrap_or(false);
        let default_mains = EmporiaCircuitPreferences {
            enabled: true,
            hidden: false,
            include_in_power_summary: true,
        };
        let mains_prefs = prefs
            .and_then(|entry| entry.circuits.get(EMPORIA_CIRCUIT_KEY_MAINS))
            .unwrap_or(&default_mains);
        let mains_enabled = mains_prefs.enabled;
        let mains_hidden = device_hidden || mains_prefs.hidden;

        // Per-node summary sensor used by `/api/analytics/power` (one sensor per node).
        let summary_power_w = compute_emporia_device_summary_power_w(device, prefs);
        upsert_sensor(
            state,
            node_id,
            &device.device_gid,
            "summary_power_w",
            "Summary Power",
            SENSOR_TYPE_POWER,
            UNIT_WATTS,
            &json!({
                "metric": EMPORIA_METRIC_SUMMARY_POWER_W,
                "source": "emporia_cloud",
                "external_provider": EMPORIA_EXTERNAL_PROVIDER,
                "external_id": device.device_gid,
                "channel_key": "summary",
                "poll_enabled": true,
                "hidden": true,
            }),
            poll_interval_seconds,
        )
        .await?;

        insert_metric(
            state,
            timestamp,
            sensor_id_for_device(&device.device_gid, "summary_power_w"),
            summary_power_w,
        )
        .await?;

        if mains_enabled {
            upsert_sensor(
                state,
                node_id,
                &device.device_gid,
                "mains_power_w",
                "Mains Power",
                SENSOR_TYPE_POWER,
                UNIT_WATTS,
                &json!({
                    "metric": "mains_power_w",
                    "source": "emporia_cloud",
                    "external_provider": EMPORIA_EXTERNAL_PROVIDER,
                    "external_id": device.device_gid,
                    "channel_key": EMPORIA_CIRCUIT_KEY_MAINS,
                    "poll_enabled": true,
                    "hidden": mains_hidden,
                    "include_in_power_summary": mains_prefs.include_in_power_summary,
                }),
                poll_interval_seconds,
            )
            .await?;

            insert_metric(
                state,
                timestamp,
                sensor_id_for_device(&device.device_gid, "mains_power_w"),
                device.main_power_w,
            )
            .await?;
        }

        if mains_enabled {
            // Legacy sensors: earlier builds persisted an aggregated mains voltage/current sensor
            // and also stored per-leg/panel voltage/current as channel sensors (e.g. Mains_A/B).
            // This creates confusing duplicates since the Emporia meter has only two distinct
            // mains voltages (L1-N, L2-N). Prefer the per-leg channel sensors and hide the legacy
            // aggregate sensors so they no longer appear in the dashboard.
            disable_sensor(
                state,
                sensor_id_for_device(&device.device_gid, "mains_voltage_v"),
            )
            .await?;
            disable_sensor(
                state,
                sensor_id_for_device(&device.device_gid, "mains_current_a"),
            )
            .await?;
        }

        for channel in &device.channels {
            let circuit_key = channel.channel_num.trim();
            if circuit_key.is_empty() {
                continue;
            }

            let default = EmporiaCircuitPreferences {
                enabled: true,
                hidden: false,
                include_in_power_summary: false,
            };
            let circuit_prefs = prefs
                .and_then(|entry| entry.circuits.get(circuit_key))
                .unwrap_or(&default);

            if !circuit_prefs.enabled {
                continue;
            }

            let circuit_hidden = device_hidden || circuit_prefs.hidden;
            persist_channel(
                state,
                node_id,
                &device.device_gid,
                channel,
                circuit_hidden,
                circuit_prefs.include_in_power_summary,
                poll_interval_seconds,
                timestamp,
            )
            .await?;
        }
    }

    mark_stale_emporia_nodes_offline(state, Utc::now(), poll_interval_seconds).await?;
    Ok(())
}

async fn persist_channel(
    state: &AppState,
    node_id: Uuid,
    device_gid: &str,
    channel: &EmporiaChannelReading,
    hidden: bool,
    include_in_power_summary: bool,
    poll_interval_seconds: u64,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let channel_num = channel.channel_num.trim();
    if channel_num.is_empty() {
        return Ok(());
    }

    let raw_channel_num = channel.raw_channel_num.trim();
    let mut sensor_name_base = channel
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            if let Some(nested_gid) = channel.nested_device_gid.as_deref() {
                format!("Device {nested_gid} · Channel {raw_channel_num}")
            } else {
                format!("Channel {raw_channel_num}")
            }
        });
    if channel.is_mains {
        let normalized = raw_channel_num.trim().to_lowercase();
        if normalized == EMPORIA_MAINS_LEG_A {
            sensor_name_base = "Mains L1".to_string();
        } else if normalized == EMPORIA_MAINS_LEG_B {
            sensor_name_base = "Mains L2".to_string();
        }
    }

    let base_config = json!({
        "source": "emporia_cloud",
        "external_provider": EMPORIA_EXTERNAL_PROVIDER,
        "external_id": device_gid,
        "channel_key": channel_num,
        "channel_num": raw_channel_num,
        "nested_device_gid": channel.nested_device_gid.clone(),
        "channel_name": channel.name.clone(),
        "is_mains": channel.is_mains,
        "percentage": channel.percentage,
        "poll_enabled": true,
        "hidden": hidden,
        "include_in_power_summary": include_in_power_summary,
    });

    let (power_w, power_is_derived) = match (channel.power_w, channel.voltage_v, channel.current_a)
    {
        (Some(power_w), _, _) => (Some(power_w), false),
        (None, Some(voltage_v), Some(current_a)) => (Some(voltage_v * current_a), true),
        (None, _, _) => (None, false),
    };

    if let Some(power_w) = power_w {
        let channel_key = format!("channel_power_w:{channel_num}");
        let mut config = base_config.clone();
        if let Some(obj) = config.as_object_mut() {
            obj.insert(
                "metric".to_string(),
                JsonValue::String("channel_power_w".to_string()),
            );
            if power_is_derived {
                obj.insert("derived_from_va".to_string(), JsonValue::Bool(true));
            }
        }
        upsert_sensor(
            state,
            node_id,
            device_gid,
            &channel_key,
            &sensor_name_base,
            SENSOR_TYPE_POWER,
            UNIT_WATTS,
            &config,
            poll_interval_seconds,
        )
        .await?;

        insert_metric(
            state,
            timestamp,
            sensor_id_for_device(device_gid, &channel_key),
            power_w,
        )
        .await?;
    }

    let voltage_sensor_key = format!("channel_voltage_v:{channel_num}");
    if channel.is_mains && is_mains_leg_channel(raw_channel_num, channel.name.as_deref()) {
        if let Some(voltage_v) = channel.voltage_v {
            let mut config = base_config.clone();
            if let Some(obj) = config.as_object_mut() {
                obj.insert(
                    "metric".to_string(),
                    JsonValue::String("channel_voltage_v".to_string()),
                );
            }
            upsert_sensor(
                state,
                node_id,
                device_gid,
                &voltage_sensor_key,
                &format!("{sensor_name_base} Voltage"),
                SENSOR_TYPE_VOLTAGE,
                UNIT_VOLTS,
                &config,
                poll_interval_seconds,
            )
            .await?;

            insert_metric(
                state,
                timestamp,
                sensor_id_for_device(device_gid, &voltage_sensor_key),
                voltage_v,
            )
            .await?;
        }
    } else {
        // Emporia reports only mains leg voltages (L1-N/L2-N); per-circuit voltages are
        // effectively duplicates of those values and create significant UI clutter.
        // Hide any legacy per-circuit voltage sensors.
        disable_sensor(state, sensor_id_for_device(device_gid, &voltage_sensor_key)).await?;
    }

    if let Some(current_a) = channel.current_a {
        let channel_key = format!("channel_current_a:{channel_num}");
        let mut config = base_config.clone();
        if let Some(obj) = config.as_object_mut() {
            obj.insert(
                "metric".to_string(),
                JsonValue::String("channel_current_a".to_string()),
            );
        }
        upsert_sensor(
            state,
            node_id,
            device_gid,
            &channel_key,
            &format!("{sensor_name_base} Current"),
            SENSOR_TYPE_CURRENT,
            UNIT_AMPS,
            &config,
            poll_interval_seconds,
        )
        .await?;

        insert_metric(
            state,
            timestamp,
            sensor_id_for_device(device_gid, &channel_key),
            current_a,
        )
        .await?;
    }

    Ok(())
}

fn is_mains_leg_channel(raw_channel_num: &str, name: Option<&str>) -> bool {
    let normalized = raw_channel_num.trim().to_lowercase();
    if normalized == EMPORIA_MAINS_LEG_A || normalized == EMPORIA_MAINS_LEG_B {
        return true;
    }
    let Some(name) = name else {
        return false;
    };
    let name = name.trim().to_lowercase();
    name == EMPORIA_MAINS_LEG_A || name == EMPORIA_MAINS_LEG_B
}

async fn disable_sensor(state: &AppState, sensor_id: String) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE sensors
        SET config = COALESCE(config, '{}'::jsonb) || '{"hidden": true, "poll_enabled": false}'::jsonb,
            deleted_at = NULL
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id)
    .execute(&state.db)
    .await
    .context("failed to disable legacy Emporia sensor")?;
    Ok(())
}

async fn upsert_emporia_node(
    state: &AppState,
    info: Option<&EmporiaDeviceInfo>,
    prefs: Option<&EmporiaDevicePreferences>,
    device_gid: &str,
    last_seen: DateTime<Utc>,
) -> Result<Uuid> {
    let name = info
        .and_then(|entry| entry.name.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("Emporia {device_gid}"));

    let config = json!({
        "node_kind": "power",
        "external_provider": EMPORIA_EXTERNAL_PROVIDER,
        "external_id": device_gid,
        "power_provider": "emporia_cloud",
        "poll_enabled": prefs.map(|entry| entry.enabled).unwrap_or(true),
        "hidden": prefs.map(|entry| entry.hidden).unwrap_or(false),
        "group_label": prefs.and_then(|entry| entry.group_label.clone()),
        "include_in_power_summary": prefs.map(|entry| entry.include_in_power_summary).unwrap_or(true),
        "address": info.and_then(|entry| entry.address.clone()),
        "model": info.and_then(|entry| entry.model.clone()),
        "firmware": info.and_then(|entry| entry.firmware.clone()),
    });

    let row: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO nodes (
            name,
            status,
            uptime_seconds,
            cpu_percent,
            storage_used_bytes,
            last_seen,
            config,
            external_provider,
            external_id
        )
        VALUES ($1, 'online', 0, 0, 0, $2, $3, $4, $5)
        ON CONFLICT (external_provider, external_id)
        DO UPDATE SET
            name = EXCLUDED.name,
            status = EXCLUDED.status,
            last_seen = EXCLUDED.last_seen,
            config = EXCLUDED.config
        RETURNING id
        "#,
    )
    .bind(&name)
    .bind(last_seen)
    .bind(SqlJson(config))
    .bind(EMPORIA_EXTERNAL_PROVIDER)
    .bind(device_gid)
    .fetch_one(&state.db)
    .await
    .context("failed to upsert Emporia node")?;

    Ok(row.0)
}

async fn upsert_sensor(
    state: &AppState,
    node_id: Uuid,
    device_gid: &str,
    sensor_key: &str,
    name: &str,
    sensor_type: &str,
    unit: &str,
    config: &JsonValue,
    poll_interval_seconds: u64,
) -> Result<()> {
    let sensor_id = sensor_id_for_device(device_gid, sensor_key);
    let interval_seconds = (poll_interval_seconds as i32).max(1);
    sqlx::query(
        r#"
        INSERT INTO sensors (
            sensor_id,
            node_id,
            name,
            type,
            unit,
            interval_seconds,
            rolling_avg_seconds,
            config,
            deleted_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, 0, $7, NULL)
        ON CONFLICT (sensor_id)
        DO UPDATE SET
            node_id = EXCLUDED.node_id,
            name = EXCLUDED.name,
            type = EXCLUDED.type,
            unit = EXCLUDED.unit,
            interval_seconds = EXCLUDED.interval_seconds,
            config = EXCLUDED.config
        WHERE sensors.deleted_at IS NULL
          AND COALESCE(sensors.config->>'poll_enabled', 'true') <> 'false'
        "#,
    )
    .bind(&sensor_id)
    .bind(node_id)
    .bind(name)
    .bind(sensor_type)
    .bind(unit)
    .bind(interval_seconds)
    .bind(SqlJson(config.clone()))
    .execute(&state.db)
    .await
    .with_context(|| format!("failed to upsert Emporia sensor {sensor_id}"))?;
    Ok(())
}

async fn insert_metric(
    state: &AppState,
    ts: DateTime<Utc>,
    sensor_id: String,
    value: f64,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at)
        SELECT $1, $2, $3, 0, now()
        WHERE EXISTS (
            SELECT 1
            FROM sensors
            WHERE sensor_id = $1
              AND deleted_at IS NULL
              AND COALESCE(config->>'poll_enabled', 'true') <> 'false'
        )
        ON CONFLICT (sensor_id, ts)
        DO UPDATE SET
            value = EXCLUDED.value,
            inserted_at = EXCLUDED.inserted_at
        "#,
    )
    .bind(sensor_id)
    .bind(ts)
    .bind(value)
    .execute(&state.db)
    .await
    .context("failed to insert Emporia metric")?;
    Ok(())
}

async fn mark_stale_emporia_nodes_offline(
    state: &AppState,
    now: DateTime<Utc>,
    poll_interval_seconds: u64,
) -> Result<()> {
    let grace_seconds = (poll_interval_seconds.saturating_mul(3)).max(900);
    let cutoff = now - chrono::Duration::seconds(grace_seconds as i64);
    sqlx::query(
        r#"
        UPDATE nodes
        SET status = 'offline'
        WHERE external_provider = $1
          AND status <> 'deleted'
          AND last_seen IS NOT NULL
          AND last_seen < $2
          AND status <> 'offline'
          AND NOT (COALESCE(config, '{}'::jsonb) @> '{"deleted": true}')
          AND NOT (COALESCE(config, '{}'::jsonb) @> '{"poll_enabled": false}')
        "#,
    )
    .bind(EMPORIA_EXTERNAL_PROVIDER)
    .bind(cutoff)
    .execute(&state.db)
    .await
    .context("failed to mark stale Emporia nodes offline")?;
    Ok(())
}

fn sensor_id_for_device(device_gid: &str, sensor_key: &str) -> String {
    ids::stable_hex_id(
        "sensor:emporia",
        &format!("device={device_gid}|{sensor_key}"),
    )
}
