use anyhow::Result;
use chrono::Utc;
use serde_json::{Map, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use sqlx::{FromRow, PgPool};
use std::collections::HashSet;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

mod eval;
pub mod types;

pub use eval::{apply_firing_timing, resolve_targets, PreviewTargetResult, ResolvedTarget};

#[derive(Debug, Clone)]
pub struct AlarmEngineService {
    pool: PgPool,
    poll_interval: Duration,
}

impl AlarmEngineService {
    pub fn new(pool: PgPool, poll_interval_seconds: u64) -> Self {
        Self {
            pool,
            poll_interval: Duration::from_secs(poll_interval_seconds.max(5)),
        }
    }

    pub fn start(self, cancel: CancellationToken) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.poll_interval);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = interval.tick() => {
                        if let Err(err) = evaluate_rules_now(&self.pool, None).await {
                            tracing::warn!(error = %err, "alarm engine tick failed");
                        }
                    }
                }
            }
        });
    }
}

#[derive(Debug, Clone, FromRow)]
struct AlarmRuleRow {
    id: i64,
    name: String,
    severity: String,
    origin: String,
    target_selector: SqlJson<JsonValue>,
    condition_ast: SqlJson<JsonValue>,
    timing: SqlJson<JsonValue>,
    message_template: String,
}

#[derive(Debug, Clone, FromRow)]
struct AlarmRuleStateRow {
    currently_firing: bool,
    window_state: SqlJson<JsonValue>,
    last_eval_at: Option<chrono::DateTime<chrono::Utc>>,
    last_transition_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, FromRow)]
struct ExistingAlarmRow {
    id: i64,
}

pub async fn preview_rule(
    pool: &PgPool,
    target_selector: &JsonValue,
    condition_ast: &JsonValue,
    timing: &JsonValue,
) -> Result<Vec<PreviewTargetResult>, String> {
    let envelope = types::parse_rule_envelope(target_selector, condition_ast, timing)
        .map_err(|err| format!("Rule validation failed: {err}"))?;
    eval::preview_rule(pool, &envelope)
        .await
        .map_err(|err| format!("Preview failed: {err}"))
}

pub fn schedule_evaluate_for_sensors(pool: PgPool, sensor_ids: Vec<String>) {
    if sensor_ids.is_empty() {
        return;
    }
    let mut deduped: Vec<String> = sensor_ids
        .into_iter()
        .map(|sensor_id| sensor_id.trim().to_string())
        .filter(|sensor_id| !sensor_id.is_empty())
        .collect();
    deduped.sort();
    deduped.dedup();
    if deduped.is_empty() {
        return;
    }

    tokio::spawn(async move {
        if let Err(err) = evaluate_rules_now(&pool, Some(&deduped)).await {
            tracing::warn!(error = %err, "alarm fast-path evaluation failed");
        }
    });
}

