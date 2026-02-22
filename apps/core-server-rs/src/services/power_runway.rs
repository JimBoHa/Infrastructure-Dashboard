use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Timelike, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::services::battery_model::BatteryModelConfig;
use crate::services::virtual_sensors;
use crate::state::AppState;

const RENOGY_SOURCE: &str = "renogy_bt2";
const METRIC_BATTERY_VOLTAGE_V: &str = "battery_voltage_v";
const UNIT_V: &str = "V";
const UNIT_W: &str = "W";

const FORECAST_PROVIDER: &str = "forecast_solar";
const FORECAST_KIND_PV: &str = "pv";
const FORECAST_SUBJECT_KIND_NODE: &str = "node";
const FORECAST_METRIC_PV_POWER_W: &str = "pv_power_w";

const MAX_SAMPLE_STALENESS_SECONDS: i64 = 5 * 60;

pub const SENSOR_SOURCE_POWER_RUNWAY: &str = "power_runway";
const SENSOR_TYPE_POWER_RUNWAY: &str = "power_runway";

const OUT_METRIC_RUNWAY_HOURS: &str = "power_runway_hours_conservative";
const OUT_METRIC_MIN_SOC_PROJECTED: &str = "power_runway_min_soc_projected_percent";

const UNIT_HOURS: &str = "hr";
const UNIT_PERCENT: &str = "%";

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PowerRunwayConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub load_sensor_ids: Vec<String>,
    #[serde(default = "default_history_days")]
    pub history_days: i64,
    #[serde(default = "default_pv_derate")]
    pub pv_derate: f64,
    #[serde(default = "default_projection_days")]
    pub projection_days: i64,
}

fn default_history_days() -> i64 {
    7
}

fn default_pv_derate() -> f64 {
    0.75
}

fn default_projection_days() -> i64 {
    5
}

impl Default for PowerRunwayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            load_sensor_ids: Vec::new(),
            history_days: default_history_days(),
            pv_derate: default_pv_derate(),
            projection_days: default_projection_days(),
        }
    }
}

pub struct PowerRunwayService {
    state: AppState,
    interval: Duration,
}

impl PowerRunwayService {
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
                        if let Err(err) = tick_once(&state).await {
                            tracing::warn!("power runway tick failed: {err:#}");
                        }
                    }
                }
            }
        });
    }
}

#[derive(sqlx::FromRow)]
struct NodeConfigRow {
    id: Uuid,
    config: SqlJson<JsonValue>,
}

#[derive(Debug, Clone, FromRow)]
struct BatteryEstimatorStateRow {
    soc_est_percent: f64,
}

#[derive(Debug, Clone)]
struct LatestSample {
    ts: DateTime<Utc>,
    value: f64,
}

#[derive(sqlx::FromRow)]
struct LoadHourRow {
    hour: i32,
    total_w: f64,
}

#[derive(sqlx::FromRow)]
struct ForecastPointRow {
    ts: DateTime<Utc>,
    value: f64,
}

