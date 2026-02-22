//! Unified bucket reader for analysis jobs.
//!
//! This module provides `read_bucket_series_for_sensors()` which handles both raw sensors
//! (read directly from the analysis lake) and derived sensors (computed on-the-fly from
//! their raw inputs).
//!
//! # Derived Sensor Support
//!
//! Derived sensors are computed at query time by:
//! 1. Identifying all transitive raw inputs (with cycle detection)
//! 2. Reading all raw inputs from the lake in a single batch
//! 3. Computing derived values for each epoch where all inputs have data
//!
//! # Unsupported Sources
//!
//! Forecast sensors (`source: "forecast_points"`) are not stored in the analysis lake, so they
//! are skipped as direct outputs by this reader. However, derived sensors *may* depend on
//! forecast sensors; in that case, we query `forecast_points` to populate the forecast input
//! buckets needed to evaluate the derived expression.

use crate::services::analysis::jobs::AnalysisJobError;
use crate::services::analysis::lake::AnalysisLakeConfig;
use crate::services::analysis::parquet_duckdb::{
    BucketAggregationMode, DuckDbQueryService, MetricsBucketReadOptions, MetricsBucketRow,
};
use crate::services::analysis::signal_semantics;
use crate::services::derived_sensors::{
    compile_derived_sensor, parse_derived_sensor_spec, DerivedSensorCompiled, DerivedSensorSpec,
};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

/// Maximum recursion depth for derived sensor input expansion.
const MAX_DERIVED_DEPTH: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketAggregationPreference {
    Auto,
    Avg,
    Last,
    Sum,
    Min,
    Max,
}

impl BucketAggregationPreference {
    fn explicit_mode(self) -> Option<BucketAggregationMode> {
        match self {
            Self::Auto => None,
            Self::Avg => Some(BucketAggregationMode::Avg),
            Self::Last => Some(BucketAggregationMode::Last),
            Self::Sum => Some(BucketAggregationMode::Sum),
            Self::Min => Some(BucketAggregationMode::Min),
            Self::Max => Some(BucketAggregationMode::Max),
        }
    }
}

/// Error types for bucket reading operations.
#[derive(Debug, Clone)]
pub enum BucketReaderError {
    /// Sensor uses an unsupported source type (e.g., forecast_points).
    UnsupportedSensorSource {
        sensor_ids: Vec<String>,
        source: String,
    },
    /// Derived sensor has a circular dependency.
    DerivedCycleDetected {
        sensor_id: String,
        cycle: Vec<String>,
    },
    /// Derived sensor chain exceeds maximum depth.
    DerivedDepthExceeded { sensor_id: String, depth: usize },
    /// Failed to compile derived sensor expression.
    DerivedCompileFailed { sensor_id: String, error: String },
    /// Lake read operation failed.
    LakeReadFailed(String),
    /// Database query failed.
    DatabaseError(String),
}

impl std::fmt::Display for BucketReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedSensorSource { sensor_ids, source } => {
                write!(
                    f,
                    "{}-backed sensors are not supported in analysis jobs: {}",
                    source,
                    sensor_ids.join(", ")
                )
            }
            Self::DerivedCycleDetected { sensor_id, cycle } => {
                write!(
                    f,
                    "Cycle detected in derived sensor {}: {}",
                    sensor_id,
                    cycle.join(" -> ")
                )
            }
            Self::DerivedDepthExceeded { sensor_id, depth } => {
                write!(
                    f,
                    "Derived sensor {} exceeds maximum depth of {} levels",
                    sensor_id, depth
                )
            }
            Self::DerivedCompileFailed { sensor_id, error } => {
                write!(
                    f,
                    "Failed to compile derived sensor {}: {}",
                    sensor_id, error
                )
            }
            Self::LakeReadFailed(msg) => write!(f, "Lake read failed: {}", msg),
            Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for BucketReaderError {}

impl BucketReaderError {
    /// Convert to an AnalysisJobError with appropriate error code and details.
    pub fn to_job_error(&self) -> AnalysisJobError {
        match self {
            Self::UnsupportedSensorSource { sensor_ids, source } => AnalysisJobError {
                code: "unsupported_sensor_source".to_string(),
                message: format!(
                    "{}-backed sensors are not supported in analysis jobs: {}",
                    source,
                    sensor_ids.join(", ")
                ),
                details: Some(serde_json::json!({
                    "sensor_ids": sensor_ids,
                    "source": source
                })),
            },
            Self::DerivedCycleDetected { sensor_id, cycle } => AnalysisJobError {
                code: "derived_cycle_detected".to_string(),
                message: format!("Cycle detected in derived sensor {}", sensor_id),
                details: Some(serde_json::json!({
                    "sensor_id": sensor_id,
                    "cycle": cycle
                })),
            },
            Self::DerivedDepthExceeded { sensor_id, depth } => AnalysisJobError {
                code: "derived_depth_exceeded".to_string(),
                message: format!(
                    "Derived sensor {} exceeds maximum depth of {} levels",
                    sensor_id, depth
                ),
                details: Some(serde_json::json!({
                    "sensor_id": sensor_id,
                    "depth": depth
                })),
            },
            Self::DerivedCompileFailed { sensor_id, error } => AnalysisJobError {
                code: "derived_compile_failed".to_string(),
                message: format!("Failed to compile derived sensor {}: {}", sensor_id, error),
                details: Some(serde_json::json!({
                    "sensor_id": sensor_id,
                    "error": error
                })),
            },
            Self::LakeReadFailed(msg) => AnalysisJobError {
                code: "bucket_read_failed".to_string(),
                message: format!("Lake read failed: {}", msg),
                details: None,
            },
            Self::DatabaseError(msg) => AnalysisJobError {
                code: "bucket_read_failed".to_string(),
                message: format!("Database error: {}", msg),
                details: None,
            },
        }
    }
}

/// Sensor source classification.
#[derive(Debug, Clone)]
pub enum SensorSource {
    /// Raw sensor - data exists directly in the analysis lake.
    Raw,
    /// Derived sensor - computed from other sensors.
    Derived {
        spec: DerivedSensorSpec,
        compiled: DerivedSensorCompiled,
    },
    /// Forecast sensor - not stored in the analysis lake.
    Forecast {
        node_id: Uuid,
        config: serde_json::Value,
    },
}

/// Row from database with sensor config.
#[derive(sqlx::FromRow)]
struct SensorConfigRow {
    sensor_id: String,
    node_id: Uuid,
    config: Option<sqlx::types::Json<serde_json::Value>>,
}