pub async fn evaluate_rules_now(pool: &PgPool, only_sensor_ids: Option<&[String]>) -> Result<usize> {
    let rules: Vec<AlarmRuleRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            severity,
            origin,
            target_selector,
            condition_ast,
            timing,
            message_template
        FROM alarm_rules
        WHERE enabled = TRUE
          AND deleted_at IS NULL
        ORDER BY id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rules.is_empty() {
        return Ok(0);
    }

    let sensor_filter: Option<HashSet<String>> = only_sensor_ids.map(|values| {
        values
            .iter()
            .map(|sensor_id| sensor_id.trim().to_string())
            .filter(|sensor_id| !sensor_id.is_empty())
            .collect::<HashSet<String>>()
    });

    let now = Utc::now();
    let mut transitions: usize = 0;

    for rule in rules {
        let envelope = match types::parse_rule_envelope(
            &rule.target_selector.0,
            &rule.condition_ast.0,
            &rule.timing.0,
        ) {
            Ok(envelope) => envelope,
            Err(err) => {
                upsert_rule_error_state(pool, rule.id, &format!("invalid_rule: {err}")).await?;
                continue;
            }
        };

        if let Some(sensor_filter) = &sensor_filter {
            if !rule_matches_sensor_filter(&envelope.target_selector, sensor_filter) {
                continue;
            }
        }

        let targets = eval::resolve_targets(pool, &envelope.target_selector).await?;
        if targets.is_empty() {
            continue;
        }

        for target in targets {
            let state_row: Option<AlarmRuleStateRow> = sqlx::query_as(
                r#"
                SELECT currently_firing, window_state, last_eval_at, last_transition_at
                FROM alarm_rule_state
                WHERE rule_id = $1 AND target_key = $2
                "#,
            )
            .bind(rule.id)
            .bind(&target.target_key)
            .fetch_optional(pool)
            .await?;

            let eval_interval_seconds = envelope.timing.eval_interval_seconds.max(0);
            if eval_interval_seconds > 0 {
                if let Some(last_eval_at) = state_row.as_ref().and_then(|row| row.last_eval_at) {
                    if (now - last_eval_at).num_seconds() < eval_interval_seconds {
                        continue;
                    }
                }
            }

            let currently_firing = state_row
                .as_ref()
                .map(|state| state.currently_firing)
                .unwrap_or(false);
            let initial_window_state = state_row
                .as_ref()
                .map(|state| state.window_state.0.clone())
                .unwrap_or(JsonValue::Object(Map::new()));

            let evaluation = match eval::evaluate_target(
                pool,
                &envelope.condition,
                &target,
                now,
                initial_window_state,
            )
            .await
            {
                Ok(evaluation) => evaluation,
                Err(err) => {
                    upsert_rule_target_state_error(pool, rule.id, &target.target_key, &err.to_string()).await?;
                    continue;
                }
            };

            let mut window_state = evaluation
                .window_state
                .as_object()
                .cloned()
                .unwrap_or_default();

            let desired_firing = eval::apply_firing_timing(
                evaluation.passed,
                currently_firing,
                &envelope.timing,
                now,
                &mut window_state,
            );

            let transition_happened = if desired_firing && !currently_firing {
                transition_to_firing(
                    pool,
                    rule.id,
                    &rule,
                    &envelope,
                    &target,
                    evaluation.observed_value,
                    now,
                )
                .await?;
                true
            } else if !desired_firing && currently_firing {
                transition_to_ok(
                    pool,
                    rule.id,
                    &rule,
                    &target,
                    evaluation.observed_value,
                    now,
                )
                .await?;
                true
            } else {
                false
            };

            if transition_happened {
                transitions = transitions.saturating_add(1);
            }

            let last_transition_at = if transition_happened {
                Some(now)
            } else {
                state_row.and_then(|row| row.last_transition_at)
            };

            sqlx::query(
                r#"
                INSERT INTO alarm_rule_state (
                    rule_id,
                    target_key,
                    currently_firing,
                    consecutive_hits,
                    window_state,
                    last_eval_at,
                    last_value,
                    last_transition_at,
                    error
                )
                VALUES ($1, $2, $3, $4, $5, NOW(), $6, $7, NULL)
                ON CONFLICT (rule_id, target_key)
                DO UPDATE SET
                    currently_firing = EXCLUDED.currently_firing,
                    consecutive_hits = EXCLUDED.consecutive_hits,
                    window_state = EXCLUDED.window_state,
                    last_eval_at = EXCLUDED.last_eval_at,
                    last_value = EXCLUDED.last_value,
                    last_transition_at = COALESCE(EXCLUDED.last_transition_at, alarm_rule_state.last_transition_at),
                    error = NULL
                "#,
            )
            .bind(rule.id)
            .bind(&target.target_key)
            .bind(desired_firing)
            .bind(evaluation.consecutive_hits)
            .bind(SqlJson(JsonValue::Object(window_state)))
            .bind(evaluation.observed_value)
            .bind(last_transition_at)
            .execute(pool)
            .await?;
        }
    }

    Ok(transitions)
}

fn rule_matches_sensor_filter(
    selector: &types::TargetSelector,
    sensor_filter: &HashSet<String>,
) -> bool {
    match selector {
        types::TargetSelector::Sensor { sensor_id } => sensor_filter.contains(sensor_id),
        types::TargetSelector::SensorSet { sensor_ids, .. } => {
            sensor_ids.iter().any(|sensor_id| sensor_filter.contains(sensor_id))
        }
        types::TargetSelector::NodeSensors { .. } | types::TargetSelector::Filter { .. } => true,
    }
}

async fn upsert_rule_error_state(pool: &PgPool, rule_id: i64, message: &str) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO alarm_rule_state (
            rule_id,
            target_key,
            currently_firing,
            consecutive_hits,
            window_state,
            last_eval_at,
            error
        )
        VALUES ($1, '__rule__', FALSE, 0, '{}'::jsonb, NOW(), $2)
        ON CONFLICT (rule_id, target_key)
        DO UPDATE SET
            last_eval_at = EXCLUDED.last_eval_at,
            error = EXCLUDED.error
        "#,
    )
    .bind(rule_id)
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}

async fn upsert_rule_target_state_error(
    pool: &PgPool,
    rule_id: i64,
    target_key: &str,
    message: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO alarm_rule_state (
            rule_id,
            target_key,
            currently_firing,
            consecutive_hits,
            window_state,
            last_eval_at,
            error
        )
        VALUES ($1, $2, FALSE, 0, '{}'::jsonb, NOW(), $3)
        ON CONFLICT (rule_id, target_key)
        DO UPDATE SET
            last_eval_at = EXCLUDED.last_eval_at,
            error = EXCLUDED.error
        "#,
    )
    .bind(rule_id)
    .bind(target_key)
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}

