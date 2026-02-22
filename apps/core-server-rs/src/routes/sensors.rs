use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::{AuthUser, OptionalAuthUser};
use crate::error::map_db_error;
use crate::ids;
use crate::services::derived_sensors;
use crate::services::sensor_visibility;
use crate::services::sensor_visibility::SensorVisibilityInfo;
use crate::state::AppState;

const SENSOR_CONFIG_SOURCE_FORECAST_POINTS: &str = "forecast_points";
const SENSOR_CONFIG_SOURCE_DERIVED: &str = derived_sensors::SENSOR_CONFIG_SOURCE_DERIVED;
/// Must match `apps/core-server-rs/src/services/analysis/bucket_reader.rs`.
const MAX_DERIVED_SENSOR_DEPTH: usize = 10;

#[derive(sqlx::FromRow)]
pub(crate) struct SensorRow {
    sensor_id: String,
    node_id: Uuid,
    name: String,
    sensor_type: String,
    unit: String,
    interval_seconds: i32,
    rolling_avg_seconds: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    config: SqlJson<JsonValue>,
    latest_value: Option<f64>,
    latest_ts: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct SensorResponse {
    sensor_id: String,
    node_id: String,
    name: String,
    #[serde(rename = "type")]
    sensor_type: String,
    unit: String,
    interval_seconds: i32,
    rolling_avg_seconds: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_ts: Option<String>,
    created_at: String,
    deleted_at: Option<String>,
    config: JsonValue,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<SensorVisibilityInfo>,
}

impl From<SensorRow> for SensorResponse {
    fn from(row: SensorRow) -> Self {
        Self {
            sensor_id: row.sensor_id,
            node_id: row.node_id.to_string(),
            name: row.name,
            sensor_type: row.sensor_type,
            unit: row.unit,
            interval_seconds: row.interval_seconds,
            rolling_avg_seconds: row.rolling_avg_seconds,
            latest_value: row.latest_value,
            latest_ts: row.latest_ts.map(|ts| ts.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            deleted_at: row.deleted_at.map(|ts| ts.to_rfc3339()),
            config: row.config.0,
            visibility: None,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct SensorsQuery {
    node_id: Option<Uuid>,
    #[serde(default)]
    include_hidden: bool,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct SensorCreateRequest {
    sensor_id: Option<String>,
    node_id: Uuid,
    name: String,
    #[serde(rename = "type")]
    sensor_type: String,
    unit: String,
    interval_seconds: i32,
    #[serde(default)]
    rolling_avg_seconds: i32,
    deleted_at: Option<String>,
    config: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize, Default, utoipa::ToSchema)]
pub(crate) struct SensorUpdateRequest {
    name: Option<String>,
    #[serde(rename = "type")]
    sensor_type: Option<String>,
    unit: Option<String>,
    interval_seconds: Option<i32>,
    rolling_avg_seconds: Option<i32>,
    deleted_at: Option<String>,
    config: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct SensorDeleteQuery {
    #[serde(default = "default_keep_data")]
    keep_data: bool,
}

fn default_keep_data() -> bool {
    true
}

#[derive(sqlx::FromRow)]
struct NodeIdentityRow {
    id: Uuid,
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    config: SqlJson<JsonValue>,
}

fn update_sensor_manifest(config: &mut JsonValue, sensor: &SensorResponse) {
    let JsonValue::Object(config_map) = config else {
        *config = JsonValue::Object(Default::default());
        return update_sensor_manifest(config, sensor);
    };

    let mut manifest = match config_map.remove("sensor_manifest") {
        Some(JsonValue::Object(map)) => map,
        _ => serde_json::Map::new(),
    };

    let display_decimals = sensor
        .config
        .get("display_decimals")
        .and_then(|value| match value {
            JsonValue::Number(num) => num.as_i64(),
            JsonValue::String(raw) => raw.trim().parse::<i64>().ok(),
            _ => None,
        })
        .filter(|value| (0..=6).contains(value));

    let mut sensor_entry = serde_json::json!({
        "sensor_id": sensor.sensor_id,
        "name": sensor.name,
        "type": sensor.sensor_type,
        "unit": sensor.unit,
        "interval_seconds": sensor.interval_seconds,
        "rolling_avg_seconds": sensor.rolling_avg_seconds,
        "deleted_at": sensor.deleted_at,
        "updated_at": chrono::Utc::now().to_rfc3339(),
    });

    if let Some(decimals) = display_decimals {
        if let JsonValue::Object(map) = &mut sensor_entry {
            map.insert(
                "display_decimals".to_string(),
                JsonValue::Number(serde_json::Number::from(decimals)),
            );
        }
    }

    manifest.insert(sensor.sensor_id.clone(), sensor_entry);

    config_map.insert("sensor_manifest".to_string(), JsonValue::Object(manifest));
}

fn remove_sensor_manifest(config: &mut JsonValue, sensor_id: &str) {
    let JsonValue::Object(config_map) = config else {
        *config = JsonValue::Object(Default::default());
        return remove_sensor_manifest(config, sensor_id);
    };

    let mut manifest = match config_map.remove("sensor_manifest") {
        Some(JsonValue::Object(map)) => map,
        _ => serde_json::Map::new(),
    };

    manifest.remove(sensor_id);
    config_map.insert("sensor_manifest".to_string(), JsonValue::Object(manifest));
}

fn parse_rfc3339(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, (StatusCode, String)> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    chrono::DateTime::parse_from_rfc3339(trimmed)
        .map(|ts| Some(ts.with_timezone(&chrono::Utc)))
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                format!("Invalid {field} timestamp"),
            )
        })
}

#[derive(sqlx::FromRow)]
struct SensorListRow {
    sensor_id: String,
    node_id: Uuid,
    name: String,
    sensor_type: String,
    unit: String,
    interval_seconds: i32,
    rolling_avg_seconds: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    config: SqlJson<JsonValue>,
    latest_value: Option<f64>,
    latest_ts: Option<chrono::DateTime<chrono::Utc>>,
    node_config: SqlJson<JsonValue>,
}

pub(crate) async fn fetch_sensors(
    db: &sqlx::PgPool,
    node_id: Option<Uuid>,
    include_hidden: bool,
) -> Result<Vec<SensorResponse>, sqlx::Error> {
    let rows: Vec<SensorListRow> = sqlx::query_as(
	        r#"
	        SELECT
	            sensors.sensor_id,
	            sensors.node_id,
	            sensors.name,
	            sensors.type as sensor_type,
	            sensors.unit,
	            sensors.interval_seconds,
	            sensors.rolling_avg_seconds,
	            sensors.created_at,
	            sensors.deleted_at,
	            COALESCE(sensors.config, '{}'::jsonb) as config,
	            CASE
	              WHEN COALESCE(sensors.config, '{}'::jsonb)->>'source' = $2 THEN forecast_latest.value
	              ELSE metrics_latest.value
	            END as latest_value,
	            CASE
	              WHEN COALESCE(sensors.config, '{}'::jsonb)->>'source' = $2 THEN forecast_latest.ts
	              ELSE metrics_latest.ts
	            END as latest_ts
                ,
                COALESCE(nodes.config, '{}'::jsonb) as node_config
	        FROM sensors
	        JOIN nodes ON nodes.id = sensors.node_id
	        LEFT JOIN LATERAL (
	            SELECT value, ts
	            FROM metrics
	            WHERE metrics.sensor_id = sensors.sensor_id
	            ORDER BY ts DESC
	            LIMIT 1
	        ) metrics_latest ON true
	        LEFT JOIN LATERAL (
	            SELECT fp.value as value, fp.ts as ts
	            FROM forecast_points fp
	            WHERE fp.provider = (COALESCE(sensors.config, '{}'::jsonb)->>'provider')
	              AND fp.kind = (COALESCE(sensors.config, '{}'::jsonb)->>'kind')
	              AND fp.subject_kind = (COALESCE(sensors.config, '{}'::jsonb)->>'subject_kind')
	              AND fp.subject = COALESCE((COALESCE(sensors.config, '{}'::jsonb)->>'subject'), sensors.node_id::text)
	              AND fp.metric = (COALESCE(sensors.config, '{}'::jsonb)->>'metric')
	              AND fp.ts <= NOW()
	              AND (
	                (COALESCE(sensors.config, '{}'::jsonb)->>'mode') IS DISTINCT FROM 'asof'
	                OR fp.issued_at <= fp.ts
	              )
	            ORDER BY fp.ts DESC, fp.issued_at DESC
	            LIMIT 1
		        ) forecast_latest ON true
		        WHERE deleted_at IS NULL
		          AND ($1::uuid IS NULL OR node_id = $1)
			        ORDER BY
			          nodes.ui_order NULLS LAST,
			          nodes.created_at ASC,
			          sensors.ui_order NULLS LAST,
			          sensors.created_at ASC,
			          sensors.sensor_id ASC
			        "#,
			    )
	    .bind(node_id)
	    .bind(SENSOR_CONFIG_SOURCE_FORECAST_POINTS)
    .fetch_all(db)
    .await?;

    let mut out: Vec<SensorResponse> = Vec::with_capacity(rows.len());
    for row in rows {
        let visibility =
            sensor_visibility::evaluate_sensor_visibility(&row.config.0, &row.node_config.0);
        if !include_hidden && !visibility.visible {
            continue;
        }
        out.push(SensorResponse {
            sensor_id: row.sensor_id,
            node_id: row.node_id.to_string(),
            name: row.name,
            sensor_type: row.sensor_type,
            unit: row.unit,
            interval_seconds: row.interval_seconds,
            rolling_avg_seconds: row.rolling_avg_seconds,
            latest_value: row.latest_value,
            latest_ts: row.latest_ts.map(|ts| ts.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            deleted_at: row.deleted_at.map(|ts| ts.to_rfc3339()),
            config: row.config.0,
            visibility: if include_hidden {
                Some(visibility)
            } else {
                None
            },
        });
    }

    Ok(out)
}

#[derive(sqlx::FromRow)]
struct LatestValueRow {
    sensor_id: String,
    latest_value: Option<f64>,
    latest_ts: Option<chrono::DateTime<chrono::Utc>>,
}

async fn fetch_latest_values(
    db: &sqlx::PgPool,
    sensor_ids: &[String],
) -> Result<
    std::collections::HashMap<String, (Option<f64>, Option<chrono::DateTime<chrono::Utc>>)>,
    sqlx::Error,
> {
    if sensor_ids.is_empty() {
        return Ok(Default::default());
    }

    let rows: Vec<LatestValueRow> = sqlx::query_as(
        r#"
        SELECT
            sensors.sensor_id,
            CASE
              WHEN COALESCE(sensors.config, '{}'::jsonb)->>'source' = $2 THEN forecast_latest.value
              ELSE metrics_latest.value
            END as latest_value,
            CASE
              WHEN COALESCE(sensors.config, '{}'::jsonb)->>'source' = $2 THEN forecast_latest.ts
              ELSE metrics_latest.ts
            END as latest_ts
        FROM sensors
        LEFT JOIN LATERAL (
            SELECT value, ts
            FROM metrics
            WHERE metrics.sensor_id = sensors.sensor_id
            ORDER BY ts DESC
            LIMIT 1
        ) metrics_latest ON true
        LEFT JOIN LATERAL (
            SELECT fp.value as value, fp.ts as ts
            FROM forecast_points fp
            WHERE fp.provider = (COALESCE(sensors.config, '{}'::jsonb)->>'provider')
              AND fp.kind = (COALESCE(sensors.config, '{}'::jsonb)->>'kind')
              AND fp.subject_kind = (COALESCE(sensors.config, '{}'::jsonb)->>'subject_kind')
              AND fp.subject = COALESCE((COALESCE(sensors.config, '{}'::jsonb)->>'subject'), sensors.node_id::text)
              AND fp.metric = (COALESCE(sensors.config, '{}'::jsonb)->>'metric')
              AND fp.ts <= NOW()
              AND (
                (COALESCE(sensors.config, '{}'::jsonb)->>'mode') IS DISTINCT FROM 'asof'
                OR fp.issued_at <= fp.ts
              )
            ORDER BY fp.ts DESC, fp.issued_at DESC
            LIMIT 1
        ) forecast_latest ON true
        WHERE sensors.sensor_id = ANY($1)
          AND sensors.deleted_at IS NULL
        "#,
    )
    .bind(sensor_ids)
    .bind(SENSOR_CONFIG_SOURCE_FORECAST_POINTS)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| (row.sensor_id, (row.latest_value, row.latest_ts)))
        .collect())
}

async fn apply_derived_latest_values(
    db: &sqlx::PgPool,
    sensors: &mut [SensorResponse],
) -> Result<(), sqlx::Error> {
    let mut derived_target_ids: Vec<(usize, String)> = Vec::new();
    let mut derived_specs_by_id: std::collections::HashMap<
        String,
        derived_sensors::DerivedSensorSpec,
    > = std::collections::HashMap::new();

    for (idx, sensor) in sensors.iter().enumerate() {
        let Ok(Some(spec)) = derived_sensors::parse_derived_sensor_spec(&sensor.config) else {
            continue;
        };
        derived_target_ids.push((idx, sensor.sensor_id.clone()));
        derived_specs_by_id.insert(sensor.sensor_id.clone(), spec);
    }

    if derived_target_ids.is_empty() {
        return Ok(());
    }

    // Expand derived-of-derived dependencies by loading any derived inputs that are not in the
    // current response slice (e.g., `/api/sensors/{id}` for a derived-of-derived sensor).
    let mut source_by_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut config_by_id: std::collections::HashMap<String, JsonValue> =
        std::collections::HashMap::new();

    let mut queue: Vec<String> = derived_specs_by_id.keys().cloned().collect();
    let mut expanded: std::collections::HashSet<String> = std::collections::HashSet::new();

    while let Some(derived_id) = queue.pop() {
        if !expanded.insert(derived_id.clone()) {
            continue;
        }
        let Some(spec) = derived_specs_by_id.get(&derived_id).cloned() else {
            continue;
        };

        let mut unknown_inputs: Vec<String> = Vec::new();
        for input in &spec.inputs {
            if !source_by_id.contains_key(&input.sensor_id) {
                unknown_inputs.push(input.sensor_id.clone());
            }
        }
        if !unknown_inputs.is_empty() {
            unknown_inputs.sort();
            unknown_inputs.dedup();

            #[derive(sqlx::FromRow)]
            struct InputSourceRow {
                sensor_id: String,
                source: Option<String>,
                config: SqlJson<JsonValue>,
            }

            let rows: Vec<InputSourceRow> = sqlx::query_as(
                r#"
                SELECT
                  sensor_id,
                  NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source,
                  COALESCE(config, '{}'::jsonb) as config
                FROM sensors
                WHERE sensor_id = ANY($1)
                  AND deleted_at IS NULL
                "#,
            )
            .bind(&unknown_inputs)
            .fetch_all(db)
            .await?;

            for row in rows {
                source_by_id.insert(row.sensor_id.clone(), row.source.unwrap_or_default());
                config_by_id.insert(row.sensor_id, row.config.0);
            }
        }

        for input in &spec.inputs {
            if source_by_id.get(&input.sensor_id).map(|src| src.as_str())
                != Some(SENSOR_CONFIG_SOURCE_DERIVED)
            {
                continue;
            }
            if derived_specs_by_id.contains_key(&input.sensor_id) {
                continue;
            }
            let Some(cfg) = config_by_id.get(&input.sensor_id) else {
                continue;
            };
            let Ok(Some(input_spec)) = derived_sensors::parse_derived_sensor_spec(cfg) else {
                continue;
            };
            derived_specs_by_id.insert(input.sensor_id.clone(), input_spec);
            queue.push(input.sensor_id.clone());
        }
    }

    // Fetch latest values for all non-derived leaf inputs (metrics + forecast_points).
    let mut all_input_ids: Vec<String> = Vec::new();
    for spec in derived_specs_by_id.values() {
        for input in &spec.inputs {
            all_input_ids.push(input.sensor_id.clone());
        }
    }
    all_input_ids.sort();
    all_input_ids.dedup();
    let latest_by_id = fetch_latest_values(db, &all_input_ids).await?;

    // Topologically sort derived specs so derived-of-derived inputs are evaluated first.
    let mut indegree: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut dependents: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for id in derived_specs_by_id.keys() {
        indegree.insert(id.clone(), 0);
    }
    for (id, spec) in &derived_specs_by_id {
        for input in &spec.inputs {
            if derived_specs_by_id.contains_key(&input.sensor_id) {
                *indegree.entry(id.clone()).or_insert(0) += 1;
                dependents
                    .entry(input.sensor_id.clone())
                    .or_default()
                    .push(id.clone());
            }
        }
    }

    let mut ready: std::collections::VecDeque<String> = indegree
        .iter()
        .filter_map(|(id, deg)| if *deg == 0 { Some(id.clone()) } else { None })
        .collect();

    let mut order: Vec<String> = Vec::with_capacity(derived_specs_by_id.len());
    while let Some(id) = ready.pop_front() {
        order.push(id.clone());
        if let Some(list) = dependents.get(&id) {
            for dep in list {
                if let Some(entry) = indegree.get_mut(dep) {
                    *entry = entry.saturating_sub(1);
                    if *entry == 0 {
                        ready.push_back(dep.clone());
                    }
                }
            }
        }
    }

    // Evaluate derived latest values in order.
    let mut computed: std::collections::HashMap<String, (f64, chrono::DateTime<chrono::Utc>)> =
        std::collections::HashMap::new();

    for id in order {
        let Some(spec) = derived_specs_by_id.get(&id) else {
            continue;
        };
        let Ok(mut compiled) = derived_sensors::compile_derived_sensor(spec) else {
            continue;
        };

        let mut vars: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        let mut min_ts: Option<chrono::DateTime<chrono::Utc>> = None;

        for input in &spec.inputs {
            let (value, ts) = if let Some((v, ts)) = computed.get(&input.sensor_id) {
                (*v, *ts)
            } else {
                let Some((value_opt, ts_opt)) = latest_by_id.get(&input.sensor_id) else {
                    vars.clear();
                    min_ts = None;
                    break;
                };
                let Some(value) = value_opt else {
                    vars.clear();
                    min_ts = None;
                    break;
                };
                let Some(ts) = ts_opt else {
                    vars.clear();
                    min_ts = None;
                    break;
                };
                (*value, *ts)
            };

            vars.insert(input.var.clone(), value);
            min_ts = match min_ts {
                None => Some(ts),
                Some(existing) => Some(existing.min(ts)),
            };
        }

        let Some(ts) = min_ts else {
            continue;
        };
        let Ok(value) = compiled.eval_with_vars(&vars) else {
            continue;
        };
        if !value.is_finite() {
            continue;
        }
        computed.insert(id, (value, ts));
    }

    for (idx, sensor_id) in derived_target_ids {
        if let Some((value, ts)) = computed.get(&sensor_id) {
            if let Some(sensor) = sensors.get_mut(idx) {
                sensor.latest_value = Some(*value);
                sensor.latest_ts = Some(ts.to_rfc3339());
            }
        }
    }

    Ok(())
}

#[derive(sqlx::FromRow)]
struct SensorSourceRow {
    sensor_id: String,
    source: Option<String>,
    config: SqlJson<JsonValue>,
}

async fn validate_derived_sensor_config(
    db: &sqlx::PgPool,
    sensor_id: &str,
    config: &JsonValue,
) -> Result<(), (StatusCode, String)> {
    let Some(spec) = derived_sensors::parse_derived_sensor_spec(config)
        .map_err(|msg| (StatusCode::BAD_REQUEST, msg))?
    else {
        return Ok(());
    };

    let input_ids: Vec<String> = spec
        .inputs
        .iter()
        .map(|input| input.sensor_id.clone())
        .collect();
    let rows: Vec<SensorSourceRow> = sqlx::query_as(
        r#"
        SELECT
          sensor_id,
          NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source,
          COALESCE(config, '{}'::jsonb) as config
        FROM sensors
        WHERE sensor_id = ANY($1)
          AND deleted_at IS NULL
        "#,
    )
    .bind(&input_ids)
    .fetch_all(db)
    .await
    .map_err(map_db_error)?;

    let mut known: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut source_by_id: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut config_by_id: std::collections::HashMap<String, JsonValue> =
        std::collections::HashMap::new();
    for row in rows {
        known.insert(row.sensor_id.clone());
        source_by_id.insert(row.sensor_id.clone(), row.source.unwrap_or_default());
        config_by_id.insert(row.sensor_id, row.config.0);
    }

    let missing: Vec<String> = input_ids
        .iter()
        .filter(|id| !known.contains(*id))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Derived sensor inputs not found: {}", missing.join(", ")),
        ));
    }

    // Disallow self-reference by id (possible when the client provides a custom sensor_id).
    let sensor_id = sensor_id.trim();
    if !sensor_id.is_empty() && input_ids.iter().any(|id| id == sensor_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Derived sensors cannot depend on themselves (input: {sensor_id})"),
        ));
    }

    // Validate derived-of-derived graphs: prevent cycles and cap depth.
    struct DerivedValidationWork {
        spec: derived_sensors::DerivedSensorSpec,
        depth: usize,
        path: Vec<String>,
    }

    let mut derived_spec_cache: std::collections::HashMap<
        String,
        derived_sensors::DerivedSensorSpec,
    > = std::collections::HashMap::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut work_queue: Vec<DerivedValidationWork> = Vec::new();

    for input_id in &input_ids {
        if source_by_id.get(input_id).map(|src| src.as_str()) != Some(SENSOR_CONFIG_SOURCE_DERIVED)
        {
            continue;
        }
        let Some(cfg) = config_by_id.get(input_id) else {
            continue;
        };
        let Some(input_spec) = derived_sensors::parse_derived_sensor_spec(cfg)
            .map_err(|msg| (StatusCode::BAD_REQUEST, msg))?
        else {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Derived sensor input {} is marked derived but missing derived config",
                    input_id
                ),
            ));
        };
        derived_spec_cache.insert(input_id.clone(), input_spec.clone());
        if visited.insert(input_id.clone()) {
            work_queue.push(DerivedValidationWork {
                spec: input_spec,
                depth: 1,
                path: vec![sensor_id.to_string(), input_id.clone()],
            });
        }
    }

    while let Some(work) = work_queue.pop() {
        if work.depth > MAX_DERIVED_SENSOR_DEPTH {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Derived sensor chain exceeds max depth of {}",
                    MAX_DERIVED_SENSOR_DEPTH
                ),
            ));
        }

        // Batch-fetch any unknown inputs for this derived sensor.
        let mut unknown: Vec<String> = Vec::new();
        for input in &work.spec.inputs {
            if source_by_id.contains_key(&input.sensor_id) {
                continue;
            }
            unknown.push(input.sensor_id.clone());
        }
        if !unknown.is_empty() {
            unknown.sort();
            unknown.dedup();

            let extra_rows: Vec<SensorSourceRow> = sqlx::query_as(
                r#"
                SELECT
                  sensor_id,
                  NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source,
                  COALESCE(config, '{}'::jsonb) as config
                FROM sensors
                WHERE sensor_id = ANY($1)
                  AND deleted_at IS NULL
                "#,
            )
            .bind(&unknown)
            .fetch_all(db)
            .await
            .map_err(map_db_error)?;

            for row in extra_rows {
                source_by_id.insert(row.sensor_id.clone(), row.source.unwrap_or_default());
                config_by_id.insert(row.sensor_id, row.config.0);
            }

            let missing: Vec<String> = unknown
                .into_iter()
                .filter(|id| !source_by_id.contains_key(id))
                .collect();
            if !missing.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Derived sensor inputs not found: {}", missing.join(", ")),
                ));
            }
        }

        for input in &work.spec.inputs {
            let input_id = input.sensor_id.as_str();
            if input_id == sensor_id {
                let mut cycle = work.path.clone();
                cycle.push(sensor_id.to_string());
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Derived sensor cycle detected: {}", cycle.join(" -> ")),
                ));
            }
            if work.path.iter().any(|entry| entry == input_id) {
                let mut cycle = work.path.clone();
                cycle.push(input_id.to_string());
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!("Derived sensor cycle detected: {}", cycle.join(" -> ")),
                ));
            }

            if source_by_id.get(input_id).map(|src| src.as_str())
                != Some(SENSOR_CONFIG_SOURCE_DERIVED)
            {
                continue;
            }
            if !visited.insert(input_id.to_string()) {
                continue;
            }

            let Some(input_spec) = derived_spec_cache.get(input_id).cloned().or_else(|| {
                config_by_id.get(input_id).and_then(|cfg| {
                    derived_sensors::parse_derived_sensor_spec(cfg)
                        .ok()
                        .flatten()
                })
            }) else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    format!(
                        "Derived sensor input {} is marked derived but missing derived config",
                        input_id
                    ),
                ));
            };
            derived_spec_cache.insert(input_id.to_string(), input_spec.clone());

            let mut path = work.path.clone();
            path.push(input_id.to_string());
            work_queue.push(DerivedValidationWork {
                spec: input_spec,
                depth: work.depth + 1,
                path,
            });
        }
    }

    derived_sensors::compile_derived_sensor(&spec)
        .map(|_| ())
        .map_err(|msg| (StatusCode::BAD_REQUEST, msg))
}