#[derive(Debug, Clone)]
struct ForecastInput {
    node_id: Uuid,
    config: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct SensorTypeRow {
    sensor_id: String,
    sensor_type: String,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct ExpandedInputs {
    raw_inputs: HashSet<String>,
    forecast_inputs: HashMap<String, ForecastInput>,
}

/// Read bucketed time series for a list of sensors, automatically handling derived sensors.
///
/// This function routes sensors based on their source:
/// - **Raw sensors**: Read directly from the `metrics/v1` lake
/// - **Derived sensors**: Compute on-the-fly from their raw inputs
/// - **Forecast sensors**: Skipped as direct outputs (no lake data); may be used as derived inputs via `forecast_points`.
///
/// # Arguments
///
/// * `db` - Database connection pool
/// * `duckdb` - DuckDB query service
/// * `lake` - Analysis lake configuration
/// * `sensor_ids` - List of sensor IDs to read
/// * `start` - Start of time range (inclusive)
/// * `end` - End of time range (exclusive)
/// * `interval_seconds` - Bucket interval in seconds
///
/// # Returns
///
/// Combined bucket rows for all sensors (raw + computed derived).
/// Forecast sensors are silently filtered out as direct outputs.
pub async fn read_bucket_series_for_sensors(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    sensor_ids: Vec<String>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval_seconds: i64,
) -> Result<Vec<MetricsBucketRow>, BucketReaderError> {
    read_bucket_series_for_sensors_with_aggregation(
        db,
        duckdb,
        lake,
        sensor_ids,
        start,
        end,
        interval_seconds,
        BucketAggregationPreference::Avg,
    )
    .await
}

pub async fn read_bucket_series_for_sensors_with_aggregation(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    sensor_ids: Vec<String>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval_seconds: i64,
    aggregation_preference: BucketAggregationPreference,
) -> Result<Vec<MetricsBucketRow>, BucketReaderError> {
    read_bucket_series_for_sensors_with_aggregation_and_options(
        db,
        duckdb,
        lake,
        sensor_ids,
        start,
        end,
        interval_seconds,
        aggregation_preference,
        MetricsBucketReadOptions::default(),
    )
    .await
}

pub async fn read_bucket_series_for_sensors_with_aggregation_and_options(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    sensor_ids: Vec<String>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval_seconds: i64,
    aggregation_preference: BucketAggregationPreference,
    bucket_options: MetricsBucketReadOptions,
) -> Result<Vec<MetricsBucketRow>, BucketReaderError> {
    if sensor_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Step 1: Classify all requested sensors by source
    let sources = classify_sensors(db, &sensor_ids).await?;

    // Step 2: Filter out forecast sensors (they have no data in the analysis lake)
    // We silently skip them rather than erroring to allow mixed sensor lists
    let forecast_ids: Vec<String> = sensor_ids
        .iter()
        .filter(|id| matches!(sources.get(*id), Some(SensorSource::Forecast { .. })))
        .cloned()
        .collect();
    if !forecast_ids.is_empty() {
        tracing::debug!(
            skipped_forecast_sensors = forecast_ids.len(),
            "Skipping forecast sensors in analysis job (no lake data)"
        );
    }

    // Step 3: Separate raw and derived sensors (excluding forecast)
    let mut raw_output_ids: Vec<String> = Vec::new();
    let mut derived_output_ids: Vec<String> = Vec::new();

    // We'll expand this cache as we discover transitive inputs.
    let mut sources_cache = sources.clone();

    // Collect all raw sensor IDs we need to read from the lake, plus any forecast inputs.
    let mut all_raw_ids: HashSet<String> = HashSet::new();
    let mut all_forecast_inputs: HashMap<String, ForecastInput> = HashMap::new();

    // Collect all derived sensors we need to evaluate (requested outputs + transitive derived inputs).
    let mut derived_required: HashMap<String, (DerivedSensorSpec, DerivedSensorCompiled)> =
        HashMap::new();
    let mut visited_derived: HashSet<String> = HashSet::new();
    let mut work_queue: Vec<ExpansionWork> = Vec::new();

    for sensor_id in &sensor_ids {
        match sources.get(sensor_id) {
            Some(SensorSource::Raw) | None => {
                raw_output_ids.push(sensor_id.clone());
                all_raw_ids.insert(sensor_id.clone());
            }
            Some(SensorSource::Derived { spec, compiled }) => {
                derived_output_ids.push(sensor_id.clone());
                derived_required.insert(sensor_id.clone(), (spec.clone(), compiled.clone()));
                if visited_derived.insert(sensor_id.clone()) {
                    work_queue.push(ExpansionWork {
                        sensor_id: sensor_id.clone(),
                        spec: spec.clone(),
                        depth: 0,
                        path: vec![sensor_id.clone()],
                    });
                }
            }
            Some(SensorSource::Forecast { .. }) => {
                // Already handled above
            }
        }
    }

    // Step 4: Expand derived dependencies to find all transitive inputs needed (raw + forecast + derived).
    while let Some(work) = work_queue.pop() {
        if work.depth > MAX_DERIVED_DEPTH {
            return Err(BucketReaderError::DerivedDepthExceeded {
                sensor_id: work.sensor_id.clone(),
                depth: work.depth,
            });
        }

        for input in &work.spec.inputs {
            let input_id = &input.sensor_id;

            // Check for cycles on the active path (shared dependencies are allowed).
            if work.path.contains(input_id) {
                let mut cycle = work.path.clone();
                cycle.push(input_id.clone());
                return Err(BucketReaderError::DerivedCycleDetected {
                    sensor_id: work
                        .path
                        .first()
                        .cloned()
                        .unwrap_or_else(|| work.sensor_id.clone()),
                    cycle,
                });
            }

            // Get or fetch source classification for this input.
            let input_source = if let Some(src) = sources_cache.get(input_id) {
                src.clone()
            } else {
                let additional = classify_sensors(db, &[input_id.clone()]).await?;
                let src = additional
                    .get(input_id)
                    .cloned()
                    .unwrap_or(SensorSource::Raw);
                sources_cache.insert(input_id.clone(), src.clone());
                src
            };

            match input_source {
                SensorSource::Raw => {
                    all_raw_ids.insert(input_id.clone());
                }
                SensorSource::Forecast { node_id, config } => {
                    all_forecast_inputs
                        .entry(input_id.clone())
                        .or_insert(ForecastInput { node_id, config });
                }
                SensorSource::Derived {
                    spec: input_spec,
                    compiled: input_compiled,
                } => {
                    derived_required
                        .entry(input_id.clone())
                        .or_insert((input_spec.clone(), input_compiled));
                    if visited_derived.insert(input_id.clone()) {
                        let mut new_path = work.path.clone();
                        new_path.push(input_id.clone());
                        work_queue.push(ExpansionWork {
                            sensor_id: input_id.clone(),
                            spec: input_spec,
                            depth: work.depth + 1,
                            path: new_path,
                        });
                    }
                }
            }
        }
    }
    if !all_forecast_inputs.is_empty() {
        tracing::debug!(
            derived_forecast_inputs = all_forecast_inputs.len(),
            "Derived sensor evaluation will query forecast_points for inputs"
        );
    }

    // Step 4b: Expand the input read window to cover any lagged lookups.
    //
    // lag_seconds uses the convention: positive means "use past values" (i.e., at output epoch
    // we look up the input at epoch - lag_seconds). Negative means "use future values".
    //
    // For derived-of-derived graphs, the effective lookback/lookahead is the *maximum transitive
    // time offset* from the output epoch to any leaf input bucket (lag sums along derived edges).
    let interval_seconds = interval_seconds.max(1);
    let bucket_slop = chrono::Duration::seconds(interval_seconds);

    let derived_order = topo_sort_derived_ids(&derived_required)?;
    let (max_lookback_seconds, max_lookahead_seconds) =
        compute_transitive_lag_window(&derived_required, &derived_output_ids, &derived_order);

    // When lags are not exact multiples of the request interval, derived evaluation will floor
    // the requested input lookup time to a bucket boundary (see `compute_derived_buckets`). To
    // ensure we have that earlier bucket available, expand the input read window by one interval
    // on both sides.
    let input_start = start - chrono::Duration::seconds(max_lookback_seconds) - bucket_slop;
    let input_end = end + chrono::Duration::seconds(max_lookahead_seconds) + bucket_slop;

    // Step 5: Read all raw inputs from lake in a single batch
    let all_raw_ids_vec: Vec<String> = all_raw_ids.into_iter().collect();
    let raw_rows = if all_raw_ids_vec.is_empty() {
        Vec::new()
    } else if let Some(mode) = aggregation_preference.explicit_mode() {
        duckdb
            .read_metrics_buckets_from_lake_with_mode_and_options(
                lake,
                input_start,
                input_end,
                all_raw_ids_vec,
                interval_seconds,
                mode,
                bucket_options,
            )
            .await
            .map_err(|err| BucketReaderError::LakeReadFailed(err.to_string()))?
    } else {
        read_raw_rows_with_auto_aggregation(
            db,
            duckdb,
            lake,
            input_start,
            input_end,
            all_raw_ids_vec,
            interval_seconds,
            bucket_options,
        )
        .await?
    };

    // Step 6: Group raw rows by epoch for efficient lookup
    let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();
    for row in &raw_rows {
        let epoch = row.bucket.timestamp();
        rows_by_epoch
            .entry(epoch)
            .or_default()
            .insert(row.sensor_id.clone(), row.value);
    }

    // Step 6b: Fetch forecast buckets for any forecast inputs required by derived sensors
    if !all_forecast_inputs.is_empty() {
        for (sensor_id, forecast) in &all_forecast_inputs {
            let Some(spec) = parse_forecast_query_spec(forecast.node_id, &forecast.config) else {
                continue;
            };
            let forecast_rows =
                read_forecast_bucket_values(db, &spec, input_start, input_end, interval_seconds)
                    .await?;
            for (bucket, value) in forecast_rows {
                if !value.is_finite() {
                    continue;
                }
                let epoch = bucket.timestamp();
                rows_by_epoch
                    .entry(epoch)
                    .or_default()
                    .insert(sensor_id.clone(), value);
            }
        }
    }

    // Step 7: Compute derived sensor values (including transitive derived dependencies).
    let raw_output_set: HashSet<String> = raw_output_ids.iter().cloned().collect();
    let derived_output_set: HashSet<String> = derived_output_ids.iter().cloned().collect();

    let mut computed_derived_rows: HashMap<String, Vec<MetricsBucketRow>> = HashMap::new();
    for derived_id in &derived_order {
        let Some((spec, compiled_template)) = derived_required.get(derived_id) else {
            continue;
        };
        let mut compiled = compiled_template.clone();
        let derived_rows = compute_derived_buckets(
            derived_id,
            spec,
            &mut compiled,
            &rows_by_epoch,
            input_start.timestamp(),
            input_end.timestamp(),
            interval_seconds,
        );

        // Insert computed derived values into the epoch map so dependents can use them.
        for row in &derived_rows {
            let epoch = row.bucket.timestamp();
            rows_by_epoch
                .entry(epoch)
                .or_default()
                .insert(derived_id.clone(), row.value);
        }

        computed_derived_rows.insert(derived_id.clone(), derived_rows);
    }

    let mut result: Vec<MetricsBucketRow> = raw_rows
        .into_iter()
        .filter(|r| raw_output_set.contains(&r.sensor_id) && r.bucket >= start && r.bucket < end)
        .collect();

    for derived_id in derived_output_ids {
        if !derived_output_set.contains(&derived_id) {
            continue;
        }
        if let Some(rows) = computed_derived_rows.get(&derived_id) {
            result.extend(
                rows.iter()
                    .filter(|r| r.bucket >= start && r.bucket < end)
                    .cloned(),
            );
        }
    }

    // Sort by sensor_id then bucket for consistent ordering
    result.sort_by(|a, b| {
        a.sensor_id
            .cmp(&b.sensor_id)
            .then_with(|| a.bucket.cmp(&b.bucket))
    });

    Ok(result)
}

async fn read_raw_rows_with_auto_aggregation(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    input_start: DateTime<Utc>,
    input_end: DateTime<Utc>,
    sensor_ids: Vec<String>,
    interval_seconds: i64,
    bucket_options: MetricsBucketReadOptions,
) -> Result<Vec<MetricsBucketRow>, BucketReaderError> {
    let sensor_type_rows: Vec<SensorTypeRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, type as sensor_type
        FROM sensors
        WHERE sensor_id = ANY($1) AND deleted_at IS NULL
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(db)
    .await
    .map_err(|err| BucketReaderError::DatabaseError(err.to_string()))?;

    let mut type_by_sensor_id: HashMap<String, String> = HashMap::new();
    for row in sensor_type_rows {
        type_by_sensor_id.insert(row.sensor_id, row.sensor_type);
    }

    let mut grouped: HashMap<BucketAggregationMode, Vec<String>> = HashMap::new();
    for sensor_id in sensor_ids {
        let mode = auto_aggregation_mode_for_sensor_type(
            type_by_sensor_id.get(&sensor_id).map(String::as_str),
        );
        grouped.entry(mode).or_default().push(sensor_id);
    }

    let order = [
        BucketAggregationMode::Avg,
        BucketAggregationMode::Last,
        BucketAggregationMode::Sum,
        BucketAggregationMode::Min,
        BucketAggregationMode::Max,
    ];

    let mut out = Vec::new();
    for mode in order {
        let Some(group_ids) = grouped.remove(&mode) else {
            continue;
        };
        if group_ids.is_empty() {
            continue;
        }
        let mut rows = duckdb
            .read_metrics_buckets_from_lake_with_mode_and_options(
                lake,
                input_start,
                input_end,
                group_ids,
                interval_seconds,
                mode,
                bucket_options,
            )
            .await
            .map_err(|err| BucketReaderError::LakeReadFailed(err.to_string()))?;
        out.append(&mut rows);
    }
    Ok(out)
}

fn auto_aggregation_mode_for_sensor_type(sensor_type: Option<&str>) -> BucketAggregationMode {
    signal_semantics::auto_bucket_mode(sensor_type)
}

/// Classify sensors by their source type.
async fn classify_sensors(
    db: &PgPool,
    sensor_ids: &[String],
) -> Result<HashMap<String, SensorSource>, BucketReaderError> {
    if sensor_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<SensorConfigRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, node_id, config
        FROM sensors
        WHERE sensor_id = ANY($1) AND deleted_at IS NULL
        "#,
    )
    .bind(sensor_ids)
    .fetch_all(db)
    .await
    .map_err(|err| BucketReaderError::DatabaseError(err.to_string()))?;

    let mut result: HashMap<String, SensorSource> = HashMap::new();

    for row in rows {
        let config = row
            .config
            .map(|c| c.0)
            .unwrap_or_else(|| serde_json::json!({}));
        let source = config
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let sensor_source = match source {
            "derived" => {
                let spec = parse_derived_sensor_spec(&config)
                    .map_err(|err| BucketReaderError::DerivedCompileFailed {
                        sensor_id: row.sensor_id.clone(),
                        error: err,
                    })?
                    .ok_or_else(|| BucketReaderError::DerivedCompileFailed {
                        sensor_id: row.sensor_id.clone(),
                        error: "Missing derived config".to_string(),
                    })?;

                let compiled = compile_derived_sensor(&spec).map_err(|err| {
                    BucketReaderError::DerivedCompileFailed {
                        sensor_id: row.sensor_id.clone(),
                        error: err,
                    }
                })?;

                SensorSource::Derived { spec, compiled }
            }
            "forecast_points" => SensorSource::Forecast {
                node_id: row.node_id,
                config,
            },
            _ => SensorSource::Raw,
        };

        result.insert(row.sensor_id, sensor_source);
    }

    Ok(result)
}

fn topo_sort_derived_ids(
    derived: &HashMap<String, (DerivedSensorSpec, DerivedSensorCompiled)>,
) -> Result<Vec<String>, BucketReaderError> {
    if derived.is_empty() {
        return Ok(Vec::new());
    }

    let mut indegree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    for id in derived.keys() {
        indegree.insert(id.clone(), 0);
    }

    for (id, (spec, _)) in derived {
        for input in &spec.inputs {
            if derived.contains_key(&input.sensor_id) {
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

    let mut order: Vec<String> = Vec::with_capacity(derived.len());
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

    if order.len() != derived.len() {
        let unresolved: Vec<String> = indegree
            .into_iter()
            .filter_map(|(id, deg)| if deg > 0 { Some(id) } else { None })
            .collect();
        let sensor_id = unresolved
            .first()
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        return Err(BucketReaderError::DerivedCycleDetected {
            sensor_id,
            cycle: unresolved,
        });
    }

    Ok(order)
}

fn compute_transitive_lag_window(
    derived: &HashMap<String, (DerivedSensorSpec, DerivedSensorCompiled)>,
    derived_output_ids: &[String],
    derived_order: &[String],
) -> (i64, i64) {
    if derived.is_empty() || derived_output_ids.is_empty() {
        return (0, 0);
    }

    // For each derived sensor, compute the min/max leaf offsets (seconds) needed relative to the
    // derived sensor's output epoch. Negative = past, positive = future.
    let mut offset_by_id: HashMap<String, (i64, i64)> = HashMap::new();

    for derived_id in derived_order {
        let Some((spec, _)) = derived.get(derived_id) else {
            continue;
        };
        let mut min_offset: Option<i64> = None;
        let mut max_offset: Option<i64> = None;

        for input in &spec.inputs {
            // desired_epoch = epoch - lag_seconds => offset from output epoch is -lag_seconds
            let desired_offset = -input.lag_seconds;
            let (child_min, child_max) = if derived.contains_key(&input.sensor_id) {
                offset_by_id
                    .get(&input.sensor_id)
                    .copied()
                    .unwrap_or((0, 0))
            } else {
                (0, 0)
            };

            let cand_min = desired_offset.saturating_add(child_min);
            let cand_max = desired_offset.saturating_add(child_max);

            min_offset = Some(min_offset.map_or(cand_min, |prev| prev.min(cand_min)));
            max_offset = Some(max_offset.map_or(cand_max, |prev| prev.max(cand_max)));
        }

        offset_by_id.insert(
            derived_id.clone(),
            (min_offset.unwrap_or(0), max_offset.unwrap_or(0)),
        );
    }

    let mut global_min: i64 = 0;
    let mut global_max: i64 = 0;
    let mut first = true;
    for id in derived_output_ids {
        let Some((min_off, max_off)) = offset_by_id.get(id).copied() else {
            continue;
        };
        if first {
            global_min = min_off;
            global_max = max_off;
            first = false;
        } else {
            global_min = global_min.min(min_off);
            global_max = global_max.max(max_off);
        }
    }

    let lookback = (-global_min).max(0);
    let lookahead = global_max.max(0);
    (lookback, lookahead)
}

#[derive(Debug, Clone)]
struct ForecastQuerySpec {
    provider: String,
    kind: String,
    metric: String,
    subject_kind: String,
    subject: String,
    require_asof: bool,
}

fn parse_forecast_query_spec(
    node_id: Uuid,
    config: &serde_json::Value,
) -> Option<ForecastQuerySpec> {
    let provider = config
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let kind = config
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let metric = config
        .get("metric")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let subject_kind = config
        .get("subject_kind")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let require_asof = config
        .get("mode")
        .and_then(|v| v.as_str())
        .is_some_and(|v| v.eq_ignore_ascii_case("asof"));

    if provider.is_empty() || kind.is_empty() || metric.is_empty() || subject_kind.is_empty() {
        return None;
    }

    let subject = config
        .get("subject")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
        .unwrap_or_else(|| node_id.to_string());

    Some(ForecastQuerySpec {
        provider: provider.to_string(),
        kind: kind.to_string(),
        metric: metric.to_string(),
        subject_kind: subject_kind.to_string(),
        subject,
        require_asof,
    })
}

async fn read_forecast_bucket_values(
    db: &PgPool,
    spec: &ForecastQuerySpec,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval_seconds: i64,
) -> Result<Vec<(DateTime<Utc>, f64)>, BucketReaderError> {
    #[derive(sqlx::FromRow)]
    struct ForecastBucketRow {
        bucket: DateTime<Utc>,
        avg_value: f64,
    }

    let interval_seconds = interval_seconds.max(1);
    let rows: Vec<ForecastBucketRow> = sqlx::query_as(
        r#"
        WITH points AS (
          SELECT DISTINCT ON (ts) ts, value, issued_at
          FROM forecast_points
              WHERE provider = $1
                AND kind = $2
                AND subject_kind = $3
                AND subject = $4
                AND metric = $5
                AND ts >= $7
                AND ts < $8
                AND ($9::bool = FALSE OR issued_at <= ts)
              ORDER BY ts ASC, issued_at DESC
            )
        SELECT
          time_bucket(make_interval(secs => $6), ts) as bucket,
          avg(value) as avg_value
        FROM points
        GROUP BY bucket
        ORDER BY bucket ASC
        "#,
    )
    .bind(&spec.provider)
    .bind(&spec.kind)
    .bind(&spec.subject_kind)
    .bind(&spec.subject)
    .bind(&spec.metric)
    .bind(interval_seconds)
    .bind(start)
    .bind(end)
    .bind(spec.require_asof)
    .fetch_all(db)
    .await
    .map_err(|err| BucketReaderError::DatabaseError(err.to_string()))?;

    Ok(rows.into_iter().map(|r| (r.bucket, r.avg_value)).collect())
}

/// Work item for iterative derived input expansion.
struct ExpansionWork {
    sensor_id: String,
    spec: DerivedSensorSpec,
    depth: usize,
    path: Vec<String>,
}

/// Expand derived sensor inputs iteratively to find all transitive raw inputs.
///
/// Uses a work queue instead of recursion to avoid stack issues and the need
/// for async_recursion.
#[cfg(test)]
async fn expand_derived_inputs(
    root_sensor_id: &str,
    root_spec: &DerivedSensorSpec,
    sources: &HashMap<String, SensorSource>,
    db: &PgPool,
) -> Result<ExpandedInputs, BucketReaderError> {
    let mut raw_inputs: HashSet<String> = HashSet::new();
    let mut forecast_inputs: HashMap<String, ForecastInput> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut sources_cache = sources.clone();

    // Initialize work queue with the root sensor's inputs
    let mut work_queue: Vec<ExpansionWork> = vec![ExpansionWork {
        sensor_id: root_sensor_id.to_string(),
        spec: root_spec.clone(),
        depth: 0,
        path: vec![root_sensor_id.to_string()],
    }];

    while let Some(work) = work_queue.pop() {
        if work.depth > MAX_DERIVED_DEPTH {
            return Err(BucketReaderError::DerivedDepthExceeded {
                sensor_id: work.sensor_id.clone(),
                depth: work.depth,
            });
        }

        for input in &work.spec.inputs {
            let input_id = &input.sensor_id;

            // Check for cycles
            if visited.contains(input_id) || work.path.contains(input_id) {
                let mut cycle = work.path.clone();
                cycle.push(input_id.clone());
                return Err(BucketReaderError::DerivedCycleDetected {
                    sensor_id: root_sensor_id.to_string(),
                    cycle,
                });
            }

            // Get the source classification for this input
            let input_source = if let Some(src) = sources_cache.get(input_id) {
                src.clone()
            } else {
                // Input not in our cache, need to fetch it
                let additional = classify_sensors(db, &[input_id.clone()]).await?;
                let src = additional
                    .get(input_id)
                    .cloned()
                    .unwrap_or(SensorSource::Raw);
                sources_cache.insert(input_id.clone(), src.clone());
                src
            };

            match input_source {
                SensorSource::Raw => {
                    raw_inputs.insert(input_id.clone());
                }
                SensorSource::Derived {
                    spec: input_spec, ..
                } => {
                    // Add to work queue for iterative expansion
                    visited.insert(input_id.clone());
                    let mut new_path = work.path.clone();
                    new_path.push(input_id.clone());

                    work_queue.push(ExpansionWork {
                        sensor_id: input_id.clone(),
                        spec: input_spec,
                        depth: work.depth + 1,
                        path: new_path,
                    });
                }
                SensorSource::Forecast { node_id, config } => {
                    forecast_inputs.insert(input_id.clone(), ForecastInput { node_id, config });
                }
            }
        }
    }

    Ok(ExpandedInputs {
        raw_inputs,
        forecast_inputs,
    })
}

/// Compute derived sensor values from raw bucket data.
fn compute_derived_buckets(
    derived_id: &str,
    spec: &DerivedSensorSpec,
    compiled: &mut DerivedSensorCompiled,
    rows_by_epoch: &HashMap<i64, HashMap<String, f64>>,
    output_start_epoch: i64,
    output_end_epoch: i64,
    interval_seconds: i64,
) -> Vec<MetricsBucketRow> {
    let interval_seconds = interval_seconds.max(1);
    let mut result: Vec<MetricsBucketRow> = Vec::new();

    let mut epochs: Vec<i64> = rows_by_epoch.keys().copied().collect();
    epochs.sort_unstable();

    // For each epoch in the requested output window, try to compute the derived value.
    for epoch in epochs {
        if epoch < output_start_epoch || epoch >= output_end_epoch {
            continue;
        }
        // Build vars map for this epoch
        let mut vars: HashMap<String, f64> = HashMap::new();
        let mut all_inputs_present = true;

        for input in &spec.inputs {
            let desired_epoch = epoch.saturating_sub(input.lag_seconds);
            let Some((_shifted_epoch, sensor_values)) = rows_by_epoch
                .get(&desired_epoch)
                .map(|v| (desired_epoch, v))
                .or_else(|| {
                    // If the desired epoch isn't a bucket boundary (i.e., lag isn't aligned to this
                    // query's interval), floor to the nearest bucket boundary. This prevents derived
                    // sensors (notably temp-compensation) from becoming empty at coarser intervals
                    // like 30m/1h when the lag was fitted at 5m resolution.
                    if desired_epoch.rem_euclid(interval_seconds) == 0 {
                        return None;
                    }
                    let floored_epoch = desired_epoch
                        .div_euclid(interval_seconds)
                        .saturating_mul(interval_seconds);
                    rows_by_epoch
                        .get(&floored_epoch)
                        .map(|v| (floored_epoch, v))
                })
            else {
                all_inputs_present = false;
                break;
            };

            let Some(value) = sensor_values.get(&input.sensor_id) else {
                all_inputs_present = false;
                break;
            };
            if !value.is_finite() {
                all_inputs_present = false;
                break;
            }
            vars.insert(input.var.clone(), *value);
        }

        if !all_inputs_present {
            continue;
        }

        // Evaluate the expression
        match compiled.eval_with_vars(&vars) {
            Ok(value) if value.is_finite() => {
                let bucket = Utc
                    .timestamp_opt(epoch, 0)
                    .single()
                    .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap());

                result.push(MetricsBucketRow {
                    sensor_id: derived_id.to_string(),
                    bucket,
                    value,
                    samples: 1, // Derived values are computed, not aggregated
                });
            }
            _ => {
                // Skip epochs where evaluation fails or returns non-finite
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_reader_error_display() {
        let err = BucketReaderError::UnsupportedSensorSource {
            sensor_ids: vec!["sensor-a".to_string(), "sensor-b".to_string()],
            source: "forecast_points".to_string(),
        };
        assert!(err.to_string().contains("forecast_points"));
        assert!(err.to_string().contains("sensor-a"));

        let err = BucketReaderError::DerivedCycleDetected {
            sensor_id: "d1".to_string(),
            cycle: vec!["d1".to_string(), "d2".to_string(), "d1".to_string()],
        };
        assert!(err.to_string().contains("Cycle"));
        assert!(err.to_string().contains("d1 -> d2 -> d1"));

        let err = BucketReaderError::DerivedDepthExceeded {
            sensor_id: "deep".to_string(),
            depth: 10,
        };
        assert!(err.to_string().contains("depth"));
        assert!(err.to_string().contains("10"));
    }

    #[test]
    fn test_bucket_reader_error_to_job_error() {
        // Test unsupported sensor source
        let err = BucketReaderError::UnsupportedSensorSource {
            sensor_ids: vec!["sensor-forecast".to_string()],
            source: "forecast_points".to_string(),
        };
        let job_err = err.to_job_error();
        assert_eq!(job_err.code, "unsupported_sensor_source");
        assert!(job_err.message.contains("forecast_points"));
        assert!(job_err.details.is_some());

        // Test cycle detection
        let err = BucketReaderError::DerivedCycleDetected {
            sensor_id: "d1".to_string(),
            cycle: vec!["d1".to_string(), "d2".to_string(), "d1".to_string()],
        };
        let job_err = err.to_job_error();
        assert_eq!(job_err.code, "derived_cycle_detected");
        assert!(job_err.details.is_some());
        let details = job_err.details.unwrap();
        assert_eq!(details["sensor_id"], "d1");

        // Test depth exceeded
        let err = BucketReaderError::DerivedDepthExceeded {
            sensor_id: "deep".to_string(),
            depth: 11,
        };
        let job_err = err.to_job_error();
        assert_eq!(job_err.code, "derived_depth_exceeded");

        // Test lake read failed
        let err = BucketReaderError::LakeReadFailed("connection timeout".to_string());
        let job_err = err.to_job_error();
        assert_eq!(job_err.code, "bucket_read_failed");
        assert!(job_err.message.contains("connection timeout"));

        // Test database error
        let err = BucketReaderError::DatabaseError("query failed".to_string());
        let job_err = err.to_job_error();
        assert_eq!(job_err.code, "bucket_read_failed");
        assert!(job_err.message.contains("query failed"));
    }

    #[test]
    fn test_parse_forecast_query_spec_defaults_subject_and_respects_asof_mode() {
        let node_id = Uuid::nil();
        let cfg = serde_json::json!({
            "source": "forecast_points",
            "provider": "open_meteo",
            "kind": "weather",
            "metric": "temperature_c",
            "subject_kind": "node",
            "mode": "asof"
        });

        let spec = parse_forecast_query_spec(node_id, &cfg).expect("spec");
        assert_eq!(spec.provider, "open_meteo");
        assert_eq!(spec.kind, "weather");
        assert_eq!(spec.metric, "temperature_c");
        assert_eq!(spec.subject_kind, "node");
        assert_eq!(spec.subject, node_id.to_string());
        assert!(spec.require_asof);
    }

    #[test]
    fn test_auto_aggregation_mode_uses_sum_for_counter_like_types() {
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("flow_meter")),
            BucketAggregationMode::Sum
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("pulse_counter")),
            BucketAggregationMode::Sum
        );
    }

    #[test]
    fn test_auto_aggregation_mode_uses_last_for_state_like_types() {
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("pump_status")),
            BucketAggregationMode::Last
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("switch_state")),
            BucketAggregationMode::Last
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("temperature")),
            BucketAggregationMode::Avg
        );
    }

    #[test]
    fn test_auto_aggregation_mode_handles_weather_semantics() {
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("wind_direction")),
            BucketAggregationMode::Last
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("rain_rate")),
            BucketAggregationMode::Avg
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("rain")),
            BucketAggregationMode::Last
        );
    }

    #[test]
    fn test_auto_aggregation_mode_normalizes_type_tokens() {
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("Flow Meter")),
            BucketAggregationMode::Sum
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("rain-gauge")),
            BucketAggregationMode::Sum
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("Pump Status")),
            BucketAggregationMode::Last
        );
        assert_eq!(
            auto_aggregation_mode_for_sensor_type(Some("Switch-State")),
            BucketAggregationMode::Last
        );
    }

    #[tokio::test]
    async fn test_expand_derived_inputs_includes_forecast_inputs() {
        use sqlx::postgres::PgPoolOptions;

        let db = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://postgres@localhost/postgres")
            .expect("lazy db");

        let spec = DerivedSensorSpec {
            expression: "raw - t".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "raw".to_string(),
                    sensor_id: "sensor-a".to_string(),
                    lag_seconds: 0,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "t".to_string(),
                    sensor_id: "forecast-t".to_string(),
                    lag_seconds: 0,
                },
            ],
        };

        let mut sources: HashMap<String, SensorSource> = HashMap::new();
        sources.insert("sensor-a".to_string(), SensorSource::Raw);
        sources.insert(
            "forecast-t".to_string(),
            SensorSource::Forecast {
                node_id: Uuid::nil(),
                config: serde_json::json!({
                    "source": "forecast_points",
                    "provider": "open_meteo",
                    "kind": "weather",
                    "metric": "temperature_c",
                    "subject_kind": "node"
                }),
            },
        );

        let expanded = expand_derived_inputs("derived-root", &spec, &sources, &db)
            .await
            .expect("expand");
        assert!(expanded.raw_inputs.contains("sensor-a"));
        assert!(expanded.forecast_inputs.contains_key("forecast-t"));
    }

    #[test]
    fn test_compute_derived_buckets_simple() {
        // Create a simple derived sensor: D = A + B
        let spec = DerivedSensorSpec {
            expression: "a + b".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "a".to_string(),
                    sensor_id: "sensor-a".to_string(),
                    lag_seconds: 0,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "b".to_string(),
                    sensor_id: "sensor-b".to_string(),
                    lag_seconds: 0,
                },
            ],
        };
        let mut compiled = compile_derived_sensor(&spec).unwrap();

        // Create test data: 2 epochs with both inputs present
        let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();

        let mut epoch1 = HashMap::new();
        epoch1.insert("sensor-a".to_string(), 10.0);
        epoch1.insert("sensor-b".to_string(), 5.0);
        rows_by_epoch.insert(1000, epoch1);

        let mut epoch2 = HashMap::new();
        epoch2.insert("sensor-a".to_string(), 20.0);
        epoch2.insert("sensor-b".to_string(), 15.0);
        rows_by_epoch.insert(2000, epoch2);

        // Compute derived values
        let result = compute_derived_buckets(
            "derived-sum",
            &spec,
            &mut compiled,
            &rows_by_epoch,
            i64::MIN,
            i64::MAX,
            60,
        );

        assert_eq!(result.len(), 2);

        // Find the results by epoch (order may vary)
        let val_1000 = result
            .iter()
            .find(|r| r.bucket.timestamp() == 1000)
            .unwrap();
        let val_2000 = result
            .iter()
            .find(|r| r.bucket.timestamp() == 2000)
            .unwrap();

        assert_eq!(val_1000.value, 15.0); // 10 + 5
        assert_eq!(val_2000.value, 35.0); // 20 + 15
        assert_eq!(val_1000.sensor_id, "derived-sum");
    }

    #[test]
    fn test_compute_derived_buckets_missing_input() {
        // Create a derived sensor: D = A + B
        let spec = DerivedSensorSpec {
            expression: "a + b".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "a".to_string(),
                    sensor_id: "sensor-a".to_string(),
                    lag_seconds: 0,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "b".to_string(),
                    sensor_id: "sensor-b".to_string(),
                    lag_seconds: 0,
                },
            ],
        };
        let mut compiled = compile_derived_sensor(&spec).unwrap();

        // Create test data: epoch 1 has both, epoch 2 missing B
        let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();

        let mut epoch1 = HashMap::new();
        epoch1.insert("sensor-a".to_string(), 10.0);
        epoch1.insert("sensor-b".to_string(), 5.0);
        rows_by_epoch.insert(1000, epoch1);

        let mut epoch2 = HashMap::new();
        epoch2.insert("sensor-a".to_string(), 20.0);
        // sensor-b missing
        rows_by_epoch.insert(2000, epoch2);

        let result = compute_derived_buckets(
            "derived-sum",
            &spec,
            &mut compiled,
            &rows_by_epoch,
            i64::MIN,
            i64::MAX,
            60,
        );

        // Only epoch 1000 should have a result
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket.timestamp(), 1000);
        assert_eq!(result[0].value, 15.0);
    }

    #[test]
    fn test_compute_derived_buckets_respects_lag_seconds() {
        // D = A + B, where B is read from 60s in the past.
        let spec = DerivedSensorSpec {
            expression: "a + b".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "a".to_string(),
                    sensor_id: "sensor-a".to_string(),
                    lag_seconds: 0,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "b".to_string(),
                    sensor_id: "sensor-b".to_string(),
                    lag_seconds: 60,
                },
            ],
        };
        let mut compiled = compile_derived_sensor(&spec).unwrap();

        let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();

        let mut t_940 = HashMap::new();
        t_940.insert("sensor-b".to_string(), 5.0);
        rows_by_epoch.insert(940, t_940);

        let mut t_1000 = HashMap::new();
        t_1000.insert("sensor-a".to_string(), 10.0);
        rows_by_epoch.insert(1000, t_1000);

        let result = compute_derived_buckets(
            "derived-sum",
            &spec,
            &mut compiled,
            &rows_by_epoch,
            i64::MIN,
            i64::MAX,
            60,
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket.timestamp(), 1000);
        assert_eq!(result[0].value, 15.0);
    }

    #[test]
    fn test_compute_derived_buckets_floors_misaligned_lag_to_bucket_boundary() {
        // D = A + B, where B is requested 90s in the past but the query interval is 60s.
        // That means the desired lookup epoch is not a bucket boundary; we should floor to 120s.
        let spec = DerivedSensorSpec {
            expression: "a + b".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "a".to_string(),
                    sensor_id: "sensor-a".to_string(),
                    lag_seconds: 0,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "b".to_string(),
                    sensor_id: "sensor-b".to_string(),
                    lag_seconds: 90,
                },
            ],
        };
        let mut compiled = compile_derived_sensor(&spec).unwrap();

        let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();

        // Output bucket at 210s; B should be read from 120s after flooring (210-90=120).
        let mut t_120 = HashMap::new();
        t_120.insert("sensor-b".to_string(), 5.0);
        rows_by_epoch.insert(120, t_120);

        let mut t_210 = HashMap::new();
        t_210.insert("sensor-a".to_string(), 10.0);
        rows_by_epoch.insert(210, t_210);

        let result = compute_derived_buckets(
            "derived-sum",
            &spec,
            &mut compiled,
            &rows_by_epoch,
            i64::MIN,
            i64::MAX,
            60,
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket.timestamp(), 210);
        assert_eq!(result[0].value, 15.0);
    }

    #[test]
    fn test_compute_derived_buckets_non_finite_input() {
        // Create a derived sensor: D = A * 2
        let spec = DerivedSensorSpec {
            expression: "a * 2".to_string(),
            inputs: vec![crate::services::derived_sensors::DerivedSensorInput {
                var: "a".to_string(),
                sensor_id: "sensor-a".to_string(),
                lag_seconds: 0,
            }],
        };
        let mut compiled = compile_derived_sensor(&spec).unwrap();

        let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();

        let mut epoch1 = HashMap::new();
        epoch1.insert("sensor-a".to_string(), f64::NAN);
        rows_by_epoch.insert(1000, epoch1);

        let mut epoch2 = HashMap::new();
        epoch2.insert("sensor-a".to_string(), 5.0);
        rows_by_epoch.insert(2000, epoch2);

        let result = compute_derived_buckets(
            "derived",
            &spec,
            &mut compiled,
            &rows_by_epoch,
            i64::MIN,
            i64::MAX,
            60,
        );

        // NaN input should be skipped
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bucket.timestamp(), 2000);
        assert_eq!(result[0].value, 10.0);
    }

    #[test]
    fn test_compute_transitive_lag_window_sums_derived_chain_offsets() {
        // d1 reads A from the past (lag 10s)
        // d2 reads d1 from the past (lag 20s) => total leaf lookback should be 30s
        let d1_spec = DerivedSensorSpec {
            expression: "a".to_string(),
            inputs: vec![crate::services::derived_sensors::DerivedSensorInput {
                var: "a".to_string(),
                sensor_id: "raw-a".to_string(),
                lag_seconds: 10,
            }],
        };
        let d2_spec = DerivedSensorSpec {
            expression: "x + b".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "x".to_string(),
                    sensor_id: "d1".to_string(),
                    lag_seconds: 20,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "b".to_string(),
                    sensor_id: "raw-b".to_string(),
                    lag_seconds: 0,
                },
            ],
        };

        let mut derived: HashMap<String, (DerivedSensorSpec, DerivedSensorCompiled)> =
            HashMap::new();
        derived.insert(
            "d1".to_string(),
            (d1_spec.clone(), compile_derived_sensor(&d1_spec).unwrap()),
        );
        derived.insert(
            "d2".to_string(),
            (d2_spec.clone(), compile_derived_sensor(&d2_spec).unwrap()),
        );

        let order = topo_sort_derived_ids(&derived).expect("topo sort");
        let (lookback, lookahead) =
            compute_transitive_lag_window(&derived, &["d2".to_string()], &order);
        assert_eq!(lookback, 30);
        assert_eq!(lookahead, 0);
    }

    #[test]
    fn test_derived_of_derived_computation_uses_intermediate_buckets() {
        // d1 = a + b
        // d2 = d1 * c
        let d1_spec = DerivedSensorSpec {
            expression: "a + b".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "a".to_string(),
                    sensor_id: "raw-a".to_string(),
                    lag_seconds: 0,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "b".to_string(),
                    sensor_id: "raw-b".to_string(),
                    lag_seconds: 0,
                },
            ],
        };
        let d2_spec = DerivedSensorSpec {
            expression: "x * c".to_string(),
            inputs: vec![
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "x".to_string(),
                    sensor_id: "d1".to_string(),
                    lag_seconds: 0,
                },
                crate::services::derived_sensors::DerivedSensorInput {
                    var: "c".to_string(),
                    sensor_id: "raw-c".to_string(),
                    lag_seconds: 0,
                },
            ],
        };

        let mut derived: HashMap<String, (DerivedSensorSpec, DerivedSensorCompiled)> =
            HashMap::new();
        derived.insert(
            "d1".to_string(),
            (d1_spec.clone(), compile_derived_sensor(&d1_spec).unwrap()),
        );
        derived.insert(
            "d2".to_string(),
            (d2_spec.clone(), compile_derived_sensor(&d2_spec).unwrap()),
        );

        let order = topo_sort_derived_ids(&derived).expect("topo sort");
        let d1_pos = order.iter().position(|id| id == "d1").expect("d1 position");
        let d2_pos = order.iter().position(|id| id == "d2").expect("d2 position");
        assert!(d1_pos < d2_pos, "Expected d1 before d2, got {:?}", order);

        let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();

        let mut t1 = HashMap::new();
        t1.insert("raw-a".to_string(), 2.0);
        t1.insert("raw-b".to_string(), 3.0);
        t1.insert("raw-c".to_string(), 10.0);
        rows_by_epoch.insert(1000, t1);

        let mut t2 = HashMap::new();
        t2.insert("raw-a".to_string(), 4.0);
        t2.insert("raw-b".to_string(), 5.0);
        t2.insert("raw-c".to_string(), 20.0);
        rows_by_epoch.insert(2000, t2);

        let mut computed: HashMap<String, Vec<MetricsBucketRow>> = HashMap::new();
        for derived_id in &order {
            let (spec, compiled_template) = derived.get(derived_id).expect("spec");
            let mut compiled = compiled_template.clone();
            let rows = compute_derived_buckets(
                derived_id,
                spec,
                &mut compiled,
                &rows_by_epoch,
                i64::MIN,
                i64::MAX,
                60,
            );

            for row in &rows {
                let epoch = row.bucket.timestamp();
                rows_by_epoch
                    .entry(epoch)
                    .or_default()
                    .insert(derived_id.clone(), row.value);
            }
            computed.insert(derived_id.clone(), rows);
        }

        let d2_rows = computed.get("d2").expect("d2 rows");
        assert_eq!(d2_rows.len(), 2);
        let v1 = d2_rows
            .iter()
            .find(|r| r.bucket.timestamp() == 1000)
            .unwrap()
            .value;
        let v2 = d2_rows
            .iter()
            .find(|r| r.bucket.timestamp() == 2000)
            .unwrap()
            .value;
        assert_eq!(v1, 50.0);
        assert_eq!(v2, 180.0);
    }

    #[test]
    fn test_compute_derived_buckets_result_sorted() {
        let spec = DerivedSensorSpec {
            expression: "x".to_string(),
            inputs: vec![crate::services::derived_sensors::DerivedSensorInput {
                var: "x".to_string(),
                sensor_id: "sensor-x".to_string(),
                lag_seconds: 0,
            }],
        };
        let mut compiled = compile_derived_sensor(&spec).unwrap();

        // Insert epochs out of order
        let mut rows_by_epoch: HashMap<i64, HashMap<String, f64>> = HashMap::new();
        for epoch in [3000_i64, 1000, 2000, 5000, 4000] {
            let mut values = HashMap::new();
            values.insert("sensor-x".to_string(), epoch as f64);
            rows_by_epoch.insert(epoch, values);
        }

        let result = compute_derived_buckets(
            "derived",
            &spec,
            &mut compiled,
            &rows_by_epoch,
            i64::MIN,
            i64::MAX,
            60,
        );

        assert_eq!(result.len(), 5);
        // Verify sorted order
        let timestamps: Vec<i64> = result.iter().map(|r| r.bucket.timestamp()).collect();
        assert_eq!(timestamps, vec![1000, 2000, 3000, 4000, 5000]);
    }
}
