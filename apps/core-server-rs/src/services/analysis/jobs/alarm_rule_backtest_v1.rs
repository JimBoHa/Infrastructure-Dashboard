use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::alarm_engine;
use crate::services::alarm_engine::types::{ConditionNode, ConsecutivePeriod, MatchMode, RuleEnvelope};
use crate::services::analysis::bucket_reader::{
    read_bucket_series_for_sensors_with_aggregation_and_options, BucketAggregationPreference,
};
use crate::services::analysis::parquet_duckdb::{
    expected_bucket_count, DuckDbQueryService, MetricsBucketReadOptions, MetricsQualityFilter,
};
use crate::services::analysis::tsse::types::BucketAggregationModeV1;
use anyhow::Context;
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as JsonValue};
use sqlx::PgPool;
use std::collections::HashMap;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

const MAX_BUCKETS: u64 = 50_000;
const DEFAULT_INTERVAL_SECONDS: i64 = 60;

#[derive(Debug, Clone, Deserialize)]
struct AlarmRuleBacktestJobParamsV1 {
    target_selector: JsonValue,
    condition_ast: JsonValue,
    timing: Option<JsonValue>,
    start: String,
    end: String,
    interval_seconds: Option<i64>,
    bucket_aggregation_mode: Option<BucketAggregationModeV1>,
}