async fn tick_once(state: &AppState) -> Result<()> {
    let nodes: Vec<NodeConfigRow> = sqlx::query_as(
        r#"
        SELECT
          id,
          COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE status <> 'deleted'
          AND NOT (COALESCE(config, '{}'::jsonb) @> '{"deleted": true}')
          AND COALESCE(config, '{}'::jsonb) @> '{"power_runway": {"enabled": true}}'
        ORDER BY id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .context("failed to query power_runway-enabled nodes")?;

    if nodes.is_empty() {
        return Ok(());
    }

    for node in nodes {
        if let Err(err) = process_node(state, node.id, &node.config.0).await {
            tracing::warn!(node_id = %node.id, "power runway node processing failed: {err:#}");
        }
    }

    Ok(())
}

async fn process_node(state: &AppState, node_id: Uuid, node_config: &JsonValue) -> Result<()> {
    let cfg_value = node_config.get("power_runway").cloned().unwrap_or(JsonValue::Null);
    let cfg: PowerRunwayConfig = serde_json::from_value(cfg_value).unwrap_or_default();
    if !cfg.enabled {
        return Ok(());
    }

    let bm_value = node_config.get("battery_model").cloned().unwrap_or(JsonValue::Null);
    let bm_cfg: BatteryModelConfig = serde_json::from_value(bm_value).unwrap_or_default();
    if !bm_cfg.enabled {
        return Ok(());
    }

    let sticker_capacity_ah = resolve_sticker_capacity_ah(&state.db, node_id, &bm_cfg).await?;
    let Some(sticker_capacity_ah) = sticker_capacity_ah else {
        return Ok(());
    };
    if !sticker_capacity_ah.is_finite() || sticker_capacity_ah <= 0.0 {
        return Ok(());
    }

    let estimator = load_estimator_state(&state.db, node_id).await?;
    let Some(estimator) = estimator else {
        return Ok(());
    };
    let soc_est_percent = estimator.soc_est_percent.clamp(0.0, 100.0);
    let cutoff_percent = bm_cfg.soc_cutoff_percent.clamp(0.0, 100.0);

    let now = Utc::now();

    let voltage_sensor_id =
        sensor_id_for_node_metric(&state.db, node_id, METRIC_BATTERY_VOLTAGE_V, Some(UNIT_V))
            .await?;
    let Some(voltage_sensor_id) = voltage_sensor_id else {
        return Ok(());
    };
    let voltage = latest_sample(&state.db, &voltage_sensor_id).await?;
    let Some(voltage) = voltage else {
        return Ok(());
    };
    if (now - voltage.ts).num_seconds() > MAX_SAMPLE_STALENESS_SECONDS {
        return Ok(());
    }
    if voltage.value <= 0.0 || !voltage.value.is_finite() {
        return Ok(());
    }

    let mut load_sensor_ids: Vec<String> = cfg
        .load_sensor_ids
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    load_sensor_ids.sort();
    load_sensor_ids.dedup();
    if load_sensor_ids.is_empty() {
        return Ok(());
    }

    // Validate load sensors are watts to avoid mixing units.
    if !validate_load_sensors_watts(&state.db, &load_sensor_ids).await? {
        return Ok(());
    }

    let history_days = cfg.history_days.clamp(1, 30);
    let pv_derate = cfg.pv_derate.clamp(0.0, 1.0);
    let projection_days = cfg.projection_days.clamp(1, 14);
    let projection_hours = projection_days.saturating_mul(24);

    let load_profile_w = load_hour_of_day_profile_w(&state.db, &load_sensor_ids, now, history_days).await?;
    let pv_forecast_w = load_pv_forecast_w(&state.db, node_id, now, projection_hours).await?;

    let result = simulate_runway_conservative(
        now,
        soc_est_percent,
        voltage.value,
        sticker_capacity_ah,
        cutoff_percent,
        pv_derate,
        projection_hours,
        &load_profile_w,
        &pv_forecast_w,
    );

    upsert_outputs(
        &state.db,
        node_id,
        now,
        result.runway_hours,
        result.min_soc_projected_percent,
    )
    .await?;

    Ok(())
}

async fn validate_load_sensors_watts(db: &PgPool, sensor_ids: &[String]) -> Result<bool> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM sensors
        WHERE sensor_id = ANY($1)
          AND deleted_at IS NULL
          AND unit = $2
        "#,
    )
    .bind(sensor_ids)
    .bind(UNIT_W)
    .fetch_one(db)
    .await
    .context("failed to validate load sensors units")?;
    Ok(count == sensor_ids.len() as i64)
}

async fn resolve_sticker_capacity_ah(db: &PgPool, node_id: Uuid, cfg: &BatteryModelConfig) -> Result<Option<f64>> {
    // Prefer explicit battery_model capacity.
    if let Some(value) = cfg.sticker_capacity_ah {
        if value.is_finite() && value > 0.0 {
            return Ok(Some(value));
        }
    }

    // Fallback: Renogy desired settings if present.
    let raw: Option<String> = sqlx::query_scalar(
        r#"
        SELECT desired->>'battery_capacity_ah'
        FROM device_settings
        WHERE node_id = $1 AND device_type = $2
        "#,
    )
    .bind(node_id)
    .bind("renogy_bt2")
    .fetch_optional(db)
    .await
    .context("failed to query renogy battery_capacity_ah from device_settings")?;

    let Some(raw) = raw else {
        return Ok(None);
    };
    let parsed = raw.trim().parse::<f64>().ok();
    Ok(parsed.filter(|v| v.is_finite() && *v > 0.0))
}