async fn transition_to_firing(
    pool: &PgPool,
    rule_id: i64,
    rule: &AlarmRuleRow,
    envelope: &types::RuleEnvelope,
    target: &eval::ResolvedTarget,
    observed_value: Option<f64>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    let existing: Option<ExistingAlarmRow> = sqlx::query_as(
        r#"
        SELECT id
        FROM alarms
        WHERE rule_id = $1 AND target_key = $2
        LIMIT 1
        "#,
    )
    .bind(rule_id)
    .bind(&target.target_key)
    .fetch_optional(&mut *tx)
    .await?;

    let rule_payload = eval::rule_payload(&envelope.condition, &envelope.timing, &rule.severity);
    let sensor_id = target.primary_sensor_id.as_deref();
    let node_id = target.node_id;

    let alarm_id = if let Some(existing) = existing {
        sqlx::query(
            r#"
            UPDATE alarms
            SET
                name = $2,
                rule = $3,
                status = 'firing',
                sensor_id = $4,
                node_id = $5,
                origin = $6,
                rule_id = $7,
                target_key = $8,
                last_fired = $9,
                resolved_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(existing.id)
        .bind(&rule.name)
        .bind(SqlJson(rule_payload))
        .bind(sensor_id)
        .bind(node_id)
        .bind(&rule.origin)
        .bind(rule_id)
        .bind(&target.target_key)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        existing.id
    } else {
        let inserted: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO alarms (
                name,
                rule,
                status,
                sensor_id,
                node_id,
                origin,
                rule_id,
                target_key,
                last_fired,
                resolved_at
            )
            VALUES ($1, $2, 'firing', $3, $4, $5, $6, $7, $8, NULL)
            RETURNING id
            "#,
        )
        .bind(&rule.name)
        .bind(SqlJson(rule_payload))
        .bind(sensor_id)
        .bind(node_id)
        .bind(&rule.origin)
        .bind(rule_id)
        .bind(&target.target_key)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        inserted.0
    };

    let message = if rule.message_template.trim().is_empty() {
        format!("{} triggered", rule.name)
    } else {
        rule.message_template.trim().to_string()
    };

    sqlx::query(
        r#"
        INSERT INTO alarm_events (
            alarm_id,
            rule_id,
            sensor_id,
            node_id,
            status,
            message,
            origin,
            anomaly_score,
            transition,
            incident_id,
            target_key
        )
        VALUES ($1, $2, $3, $4, 'firing', $5, $6, $7, 'fired', $8, $9)
        "#,
    )
    .bind(alarm_id)
    .bind(rule_id)
    .bind(sensor_id)
    .bind(node_id)
    .bind(message)
    .bind(&rule.origin)
    .bind(observed_value)
    .bind(
        crate::services::incidents::get_or_create_incident(
            &mut tx,
            now,
            &crate::services::incidents::IncidentKey {
                rule_id: Some(rule_id),
                target_key: Some(target.target_key.clone()),
            },
            &rule.severity,
            &rule.name,
            "fired",
        )
        .await?,
    )
    .bind(&target.target_key)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

async fn transition_to_ok(
    pool: &PgPool,
    rule_id: i64,
    rule: &AlarmRuleRow,
    target: &eval::ResolvedTarget,
    observed_value: Option<f64>,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    let existing: Option<ExistingAlarmRow> = sqlx::query_as(
        r#"
        SELECT id
        FROM alarms
        WHERE rule_id = $1 AND target_key = $2
        LIMIT 1
        "#,
    )
    .bind(rule_id)
    .bind(&target.target_key)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(existing) = existing else {
        tx.commit().await?;
        return Ok(());
    };

    sqlx::query(
        r#"
        UPDATE alarms
        SET
            status = 'ok',
            resolved_at = $2
        WHERE id = $1
        "#,
    )
    .bind(existing.id)
    .bind(now)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO alarm_events (
            alarm_id,
            rule_id,
            sensor_id,
            node_id,
            status,
            message,
            origin,
            anomaly_score,
            transition,
            incident_id,
            target_key
        )
        VALUES ($1, $2, $3, $4, 'ok', $5, $6, $7, 'resolved', $8, $9)
        "#,
    )
    .bind(existing.id)
    .bind(rule_id)
    .bind(target.primary_sensor_id.as_deref())
    .bind(target.node_id)
    .bind(format!("{} resolved", rule.name))
    .bind(&rule.origin)
    .bind(observed_value)
    .bind(
        crate::services::incidents::get_or_create_incident(
            &mut tx,
            now,
            &crate::services::incidents::IncidentKey {
                rule_id: Some(rule_id),
                target_key: Some(target.target_key.clone()),
            },
            &rule.severity,
            &rule.name,
            "resolved",
        )
        .await?,
    )
    .bind(&target.target_key)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}