#[derive(Debug, Clone, Serialize)]
struct AlarmRuleBacktestJobParamsNormalizedV1 {
    target_selector: JsonValue,
    condition_ast: JsonValue,
    timing: JsonValue,
    start: String,
    end: String,
    interval_seconds: i64,
    bucket_aggregation_mode: BucketAggregationModeV1,
    eval_step_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
struct AlarmRuleBacktestTransitionV1 {
    timestamp: String,
    transition: String,
    observed_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct AlarmRuleBacktestIntervalV1 {
    start_ts: String,
    end_ts: String,
    duration_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
struct AlarmRuleBacktestTargetSummaryV1 {
    fired_count: u32,
    resolved_count: u32,
    interval_count: u32,
    time_firing_seconds: i64,
    min_interval_seconds: Option<i64>,
    max_interval_seconds: Option<i64>,
    median_interval_seconds: Option<i64>,
    p95_interval_seconds: Option<i64>,
    mean_interval_seconds: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
struct AlarmRuleBacktestTargetResultV1 {
    target_key: String,
    sensor_ids: Vec<String>,
    transitions: Vec<AlarmRuleBacktestTransitionV1>,
    firing_intervals: Vec<AlarmRuleBacktestIntervalV1>,
    summary: AlarmRuleBacktestTargetSummaryV1,
}

#[derive(Debug, Clone, Serialize)]
struct AlarmRuleBacktestSummaryV1 {
    target_count: u32,
    total_fired: u32,
    total_resolved: u32,
    total_time_firing_seconds: i64,
}

#[derive(Debug, Clone, Serialize)]
struct AlarmRuleBacktestResultV1 {
    job_type: String,
    computed_through_ts: String,
    params: AlarmRuleBacktestJobParamsNormalizedV1,
    summary: AlarmRuleBacktestSummaryV1,
    targets: Vec<AlarmRuleBacktestTargetResultV1>,
    timings_ms: HashMap<String, u64>,
}

#[derive(Debug, Clone)]
struct DenseSeriesIndex {
    start_bucket_epoch: i64,
    interval_seconds: i64,
    bucket_count: usize,
    values_by_sensor: HashMap<String, Vec<Option<f64>>>,
}

impl DenseSeriesIndex {
    fn value_at(&self, sensor_id: &str, idx: usize) -> Option<f64> {
        self.values_by_sensor
            .get(sensor_id)
            .and_then(|series| series.get(idx).copied().flatten())
    }

    fn slice_values(&self, sensor_id: &str, start_idx: usize, end_idx: usize) -> Vec<f64> {
        let Some(series) = self.values_by_sensor.get(sensor_id) else {
            return Vec::new();
        };
        let start_idx = start_idx.min(series.len());
        let end_idx = end_idx.min(series.len().saturating_sub(1));
        if start_idx > end_idx {
            return Vec::new();
        }
        series[start_idx..=end_idx]
            .iter()
            .copied()
            .flatten()
            .filter(|v| v.is_finite())
            .collect()
    }
}

fn to_bucket_aggregation_preference(mode: BucketAggregationModeV1) -> BucketAggregationPreference {
    match mode {
        BucketAggregationModeV1::Auto => BucketAggregationPreference::Auto,
        BucketAggregationModeV1::Avg => BucketAggregationPreference::Avg,
        BucketAggregationModeV1::Last => BucketAggregationPreference::Last,
        BucketAggregationModeV1::Sum => BucketAggregationPreference::Sum,
        BucketAggregationModeV1::Min => BucketAggregationPreference::Min,
        BucketAggregationModeV1::Max => BucketAggregationPreference::Max,
    }
}

fn quantile_sorted(sorted: &[i64], q: f64) -> Option<i64> {
    if sorted.is_empty() {
        return None;
    }
    let q = q.clamp(0.0, 1.0);
    if sorted.len() == 1 {
        return Some(sorted[0]);
    }
    let pos = q * (sorted.len() as f64 - 1.0);
    let idx = pos.floor() as usize;
    let frac = pos - idx as f64;
    let a = sorted[idx] as f64;
    let b = sorted[(idx + 1).min(sorted.len() - 1)] as f64;
    Some((a + (b - a) * frac).round() as i64)
}

fn period_bucket(period: ConsecutivePeriod, now: DateTime<Utc>) -> i64 {
    match period {
        ConsecutivePeriod::Eval => now.timestamp(),
        ConsecutivePeriod::Hour => now.timestamp() / 3600,
        ConsecutivePeriod::Day => now.date_naive().num_days_from_ce() as i64,
    }
}

fn eval_values<F>(values: Vec<f64>, mode: MatchMode, predicate: F) -> (bool, Option<f64>)
where
    F: Fn(f64) -> bool,
{
    if values.is_empty() {
        return (false, None);
    }
    let observed_value = values.first().copied();
    let passed = match mode {
        MatchMode::All => values.iter().all(|value| predicate(*value)),
        MatchMode::PerSensor | MatchMode::Any => values.iter().any(|value| predicate(*value)),
    };
    (passed, observed_value)
}

#[derive(Debug, Clone)]
struct WindowStats {
    avg: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    stddev: Option<f64>,
    median: Option<f64>,
}

fn window_stats(values: &[f64]) -> WindowStats {
    if values.is_empty() {
        return WindowStats {
            avg: None,
            min: None,
            max: None,
            stddev: None,
            median: None,
        };
    }

    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return WindowStats {
            avg: None,
            min: None,
            max: None,
            stddev: None,
            median: None,
        };
    }
    sorted.sort_by(|a, b| a.total_cmp(b));

    let n = sorted.len() as f64;
    let avg = Some(sorted.iter().sum::<f64>() / n);
    let min = sorted.first().copied();
    let max = sorted.last().copied();

    let median = if sorted.len() == 1 {
        Some(sorted[0])
    } else {
        let pos = 0.5 * (sorted.len() as f64 - 1.0);
        let idx = pos.floor() as usize;
        let frac = pos - idx as f64;
        let a = sorted[idx];
        let b = sorted[(idx + 1).min(sorted.len() - 1)];
        Some(a + (b - a) * frac)
    };

    let stddev = if sorted.len() < 2 {
        None
    } else if let Some(mean) = avg {
        let variance =
            sorted.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
        let stddev = variance.sqrt();
        if stddev.is_finite() {
            Some(stddev)
        } else {
            None
        }
    } else {
        None
    };

    WindowStats {
        avg,
        min,
        max,
        stddev,
        median,
    }
}

fn eval_condition_backtest(
    node: &ConditionNode,
    target: &alarm_engine::ResolvedTarget,
    now: DateTime<Utc>,
    series: &DenseSeriesIndex,
    idx: usize,
    last_seen_epoch_by_sensor: &HashMap<&str, i64>,
    state_window: &mut Map<String, JsonValue>,
    path: &str,
) -> (bool, Option<f64>) {
    match node {
        ConditionNode::Threshold { op, value } => {
            let values: Vec<f64> = target
                .sensor_ids
                .iter()
                .filter_map(|sensor_id| series.value_at(sensor_id, idx))
                .collect();
            eval_values(values, target.match_mode, |sample| {
                alarm_engine::types::compare(sample, *op, *value)
            })
        }
        ConditionNode::Range { mode, low, high } => {
            let values: Vec<f64> = target
                .sensor_ids
                .iter()
                .filter_map(|sensor_id| series.value_at(sensor_id, idx))
                .collect();
            eval_values(values, target.match_mode, |sample| {
                let inside = sample >= *low && sample <= *high;
                match mode {
                    alarm_engine::types::RangeMode::Inside => inside,
                    alarm_engine::types::RangeMode::Outside => !inside,
                }
            })
        }
        ConditionNode::Offline {
            missing_for_seconds,
        } => {
            let statuses: Vec<bool> = target
                .sensor_ids
                .iter()
                .map(|sensor_id| {
                    let key = sensor_id.as_str();
                    let Some(last_seen) = last_seen_epoch_by_sensor.get(key) else {
                        return true;
                    };
                    now.timestamp().saturating_sub(*last_seen) >= *missing_for_seconds
                })
                .collect();
            let passed = match target.match_mode {
                MatchMode::All => statuses.iter().all(|value| *value),
                MatchMode::PerSensor | MatchMode::Any => statuses.iter().any(|value| *value),
            };
            (passed, None)
        }
        ConditionNode::RollingWindow {
            window_seconds,
            aggregate,
            op,
            value,
        } => {
            let window_seconds = (*window_seconds).max(1);
            let cutoff = now
                .timestamp()
                .saturating_sub(window_seconds);
            let start_idx = ((cutoff - series.start_bucket_epoch)
                .div_euclid(series.interval_seconds))
            .max(0) as usize;
            let mut samples: Vec<f64> = Vec::new();
            for sensor_id in &target.sensor_ids {
                let window_values = series.slice_values(sensor_id, start_idx, idx);
                let stats = window_stats(&window_values);
                let sample = match aggregate {
                    alarm_engine::types::AggregateOp::Avg => stats.avg,
                    alarm_engine::types::AggregateOp::Min => stats.min,
                    alarm_engine::types::AggregateOp::Max => stats.max,
                    alarm_engine::types::AggregateOp::Stddev => stats.stddev,
                };
                if let Some(sample) = sample {
                    samples.push(sample);
                }
            }
            eval_values(samples, target.match_mode, |sample| {
                alarm_engine::types::compare(sample, *op, *value)
            })
        }
        ConditionNode::Deviation {
            window_seconds,
            baseline,
            mode,
            value,
        } => {
            let window_seconds = (*window_seconds).max(1);
            let cutoff = now
                .timestamp()
                .saturating_sub(window_seconds);
            let start_idx = ((cutoff - series.start_bucket_epoch)
                .div_euclid(series.interval_seconds))
            .max(0) as usize;

            let mut samples: Vec<f64> = Vec::new();
            for sensor_id in &target.sensor_ids {
                let Some(current) = series.value_at(sensor_id, idx) else {
                    continue;
                };
                let window_values = series.slice_values(sensor_id, start_idx, idx);
                let stats = window_stats(&window_values);
                let baseline_value = match baseline {
                    alarm_engine::types::BaselineOp::Mean => stats.avg,
                    alarm_engine::types::BaselineOp::Median => stats.median,
                };
                let Some(baseline_value) = baseline_value else {
                    continue;
                };
                let delta = (current - baseline_value).abs();
                let deviation = match mode {
                    alarm_engine::types::DeviationMode::Absolute => delta,
                    alarm_engine::types::DeviationMode::Percent => {
                        if baseline_value.abs() <= f64::EPSILON {
                            continue;
                        }
                        (delta / baseline_value.abs()) * 100.0
                    }
                };
                samples.push(deviation);
            }
            eval_values(samples, target.match_mode, |sample| sample >= *value)
        }
        ConditionNode::ConsecutivePeriods {
            period,
            count,
            child,
        } => {
            let child_path = format!("{path}.cp");
            let (child_passed, child_observed) = eval_condition_backtest(
                child,
                target,
                now,
                series,
                idx,
                last_seen_epoch_by_sensor,
                state_window,
                &child_path,
            );

            let state_key = format!("cp:{path}");
            let mut state = state_window
                .get(&state_key)
                .and_then(JsonValue::as_object)
                .cloned()
                .unwrap_or_default();

            let current_period = period_bucket(*period, now);
            let mut streak = state.get("streak").and_then(JsonValue::as_i64).unwrap_or(0);
            let last_period = state.get("last_period").and_then(JsonValue::as_i64);

            if child_passed {
                match period {
                    ConsecutivePeriod::Eval => {
                        streak += 1;
                    }
                    ConsecutivePeriod::Hour | ConsecutivePeriod::Day => {
                        if let Some(last_period) = last_period {
                            if last_period == current_period {
                                if streak < 1 {
                                    streak = 1;
                                }
                            } else if last_period + 1 == current_period {
                                streak += 1;
                            } else {
                                streak = 1;
                            }
                        } else {
                            streak = 1;
                        }
                    }
                }
                state.insert("last_period".to_string(), JsonValue::from(current_period));
            } else {
                streak = 0;
                state.insert("last_period".to_string(), JsonValue::from(current_period));
            }

            state.insert("streak".to_string(), JsonValue::from(streak));
            state_window.insert(state_key, JsonValue::Object(state));
            state_window.insert("consecutive_hits".to_string(), JsonValue::from(streak));

            (streak >= *count as i64, child_observed)
        }
        ConditionNode::All { children } => {
            let mut observed = None;
            for (index, child) in children.iter().enumerate() {
                let child_path = format!("{path}.all[{index}]");
                let (passed, child_observed) = eval_condition_backtest(
                    child,
                    target,
                    now,
                    series,
                    idx,
                    last_seen_epoch_by_sensor,
                    state_window,
                    &child_path,
                );
                if observed.is_none() {
                    observed = child_observed;
                }
                if !passed {
                    return (false, observed);
                }
            }
            (true, observed)
        }
        ConditionNode::Any { children } => {
            let mut observed = None;
            for (index, child) in children.iter().enumerate() {
                let child_path = format!("{path}.any[{index}]");
                let (passed, child_observed) = eval_condition_backtest(
                    child,
                    target,
                    now,
                    series,
                    idx,
                    last_seen_epoch_by_sensor,
                    state_window,
                    &child_path,
                );
                if observed.is_none() {
                    observed = child_observed;
                }
                if passed {
                    return (true, observed);
                }
            }
            (false, observed)
        }
        ConditionNode::Not { child } => {
            let child_path = format!("{path}.not");
            let (passed, observed) = eval_condition_backtest(
                child,
                target,
                now,
                series,
                idx,
                last_seen_epoch_by_sensor,
                state_window,
                &child_path,
            );
            (!passed, observed)
        }
    }
}

pub async fn execute(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &crate::services::analysis::lake::AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let params: AlarmRuleBacktestJobParamsV1 = serde_json::from_value(job.params.0.clone())
        .map_err(|err| JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: err.to_string(),
            details: None,
        }))?;

