use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use std::collections::HashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;
use tokio_modbus::prelude::Reader;

use crate::device_catalog::{find_model, DeviceModel, DevicePoint};
use crate::ids;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalDeviceConfig {
    pub vendor_id: String,
    pub model_id: String,
    pub protocol: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub unit_id: Option<u8>,
    pub poll_interval_seconds: Option<u64>,
    pub snmp_community: Option<String>,
    pub http_base_url: Option<String>,
    pub http_username: Option<String>,
    pub http_password: Option<String>,
    #[serde(default)]
    pub lip_username: Option<String>,
    #[serde(default)]
    pub lip_password: Option<String>,
    #[serde(default)]
    pub lip_integration_report: Option<String>,
    #[serde(default)]
    pub leap_client_cert_pem: Option<String>,
    #[serde(default)]
    pub leap_client_key_pem: Option<String>,
    #[serde(default)]
    pub leap_ca_pem: Option<String>,
    #[serde(default)]
    pub leap_verify_ca: Option<bool>,
    #[serde(default)]
    pub discovered_points: Option<Vec<DevicePoint>>,
}

#[derive(sqlx::FromRow)]
struct ExternalDeviceRow {
    id: Uuid,
    name: String,
    external_provider: Option<String>,
    external_id: Option<String>,
    config: SqlJson<JsonValue>,
    created_at: DateTime<Utc>,
}

pub struct ExternalDeviceService {
    state: AppState,
    interval: std::time::Duration,
}

impl ExternalDeviceService {
    pub fn new(state: AppState, interval: std::time::Duration) -> Self {
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
                        if let Err(err) = poll_all_devices(&state).await {
                            warn!("external device poll failed: {err:#}");
                        }
                    }
                }
            }
        });
    }
}

pub async fn poll_all_devices(state: &AppState) -> Result<()> {
    let devices: Vec<ExternalDeviceRow> = sqlx::query_as(
        r#"
        SELECT id, name, external_provider, external_id, config, created_at
        FROM nodes
        WHERE external_provider IS NOT NULL
        "#,
    )
    .fetch_all(&state.db)
    .await
    .context("failed to query external devices")?;

    for device in devices {
        if let Err(err) = poll_device(state, &device).await {
            warn!(
                node_id = %device.id,
                node_name = %device.name,
                error = %err,
                "external device poll failed"
            );
        }
    }

    Ok(())
}