#[utoipa::path(
    get,
    path = "/api/sensors",
    tag = "sensors",
    params(SensorsQuery),
    responses((status = 200, description = "Sensors", body = Vec<SensorResponse>))
)]
pub(crate) async fn list_sensors(
    axum::extract::State(state): axum::extract::State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Query(query): Query<SensorsQuery>,
) -> Result<Json<Vec<SensorResponse>>, (StatusCode, String)> {
    if query.include_hidden {
        let Some(user) = user else {
            return Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()));
        };
        crate::auth::require_capabilities(&user, &["config.write"])
            .map_err(|err| (err.status, err.message))?;
    }

    let mut sensors = fetch_sensors(&state.db, query.node_id, query.include_hidden)
        .await
        .map_err(map_db_error)?;
    apply_derived_latest_values(&state.db, &mut sensors)
        .await
        .map_err(map_db_error)?;
    Ok(Json(sensors))
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct SensorGetQuery {
    #[serde(default)]
    include_hidden: bool,
}

#[utoipa::path(
    get,
    path = "/api/sensors/{sensor_id}",
    tag = "sensors",
    params(("sensor_id" = String, Path, description = "Sensor id"), SensorGetQuery),
    responses(
        (status = 200, description = "Sensor", body = SensorResponse),
        (status = 404, description = "Sensor not found")
    )
)]
pub(crate) async fn get_sensor(
    axum::extract::State(state): axum::extract::State<AppState>,
    OptionalAuthUser(user): OptionalAuthUser,
    Path(sensor_id): Path<String>,
    Query(query): Query<SensorGetQuery>,
) -> Result<Json<SensorResponse>, (StatusCode, String)> {
    let row: Option<SensorRow> = sqlx::query_as(
        r#"
        SELECT
            sensors.sensor_id,
            sensors.node_id,
            sensors.name,
            sensors.type as sensor_type,
            sensors.unit,
            sensors.interval_seconds,
            sensors.rolling_avg_seconds,
            sensors.created_at,
            sensors.deleted_at,
            COALESCE(sensors.config, '{}'::jsonb) as config,
            CASE
              WHEN COALESCE(sensors.config, '{}'::jsonb)->>'source' = 'forecast_points' THEN forecast_latest.value
              ELSE metrics_latest.value
            END as latest_value,
            CASE
              WHEN COALESCE(sensors.config, '{}'::jsonb)->>'source' = 'forecast_points' THEN forecast_latest.ts
              ELSE metrics_latest.ts
            END as latest_ts
        FROM sensors
        LEFT JOIN LATERAL (
            SELECT value, ts
            FROM metrics
            WHERE metrics.sensor_id = sensors.sensor_id
            ORDER BY ts DESC
            LIMIT 1
        ) metrics_latest ON true
        LEFT JOIN LATERAL (
            SELECT fp.value as value, fp.ts as ts
            FROM forecast_points fp
            WHERE fp.provider = (COALESCE(sensors.config, '{}'::jsonb)->>'provider')
              AND fp.kind = (COALESCE(sensors.config, '{}'::jsonb)->>'kind')
              AND fp.subject_kind = (COALESCE(sensors.config, '{}'::jsonb)->>'subject_kind')
              AND fp.subject = COALESCE((COALESCE(sensors.config, '{}'::jsonb)->>'subject'), sensors.node_id::text)
              AND fp.metric = (COALESCE(sensors.config, '{}'::jsonb)->>'metric')
              AND fp.ts <= NOW()
              AND (
                (COALESCE(sensors.config, '{}'::jsonb)->>'mode') IS DISTINCT FROM 'asof'
                OR fp.issued_at <= fp.ts
              )
            ORDER BY fp.ts DESC, fp.issued_at DESC
            LIMIT 1
        ) forecast_latest ON true
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id.trim())
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, "Sensor not found".to_string()));
    };

    if query.include_hidden {
        let Some(user) = user else {
            return Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()));
        };
        crate::auth::require_capabilities(&user, &["config.write"])
            .map_err(|err| (err.status, err.message))?;
    }

    #[derive(sqlx::FromRow)]
    struct NodeConfigOnlyRow {
        config: SqlJson<JsonValue>,
    }

    let node_config = sqlx::query_as::<_, NodeConfigOnlyRow>(
        r#"
        SELECT COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(row.node_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?
    .map(|entry| entry.config)
    .unwrap_or_else(|| SqlJson(serde_json::json!({})));

    let visibility = sensor_visibility::evaluate_sensor_visibility(&row.config.0, &node_config.0);
    if !query.include_hidden && !visibility.visible {
        return Err((StatusCode::NOT_FOUND, "Sensor not found".to_string()));
    }

    let mut sensor = SensorResponse::from(row);
    if query.include_hidden {
        sensor.visibility = Some(visibility);
    }
    if derived_sensors::parse_derived_sensor_spec(&sensor.config)
        .map_err(|msg| (StatusCode::BAD_REQUEST, msg))?
        .is_some()
    {
        apply_derived_latest_values(&state.db, std::slice::from_mut(&mut sensor))
            .await
            .map_err(map_db_error)?;
    }
    Ok(Json(sensor))
}

#[utoipa::path(
    post,
    path = "/api/sensors",
    tag = "sensors",
    request_body = SensorCreateRequest,
    responses(
        (status = 201, description = "Created sensor", body = SensorResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Parent node not found"),
        (status = 409, description = "Sensor id already exists")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_sensor(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<SensorCreateRequest>,
) -> Result<(StatusCode, Json<SensorResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    if payload.name.trim().is_empty()
        || payload.sensor_type.trim().is_empty()
        || payload.unit.trim().is_empty()
        || payload.interval_seconds < 0
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid sensor payload".to_string(),
        ));
    }

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let node: Option<NodeIdentityRow> = sqlx::query_as(
        r#"
        SELECT id, mac_eth::text as mac_eth, mac_wifi::text as mac_wifi, COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(payload.node_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(mut node) = node else {
        return Err((StatusCode::NOT_FOUND, "Parent node not found".to_string()));
    };

    let created_at = chrono::Utc::now();
    let sensor_id = if let Some(id) = payload
        .sensor_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT sensor_id FROM sensors WHERE sensor_id = $1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(map_db_error)?;
        if exists.is_some() {
            return Err((StatusCode::CONFLICT, "Sensor id already exists".to_string()));
        }
        id.to_string()
    } else {
        let mac_eth = node.mac_eth.as_deref();
        let mac_wifi = node.mac_wifi.as_deref();
        let mut allocated: Option<String> = None;
        for counter in 0..2048u32 {
            let candidate =
                ids::deterministic_hex_id("sensor", mac_eth, mac_wifi, created_at, counter);
            let exists: Option<(String,)> =
                sqlx::query_as("SELECT sensor_id FROM sensors WHERE sensor_id = $1")
                    .bind(&candidate)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(map_db_error)?;
            if exists.is_none() {
                allocated = Some(candidate);
                break;
            }
        }
        allocated.ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to allocate unique sensor id".to_string(),
        ))?
    };

    let deleted_at = parse_rfc3339(payload.deleted_at.as_deref(), "deleted_at")?;
    let config = payload.config.unwrap_or_else(|| serde_json::json!({}));

    if !config.is_object() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Sensor config must be an object".to_string(),
        ));
    }

    validate_derived_sensor_config(&state.db, &sensor_id, &config).await?;

    let row: SensorRow = sqlx::query_as(
        r#"
	        INSERT INTO sensors (
	            sensor_id,
	            node_id,
	            name,
	            type,
	            unit,
	            interval_seconds,
	            rolling_avg_seconds,
	            deleted_at,
	            config,
	            ui_order
	        )
	        VALUES (
	            $1,
	            $2,
	            $3,
	            $4,
	            $5,
	            $6,
	            $7,
	            $8,
	            $9,
	            (SELECT COALESCE(MAX(ui_order), 0) + 1 FROM sensors WHERE node_id = $2 AND deleted_at IS NULL)
	        )
	        RETURNING
	            sensor_id,
	            node_id,
	            name,
            type as sensor_type,
            unit,
            interval_seconds,
            rolling_avg_seconds,
            created_at,
            deleted_at,
            COALESCE(config, '{}'::jsonb) as config,
            NULL::double precision as latest_value,
            NULL::timestamptz as latest_ts
        "#,
    )
    .bind(&sensor_id)
    .bind(payload.node_id)
    .bind(payload.name.trim())
    .bind(payload.sensor_type.trim())
    .bind(payload.unit.trim())
    .bind(payload.interval_seconds)
    .bind(payload.rolling_avg_seconds.max(0))
    .bind(deleted_at)
    .bind(SqlJson(config))
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let sensor = SensorResponse::from(row);
    update_sensor_manifest(&mut node.config.0, &sensor);
    let _ = sqlx::query("UPDATE nodes SET config = $2 WHERE id = $1")
        .bind(node.id)
        .bind(SqlJson(node.config.0))
        .execute(&mut *tx)
        .await
        .map_err(map_db_error)?;

    tx.commit().await.map_err(map_db_error)?;
    Ok((StatusCode::CREATED, Json(sensor)))
}