    let start = DateTime::parse_from_rfc3339(params.start.trim())
        .map_err(|_| JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "Invalid start timestamp".to_string(),
            details: None,
        }))?
        .with_timezone(&Utc);
    let end_inclusive = DateTime::parse_from_rfc3339(params.end.trim())
        .map_err(|_| JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "Invalid end timestamp".to_string(),
            details: None,
        }))?
        .with_timezone(&Utc);
    if end_inclusive <= start {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "end must be after start".to_string(),
            details: None,
        }));
    }
    let end = end_inclusive + Duration::microseconds(1);

    let timing = params.timing.clone().unwrap_or(JsonValue::Object(Default::default()));
    let envelope: RuleEnvelope =
        alarm_engine::types::parse_rule_envelope(&params.target_selector, &params.condition_ast, &timing)
            .map_err(|err| JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err,
                details: None,
            }))?;

    let bucket_aggregation_mode = params.bucket_aggregation_mode.unwrap_or(BucketAggregationModeV1::Auto);

    let eval_step_seconds = if envelope.timing.eval_interval_seconds > 0 {
        envelope.timing.eval_interval_seconds.max(1)
    } else {
        DEFAULT_INTERVAL_SECONDS
    };

    let mut interval_seconds = params
        .interval_seconds
        .unwrap_or_else(|| eval_step_seconds)
        .max(1);

    let expected = expected_bucket_count(start, end, interval_seconds);
    if expected > MAX_BUCKETS {
        let horizon_seconds = (end_inclusive - start).num_seconds().max(1);
        interval_seconds = ((horizon_seconds as f64) / (MAX_BUCKETS as f64)).ceil() as i64;
        interval_seconds = interval_seconds.max(1);
    }

    let mut progress = AnalysisJobProgress {
        phase: "resolve_targets".to_string(),
        completed: 0,
        total: None,
        message: Some("Resolving alarm rule targets".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let targets = alarm_engine::resolve_targets(db, &envelope.target_selector)
        .await
        .map_err(|err| JobFailure::Failed(AnalysisJobError {
            code: "target_resolution_failed".to_string(),
            message: err.to_string(),
            details: None,
        }))?;
    if targets.is_empty() {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "No targets matched the selector".to_string(),
            details: None,
        }));
    }

    let mut sensor_ids: Vec<String> = targets
        .iter()
        .flat_map(|t| t.sensor_ids.iter().cloned())
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    sensor_ids.sort();
    sensor_ids.dedup();
    if sensor_ids.is_empty() {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "No sensors resolved for selector".to_string(),
            details: None,
        }));
    }

    progress.phase = "load_series".to_string();
    progress.message = Some("Loading bucketed sensor series".to_string());
    progress.completed = 0;
    progress.total = Some(sensor_ids.len() as u64);
    let _ = store::update_progress(db, job.id, &progress).await;

    let load_started = Instant::now();
    let options = MetricsBucketReadOptions {
        min_samples_per_bucket: Some(1),
        quality_filter: MetricsQualityFilter::GoodOnly,
    };
    let rows = read_bucket_series_for_sensors_with_aggregation_and_options(
        db,
        duckdb,
        lake,
        sensor_ids.clone(),
        start,
        end,
        interval_seconds,
        to_bucket_aggregation_preference(bucket_aggregation_mode),
        options,
    )
    .await
    .map_err(|err| JobFailure::Failed(err.to_job_error()))?;
    let load_ms = load_started.elapsed().as_millis() as u64;

    let start_bucket_epoch = start.timestamp().div_euclid(interval_seconds) * interval_seconds;
    let bucket_count = expected_bucket_count(start, end, interval_seconds) as usize;

    let mut values_by_sensor: HashMap<String, Vec<Option<f64>>> = HashMap::new();
    for id in sensor_ids.iter() {
        values_by_sensor.insert(id.clone(), vec![None; bucket_count]);
    }

    for row in rows {
        let epoch = row.bucket.timestamp();
        let idx = epoch
            .saturating_sub(start_bucket_epoch)
            .div_euclid(interval_seconds);
        if idx < 0 {
            continue;
        }
        let idx = idx as usize;
        if idx >= bucket_count {
            continue;
        }
        if let Some(series) = values_by_sensor.get_mut(&row.sensor_id) {
            series[idx] = if row.value.is_finite() { Some(row.value) } else { None };
        }
    }

    let series = DenseSeriesIndex {
        start_bucket_epoch,
        interval_seconds,
        bucket_count,
        values_by_sensor,
    };

    let eval_step_seconds = eval_step_seconds.max(interval_seconds);
    let step_buckets = ((eval_step_seconds as f64) / (interval_seconds as f64)).ceil() as usize;
    let step_buckets = step_buckets.max(1);

    progress.phase = "simulate".to_string();
    progress.message = Some("Simulating alarm evaluation over history".to_string());
    progress.completed = 0;
    progress.total = Some(series.bucket_count as u64);
    let _ = store::update_progress(db, job.id, &progress).await;

    let sim_started = Instant::now();
    let mut timings_ms: HashMap<String, u64> = HashMap::new();
    timings_ms.insert("load_series".to_string(), load_ms);

    let mut last_seen_epoch_by_sensor: HashMap<&str, i64> = HashMap::new();
    let mut target_state: Vec<(alarm_engine::ResolvedTarget, Map<String, JsonValue>, bool, Option<DateTime<Utc>>, Vec<AlarmRuleBacktestTransitionV1>, Vec<AlarmRuleBacktestIntervalV1>)> =
        Vec::new();
    for target in targets.into_iter() {
        target_state.push((target, Map::new(), false, None, Vec::new(), Vec::new()));
    }

    for idx in (0..series.bucket_count).step_by(step_buckets) {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }

        // Evaluation timestamp = end of the bucket.
        let bucket_end_epoch =
            series.start_bucket_epoch + ((idx as i64 + 1).saturating_mul(series.interval_seconds));
        let now = Utc
            .timestamp_opt(bucket_end_epoch, 0)
            .single()
            .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap());

        for sensor_id in sensor_ids.iter() {
            if series.value_at(sensor_id, idx).is_some() {
                last_seen_epoch_by_sensor.insert(sensor_id.as_str(), now.timestamp());
            }
        }

        for (target, window_state, currently_firing, open_start, transitions, intervals) in
            target_state.iter_mut()
        {
            let (should_fire_now, observed_value) = eval_condition_backtest(
                &envelope.condition,
                target,
                now,
                &series,
                idx,
                &last_seen_epoch_by_sensor,
                window_state,
                "root",
            );

            let desired_firing = alarm_engine::apply_firing_timing(
                should_fire_now,
                *currently_firing,
                &envelope.timing,
                now,
                window_state,
            );

            if desired_firing && !*currently_firing {
                transitions.push(AlarmRuleBacktestTransitionV1 {
                    timestamp: now.to_rfc3339(),
                    transition: "fired".to_string(),
                    observed_value,
                });
                *open_start = Some(now);
            } else if !desired_firing && *currently_firing {
                transitions.push(AlarmRuleBacktestTransitionV1 {
                    timestamp: now.to_rfc3339(),
                    transition: "resolved".to_string(),
                    observed_value,
                });
                if let Some(start_ts) = open_start.take() {
                    let duration_seconds = (now - start_ts).num_seconds().max(0);
                    intervals.push(AlarmRuleBacktestIntervalV1 {
                        start_ts: start_ts.to_rfc3339(),
                        end_ts: now.to_rfc3339(),
                        duration_seconds,
                    });
                }
            }

            *currently_firing = desired_firing;
        }

        progress.completed = (idx as u64).saturating_add(1).min(series.bucket_count as u64);
        if progress.completed % 250 == 0 {
            let _ = store::update_progress(db, job.id, &progress).await;
        }
    }

    let sim_ms = sim_started.elapsed().as_millis() as u64;
    timings_ms.insert("simulate".to_string(), sim_ms);

    // Finalize any open firing intervals.
    let end_ts = end_inclusive;
    for (_target, _window_state, currently_firing, open_start, _transitions, intervals) in
        target_state.iter_mut()
    {
        if *currently_firing {
            if let Some(start_ts) = open_start.take() {
                let duration_seconds = (end_ts - start_ts).num_seconds().max(0);
                intervals.push(AlarmRuleBacktestIntervalV1 {
                    start_ts: start_ts.to_rfc3339(),
                    end_ts: end_ts.to_rfc3339(),
                    duration_seconds,
                });
            }
        }
    }

    progress.phase = "finalize".to_string();
    progress.message = Some("Computing backtest summaries".to_string());
    progress.completed = series.bucket_count as u64;
    progress.total = Some(series.bucket_count as u64);
    let _ = store::update_progress(db, job.id, &progress).await;

    let mut targets_out: Vec<AlarmRuleBacktestTargetResultV1> = Vec::new();
    let mut total_fired: u32 = 0;
    let mut total_resolved: u32 = 0;
    let mut total_time_firing_seconds: i64 = 0;

    for (target, _window_state, _currently_firing, _open_start, transitions, intervals) in
        target_state.into_iter()
    {
        let fired_count = transitions
            .iter()
            .filter(|t| t.transition == "fired")
            .count() as u32;
        let resolved_count = transitions
            .iter()
            .filter(|t| t.transition == "resolved")
            .count() as u32;
        total_fired = total_fired.saturating_add(fired_count);
        total_resolved = total_resolved.saturating_add(resolved_count);

        let mut durations: Vec<i64> = intervals.iter().map(|i| i.duration_seconds).collect();
        durations.sort();
        let time_firing_seconds: i64 = durations.iter().copied().sum();
        total_time_firing_seconds = total_time_firing_seconds.saturating_add(time_firing_seconds);

        let min_interval_seconds = durations.first().copied();
        let max_interval_seconds = durations.last().copied();
        let median_interval_seconds = quantile_sorted(&durations, 0.5);
        let p95_interval_seconds = quantile_sorted(&durations, 0.95);
        let mean_interval_seconds = if durations.is_empty() {
            None
        } else {
            Some(time_firing_seconds as f64 / durations.len() as f64)
        };

        targets_out.push(AlarmRuleBacktestTargetResultV1 {
            target_key: target.target_key,
            sensor_ids: target.sensor_ids,
            transitions,
            firing_intervals: intervals.clone(),
            summary: AlarmRuleBacktestTargetSummaryV1 {
                fired_count,
                resolved_count,
                interval_count: intervals.len() as u32,
                time_firing_seconds,
                min_interval_seconds,
                max_interval_seconds,
                median_interval_seconds,
                p95_interval_seconds,
                mean_interval_seconds,
            },
        });
    }

    targets_out.sort_by(|a, b| a.target_key.cmp(&b.target_key));

    let normalized = AlarmRuleBacktestJobParamsNormalizedV1 {
        target_selector: params.target_selector,
        condition_ast: params.condition_ast,
        timing,
        start: start.to_rfc3339(),
        end: end_inclusive.to_rfc3339(),
        interval_seconds,
        bucket_aggregation_mode,
        eval_step_seconds,
    };

    let result = AlarmRuleBacktestResultV1 {
        job_type: "alarm_rule_backtest_v1".to_string(),
        computed_through_ts: Utc::now().to_rfc3339(),
        params: normalized,
        summary: AlarmRuleBacktestSummaryV1 {
            target_count: targets_out.len() as u32,
            total_fired,
            total_resolved,
            total_time_firing_seconds,
        },
        targets: targets_out,
        timings_ms,
    };

    serde_json::to_value(&result)
        .context("failed to serialize alarm_rule_backtest_v1 result")
        .map_err(|err| JobFailure::Failed(AnalysisJobError {
            code: "result_encode_failed".to_string(),
            message: err.to_string(),
            details: None,
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::alarm_engine::types::{CompareOp, TimingConfig};

    fn simulate_condition_series(
        condition: &ConditionNode,
        timing: &TimingConfig,
        values: Vec<Option<f64>>,
        interval_seconds: i64,
    ) -> Vec<(i64, String)> {
        let sensor_id = "sensor-1".to_string();
        let bucket_count = values.len();
        let mut values_by_sensor = HashMap::new();
        values_by_sensor.insert(sensor_id.clone(), values);
        let series = DenseSeriesIndex {
            start_bucket_epoch: 0,
            interval_seconds,
            bucket_count,
            values_by_sensor,
        };

        let target = alarm_engine::ResolvedTarget {
            target_key: format!("sensor:{sensor_id}"),
            sensor_ids: vec![sensor_id.clone()],
            node_id: None,
            primary_sensor_id: Some(sensor_id.clone()),
            match_mode: MatchMode::PerSensor,
        };

        let mut last_seen_epoch_by_sensor: HashMap<&str, i64> = HashMap::new();
        let mut state_window: Map<String, JsonValue> = Map::new();
        let mut currently_firing = false;
        let mut transitions: Vec<(i64, String)> = Vec::new();

        for idx in 0..bucket_count {
            let now_epoch = (idx as i64 + 1).saturating_mul(interval_seconds);
            let now = Utc
                .timestamp_opt(now_epoch, 0)
                .single()
                .unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap());

            if series.value_at(&sensor_id, idx).is_some() {
                last_seen_epoch_by_sensor.insert(sensor_id.as_str(), now.timestamp());
            }

            let (should_fire_now, _observed_value) = eval_condition_backtest(
                condition,
                &target,
                now,
                &series,
                idx,
                &last_seen_epoch_by_sensor,
                &mut state_window,
                "root",
            );

            let desired_firing = alarm_engine::apply_firing_timing(
                should_fire_now,
                currently_firing,
                timing,
                now,
                &mut state_window,
            );

            if desired_firing && !currently_firing {
                transitions.push((now_epoch, "fired".to_string()));
            } else if !desired_firing && currently_firing {
                transitions.push((now_epoch, "resolved".to_string()));
            }
            currently_firing = desired_firing;
        }

        transitions
    }

    #[test]
    fn backtest_respects_debounce_and_clear_hysteresis() {
        let condition = ConditionNode::Threshold {
            op: CompareOp::Gt,
            value: 10.0,
        };
        let timing = TimingConfig {
            debounce_seconds: 120,
            clear_hysteresis_seconds: 120,
            eval_interval_seconds: 60,
        };

        let transitions = simulate_condition_series(
            &condition,
            &timing,
            vec![
                Some(0.0),
                Some(11.0),
                Some(11.0),
                Some(11.0),
                Some(0.0),
                Some(0.0),
                Some(0.0),
            ],
            60,
        );

        assert_eq!(
            transitions,
            vec![(240, "fired".to_string()), (420, "resolved".to_string())]
        );
    }
}