pub async fn poll_device_by_id(state: &AppState, node_id: Uuid) -> Result<(String, usize)> {
    let device: ExternalDeviceRow = sqlx::query_as(
        r#"
        SELECT id, name, external_provider, external_id, config, created_at
        FROM nodes
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .fetch_one(&state.db)
    .await
    .context("failed to load external device")?;
    let mut config =
        parse_external_device_config(&device.config.0).context("invalid device config")?;
    let model =
        find_model(&config.vendor_id, &config.model_id).context("unknown device model")?;
    if let Some(points) = discover_device_points(state, &device, &config, &model).await? {
        config.discovered_points = Some(points);
        update_device_config(state, device.id, &config).await?;
    }
    let points = points_for_device(&config, &model);
    poll_device(state, &device).await?;
    Ok((model.id, points.len()))
}

pub async fn poll_device(state: &AppState, device: &ExternalDeviceRow) -> Result<()> {
    let config = parse_external_device_config(&device.config.0).context("invalid device config")?;
    let model =
        find_model(&config.vendor_id, &config.model_id).context("unknown device model")?;
    let poll_interval_seconds = config.poll_interval_seconds.unwrap_or(30).max(1);
    let now = Utc::now();
    let points = points_for_device(&config, &model);

    ensure_device_sensors(
        state,
        device,
        &config,
        &points,
        poll_interval_seconds,
    )
    .await?;

    match config.protocol.as_str() {
        "modbus_tcp" => poll_modbus_device(state, device, &config, &points, now).await?,
        "snmp" => poll_snmp_device(state, device, &config, &points, now).await?,
        "http_json" => poll_http_device(state, device, &config, &points, now).await?,
        "lutron_lip" => poll_lutron_lip_device(state, device, &config, &points, now).await?,
        "lutron_leap" => poll_lutron_leap_device(state, device, &config, &points, now).await?,
        _ => {
            warn!(
                node_id = %device.id,
                protocol = %config.protocol,
                "unsupported external device protocol"
            );
        }
    }

    mark_device_online(state, device.id, now).await?;
    Ok(())
}

fn parse_external_device_config(config: &JsonValue) -> Option<ExternalDeviceConfig> {
    config
        .get("external_device")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn points_for_device(config: &ExternalDeviceConfig, model: &DeviceModel) -> Vec<DevicePoint> {
    let mut points = model.points.clone();
    if let Some(extra) = config.discovered_points.as_ref() {
        for point in extra {
            if points.iter().any(|entry| entry.metric == point.metric) {
                continue;
            }
            points.push(point.clone());
        }
    }
    points
}

async fn update_device_config(
    state: &AppState,
    node_id: Uuid,
    config: &ExternalDeviceConfig,
) -> Result<()> {
    let config_json = json!({
        "external_device": config,
    });
    sqlx::query(
        r#"
        UPDATE nodes
        SET config = $2
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .bind(SqlJson(config_json))
    .execute(&state.db)
    .await
    .context("failed to update external device config")?;
    Ok(())
}

async fn mark_device_online(state: &AppState, node_id: Uuid, now: DateTime<Utc>) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE nodes
        SET status = 'online',
            last_seen = $2
        WHERE id = $1
        "#,
    )
    .bind(node_id)
    .bind(now)
    .execute(&state.db)
    .await
    .context("failed to update external device status")?;
    Ok(())
}

async fn ensure_device_sensors(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    poll_interval_seconds: u64,
) -> Result<()> {
    for point in points {
        let sensor_id = ids::stable_hex_id(
            "external_sensor",
            &format!("{}:{}:{}", device.id, config.model_id, point.metric),
        );
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
                config,
                deleted_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, 0, $7, NULL)
            ON CONFLICT (sensor_id) DO UPDATE SET
                node_id = EXCLUDED.node_id,
                name = EXCLUDED.name,
                type = EXCLUDED.type,
                unit = EXCLUDED.unit,
                interval_seconds = EXCLUDED.interval_seconds,
                config = EXCLUDED.config
            WHERE sensors.deleted_at IS NULL
            "#,
        )
        .bind(sensor_id)
        .bind(device.id)
        .bind(&point.name)
        .bind(&point.sensor_type)
        .bind(&point.unit)
        .bind((poll_interval_seconds as i32).max(1))
        .bind(SqlJson(json!({
            "source": "external_device",
            "vendor_id": config.vendor_id,
            "model_id": config.model_id,
            "protocol": config.protocol,
            "metric": point.metric,
            "poll_enabled": true,
        })))
        .execute(&state.db)
        .await
        .with_context(|| format!("failed to upsert external sensor {}", point.metric))?;
    }
    Ok(())
}

async fn poll_modbus_device(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    now: DateTime<Utc>,
) -> Result<()> {
    let host = config
        .host
        .as_ref()
        .context("modbus device missing host")?;
    let port = config.port.unwrap_or(502);
    let socket_addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .context("invalid modbus socket address")?;
    let mut ctx = if let Some(unit) = config.unit_id {
        tokio_modbus::client::tcp::connect_slave(socket_addr, tokio_modbus::slave::Slave(unit))
            .await
            .context("modbus connect failed")?
    } else {
        tokio_modbus::client::tcp::connect(socket_addr)
            .await
            .context("modbus connect failed")?
    };

    for point in points.iter().filter(|p| p.protocol == "modbus_tcp") {
        let register = match point.register {
            Some(register) => register,
            None => continue,
        };
        let data_type = point.data_type.as_deref().unwrap_or("u16");
        let value = read_modbus_value(&mut ctx, register, data_type)
            .await
            .with_context(|| format!("modbus read failed for {}", point.metric))?;
        let scaled = value * point.scale.unwrap_or(1.0);
        insert_metric(
            state,
            now,
            device.id,
            &config.model_id,
            &point.metric,
            scaled,
        )
        .await?;
    }

    Ok(())
}