async fn load_estimator_state(db: &PgPool, node_id: Uuid) -> Result<Option<BatteryEstimatorStateRow>> {
    let row: Option<BatteryEstimatorStateRow> = sqlx::query_as(
        r#"
        SELECT soc_est_percent
        FROM battery_estimator_state
        WHERE node_id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(db)
    .await
    .context("failed to load battery_estimator_state for runway")?;
    Ok(row)
}

async fn sensor_id_for_node_metric(
    db: &PgPool,
    node_id: Uuid,
    metric: &str,
    expected_unit: Option<&str>,
) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE deleted_at IS NULL
          AND node_id = $1
          AND COALESCE(config->>'source', '') = $2
          AND COALESCE(config->>'metric', '') = $3
          AND ($4::text IS NULL OR unit = $4)
        ORDER BY created_at ASC
        LIMIT 1
        "#,
    )
    .bind(node_id)
    .bind(RENOGY_SOURCE)
    .bind(metric)
    .bind(expected_unit)
    .fetch_optional(db)
    .await
    .with_context(|| format!("failed to resolve {metric} sensor for node {node_id}"))?;

    Ok(row.map(|(sensor_id,)| sensor_id))
}

async fn latest_sample(db: &PgPool, sensor_id: &str) -> Result<Option<LatestSample>> {
    let row: Option<(DateTime<Utc>, f64)> = sqlx::query_as(
        r#"
        SELECT ts, value
        FROM metrics
        WHERE sensor_id = $1
        ORDER BY ts DESC
        LIMIT 1
        "#,
    )
    .bind(sensor_id)
    .fetch_optional(db)
    .await
    .with_context(|| format!("failed to load latest metric for sensor {sensor_id}"))?;

    Ok(row.map(|(ts, value)| LatestSample { ts, value }))
}

async fn load_hour_of_day_profile_w(
    db: &PgPool,
    sensor_ids: &[String],
    now: DateTime<Utc>,
    history_days: i64,
) -> Result<[f64; 24]> {
    let since = now - ChronoDuration::days(history_days);
    let rows: Vec<LoadHourRow> = sqlx::query_as(
        r#"
        SELECT hour, SUM(avg_w) as total_w
        FROM (
          SELECT
            sensor_id,
            EXTRACT(HOUR FROM (ts AT TIME ZONE 'UTC'))::int as hour,
            AVG(value) as avg_w
          FROM metrics
          WHERE sensor_id = ANY($1)
            AND ts >= $2
          GROUP BY sensor_id, hour
        ) t
        GROUP BY hour
        ORDER BY hour ASC
        "#,
    )
    .bind(sensor_ids)
    .bind(since)
    .fetch_all(db)
    .await
    .context("failed to build load hour-of-day profile")?;

    let overall_mean: Option<f64> = sqlx::query_scalar(
        r#"
        SELECT AVG(value)
        FROM metrics
        WHERE sensor_id = ANY($1)
          AND ts >= $2
        "#,
    )
    .bind(sensor_ids)
    .bind(since)
    .fetch_one(db)
    .await
    .context("failed to query overall mean load")?;

    let fallback = overall_mean.unwrap_or(0.0).max(0.0);

    let mut out = [fallback; 24];
    for row in rows {
        if row.hour >= 0 && row.hour <= 23 && row.total_w.is_finite() {
            out[row.hour as usize] = row.total_w.max(0.0);
        }
    }

    Ok(out)
}