#[utoipa::path(
    put,
    path = "/api/sensors/{sensor_id}",
    tag = "sensors",
    request_body = SensorUpdateRequest,
    params(("sensor_id" = String, Path, description = "Sensor id")),
    responses(
        (status = 200, description = "Updated sensor", body = SensorResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Sensor not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_sensor(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(sensor_id): Path<String>,
    Json(payload): Json<SensorUpdateRequest>,
) -> Result<Json<SensorResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let existing: Option<SensorRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            node_id,
            name,
            type as sensor_type,
            unit,
            interval_seconds,
            rolling_avg_seconds,
            created_at,
            deleted_at,
            COALESCE(config, '{}'::jsonb) as config,
            NULL::double precision as latest_value,
            NULL::timestamptz as latest_ts
        FROM sensors
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(mut existing) = existing else {
        return Err((StatusCode::NOT_FOUND, "Sensor not found".to_string()));
    };

    let existing_source = existing
        .config
        .0
        .get("source")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let has_metrics: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(SELECT 1 FROM metrics WHERE sensor_id = $1 LIMIT 1)
        "#,
    )
    .bind(existing.sensor_id.trim())
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let mut has_forecast_history = false;
    if existing_source == SENSOR_CONFIG_SOURCE_FORECAST_POINTS {
        let provider = existing
            .config
            .0
            .get("provider")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        let kind = existing
            .config
            .0
            .get("kind")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        let subject_kind = existing
            .config
            .0
            .get("subject_kind")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        let metric = existing
            .config
            .0
            .get("metric")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .trim();
        let subject = existing
            .config
            .0
            .get("subject")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| existing.node_id.to_string());

        if !provider.is_empty()
            && !kind.is_empty()
            && !subject_kind.is_empty()
            && !metric.is_empty()
            && !subject.is_empty()
        {
            has_forecast_history = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                  SELECT 1
                  FROM forecast_points
                  WHERE provider = $1
                    AND kind = $2
                    AND subject_kind = $3
                    AND subject = $4
                    AND metric = $5
                  LIMIT 1
                )
                "#,
            )
            .bind(provider)
            .bind(kind)
            .bind(subject_kind)
            .bind(&subject)
            .bind(metric)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_db_error)?;
        }
    }

    let has_history = has_metrics || has_forecast_history;

    if let Some(name) = payload.name {
        if !name.trim().is_empty() {
            existing.name = name.trim().to_string();
        }
    }
    if let Some(sensor_type) = payload.sensor_type {
        let trimmed = sensor_type.trim();
        if !trimmed.is_empty() {
            if has_history && trimmed != existing.sensor_type.trim() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Sensor type cannot be changed after data exists (create a new sensor instead)"
                        .to_string(),
                ));
            }
            existing.sensor_type = trimmed.to_string();
        }
    }
    if let Some(unit) = payload.unit {
        let trimmed = unit.trim();
        if !trimmed.is_empty() {
            if has_history && trimmed != existing.unit.trim() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Sensor unit cannot be changed after data exists (create a new sensor instead)"
                        .to_string(),
                ));
            }
            existing.unit = trimmed.to_string();
        }
    }
    if let Some(interval_seconds) = payload.interval_seconds {
        if interval_seconds >= 0 {
            existing.interval_seconds = interval_seconds;
        }
    }
    if let Some(rolling_avg_seconds) = payload.rolling_avg_seconds {
        existing.rolling_avg_seconds = rolling_avg_seconds.max(0);
    }

    if let Some(deleted_at_raw) = payload.deleted_at {
        existing.deleted_at = parse_rfc3339(Some(&deleted_at_raw), "deleted_at")?;
    }

    if let Some(mut config) = payload.config {
        if !config.is_object() {
            return Err((
                StatusCode::BAD_REQUEST,
                "Sensor config must be an object".to_string(),
            ));
        }
        let Some(obj) = config.as_object_mut() else {
            return Err((
                StatusCode::BAD_REQUEST,
                "Sensor config must be an object".to_string(),
            ));
        };

        if !obj.contains_key("source") && !existing_source.is_empty() {
            obj.insert(
                "source".to_string(),
                serde_json::Value::String(existing_source.clone()),
            );
        }

        let requested_source = if let Some(value) = obj.get("source") {
            if let Some(source) = value.as_str() {
                source.trim().to_string()
            } else if value.is_null() {
                "".to_string()
            } else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Sensor config source must be a string".to_string(),
                ));
            }
        } else {
            "".to_string()
        };

        if requested_source != existing_source {
            if has_history {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Sensor source cannot be changed after data exists (create a new sensor instead)"
                        .to_string(),
                ));
            }
            if !existing_source.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Sensor source cannot be changed after creation (create a new sensor instead)"
                        .to_string(),
                ));
            }
            if requested_source == SENSOR_CONFIG_SOURCE_DERIVED
                || requested_source == SENSOR_CONFIG_SOURCE_FORECAST_POINTS
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Cannot convert an existing sensor into a derived/forecast sensor (create a new sensor instead)"
                        .to_string(),
                ));
            }
        }

        existing.config = SqlJson(config);
    }

    validate_derived_sensor_config(&state.db, &existing.sensor_id, &existing.config.0).await?;

    let row: SensorRow = sqlx::query_as(
        r#"
        UPDATE sensors
        SET name = $2,
            type = $3,
            unit = $4,
            interval_seconds = $5,
            rolling_avg_seconds = $6,
            deleted_at = $7,
            config = $8
        WHERE sensor_id = $1
        RETURNING
            sensor_id,
            node_id,
            name,
            type as sensor_type,
            unit,
            interval_seconds,
            rolling_avg_seconds,
            created_at,
            deleted_at,
            COALESCE(config, '{}'::jsonb) as config,
            NULL::double precision as latest_value,
            NULL::timestamptz as latest_ts
        "#,
    )
    .bind(existing.sensor_id.trim())
    .bind(existing.name.trim())
    .bind(existing.sensor_type.trim())
    .bind(existing.unit.trim())
    .bind(existing.interval_seconds)
    .bind(existing.rolling_avg_seconds)
    .bind(existing.deleted_at)
    .bind(existing.config)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let sensor = SensorResponse::from(row);
    let node: Option<NodeIdentityRow> = sqlx::query_as(
        r#"
        SELECT id, mac_eth::text as mac_eth, mac_wifi::text as mac_wifi, COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(Uuid::parse_str(sensor.node_id.trim()).ok())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    if let Some(mut node) = node {
        update_sensor_manifest(&mut node.config.0, &sensor);
        let _ = sqlx::query("UPDATE nodes SET config = $2 WHERE id = $1")
            .bind(node.id)
            .bind(SqlJson(node.config.0))
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
    }

    tx.commit().await.map_err(map_db_error)?;
    Ok(Json(sensor))
}