async fn read_modbus_value(
    ctx: &mut tokio_modbus::client::Context,
    register: u32,
    data_type: &str,
) -> Result<f64> {
    let addr = register.saturating_sub(1) as u16;
    match data_type {
        "u16" => {
            let values = ctx.read_holding_registers(addr, 1).await?;
            Ok(values.get(0).copied().unwrap_or(0) as f64)
        }
        "i16" => {
            let values = ctx.read_holding_registers(addr, 1).await?;
            let raw = values.get(0).copied().unwrap_or(0);
            Ok(i16::from_be_bytes(raw.to_be_bytes()) as f64)
        }
        "u32" | "i32" | "f32_be" => {
            let values = ctx.read_holding_registers(addr, 2).await?;
            let hi = values.get(0).copied().unwrap_or(0);
            let lo = values.get(1).copied().unwrap_or(0);
            let combined = ((hi as u32) << 16) | (lo as u32);
            match data_type {
                "u32" => Ok(combined as f64),
                "i32" => Ok(i32::from_be_bytes(combined.to_be_bytes()) as f64),
                "f32_be" => Ok(f32::from_bits(combined).into()),
                _ => Ok(combined as f64),
            }
        }
        _ => {
            let values = ctx.read_holding_registers(addr, 1).await?;
            Ok(values.get(0).copied().unwrap_or(0) as f64)
        }
    }
}

async fn poll_snmp_device(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    now: DateTime<Utc>,
) -> Result<()> {
    let host = config
        .host
        .as_ref()
        .context("snmp device missing host")?;
    let port = config.port.unwrap_or(161);
    let community = config
        .snmp_community
        .clone()
        .unwrap_or_else(|| "public".to_string());
    let host = host.clone();
    let oids: Vec<(String, String)> = points
        .iter()
        .filter(|p| p.protocol == "snmp")
        .filter_map(|p| p.oid.as_ref().map(|oid| (p.metric.clone(), oid.clone())))
        .collect();

    let results = tokio::task::spawn_blocking(move || -> Result<Vec<(String, f64)>> {
        let timeout = std::time::Duration::from_secs(3);
        let mut session = snmp::SyncSession::new(
            (host.as_str(), port),
            community.as_bytes(),
            Some(timeout),
            0,
        )
        .context("snmp session init failed")?;
        let mut values = Vec::new();
        for (metric, oid) in oids {
            let oid = parse_snmp_oid(&oid)?;
            let mut response = session
                .get(&oid)
                .map_err(|err| anyhow::anyhow!("snmp get failed: {err:?}"))?;
            if let Some((_name, value)) = response.varbinds.next() {
                values.push((metric, snmp_value_to_f64(&value)));
            }
        }
        Ok(values)
    })
    .await
    .context("snmp blocking task failed")??;

    for (metric, value) in results {
        insert_metric(state, now, device.id, &config.model_id, &metric, value).await?;
    }
    Ok(())
}

fn snmp_value_to_f64(value: &snmp::Value<'_>) -> f64 {
    match value {
        snmp::Value::Integer(v) => *v as f64,
        snmp::Value::OctetString(bytes) => {
            let s = String::from_utf8_lossy(bytes);
            s.trim().parse::<f64>().unwrap_or(0.0)
        }
        snmp::Value::ObjectIdentifier(_) => 0.0,
        snmp::Value::IpAddress(addr) => {
            (addr[0] as f64) * 16777216.0
                + (addr[1] as f64) * 65536.0
                + (addr[2] as f64) * 256.0
                + (addr[3] as f64)
        }
        snmp::Value::Counter32(v) => *v as f64,
        snmp::Value::Unsigned32(v) => *v as f64,
        snmp::Value::Timeticks(v) => *v as f64,
        snmp::Value::Counter64(v) => *v as f64,
        snmp::Value::Opaque(_) => 0.0,
        snmp::Value::Null => 0.0,
        _ => 0.0,
    }
}

fn parse_snmp_oid(oid: &str) -> Result<Vec<u32>> {
    let trimmed = oid.trim().trim_start_matches('.');
    let mut parts = Vec::new();
    for part in trimmed.split('.') {
        if part.is_empty() {
            continue;
        }
        parts.push(part.parse::<u32>().with_context(|| format!("invalid OID segment {part}"))?);
    }
    if parts.is_empty() {
        Err(anyhow::anyhow!("empty OID"))
    } else {
        Ok(parts)
    }
}

