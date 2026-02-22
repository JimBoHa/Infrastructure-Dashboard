use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;

use crate::auth::AuthUser;
use crate::error::map_db_error;
use crate::services::alarm_engine::types::TargetSelector;
use crate::services::analysis::bucket_reader::{
    read_bucket_series_for_sensors_with_aggregation_and_options, BucketAggregationPreference,
};
use crate::services::analysis::parquet_duckdb::{
    bucket_coverage_pct, expected_bucket_count, MetricsBucketReadOptions, MetricsQualityFilter,
};
use crate::state::AppState;

const CAP_ALERTS_VIEW: &str = "alerts.view";
const DEFAULT_STATS_HORIZON_SECONDS: i64 = 7 * 24 * 3600;
const STATS_MAX_EXPECTED_BUCKETS: u64 = 50_000;
const STATS_DEFAULT_MAX_EXPECTED_BUCKETS: u64 = 20_000;

#[derive(Debug, Clone, sqlx::FromRow)]
struct AlarmRuleRow {
    id: i64,
    name: String,
    description: String,
    enabled: bool,
    severity: String,
    origin: String,
    target_selector: SqlJson<JsonValue>,
    condition_ast: SqlJson<JsonValue>,
    timing: SqlJson<JsonValue>,
    message_template: String,
    created_by: Option<uuid::Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct AlarmRuleListRow {
    id: i64,
    name: String,
    description: String,
    enabled: bool,
    severity: String,
    origin: String,
    target_selector: SqlJson<JsonValue>,
    condition_ast: SqlJson<JsonValue>,
    timing: SqlJson<JsonValue>,
    message_template: String,
    created_by: Option<uuid::Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    active_count: i64,
    last_eval_at: Option<chrono::DateTime<chrono::Utc>>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmRuleResponse {
    id: i64,
    name: String,
    description: String,
    enabled: bool,
    severity: String,
    origin: String,
    target_selector: JsonValue,
    condition_ast: JsonValue,
    timing: JsonValue,
    message_template: String,
    created_by: Option<String>,
    created_at: String,
    updated_at: String,
    deleted_at: Option<String>,
    active_count: i64,
    last_eval_at: Option<String>,
    last_error: Option<String>,
}

impl From<AlarmRuleListRow> for AlarmRuleResponse {
    fn from(row: AlarmRuleListRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            enabled: row.enabled,
            severity: row.severity,
            origin: row.origin,
            target_selector: row.target_selector.0,
            condition_ast: row.condition_ast.0,
            timing: row.timing.0,
            message_template: row.message_template,
            created_by: row.created_by.map(|value| value.to_string()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
            deleted_at: row.deleted_at.map(|value| value.to_rfc3339()),
            active_count: row.active_count,
            last_eval_at: row.last_eval_at.map(|value| value.to_rfc3339()),
            last_error: row.last_error,
        }
    }
}

impl AlarmRuleResponse {
    fn from_detail(row: AlarmRuleRow, active_count: i64, last_eval_at: Option<chrono::DateTime<chrono::Utc>>, last_error: Option<String>) -> Self {
        Self {
            id: row.id,
            name: row.name,
            description: row.description,
            enabled: row.enabled,
            severity: row.severity,
            origin: row.origin,
            target_selector: row.target_selector.0,
            condition_ast: row.condition_ast.0,
            timing: row.timing.0,
            message_template: row.message_template,
            created_by: row.created_by.map(|value| value.to_string()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
            deleted_at: row.deleted_at.map(|value| value.to_rfc3339()),
            active_count,
            last_eval_at: last_eval_at.map(|value| value.to_rfc3339()),
            last_error,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct CreateAlarmRuleRequest {
    name: String,
    description: Option<String>,
    enabled: Option<bool>,
    severity: Option<String>,
    origin: Option<String>,
    target_selector: JsonValue,
    condition_ast: JsonValue,
    timing: Option<JsonValue>,
    message_template: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct UpdateAlarmRuleRequest {
    name: Option<String>,
    description: Option<String>,
    enabled: Option<bool>,
    severity: Option<String>,
    origin: Option<String>,
    target_selector: Option<JsonValue>,
    condition_ast: Option<JsonValue>,
    timing: Option<JsonValue>,
    message_template: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct AlarmRulePreviewRequest {
    target_selector: JsonValue,
    condition_ast: JsonValue,
    timing: Option<JsonValue>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmRulePreviewResponse {
    targets_evaluated: usize,
    results: Vec<crate::services::alarm_engine::PreviewTargetResult>,
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub(crate) struct AlarmRuleStatsRequest {
    target_selector: JsonValue,
    /// Start timestamp (RFC3339). If omitted, defaults to end - 7 days.
    start: Option<String>,
    /// End timestamp (RFC3339). If omitted, defaults to now.
    end: Option<String>,
    /// Bucket interval in seconds (optional).
    interval_seconds: Option<i64>,
    /// Bucket aggregation mode (auto/avg/last/sum/min/max).
    bucket_aggregation_mode: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmRuleStatsBandSet {
    lower_1: Option<f64>,
    upper_1: Option<f64>,
    lower_2: Option<f64>,
    upper_2: Option<f64>,
    lower_3: Option<f64>,
    upper_3: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmRuleStatsBands {
    classic: AlarmRuleStatsBandSet,
    robust: AlarmRuleStatsBandSet,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmRuleStatsSensorResponse {
    sensor_id: String,
    unit: String,
    interval_seconds: i64,
    n: u64,
    min: Option<f64>,
    max: Option<f64>,
    mean: Option<f64>,
    median: Option<f64>,
    stddev: Option<f64>,
    p01: Option<f64>,
    p05: Option<f64>,
    p25: Option<f64>,
    p75: Option<f64>,
    p95: Option<f64>,
    p99: Option<f64>,
    mad: Option<f64>,
    iqr: Option<f64>,
    coverage_pct: Option<f64>,
    missing_pct: Option<f64>,
    bands: AlarmRuleStatsBands,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmRuleStatsResponse {
    start: String,
    end: String,
    interval_seconds: i64,
    bucket_aggregation_mode: String,
    sensors: Vec<AlarmRuleStatsSensorResponse>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub(crate) struct AlarmRuleDeleteResponse {
    status: String,
}

fn normalize_severity(value: &str) -> Result<String, (StatusCode, String)> {
    let normalized = value.trim().to_lowercase();
    match normalized.as_str() {
        "info" | "warning" | "critical" => Ok(normalized),
        _ => Err((
            StatusCode::BAD_REQUEST,
            "severity must be one of: info, warning, critical".to_string(),
        )),
    }
}

async fn fetch_rule_row(state: &AppState, rule_id: i64) -> Result<Option<AlarmRuleRow>, (StatusCode, String)> {
    let row: Option<AlarmRuleRow> = sqlx::query_as(
        r#"
        SELECT
            id,
            name,
            description,
            enabled,
            severity,
            origin,
            target_selector,
            condition_ast,
            timing,
            message_template,
            created_by,
            created_at,
            updated_at,
            deleted_at
        FROM alarm_rules
        WHERE id = $1
        "#,
    )
    .bind(rule_id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(row)
}

async fn fetch_rule_detail(state: &AppState, row: AlarmRuleRow) -> Result<AlarmRuleResponse, (StatusCode, String)> {
    let active_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM alarms WHERE rule_id = $1 AND status = 'firing'",
    )
    .bind(row.id)
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    let status_row: Option<(Option<chrono::DateTime<chrono::Utc>>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT max(last_eval_at) as last_eval_at,
               max(error) FILTER (WHERE error IS NOT NULL) as last_error
        FROM alarm_rule_state
        WHERE rule_id = $1
        "#,
    )
    .bind(row.id)
    .fetch_optional(&state.db)
    .await
    .map_err(map_db_error)?;

    let (last_eval_at, last_error) = status_row.unwrap_or((None, None));
    Ok(AlarmRuleResponse::from_detail(
        row,
        active_count,
        last_eval_at,
        last_error,
    ))
}

#[utoipa::path(
    get,
    path = "/api/alarm-rules",
    tag = "alarm_rules",
    responses(
        (status = 200, description = "Alarm rules", body = Vec<AlarmRuleResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn list_alarm_rules(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Json<Vec<AlarmRuleResponse>>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let rows: Vec<AlarmRuleListRow> = sqlx::query_as(
        r#"
        SELECT
            r.id,
            r.name,
            r.description,
            r.enabled,
            r.severity,
            r.origin,
            r.target_selector,
            r.condition_ast,
            r.timing,
            r.message_template,
            r.created_by,
            r.created_at,
            r.updated_at,
            r.deleted_at,
            COALESCE((
                SELECT count(*)
                FROM alarms a
                WHERE a.rule_id = r.id
                  AND a.status = 'firing'
            ), 0)::bigint as active_count,
            (
                SELECT max(last_eval_at)
                FROM alarm_rule_state s
                WHERE s.rule_id = r.id
            ) as last_eval_at,
            (
                SELECT max(error)
                FROM alarm_rule_state s
                WHERE s.rule_id = r.id
                  AND s.error IS NOT NULL
            ) as last_error
        FROM alarm_rules r
        WHERE r.deleted_at IS NULL
        ORDER BY r.id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    Ok(Json(rows.into_iter().map(AlarmRuleResponse::from).collect()))
}

#[utoipa::path(
    get,
    path = "/api/alarm-rules/{rule_id}",
    tag = "alarm_rules",
    params(("rule_id" = i64, Path, description = "Alarm rule id")),
    responses(
        (status = 200, description = "Alarm rule detail", body = AlarmRuleResponse),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn get_alarm_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(rule_id): Path<i64>,
) -> Result<Json<AlarmRuleResponse>, (StatusCode, String)> {
    crate::auth::require_any_capabilities(&user, &[CAP_ALERTS_VIEW, "config.write"])
        .map_err(|err| (err.status, err.message))?;

    let Some(row) = fetch_rule_row(&state, rule_id).await? else {
        return Err((StatusCode::NOT_FOUND, "Alarm rule not found".to_string()));
    };

    if row.deleted_at.is_some() {
        return Err((StatusCode::NOT_FOUND, "Alarm rule not found".to_string()));
    }

    Ok(Json(fetch_rule_detail(&state, row).await?))
}

#[utoipa::path(
    post,
    path = "/api/alarm-rules",
    tag = "alarm_rules",
    request_body = CreateAlarmRuleRequest,
    responses(
        (status = 201, description = "Created alarm rule", body = AlarmRuleResponse),
        (status = 400, description = "Invalid request")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn create_alarm_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<CreateAlarmRuleRequest>,
) -> Result<(StatusCode, Json<AlarmRuleResponse>), (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let name = payload.name.trim();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".to_string()));
    }

    let severity = normalize_severity(payload.severity.as_deref().unwrap_or("warning"))?;
    let origin = payload
        .origin
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("threshold")
        .to_string();
    let description = payload.description.unwrap_or_default();
    let timing = payload.timing.unwrap_or(JsonValue::Object(Default::default()));
    let message_template = payload.message_template.unwrap_or_default();

    crate::services::alarm_engine::types::parse_rule_envelope(
        &payload.target_selector,
        &payload.condition_ast,
        &timing,
    )
    .map_err(|err| (StatusCode::BAD_REQUEST, err))?;

    let inserted: (i64,) = sqlx::query_as(
        r#"
        INSERT INTO alarm_rules (
            name,
            description,
            enabled,
            severity,
            origin,
            target_selector,
            condition_ast,
            timing,
            message_template,
            created_by,
            updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW())
        RETURNING id
        "#,
    )
    .bind(name)
    .bind(description)
    .bind(payload.enabled.unwrap_or(true))
    .bind(severity)
    .bind(origin)
    .bind(SqlJson(payload.target_selector))
    .bind(SqlJson(payload.condition_ast))
    .bind(SqlJson(timing))
    .bind(message_template)
        .bind(user.user_id())
    .fetch_one(&state.db)
    .await
    .map_err(map_db_error)?;

    let row = fetch_rule_row(&state, inserted.0)
        .await?
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch created alarm rule".to_string()))?;

    let detail = fetch_rule_detail(&state, row).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[utoipa::path(
    put,
    path = "/api/alarm-rules/{rule_id}",
    tag = "alarm_rules",
    params(("rule_id" = i64, Path, description = "Alarm rule id")),
    request_body = UpdateAlarmRuleRequest,
    responses(
        (status = 200, description = "Updated alarm rule", body = AlarmRuleResponse),
        (status = 404, description = "Not found")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn update_alarm_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(rule_id): Path<i64>,
    Json(payload): Json<UpdateAlarmRuleRequest>,
) -> Result<Json<AlarmRuleResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let Some(existing) = fetch_rule_row(&state, rule_id).await? else {
        return Err((StatusCode::NOT_FOUND, "Alarm rule not found".to_string()));
    };
    if existing.deleted_at.is_some() {
        return Err((StatusCode::NOT_FOUND, "Alarm rule not found".to_string()));
    }

    let name = payload.name.unwrap_or(existing.name);
    if name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".to_string()));
    }

    let severity = normalize_severity(payload.severity.as_deref().unwrap_or(&existing.severity))?;
    let origin = payload.origin.unwrap_or(existing.origin);
    let description = payload.description.unwrap_or(existing.description);
    let enabled = payload.enabled.unwrap_or(existing.enabled);
    let target_selector = payload.target_selector.unwrap_or(existing.target_selector.0);
    let condition_ast = payload.condition_ast.unwrap_or(existing.condition_ast.0);
    let timing = payload.timing.unwrap_or(existing.timing.0);
    let message_template = payload.message_template.unwrap_or(existing.message_template);

    crate::services::alarm_engine::types::parse_rule_envelope(&target_selector, &condition_ast, &timing)
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;

    sqlx::query(
        r#"
        UPDATE alarm_rules
        SET
            name = $2,
            description = $3,
            enabled = $4,
            severity = $5,
            origin = $6,
            target_selector = $7,
            condition_ast = $8,
            timing = $9,
            message_template = $10,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(rule_id)
    .bind(name.trim())
    .bind(description)
    .bind(enabled)
    .bind(severity)
    .bind(origin)
    .bind(SqlJson(target_selector))
    .bind(SqlJson(condition_ast))
    .bind(SqlJson(timing))
    .bind(message_template)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    let row = fetch_rule_row(&state, rule_id)
        .await?
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch updated alarm rule".to_string()))?;
    Ok(Json(fetch_rule_detail(&state, row).await?))
}

#[utoipa::path(
    delete,
    path = "/api/alarm-rules/{rule_id}",
    tag = "alarm_rules",
    params(("rule_id" = i64, Path, description = "Alarm rule id")),
    responses((status = 200, description = "Deleted alarm rule", body = AlarmRuleDeleteResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn delete_alarm_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(rule_id): Path<i64>,
) -> Result<Json<AlarmRuleDeleteResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let result = sqlx::query(
        r#"
        UPDATE alarm_rules
        SET enabled = FALSE, deleted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(rule_id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Alarm rule not found".to_string()));
    }

    Ok(Json(AlarmRuleDeleteResponse {
        status: "deleted".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/alarm-rules/{rule_id}/enable",
    tag = "alarm_rules",
    params(("rule_id" = i64, Path, description = "Alarm rule id")),
    responses((status = 200, description = "Enabled alarm rule", body = AlarmRuleResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn enable_alarm_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(rule_id): Path<i64>,
) -> Result<Json<AlarmRuleResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let updated = sqlx::query(
        r#"
        UPDATE alarm_rules
        SET enabled = TRUE, deleted_at = NULL, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(rule_id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Alarm rule not found".to_string()));
    }

    let row = fetch_rule_row(&state, rule_id)
        .await?
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch enabled alarm rule".to_string()))?;
    Ok(Json(fetch_rule_detail(&state, row).await?))
}

#[utoipa::path(
    post,
    path = "/api/alarm-rules/{rule_id}/disable",
    tag = "alarm_rules",
    params(("rule_id" = i64, Path, description = "Alarm rule id")),
    responses((status = 200, description = "Disabled alarm rule", body = AlarmRuleResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn disable_alarm_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Path(rule_id): Path<i64>,
) -> Result<Json<AlarmRuleResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let updated = sqlx::query(
        r#"
        UPDATE alarm_rules
        SET enabled = FALSE, updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(rule_id)
    .execute(&state.db)
    .await
    .map_err(map_db_error)?;

    if updated.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Alarm rule not found".to_string()));
    }

    let row = fetch_rule_row(&state, rule_id)
        .await?
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch disabled alarm rule".to_string()))?;
    Ok(Json(fetch_rule_detail(&state, row).await?))
}

#[utoipa::path(
    post,
    path = "/api/alarm-rules/preview",
    tag = "alarm_rules",
    request_body = AlarmRulePreviewRequest,
    responses((status = 200, description = "Alarm rule preview", body = AlarmRulePreviewResponse)),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn preview_alarm_rule(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<AlarmRulePreviewRequest>,
) -> Result<Json<AlarmRulePreviewResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let timing = payload.timing.unwrap_or(JsonValue::Object(Default::default()));
    let results = crate::services::alarm_engine::preview_rule(
        &state.db,
        &payload.target_selector,
        &payload.condition_ast,
        &timing,
    )
    .await
    .map_err(|err| (StatusCode::BAD_REQUEST, err))?;

    Ok(Json(AlarmRulePreviewResponse {
        targets_evaluated: results.len(),
        results,
    }))
}

#[derive(sqlx::FromRow)]
struct SensorMetaRow {
    sensor_id: String,
    unit: String,
    interval_seconds: i32,
}

fn parse_rfc3339(raw: &str, field: &str) -> Result<DateTime<Utc>, (StatusCode, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err((StatusCode::BAD_REQUEST, format!("{field} cannot be blank")));
    }
    let parsed = DateTime::parse_from_rfc3339(trimmed)
        .map_err(|_| (StatusCode::BAD_REQUEST, format!("{field} must be RFC3339")))?;
    Ok(parsed.with_timezone(&Utc))
}

fn normalize_bucket_aggregation_mode(
    raw: Option<&str>,
) -> Result<(BucketAggregationPreference, String), (StatusCode, String)> {
    let mode = raw.unwrap_or("auto").trim().to_lowercase();
    let preference = match mode.as_str() {
        "" | "auto" => BucketAggregationPreference::Auto,
        "avg" => BucketAggregationPreference::Avg,
        "last" => BucketAggregationPreference::Last,
        "sum" => BucketAggregationPreference::Sum,
        "min" => BucketAggregationPreference::Min,
        "max" => BucketAggregationPreference::Max,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                "bucket_aggregation_mode must be one of: auto, avg, last, sum, min, max".to_string(),
            ));
        }
    };
    Ok((preference, if mode.is_empty() { "auto".to_string() } else { mode }))
}

fn default_interval_seconds(sensor_meta: &[SensorMetaRow]) -> i64 {
    let mut intervals: Vec<i64> = sensor_meta
        .iter()
        .map(|row| row.interval_seconds as i64)
        .map(|value| if value <= 0 { 60 } else { value })
        .collect();
    if intervals.is_empty() {
        return 60;
    }
    intervals.sort();
    intervals[intervals.len() / 2].clamp(1, 86_400)
}

fn quantile_sorted(sorted: &[f64], q: f64) -> Option<f64> {
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
    let a = sorted[idx];
    let b = sorted[(idx + 1).min(sorted.len() - 1)];
    Some(a + (b - a) * frac)
}

fn mean_stddev(values: &[f64]) -> (Option<f64>, Option<f64>) {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.is_empty() {
        return (None, None);
    }
    let n = finite.len() as f64;
    let mean = finite.iter().sum::<f64>() / n;
    if finite.len() < 2 {
        return (Some(mean), None);
    }
    let variance = finite
        .iter()
        .map(|v| {
            let d = v - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    let stddev = variance.sqrt();
    (Some(mean), if stddev.is_finite() { Some(stddev) } else { None })
}

fn mad(values: &[f64], center: f64) -> Option<f64> {
    let mut deviations: Vec<f64> = values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| (v - center).abs())
        .collect();
    if deviations.is_empty() {
        return None;
    }
    deviations.sort_by(|a, b| a.total_cmp(b));
    quantile_sorted(&deviations, 0.5)
}

fn band_set(center: Option<f64>, sigma: Option<f64>) -> AlarmRuleStatsBandSet {
    let (Some(center), Some(sigma)) = (center, sigma) else {
        return AlarmRuleStatsBandSet {
            lower_1: None,
            upper_1: None,
            lower_2: None,
            upper_2: None,
            lower_3: None,
            upper_3: None,
        };
    };
    if !center.is_finite() || !sigma.is_finite() {
        return AlarmRuleStatsBandSet {
            lower_1: None,
            upper_1: None,
            lower_2: None,
            upper_2: None,
            lower_3: None,
            upper_3: None,
        };
    }

    AlarmRuleStatsBandSet {
        lower_1: Some(center - 1.0 * sigma),
        upper_1: Some(center + 1.0 * sigma),
        lower_2: Some(center - 2.0 * sigma),
        upper_2: Some(center + 2.0 * sigma),
        lower_3: Some(center - 3.0 * sigma),
        upper_3: Some(center + 3.0 * sigma),
    }
}

#[utoipa::path(
    post,
    path = "/api/alarm-rules/stats",
    tag = "alarm_rules",
    request_body = AlarmRuleStatsRequest,
    responses(
        (status = 200, description = "Alarm rule stats guidance", body = AlarmRuleStatsResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("HTTPBearer" = []))
)]
pub(crate) async fn alarm_rule_stats(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthUser(user): AuthUser,
    Json(payload): Json<AlarmRuleStatsRequest>,
) -> Result<Json<AlarmRuleStatsResponse>, (StatusCode, String)> {
    crate::auth::require_capabilities(&user, &["config.write"])
        .map_err(|err| (err.status, err.message))?;

    let selector: TargetSelector = serde_json::from_value(payload.target_selector.clone())
        .map_err(|err| (StatusCode::BAD_REQUEST, format!("invalid target_selector: {err}")))?;

    let end = if let Some(raw) = payload.end.as_deref() {
        parse_rfc3339(raw, "end")?
    } else {
        Utc::now()
    };
    let start = if let Some(raw) = payload.start.as_deref() {
        parse_rfc3339(raw, "start")?
    } else {
        end - Duration::seconds(DEFAULT_STATS_HORIZON_SECONDS)
    };

    if end <= start {
        return Err((StatusCode::BAD_REQUEST, "end must be after start".to_string()));
    }

    let targets = crate::services::alarm_engine::resolve_targets(&state.db, &selector)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let mut sensor_ids: Vec<String> = targets
        .into_iter()
        .flat_map(|t| t.sensor_ids)
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    sensor_ids.sort();
    sensor_ids.dedup();
    if sensor_ids.is_empty() {
        return Ok(Json(AlarmRuleStatsResponse {
            start: start.to_rfc3339(),
            end: end.to_rfc3339(),
            interval_seconds: payload.interval_seconds.unwrap_or(60).max(1),
            bucket_aggregation_mode: payload
                .bucket_aggregation_mode
                .unwrap_or_else(|| "auto".to_string()),
            sensors: Vec::new(),
        }));
    }

    let meta_rows: Vec<SensorMetaRow> = sqlx::query_as(
        r#"
        SELECT sensor_id, unit, interval_seconds
        FROM sensors
        WHERE sensor_id = ANY($1)
          AND deleted_at IS NULL
        "#,
    )
    .bind(&sensor_ids)
    .fetch_all(&state.db)
    .await
    .map_err(map_db_error)?;

    let mut interval_seconds = payload
        .interval_seconds
        .unwrap_or_else(|| default_interval_seconds(&meta_rows))
        .max(1)
        .min(86_400);

    let expected = expected_bucket_count(start, end, interval_seconds);
    if payload.interval_seconds.is_some() {
        if expected > STATS_MAX_EXPECTED_BUCKETS {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Requested interval_seconds produces too many buckets for this range (expected {}, limit {}). Increase interval_seconds or narrow the range.",
                    expected, STATS_MAX_EXPECTED_BUCKETS
                ),
            ));
        }
    } else if expected > STATS_DEFAULT_MAX_EXPECTED_BUCKETS {
        let horizon_seconds = (end - start).num_seconds().max(1);
        let scaled = (horizon_seconds as f64 / STATS_DEFAULT_MAX_EXPECTED_BUCKETS as f64).ceil() as i64;
        interval_seconds = interval_seconds.max(scaled).clamp(1, 86_400);
    }

    let (aggregation_preference, aggregation_label) =
        normalize_bucket_aggregation_mode(payload.bucket_aggregation_mode.as_deref())?;

    let options = MetricsBucketReadOptions {
        min_samples_per_bucket: Some(1),
        quality_filter: MetricsQualityFilter::GoodOnly,
    };

    let rows = read_bucket_series_for_sensors_with_aggregation_and_options(
        &state.db,
        state.analysis_jobs.duckdb(),
        state.analysis_jobs.lake_config(),
        sensor_ids.clone(),
        start,
        end,
        interval_seconds,
        aggregation_preference,
        options,
    )
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    let mut values_by_sensor: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
    for row in rows {
        if !row.value.is_finite() {
            continue;
        }
        values_by_sensor.entry(row.sensor_id).or_default().push(row.value);
    }

    let mut meta_by_sensor: std::collections::HashMap<String, SensorMetaRow> = std::collections::HashMap::new();
    for row in meta_rows {
        meta_by_sensor.insert(row.sensor_id.clone(), row);
    }

    let mut sensors_out: Vec<AlarmRuleStatsSensorResponse> = Vec::new();
    for sensor_id in sensor_ids {
        let meta = meta_by_sensor.get(&sensor_id);
        let unit = meta.map(|m| m.unit.clone()).unwrap_or_default();
        let sensor_interval = meta.map(|m| m.interval_seconds as i64).unwrap_or(0).max(0);
        let mut values = values_by_sensor.remove(&sensor_id).unwrap_or_default();
        values.retain(|v| v.is_finite());

        values.sort_by(|a, b| a.total_cmp(b));
        let n = values.len() as u64;

        let min = values.first().copied();
        let max = values.last().copied();
        let median = quantile_sorted(&values, 0.5);
        let p01 = quantile_sorted(&values, 0.01);
        let p05 = quantile_sorted(&values, 0.05);
        let p25 = quantile_sorted(&values, 0.25);
        let p75 = quantile_sorted(&values, 0.75);
        let p95 = quantile_sorted(&values, 0.95);
        let p99 = quantile_sorted(&values, 0.99);
        let (mean, stddev) = mean_stddev(&values);

        let mad_raw = median.and_then(|m| mad(&values, m));
        let iqr = match (p25, p75) {
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        };

        let robust_sigma = mad_raw.map(|v| v * 1.4826);
        let classic_bands = band_set(mean, stddev);
        let robust_bands = band_set(median, robust_sigma);

        let coverage_pct = bucket_coverage_pct(n, start, end, interval_seconds);
        let missing_pct = coverage_pct.map(|v| (100.0 - v).max(0.0));

        sensors_out.push(AlarmRuleStatsSensorResponse {
            sensor_id,
            unit,
            interval_seconds: sensor_interval,
            n,
            min,
            max,
            mean,
            median,
            stddev,
            p01,
            p05,
            p25,
            p75,
            p95,
            p99,
            mad: mad_raw,
            iqr,
            coverage_pct,
            missing_pct,
            bands: AlarmRuleStatsBands {
                classic: classic_bands,
                robust: robust_bands,
            },
        });
    }

    sensors_out.sort_by(|a, b| a.sensor_id.cmp(&b.sensor_id));

    Ok(Json(AlarmRuleStatsResponse {
        start: start.to_rfc3339(),
        end: end.to_rfc3339(),
        interval_seconds,
        bucket_aggregation_mode: aggregation_label,
        sensors: sensors_out,
    }))
}

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/alarm-rules", get(list_alarm_rules).post(create_alarm_rule))
        .route("/alarm-rules/preview", post(preview_alarm_rule))
        .route("/alarm-rules/stats", post(alarm_rule_stats))
        .route(
            "/alarm-rules/{rule_id}",
            get(get_alarm_rule)
                .put(update_alarm_rule)
                .delete(delete_alarm_rule),
        )
        .route("/alarm-rules/{rule_id}/enable", post(enable_alarm_rule))
        .route("/alarm-rules/{rule_id}/disable", post(disable_alarm_rule))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantile_sorted_interpolates() {
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let median = quantile_sorted(&values, 0.5).unwrap();
        assert!((median - 2.5).abs() < 1e-12);

        let p25 = quantile_sorted(&values, 0.25).unwrap();
        assert!((p25 - 1.75).abs() < 1e-12);
    }

    #[test]
    fn mean_stddev_matches_population_stddev() {
        let (mean, stddev) = mean_stddev(&[1.0, 2.0, 3.0]);
        assert!((mean.unwrap() - 2.0).abs() < 1e-12);
        let expected = (2.0_f64 / 3.0_f64).sqrt();
        assert!((stddev.unwrap() - expected).abs() < 1e-12);
    }

    #[test]
    fn mad_uses_median_abs_deviation() {
        let mut values: Vec<f64> = vec![1.0, 2.0, 100.0];
        values.sort_by(|a, b| a.total_cmp(b));
        let center = quantile_sorted(&values, 0.5).unwrap();
        assert!((center - 2.0).abs() < 1e-12);
        let mad_value = mad(&values, center).unwrap();
        assert!((mad_value - 1.0).abs() < 1e-12);
    }

    #[test]
    fn band_set_returns_expected_ranges() {
        let bands = band_set(Some(10.0), Some(2.0));
        assert_eq!(bands.lower_1, Some(8.0));
        assert_eq!(bands.upper_1, Some(12.0));
        assert_eq!(bands.lower_2, Some(6.0));
        assert_eq!(bands.upper_2, Some(14.0));
        assert_eq!(bands.lower_3, Some(4.0));
        assert_eq!(bands.upper_3, Some(16.0));
    }

    #[test]
    fn default_interval_seconds_picks_median_and_sanitizes() {
        let rows = vec![
            SensorMetaRow {
                sensor_id: "a".to_string(),
                unit: "C".to_string(),
                interval_seconds: 30,
            },
            SensorMetaRow {
                sensor_id: "b".to_string(),
                unit: "C".to_string(),
                interval_seconds: 0,
            },
            SensorMetaRow {
                sensor_id: "c".to_string(),
                unit: "C".to_string(),
                interval_seconds: 60,
            },
        ];
        assert_eq!(default_interval_seconds(&rows), 60);
    }

    #[test]
    fn normalize_bucket_aggregation_mode_accepts_known_values() {
        let (pref, label) = normalize_bucket_aggregation_mode(None).unwrap();
        assert!(matches!(pref, BucketAggregationPreference::Auto));
        assert_eq!(label, "auto");

        let (pref, label) = normalize_bucket_aggregation_mode(Some("avg")).unwrap();
        assert!(matches!(pref, BucketAggregationPreference::Avg));
        assert_eq!(label, "avg");

        let (pref, label) = normalize_bucket_aggregation_mode(Some("")).unwrap();
        assert!(matches!(pref, BucketAggregationPreference::Auto));
        assert_eq!(label, "auto");

        assert!(normalize_bucket_aggregation_mode(Some("bogus")).is_err());
    }
}
