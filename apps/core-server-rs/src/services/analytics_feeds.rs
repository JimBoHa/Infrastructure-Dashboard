use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::services::emporia::EmporiaService;
use crate::services::emporia_ingest::persist_emporia_usage;
use crate::services::emporia_preferences::{
    derive_emporia_power_summary, merge_emporia_circuit_preferences,
    merge_emporia_device_preferences,
};
use crate::state::AppState;

const EMPORIA_CREDENTIAL_NAME: &str = "emporia";

#[derive(sqlx::FromRow)]
struct CredentialRow {
    value: String,
    metadata: SqlJson<JsonValue>,
}

#[derive(Debug)]
pub struct FeedPollResult {
    pub name: String,
    pub status: String,
}

pub struct AnalyticsFeedService {
    state: AppState,
    interval: Duration,
}

impl AnalyticsFeedService {
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
                        if let Err(err) = poll_all_feeds(&state).await {
                            warn!("analytics feed poll failed: {err:#}");
                        }
                    }
                }
            }
        });
    }
}

pub async fn poll_all_feeds(state: &AppState) -> Result<Vec<FeedPollResult>> {
    let mut results = Vec::new();
    match poll_emporia(state).await {
        Ok(result) => results.push(result),
        Err(err) => {
            let meta = json!({
                "detail": format!("Emporia poll failed: {err}"),
            });
            persist_integration_status(state, "Emporia", "error", &meta).await?;
            results.push(FeedPollResult {
                name: "Emporia".to_string(),
                status: "error".to_string(),
            });
        }
    }
    Ok(results)
}

async fn poll_emporia(state: &AppState) -> Result<FeedPollResult> {
    let row: Option<CredentialRow> = sqlx::query_as(
        r#"
        SELECT value, metadata
        FROM setup_credentials
        WHERE name = $1
        "#,
    )
    .bind(EMPORIA_CREDENTIAL_NAME)
    .fetch_optional(&state.db)
    .await
    .context("failed to load Emporia credential")?;

    let mut id_token = row
        .as_ref()
        .map(|row| row.value.trim().to_string())
        .unwrap_or_default();
    let mut metadata = row
        .as_ref()
        .map(|row| row.metadata.0.clone())
        .unwrap_or_else(|| json!({}));

    let refresh_token = metadata
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let username = metadata
        .get("username")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let legacy_site_ids: Vec<String> = metadata
        .get("site_ids")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    if let Some(num) = item.as_i64() {
                        Some(num.to_string())
                    } else {
                        item.as_str().map(|s| s.to_string())
                    }
                })
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    if id_token.is_empty() && refresh_token.is_none() {
        return Ok(FeedPollResult {
            name: "Emporia".to_string(),
            status: "missing".to_string(),
        });
    }

    let emporia = EmporiaService::new(state.http.clone());

    // If we have no id_token but do have a refresh token, try to refresh first.
    if id_token.is_empty() {
        if let Some(token) = refresh_token.as_ref() {
            let refreshed = emporia.refresh_with_token(token).await?;
            id_token = refreshed.id_token.clone();
            metadata["refresh_token"] = JsonValue::String(token.clone());
        }
    }

    if id_token.is_empty() {
        anyhow::bail!("Emporia credential missing id_token; run setup login first");
    }

    let devices = match emporia.fetch_devices(&id_token).await {
        Ok(value) => value,
        Err(err) if is_auth_error(&err) => {
            if let Some(token) = refresh_token.as_ref() {
                let refreshed = emporia.refresh_with_token(token).await?;
                id_token = refreshed.id_token.clone();
                metadata["refresh_token"] = JsonValue::String(token.clone());
                emporia.fetch_devices(&id_token).await?
            } else {
                return Err(err);
            }
        }
        Err(err) => return Err(err),
    };

    // Merge per-device preferences (enabled + summary inclusion + group labels).
    let (mut device_prefs, enabled_device_gids, included_device_gids) =
        merge_emporia_device_preferences(&devices, &mut metadata, &legacy_site_ids);

    if enabled_device_gids.is_empty() {
        return Ok(FeedPollResult {
            name: "Emporia".to_string(),
            status: "missing".to_string(),
        });
    }

    let usage = match emporia.fetch_usage(&id_token, &enabled_device_gids).await {
        Ok(value) => value,
        Err(err) if is_auth_error(&err) => {
            if let Some(token) = refresh_token.as_ref() {
                let refreshed = emporia.refresh_with_token(token).await?;
                id_token = refreshed.id_token.clone();
                metadata["refresh_token"] = JsonValue::String(token.clone());
                emporia.fetch_usage(&id_token, &enabled_device_gids).await?
            } else {
                return Err(err);
            }
        }
        Err(err) => return Err(err),
    };

    // Ensure every device has a preferences entry for every channel we see.
    merge_emporia_circuit_preferences(&usage, &mut device_prefs, &mut metadata);

    let timestamp = usage.timestamp;
    let devices_seen: Vec<JsonValue> = usage
        .devices
        .iter()
        .map(|d| {
            json!({
                "device_gid": d.device_gid,
                "main_kw": d.main_power_w / 1000.0,
                "channel_count": d.channels.len(),
            })
        })
        .collect();

    let mut summary_device_gids: Vec<String> = included_device_gids.iter().cloned().collect();
    summary_device_gids.sort();

    let power_meta = json!({
        "source": "emporia_cloud",
        "device_gids_polled": enabled_device_gids,
        "device_gids_in_power_summary": summary_device_gids,
        "devices": devices_seen,
    });

    persist_power_samples(
        state,
        timestamp,
        &usage,
        &power_meta,
        &included_device_gids,
        &device_prefs,
    )
    .await?;
    persist_emporia_usage(state, &devices, &usage, &device_prefs).await?;

    let status_meta = json!({
        "detail": "Emporia cloud readings ingested",
        "device_gids_polled": metadata.get("site_ids").cloned().unwrap_or(JsonValue::Null),
        "devices_seen": devices_seen,
    });
    persist_integration_status(state, "Emporia", "ok", &status_meta).await?;

    // Save refreshed tokens/site ids if they changed.
    let mut persisted_metadata = metadata.clone();
    if let Some(refresh) = refresh_token {
        persisted_metadata["refresh_token"] = JsonValue::String(refresh);
    }
    if let Some(name) = username {
        persisted_metadata["username"] = JsonValue::String(name);
    }

    sqlx::query(
        r#"
        INSERT INTO setup_credentials (name, value, metadata, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (name)
        DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata, updated_at = NOW()
        "#,
    )
    .bind(EMPORIA_CREDENTIAL_NAME)
    .bind(&id_token)
    .bind(SqlJson(persisted_metadata))
    .execute(&state.db)
    .await
    .context("failed to persist Emporia credential")?;

    Ok(FeedPollResult {
        name: "Emporia".to_string(),
        status: "ok".to_string(),
    })
}