async fn poll_http_device(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    now: DateTime<Utc>,
) -> Result<()> {
    let base_url = config
        .http_base_url
        .as_ref()
        .context("http device missing base_url")?;
    let client = state.http.clone();
    for point in points.iter().filter(|p| p.protocol == "http_json") {
        let path = point.path.as_deref().unwrap_or("");
        let url = format!("{}{}", base_url.trim_end_matches('/'), path);
        let mut request = client.get(url);
        if let (Some(user), Some(pass)) =
            (config.http_username.as_ref(), config.http_password.as_ref())
        {
            request = request.basic_auth(user, Some(pass));
        }
        let response = request.send().await?;
        let payload: JsonValue = response.json().await?;
        let value = point
            .json_pointer
            .as_deref()
            .and_then(|ptr| payload.pointer(ptr))
            .and_then(|val| match val {
                JsonValue::Number(num) => num.as_f64(),
                JsonValue::String(text) => text.parse::<f64>().ok(),
                _ => None,
            })
            .unwrap_or(0.0);
        insert_metric(
            state,
            now,
            device.id,
            &config.model_id,
            &point.metric,
            value,
        )
        .await?;
    }
    Ok(())
}

async fn insert_metric(
    state: &AppState,
    ts: DateTime<Utc>,
    node_id: Uuid,
    model_id: &str,
    metric: &str,
    value: f64,
) -> Result<()> {
    let sensor_id = ids::stable_hex_id(
        "external_sensor",
        &format!("{}:{}:{}", node_id, model_id, metric),
    );
    sqlx::query(
        r#"
        INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at)
        SELECT $1, $2, $3, 0, now()
        WHERE EXISTS (
            SELECT 1
            FROM sensors
            WHERE sensor_id = $1
              AND deleted_at IS NULL
              AND COALESCE(config->>'poll_enabled', 'true') <> 'false'
        )
        ON CONFLICT (sensor_id, ts)
        DO UPDATE SET
            value = EXCLUDED.value,
            inserted_at = EXCLUDED.inserted_at
        "#,
    )
    .bind(sensor_id)
    .bind(ts)
    .bind(value)
    .execute(&state.db)
    .await
    .context("failed to insert external device metric")?;
    Ok(())
}

async fn discover_device_points(
    _state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    model: &DeviceModel,
) -> Result<Option<Vec<DevicePoint>>> {
    match config.protocol.as_str() {
        "lutron_lip" => {
            let report = match config.lip_integration_report.as_deref() {
                Some(report) if !report.trim().is_empty() => report,
                _ => return Ok(None),
            };
            let outputs = parse_lutron_integration_report(report);
            if outputs.is_empty() {
                return Ok(None);
            }
            let points = outputs
                .into_iter()
                .map(|output| DevicePoint {
                    name: format!("{} Level", output.name),
                    metric: format!("{}_level_percent", slugify_metric(&output.name)),
                    sensor_type: "percentage".to_string(),
                    unit: "%".to_string(),
                    protocol: "lutron_lip".to_string(),
                    register: None,
                    data_type: None,
                    scale: None,
                    oid: None,
                    path: Some(format!("OUTPUT,{},{}", output.integration_id, output.output_id)),
                    json_pointer: None,
                    bacnet_object: None,
                })
                .collect::<Vec<_>>();
            Ok(Some(points))
        }
        "lutron_leap" => {
            let points = discover_lutron_leap_points(config)
                .await
                .with_context(|| {
                    format!(
                        "failed to discover LEAP points for device {} ({})",
                        device.name, model.id
                    )
                })?;
            if points.is_empty() {
                Ok(None)
            } else {
                Ok(Some(points))
            }
        }
        _ => Ok(None),
    }
}

async fn poll_lutron_lip_device(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    now: DateTime<Utc>,
) -> Result<()> {
    let host = config.host.as_ref().context("LIP device missing host")?;
    let port = config.port.unwrap_or(23);
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(addr).await.context("LIP connect failed")?;
    let (reader, mut writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);

    lip_login(
        &mut reader,
        &mut writer,
        config.lip_username.as_deref(),
        config.lip_password.as_deref(),
    )
    .await?;

    for point in points.iter().filter(|p| p.protocol == "lutron_lip") {
        let path = match point.path.as_deref() {
            Some(path) => path,
            None => continue,
        };
        let query = format!("?{}\r\n", path);
        writer.write_all(query.as_bytes()).await?;
        writer.flush().await?;

        let mut line = String::new();
        let response = timeout(std::time::Duration::from_secs(3), reader.read_line(&mut line))
            .await
            .context("LIP read timed out")??;
        if response == 0 {
            continue;
        }
        if let Some(value) = parse_lip_output_value(&line) {
            insert_metric(state, now, device.id, &config.model_id, &point.metric, value).await?;
        }
    }

    Ok(())
}

