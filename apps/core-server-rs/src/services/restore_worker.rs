use anyhow::Context;
use chrono::{NaiveDate, Utc};
use reqwest::header::AUTHORIZATION;
use serde_json::Value as JsonValue;
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::backup_bundle::{NodeBackupBundle, NODE_BACKUP_SCHEMA_VERSION};
use crate::ids;
use crate::routes::node_sensors::{NodeAds1263SettingsDraft, NodeSensorDraft};
use crate::state::AppState;

const RESTORE_POLL_INTERVAL: Duration = Duration::from_secs(5);
const RESTORE_RETRY_COOLDOWN_SECONDS: i64 = 30;

pub struct RestoreWorkerService {
    state: AppState,
}

impl RestoreWorkerService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn start(self, cancel: CancellationToken) {
        let state = self.state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(RESTORE_POLL_INTERVAL);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {
                        if let Err(err) = poll_once(&state).await {
                            tracing::warn!("restore worker failed: {err:#}");
                        }
                    }
                }
            }
        });
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct RestoreJobRow {
    id: Uuid,
    backup_node_id: Uuid,
    backup_date: NaiveDate,
    target_node_id: Uuid,
}

async fn poll_once(state: &AppState) -> anyhow::Result<()> {
    let Some(job) = claim_next_job(&state.db).await? else {
        return Ok(());
    };

    let outcome = process_job(state, &job).await;
    match outcome {
        Ok(ProcessOutcome::Completed { message }) => {
            finish_job_ok(&state.db, job.id, message.as_deref()).await?;
        }
        Ok(ProcessOutcome::RetryLater { message }) => {
            requeue_job(&state.db, job.id, &message).await?;
        }
        Err(err) => {
            finish_job_error(&state.db, job.id, &err.to_string()).await?;
        }
    }

    Ok(())
}

async fn claim_next_job(db: &PgPool) -> anyhow::Result<Option<RestoreJobRow>> {
    let mut tx = db.begin().await?;
    let row: Option<RestoreJobRow> = sqlx::query_as(
        r#"
        SELECT id, backup_node_id, backup_date, target_node_id
        FROM restore_events
        WHERE status = 'queued'
          AND (
            last_attempt_at IS NULL
            OR last_attempt_at < NOW() - make_interval(secs => $1::int)
          )
        ORDER BY created_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
    )
    .bind(RESTORE_RETRY_COOLDOWN_SECONDS as i32)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(job) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let _ = sqlx::query(
        r#"
        UPDATE restore_events
        SET
            status = 'running',
            attempt_count = attempt_count + 1,
            last_attempt_at = NOW(),
            started_at = COALESCE(started_at, NOW()),
            updated_at = NOW(),
            message = NULL
        WHERE id = $1
        "#,
    )
    .bind(job.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Some(job))
}