async fn persist_power_samples(
    state: &AppState,
    timestamp: chrono::DateTime<Utc>,
    usage: &crate::services::emporia::EmporiaUsageAggregate,
    meta: &JsonValue,
    included_device_gids: &std::collections::HashSet<String>,
    device_prefs: &std::collections::HashMap<
        String,
        crate::services::emporia_preferences::EmporiaDevicePreferences,
    >,
) -> Result<()> {
    let (summary_kw, summary_consumption_kwh) =
        derive_emporia_power_summary(usage, included_device_gids, device_prefs);
    let metrics = [
        ("total_kw", summary_kw),
        ("grid_kw", summary_kw),
        ("solar_kw", usage.solar_kw),
        ("consumption_kwh", summary_consumption_kwh),
        ("battery_kw", 0.0_f64),
    ];

    for (metric, value) in metrics {
        sqlx::query(
            r#"
            INSERT INTO analytics_power_samples (recorded_at, metric, value, metadata, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (recorded_at, metric)
            DO UPDATE SET value = EXCLUDED.value, metadata = EXCLUDED.metadata
            "#,
        )
        .bind(timestamp)
        .bind(metric)
        .bind(value)
        .bind(SqlJson(meta.clone()))
        .execute(&state.db)
        .await
        .with_context(|| format!("failed to persist power metric {metric}"))?;
    }

    Ok(())
}

async fn persist_integration_status(
    state: &AppState,
    name: &str,
    status: &str,
    meta: &JsonValue,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO analytics_integration_status (category, name, status, recorded_at, metadata)
        VALUES ($1, $2, $3, NOW(), $4)
        "#,
    )
    .bind("power")
    .bind(name)
    .bind(status)
    .bind(SqlJson(meta.clone()))
    .execute(&state.db)
    .await
    .context("failed to persist analytics integration status")?;
    Ok(())
}

fn is_auth_error(err: &anyhow::Error) -> bool {
    let message = err.to_string().to_lowercase();
    message.contains("401") || message.contains("unauthorized") || message.contains("forbidden")
}