async fn poll_lutron_leap_device(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    now: DateTime<Utc>,
) -> Result<()> {
    let host = config.host.as_ref().context("LEAP device missing host")?;
    let port = config.port.unwrap_or(8081);
    let mut connection = leap_connect(host, port, config).await?;
    let mut grouped: HashMap<String, Vec<&DevicePoint>> = HashMap::new();
    for point in points.iter().filter(|p| p.protocol == "lutron_leap") {
        let path = match point.path.as_deref() {
            Some(path) => path.to_string(),
            None => continue,
        };
        grouped.entry(path).or_default().push(point);
    }

    for (path, point_group) in grouped {
        let status_url = format!("{}/status", path.trim_end_matches('/'));
        let response = leap_read_request(&mut connection, &status_url).await?;
        for point in point_group {
            let json_pointer = match point.json_pointer.as_deref() {
                Some(pointer) => pointer,
                None => continue,
            };
            let value = response
                .pointer(json_pointer)
                .and_then(leap_value_to_f64);
            if let Some(value) = value {
                insert_metric(state, now, device.id, &config.model_id, &point.metric, value)
                    .await?;
            }
        }
    }

    Ok(())
}

async fn lip_login(
    reader: &mut BufReader<tokio::io::ReadHalf<TcpStream>>,
    writer: &mut tokio::io::WriteHalf<TcpStream>,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<()> {
    let mut line = String::new();
    for _ in 0..4 {
        line.clear();
        let read = timeout(std::time::Duration::from_secs(2), reader.read_line(&mut line)).await;
        let Ok(Ok(count)) = read else { continue };
        if count == 0 {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("login") {
            if let Some(username) = username {
                writer.write_all(format!("{}\r\n", username).as_bytes()).await?;
                writer.flush().await?;
            }
        } else if lower.contains("password") {
            if let Some(password) = password {
                writer.write_all(format!("{}\r\n", password).as_bytes()).await?;
                writer.flush().await?;
            }
        }
    }
    Ok(())
}

fn parse_lip_output_value(line: &str) -> Option<f64> {
    let trimmed = line.trim();
    if !trimmed.starts_with("~OUTPUT") {
        return None;
    }
    let parts: Vec<&str> = trimmed.split(',').collect();
    parts.last()?.trim().parse::<f64>().ok()
}

struct LipOutput {
    integration_id: u32,
    output_id: u32,
    name: String,
}

fn parse_lutron_integration_report(report: &str) -> Vec<LipOutput> {
    let mut outputs = Vec::new();
    for raw_line in report.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if !upper.contains("OUTPUT") {
            continue;
        }
        let sanitized = line.replace('"', "");
        let parts: Vec<&str> = sanitized.split(',').map(|entry| entry.trim()).collect();
        let (integration_id, output_id, name) = if parts.len() >= 4
            && parts[1].parse::<u32>().is_ok()
            && parts[2].parse::<u32>().is_ok()
        {
            (
                parts[1].parse::<u32>().unwrap_or(1),
                parts[2].parse::<u32>().unwrap_or(0),
                parts[3].to_string(),
            )
        } else if parts.len() >= 3 && parts[1].parse::<u32>().is_ok() {
            (
                1,
                parts[1].parse::<u32>().unwrap_or(0),
                parts[2].to_string(),
            )
        } else {
            let numbers: Vec<u32> = line
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<u32>().ok())
                .collect();
            if numbers.is_empty() {
                continue;
            }
            let integration_id = if numbers.len() > 1 { numbers[0] } else { 1 };
            let output_id = if numbers.len() > 1 { numbers[1] } else { numbers[0] };
            (integration_id, output_id, format!("Output {}", output_id))
        };
        if output_id == 0 {
            continue;
        }
        outputs.push(LipOutput {
            integration_id,
            output_id,
            name: if name.is_empty() {
                format!("Output {}", output_id)
            } else {
                name
            },
        });
    }
    outputs
}

struct LeapConnection {
    reader: BufReader<tokio::io::ReadHalf<tokio_rustls::client::TlsStream<TcpStream>>>,
    writer: tokio::io::WriteHalf<tokio_rustls::client::TlsStream<TcpStream>>,
}