async fn load_pv_forecast_w(
    db: &PgPool,
    node_id: Uuid,
    now: DateTime<Utc>,
    projection_hours: i64,
) -> Result<HashMap<DateTime<Utc>, f64>> {
    let subject = node_id.to_string();
    let issued_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        r#"
        SELECT issued_at
        FROM forecast_points
        WHERE provider = $1
          AND kind = $2
          AND subject_kind = $3
          AND subject = $4
        ORDER BY issued_at DESC
        LIMIT 1
        "#,
    )
    .bind(FORECAST_PROVIDER)
    .bind(FORECAST_KIND_PV)
    .bind(FORECAST_SUBJECT_KIND_NODE)
    .bind(&subject)
    .fetch_optional(db)
    .await
    .context("failed to load PV forecast issued_at")?;

    let Some(issued_at) = issued_at else {
        return Ok(HashMap::new());
    };

    let start = truncate_to_hour(now);
    let end = start + ChronoDuration::hours(projection_hours);

    let rows: Vec<ForecastPointRow> = sqlx::query_as(
        r#"
        SELECT ts, value
        FROM forecast_points
        WHERE provider = $1
          AND kind = $2
          AND subject_kind = $3
          AND subject = $4
          AND issued_at = $5
          AND metric = $6
          AND ts >= $7
          AND ts <= $8
        ORDER BY ts ASC
        "#,
    )
    .bind(FORECAST_PROVIDER)
    .bind(FORECAST_KIND_PV)
    .bind(FORECAST_SUBJECT_KIND_NODE)
    .bind(&subject)
    .bind(issued_at)
    .bind(FORECAST_METRIC_PV_POWER_W)
    .bind(start)
    .bind(end)
    .fetch_all(db)
    .await
    .context("failed to query PV forecast points")?;

    let mut out = HashMap::new();
    for row in rows {
        if row.value.is_finite() {
            out.insert(row.ts, row.value.max(0.0));
        }
    }
    Ok(out)
}

fn truncate_to_hour(dt: DateTime<Utc>) -> DateTime<Utc> {
    dt.with_minute(0)
        .and_then(|dt| dt.with_second(0))
        .and_then(|dt| dt.with_nanosecond(0))
        .unwrap_or(dt)
}

#[derive(Debug, Clone)]
struct RunwayResult {
    runway_hours: f64,
    min_soc_projected_percent: f64,
}

fn simulate_runway_conservative(
    now: DateTime<Utc>,
    soc_est_percent: f64,
    battery_voltage_v: f64,
    sticker_capacity_ah: f64,
    cutoff_percent: f64,
    pv_derate: f64,
    projection_hours: i64,
    load_profile_w: &[f64; 24],
    pv_forecast_w: &HashMap<DateTime<Utc>, f64>,
) -> RunwayResult {
    let soc_est_percent = soc_est_percent.clamp(0.0, 100.0);
    let cutoff_percent = cutoff_percent.clamp(0.0, 100.0);

    if battery_voltage_v <= 0.0 || sticker_capacity_ah <= 0.0 {
        return RunwayResult {
            runway_hours: 0.0,
            min_soc_projected_percent: soc_est_percent,
        };
    }

    let capacity_wh = sticker_capacity_ah * battery_voltage_v;
    let mut energy_wh = capacity_wh * (soc_est_percent / 100.0);
    let cutoff_wh = capacity_wh * (cutoff_percent / 100.0);

    if energy_wh <= cutoff_wh + 1e-9 {
        return RunwayResult {
            runway_hours: 0.0,
            min_soc_projected_percent: soc_est_percent.min(cutoff_percent),
        };
    }

    let start_hour = truncate_to_hour(now);
    let offset_seconds = (now - start_hour).num_seconds().clamp(0, 3600) as f64;
    let first_dt_hours = (1.0 - (offset_seconds / 3600.0)).clamp(0.0, 1.0);

    let mut elapsed_hours = 0.0;
    let mut min_soc = soc_est_percent;

    for i in 0..projection_hours {
        let bucket_start = start_hour + ChronoDuration::hours(i);
        let dt_hours = if i == 0 { first_dt_hours } else { 1.0 };
        if dt_hours <= 0.0 {
            continue;
        }

        let hour_idx = bucket_start.hour().clamp(0, 23) as usize;
        let load_w = load_profile_w[hour_idx].max(0.0);
        let pv_w = pv_forecast_w.get(&bucket_start).copied().unwrap_or(0.0).max(0.0);
        let net_w = (pv_w * pv_derate) - load_w;

        let prev_energy = energy_wh;
        energy_wh = (energy_wh + net_w * dt_hours).clamp(0.0, capacity_wh);
        let soc_now = (energy_wh / capacity_wh) * 100.0;
        if soc_now.is_finite() {
            min_soc = min_soc.min(soc_now);
        }

        if energy_wh <= cutoff_wh + 1e-9 {
            if prev_energy > cutoff_wh + 1e-9 && (prev_energy - energy_wh).abs() > f64::EPSILON {
                let frac = ((prev_energy - cutoff_wh) / (prev_energy - energy_wh)).clamp(0.0, 1.0);
                return RunwayResult {
                    runway_hours: elapsed_hours + dt_hours * frac,
                    min_soc_projected_percent: min_soc,
                };
            }
            return RunwayResult {
                runway_hours: elapsed_hours,
                min_soc_projected_percent: min_soc,
            };
        }

        elapsed_hours += dt_hours;
    }

    RunwayResult {
        runway_hours: elapsed_hours,
        min_soc_projected_percent: min_soc,
    }
}

