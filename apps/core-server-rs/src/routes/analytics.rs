use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Datelike, TimeZone, Timelike, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use std::collections::HashMap;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::state::AppState;

const CAP_ANALYTICS_VIEW: &str = "analytics.view";

const RENOGY_SOURCE: &str = "renogy_bt2";
const RENOGY_METRIC_PV_POWER_W: &str = "pv_power_w";
const RENOGY_METRIC_LOAD_POWER_W: &str = "load_power_w";
const RENOGY_METRIC_BATTERY_VOLTAGE_V: &str = "battery_voltage_v";
const RENOGY_METRIC_BATTERY_CURRENT_A: &str = "battery_current_a";
const RENOGY_METRIC_BATTERY_SOC_PERCENT: &str = "battery_soc_percent";
const RENOGY_METRIC_RUNTIME_HOURS: &str = "runtime_hours";

const EMPORIA_SOURCE: &str = "emporia_cloud";
const EMPORIA_METRIC_MAINS_POWER_W: &str = "mains_power_w";
const EMPORIA_METRIC_SUMMARY_POWER_W: &str = "power_summary_w";

const UNIT_WATTS: &str = "W";
const UNIT_VOLTS: &str = "V";
const UNIT_AMPS: &str = "A";
const UNIT_PERCENT: &str = "%";
const UNIT_HOURS: &str = "hr";

const POWER_BUCKET_24H_SECONDS: i32 = 300;
const POWER_BUCKET_24H_SOLAR_SECONDS: i32 = 60;
const POWER_BUCKET_24H_BATTERY_SECONDS: i32 = 60;
const POWER_BUCKET_168H_SECONDS: i32 = 3600;