async fn leap_connect(host: &str, port: u16, config: &ExternalDeviceConfig) -> Result<LeapConnection> {
    let cert_pem = config
        .leap_client_cert_pem
        .as_deref()
        .context("LEAP client cert missing")?;
    let key_pem = config
        .leap_client_key_pem
        .as_deref()
        .context("LEAP client key missing")?;
    let ca_pem = config.leap_ca_pem.as_deref();

    let certs = load_certs_from_pem(cert_pem).context("invalid LEAP client cert")?;
    let key = load_private_key_from_pem(key_pem).context("invalid LEAP client key")?;

    let mut roots = RootCertStore::empty();
    if let Some(ca_pem) = ca_pem {
        for cert in load_certs_from_pem(ca_pem).context("invalid LEAP CA cert")? {
            roots.add(cert)?;
        }
    }

    if config.leap_verify_ca.unwrap_or(true) && roots.is_empty() {
        return Err(anyhow::anyhow!(
            "LEAP requires a CA certificate (leap_ca_pem) when verification is enabled"
        ));
    }

    let tls_config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_client_auth_cert(certs, key)?;

    let connector = TlsConnector::from(Arc::new(tls_config));
    let addr = format!("{}:{}", host, port);
    let stream = TcpStream::connect(addr).await.context("LEAP connect failed")?;
    let server_name = ServerName::try_from(host.to_string()).context("invalid LEAP host name")?;
    let tls = connector
        .connect(server_name, stream)
        .await
        .context("LEAP TLS handshake failed")?;
    let (reader, writer) = tokio::io::split(tls);
    Ok(LeapConnection {
        reader: BufReader::new(reader),
        writer,
    })
}

async fn leap_read_request(connection: &mut LeapConnection, url: &str) -> Result<JsonValue> {
    let tag = Uuid::new_v4().to_string();
    let payload = json!({
        "CommuniqueType": "ReadRequest",
        "Header": {
            "ClientTag": tag,
            "Url": url,
        },
    });
    let message = format!("{}\n", payload.to_string());
    connection.writer.write_all(message.as_bytes()).await?;
    connection.writer.flush().await?;

    let mut line = String::new();
    loop {
        line.clear();
        let read = timeout(std::time::Duration::from_secs(5), connection.reader.read_line(&mut line))
            .await
            .context("LEAP response timeout")??;
        if read == 0 {
            return Err(anyhow::anyhow!("LEAP connection closed"));
        }
        if let Ok(response) = serde_json::from_str::<JsonValue>(line.trim()) {
            let resp_tag = response
                .get("Header")
                .and_then(|header| header.get("ClientTag"))
                .and_then(|tag| tag.as_str());
            if resp_tag == Some(tag.as_str()) {
                return Ok(response);
            }
        }
    }
}

async fn discover_lutron_leap_points(config: &ExternalDeviceConfig) -> Result<Vec<DevicePoint>> {
    let host = config.host.as_ref().context("LEAP device missing host")?;
    let port = config.port.unwrap_or(8081);
    let mut connection = leap_connect(host, port, config).await?;
    let mut zones = discover_leap_zones(&mut connection).await?;
    if zones.is_empty() {
        return Ok(Vec::new());
    }

    let mut points = Vec::new();
    for zone in zones.drain(..) {
        let base_metric = slugify_metric(&zone.name);
        let path = zone.href.clone();
        let zone_label = if zone.name.is_empty() {
            format!("Zone {}", zone.href)
        } else {
            zone.name.clone()
        };
        points.push(DevicePoint {
            name: format!("{} Level", zone_label),
            metric: format!("{}_level_percent", base_metric),
            sensor_type: "percentage".to_string(),
            unit: "%".to_string(),
            protocol: "lutron_leap".to_string(),
            register: None,
            data_type: None,
            scale: None,
            oid: None,
            path: Some(path.clone()),
            json_pointer: Some("/Body/ZoneStatus/Level".to_string()),
            bacnet_object: None,
        });
        points.push(DevicePoint {
            name: format!("{} Switch", zone_label),
            metric: format!("{}_switch", base_metric),
            sensor_type: "status".to_string(),
            unit: "".to_string(),
            protocol: "lutron_leap".to_string(),
            register: None,
            data_type: None,
            scale: None,
            oid: None,
            path: Some(path.clone()),
            json_pointer: Some("/Body/ZoneStatus/SwitchedLevel".to_string()),
            bacnet_object: None,
        });
        points.push(DevicePoint {
            name: format!("{} Fan Speed", zone_label),
            metric: format!("{}_fan_speed", base_metric),
            sensor_type: "percentage".to_string(),
            unit: "%".to_string(),
            protocol: "lutron_leap".to_string(),
            register: None,
            data_type: None,
            scale: None,
            oid: None,
            path: Some(path.clone()),
            json_pointer: Some("/Body/ZoneStatus/FanSpeed".to_string()),
            bacnet_object: None,
        });
        points.push(DevicePoint {
            name: format!("{} Shade Tilt", zone_label),
            metric: format!("{}_tilt", base_metric),
            sensor_type: "percentage".to_string(),
            unit: "%".to_string(),
            protocol: "lutron_leap".to_string(),
            register: None,
            data_type: None,
            scale: None,
            oid: None,
            path: Some(path.clone()),
            json_pointer: Some("/Body/ZoneStatus/Tilt".to_string()),
            bacnet_object: None,
        });
        points.push(DevicePoint {
            name: format!("{} Availability", zone_label),
            metric: format!("{}_availability", base_metric),
            sensor_type: "status".to_string(),
            unit: "".to_string(),
            protocol: "lutron_leap".to_string(),
            register: None,
            data_type: None,
            scale: None,
            oid: None,
            path: Some(path.clone()),
            json_pointer: Some("/Body/ZoneStatus/Availability".to_string()),
            bacnet_object: None,
        });
    }

    Ok(points)
}