async fn upsert_outputs(db: &PgPool, node_id: Uuid, ts: DateTime<Utc>, runway_hours: f64, min_soc: f64) -> Result<()> {
    let runway_sensor_id = virtual_sensors::ensure_read_only_virtual_sensor(
        db,
        node_id,
        SENSOR_SOURCE_POWER_RUNWAY,
        &format!("{node_id}|{OUT_METRIC_RUNWAY_HOURS}"),
        "Power runway (conservative, hr)",
        SENSOR_TYPE_POWER_RUNWAY,
        UNIT_HOURS,
        600,
        json!({ "metric": OUT_METRIC_RUNWAY_HOURS }),
    )
    .await?;

    let min_soc_sensor_id = virtual_sensors::ensure_read_only_virtual_sensor(
        db,
        node_id,
        SENSOR_SOURCE_POWER_RUNWAY,
        &format!("{node_id}|{OUT_METRIC_MIN_SOC_PROJECTED}"),
        "Power runway min SOC projected (%)",
        SENSOR_TYPE_POWER_RUNWAY,
        UNIT_PERCENT,
        600,
        json!({ "metric": OUT_METRIC_MIN_SOC_PROJECTED }),
    )
    .await?;

    upsert_metric(db, ts, &runway_sensor_id, runway_hours.max(0.0)).await?;
    upsert_metric(db, ts, &min_soc_sensor_id, min_soc.clamp(0.0, 100.0)).await?;
    Ok(())
}

async fn upsert_metric(db: &PgPool, ts: DateTime<Utc>, sensor_id: &str, value: f64) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at)
        SELECT $1, $2, $3, 0, now()
        WHERE EXISTS (
            SELECT 1
            FROM sensors
            WHERE sensor_id = $1
              AND deleted_at IS NULL
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
    .execute(db)
    .await
    .with_context(|| format!("failed to upsert metric for {sensor_id}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runway_simulation_handles_zero_pv() {
        let now = Utc::now();
        let load_profile = [100.0; 24];
        let pv = HashMap::new();

        // 100Ah @ 12V => 1200Wh. Start SOC 50% => 600Wh. cutoff 20% => 240Wh.
        // usable = 360Wh. load=100W => 3.6h (with some rounding because of fractional first hour).
        let result = simulate_runway_conservative(
            now,
            50.0,
            12.0,
            100.0,
            20.0,
            0.75,
            24,
            &load_profile,
            &pv,
        );
        assert!(result.runway_hours > 3.0);
        assert!(result.runway_hours < 4.2);
        assert!(result.min_soc_projected_percent <= 50.0);
    }

    #[test]
    fn runway_simulation_never_drops_below_cutoff_when_net_positive() {
        let now = Utc::now();
        let load_profile = [100.0; 24];
        let mut pv = HashMap::new();
        let start_hour = truncate_to_hour(now);
        for i in 0..120 {
            pv.insert(start_hour + ChronoDuration::hours(i), 500.0);
        }

        let result = simulate_runway_conservative(
            now,
            50.0,
            12.0,
            100.0,
            20.0,
            1.0,
            120,
            &load_profile,
            &pv,
        );
        assert!(result.runway_hours > 100.0);
        assert!(result.min_soc_projected_percent >= 20.0);
    }
}