async fn finish_job_ok(db: &PgPool, job_id: Uuid, message: Option<&str>) -> anyhow::Result<()> {
    let _ = sqlx::query(
        r#"
        UPDATE restore_events
        SET
            status = 'ok',
            message = $2,
            finished_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(message)
    .execute(db)
    .await?;

    update_target_last_restore(db, job_id, "ok", message).await?;
    Ok(())
}

async fn finish_job_error(db: &PgPool, job_id: Uuid, message: &str) -> anyhow::Result<()> {
    let _ = sqlx::query(
        r#"
        UPDATE restore_events
        SET
            status = 'error',
            message = $2,
            finished_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(message)
    .execute(db)
    .await?;

    update_target_last_restore(db, job_id, "error", Some(message)).await?;
    Ok(())
}

async fn requeue_job(db: &PgPool, job_id: Uuid, message: &str) -> anyhow::Result<()> {
    let _ = sqlx::query(
        r#"
        UPDATE restore_events
        SET
            status = 'queued',
            message = $2,
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(message)
    .execute(db)
    .await?;

    update_target_last_restore(db, job_id, "queued", Some(message)).await?;
    Ok(())
}

async fn update_target_last_restore(
    db: &PgPool,
    job_id: Uuid,
    status: &str,
    message: Option<&str>,
) -> anyhow::Result<()> {
    let row: Option<(Uuid, Uuid, NaiveDate)> = sqlx::query_as(
        r#"
        SELECT target_node_id, backup_node_id, backup_date
        FROM restore_events
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(db)
    .await?;

    let Some((target_node_id, backup_node_id, backup_date)) = row else {
        return Ok(());
    };

    let entry = serde_json::json!({
        "restore_event_id": job_id.to_string(),
        "backup_node_id": backup_node_id.to_string(),
        "date": backup_date.format("%Y-%m-%d").to_string(),
        "recorded_at": Utc::now().to_rfc3339(),
        "status": status,
        "message": message,
    });

    let _ = sqlx::query(
        r#"
        UPDATE nodes
        SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{last_restore}', $2::jsonb, true)
        WHERE id = $1
        "#,
    )
    .bind(target_node_id)
    .bind(SqlJson(entry))
    .execute(db)
    .await?;

    Ok(())
}

enum ProcessOutcome {
    Completed { message: Option<String> },
    RetryLater { message: String },
}

async fn process_job(state: &AppState, job: &RestoreJobRow) -> anyhow::Result<ProcessOutcome> {
    let date_str = job.backup_date.format("%Y-%m-%d").to_string();
    let backup_path = backup_path(
        &state.config.backup_storage_path,
        job.backup_node_id,
        &date_str,
    );
    let bytes = tokio::fs::read(&backup_path)
        .await
        .with_context(|| format!("failed to read backup file {}", backup_path.display()))?;

    let bundle: NodeBackupBundle =
        serde_json::from_slice(&bytes).context("failed to parse backup bundle JSON")?;
    if bundle.schema_version != NODE_BACKUP_SCHEMA_VERSION {
        anyhow::bail!(
            "unsupported backup bundle schema_version {} (expected {})",
            bundle.schema_version,
            NODE_BACKUP_SCHEMA_VERSION
        );
    }

    let backup_node_from_file =
        Uuid::parse_str(bundle.node_id.trim()).context("backup bundle contains invalid node_id")?;
    if backup_node_from_file != job.backup_node_id {
        anyhow::bail!(
            "backup bundle node_id {} does not match requested backup_node_id {}",
            bundle.node_id,
            job.backup_node_id
        );
    }

    let applied = apply_bundle_to_db(&state.db, job, &bundle).await?;

    match sync_node_agent_sensors(
        state,
        job.target_node_id,
        &applied.desired_sensors,
        &applied.deleted_sensor_ids,
        applied.desired_ads1263.as_ref(),
    )
    .await
    {
        Ok(()) => Ok(ProcessOutcome::Completed {
            message: Some("Restore applied to DB + node-agent.".to_string()),
        }),
        Err(SyncError::Retryable(message)) => Ok(ProcessOutcome::RetryLater { message }),
        Err(SyncError::Fatal(message)) => anyhow::bail!("{message}"),
    }
}

fn backup_path(root: &Path, node_id: Uuid, date: &str) -> PathBuf {
    root.join(node_id.to_string()).join(format!("{date}.json"))
}

#[derive(Debug, Clone)]
struct AppliedRestore {
    desired_sensors: Vec<NodeSensorDraft>,
    desired_ads1263: Option<NodeAds1263SettingsDraft>,
    deleted_sensor_ids: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct NodeRestoreRow {
    mac_eth: Option<String>,
    mac_wifi: Option<String>,
    config: SqlJson<JsonValue>,
}

fn build_core_sensor_config(sensor: &NodeSensorDraft) -> anyhow::Result<JsonValue> {
    let interval_seconds = sensor.interval_seconds.round();
    let rolling_average_seconds = sensor.rolling_average_seconds.round();
    if interval_seconds < 0.0 || rolling_average_seconds < 0.0 {
        anyhow::bail!("interval_seconds and rolling_average_seconds must be non-negative");
    }
    Ok(serde_json::json!({
        "source": "node_agent",
        "driver_type": sensor.driver_type,
        "channel": sensor.channel,
        "location": sensor.location,
        "interval_seconds": interval_seconds,
        "rolling_average_seconds": rolling_average_seconds,
        "input_min": sensor.input_min,
        "input_max": sensor.input_max,
        "output_min": sensor.output_min,
        "output_max": sensor.output_max,
        "offset": sensor.offset,
        "scale": sensor.scale,
        "pulses_per_unit": sensor.pulses_per_unit,
        "current_loop_shunt_ohms": sensor.current_loop_shunt_ohms,
        "current_loop_range_m": sensor.current_loop_range_m,
    }))
}

async fn sensor_id_exists(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    sensor_id: &str,
) -> anyhow::Result<bool> {
    let existing: Option<String> =
        sqlx::query_scalar("SELECT sensor_id FROM sensors WHERE sensor_id = $1")
            .bind(sensor_id)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(existing.is_some())
}

async fn allocate_sensor_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    mac_eth: Option<&str>,
    mac_wifi: Option<&str>,
    created_at: chrono::DateTime<chrono::Utc>,
    used: &mut BTreeSet<String>,
) -> anyhow::Result<String> {
    for counter in 0..4096u32 {
        let candidate = ids::deterministic_hex_id("sensor", mac_eth, mac_wifi, created_at, counter);
        if used.contains(&candidate) {
            continue;
        }
        if sensor_id_exists(tx, &candidate).await? {
            continue;
        }
        used.insert(candidate.clone());
        return Ok(candidate);
    }
    anyhow::bail!("Unable to allocate unique sensor id");
}

async fn apply_bundle_to_db(
    db: &PgPool,
    job: &RestoreJobRow,
    bundle: &NodeBackupBundle,
) -> anyhow::Result<AppliedRestore> {
    let mut tx = db.begin().await?;
    let Some(target) = sqlx::query_as::<_, NodeRestoreRow>(
        r#"
        SELECT
            mac_eth::text as mac_eth,
            mac_wifi::text as mac_wifi,
            COALESCE(config, '{}'::jsonb) as config
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(job.target_node_id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        anyhow::bail!("Target node not found");
    };

    let now = Utc::now();
    let mut desired_sensors: Vec<NodeSensorDraft> = bundle.desired_sensors.clone();
    let mut used_ids: BTreeSet<String> = BTreeSet::new();
    for sensor in &desired_sensors {
        let trimmed = sensor.sensor_id.trim();
        if !trimmed.is_empty() && !used_ids.insert(trimmed.to_string()) {
            anyhow::bail!("Duplicate sensor_id {trimmed} in backup bundle");
        }
    }
    let mac_eth = target.mac_eth.as_deref();
    let mac_wifi = target.mac_wifi.as_deref();
    for sensor in &mut desired_sensors {
        sensor.preset = sensor.preset.trim().to_string();
        sensor.sensor_id = sensor.sensor_id.trim().to_string();
        sensor.name = sensor.name.trim().to_string();
        sensor.driver_type = sensor.driver_type.trim().to_string();
        sensor.unit = sensor.unit.trim().to_string();
        sensor.location = sensor
            .location
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string());

        if sensor.sensor_id.is_empty() {
            sensor.sensor_id =
                allocate_sensor_id(&mut tx, mac_eth, mac_wifi, now, &mut used_ids).await?;
        }

        let existing_node: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT node_id
            FROM sensors
            WHERE sensor_id = $1
            "#,
        )
        .bind(sensor.sensor_id.trim())
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(existing_node_id) = existing_node {
            if existing_node_id != job.backup_node_id && existing_node_id != job.target_node_id {
                anyhow::bail!(
                    "sensor_id {} is already assigned to a different node ({})",
                    sensor.sensor_id,
                    existing_node_id
                );
            }
        }
    }

    let existing_ids: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT sensor_id
        FROM sensors
        WHERE node_id = $1
          AND deleted_at IS NULL
          AND COALESCE(config->>'source', '') = 'node_agent'
        "#,
    )
    .bind(job.target_node_id)
    .fetch_all(&mut *tx)
    .await?;

    let desired_id_set: BTreeSet<String> = desired_sensors
        .iter()
        .map(|sensor| sensor.sensor_id.clone())
        .collect();
    let deleted_sensor_ids: Vec<String> = existing_ids
        .into_iter()
        .filter(|sensor_id| !desired_id_set.contains(sensor_id))
        .collect();

    if !deleted_sensor_ids.is_empty() {
        for sensor_id in &deleted_sensor_ids {
            sqlx::query(
                r#"
                UPDATE sensors
                SET deleted_at = $2,
                    name = CASE WHEN name LIKE '%-deleted' THEN name ELSE name || '-deleted' END
                WHERE sensor_id = $1
                "#,
            )
            .bind(sensor_id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
    }

    for sensor in &desired_sensors {
        let config = build_core_sensor_config(sensor)?;
        let interval_seconds = sensor.interval_seconds.round().max(0.0) as i32;
        let rolling_avg_seconds = sensor.rolling_average_seconds.round().max(0.0) as i32;

        sqlx::query(
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
                config
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8)
            ON CONFLICT (sensor_id) DO UPDATE SET
                node_id = EXCLUDED.node_id,
                name = EXCLUDED.name,
                type = EXCLUDED.type,
                unit = EXCLUDED.unit,
                interval_seconds = EXCLUDED.interval_seconds,
                rolling_avg_seconds = EXCLUDED.rolling_avg_seconds,
                deleted_at = NULL,
                config = COALESCE(sensors.config, '{}'::jsonb) || EXCLUDED.config
            "#,
        )
        .bind(&sensor.sensor_id)
        .bind(job.target_node_id)
        .bind(&sensor.name)
        .bind(&sensor.preset)
        .bind(&sensor.unit)
        .bind(interval_seconds)
        .bind(rolling_avg_seconds)
        .bind(SqlJson(config))
        .execute(&mut *tx)
        .await?;
    }

    let existing_outputs: Vec<String> =
        sqlx::query_scalar("SELECT id FROM outputs WHERE node_id = $1")
            .bind(job.target_node_id)
            .fetch_all(&mut *tx)
            .await?;
    let desired_outputs: BTreeSet<String> = bundle
        .outputs
        .iter()
        .map(|output| output.id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    for output_id in existing_outputs {
        if !desired_outputs.contains(output_id.as_str()) {
            let _ = sqlx::query("DELETE FROM outputs WHERE id = $1")
                .bind(output_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    for output in &bundle.outputs {
        let output_id = output.id.trim();
        if output_id.is_empty() {
            anyhow::bail!("backup bundle output id cannot be blank");
        }

        let existing_node: Option<Uuid> =
            sqlx::query_scalar("SELECT node_id FROM outputs WHERE id = $1")
                .bind(output_id)
                .fetch_optional(&mut *tx)
                .await?;

        if let Some(existing_node_id) = existing_node {
            if existing_node_id != job.backup_node_id && existing_node_id != job.target_node_id {
                anyhow::bail!(
                    "output id {} is already assigned to a different node ({})",
                    output_id,
                    existing_node_id
                );
            }
        }

        sqlx::query(
            r#"
            INSERT INTO outputs (id, node_id, name, type, supported_states, config)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (id) DO UPDATE SET
                node_id = EXCLUDED.node_id,
                name = EXCLUDED.name,
                type = EXCLUDED.type,
                supported_states = EXCLUDED.supported_states,
                config = EXCLUDED.config
            "#,
        )
        .bind(output_id)
        .bind(job.target_node_id)
        .bind(output.name.trim())
        .bind(output.output_type.trim())
        .bind(SqlJson(output.supported_states.clone()))
        .bind(SqlJson(output.config.clone()))
        .execute(&mut *tx)
        .await?;
    }

    let mut config = target.config.0.clone();
    let config_obj = match config.as_object_mut() {
        Some(map) => map,
        None => {
            config = JsonValue::Object(Default::default());
            config.as_object_mut().unwrap()
        }
    };
    config_obj.insert(
        "desired_sensors".to_string(),
        serde_json::to_value(&desired_sensors).context("serialize desired_sensors")?,
    );
    config_obj.insert(
        "desired_sensors_updated_at".to_string(),
        JsonValue::String(now.to_rfc3339()),
    );
    if let Some(ads1263) = &bundle.desired_ads1263 {
        config_obj.insert(
            "desired_ads1263".to_string(),
            serde_json::to_value(ads1263).context("serialize desired_ads1263")?,
        );
    }
    config_obj.insert(
        "last_restore".to_string(),
        serde_json::json!({
            "backup_node_id": job.backup_node_id.to_string(),
            "date": job.backup_date.format("%Y-%m-%d").to_string(),
            "recorded_at": now.to_rfc3339(),
            "status": "running",
        }),
    );

    let _ = sqlx::query("UPDATE nodes SET name = $2, config = $3 WHERE id = $1")
        .bind(job.target_node_id)
        .bind(bundle.node_name.trim())
        .bind(SqlJson(config))
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(AppliedRestore {
        desired_sensors,
        desired_ads1263: bundle.desired_ads1263.clone(),
        deleted_sensor_ids,
    })
}

fn build_node_agent_sensor_payload(sensor: &NodeSensorDraft) -> JsonValue {
    serde_json::json!({
        "sensor_id": sensor.sensor_id,
        "name": sensor.name,
        "type": sensor.driver_type,
        "channel": sensor.channel,
        "unit": sensor.unit,
        "location": sensor.location,
        "interval_seconds": sensor.interval_seconds,
        "rolling_average_seconds": sensor.rolling_average_seconds,
        "input_min": sensor.input_min,
        "input_max": sensor.input_max,
        "output_min": sensor.output_min,
        "output_max": sensor.output_max,
        "offset": sensor.offset,
        "scale": sensor.scale,
        "pulses_per_unit": sensor.pulses_per_unit,
        "current_loop_shunt_ohms": sensor.current_loop_shunt_ohms,
        "current_loop_range_m": sensor.current_loop_range_m,
    })
}

enum SyncError {
    Retryable(String),
    Fatal(String),
}

async fn sync_node_agent_sensors(
    state: &AppState,
    node_id: Uuid,
    desired_sensors: &[NodeSensorDraft],
    deleted_sensor_ids: &[String],
    desired_ads1263: Option<&NodeAds1263SettingsDraft>,
) -> Result<(), SyncError> {
    let Some(token) = crate::node_agent_auth::node_agent_bearer_token(&state.db, node_id).await
    else {
        return Err(SyncError::Fatal(
            "Missing node-agent auth token for this node. Re-provision the node with the controller-issued token so restores can be pushed."
                .to_string(),
        ));
    };
    let auth_header = format!("Bearer {token}");
    let endpoint = crate::services::node_agent_resolver::resolve_node_agent_endpoint(
        &state.db,
        node_id,
        state.config.node_agent_port,
        false,
    )
    .await
    .map_err(|err| SyncError::Retryable(format!("Failed to locate node-agent endpoint: {err}")))?;
    let Some(endpoint) = endpoint else {
        return Err(SyncError::Retryable(
            "Target node has no reachable node-agent endpoint yet; waiting for node to come online."
                .to_string(),
        ));
    };

    let mut base_urls: Vec<String> = Vec::new();
    base_urls.push(endpoint.base_url.clone());
    if let Ok(Some(refreshed)) = crate::services::node_agent_resolver::resolve_node_agent_endpoint(
        &state.db,
        node_id,
        state.config.node_agent_port,
        true,
    )
    .await
    {
        if !base_urls.contains(&refreshed.base_url) {
            base_urls.push(refreshed.base_url);
        }
    }
    if let Some(fallback) = endpoint.ip_fallback.clone() {
        if !base_urls.contains(&fallback) {
            base_urls.push(fallback);
        }
    }

    async fn get_config(
        http: &reqwest::Client,
        base_url: &str,
        auth_header: &str,
    ) -> Result<JsonValue, SyncError> {
        let url = format!("{base_url}/v1/config");
        let resp = http
            .get(url)
            .header(AUTHORIZATION, auth_header)
            .timeout(Duration::from_secs(4))
            .send()
            .await
            .map_err(|err| SyncError::Retryable(format!("node-agent /v1/config failed: {err}")))?;
        if !resp.status().is_success() {
            return Err(SyncError::Retryable(format!(
                "node-agent /v1/config returned {}",
                resp.status()
            )));
        }
        resp.json::<JsonValue>().await.map_err(|err| {
            SyncError::Retryable(format!("node-agent /v1/config parse failed: {err}"))
        })
    }

    let mut existing_payload: Option<JsonValue> = None;
    let mut selected_base_url: Option<String> = None;
    let mut last_error: Option<SyncError> = None;
    for base_url in &base_urls {
        match get_config(&state.http, base_url, &auth_header).await {
            Ok(payload) => {
                existing_payload = Some(payload);
                selected_base_url = Some(base_url.clone());
                break;
            }
            Err(err) => {
                last_error = Some(err);
            }
        }
    }
    let existing_payload =
        existing_payload.ok_or_else(|| last_error.unwrap_or_else(|| SyncError::Retryable(
            "node-agent /v1/config failed".to_string(),
        )))?;
    let selected_base_url = selected_base_url.unwrap_or_else(|| endpoint.base_url.clone());

    let existing_sensors: Vec<JsonValue> = existing_payload
        .get("sensors")
        .and_then(|value| value.as_array())
        .map(|arr| arr.iter().cloned().collect())
        .unwrap_or_default();

    let deleted_ids: BTreeSet<&str> = deleted_sensor_ids.iter().map(|v| v.as_str()).collect();
    let desired_ids: BTreeSet<&str> = desired_sensors
        .iter()
        .map(|v| v.sensor_id.as_str())
        .collect();

    let mut merged: Vec<JsonValue> = Vec::new();
    for sensor in existing_sensors {
        let Some(sensor_id) = sensor.get("sensor_id").and_then(|v| v.as_str()) else {
            continue;
        };
        if deleted_ids.contains(sensor_id) || desired_ids.contains(sensor_id) {
            continue;
        }
        merged.push(sensor);
    }
    for sensor in desired_sensors {
        merged.push(build_node_agent_sensor_payload(sensor));
    }

    let mut payload = serde_json::json!({ "sensors": merged });
    if let Some(ads1263) = desired_ads1263 {
        payload.as_object_mut().unwrap().insert(
            "ads1263".to_string(),
            serde_json::to_value(ads1263).map_err(|err| {
                SyncError::Fatal(format!("failed to serialize ads1263 payload: {err}"))
            })?,
        );
    }

    async fn post_restore(
        http: &reqwest::Client,
        base_url: &str,
        auth_header: &str,
        payload: &JsonValue,
    ) -> Result<(), SyncError> {
        let url = format!("{base_url}/v1/config/restore");
        let resp = http
            .post(url)
            .header(AUTHORIZATION, auth_header)
            .timeout(Duration::from_secs(10))
            .json(payload)
            .send()
            .await
            .map_err(|err| SyncError::Retryable(format!("node-agent restore failed: {err}")))?;
        if resp.status().is_success() {
            return Ok(());
        }
        if resp.status().is_client_error() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(SyncError::Fatal(format!(
                "node-agent rejected restore payload ({status}): {body}"
            )));
        }
        Err(SyncError::Retryable(format!(
            "node-agent restore failed ({})",
            resp.status()
        )))
    }

    let mut restore_urls: Vec<String> = Vec::new();
    restore_urls.push(selected_base_url.clone());
    for base_url in base_urls {
        if base_url != selected_base_url {
            restore_urls.push(base_url);
        }
    }

    let mut last_error: Option<SyncError> = None;
    for base_url in restore_urls {
        match post_restore(&state.http, &base_url, &auth_header, &payload).await {
            Ok(()) => return Ok(()),
            Err(err) => match err {
                SyncError::Fatal(_) => return Err(err),
                retryable => last_error = Some(retryable),
            },
        }
    }

    Err(last_error.unwrap_or_else(|| {
        SyncError::Retryable("node-agent restore failed".to_string())
    }))
}
