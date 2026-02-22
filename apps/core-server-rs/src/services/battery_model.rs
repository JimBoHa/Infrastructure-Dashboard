use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use sqlx::{FromRow, PgPool};
use std::time::Duration as StdDuration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::services::virtual_sensors;
use crate::state::AppState;

const RENOGY_SOURCE: &str = "renogy_bt2";
const RENOGY_DEVICE_TYPE: &str = "renogy_bt2";

const METRIC_BATTERY_CURRENT_A: &str = "battery_current_a";
const METRIC_BATTERY_VOLTAGE_V: &str = "battery_voltage_v";
const METRIC_BATTERY_SOC_PERCENT: &str = "battery_soc_percent";

pub const SENSOR_SOURCE_BATTERY_MODEL: &str = "battery_model";
const SENSOR_TYPE_BATTERY_MODEL: &str = "battery_model";

const OUT_METRIC_SOC_EST_PERCENT: &str = "battery_soc_est_percent";
const OUT_METRIC_REMAINING_AH: &str = "battery_remaining_ah";
const OUT_METRIC_CAPACITY_EST_AH: &str = "battery_capacity_est_ah";

const UNIT_PERCENT: &str = "%";
const UNIT_A: &str = "A";
const UNIT_V: &str = "V";
const UNIT_AH: &str = "Ah";

const MAX_SAMPLE_STALENESS_SECONDS: i64 = 5 * 60;
const MAX_INTEGRATION_GAP_SECONDS: i64 = 15 * 60;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BatteryChemistry {
    Lifepo4,
    LeadAcid,
}

impl Default for BatteryChemistry {
    fn default() -> Self {
        Self::Lifepo4
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurrentSignMode {
    Auto,
    PositiveIsCharging,
    PositiveIsDischarging,
}

impl Default for CurrentSignMode {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SocAnchorMode {
    Disabled,
    BlendToRenogyWhenResting,
}

impl Default for SocAnchorMode {
    fn default() -> Self {
        Self::BlendToRenogyWhenResting
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CapacityEstimationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_min_soc_span_percent")]
    pub min_soc_span_percent: f64,
    #[serde(default = "default_capacity_ema_alpha")]
    pub ema_alpha: f64,
    #[serde(default = "default_capacity_clamp_min_ah")]
    pub clamp_min_ah: f64,
    #[serde(default = "default_capacity_clamp_max_ah")]
    pub clamp_max_ah: f64,
}

fn default_true() -> bool {
    true
}

fn default_min_soc_span_percent() -> f64 {
    30.0
}

fn default_capacity_ema_alpha() -> f64 {
    0.1
}

fn default_capacity_clamp_min_ah() -> f64 {
    1.0
}

fn default_capacity_clamp_max_ah() -> f64 {
    2000.0
}

impl Default for CapacityEstimationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_soc_span_percent: default_min_soc_span_percent(),
            ema_alpha: default_capacity_ema_alpha(),
            clamp_min_ah: default_capacity_clamp_min_ah(),
            clamp_max_ah: default_capacity_clamp_max_ah(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BatteryModelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub chemistry: BatteryChemistry,
    #[serde(default)]
    pub current_sign: CurrentSignMode,
    #[serde(default)]
    pub sticker_capacity_ah: Option<f64>,
    #[serde(default = "default_soc_cutoff_percent")]
    pub soc_cutoff_percent: f64,
    #[serde(default = "default_rest_current_abs_a")]
    pub rest_current_abs_a: f64,
    #[serde(default = "default_rest_minutes_required")]
    pub rest_minutes_required: i64,
    #[serde(default)]
    pub soc_anchor_mode: SocAnchorMode,
    #[serde(default = "default_soc_anchor_max_step_percent")]
    pub soc_anchor_max_step_percent: f64,
    #[serde(default)]
    pub capacity_estimation: CapacityEstimationConfig,
}

fn default_soc_cutoff_percent() -> f64 {
    20.0
}

fn default_rest_current_abs_a() -> f64 {
    2.0
}

fn default_rest_minutes_required() -> i64 {
    10
}

fn default_soc_anchor_max_step_percent() -> f64 {
    1.0
}

impl Default for BatteryModelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            chemistry: BatteryChemistry::default(),
            current_sign: CurrentSignMode::default(),
            sticker_capacity_ah: None,
            soc_cutoff_percent: default_soc_cutoff_percent(),
            rest_current_abs_a: default_rest_current_abs_a(),
            rest_minutes_required: default_rest_minutes_required(),
            soc_anchor_mode: SocAnchorMode::default(),
            soc_anchor_max_step_percent: default_soc_anchor_max_step_percent(),
            capacity_estimation: CapacityEstimationConfig::default(),
        }
    }
}

pub struct BatteryEstimatorService {
    state: AppState,
    interval: StdDuration,
}

impl BatteryEstimatorService {
    pub fn new(state: AppState, interval: StdDuration) -> Self {
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
                            tracing::warn!("battery estimator tick failed: {err:#}");
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
    current_sign: i16,
    sign_locked: bool,
    sign_votes_pos: i32,
    sign_votes_neg: i32,
    soc_est_percent: f64,
    capacity_est_ah: f64,
    last_anchor_soc_percent: Option<f64>,
    segment_ah_accumulated: f64,
    rest_started_at: Option<DateTime<Utc>>,
    last_ts: DateTime<Utc>,
    last_current_a: Option<f64>,
    last_voltage_v: Option<f64>,
    last_renogy_soc_percent: Option<f64>,
}

#[derive(Debug, Clone)]
struct LatestSample {
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
          AND COALESCE(config, '{}'::jsonb) @> '{"battery_model": {"enabled": true}}'
        ORDER BY id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .context("failed to query battery_model-enabled nodes")?;