#[utoipa::path(
    delete,
    path = "/api/sensors/{sensor_id}",
    tag = "sensors",
    params(("sensor_id" = String, Path, description = "Sensor id"), SensorDeleteQuery),
    responses(
        (status = 204, description = "Deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Sensor not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_sensor(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(sensor_id): Path<String>,
    Query(query): Query<SensorDeleteQuery>,
) -> Result<StatusCode, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let mut tx = state.db.begin().await.map_err(map_db_error)?;

    let existing: Option<SensorRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            node_id,
            name,
            type as sensor_type,
            unit,
            interval_seconds,
            rolling_avg_seconds,
            created_at,
            deleted_at,
            COALESCE(config, '{}'::jsonb) as config,
            NULL::double precision as latest_value,
            NULL::timestamptz as latest_ts
        FROM sensors
        WHERE sensor_id = $1
        "#,
    )
    .bind(sensor_id.trim())
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    let Some(existing) = existing else {
        return Err((StatusCode::NOT_FOUND, "Sensor not found".to_string()));
    };

    let node: Option<NodeIdentityRow> = sqlx::query_as(
        r#"
        SELECT id, mac_eth::text as mac_eth, mac_wifi::text as mac_wifi, COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(existing.node_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_db_error)?;

    if !query.keep_data {
        crate::auth::require_capabilities(&user, &["ops.purge"])
            .map_err(|err| (err.status, err.message))?;

        let _ = sqlx::query("DELETE FROM alarm_events WHERE sensor_id = $1")
            .bind(existing.sensor_id.trim())
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        let _ = sqlx::query("DELETE FROM alarms WHERE sensor_id = $1")
            .bind(existing.sensor_id.trim())
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
        let _ = sqlx::query("DELETE FROM sensors WHERE sensor_id = $1")
            .bind(existing.sensor_id.trim())
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;

        if let Some(mut node) = node {
            remove_sensor_manifest(&mut node.config.0, existing.sensor_id.trim());
            let _ = sqlx::query("UPDATE nodes SET config = $2 WHERE id = $1")
                .bind(node.id)
                .bind(SqlJson(node.config.0))
                .execute(&mut *tx)
                .await
                .map_err(map_db_error)?;
        }

        tx.commit().await.map_err(map_db_error)?;
        return Ok(StatusCode::NO_CONTENT);
    }

    let deleted_at = chrono::Utc::now();
    let deleted_stamp = deleted_at.format("%Y%m%dT%H%M%SZ").to_string();
    let name = if existing.name.contains("-deleted") {
        existing.name
    } else {
        format!("{}-deleted-{}", existing.name, deleted_stamp)
    };

    let row: SensorRow = sqlx::query_as(
        r#"
        UPDATE sensors
        SET name = $2,
            deleted_at = $3
        WHERE sensor_id = $1
        RETURNING
            sensor_id,
            node_id,
            name,
            type as sensor_type,
            unit,
            interval_seconds,
            rolling_avg_seconds,
            created_at,
            deleted_at,
            COALESCE(config, '{}'::jsonb) as config,
            NULL::double precision as latest_value,
            NULL::timestamptz as latest_ts
        "#,
    )
    .bind(existing.sensor_id.trim())
    .bind(name.trim())
    .bind(deleted_at)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_db_error)?;

    if let Some(mut node) = node {
        let sensor = SensorResponse::from(row);
        update_sensor_manifest(&mut node.config.0, &sensor);
        let _ = sqlx::query("UPDATE nodes SET config = $2 WHERE id = $1")
            .bind(node.id)
            .bind(SqlJson(node.config.0))
            .execute(&mut *tx)
            .await
            .map_err(map_db_error)?;
    }

    tx.commit().await.map_err(map_db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sensors", get(list_sensors).post(create_sensor))
        .route(
            "/sensors/{sensor_id}",
            get(get_sensor).put(update_sensor).delete(delete_sensor),
        )
}
