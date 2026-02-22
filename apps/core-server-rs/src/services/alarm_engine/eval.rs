use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use serde::Serialize;
use serde_json::{json, Map, Value as JsonValue};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

use super::types::{
    compare, AggregateOp, BaselineOp, ConditionNode, ConsecutivePeriod, DeviationMode, MatchMode,
    RangeMode, RuleEnvelope, TargetSelector,
};

#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub target_key: String,
    pub sensor_ids: Vec<String>,
    pub node_id: Option<Uuid>,
    pub primary_sensor_id: Option<String>,
    pub match_mode: MatchMode,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct PreviewTargetResult {
    pub target_key: String,
    pub sensor_ids: Vec<String>,
    pub passed: bool,
    pub observed_value: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct TargetEvaluation {
    pub passed: bool,
    pub observed_value: Option<f64>,
    pub window_state: JsonValue,
    pub consecutive_hits: i32,
}

#[derive(Debug, Clone)]
struct EvalOutcome {
    passed: bool,
    observed_value: Option<f64>,
}

#[derive(Debug, Clone)]
struct LatestPoint {
    ts: DateTime<Utc>,
    value: f64,
}

#[derive(Debug, Clone)]
struct WindowStats {
    avg: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    stddev: Option<f64>,
    median: Option<f64>,
}

#[derive(Debug, Clone, FromRow)]
struct SensorRow {
    sensor_id: String,
    node_id: Uuid,
}

#[derive(Debug, Clone, FromRow)]
struct LatestRow {
    sensor_id: String,
    ts: DateTime<Utc>,
    value: f64,
}

#[derive(Debug, Clone, FromRow)]
struct WindowStatsRow {
    sensor_id: String,
    avg_value: Option<f64>,
    min_value: Option<f64>,
    max_value: Option<f64>,
    stddev_value: Option<f64>,
    median_value: Option<f64>,
}

pub async fn resolve_targets(pool: &PgPool, selector: &TargetSelector) -> Result<Vec<ResolvedTarget>> {
    match selector {
        TargetSelector::Sensor { sensor_id } => {
            let row: Option<SensorRow> = sqlx::query_as(
                r#"
                SELECT sensor_id, node_id
                FROM sensors
                WHERE sensor_id = $1
                  AND deleted_at IS NULL
                "#,
            )
            .bind(sensor_id.trim())
            .fetch_optional(pool)
            .await?;

            let Some(row) = row else {
                return Ok(Vec::new());
            };

            Ok(vec![ResolvedTarget {
                target_key: format!("sensor:{}", row.sensor_id),
                sensor_ids: vec![row.sensor_id.clone()],
                node_id: Some(row.node_id),
                primary_sensor_id: Some(row.sensor_id),
                match_mode: MatchMode::PerSensor,
            }])
        }
        TargetSelector::SensorSet { sensor_ids, r#match } => {
            let cleaned: Vec<String> = sensor_ids
                .iter()
                .map(|sensor_id| sensor_id.trim().to_string())
                .filter(|sensor_id| !sensor_id.is_empty())
                .collect();
            if cleaned.is_empty() {
                return Ok(Vec::new());
            }
            let rows: Vec<SensorRow> = sqlx::query_as(
                r#"
                SELECT sensor_id, node_id
                FROM sensors
                WHERE sensor_id = ANY($1)
                  AND deleted_at IS NULL
                ORDER BY sensor_id ASC
                "#,
            )
            .bind(&cleaned)
            .fetch_all(pool)
            .await?;
            Ok(rows_to_targets(rows, *r#match))
        }
        TargetSelector::NodeSensors {
            node_id,
            types,
            r#match,
        } => {
            let rows: Vec<SensorRow> = if types.is_empty() {
                sqlx::query_as(
                    r#"
                    SELECT sensor_id, node_id
                    FROM sensors
                    WHERE node_id = $1
                      AND deleted_at IS NULL
                    ORDER BY sensor_id ASC
                    "#,
                )
                .bind(*node_id)
                .fetch_all(pool)
                .await?
            } else {
                let type_values: Vec<String> = types
                    .iter()
                    .map(|sensor_type| sensor_type.trim().to_string())
                    .filter(|sensor_type| !sensor_type.is_empty())
                    .collect();
                sqlx::query_as(
                    r#"
                    SELECT sensor_id, node_id
                    FROM sensors
                    WHERE node_id = $1
                      AND type = ANY($2)
                      AND deleted_at IS NULL
                    ORDER BY sensor_id ASC
                    "#,
                )
                .bind(*node_id)
                .bind(&type_values)
                .fetch_all(pool)
                .await?
            };
            Ok(rows_to_targets(rows, *r#match))
        }
        TargetSelector::Filter {
            provider,
            metric,
            sensor_type,
            r#match,
        } => {
            let provider = provider
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let metric = metric
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);
            let sensor_type = sensor_type
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);

            let mut qb = QueryBuilder::<Postgres>::new(
                "SELECT sensor_id, node_id FROM sensors WHERE deleted_at IS NULL",
            );
            if let Some(provider) = provider {
                qb.push(" AND NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') = ")
                    .push_bind(provider);
            }
            if let Some(metric) = metric {
                qb.push(" AND NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'metric'), '') = ")
                    .push_bind(metric);
            }
            if let Some(sensor_type) = sensor_type {
                qb.push(" AND type = ").push_bind(sensor_type);
            }
            qb.push(" ORDER BY sensor_id ASC");

            let rows: Vec<SensorRow> = qb.build_query_as().fetch_all(pool).await?;
            Ok(rows_to_targets(rows, *r#match))
        }
    }
}

fn rows_to_targets(rows: Vec<SensorRow>, mode: MatchMode) -> Vec<ResolvedTarget> {
    if rows.is_empty() {
        return Vec::new();
    }

    match mode {
        MatchMode::PerSensor => rows
            .into_iter()
            .map(|row| ResolvedTarget {
                target_key: format!("sensor:{}", row.sensor_id),
                sensor_ids: vec![row.sensor_id.clone()],
                node_id: Some(row.node_id),
                primary_sensor_id: Some(row.sensor_id),
                match_mode: MatchMode::PerSensor,
            })
            .collect(),
        MatchMode::Any | MatchMode::All => {
            let sensor_ids: Vec<String> = rows.iter().map(|row| row.sensor_id.clone()).collect();
            let node_id = rows
                .first()
                .map(|row| row.node_id)
                .filter(|candidate| rows.iter().all(|row| row.node_id == *candidate));
            vec![ResolvedTarget {
                target_key: format!("selector:{:?}:{}", mode, sensor_ids.join(",")),
                sensor_ids,
                node_id,
                primary_sensor_id: None,
                match_mode: mode,
            }]
        }
    }
}

pub async fn preview_rule(pool: &PgPool, envelope: &RuleEnvelope) -> Result<Vec<PreviewTargetResult>> {
    let targets = resolve_targets(pool, &envelope.target_selector).await?;
    let now = Utc::now();
    let mut out: Vec<PreviewTargetResult> = Vec::with_capacity(targets.len());

    for target in targets {
        let evaluation = evaluate_target(pool, &envelope.condition, &target, now, JsonValue::Null).await?;
        out.push(PreviewTargetResult {
            target_key: target.target_key,
            sensor_ids: target.sensor_ids,
            passed: evaluation.passed,
            observed_value: evaluation.observed_value,
        });
    }

    Ok(out)
}

pub async fn evaluate_target(
    pool: &PgPool,
    condition: &ConditionNode,
    target: &ResolvedTarget,
    now: DateTime<Utc>,
    state_window: JsonValue,
) -> Result<TargetEvaluation> {
    let latest_map = fetch_latest_map(pool, &target.sensor_ids).await?;
    let mut window_cache: HashMap<i64, HashMap<String, WindowStats>> = HashMap::new();
    let mut window_state = state_window.as_object().cloned().unwrap_or_default();

    let outcome = eval_condition(
        pool,
        condition,
        target,
        now,
        &latest_map,
        &mut window_cache,
        &mut window_state,
        "root",
    )
    .await?;

    let consecutive_hits = window_state
        .get("consecutive_hits")
        .and_then(JsonValue::as_i64)
        .unwrap_or(0) as i32;

    Ok(TargetEvaluation {
        passed: outcome.passed,
        observed_value: outcome.observed_value,
        window_state: JsonValue::Object(window_state),
        consecutive_hits,
    })
}

fn eval_condition<'a>(
    pool: &'a PgPool,
    node: &'a ConditionNode,
    target: &'a ResolvedTarget,
    now: DateTime<Utc>,
    latest_map: &'a HashMap<String, LatestPoint>,
    window_cache: &'a mut HashMap<i64, HashMap<String, WindowStats>>,
    state_window: &'a mut Map<String, JsonValue>,
    path: &'a str,
) -> Pin<Box<dyn Future<Output = Result<EvalOutcome>> + Send + 'a>> {
    Box::pin(async move {
    match node {
        ConditionNode::Threshold { op, value } => {
            let values: Vec<f64> = target
                .sensor_ids
                .iter()
                .filter_map(|sensor_id| latest_map.get(sensor_id).map(|point| point.value))
                .collect();
            Ok(eval_values(values, target.match_mode, |sample| compare(sample, *op, *value)))
        }
        ConditionNode::Range { mode, low, high } => {
            let values: Vec<f64> = target
                .sensor_ids
                .iter()
                .filter_map(|sensor_id| latest_map.get(sensor_id).map(|point| point.value))
                .collect();
            Ok(eval_values(values, target.match_mode, |sample| {
                let inside = sample >= *low && sample <= *high;
                match mode {
                    RangeMode::Inside => inside,
                    RangeMode::Outside => !inside,
                }
            }))
        }
        ConditionNode::Offline {
            missing_for_seconds,
        } => {
            let statuses: Vec<bool> = target
                .sensor_ids
                .iter()
                .map(|sensor_id| {
                    if let Some(point) = latest_map.get(sensor_id) {
                        (now - point.ts).num_seconds() >= *missing_for_seconds
                    } else {
                        true
                    }
                })
                .collect();
            let passed = match target.match_mode {
                MatchMode::All => statuses.iter().all(|value| *value),
                MatchMode::PerSensor | MatchMode::Any => statuses.iter().any(|value| *value),
            };
            Ok(EvalOutcome {
                passed,
                observed_value: None,
            })
        }
        ConditionNode::RollingWindow {
            window_seconds,
            aggregate,
            op,
            value,
        } => {
            let stats_map = get_window_stats(pool, target, now, *window_seconds, window_cache).await?;
            let mut samples: Vec<f64> = Vec::new();
            for sensor_id in &target.sensor_ids {
                let Some(stats) = stats_map.get(sensor_id) else {
                    continue;
                };
                let sample = match aggregate {
                    AggregateOp::Avg => stats.avg,
                    AggregateOp::Min => stats.min,
                    AggregateOp::Max => stats.max,
                    AggregateOp::Stddev => stats.stddev,
                };
                if let Some(sample) = sample {
                    samples.push(sample);
                }
            }
            Ok(eval_values(samples, target.match_mode, |sample| compare(sample, *op, *value)))
        }
        ConditionNode::Deviation {
            window_seconds,
            baseline,
            mode,
            value,
        } => {
            let stats_map = get_window_stats(pool, target, now, *window_seconds, window_cache).await?;
            let mut samples: Vec<f64> = Vec::new();
            for sensor_id in &target.sensor_ids {
                let Some(current) = latest_map.get(sensor_id).map(|point| point.value) else {
                    continue;
                };
                let Some(stats) = stats_map.get(sensor_id) else {
                    continue;
                };
                let baseline_value = match baseline {
                    BaselineOp::Mean => stats.avg,
                    BaselineOp::Median => stats.median,
                };
                let Some(baseline_value) = baseline_value else {
                    continue;
                };
                let delta = (current - baseline_value).abs();
                let deviation = match mode {
                    DeviationMode::Absolute => delta,
                    DeviationMode::Percent => {
                        if baseline_value.abs() <= f64::EPSILON {
                            continue;
                        }
                        (delta / baseline_value.abs()) * 100.0
                    }
                };
                samples.push(deviation);
            }
            Ok(eval_values(samples, target.match_mode, |sample| sample >= *value))
        }
        ConditionNode::ConsecutivePeriods {
            period,
            count,
            child,
        } => {
            let child_path = format!("{path}.cp");
            let child_outcome = eval_condition(
                pool,
                child,
                target,
                now,
                latest_map,
                window_cache,
                state_window,
                &child_path,
            )
            .await?;

            let state_key = format!("cp:{path}");
            let mut state = state_window
                .get(&state_key)
                .and_then(JsonValue::as_object)
                .cloned()
                .unwrap_or_default();

            let current_period = period_bucket(*period, now);
            let mut streak = state
                .get("streak")
                .and_then(JsonValue::as_i64)
                .unwrap_or(0);
            let last_period = state.get("last_period").and_then(JsonValue::as_i64);

            if child_outcome.passed {
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

            Ok(EvalOutcome {
                passed: streak >= *count as i64,
                observed_value: child_outcome.observed_value,
            })
        }
        ConditionNode::All { children } => {
            let mut observed = None;
            for (index, child) in children.iter().enumerate() {
                let child_path = format!("{path}.all[{index}]");
                let outcome = eval_condition(
                    pool,
                    child,
                    target,
                    now,
                    latest_map,
                    window_cache,
                    state_window,
                    &child_path,
                )
                .await?;
                if observed.is_none() {
                    observed = outcome.observed_value;
                }
                if !outcome.passed {
                    return Ok(EvalOutcome {
                        passed: false,
                        observed_value: observed,
                    });
                }
            }
            Ok(EvalOutcome {
                passed: true,
                observed_value: observed,
            })
        }
        ConditionNode::Any { children } => {
            let mut observed = None;
            for (index, child) in children.iter().enumerate() {
                let child_path = format!("{path}.any[{index}]");
                let outcome = eval_condition(
                    pool,
                    child,
                    target,
                    now,
                    latest_map,
                    window_cache,
                    state_window,
                    &child_path,
                )
                .await?;
                if observed.is_none() {
                    observed = outcome.observed_value;
                }
                if outcome.passed {
                    return Ok(EvalOutcome {
                        passed: true,
                        observed_value: observed,
                    });
                }
            }
            Ok(EvalOutcome {
                passed: false,
                observed_value: observed,
            })
        }
        ConditionNode::Not { child } => {
            let child_path = format!("{path}.not");
            let outcome = eval_condition(
                pool,
                child,
                target,
                now,
                latest_map,
                window_cache,
                state_window,
                &child_path,
            )
            .await?;
            Ok(EvalOutcome {
                passed: !outcome.passed,
                observed_value: outcome.observed_value,
            })
        }
    }
    })
}

fn period_bucket(period: ConsecutivePeriod, now: DateTime<Utc>) -> i64 {
    match period {
        ConsecutivePeriod::Eval => now.timestamp(),
        ConsecutivePeriod::Hour => now.timestamp() / 3600,
        ConsecutivePeriod::Day => now.date_naive().num_days_from_ce() as i64,
    }
}

fn eval_values<F>(values: Vec<f64>, mode: MatchMode, predicate: F) -> EvalOutcome
where
    F: Fn(f64) -> bool,
{
    if values.is_empty() {
        return EvalOutcome {
            passed: false,
            observed_value: None,
        };
    }

    let observed_value = values.first().copied();
    let passed = match mode {
        MatchMode::All => values.iter().all(|value| predicate(*value)),
        MatchMode::PerSensor | MatchMode::Any => values.iter().any(|value| predicate(*value)),
    };

    EvalOutcome {
        passed,
        observed_value,
    }
}

async fn fetch_latest_map(pool: &PgPool, sensor_ids: &[String]) -> Result<HashMap<String, LatestPoint>> {
    if sensor_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows: Vec<LatestRow> = sqlx::query_as(
        r#"
        SELECT DISTINCT ON (sensor_id)
            sensor_id,
            ts,
            value
        FROM metrics
        WHERE sensor_id = ANY($1)
        ORDER BY sensor_id, ts DESC
        "#,
    )
    .bind(sensor_ids)
    .fetch_all(pool)
    .await?;

    let mut out: HashMap<String, LatestPoint> = HashMap::new();
    for row in rows {
        out.insert(
            row.sensor_id,
            LatestPoint {
                ts: row.ts,
                value: row.value,
            },
        );
    }
    Ok(out)
}

async fn get_window_stats(
    pool: &PgPool,
    target: &ResolvedTarget,
    now: DateTime<Utc>,
    window_seconds: i64,
    cache: &mut HashMap<i64, HashMap<String, WindowStats>>,
) -> Result<HashMap<String, WindowStats>> {
    if let Some(cached) = cache.get(&window_seconds) {
        return Ok(cached.clone());
    }

    let start = now - chrono::Duration::seconds(window_seconds.max(1));
    let rows: Vec<WindowStatsRow> = sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            avg(value) as avg_value,
            min(value) as min_value,
            max(value) as max_value,
            COALESCE(stddev_pop(value), 0) as stddev_value,
            percentile_cont(0.5) within group (order by value) as median_value
        FROM metrics
        WHERE sensor_id = ANY($1)
          AND ts >= $2
        GROUP BY sensor_id
        "#,
    )
    .bind(&target.sensor_ids)
    .bind(start)
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<String, WindowStats> = HashMap::new();
    for row in rows {
        map.insert(
            row.sensor_id,
            WindowStats {
                avg: row.avg_value,
                min: row.min_value,
                max: row.max_value,
                stddev: row.stddev_value,
                median: row.median_value,
            },
        );
    }

    cache.insert(window_seconds, map.clone());
    Ok(map)
}

pub fn apply_firing_timing(
    should_fire_now: bool,
    currently_firing: bool,
    timing: &super::types::TimingConfig,
    now: DateTime<Utc>,
    window_state: &mut Map<String, JsonValue>,
) -> bool {
    let now_epoch = now.timestamp();

    if should_fire_now {
        window_state.remove("first_false_at");
        if currently_firing {
            return true;
        }

        if timing.debounce_seconds <= 0 {
            window_state.remove("first_true_at");
            return true;
        }

        let first_true = window_state
            .get("first_true_at")
            .and_then(JsonValue::as_i64)
            .unwrap_or_else(|| {
                window_state.insert("first_true_at".to_string(), JsonValue::from(now_epoch));
                now_epoch
            });

        return now_epoch - first_true >= timing.debounce_seconds;
    }

    window_state.remove("first_true_at");
    if !currently_firing {
        window_state.remove("first_false_at");
        return false;
    }

    if timing.clear_hysteresis_seconds <= 0 {
        window_state.remove("first_false_at");
        return false;
    }

    let first_false = window_state
        .get("first_false_at")
        .and_then(JsonValue::as_i64)
        .unwrap_or_else(|| {
            window_state.insert("first_false_at".to_string(), JsonValue::from(now_epoch));
            now_epoch
        });

    now_epoch - first_false < timing.clear_hysteresis_seconds
}

pub fn rule_payload(
    condition: &ConditionNode,
    timing: &super::types::TimingConfig,
    severity: &str,
) -> JsonValue {
    json!({
        "type": "rule",
        "severity": severity,
        "condition": condition,
        "timing": timing,
    })
}