    if nodes.is_empty() {
        return Ok(());
    }

    for node in nodes {
        if let Err(err) = process_node(state, node.id, &node.config.0).await {
            tracing::warn!(node_id = %node.id, "battery estimator node processing failed: {err:#}");
        }
    }

    Ok(())
}

async fn process_node(state: &AppState, node_id: Uuid, node_config: &JsonValue) -> Result<()> {
    let cfg_value = node_config.get("battery_model").cloned().unwrap_or(JsonValue::Null);
    let cfg: BatteryModelConfig = serde_json::from_value(cfg_value).unwrap_or_default();
    if !cfg.enabled {
        return Ok(());
    }

    let sticker_capacity_ah = resolve_sticker_capacity_ah(&state.db, node_id, &cfg).await?;
    let Some(sticker_capacity_ah) = sticker_capacity_ah else {
        tracing::info!(node_id = %node_id, "battery_model enabled but sticker capacity is missing; skipping estimator");
        return Ok(());
    };
    if !sticker_capacity_ah.is_finite() || sticker_capacity_ah <= 0.0 {
        tracing::info!(node_id = %node_id, sticker_capacity_ah, "battery_model sticker capacity invalid; skipping estimator");
        return Ok(());
    }

    let current_sensor_id =
        sensor_id_for_node_metric(&state.db, node_id, METRIC_BATTERY_CURRENT_A, Some(UNIT_A))
            .await?;
    let voltage_sensor_id =
        sensor_id_for_node_metric(&state.db, node_id, METRIC_BATTERY_VOLTAGE_V, Some(UNIT_V))
            .await?;
    let renogy_soc_sensor_id = sensor_id_for_node_metric(
        &state.db,
        node_id,
        METRIC_BATTERY_SOC_PERCENT,
        Some(UNIT_PERCENT),
    )
    .await?;

    let Some(current_sensor_id) = current_sensor_id else {
        return Ok(());
    };
    let Some(voltage_sensor_id) = voltage_sensor_id else {
        return Ok(());
    };

    let now = Utc::now();
    let current = latest_sample(&state.db, &current_sensor_id).await?;
    if is_stale(now, current.as_ref()) {
        return Ok(());
    }
    let voltage = latest_sample(&state.db, &voltage_sensor_id).await?;
    let renogy_soc = if let Some(sensor_id) = renogy_soc_sensor_id.as_deref() {
        latest_sample(&state.db, sensor_id).await?
    } else {
        None
    };

    let Some(current) = current else {
        return Ok(());
    };

    let sample_ts = current.ts;
    let state_row = load_state(&state.db, node_id).await?;

    let mut st = state_row.unwrap_or_else(|| BatteryEstimatorStateRow {
        current_sign: 1,
        sign_locked: false,
        sign_votes_pos: 0,
        sign_votes_neg: 0,
        soc_est_percent: renogy_soc
            .as_ref()
            .map(|v| v.value)
            .unwrap_or(0.0)
            .clamp(0.0, 100.0),
        capacity_est_ah: sticker_capacity_ah,
        last_anchor_soc_percent: renogy_soc.as_ref().map(|v| v.value),
        segment_ah_accumulated: 0.0,
        rest_started_at: None,
        last_ts: sample_ts,
        last_current_a: Some(current.value),
        last_voltage_v: voltage.as_ref().map(|v| v.value),
        last_renogy_soc_percent: renogy_soc.as_ref().map(|v| v.value),
    });

    apply_config_current_sign(&cfg, &mut st);
    maybe_vote_current_sign(&cfg, &current, &renogy_soc, &mut st);

    let dt_seconds = (sample_ts - st.last_ts).num_seconds();
    if dt_seconds > 0 {
        if dt_seconds > MAX_INTEGRATION_GAP_SECONDS {
            // Large gaps are treated as estimator reset points to avoid huge integration jumps.
            if let Some(renogy_soc) = renogy_soc.as_ref().map(|v| v.value) {
                st.soc_est_percent = renogy_soc.clamp(0.0, 100.0);
                st.last_anchor_soc_percent = Some(renogy_soc);
            }
            st.segment_ah_accumulated = 0.0;
        } else {
            let dt_hours = (dt_seconds as f64) / 3600.0;
            let current_sign = st.current_sign as f64;
            let capacity_for_soc = if st.capacity_est_ah > 0.0 {
                st.capacity_est_ah
            } else {
                sticker_capacity_ah
            };
            if capacity_for_soc > 0.0 {
                let d_ah = current_sign * current.value * dt_hours;
                st.segment_ah_accumulated += d_ah;
                st.soc_est_percent = clamp_pct(st.soc_est_percent + (d_ah / capacity_for_soc) * 100.0);
            }
        }
    }

    // Update “resting” detector and apply SOC anchoring and capacity estimation.
    update_resting_state(&cfg, &current, &mut st);
    if is_resting(&cfg, sample_ts, &st) {
        if let Some(renogy_soc) = renogy_soc.as_ref().map(|v| v.value) {
            st.soc_est_percent = apply_soc_anchor(&cfg, st.soc_est_percent, renogy_soc);
            maybe_update_capacity_estimate(&cfg, renogy_soc, &mut st);
        }
    }

    st.last_ts = sample_ts;
    st.last_current_a = Some(current.value);
    st.last_voltage_v = voltage.as_ref().map(|v| v.value);
    st.last_renogy_soc_percent = renogy_soc.as_ref().map(|v| v.value);

    save_state(&state.db, node_id, &st).await?;

    let remaining_ah = sticker_capacity_ah * (st.soc_est_percent / 100.0);
    upsert_outputs(&state.db, node_id, sample_ts, st.soc_est_percent, remaining_ah, st.capacity_est_ah).await?;

    Ok(())
}

fn is_stale(now: DateTime<Utc>, sample: Option<&LatestSample>) -> bool {
    let Some(sample) = sample else {
        return true;
    };
    (now - sample.ts).num_seconds() > MAX_SAMPLE_STALENESS_SECONDS
}

fn clamp_pct(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 100.0)
}

fn apply_soc_anchor(cfg: &BatteryModelConfig, soc_est: f64, renogy_soc: f64) -> f64 {
    if cfg.soc_anchor_mode == SocAnchorMode::Disabled {
        return soc_est;
    }
    let max_step = cfg.soc_anchor_max_step_percent.max(0.0);
    if max_step <= f64::EPSILON {
        return soc_est;
    }
    let delta = renogy_soc - soc_est;
    let step = (delta * 0.1).clamp(-max_step, max_step);
    clamp_pct(soc_est + step)
}

fn maybe_update_capacity_estimate(cfg: &BatteryModelConfig, renogy_soc: f64, st: &mut BatteryEstimatorStateRow) {
    if !cfg.capacity_estimation.enabled {
        return;
    }
    let min_span = cfg.capacity_estimation.min_soc_span_percent.max(1.0);
    let Some(anchor) = st.last_anchor_soc_percent else {
        st.last_anchor_soc_percent = Some(renogy_soc);
        st.segment_ah_accumulated = 0.0;
        return;
    };

    let delta_soc = renogy_soc - anchor;
    if delta_soc.abs() < min_span {
        return;
    }
    let denom = delta_soc.abs() / 100.0;
    if denom <= f64::EPSILON {
        return;
    }
    let implied = st.segment_ah_accumulated.abs() / denom;
    if !implied.is_finite() {
        return;
    }
    let implied = implied
        .clamp(cfg.capacity_estimation.clamp_min_ah, cfg.capacity_estimation.clamp_max_ah);
    let alpha = cfg.capacity_estimation.ema_alpha.clamp(0.0, 1.0);
    st.capacity_est_ah = if st.capacity_est_ah > 0.0 {
        (1.0 - alpha) * st.capacity_est_ah + alpha * implied
    } else {
        implied
    };
    st.last_anchor_soc_percent = Some(renogy_soc);
    st.segment_ah_accumulated = 0.0;
}

fn apply_config_current_sign(cfg: &BatteryModelConfig, st: &mut BatteryEstimatorStateRow) {
    match cfg.current_sign {
        CurrentSignMode::Auto => {}
        CurrentSignMode::PositiveIsCharging => {
            st.current_sign = 1;
            st.sign_locked = true;
        }
        CurrentSignMode::PositiveIsDischarging => {
            st.current_sign = -1;
            st.sign_locked = true;
        }
    }
}

fn maybe_vote_current_sign(
    cfg: &BatteryModelConfig,
    current: &LatestSample,
    renogy_soc: &Option<LatestSample>,
    st: &mut BatteryEstimatorStateRow,
) {
    if cfg.current_sign != CurrentSignMode::Auto || st.sign_locked {
        return;
    }
    let Some(renogy_soc) = renogy_soc.as_ref() else {
        return;
    };
    let Some(prev_renogy_soc) = st.last_renogy_soc_percent else {
        return;
    };
    let soc_delta = renogy_soc.value - prev_renogy_soc;
    if soc_delta.abs() < 1.0 {
        return;
    }
    let i = current.value;
    if i.abs() < 0.05 {
        return;
    }

    let expected_sign = if soc_delta > 0.0 {
        if i > 0.0 { 1 } else { -1 }
    } else {
        if i > 0.0 { -1 } else { 1 }
    };

    if expected_sign > 0 {
        st.sign_votes_pos = st.sign_votes_pos.saturating_add(1);
    } else {
        st.sign_votes_neg = st.sign_votes_neg.saturating_add(1);
    }

    if st.sign_votes_pos >= 3 && st.sign_votes_pos >= st.sign_votes_neg.saturating_add(2) {
        st.current_sign = 1;
        st.sign_locked = true;
    } else if st.sign_votes_neg >= 3 && st.sign_votes_neg >= st.sign_votes_pos.saturating_add(2) {
        st.current_sign = -1;
        st.sign_locked = true;
    }
}

fn update_resting_state(cfg: &BatteryModelConfig, current: &LatestSample, st: &mut BatteryEstimatorStateRow) {
    let is_low = current.value.abs() <= cfg.rest_current_abs_a.max(0.0);
    if is_low {
        if st.rest_started_at.is_none() {
            st.rest_started_at = Some(current.ts);
        }
    } else {
        st.rest_started_at = None;
    }
}

fn is_resting(cfg: &BatteryModelConfig, now: DateTime<Utc>, st: &BatteryEstimatorStateRow) -> bool {
    let Some(started_at) = st.rest_started_at else {
        return false;
    };
    let required = cfg.rest_minutes_required.max(0) as i64;
    if required == 0 {
        return true;
    }
    (now - started_at).num_seconds() >= required.saturating_mul(60)
}

async fn resolve_sticker_capacity_ah(db: &PgPool, node_id: Uuid, cfg: &BatteryModelConfig) -> Result<Option<f64>> {
    if let Some(value) = cfg.sticker_capacity_ah {
        if value.is_finite() && value > 0.0 {
            return Ok(Some(value));
        }
    }

    // Fallback: read from Renogy desired settings if present.
    let raw: Option<String> = sqlx::query_scalar(
        r#"
        SELECT desired->>'battery_capacity_ah'
        FROM device_settings
        WHERE node_id = $1 AND device_type = $2
        "#,
    )
    .bind(node_id)
    .bind(RENOGY_DEVICE_TYPE)
    .fetch_optional(db)
    .await
    .context("failed to query renogy battery_capacity_ah from device_settings")?;

    let Some(raw) = raw else {
        return Ok(None);
    };
    let parsed = raw.trim().parse::<f64>().ok();
    Ok(parsed.filter(|v| v.is_finite() && *v > 0.0))
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

async fn load_state(db: &PgPool, node_id: Uuid) -> Result<Option<BatteryEstimatorStateRow>> {
    let row: Option<BatteryEstimatorStateRow> = sqlx::query_as(
        r#"
        SELECT
          current_sign,
          sign_locked,
          sign_votes_pos,
          sign_votes_neg,
          soc_est_percent,
          capacity_est_ah,
          last_anchor_soc_percent,
          segment_ah_accumulated,
          rest_started_at,
          last_ts,
          last_current_a,
          last_voltage_v,
          last_renogy_soc_percent
        FROM battery_estimator_state
        WHERE node_id = $1
        "#,
    )
    .bind(node_id)
    .fetch_optional(db)
    .await
    .context("failed to load battery_estimator_state")?;

    Ok(row)
}

async fn save_state(db: &PgPool, node_id: Uuid, st: &BatteryEstimatorStateRow) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO battery_estimator_state (
          node_id,
          current_sign,
          sign_locked,
          sign_votes_pos,
          sign_votes_neg,
          soc_est_percent,
          capacity_est_ah,
          last_anchor_soc_percent,
          segment_ah_accumulated,
          rest_started_at,
          last_ts,
          last_current_a,
          last_voltage_v,
          last_renogy_soc_percent,
          updated_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,now())
        ON CONFLICT (node_id) DO UPDATE
          SET current_sign = EXCLUDED.current_sign,
              sign_locked = EXCLUDED.sign_locked,
              sign_votes_pos = EXCLUDED.sign_votes_pos,
              sign_votes_neg = EXCLUDED.sign_votes_neg,
              soc_est_percent = EXCLUDED.soc_est_percent,
              capacity_est_ah = EXCLUDED.capacity_est_ah,
              last_anchor_soc_percent = EXCLUDED.last_anchor_soc_percent,
              segment_ah_accumulated = EXCLUDED.segment_ah_accumulated,
              rest_started_at = EXCLUDED.rest_started_at,
              last_ts = EXCLUDED.last_ts,
              last_current_a = EXCLUDED.last_current_a,
              last_voltage_v = EXCLUDED.last_voltage_v,
              last_renogy_soc_percent = EXCLUDED.last_renogy_soc_percent,
              updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(node_id)
    .bind(st.current_sign)
    .bind(st.sign_locked)
    .bind(st.sign_votes_pos)
    .bind(st.sign_votes_neg)
    .bind(st.soc_est_percent)
    .bind(st.capacity_est_ah)
    .bind(st.last_anchor_soc_percent)
    .bind(st.segment_ah_accumulated)
    .bind(st.rest_started_at)
    .bind(st.last_ts)
    .bind(st.last_current_a)
    .bind(st.last_voltage_v)
    .bind(st.last_renogy_soc_percent)
    .execute(db)
    .await
    .context("failed to upsert battery_estimator_state")?;

    Ok(())
}

async fn upsert_outputs(
    db: &PgPool,
    node_id: Uuid,
    ts: DateTime<Utc>,
    soc_est_percent: f64,
    remaining_ah: f64,
    capacity_est_ah: f64,
) -> Result<()> {
    let soc_sensor_id = virtual_sensors::ensure_read_only_virtual_sensor(
        db,
        node_id,
        SENSOR_SOURCE_BATTERY_MODEL,
        &format!("{node_id}|{OUT_METRIC_SOC_EST_PERCENT}"),
        "Battery SOC (est)",
        SENSOR_TYPE_BATTERY_MODEL,
        UNIT_PERCENT,
        30,
        json!({ "metric": OUT_METRIC_SOC_EST_PERCENT }),
    )
    .await?;

    let remaining_sensor_id = virtual_sensors::ensure_read_only_virtual_sensor(
        db,
        node_id,
        SENSOR_SOURCE_BATTERY_MODEL,
        &format!("{node_id}|{OUT_METRIC_REMAINING_AH}"),
        "Battery remaining (Ah)",
        SENSOR_TYPE_BATTERY_MODEL,
        UNIT_AH,
        30,
        json!({ "metric": OUT_METRIC_REMAINING_AH }),
    )
    .await?;

    let capacity_sensor_id = virtual_sensors::ensure_read_only_virtual_sensor(
        db,
        node_id,
        SENSOR_SOURCE_BATTERY_MODEL,
        &format!("{node_id}|{OUT_METRIC_CAPACITY_EST_AH}"),
        "Battery capacity (est, Ah)",
        SENSOR_TYPE_BATTERY_MODEL,
        UNIT_AH,
        300,
        json!({ "metric": OUT_METRIC_CAPACITY_EST_AH }),
    )
    .await?;

    upsert_metric(db, ts, &soc_sensor_id, soc_est_percent).await?;
    upsert_metric(db, ts, &remaining_sensor_id, remaining_ah).await?;
    if capacity_est_ah.is_finite() && capacity_est_ah > 0.0 {
        upsert_metric(db, ts, &capacity_sensor_id, capacity_est_ah).await?;
    }

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
    fn apply_soc_anchor_is_bounded() {
        let mut cfg = BatteryModelConfig::default();
        cfg.soc_anchor_mode = SocAnchorMode::BlendToRenogyWhenResting;
        cfg.soc_anchor_max_step_percent = 1.0;

        let anchored = apply_soc_anchor(&cfg, 50.0, 80.0);
        assert!((anchored - 51.0).abs() < 1e-9);

        let anchored2 = apply_soc_anchor(&cfg, 50.0, 0.0);
        assert!((anchored2 - 49.0).abs() < 1e-9);
    }

    #[test]
    fn maybe_update_capacity_estimate_updates_ema_and_resets_segment() {
        let mut cfg = BatteryModelConfig::default();
        cfg.capacity_estimation.enabled = true;
        cfg.capacity_estimation.min_soc_span_percent = 30.0;
        cfg.capacity_estimation.ema_alpha = 0.5;
        cfg.capacity_estimation.clamp_min_ah = 1.0;
        cfg.capacity_estimation.clamp_max_ah = 2000.0;

        let mut st = BatteryEstimatorStateRow {
            current_sign: 1,
            sign_locked: true,
            sign_votes_pos: 0,
            sign_votes_neg: 0,
            soc_est_percent: 0.0,
            capacity_est_ah: 100.0,
            last_anchor_soc_percent: Some(80.0),
            segment_ah_accumulated: 25.0,
            rest_started_at: None,
            last_ts: Utc::now(),
            last_current_a: None,
            last_voltage_v: None,
            last_renogy_soc_percent: None,
        };

        // delta_soc = 40 => implied capacity = 25 / 0.4 = 62.5
        maybe_update_capacity_estimate(&cfg, 40.0, &mut st);
        assert!((st.capacity_est_ah - 81.25).abs() < 1e-9);
        assert_eq!(st.last_anchor_soc_percent, Some(40.0));
        assert!((st.segment_ah_accumulated - 0.0).abs() < 1e-9);
    }
}