struct LeapZone {
    href: String,
    name: String,
}

async fn discover_leap_zones(connection: &mut LeapConnection) -> Result<Vec<LeapZone>> {
    let zones = leap_read_request(connection, "/zone").await?;
    if let Some(zone_defs) = zones
        .pointer("/Body/Zones")
        .and_then(|value| value.as_array())
    {
        let mut entries = Vec::new();
        for zone in zone_defs {
            let href = zone.get("href").and_then(|value| value.as_str());
            let name = zone.get("Name").and_then(|value| value.as_str()).unwrap_or("");
            if let Some(href) = href {
                entries.push(LeapZone {
                    href: href.to_string(),
                    name: name.to_string(),
                });
            }
        }
        if !entries.is_empty() {
            return Ok(entries);
        }
    }

    let areas = leap_read_request(connection, "/area").await?;
    let area_defs = areas
        .pointer("/Body/Areas")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let mut zones = Vec::new();
    for area in area_defs {
        let href = area.get("href").and_then(|value| value.as_str()).unwrap_or("");
        if href.is_empty() {
            continue;
        }
        let associated = leap_read_request(
            connection,
            &format!("{}/associatedzone", href.trim_end_matches('/')),
        )
        .await?;
        if let Some(zone_defs) = associated
            .pointer("/Body/Zones")
            .and_then(|value| value.as_array())
        {
            for zone in zone_defs {
                let href = zone.get("href").and_then(|value| value.as_str());
                let name = zone.get("Name").and_then(|value| value.as_str()).unwrap_or("");
                if let Some(href) = href {
                    zones.push(LeapZone {
                        href: href.to_string(),
                        name: name.to_string(),
                    });
                }
            }
        }
    }
    Ok(zones)
}

fn leap_value_to_f64(value: &JsonValue) -> Option<f64> {
    match value {
        JsonValue::Number(num) => num.as_f64(),
        JsonValue::String(text) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "on" | "open" | "available" => Some(1.0),
                "off" | "closed" | "unavailable" => Some(0.0),
                "mixed" => Some(0.5),
                _ => normalized.parse::<f64>().ok(),
            }
        }
        _ => None,
    }
}

fn load_certs_from_pem(pem: &str) -> Result<Vec<CertificateDer<'static>>> {
    let mut cursor = Cursor::new(pem.as_bytes());
    let mut certs = Vec::new();
    for cert in rustls_pemfile::certs(&mut cursor) {
        certs.push(cert.context("invalid cert")?);
    }
    Ok(certs)
}

fn load_private_key_from_pem(pem: &str) -> Result<PrivateKeyDer<'static>> {
    let mut cursor = Cursor::new(pem.as_bytes());
    let key = rustls_pemfile::private_key(&mut cursor)
        .context("invalid private key")?
        .context("private key missing")?;
    Ok(key)
}

fn slugify_metric(name: &str) -> String {
    let mut out = String::new();
    let mut last_underscore = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_underscore = false;
        } else if !last_underscore {
            out.push('_');
            last_underscore = true;
        }
    }
    if out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "zone".to_string()
    } else {
        out
    }
}