const SOIL_BUCKET_168H_SECONDS: i32 = 3600;
const SOIL_HISTORY_HOURS: i64 = 168;
const SOIL_SENSOR_TYPES: [&str; 2] = ["moisture", "percentage"];

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsIntegration {
    pub(crate) name: String,
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) meta: JsonValue,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsTimeSeriesPoint {
    pub(crate) timestamp: String,
    pub(crate) value: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsRateSchedule {
    pub(crate) provider: String,
    pub(crate) current_rate: f64,
    pub(crate) est_monthly_cost: f64,
    pub(crate) data_status: Option<String>,
    pub(crate) data_age_seconds: Option<i64>,
    pub(crate) recorded_at: Option<String>,
    pub(crate) details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) currency: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsPower {
    pub(crate) live_kw: f64,
    pub(crate) live_solar_kw: f64,
    pub(crate) live_grid_kw: f64,
    #[serde(default)]
    pub(crate) live_battery_kw: f64,
    pub(crate) kwh_24h: f64,
    pub(crate) kwh_168h: f64,
    #[serde(default)]
    pub(crate) solar_kwh_24h: f64,
    #[serde(default)]
    pub(crate) solar_kwh_168h: f64,
    #[serde(default)]
    pub(crate) grid_kwh_24h: f64,
    #[serde(default)]
    pub(crate) grid_kwh_168h: f64,
    #[serde(default)]
    pub(crate) battery_kwh_24h: f64,
    #[serde(default)]
    pub(crate) battery_kwh_168h: f64,
    pub(crate) series_24h: Vec<AnalyticsTimeSeriesPoint>,
    pub(crate) series_168h: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) solar_series_24h: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) solar_series_168h: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) grid_series_24h: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) grid_series_168h: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) battery_series_24h: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) battery_series_168h: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) integrations: Vec<AnalyticsIntegration>,
    pub(crate) rate_schedule: AnalyticsRateSchedule,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsSoilField {
    pub(crate) name: String,
    pub(crate) min: f64,
    pub(crate) max: f64,
    pub(crate) avg: f64,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsSoil {
    #[serde(default)]
    pub(crate) fields: Vec<AnalyticsSoilField>,
    pub(crate) series: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) series_avg: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) series_min: Vec<AnalyticsTimeSeriesPoint>,
    #[serde(default)]
    pub(crate) series_max: Vec<AnalyticsTimeSeriesPoint>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsFeedStatusEntry {
    pub(crate) status: Option<String>,
    pub(crate) last_seen: Option<String>,
    pub(crate) details: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsFeedHistoryEntry {
    pub(crate) category: String,
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) recorded_at: String,
    #[serde(default)]
    pub(crate) meta: JsonValue,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsFeedStatusResponse {
    pub(crate) enabled: bool,
    #[serde(default)]
    pub(crate) feeds: std::collections::HashMap<String, AnalyticsFeedStatusEntry>,
    #[serde(default)]
    pub(crate) history: Vec<AnalyticsFeedHistoryEntry>,
}

#[derive(sqlx::FromRow)]
struct IntegrationStatusRow {
    category: String,
    name: String,
    status: String,
    recorded_at: chrono::DateTime<chrono::Utc>,
    metadata: SqlJson<JsonValue>,
}

#[utoipa::path(
    get,
    path = "/api/analytics/feeds/status",
    tag = "analytics",
    responses(
        (status = 200, description = "Analytics feed status", body = AnalyticsFeedStatusResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn feeds_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<AnalyticsFeedStatusResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ANALYTICS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;
    let (feeds, history) = load_feed_status(&state).await?;
    Ok(Json(AnalyticsFeedStatusResponse {
        enabled: state.config.enable_analytics_feeds || state.config.enable_forecast_ingestion,
        feeds,
        history,
    }))
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsPollResponse {
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) feeds: std::collections::HashMap<String, String>,
}

#[utoipa::path(
    post,
    path = "/api/analytics/feeds/poll",
    tag = "analytics",
    responses(
        (status = 200, description = "Triggered feed poll", body = AnalyticsPollResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn feeds_poll(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<AnalyticsPollResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let mut feeds = std::collections::HashMap::new();
    let results = crate::services::analytics_feeds::poll_all_feeds(&state)
        .await
        .map_err(internal_error)?;
    for result in results {
        feeds.insert(result.name.clone(), result.status.clone());
    }
    Ok(Json(AnalyticsPollResponse {
        status: "ok".to_string(),
        feeds,
    }))
}

async fn load_feed_status(
    state: &AppState,
) -> Result<
    (
        std::collections::HashMap<String, AnalyticsFeedStatusEntry>,
        Vec<AnalyticsFeedHistoryEntry>,
    ),
    (StatusCode, String),
> {
    let latest: Vec<IntegrationStatusRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (category, name) category, name, status, recorded_at, metadata
        FROM analytics_integration_status
        ORDER BY category, name, recorded_at DESC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut feeds = std::collections::HashMap::new();
    for row in latest {
        let meta = row.metadata.0;
        let details = meta
            .get("detail")
            .or_else(|| meta.get("reason"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        feeds.insert(
            row.name.clone(),
            AnalyticsFeedStatusEntry {
                status: Some(row.status),
                last_seen: Some(row.recorded_at.to_rfc3339()),
                details,
            },
        );
    }

    let history_rows: Vec<IntegrationStatusRow> = sqlx::query_as(
        r#"
        SELECT category, name, status, recorded_at, metadata
        FROM analytics_integration_status
        ORDER BY recorded_at DESC
        LIMIT 50
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let history = history_rows
        .into_iter()
        .map(|row| AnalyticsFeedHistoryEntry {
            category: row.category,
            name: row.name,
            status: row.status,
            recorded_at: row.recorded_at.to_rfc3339(),
            meta: row.metadata.0,
        })
        .collect();

    Ok((feeds, history))
}

fn internal_error(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

fn empty_rate_schedule() -> AnalyticsRateSchedule {
    AnalyticsRateSchedule {
        provider: "".to_string(),
        current_rate: 0.0,
        est_monthly_cost: 0.0,
        data_status: None,
        data_age_seconds: None,
        recorded_at: None,
        details: None,
        currency: None,
    }
}

fn build_series(
    reference: chrono::DateTime<chrono::Utc>,
    total_hours: i64,
    step_hours: i64,
) -> Vec<AnalyticsTimeSeriesPoint> {
    let step_hours = step_hours.max(1);
    let mut points: Vec<AnalyticsTimeSeriesPoint> = Vec::new();
    let mut offset = total_hours.max(0);
    while offset >= 0 {
        let ts = reference - chrono::Duration::hours(offset);
        points.push(AnalyticsTimeSeriesPoint {
            timestamp: ts.to_rfc3339(),
            value: 0.0,
        });
        offset -= step_hours;
        if offset < 0 {
            break;
        }
    }
    points
}

fn series_from_map(
    values: HashMap<chrono::DateTime<chrono::Utc>, f64>,
) -> Vec<AnalyticsTimeSeriesPoint> {
    let mut entries: Vec<(chrono::DateTime<chrono::Utc>, f64)> = values.into_iter().collect();
    entries.sort_by_key(|(ts, _)| *ts);
    entries
        .into_iter()
        .map(|(ts, value)| AnalyticsTimeSeriesPoint {
            timestamp: ts.to_rfc3339(),
            value: round3(value),
        })
        .collect()
}

fn sum_time_series_maps(
    left: &HashMap<chrono::DateTime<chrono::Utc>, f64>,
    right: &HashMap<chrono::DateTime<chrono::Utc>, f64>,
) -> HashMap<chrono::DateTime<chrono::Utc>, f64> {
    let mut summed = left.clone();
    for (bucket, value) in right {
        *summed.entry(*bucket).or_insert(0.0) += value;
    }
    summed
}

async fn load_rate_schedule(state: &AppState) -> AnalyticsRateSchedule {
    let mut rate_schedule = empty_rate_schedule();

    if let (Some(base), Some(path)) = (
        state.config.rates_api_base_url.clone(),
        state.config.rates_api_path.clone(),
    ) {
        if let Ok(url) = reqwest::Url::parse(&format!("{}{}", base.trim_end_matches('/'), path)) {
            if let Ok(response) = state.http.get(url).send().await {
                if let Ok(payload) = response.json::<JsonValue>().await {
                    let provider = payload
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let default_rate = payload
                        .get("default_rate")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    rate_schedule.provider = provider.to_string();
                    rate_schedule.current_rate = default_rate;
                    rate_schedule.currency = payload
                        .get("currency")
                        .and_then(|v| v.as_str())
                        .map(|v| v.to_string());
                }
            }
        }
    }

    rate_schedule
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct SensorIdRow {
    sensor_id: String,
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct LatestAggregateRow {
    total_value: f64,
    avg_value: f64,
    last_ts: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct BatteryPowerRow {
    total_watts: f64,
}

async fn sensor_ids_for_integration_metric(
    state: &AppState,
    source: &str,
    metric: &str,
    expected_unit: Option<&str>,
) -> Result<Vec<String>, (StatusCode, String)> {
    let rows: Vec<SensorIdRow> = sqlx::query_as(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE deleted_at IS NULL
          AND COALESCE(config->>'source', '') = $1
          AND COALESCE(config->>'metric', '') = $2
          AND ($3::text IS NULL OR unit = $3)
        ORDER BY created_at ASC
        "#,
    )
    .bind(source)
    .bind(metric)
    .bind(expected_unit)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(rows.into_iter().map(|row| row.sensor_id).collect())
}

async fn emporia_power_sensor_ids_for_power_summary(
    state: &AppState,
) -> Result<Vec<String>, (StatusCode, String)> {
    let metadata: Option<SqlJson<JsonValue>> = sqlx::query_scalar(
        r#"
        SELECT metadata
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind("emporia")
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(metadata) = metadata.map(|value| value.0) else {
        let summary = sensor_ids_for_integration_metric(
            state,
            EMPORIA_SOURCE,
            EMPORIA_METRIC_SUMMARY_POWER_W,
            Some(UNIT_WATTS),
        )
        .await?;
        if !summary.is_empty() {
            return Ok(summary);
        }
        return sensor_ids_for_integration_metric(
            state,
            EMPORIA_SOURCE,
            EMPORIA_METRIC_MAINS_POWER_W,
            Some(UNIT_WATTS),
        )
        .await;
    };

    let included_device_gids = included_emporia_device_gids_from_metadata(&metadata);
    let Some(included_device_gids) = included_device_gids else {
        let summary = sensor_ids_for_integration_metric(
            state,
            EMPORIA_SOURCE,
            EMPORIA_METRIC_SUMMARY_POWER_W,
            Some(UNIT_WATTS),
        )
        .await?;
        if !summary.is_empty() {
            return Ok(summary);
        }
        return sensor_ids_for_integration_metric(
            state,
            EMPORIA_SOURCE,
            EMPORIA_METRIC_MAINS_POWER_W,
            Some(UNIT_WATTS),
        )
        .await;
    };

    if included_device_gids.is_empty() {
        return Ok(vec![]);
    }

    let rows: Vec<SensorIdRow> = sqlx::query_as(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE deleted_at IS NULL
          AND COALESCE(config->>'source', '') = $1
          AND COALESCE(config->>'metric', '') = $2
          AND unit = $3
          AND COALESCE(config->>'external_id', '') = ANY($4)
        ORDER BY created_at ASC
        "#,
    )
    .bind(EMPORIA_SOURCE)
    .bind(EMPORIA_METRIC_SUMMARY_POWER_W)
    .bind(UNIT_WATTS)
    .bind(included_device_gids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let sensor_ids: Vec<String> = rows.into_iter().map(|row| row.sensor_id).collect();
    if !sensor_ids.is_empty() {
        return Ok(sensor_ids);
    }

    sensor_ids_for_integration_metric(
        state,
        EMPORIA_SOURCE,
        EMPORIA_METRIC_MAINS_POWER_W,
        Some(UNIT_WATTS),
    )
    .await
}

fn included_emporia_device_gids_from_metadata(metadata: &JsonValue) -> Option<Vec<String>> {
    if let Some(devices_obj) = metadata.get("devices").and_then(|v| v.as_object()) {
        let mut included = Vec::new();
        for (device_gid, entry) in devices_obj {
            let enabled = entry
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let include_in_summary = entry
                .get("include_in_power_summary")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if enabled && include_in_summary {
                let gid = device_gid.trim();
                if !gid.is_empty() {
                    included.push(gid.to_string());
                }
            }
        }
        included.sort();
        included.dedup();
        return Some(included);
    }

    // Legacy fallback: `site_ids` was the allowlist in early builds.
    if let Some(site_ids) = metadata.get("site_ids").and_then(|v| v.as_array()) {
        let mut included = Vec::new();
        for item in site_ids {
            let gid = if let Some(num) = item.as_i64() {
                Some(num.to_string())
            } else {
                item.as_str().map(|s| s.to_string())
            };
            if let Some(gid) = gid {
                let gid = gid.trim().to_string();
                if !gid.is_empty() {
                    included.push(gid);
                }
            }
        }
        included.sort();
        included.dedup();
        return Some(included);
    }

    None
}

async fn latest_metrics_aggregate(
    state: &AppState,
    sensor_ids: &[String],
) -> Result<Option<LatestAggregateRow>, (StatusCode, String)> {
    if sensor_ids.is_empty() {
        return Ok(None);
    }

    let row: Option<LatestAggregateRow> = sqlx::query_as(
        r#"
        WITH latest_per_sensor AS (
            SELECT DISTINCT ON (sensor_id) sensor_id, value, ts
            FROM metrics
            WHERE sensor_id = ANY($1)
            ORDER BY sensor_id, ts DESC
        ),
        latest_per_node AS (
            SELECT DISTINCT ON (s.node_id) s.node_id, l.value, l.ts
            FROM latest_per_sensor l
            JOIN sensors s ON s.sensor_id = l.sensor_id
            ORDER BY s.node_id, l.ts DESC
        )
        SELECT
            COALESCE(sum(value), 0) as total_value,
            COALESCE(avg(value), 0) as avg_value,
            max(ts) as last_ts
        FROM latest_per_node
        "#,
    )
    .bind(sensor_ids)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(row)
}

async fn latest_battery_power_watts(
    state: &AppState,
    voltage_sensor_ids: &[String],
    current_sensor_ids: &[String],
) -> Result<BatteryPowerRow, (StatusCode, String)> {
    if voltage_sensor_ids.is_empty() || current_sensor_ids.is_empty() {
        return Ok(BatteryPowerRow { total_watts: 0.0 });
    }

    let row: BatteryPowerRow = sqlx::query_as(
        r#"
        WITH voltage_sensor AS (
            SELECT DISTINCT ON (node_id) node_id, sensor_id
            FROM sensors
            WHERE sensor_id = ANY($1)
              AND deleted_at IS NULL
            ORDER BY node_id, created_at ASC
        ),
        current_sensor AS (
            SELECT DISTINCT ON (node_id) node_id, sensor_id
            FROM sensors
            WHERE sensor_id = ANY($2)
              AND deleted_at IS NULL
            ORDER BY node_id, created_at ASC
        ),
        latest_voltage AS (
            SELECT DISTINCT ON (metrics.sensor_id) metrics.sensor_id, metrics.ts, metrics.value
            FROM metrics
            JOIN voltage_sensor vs ON vs.sensor_id = metrics.sensor_id
            ORDER BY metrics.sensor_id, metrics.ts DESC
        ),
        latest_current AS (
            SELECT DISTINCT ON (metrics.sensor_id) metrics.sensor_id, metrics.ts, metrics.value
            FROM metrics
            JOIN current_sensor cs ON cs.sensor_id = metrics.sensor_id
            ORDER BY metrics.sensor_id, metrics.ts DESC
        ),
        per_node AS (
            SELECT
                vs.node_id,
                lv.value as voltage_v,
                lc.value as current_a,
                GREATEST(lv.ts, lc.ts) as ts
            FROM voltage_sensor vs
            JOIN current_sensor cs ON cs.node_id = vs.node_id
            JOIN latest_voltage lv ON lv.sensor_id = vs.sensor_id
            JOIN latest_current lc ON lc.sensor_id = cs.sensor_id
        )
        SELECT
            COALESCE(sum(voltage_v * current_a), 0) as total_watts
        FROM per_node
        "#,
    )
    .bind(voltage_sensor_ids)
    .bind(current_sensor_ids)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(row)
}

async fn bucketed_average_battery_power_watts(
    state: &AppState,
    voltage_sensor_ids: &[String],
    current_sensor_ids: &[String],
    since: chrono::DateTime<chrono::Utc>,
    bucket_seconds: i32,
) -> Result<HashMap<chrono::DateTime<chrono::Utc>, f64>, (StatusCode, String)> {
    if voltage_sensor_ids.is_empty() || current_sensor_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let bucket_seconds = bucket_seconds.max(1);

    let buckets: Vec<HourBucketRow> = sqlx::query_as(
        r#"
        WITH voltage_sensor AS (
            SELECT DISTINCT ON (node_id) node_id, sensor_id
            FROM sensors
            WHERE sensor_id = ANY($1)
              AND deleted_at IS NULL
            ORDER BY node_id, created_at ASC
        ),
        current_sensor AS (
            SELECT DISTINCT ON (node_id) node_id, sensor_id
            FROM sensors
            WHERE sensor_id = ANY($2)
              AND deleted_at IS NULL
            ORDER BY node_id, created_at ASC
        ),
        voltage_bucketed AS (
            SELECT
                vs.node_id,
                time_bucket(make_interval(secs => $4), metrics.ts) as bucket,
                avg(metrics.value) as avg_value
            FROM metrics
            JOIN voltage_sensor vs ON vs.sensor_id = metrics.sensor_id
            WHERE metrics.ts >= $3
            GROUP BY vs.node_id, bucket
        ),
        current_bucketed AS (
            SELECT
                cs.node_id,
                time_bucket(make_interval(secs => $4), metrics.ts) as bucket,
                avg(metrics.value) as avg_value
            FROM metrics
            JOIN current_sensor cs ON cs.sensor_id = metrics.sensor_id
            WHERE metrics.ts >= $3
            GROUP BY cs.node_id, bucket
        )
        SELECT
            voltage_bucketed.bucket as bucket,
            sum(voltage_bucketed.avg_value * current_bucketed.avg_value) as avg_value
        FROM voltage_bucketed
        JOIN current_bucketed
          ON current_bucketed.node_id = voltage_bucketed.node_id
         AND current_bucketed.bucket = voltage_bucketed.bucket
        GROUP BY voltage_bucketed.bucket
        ORDER BY voltage_bucketed.bucket ASC
        "#,
    )
    .bind(voltage_sensor_ids)
    .bind(current_sensor_ids)
    .bind(since)
    .bind(bucket_seconds)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut values = HashMap::new();
    for row in buckets {
        values.insert(row.bucket, row.avg_value);
    }
    Ok(values)
}

async fn integrate_metrics_kwh(
    state: &AppState,
    sensor_ids: &[String],
    since: chrono::DateTime<chrono::Utc>,
    until: chrono::DateTime<chrono::Utc>,
) -> Result<f64, (StatusCode, String)> {
    if sensor_ids.is_empty() {
        return Ok(0.0);
    }

    let kwh: f64 = sqlx::query_scalar(
        r#"
        WITH selected_sensors AS (
            SELECT DISTINCT ON (node_id) node_id, sensor_id
            FROM sensors
            WHERE sensor_id = ANY($1)
              AND deleted_at IS NULL
            ORDER BY node_id, created_at ASC
        ),
        samples AS (
            SELECT selected_sensors.node_id,
                   metrics.ts,
                   metrics.value,
                   lead(metrics.ts) OVER (PARTITION BY selected_sensors.node_id ORDER BY metrics.ts) as next_ts
            FROM metrics
            JOIN selected_sensors ON metrics.sensor_id = selected_sensors.sensor_id
            WHERE metrics.ts >= $2
              AND metrics.ts <= $3
        )
        SELECT
            COALESCE(
                sum(value * extract(epoch FROM (COALESCE(next_ts, $3) - ts))),
                0
            ) / 3600.0 / 1000.0 as kwh
        FROM samples
        "#,
    )
    .bind(sensor_ids)
    .bind(since)
    .bind(until)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(round3(kwh))
}

async fn bucketed_average_series_watts(
    state: &AppState,
    sensor_ids: &[String],
    since: chrono::DateTime<chrono::Utc>,
    bucket_seconds: i32,
) -> Result<HashMap<chrono::DateTime<chrono::Utc>, f64>, (StatusCode, String)> {
    if sensor_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let bucket_seconds = bucket_seconds.max(1);

    let buckets: Vec<HourBucketRow> = sqlx::query_as(
        r#"
        SELECT bucket, sum(avg_value) as avg_value
        FROM (
            SELECT selected.node_id,
                   time_bucket(make_interval(secs => $3), metrics.ts) as bucket,
                   avg(metrics.value) as avg_value
            FROM (
                SELECT DISTINCT ON (node_id) node_id, sensor_id
                FROM sensors
                WHERE sensor_id = ANY($1)
                  AND deleted_at IS NULL
                ORDER BY node_id, created_at ASC
            ) selected
            JOIN metrics ON metrics.sensor_id = selected.sensor_id
            WHERE metrics.ts >= $2
            GROUP BY selected.node_id, bucket
        ) per_node
        GROUP BY bucket
        ORDER BY bucket ASC
        "#,
    )
    .bind(sensor_ids)
    .bind(since)
    .bind(bucket_seconds)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut values = HashMap::new();
    for row in buckets {
        values.insert(row.bucket, row.avg_value);
    }
    Ok(values)
}

#[utoipa::path(
    get,
    path = "/api/analytics/power",
    tag = "analytics",
    responses(
        (status = 200, description = "Power analytics", body = AnalyticsPower),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn power(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<AnalyticsPower>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ANALYTICS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;
    let rate_schedule = load_rate_schedule(&state).await;

    let now = chrono::Utc::now();
    let emporia_power_sensor_ids = emporia_power_sensor_ids_for_power_summary(&state).await?;
    let load_power_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_LOAD_POWER_W,
        Some(UNIT_WATTS),
    )
    .await?;
    let solar_power_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_PV_POWER_W,
        Some(UNIT_WATTS),
    )
    .await?;
    let battery_voltage_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_BATTERY_VOLTAGE_V,
        Some(UNIT_VOLTS),
    )
    .await?;
    let battery_current_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_BATTERY_CURRENT_A,
        Some(UNIT_AMPS),
    )
    .await?;

    let load_live = latest_metrics_aggregate(&state, &load_power_sensor_ids)
        .await?
        .unwrap_or(LatestAggregateRow {
            total_value: 0.0,
            avg_value: 0.0,
            last_ts: None,
        });
    let emporia_live = latest_metrics_aggregate(&state, &emporia_power_sensor_ids)
        .await?
        .unwrap_or(LatestAggregateRow {
            total_value: 0.0,
            avg_value: 0.0,
            last_ts: None,
        });
    let solar_live = latest_metrics_aggregate(&state, &solar_power_sensor_ids)
        .await?
        .unwrap_or(LatestAggregateRow {
            total_value: 0.0,
            avg_value: 0.0,
            last_ts: None,
        });
    let battery_live = latest_battery_power_watts(
        &state,
        &battery_voltage_sensor_ids,
        &battery_current_sensor_ids,
    )
    .await?;

    let load_since_168 = now - chrono::Duration::hours(168);
    let emporia_since_168 = load_since_168;
    let load_hourly = bucketed_average_series_watts(
        &state,
        &load_power_sensor_ids,
        load_since_168,
        POWER_BUCKET_168H_SECONDS,
    )
    .await?;
    let emporia_hourly = bucketed_average_series_watts(
        &state,
        &emporia_power_sensor_ids,
        emporia_since_168,
        POWER_BUCKET_168H_SECONDS,
    )
    .await?;
    let solar_hourly = bucketed_average_series_watts(
        &state,
        &solar_power_sensor_ids,
        load_since_168,
        POWER_BUCKET_168H_SECONDS,
    )
    .await?;
    let battery_hourly_watts = bucketed_average_battery_power_watts(
        &state,
        &battery_voltage_sensor_ids,
        &battery_current_sensor_ids,
        load_since_168,
        POWER_BUCKET_168H_SECONDS,
    )
    .await?;

    let load_bucketed_24h = bucketed_average_series_watts(
        &state,
        &load_power_sensor_ids,
        now - chrono::Duration::hours(24),
        POWER_BUCKET_24H_SECONDS,
    )
    .await?;
    let emporia_bucketed_24h = bucketed_average_series_watts(
        &state,
        &emporia_power_sensor_ids,
        now - chrono::Duration::hours(24),
        POWER_BUCKET_24H_SECONDS,
    )
    .await?;
    let solar_bucketed_24h = bucketed_average_series_watts(
        &state,
        &solar_power_sensor_ids,
        now - chrono::Duration::hours(24),
        POWER_BUCKET_24H_SOLAR_SECONDS,
    )
    .await?;
    let battery_bucketed_24h = bucketed_average_battery_power_watts(
        &state,
        &battery_voltage_sensor_ids,
        &battery_current_sensor_ids,
        now - chrono::Duration::hours(24),
        POWER_BUCKET_24H_BATTERY_SECONDS,
    )
    .await?;

    let mut load_hourly_kw: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in load_hourly {
        load_hourly_kw.insert(bucket, avg_watts / 1000.0);
    }
    let mut emporia_hourly_kw: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in emporia_hourly {
        emporia_hourly_kw.insert(bucket, avg_watts / 1000.0);
    }
    let mut solar_hourly_kw: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in solar_hourly {
        solar_hourly_kw.insert(bucket, avg_watts / 1000.0);
    }
    let mut battery_hourly_kw: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in battery_hourly_watts.iter() {
        battery_hourly_kw.insert(*bucket, *avg_watts / 1000.0);
    }

    let mut load_bucketed_kw_24h: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in load_bucketed_24h {
        load_bucketed_kw_24h.insert(bucket, avg_watts / 1000.0);
    }
    let mut emporia_bucketed_kw_24h: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in emporia_bucketed_24h {
        emporia_bucketed_kw_24h.insert(bucket, avg_watts / 1000.0);
    }
    let mut solar_bucketed_kw_24h: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in solar_bucketed_24h {
        solar_bucketed_kw_24h.insert(bucket, avg_watts / 1000.0);
    }
    let mut battery_bucketed_kw_24h: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for (bucket, avg_watts) in battery_bucketed_24h {
        battery_bucketed_kw_24h.insert(bucket, avg_watts / 1000.0);
    }

    let total_hourly_kw = sum_time_series_maps(&load_hourly_kw, &emporia_hourly_kw);
    let total_bucketed_kw_24h =
        sum_time_series_maps(&load_bucketed_kw_24h, &emporia_bucketed_kw_24h);

    let kwh_24h = integrate_metrics_kwh(
        &state,
        &load_power_sensor_ids,
        now - chrono::Duration::hours(24),
        now,
    )
    .await?;
    let kwh_168h =
        integrate_metrics_kwh(&state, &load_power_sensor_ids, load_since_168, now).await?;
    let grid_kwh_24h = integrate_metrics_kwh(
        &state,
        &emporia_power_sensor_ids,
        now - chrono::Duration::hours(24),
        now,
    )
    .await?;
    let grid_kwh_168h =
        integrate_metrics_kwh(&state, &emporia_power_sensor_ids, load_since_168, now).await?;
    let solar_kwh_24h = integrate_metrics_kwh(
        &state,
        &solar_power_sensor_ids,
        now - chrono::Duration::hours(24),
        now,
    )
    .await?;
    let solar_kwh_168h =
        integrate_metrics_kwh(&state, &solar_power_sensor_ids, load_since_168, now).await?;

    let battery_kwh_168h = round3(battery_hourly_watts.values().sum::<f64>() / 1000.0);
    let battery_since_24 = truncate_to_hour(now) - chrono::Duration::hours(24);
    let battery_kwh_24h = round3(
        battery_hourly_watts
            .iter()
            .filter(|(bucket, _)| **bucket >= battery_since_24)
            .map(|(_, watts)| *watts)
            .sum::<f64>()
            / 1000.0,
    );

    Ok(Json(AnalyticsPower {
        live_kw: round3((load_live.total_value + emporia_live.total_value) / 1000.0),
        live_solar_kw: round3(solar_live.total_value / 1000.0),
        live_grid_kw: round3(emporia_live.total_value / 1000.0),
        live_battery_kw: round3(battery_live.total_watts / 1000.0),
        kwh_24h: round3(kwh_24h + grid_kwh_24h),
        kwh_168h: round3(kwh_168h + grid_kwh_168h),
        solar_kwh_24h,
        solar_kwh_168h,
        grid_kwh_24h,
        grid_kwh_168h,
        battery_kwh_24h,
        battery_kwh_168h,
        series_24h: series_from_map(total_bucketed_kw_24h),
        series_168h: series_from_map(total_hourly_kw),
        solar_series_24h: series_from_map(solar_bucketed_kw_24h),
        solar_series_168h: series_from_map(solar_hourly_kw),
        grid_series_24h: series_from_map(emporia_bucketed_kw_24h),
        grid_series_168h: series_from_map(emporia_hourly_kw),
        battery_series_24h: series_from_map(battery_bucketed_kw_24h),
        battery_series_168h: series_from_map(battery_hourly_kw),
        integrations: vec![],
        rate_schedule,
    }))
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsWater {
    pub(crate) domestic_gal_24h: f64,
    pub(crate) domestic_gal_168h: f64,
    pub(crate) ag_gal_168h: f64,
    pub(crate) reservoir_depth: Vec<AnalyticsTimeSeriesPoint>,
    pub(crate) domestic_series: Vec<AnalyticsTimeSeriesPoint>,
    pub(crate) ag_series: Vec<AnalyticsTimeSeriesPoint>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct WaterLevelSensorRow {
    sensor_id: String,
    name: String,
    unit: String,
    config: SqlJson<JsonValue>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct HourBucketRow {
    bucket: chrono::DateTime<chrono::Utc>,
    avg_value: f64,
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn truncate_to_hour(ts: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    Utc.with_ymd_and_hms(ts.year(), ts.month(), ts.day(), ts.hour(), 0, 0)
        .single()
        .unwrap_or(ts)
}

fn depth_to_feet(value: f64, unit: &str) -> f64 {
    match unit.trim().to_lowercase().as_str() {
        "ft" => value,
        "in" => value / 12.0,
        "m" => value * 3.280_84,
        "cm" => value * 0.032_808_4,
        "mm" => value * 0.003_280_84,
        _ => value,
    }
}

fn json_string_field(value: &JsonValue, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn sensor_matches_reservoir(sensor: &WaterLevelSensorRow) -> bool {
    let name = sensor.name.to_lowercase();
    if name.contains("reservoir") {
        return true;
    }
    let config = &sensor.config.0;
    let location = json_string_field(config, "location")
        .unwrap_or_default()
        .to_lowercase();
    location.contains("reservoir")
}

fn select_reservoir_sensor(sensors: &[WaterLevelSensorRow]) -> Option<&WaterLevelSensorRow> {
    let explicit = sensors.iter().find(|sensor| {
        let role = json_string_field(&sensor.config.0, "analytics_role")
            .unwrap_or_default()
            .to_lowercase();
        matches!(role.as_str(), "reservoir_depth" | "reservoir")
    });
    if explicit.is_some() {
        return explicit;
    }
    sensors
        .iter()
        .find(|sensor| sensor_matches_reservoir(sensor))
}

fn build_trailing_window_avg_series(
    hourly_values: &HashMap<chrono::DateTime<chrono::Utc>, f64>,
    reference: chrono::DateTime<chrono::Utc>,
    total_hours: i64,
    step_hours: i64,
) -> Vec<AnalyticsTimeSeriesPoint> {
    let aligned = truncate_to_hour(reference);
    let step_hours = step_hours.max(1);
    let mut points: Vec<AnalyticsTimeSeriesPoint> = Vec::new();
    let mut offset = total_hours.max(0);
    while offset >= 0 {
        let anchor = aligned - chrono::Duration::hours(offset);
        let mut window: Vec<f64> = Vec::new();
        for idx in 0..step_hours {
            let bucket = anchor - chrono::Duration::hours(idx);
            if let Some(value) = hourly_values.get(&bucket) {
                window.push(*value);
            }
        }
        let value = if window.is_empty() {
            0.0
        } else {
            round3(window.iter().sum::<f64>() / (window.len() as f64))
        };
        points.push(AnalyticsTimeSeriesPoint {
            timestamp: anchor.to_rfc3339(),
            value,
        });
        offset -= step_hours;
        if offset < 0 {
            break;
        }
    }
    points
}

#[utoipa::path(
    get,
    path = "/api/analytics/water",
    tag = "analytics",
    responses(
        (status = 200, description = "Water analytics", body = AnalyticsWater),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn water(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<AnalyticsWater>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ANALYTICS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;
    let now = chrono::Utc::now();
    let mut response = AnalyticsWater {
        domestic_gal_24h: 0.0,
        domestic_gal_168h: 0.0,
        ag_gal_168h: 0.0,
        reservoir_depth: build_series(now, 168, 24),
        domestic_series: build_series(now, 24, 1),
        ag_series: build_series(now, 168, 24),
    };

    let sensors: Vec<WaterLevelSensorRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            name,
            unit,
            COALESCE(config, '{}'::jsonb) as config
        FROM sensors
        WHERE deleted_at IS NULL
          AND type = 'water_level'
        ORDER BY created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(sensor) = select_reservoir_sensor(&sensors) else {
        return Ok(Json(response));
    };

    let reference: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        r#"
        SELECT max(ts) as "max_ts"
        FROM metrics
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor.sensor_id.as_str())
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(reference) = reference else {
        return Ok(Json(response));
    };

    let since = now - chrono::Duration::hours(168);
    let buckets: Vec<HourBucketRow> = sqlx::query_as(
        r#"
        SELECT
            date_trunc('hour', ts) as bucket,
            avg(value) as avg_value
        FROM metrics
        WHERE sensor_id = $1
          AND ts >= $2
        GROUP BY bucket
        ORDER BY bucket ASC
        "#,
    )
    .bind(sensor.sensor_id.as_str())
    .bind(since)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    if buckets.is_empty() {
        return Ok(Json(response));
    }

    let mut hourly_values: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
    for row in buckets {
        hourly_values.insert(row.bucket, depth_to_feet(row.avg_value, &sensor.unit));
    }

    response.reservoir_depth = build_trailing_window_avg_series(&hourly_values, reference, 168, 24);
    Ok(Json(response))
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AnalyticsStatus {
    pub(crate) alarms_last_168h: i64,
    pub(crate) nodes_online: i64,
    pub(crate) nodes_offline: i64,
    #[serde(default)]
    pub(crate) remote_nodes_online: i64,
    #[serde(default)]
    pub(crate) remote_nodes_offline: i64,
    pub(crate) battery_soc: i64,
    pub(crate) solar_kw: f64,
    #[serde(default)]
    pub(crate) current_load_kw: f64,
    pub(crate) battery_runtime_hours: f64,
    #[serde(default)]
    pub(crate) estimated_runtime_hours: f64,
    #[serde(default)]
    pub(crate) storage_capacity_kwh: f64,
    #[serde(default)]
    pub(crate) last_updated: Option<String>,
    #[serde(default)]
    pub(crate) feeds: std::collections::HashMap<String, AnalyticsFeedStatusEntry>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct SoilSensorRow {
    sensor_id: String,
    name: String,
    config: SqlJson<JsonValue>,
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct SoilBucketRow {
    bucket: chrono::DateTime<chrono::Utc>,
    avg_value: f64,
    min_value: f64,
    max_value: f64,
}

#[derive(sqlx::FromRow, Debug, Clone)]
struct LatestSensorValueRow {
    sensor_id: String,
    value: f64,
}

fn soil_field_label(name: &str, config: &JsonValue) -> Option<String> {
    if let Some(label) = json_string_field(config, "field") {
        let trimmed = label.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(label) = json_string_field(config, "location") {
        let trimmed = label.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(label) = json_string_field(config, "ws_field") {
        let trimmed = label.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let start = name.rfind('(')?;
    let end = name.rfind(')')?;
    if end <= start + 1 {
        return None;
    }
    let inside = name[start + 1..end].trim();
    if inside.is_empty() {
        return None;
    }
    Some(inside.to_string())
}

async fn soil_sensor_ids(state: &AppState) -> Result<Vec<String>, (StatusCode, String)> {
    let rows: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE deleted_at IS NULL
          AND unit = $1
          AND type = ANY($2)
        ORDER BY created_at ASC
        "#,
    )
    .bind(UNIT_PERCENT)
    .bind(SOIL_SENSOR_TYPES.as_slice())
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(rows.into_iter().map(|(id,)| id).collect())
}

#[utoipa::path(
    get,
    path = "/api/analytics/soil",
    tag = "analytics",
    responses(
        (status = 200, description = "Soil analytics", body = AnalyticsSoil),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn soil(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<AnalyticsSoil>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ANALYTICS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;
    let now = chrono::Utc::now();
    let mut response = AnalyticsSoil {
        fields: vec![],
        series: build_series(now, SOIL_HISTORY_HOURS, 6),
        series_avg: vec![],
        series_min: vec![],
        series_max: vec![],
    };

    let sensor_ids = soil_sensor_ids(&state).await?;
    if sensor_ids.is_empty() {
        return Ok(Json(response));
    }

    let since = now - chrono::Duration::hours(SOIL_HISTORY_HOURS);
    let bucket_seconds = SOIL_BUCKET_168H_SECONDS.max(1);
    let buckets: Vec<SoilBucketRow> = sqlx::query_as(
        r#"
        WITH per_sensor AS (
            SELECT
                time_bucket(make_interval(secs => $3), metrics.ts) as bucket,
                metrics.sensor_id,
                avg(metrics.value) as avg_value
            FROM metrics
            WHERE metrics.sensor_id = ANY($1)
              AND metrics.ts >= $2
            GROUP BY bucket, metrics.sensor_id
        )
        SELECT
            bucket,
            avg(avg_value) as avg_value,
            min(avg_value) as min_value,
            max(avg_value) as max_value
        FROM per_sensor
        GROUP BY bucket
        ORDER BY bucket ASC
        "#,
    )
    .bind(&sensor_ids)
    .bind(since)
    .bind(bucket_seconds)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    if !buckets.is_empty() {
        let mut avg_map: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
        let mut min_map: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
        let mut max_map: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
        for row in buckets {
            avg_map.insert(row.bucket, row.avg_value);
            min_map.insert(row.bucket, row.min_value);
            max_map.insert(row.bucket, row.max_value);
        }
        response.series_avg = series_from_map(avg_map);
        response.series_min = series_from_map(min_map);
        response.series_max = series_from_map(max_map);
        response.series = response.series_avg.clone();
    }

    let sensors: Vec<SoilSensorRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, name, COALESCE(config, '{}'::jsonb) as config
        FROM sensors
        WHERE sensor_id = ANY($1)
          AND deleted_at IS NULL
        ORDER BY created_at ASC
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let latest_values: Vec<LatestSensorValueRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (sensor_id) sensor_id, value
        FROM metrics
        WHERE sensor_id = ANY($1)
        ORDER BY sensor_id, ts DESC
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    if !sensors.is_empty() && !latest_values.is_empty() {
        let latest_by_id: HashMap<String, f64> = latest_values
            .into_iter()
            .map(|row| (row.sensor_id, row.value))
            .collect();
        let mut field_values: HashMap<String, Vec<f64>> = HashMap::new();
        for sensor in sensors {
            let Some(value) = latest_by_id.get(&sensor.sensor_id) else {
                continue;
            };
            let label = soil_field_label(&sensor.name, &sensor.config.0).unwrap_or_else(|| {
                let trimmed = sensor.name.trim();
                if trimmed.is_empty() {
                    "Unassigned".to_string()
                } else {
                    trimmed.to_string()
                }
            });
            field_values.entry(label).or_default().push(*value);
        }

        let mut fields: Vec<AnalyticsSoilField> = Vec::new();
        for (name, values) in field_values {
            if values.is_empty() {
                continue;
            }
            let min = values
                .iter()
                .copied()
                .fold(f64::INFINITY, |acc, v| acc.min(v));
            let max = values
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, |acc, v| acc.max(v));
            let avg = values.iter().sum::<f64>() / (values.len() as f64);
            fields.push(AnalyticsSoilField {
                name,
                min: round3(min),
                max: round3(max),
                avg: round3(avg),
            });
        }
        fields.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        response.fields = fields;
    }

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/analytics/status",
    tag = "analytics",
    responses(
        (status = 200, description = "System analytics status", body = AnalyticsStatus),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn status(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<AnalyticsStatus>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ANALYTICS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;
    let online: i64 = sqlx::query_scalar("SELECT count(*) FROM nodes WHERE status = 'online'")
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?;
    let total: i64 = sqlx::query_scalar("SELECT count(*) FROM nodes")
        .fetch_one(&state.db)
        .await
        .map_err(map_db_error)?;
    let offline = (total - online).max(0);
    let alarms_last_168h: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*) FROM alarm_events
        WHERE created_at >= NOW() - INTERVAL '168 hours'
        "#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    let solar_power_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_PV_POWER_W,
        Some(UNIT_WATTS),
    )
    .await?;
    let load_power_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_LOAD_POWER_W,
        Some(UNIT_WATTS),
    )
    .await?;
    let soc_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_BATTERY_SOC_PERCENT,
        Some(UNIT_PERCENT),
    )
    .await?;
    let runtime_sensor_ids = sensor_ids_for_integration_metric(
        &state,
        RENOGY_SOURCE,
        RENOGY_METRIC_RUNTIME_HOURS,
        Some(UNIT_HOURS),
    )
    .await?;

    let solar_live = latest_metrics_aggregate(&state, &solar_power_sensor_ids)
        .await?
        .unwrap_or(LatestAggregateRow {
            total_value: 0.0,
            avg_value: 0.0,
            last_ts: None,
        });
    let load_live = latest_metrics_aggregate(&state, &load_power_sensor_ids)
        .await?
        .unwrap_or(LatestAggregateRow {
            total_value: 0.0,
            avg_value: 0.0,
            last_ts: None,
        });
    let soc_live = latest_metrics_aggregate(&state, &soc_sensor_ids)
        .await?
        .unwrap_or(LatestAggregateRow {
            total_value: 0.0,
            avg_value: 0.0,
            last_ts: None,
        });
    let runtime_live = latest_metrics_aggregate(&state, &runtime_sensor_ids)
        .await?
        .unwrap_or(LatestAggregateRow {
            total_value: 0.0,
            avg_value: 0.0,
            last_ts: None,
        });

    let last_updated = [
        solar_live.last_ts,
        load_live.last_ts,
        soc_live.last_ts,
        runtime_live.last_ts,
    ]
    .into_iter()
    .flatten()
    .max()
    .map(|ts| ts.to_rfc3339());

    Ok(Json(AnalyticsStatus {
        alarms_last_168h,
        nodes_online: online,
        nodes_offline: offline,
        remote_nodes_online: 0,
        remote_nodes_offline: 0,
        battery_soc: soc_live.avg_value.round().clamp(0.0, 100.0) as i64,
        solar_kw: round3(solar_live.total_value / 1000.0),
        current_load_kw: round3(load_live.total_value / 1000.0),
        battery_runtime_hours: round3(runtime_live.avg_value),
        estimated_runtime_hours: round3(runtime_live.avg_value),
        storage_capacity_kwh: 0.0,
        last_updated,
        feeds: std::collections::HashMap::new(),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/analytics/feeds/status", get(feeds_status))
        .route("/analytics/feeds/poll", post(feeds_poll))
        .route("/analytics/power", get(power))
        .route("/analytics/water", get(water))
        .route("/analytics/soil", get(soil))
        .route("/analytics/status", get(status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_to_feet_converts_units() {
        assert!((depth_to_feet(12.0, "in") - 1.0).abs() < 1e-9);
        assert!((depth_to_feet(1.0, "m") - 3.280_84).abs() < 1e-9);
        assert!((depth_to_feet(2.0, "ft") - 2.0).abs() < 1e-9);
    }

    #[test]
    fn select_reservoir_sensor_prefers_explicit_role_then_name() {
        let sensors = vec![
            WaterLevelSensorRow {
                sensor_id: "a".to_string(),
                name: "Tank".to_string(),
                unit: "in".to_string(),
                config: SqlJson(serde_json::json!({})),
            },
            WaterLevelSensorRow {
                sensor_id: "b".to_string(),
                name: "Reservoir Level".to_string(),
                unit: "in".to_string(),
                config: SqlJson(serde_json::json!({})),
            },
            WaterLevelSensorRow {
                sensor_id: "c".to_string(),
                name: "Other".to_string(),
                unit: "in".to_string(),
                config: SqlJson(serde_json::json!({"analytics_role": "reservoir_depth"})),
            },
        ];

        let selected = select_reservoir_sensor(&sensors).expect("expected sensor");
        assert_eq!(selected.sensor_id, "c");
    }

    #[test]
    fn build_trailing_window_avg_series_matches_trailing_average() {
        let reference = Utc.with_ymd_and_hms(2025, 1, 2, 0, 30, 0).unwrap();
        let aligned = truncate_to_hour(reference);
        let mut hourly_values: HashMap<chrono::DateTime<chrono::Utc>, f64> = HashMap::new();
        hourly_values.insert(aligned, 24.0);
        hourly_values.insert(aligned - chrono::Duration::hours(1), 12.0);

        let series = build_trailing_window_avg_series(&hourly_values, reference, 24, 24);
        assert_eq!(series.len(), 2);
        let latest = series.last().unwrap();
        assert!((latest.value - 18.0).abs() < 1e-9);
    }
}
