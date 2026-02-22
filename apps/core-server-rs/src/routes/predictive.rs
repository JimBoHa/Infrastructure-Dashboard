use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{internal_error, map_db_error};
use crate::state::AppState;

const PREDICTIVE_CREDENTIAL_NAME: &str = "predictive_alarms";
const PREDICTIVE_TRACE_LIMIT: i64 = 50;
const PREDICTIVE_BOOTSTRAP_MIN_SAMPLES: i64 = 12;
const PREDICTIVE_EVENT_COOLDOWN_SECONDS: i64 = 300;
const PREDICTIVE_SCORE_THRESHOLD: f64 = 0.7;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct PredictiveStatus {
    enabled: bool,
    running: bool,
    token_present: bool,
    api_base_url: String,
    model: Option<String>,
    fallback_models: Vec<String>,
    bootstrap_on_start: bool,
    bootstrap_max_sensors: i32,
    bootstrap_lookback_hours: i32,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct PredictiveTraceEntry {
    timestamp: String,
    code: String,
    output: String,
    model: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::IntoParams)]
pub(crate) struct PredictiveBootstrapQuery {
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct PredictiveBootstrapResponse {
    submitted_samples: i32,
    #[serde(default)]
    predictions: i32,
}

#[derive(Debug, Clone, serde::Deserialize, Default, utoipa::ToSchema)]
pub(crate) struct PredictiveConfigUpdate {
    enabled: Option<bool>,
    api_base_url: Option<String>,
    model: Option<Option<String>>,
    api_token: Option<String>,
}

#[derive(sqlx::FromRow)]
struct CredentialRow {
    value: String,
    metadata: SqlJson<JsonValue>,
}

#[utoipa::path(
    get,
    path = "/api/predictive/trace",
    tag = "predictive",
    responses(
        (status = 200, description = "Predictive trace entries", body = Vec<PredictiveTraceEntry>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn predictive_trace(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<PredictiveTraceEntry>>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let rows: Vec<(DateTime<Utc>, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT recorded_at, code, output, model
        FROM predictive_trace
        ORDER BY recorded_at DESC
        LIMIT $1
        "#,
    )
    .bind(PREDICTIVE_TRACE_LIMIT)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(
        rows.into_iter()
            .map(|(recorded_at, code, output, model)| PredictiveTraceEntry {
                timestamp: recorded_at.to_rfc3339(),
                code,
                output,
                model,
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/predictive/status",
    tag = "predictive",
    responses(
        (status = 200, description = "Predictive status", body = PredictiveStatus),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn predictive_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<PredictiveStatus>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;
    Ok(Json(load_status(&state.db).await.map_err(internal_error)?))
}

#[utoipa::path(
    put,
    path = "/api/predictive/config",
    tag = "predictive",
    request_body = PredictiveConfigUpdate,
    responses(
        (status = 200, description = "Predictive status", body = PredictiveStatus),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_config(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<PredictiveConfigUpdate>,
) -> Result<Json<PredictiveStatus>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let existing: Option<CredentialRow> = sqlx::query_as(
        r#"
        SELECT value, metadata
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind(PREDICTIVE_CREDENTIAL_NAME)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut value = existing
        .as_ref()
        .map(|row| row.value.clone())
        .unwrap_or_default();
    let mut metadata = existing
        .as_ref()
        .map(|row| row.metadata.0.clone())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(enabled) = payload.enabled {
        metadata["enabled"] = JsonValue::Bool(enabled);
    }
    if let Some(api_base_url) = payload.api_base_url {
        metadata["api_base_url"] = JsonValue::String(api_base_url.trim().to_string());
    }
    if let Some(model) = payload.model {
        metadata["model"] = model
            .map(|m| JsonValue::String(m.trim().to_string()))
            .unwrap_or(JsonValue::Null);
    }
    if let Some(token) = payload.api_token {
        value = token.trim().to_string();
    }

    let _ = sqlx::query(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        "#,
    )
    .bind(PREDICTIVE_CREDENTIAL_NAME)
    .bind(value)
    .bind(SqlJson(metadata))
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(load_status(&state.db).await.map_err(internal_error)?))
}

#[utoipa::path(
    post,
    path = "/api/predictive/bootstrap",
    operation_id = "predictive_bootstrap",
    tag = "predictive",
    params(PredictiveBootstrapQuery),
    responses(
        (status = 200, description = "Bootstrap result", body = PredictiveBootstrapResponse),
        (status = 400, description = "Predictive alarms are disabled"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn bootstrap(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<PredictiveBootstrapQuery>,
) -> Result<Json<PredictiveBootstrapResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let status = load_status(&state.db).await.map_err(internal_error)?;
    if !status.enabled {
        return Err((
            StatusCode::BAD_REQUEST,
            "Predictive alarms are disabled.".to_string(),
        ));
    }

    let now = Utc::now();
    let _ = append_trace(
        &state.db,
        "bootstrap_start",
        &format!(
            "bootstrap requested (force={}, lookback_hours={}, max_sensors={})",
            query.force, status.bootstrap_lookback_hours, status.bootstrap_max_sensors
        ),
        status.model.as_deref(),
    )
    .await;

    let (sensors_evaluated, alarms_fired) = run_bootstrap(
        &state.db,
        now,
        status.bootstrap_lookback_hours,
        status.bootstrap_max_sensors,
        query.force,
    )
    .await
    .map_err(internal_error)?;

    let _ = append_trace(
        &state.db,
        "bootstrap_done",
        &format!(
            "bootstrap finished: sensors_evaluated={sensors_evaluated} alarms_fired={alarms_fired}"
        ),
        status.model.as_deref(),
    )
    .await;

    Ok(Json(PredictiveBootstrapResponse {
        submitted_samples: sensors_evaluated,
        predictions: alarms_fired,
    }))
}

async fn append_trace(
    db: &sqlx::PgPool,
    code: &str,
    output: &str,
    model: Option<&str>,
) -> anyhow::Result<()> {
    let _ = sqlx::query(
        r#"
        INSERT INTO predictive_trace (recorded_at, code, output, model)
        VALUES (NOW(), $1, $2, $3)
        "#,
    )
    .bind(code.trim())
    .bind(output.trim())
    .bind(model.map(str::trim).filter(|v| !v.is_empty()))
    .execute(db)
    .await?;

    let _ = sqlx::query(
        r#"
        DELETE FROM predictive_trace
        WHERE id NOT IN (
            SELECT id
            FROM predictive_trace
            ORDER BY recorded_at DESC
            LIMIT $1
        )
        "#,
    )
    .bind(PREDICTIVE_TRACE_LIMIT)
    .execute(db)
    .await?;

    Ok(())
}

#[derive(sqlx::FromRow)]
struct SensorBootstrapRow {
    sensor_id: String,
    sensor_name: String,
    node_id: Uuid,
    sample_count: i64,
    mean: f64,
    stddev: Option<f64>,
    last_value: f64,
    last_ts: DateTime<Utc>,
}

async fn run_bootstrap(
    db: &sqlx::PgPool,
    now: DateTime<Utc>,
    lookback_hours: i32,
    max_sensors: i32,
    force: bool,
) -> anyhow::Result<(i32, i32)> {
    let lookback_hours = lookback_hours.max(1);
    let max_sensors = max_sensors.clamp(1, 500);
    let start = now - chrono::Duration::hours(lookback_hours as i64);

    let rows: Vec<SensorBootstrapRow> = sqlx::query_as(
        r#"
        WITH stats AS (
            SELECT
                sensor_id,
                AVG(value)::double precision as mean,
                STDDEV_POP(value)::double precision as stddev,
                COUNT(*)::bigint as sample_count,
                MAX(ts) as last_ts
            FROM metrics
            WHERE ts >= $1
            GROUP BY sensor_id
        ),
        latest AS (
            SELECT m.sensor_id, m.value::double precision as last_value, m.ts as last_ts
            FROM metrics m
            JOIN stats s
              ON s.sensor_id = m.sensor_id
             AND s.last_ts = m.ts
        ),
        selected AS (
            SELECT s.sensor_id, s.mean, s.stddev, s.sample_count, l.last_value, l.last_ts
            FROM stats s
            JOIN latest l ON l.sensor_id = s.sensor_id
            ORDER BY l.last_ts DESC
            LIMIT $2
        )
        SELECT
            selected.sensor_id as sensor_id,
            sensors.name as sensor_name,
            sensors.node_id as node_id,
            selected.sample_count as sample_count,
            selected.mean as mean,
            selected.stddev as stddev,
            selected.last_value as last_value,
            selected.last_ts as last_ts
        FROM selected
        JOIN sensors ON sensors.sensor_id = selected.sensor_id
        WHERE sensors.deleted_at IS NULL
        ORDER BY selected.last_ts DESC
        "#,
    )
    .bind(start)
    .bind(max_sensors as i64)
    .fetch_all(db)
    .await?;

    let mut tx = db.begin().await?;

    let mut sensors_evaluated: i32 = 0;
    let mut alarms_fired: i32 = 0;
    for row in rows {
        sensors_evaluated = sensors_evaluated.saturating_add(1);
        let Some((score, z)) = score_anomaly(&row) else {
            continue;
        };
        if score < PREDICTIVE_SCORE_THRESHOLD {
            continue;
        }

        let fired =
            upsert_predictive_alarm(&mut tx, now, &row, score, z, lookback_hours, force).await?;
        if fired {
            alarms_fired = alarms_fired.saturating_add(1);
        }
    }

    tx.commit().await?;
    Ok((sensors_evaluated, alarms_fired))
}

fn score_anomaly(row: &SensorBootstrapRow) -> Option<(f64, f64)> {
    if row.sample_count < PREDICTIVE_BOOTSTRAP_MIN_SAMPLES {
        return None;
    }

    let stddev = row.stddev.unwrap_or(0.0);
    let diff = (row.last_value - row.mean).abs();
    let z = if stddev.is_finite() && stddev > 1e-9 {
        diff / stddev
    } else if diff < 1e-9 {
        0.0
    } else {
        10.0
    };

    // Map z-score into a 0..1-ish anomaly score (sigmoid around z≈3).
    let score = 1.0 / (1.0 + (-(z - 3.0)).exp());
    Some((score.clamp(0.0, 1.0), z))
}

async fn upsert_predictive_alarm(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    now: DateTime<Utc>,
    row: &SensorBootstrapRow,
    score: f64,
    z: f64,
    lookback_hours: i32,
    force: bool,
) -> Result<bool, sqlx::Error> {
    let existing: Option<(i64, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT id, last_fired
        FROM alarms
        WHERE origin = 'predictive' AND sensor_id = $1
        LIMIT 1
        "#,
    )
    .bind(row.sensor_id.trim())
    .fetch_optional(&mut **tx)
    .await?;

    let rule = serde_json::json!({
        "type": "predictive",
        "method": "zscore",
        "lookback_hours": lookback_hours,
        "mean": row.mean,
        "stddev": row.stddev,
        "last_value": row.last_value,
        "last_ts": row.last_ts.to_rfc3339(),
        "z": z,
        "score": score,
    });
    let message = format!(
        "Predictive anomaly: score={score:.2} z={z:.2} last={:.3} mean={:.3} stddev={}",
        row.last_value,
        row.mean,
        row.stddev.unwrap_or(0.0),
    );

    let should_emit = existing
        .as_ref()
        .and_then(|(_, last_fired)| last_fired.as_ref().copied())
        .map(|last_fired| (now - last_fired).num_seconds() >= PREDICTIVE_EVENT_COOLDOWN_SECONDS)
        .unwrap_or(true)
        || force;

    let alarm_id = if let Some((alarm_id, last_fired)) = existing {
        let last_fired = if should_emit { Some(now) } else { last_fired };
        sqlx::query(
            r#"
            UPDATE alarms
            SET
                name = $2,
                rule = $3,
                status = 'firing',
                node_id = $4,
                anomaly_score = $5,
                last_fired = $6,
                origin = 'predictive'
            WHERE id = $1
            "#,
        )
        .bind(alarm_id)
        .bind(format!(
            "Predictive anomaly: {} ({})",
            row.sensor_name, row.sensor_id
        ))
        .bind(SqlJson(rule))
        .bind(row.node_id)
        .bind(score)
        .bind(last_fired)
        .execute(&mut **tx)
        .await?;
        alarm_id
    } else {
        let inserted: (i64,) = sqlx::query_as(
            r#"
            INSERT INTO alarms (name, rule, status, sensor_id, node_id, origin, anomaly_score, last_fired)
            VALUES ($1, $2, 'firing', $3, $4, 'predictive', $5, $6)
            RETURNING id
            "#,
        )
        .bind(format!("Predictive anomaly: {} ({})", row.sensor_name, row.sensor_id))
        .bind(SqlJson(rule))
        .bind(row.sensor_id.trim())
        .bind(row.node_id)
        .bind(score)
        .bind(now)
        .fetch_one(&mut **tx)
        .await?;
        inserted.0
    };

    if should_emit {
        let target_key = format!("sensor:{}", row.sensor_id.trim());
        let _ = sqlx::query(
            r#"
            INSERT INTO alarm_events (
                alarm_id,
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
            VALUES ($1, $2, $3, 'firing', $4, 'predictive', $5, 'fired', $6, $7)
            "#,
        )
        .bind(alarm_id)
        .bind(row.sensor_id.trim())
        .bind(row.node_id)
        .bind(message)
        .bind(score)
        .bind(
            crate::services::incidents::get_or_create_incident(
                tx,
                now,
                &crate::services::incidents::IncidentKey {
                    rule_id: None,
                    target_key: Some(target_key.clone()),
                },
                "warning",
                &format!("Predictive anomaly: {} ({})", row.sensor_name, row.sensor_id),
                "fired",
            )
            .await?,
        )
        .bind(target_key)
        .execute(&mut **tx)
        .await?;
    }

    Ok(should_emit)
}

async fn load_status(db: &sqlx::PgPool) -> anyhow::Result<PredictiveStatus> {
    let row: Option<CredentialRow> = sqlx::query_as(
        r#"
        SELECT value, metadata
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind(PREDICTIVE_CREDENTIAL_NAME)
    .fetch_optional(db)
    .await?;

    let mut enabled = false;
    let mut api_base_url = String::new();
    let mut model: Option<String> = None;
    let mut token_present = false;

    if let Some(row) = row {
        token_present = !row.value.trim().is_empty();
        if let JsonValue::Object(map) = row.metadata.0 {
            if let Some(JsonValue::Bool(v)) = map.get("enabled") {
                enabled = *v;
            }
            if let Some(JsonValue::String(v)) = map.get("api_base_url") {
                api_base_url = v.clone();
            }
            if let Some(JsonValue::String(v)) = map.get("model") {
                model = Some(v.clone());
            }
        }
    }

    Ok(PredictiveStatus {
        enabled,
        running: false,
        token_present,
        api_base_url,
        model,
        fallback_models: vec![],
        bootstrap_on_start: false,
        bootstrap_max_sensors: 25,
        bootstrap_lookback_hours: 24,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/predictive/trace", get(predictive_trace))
        .route("/predictive/status", get(predictive_status))
        .route("/predictive/config", put(update_config))
        .route("/predictive/bootstrap", post(bootstrap))
}
