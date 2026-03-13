use anyhow::{anyhow, Context, Result};
use bacnet_client::client::BACnetClient;
use bacnet_encoding::primitives::decode_application_value;
use bacnet_transport::bip::{BipTransport, ForeignDeviceConfig};
use bacnet_types::enums::{BvlcResultCode, EngineeringUnits, ObjectType, PropertyIdentifier};
use bacnet_types::primitives::{ObjectIdentifier, PropertyValue};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::types::Json as SqlJson;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_modbus::prelude::Reader;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;
use tokio_util::sync::CancellationToken;
use tracing::warn;
use url::Url;
use uuid::Uuid;

use crate::device_catalog::{find_model, DeviceModel, DevicePoint};
use crate::ids;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    #[serde(default)]
    pub bacnet_device_instance: Option<u32>,
    #[serde(default)]
    pub bacnet_vendor_id: Option<u16>,
    #[serde(default)]
    pub bacnet_bbmd_host: Option<String>,
    #[serde(default)]
    pub bacnet_bbmd_port: Option<u16>,
    #[serde(default)]
    pub bacnet_foreign_ttl_seconds: Option<u16>,
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
    let original_config =
        parse_external_device_config(&device.config.0).context("invalid device config")?;
    let mut config = normalize_runtime_external_device_config(state, &original_config).await?;
    let model = find_model(&config.vendor_id, &config.model_id).context("unknown device model")?;
    let discovery = discover_device_points(state, &device, &config, &model).await?;
    if let Some(points) = discovery.points {
        config.discovered_points = Some(points);
    }
    if let Some(instance) = discovery.bacnet_device_instance {
        config.bacnet_device_instance = Some(instance);
    }
    if let Some(vendor_id) = discovery.bacnet_vendor_id {
        config.bacnet_vendor_id = Some(vendor_id);
    }
    if config != original_config {
        update_device_config(state, device.id, &config).await?;
    }
    let points = points_for_device(&config, &model);
    match poll_device_with_config(state, &device, &config, &model).await {
        Ok(()) => Ok((model.id, points.len())),
        Err(modbus_err)
            if config.protocol == "modbus_tcp"
                && model.protocols.iter().any(|protocol| protocol == "bacnet_ip") =>
        {
            match try_bacnet_fallback_sync(state, &device, &config, &model).await {
                Ok(Some(fallback_config)) => {
                    let fallback_points = points_for_device(&fallback_config, &model);
                    Ok((model.id, fallback_points.len()))
                }
                Ok(None) => Err(modbus_err),
                Err(bacnet_err) => Err(anyhow!(
                    "modbus polling failed ({modbus_err}); BACnet fallback failed ({bacnet_err})"
                )),
            }
        }
        Err(err) => Err(err),
    }
}

async fn poll_device(state: &AppState, device: &ExternalDeviceRow) -> Result<()> {
    let original_config =
        parse_external_device_config(&device.config.0).context("invalid device config")?;
    let config = normalize_runtime_external_device_config(state, &original_config).await?;
    if config != original_config {
        update_device_config(state, device.id, &config).await?;
    }
    let model = find_model(&config.vendor_id, &config.model_id).context("unknown device model")?;
    poll_device_with_config(state, device, &config, &model).await
}

async fn poll_device_with_config(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    model: &DeviceModel,
) -> Result<()> {
    let poll_interval_seconds = config.poll_interval_seconds.unwrap_or(30).max(1);
    let now = Utc::now();
    let points = points_for_device(&config, &model);

    ensure_device_sensors(state, device, &config, &points, poll_interval_seconds).await?;

    match config.protocol.as_str() {
        "modbus_tcp" => poll_modbus_device(state, device, &config, &points, now).await?,
        "bacnet_ip" => poll_bacnet_device(state, device, &config, &points, now).await?,
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

#[derive(Default)]
struct DeviceDiscoveryResult {
    points: Option<Vec<DevicePoint>>,
    bacnet_device_instance: Option<u32>,
    bacnet_vendor_id: Option<u16>,
}

fn parse_external_device_config(config: &JsonValue) -> Option<ExternalDeviceConfig> {
    config
        .get("external_device")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn parse_hostname_from_url(value: &str) -> Option<String> {
    Url::parse(value)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
}

fn normalize_http_device_config(config: &mut ExternalDeviceConfig) {
    if config.protocol != "http_json" {
        return;
    }
    let host = config.host.as_deref().map(str::trim).filter(|value| !value.is_empty());
    let base_url = config
        .http_base_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let normalized_base_url = if let Some(url) = base_url {
        Some(url.to_string())
    } else if let Some(host_text) = host {
        if host_text.starts_with("http://") || host_text.starts_with("https://") {
            Some(host_text.to_string())
        } else {
            Some(format!("http://{host_text}"))
        }
    } else {
        None
    };

    let normalized_base_url = normalized_base_url.map(|value| {
        if config.vendor_id == "metasys" && config.model_id == "metasys_server" {
            if let Ok(mut parsed) = Url::parse(&value) {
                if parsed.path().is_empty() || parsed.path() == "/" {
                    parsed.set_path("/metasys");
                    return parsed.to_string();
                }
            }
        }
        value
    });

    if let Some(url) = normalized_base_url.as_deref() {
        config.host = parse_hostname_from_url(url).or_else(|| config.host.clone());
    }
    config.http_base_url = normalized_base_url;
}

fn gateway_host_from_config(config: &ExternalDeviceConfig) -> Option<String> {
    config
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            config
                .http_base_url
                .as_deref()
                .and_then(parse_hostname_from_url)
        })
}

async fn infer_bacnet_bbmd_host(state: &AppState) -> Result<Option<String>> {
    let rows: Vec<(SqlJson<JsonValue>,)> = sqlx::query_as(
        r#"
        SELECT config
        FROM nodes
        WHERE external_provider = 'metasys'
        ORDER BY last_seen DESC NULLS LAST, created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .context("failed to inspect Metasys gateway config")?;

    for (config_json,) in rows {
        if let Some(config) = parse_external_device_config(&config_json.0) {
            if let Some(host) = gateway_host_from_config(&config) {
                return Ok(Some(host));
            }
        }
    }
    Ok(None)
}

async fn normalize_runtime_external_device_config(
    state: &AppState,
    config: &ExternalDeviceConfig,
) -> Result<ExternalDeviceConfig> {
    let mut normalized = config.clone();
    normalize_http_device_config(&mut normalized);
    if normalized.protocol == "bacnet_ip"
        && normalized
            .bacnet_bbmd_host
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        if let Some(host) = infer_bacnet_bbmd_host(state).await? {
            normalized.bacnet_bbmd_host = Some(host);
        }
    }
    Ok(normalized)
}

async fn try_bacnet_fallback_sync(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    model: &DeviceModel,
) -> Result<Option<ExternalDeviceConfig>> {
    if config.protocol != "modbus_tcp" || !model.protocols.iter().any(|protocol| protocol == "bacnet_ip")
    {
        return Ok(None);
    }

    let mut fallback = config.clone();
    fallback.protocol = "bacnet_ip".to_string();
    fallback.port = Some(if config.port == Some(502) { 47808 } else { config.port.unwrap_or(47808) });
    let mut fallback = normalize_runtime_external_device_config(state, &fallback).await?;
    let discovery = discover_device_points(state, device, &fallback, model).await?;
    if let Some(points) = discovery.points {
        fallback.discovered_points = Some(points);
    }
    if let Some(instance) = discovery.bacnet_device_instance {
        fallback.bacnet_device_instance = Some(instance);
    }
    if let Some(vendor_id) = discovery.bacnet_vendor_id {
        fallback.bacnet_vendor_id = Some(vendor_id);
    }
    update_device_config(state, device.id, &fallback).await?;
    poll_device_with_config(state, device, &fallback, model).await?;
    Ok(Some(fallback))
}

fn points_for_device(config: &ExternalDeviceConfig, model: &DeviceModel) -> Vec<DevicePoint> {
    let mut points = model
        .points
        .iter()
        .filter(|point| point.protocol == config.protocol)
        .cloned()
        .collect::<Vec<_>>();
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
    let host = config.host.as_ref().context("modbus device missing host")?;
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

    let mut success_count = 0usize;
    let mut failed_metrics = Vec::new();
    for point in points.iter().filter(|p| p.protocol == "modbus_tcp") {
        let register = match point.register {
            Some(register) => register,
            None => continue,
        };
        let data_type = point.data_type.as_deref().unwrap_or("u16");
        let value = match read_modbus_value(&mut ctx, register, data_type).await {
            Ok(value) => value,
            Err(err) => {
                failed_metrics.push(format!("{} ({err})", point.metric));
                continue;
            }
        };
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
        success_count += 1;
    }

    if success_count == 0 {
        if !failed_metrics.is_empty() {
            return Err(anyhow!(
                "modbus reads failed for all points: {}",
                failed_metrics.join(", ")
            ));
        }
        return Err(anyhow!("modbus device had no readable points configured"));
    }

    if !failed_metrics.is_empty() {
        warn!(
            node_id = %device.id,
            host = config.host.as_deref().unwrap_or(""),
            failed = %failed_metrics.join(", "),
            "some modbus points failed to read"
        );
    }

    Ok(())
}

async fn read_modbus_value(
    ctx: &mut tokio_modbus::client::Context,
    register: u32,
    data_type: &str,
) -> Result<f64> {
    let (addr, use_input_registers) = if (30001..=39999).contains(&register) {
        (register.saturating_sub(30001) as u16, true)
    } else if (40001..=49999).contains(&register) {
        (register.saturating_sub(40001) as u16, false)
    } else {
        (register.saturating_sub(1) as u16, false)
    };
    match data_type {
        "u16" => {
            let values = if use_input_registers {
                ctx.read_input_registers(addr, 1).await?
            } else {
                ctx.read_holding_registers(addr, 1).await?
            };
            Ok(values.get(0).copied().unwrap_or(0) as f64)
        }
        "i16" => {
            let values = if use_input_registers {
                ctx.read_input_registers(addr, 1).await?
            } else {
                ctx.read_holding_registers(addr, 1).await?
            };
            let raw = values.get(0).copied().unwrap_or(0);
            Ok(i16::from_be_bytes(raw.to_be_bytes()) as f64)
        }
        "u32" | "i32" | "f32_be" => {
            let values = if use_input_registers {
                ctx.read_input_registers(addr, 2).await?
            } else {
                ctx.read_holding_registers(addr, 2).await?
            };
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
            let values = if use_input_registers {
                ctx.read_input_registers(addr, 1).await?
            } else {
                ctx.read_holding_registers(addr, 1).await?
            };
            Ok(values.get(0).copied().unwrap_or(0) as f64)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ExternalDeviceSweepCandidate {
    pub host: String,
    pub display_name: Option<String>,
    pub protocols: Vec<String>,
    pub vendor_id: Option<String>,
    pub model_id: Option<String>,
    pub notes: Vec<String>,
}

struct BacnetIdentity {
    device_instance: u32,
    vendor_id: u16,
    object_name: Option<String>,
}

pub async fn sweep_external_network(
    range_expr: Option<&str>,
) -> Result<(String, Vec<ExternalDeviceSweepCandidate>)> {
    let (label, hosts) = resolve_scan_hosts(range_expr)?;
    let candidates = futures::stream::iter(hosts.into_iter().map(|host| async move {
        probe_external_host(host).await
    }))
    .buffer_unordered(24)
    .collect::<Vec<_>>()
    .await;

    let mut out = Vec::new();
    for candidate in candidates {
        if let Some(entry) = candidate? {
            out.push(entry);
        }
    }
    out.sort_by(|a, b| a.host.cmp(&b.host));
    Ok((label, out))
}

async fn probe_external_host(host: Ipv4Addr) -> Result<Option<ExternalDeviceSweepCandidate>> {
    let mut protocols = Vec::new();
    let mut notes = Vec::new();
    let mut display_name = None;
    let mut vendor_id = None;
    let mut model_id = None;

    if tcp_port_open(host, 502).await {
        protocols.push("modbus_tcp".to_string());
        notes.push("Modbus TCP port 502 accepted a connection.".to_string());
    }

    if let Ok(Some(identity)) = probe_bacnet_identity(host, 47808).await {
        protocols.push("bacnet_ip".to_string());
        display_name = identity.object_name.clone();
        notes.push(format!(
            "BACnet/IP device instance {} responded on UDP 47808.",
            identity.device_instance
        ));
        if let Some((suggested_vendor_id, suggested_model_id)) =
            classify_bacnet_identity(identity.object_name.as_deref(), identity.vendor_id)
        {
            vendor_id = Some(suggested_vendor_id.to_string());
            model_id = Some(suggested_model_id.to_string());
        }
    }

    if protocols.is_empty() {
        return Ok(None);
    }

    protocols.sort();
    protocols.dedup();
    Ok(Some(ExternalDeviceSweepCandidate {
        host: host.to_string(),
        display_name,
        protocols,
        vendor_id,
        model_id,
        notes,
    }))
}

fn resolve_scan_hosts(range_expr: Option<&str>) -> Result<(String, Vec<Ipv4Addr>)> {
    let expr = range_expr.map(str::trim).filter(|value| !value.is_empty());
    match expr {
        None => default_local_scan_hosts(),
        Some(value) if value.contains('/') => hosts_from_cidr(value),
        Some(value) if value.contains('-') => hosts_from_range(value),
        Some(value) => {
            let ip: Ipv4Addr = value.parse().with_context(|| format!("invalid IPv4 {value}"))?;
            Ok((ip.to_string(), vec![ip]))
        }
    }
}

fn default_local_scan_hosts() -> Result<(String, Vec<Ipv4Addr>)> {
    let interfaces = if_addrs::get_if_addrs().context("failed to enumerate local interfaces")?;
    let iface = interfaces
        .into_iter()
        .find_map(|iface| match iface.addr {
            if_addrs::IfAddr::V4(v4)
                if !iface.is_loopback()
                    && is_private_ipv4(v4.ip)
                    && v4.prefixlen <= 30 =>
            {
                Some(v4)
            }
            _ => None,
        })
        .context("no active private IPv4 interface found for local sweep")?;
    let network = ipv4_and_mask(iface.ip, iface.netmask);
    let label = format!("{network}/{}", iface.prefixlen);
    let hosts = enumerate_ipv4_hosts(network, iface.prefixlen)?;
    Ok((label, hosts))
}

fn hosts_from_cidr(expr: &str) -> Result<(String, Vec<Ipv4Addr>)> {
    let (ip_text, prefix_text) = expr
        .split_once('/')
        .ok_or_else(|| anyhow!("invalid CIDR range {expr}"))?;
    let ip: Ipv4Addr = ip_text
        .trim()
        .parse()
        .with_context(|| format!("invalid IPv4 {ip_text}"))?;
    let prefix: u8 = prefix_text
        .trim()
        .parse()
        .with_context(|| format!("invalid prefix {prefix_text}"))?;
    let network = ipv4_with_prefix_network(ip, prefix)?;
    let label = format!("{network}/{prefix}");
    let hosts = enumerate_ipv4_hosts(network, prefix)?;
    Ok((label, hosts))
}

fn hosts_from_range(expr: &str) -> Result<(String, Vec<Ipv4Addr>)> {
    let (start_text, end_text) = expr
        .split_once('-')
        .ok_or_else(|| anyhow!("invalid range {expr}"))?;
    let start: Ipv4Addr = start_text
        .trim()
        .parse()
        .with_context(|| format!("invalid IPv4 {start_text}"))?;
    let end: Ipv4Addr = end_text
        .trim()
        .parse()
        .with_context(|| format!("invalid IPv4 {end_text}"))?;
    let start_raw = ipv4_to_u32(start);
    let end_raw = ipv4_to_u32(end);
    anyhow::ensure!(start_raw <= end_raw, "range start must be <= range end");
    let span = end_raw - start_raw + 1;
    anyhow::ensure!(span <= 1024, "scan range too large; limit is 1024 hosts");
    let hosts = (start_raw..=end_raw).map(u32_to_ipv4).collect::<Vec<_>>();
    Ok((format!("{}-{}", start, end), hosts))
}

fn enumerate_ipv4_hosts(network: Ipv4Addr, prefix: u8) -> Result<Vec<Ipv4Addr>> {
    anyhow::ensure!(prefix <= 30, "scan ranges must be /30 or broader");
    let host_bits = 32u32.saturating_sub(prefix as u32);
    let total = 1u32
        .checked_shl(host_bits)
        .ok_or_else(|| anyhow!("invalid prefix {prefix}"))?;
    let usable = total.saturating_sub(2);
    anyhow::ensure!(usable <= 1024, "scan range too large; limit is 1024 hosts");
    let start = ipv4_to_u32(network).saturating_add(1);
    let end = ipv4_to_u32(network).saturating_add(total.saturating_sub(2));
    Ok((start..=end).map(u32_to_ipv4).collect())
}

fn ipv4_to_u32(ip: Ipv4Addr) -> u32 {
    u32::from_be_bytes(ip.octets())
}

fn u32_to_ipv4(value: u32) -> Ipv4Addr {
    Ipv4Addr::from(value.to_be_bytes())
}

fn ipv4_with_prefix_network(ip: Ipv4Addr, prefix: u8) -> Result<Ipv4Addr> {
    anyhow::ensure!(prefix <= 30, "prefix must be between 0 and 30");
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix as u32)
    };
    Ok(u32_to_ipv4(ipv4_to_u32(ip) & mask))
}

fn ipv4_and_mask(ip: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    u32_to_ipv4(ipv4_to_u32(ip) & ipv4_to_u32(mask))
}

fn ipv4_broadcast(network: Ipv4Addr, mask: Ipv4Addr) -> Ipv4Addr {
    let host_mask = !ipv4_to_u32(mask);
    u32_to_ipv4(ipv4_to_u32(network) | host_mask)
}

fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.octets()[0] == 100
}

async fn tcp_port_open(host: Ipv4Addr, port: u16) -> bool {
    matches!(
        timeout(std::time::Duration::from_millis(700), TcpStream::connect((host, port))).await,
        Ok(Ok(_))
    )
}

async fn poll_bacnet_device(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    now: DateTime<Utc>,
) -> Result<()> {
    let host = config.host.as_ref().context("bacnet device missing host")?;
    let host_ip = parse_ipv4_host(host)?;
    let port = config.port.unwrap_or(47808);
    let mut client = create_bacnet_client(config, host_ip, port).await?;
    let identity = discover_or_resolve_bacnet_identity(&client, host_ip, port, config).await?;
    let direct_target = config
        .bacnet_device_instance
        .map(|_| bip_mac(host_ip, port));
    let mut success_count = 0usize;
    let mut failed_metrics = Vec::new();

    for point in points.iter().filter(|p| p.protocol == "bacnet_ip") {
        let bacnet_object = match point.bacnet_object.as_deref() {
            Some(value) => value,
            None => continue,
        };
        let object_identifier = parse_bacnet_object_identifier(bacnet_object)?;
        let property = match bacnet_read_property_value(
            &client,
            identity.device_instance,
            direct_target.as_deref(),
            object_identifier,
            PropertyIdentifier::PRESENT_VALUE,
            None,
        )
        .await
        {
            Ok(property) => property,
            Err(err) => {
                failed_metrics.push(format!("{} ({err})", point.metric));
                continue;
            }
        };
        let value = match property_value_to_f64(&property) {
            Some(value) => value,
            None => {
                failed_metrics.push(format!(
                    "{} (unsupported presentValue shape)",
                    point.metric
                ));
                continue;
            }
        };
        insert_metric(
            state,
            now,
            device.id,
            &config.model_id,
            &point.metric,
            value,
        )
        .await?;
        success_count += 1;
    }

    client.stop().await.ok();

    if success_count == 0 {
        return Err(anyhow!("no BACnet points could be read"));
    }
    if !failed_metrics.is_empty() {
        warn!(
            node_id = %device.id,
            host = config.host.as_deref().unwrap_or(""),
            failed = %failed_metrics.join(", "),
            "some BACnet points failed to read"
        );
    }
    Ok(())
}

async fn probe_bacnet_identity(host: Ipv4Addr, port: u16) -> Result<Option<BacnetIdentity>> {
    let temp_config = ExternalDeviceConfig {
        vendor_id: String::new(),
        model_id: String::new(),
        protocol: "bacnet_ip".to_string(),
        host: Some(host.to_string()),
        port: Some(port),
        unit_id: None,
        poll_interval_seconds: None,
        snmp_community: None,
        http_base_url: None,
        http_username: None,
        http_password: None,
        lip_username: None,
        lip_password: None,
        lip_integration_report: None,
        leap_client_cert_pem: None,
        leap_client_key_pem: None,
        leap_ca_pem: None,
        leap_verify_ca: None,
        discovered_points: None,
        bacnet_device_instance: None,
        bacnet_vendor_id: None,
        bacnet_bbmd_host: None,
        bacnet_bbmd_port: None,
        bacnet_foreign_ttl_seconds: None,
    };
    let mut client = create_bacnet_client(&temp_config, host, port).await?;
    let result = discover_bacnet_identity(&client, host, port).await;
    client.stop().await.ok();
    result
}

async fn discover_bacnet_points(
    config: &ExternalDeviceConfig,
) -> Result<DeviceDiscoveryResult> {
    let host = config.host.as_ref().context("bacnet device missing host")?;
    let host_ip = parse_ipv4_host(host)?;
    let port = config.port.unwrap_or(47808);
    let mut client = create_bacnet_client(config, host_ip, port).await?;
    let identity = discover_or_resolve_bacnet_identity(&client, host_ip, port, config).await?;
    let direct_target = config
        .bacnet_device_instance
        .map(|_| bip_mac(host_ip, port));
    let device_identifier = ObjectIdentifier::new(ObjectType::DEVICE, identity.device_instance)?;
    let object_count = match bacnet_read_property_value(
        &client,
        identity.device_instance,
        direct_target.as_deref(),
        device_identifier,
        PropertyIdentifier::OBJECT_LIST,
        Some(0),
    )
    .await?
    {
        PropertyValue::Unsigned(value) => value.min(1024) as u32,
        _ => 0,
    };

    let mut metrics = HashSet::new();
    let mut points = Vec::new();
    for index in 1..=object_count {
        let object_identifier = match bacnet_read_property_value(
            &client,
            identity.device_instance,
            direct_target.as_deref(),
            device_identifier,
            PropertyIdentifier::OBJECT_LIST,
            Some(index),
        )
        .await
        {
            Ok(PropertyValue::ObjectIdentifier(value)) => value,
            Ok(_) => continue,
            Err(_) => continue,
        };
        if !is_supported_bacnet_object_type(object_identifier.object_type()) {
            continue;
        }
        let object_name = match bacnet_read_property_value(
            &client,
            identity.device_instance,
            direct_target.as_deref(),
            object_identifier,
            PropertyIdentifier::OBJECT_NAME,
            None,
        )
        .await
        {
            Ok(PropertyValue::CharacterString(value)) => value,
            _ => format!(
                "{} {}",
                bacnet_object_type_label(object_identifier.object_type()),
                object_identifier.instance_number()
            ),
        };
        let present_value = match bacnet_read_property_value(
            &client,
            identity.device_instance,
            direct_target.as_deref(),
            object_identifier,
            PropertyIdentifier::PRESENT_VALUE,
            None,
        )
        .await
        {
            Ok(value) => value,
            Err(_) => continue,
        };
        let value = match property_value_to_f64(&present_value) {
            Some(value) => value,
            None => continue,
        };
        let units = bacnet_read_property_value(
            &client,
            identity.device_instance,
            direct_target.as_deref(),
            object_identifier,
            PropertyIdentifier::UNITS,
            None,
        )
        .await
        .ok();
        let unit_code = units.and_then(|value| match value {
            PropertyValue::Enumerated(code) => Some(code),
            _ => None,
        });
        let mut metric = slugify_metric(&object_name);
        if metric == "zone" || !metrics.insert(metric.clone()) {
            metric = format!(
                "{}_{}_{}",
                metric,
                bacnet_object_type_metric(object_identifier.object_type()),
                object_identifier.instance_number()
            );
            metrics.insert(metric.clone());
        }
        let unit = unit_code
            .map(engineering_units_label)
            .unwrap_or_else(|| bacnet_default_unit(object_identifier.object_type(), &present_value));
        let sensor_type = infer_bacnet_sensor_type(object_identifier.object_type(), unit_code, value);
        points.push(DevicePoint {
            name: object_name,
            metric,
            sensor_type,
            unit,
            protocol: "bacnet_ip".to_string(),
            register: None,
            data_type: None,
            scale: None,
            oid: None,
            path: None,
            json_pointer: None,
            bacnet_object: Some(format_bacnet_object_identifier(object_identifier)),
        });
    }

    client.stop().await.ok();

    Ok(DeviceDiscoveryResult {
        points: Some(points),
        bacnet_device_instance: Some(identity.device_instance),
        bacnet_vendor_id: Some(identity.vendor_id),
    })
}

async fn create_bacnet_client(
    config: &ExternalDeviceConfig,
    host: Ipv4Addr,
    port: u16,
) -> Result<BACnetClient<BipTransport>> {
    let interface = local_bacnet_interface_for_target(host, port)?;
    let build_client = |local_port| async move {
        let mut transport = BipTransport::new(interface.ip, local_port, interface.broadcast);
        if let Some(bbmd_host) = config
            .bacnet_bbmd_host
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let bbmd_ip = parse_ipv4_host(bbmd_host)
                .with_context(|| format!("invalid BACnet BBMD host {bbmd_host}"))?;
            transport.register_as_foreign_device(ForeignDeviceConfig {
                bbmd_ip,
                bbmd_port: config.bacnet_bbmd_port.unwrap_or(47808),
                ttl: config.bacnet_foreign_ttl_seconds.unwrap_or(300),
            });
        }
        BACnetClient::generic_builder()
            .transport(transport)
            .apdu_timeout_ms(2000)
            .build()
            .await
            .context("failed to start BACnet client")
    };
    let client = match build_client(47808).await {
        Ok(client) => client,
        Err(primary_err) => build_client(0)
            .await
            .with_context(|| format!("failed to start BACnet client ({primary_err})"))?,
    };
    if let Some(bbmd_host) = config
        .bacnet_bbmd_host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let bbmd_ip = parse_ipv4_host(bbmd_host)
            .with_context(|| format!("invalid BACnet BBMD host {bbmd_host}"))?;
        let bbmd_port = config.bacnet_bbmd_port.unwrap_or(47808);
        let result = client
            .register_foreign_device_bvlc(
                &bip_mac(bbmd_ip, bbmd_port),
                config.bacnet_foreign_ttl_seconds.unwrap_or(300),
            )
            .await
            .with_context(|| {
                format!(
                    "BACnet BBMD gateway {bbmd_host}:{bbmd_port} did not acknowledge foreign-device registration"
                )
            })?;
        anyhow::ensure!(
            result == BvlcResultCode::SUCCESSFUL_COMPLETION,
            "BACnet BBMD gateway {}:{} rejected foreign-device registration with {:?}",
            bbmd_host,
            bbmd_port,
            result
        );
    }
    Ok(client)
}

struct BacnetLocalInterface {
    ip: Ipv4Addr,
    broadcast: Ipv4Addr,
}

fn local_bacnet_interface_for_target(host: Ipv4Addr, port: u16) -> Result<BacnetLocalInterface> {
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            let if_addrs::IfAddr::V4(ref v4) = iface.addr else {
                continue;
            };
            if iface.is_loopback() || v4.prefixlen > 30 {
                continue;
            }
            let network = ipv4_and_mask(v4.ip, v4.netmask);
            let target_network = ipv4_and_mask(host, v4.netmask);
            if network == target_network {
                return Ok(BacnetLocalInterface {
                    ip: v4.ip,
                    broadcast: ipv4_broadcast(network, v4.netmask),
                });
            }
        }
    }

    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).context("failed to bind UDP socket")?;
    socket
        .connect((host, port))
        .with_context(|| format!("failed to resolve local interface for {host}:{port}"))?;
    match socket.local_addr().context("failed to inspect local UDP socket")?.ip() {
        IpAddr::V4(ip) => Ok(BacnetLocalInterface {
            ip,
            broadcast: Ipv4Addr::BROADCAST,
        }),
        IpAddr::V6(_) => Err(anyhow!("local interface resolved to IPv6 for BACnet target")),
    }
}

async fn discover_or_resolve_bacnet_identity(
    client: &BACnetClient<BipTransport>,
    host: Ipv4Addr,
    port: u16,
    config: &ExternalDeviceConfig,
) -> Result<BacnetIdentity> {
    if let Some(instance) = config.bacnet_device_instance {
        if let Ok(object_name) = bacnet_read_property_value(
            client,
            instance,
            Some(&bip_mac(host, port)),
            ObjectIdentifier::new(ObjectType::DEVICE, instance)?,
            PropertyIdentifier::OBJECT_NAME,
            None,
        )
        .await
        {
            return Ok(BacnetIdentity {
                device_instance: instance,
                vendor_id: config.bacnet_vendor_id.unwrap_or(0),
                object_name: match object_name {
                    PropertyValue::CharacterString(text) => Some(text),
                    _ => None,
                },
            });
        }
    }
    discover_bacnet_identity(client, host, port)
        .await?
        .context("no BACnet device responded to discovery")
}

async fn discover_bacnet_identity(
    client: &BACnetClient<BipTransport>,
    host: Ipv4Addr,
    port: u16,
) -> Result<Option<BacnetIdentity>> {
    client
        .who_is(None, None)
        .await
        .context("global Who-Is failed")?;
    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    let target = bip_mac(host, port);
    let mut devices = client.discovered_devices().await;
    let mut maybe_device = devices
        .iter()
        .find(|device| device.mac_address.as_slice() == target.as_slice())
        .cloned();
    if maybe_device.is_none() {
        client
            .who_is_directed(&target, None, None)
            .await
            .context("directed Who-Is failed")?;
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        devices = client.discovered_devices().await;
        maybe_device = devices
            .iter()
            .find(|device| device.mac_address.as_slice() == target.as_slice())
            .cloned();
    }
    let Some(device) = maybe_device else {
        return Ok(None);
    };
    let object_name = bacnet_read_property_value(
        client,
        device.object_identifier.instance_number(),
        None,
        device.object_identifier,
        PropertyIdentifier::OBJECT_NAME,
        None,
    )
    .await
    .ok()
    .and_then(|value| match value {
        PropertyValue::CharacterString(text) => Some(text),
        _ => None,
    });
    Ok(Some(BacnetIdentity {
        device_instance: device.object_identifier.instance_number(),
        vendor_id: device.vendor_id,
        object_name,
    }))
}

async fn bacnet_read_property_value(
    client: &BACnetClient<BipTransport>,
    device_instance: u32,
    destination_mac: Option<&[u8]>,
    object_identifier: ObjectIdentifier,
    property_identifier: PropertyIdentifier,
    property_array_index: Option<u32>,
) -> Result<PropertyValue> {
    let ack = if let Some(destination_mac) = destination_mac {
        client
            .read_property(
                destination_mac,
                object_identifier,
                property_identifier,
                property_array_index,
            )
            .await
    } else {
        client
            .read_property_from_device(
                device_instance,
                object_identifier,
                property_identifier,
                property_array_index,
            )
            .await
    }
    .with_context(|| {
        format!(
            "failed to read BACnet property {:?} from {}",
            property_identifier, object_identifier
        )
    })?;
    let (value, _) =
        decode_application_value(&ack.property_value, 0).context("failed to decode BACnet property value")?;
    Ok(value)
}

fn parse_bacnet_object_identifier(value: &str) -> Result<ObjectIdentifier> {
    let (kind, instance_text) = value
        .split_once(':')
        .ok_or_else(|| anyhow!("invalid BACnet object identifier {value}"))?;
    let object_type = match kind.trim().to_ascii_lowercase().as_str() {
        "analog_input" => ObjectType::ANALOG_INPUT,
        "analog_output" => ObjectType::ANALOG_OUTPUT,
        "analog_value" => ObjectType::ANALOG_VALUE,
        "binary_input" => ObjectType::BINARY_INPUT,
        "binary_output" => ObjectType::BINARY_OUTPUT,
        "binary_value" => ObjectType::BINARY_VALUE,
        "multi_state_input" => ObjectType::MULTI_STATE_INPUT,
        "multi_state_output" => ObjectType::MULTI_STATE_OUTPUT,
        "multi_state_value" => ObjectType::MULTI_STATE_VALUE,
        "device" => ObjectType::DEVICE,
        other => return Err(anyhow!("unsupported BACnet object type {other}")),
    };
    let instance = instance_text
        .trim()
        .parse::<u32>()
        .with_context(|| format!("invalid BACnet object instance {instance_text}"))?;
    ObjectIdentifier::new(object_type, instance).context("invalid BACnet object identifier")
}

fn format_bacnet_object_identifier(object_identifier: ObjectIdentifier) -> String {
    format!(
        "{}:{}",
        bacnet_object_type_metric(object_identifier.object_type()),
        object_identifier.instance_number()
    )
}

fn bacnet_object_type_metric(object_type: ObjectType) -> &'static str {
    match object_type {
        ObjectType::ANALOG_INPUT => "analog_input",
        ObjectType::ANALOG_OUTPUT => "analog_output",
        ObjectType::ANALOG_VALUE => "analog_value",
        ObjectType::BINARY_INPUT => "binary_input",
        ObjectType::BINARY_OUTPUT => "binary_output",
        ObjectType::BINARY_VALUE => "binary_value",
        ObjectType::MULTI_STATE_INPUT => "multi_state_input",
        ObjectType::MULTI_STATE_OUTPUT => "multi_state_output",
        ObjectType::MULTI_STATE_VALUE => "multi_state_value",
        ObjectType::DEVICE => "device",
        _ => "object",
    }
}

fn bacnet_object_type_label(object_type: ObjectType) -> &'static str {
    match object_type {
        ObjectType::ANALOG_INPUT => "Analog input",
        ObjectType::ANALOG_OUTPUT => "Analog output",
        ObjectType::ANALOG_VALUE => "Analog value",
        ObjectType::BINARY_INPUT => "Binary input",
        ObjectType::BINARY_OUTPUT => "Binary output",
        ObjectType::BINARY_VALUE => "Binary value",
        ObjectType::MULTI_STATE_INPUT => "Multi-state input",
        ObjectType::MULTI_STATE_OUTPUT => "Multi-state output",
        ObjectType::MULTI_STATE_VALUE => "Multi-state value",
        ObjectType::DEVICE => "Device",
        _ => "Object",
    }
}

fn is_supported_bacnet_object_type(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::ANALOG_INPUT
            | ObjectType::ANALOG_OUTPUT
            | ObjectType::ANALOG_VALUE
            | ObjectType::BINARY_INPUT
            | ObjectType::BINARY_OUTPUT
            | ObjectType::BINARY_VALUE
            | ObjectType::MULTI_STATE_INPUT
            | ObjectType::MULTI_STATE_OUTPUT
            | ObjectType::MULTI_STATE_VALUE
    )
}

fn engineering_units_label(raw: u32) -> String {
    match EngineeringUnits::from_raw(raw) {
        EngineeringUnits::VOLTS => "V".to_string(),
        EngineeringUnits::KILOVOLTS => "kV".to_string(),
        EngineeringUnits::AMPERES => "A".to_string(),
        EngineeringUnits::MILLIAMPERES => "mA".to_string(),
        EngineeringUnits::WATTS => "W".to_string(),
        EngineeringUnits::KILOWATTS => "kW".to_string(),
        EngineeringUnits::MEGAWATTS => "MW".to_string(),
        EngineeringUnits::WATT_HOURS => "Wh".to_string(),
        EngineeringUnits::KILOWATT_HOURS => "kWh".to_string(),
        EngineeringUnits::MEGAWATT_HOURS => "MWh".to_string(),
        EngineeringUnits::HERTZ => "Hz".to_string(),
        EngineeringUnits::POWER_FACTOR => "pf".to_string(),
        EngineeringUnits::PERCENT | EngineeringUnits::PERCENT_RELATIVE_HUMIDITY => "%".to_string(),
        EngineeringUnits::DEGREES_CELSIUS => "degC".to_string(),
        EngineeringUnits::DEGREES_FAHRENHEIT => "degF".to_string(),
        EngineeringUnits::DEGREES_PHASE => "deg".to_string(),
        EngineeringUnits::NO_UNITS => "".to_string(),
        other => format!("{other}").replace('_', " ").to_ascii_lowercase(),
    }
}

fn bacnet_default_unit(object_type: ObjectType, value: &PropertyValue) -> String {
    match object_type {
        ObjectType::BINARY_INPUT | ObjectType::BINARY_OUTPUT | ObjectType::BINARY_VALUE => {
            "".to_string()
        }
        ObjectType::MULTI_STATE_INPUT
        | ObjectType::MULTI_STATE_OUTPUT
        | ObjectType::MULTI_STATE_VALUE => "".to_string(),
        _ => match value {
            PropertyValue::Boolean(_) => "".to_string(),
            _ => "".to_string(),
        },
    }
}

fn infer_bacnet_sensor_type(
    object_type: ObjectType,
    unit_code: Option<u32>,
    value: f64,
) -> String {
    if matches!(
        object_type,
        ObjectType::BINARY_INPUT
            | ObjectType::BINARY_OUTPUT
            | ObjectType::BINARY_VALUE
            | ObjectType::MULTI_STATE_INPUT
            | ObjectType::MULTI_STATE_OUTPUT
            | ObjectType::MULTI_STATE_VALUE
    ) {
        return "status".to_string();
    }
    match unit_code.map(EngineeringUnits::from_raw) {
        Some(EngineeringUnits::VOLTS) | Some(EngineeringUnits::KILOVOLTS) => {
            "voltage".to_string()
        }
        Some(EngineeringUnits::AMPERES) | Some(EngineeringUnits::MILLIAMPERES) => {
            "current".to_string()
        }
        Some(EngineeringUnits::WATTS)
        | Some(EngineeringUnits::KILOWATTS)
        | Some(EngineeringUnits::MEGAWATTS) => "power".to_string(),
        Some(EngineeringUnits::WATT_HOURS)
        | Some(EngineeringUnits::KILOWATT_HOURS)
        | Some(EngineeringUnits::MEGAWATT_HOURS) => "energy".to_string(),
        Some(EngineeringUnits::HERTZ) => "frequency".to_string(),
        Some(EngineeringUnits::POWER_FACTOR) => "power_factor".to_string(),
        Some(EngineeringUnits::PERCENT) | Some(EngineeringUnits::PERCENT_RELATIVE_HUMIDITY) => {
            "percentage".to_string()
        }
        Some(EngineeringUnits::DEGREES_CELSIUS)
        | Some(EngineeringUnits::DEGREES_FAHRENHEIT) => "temperature".to_string(),
        Some(EngineeringUnits::DEGREES_PHASE) => "phase_angle".to_string(),
        _ => {
            if value.fract() == 0.0 {
                "generic".to_string()
            } else {
                "analog".to_string()
            }
        }
    }
}

fn property_value_to_f64(value: &PropertyValue) -> Option<f64> {
    match value {
        PropertyValue::Null => None,
        PropertyValue::Boolean(value) => Some(if *value { 1.0 } else { 0.0 }),
        PropertyValue::Unsigned(value) => Some(*value as f64),
        PropertyValue::Signed(value) => Some(*value as f64),
        PropertyValue::Real(value) => Some(*value as f64),
        PropertyValue::Double(value) => Some(*value),
        PropertyValue::Enumerated(value) => Some(*value as f64),
        PropertyValue::CharacterString(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn classify_bacnet_identity(name: Option<&str>, vendor_id: u16) -> Option<(&'static str, &'static str)> {
    let lower = name.unwrap_or("").to_ascii_lowercase();
    if lower.contains("setra") || lower.contains("power squad") || lower.contains("powersquad") {
        return Some(("setra", "setra_power_meter_generic"));
    }
    if lower.contains("megatron") {
        return Some(("megatron", "megatron_controller"));
    }
    if vendor_id == 0 {
        None
    } else {
        None
    }
}

fn parse_ipv4_host(host: &str) -> Result<Ipv4Addr> {
    host.parse::<Ipv4Addr>()
        .with_context(|| format!("host must be an IPv4 address for this protocol: {host}"))
}

fn bip_mac(host: Ipv4Addr, port: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(6);
    out.extend_from_slice(&host.octets());
    out.extend_from_slice(&port.to_be_bytes());
    out
}

async fn poll_snmp_device(
    state: &AppState,
    device: &ExternalDeviceRow,
    config: &ExternalDeviceConfig,
    points: &[DevicePoint],
    now: DateTime<Utc>,
) -> Result<()> {
    let host = config.host.as_ref().context("snmp device missing host")?;
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
        parts.push(
            part.parse::<u32>()
                .with_context(|| format!("invalid OID segment {part}"))?,
        );
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
) -> Result<DeviceDiscoveryResult> {
    match config.protocol.as_str() {
        "lutron_lip" => {
            let report = match config.lip_integration_report.as_deref() {
                Some(report) if !report.trim().is_empty() => report,
                _ => return Ok(DeviceDiscoveryResult::default()),
            };
            let outputs = parse_lutron_integration_report(report);
            if outputs.is_empty() {
                return Ok(DeviceDiscoveryResult::default());
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
                    path: Some(format!(
                        "OUTPUT,{},{}",
                        output.integration_id, output.output_id
                    )),
                    json_pointer: None,
                    bacnet_object: None,
                })
                .collect::<Vec<_>>();
            Ok(DeviceDiscoveryResult {
                points: Some(points),
                ..DeviceDiscoveryResult::default()
            })
        }
        "bacnet_ip" => discover_bacnet_points(config)
            .await
            .with_context(|| {
                let gateway_hint = if config
                    .bacnet_bbmd_host
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some()
                {
                    " The configured BBMD gateway did not return any matching BACnet responses; verify the device instance/network or the routed BACnet path."
                } else {
                    " No BBMD gateway is configured; add a Metasys/BBMD host if this device is not on the controller's local BACnet subnet."
                };
                format!(
                    "failed to discover BACnet points for device {} ({}){}. ",
                    device.name, model.id, gateway_hint
                )
            }),
        "lutron_leap" => {
            let points = discover_lutron_leap_points(config).await.with_context(|| {
                format!(
                    "failed to discover LEAP points for device {} ({})",
                    device.name, model.id
                )
            })?;
            if points.is_empty() {
                Ok(DeviceDiscoveryResult::default())
            } else {
                Ok(DeviceDiscoveryResult {
                    points: Some(points),
                    ..DeviceDiscoveryResult::default()
                })
            }
        }
        _ => Ok(DeviceDiscoveryResult::default()),
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
    let stream = TcpStream::connect(addr)
        .await
        .context("LIP connect failed")?;
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
        let response = timeout(
            std::time::Duration::from_secs(3),
            reader.read_line(&mut line),
        )
        .await
        .context("LIP read timed out")??;
        if response == 0 {
            continue;
        }
        if let Some(value) = parse_lip_output_value(&line) {
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
            let value = response.pointer(json_pointer).and_then(leap_value_to_f64);
            if let Some(value) = value {
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
        let read = timeout(
            std::time::Duration::from_secs(2),
            reader.read_line(&mut line),
        )
        .await;
        let Ok(Ok(count)) = read else { continue };
        if count == 0 {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("login") {
            if let Some(username) = username {
                writer
                    .write_all(format!("{}\r\n", username).as_bytes())
                    .await?;
                writer.flush().await?;
            }
        } else if lower.contains("password") {
            if let Some(password) = password {
                writer
                    .write_all(format!("{}\r\n", password).as_bytes())
                    .await?;
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
            let output_id = if numbers.len() > 1 {
                numbers[1]
            } else {
                numbers[0]
            };
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

async fn leap_connect(
    host: &str,
    port: u16,
    config: &ExternalDeviceConfig,
) -> Result<LeapConnection> {
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
    let stream = TcpStream::connect(addr)
        .await
        .context("LEAP connect failed")?;
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
        let read = timeout(
            std::time::Duration::from_secs(5),
            connection.reader.read_line(&mut line),
        )
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
            let name = zone
                .get("Name")
                .and_then(|value| value.as_str())
                .unwrap_or("");
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
        let href = area
            .get("href")
            .and_then(|value| value.as_str())
            .unwrap_or("");
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
                let name = zone
                    .get("Name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_point(metric: &str, protocol: &str, bacnet_object: Option<&str>) -> DevicePoint {
        DevicePoint {
            name: metric.to_string(),
            metric: metric.to_string(),
            sensor_type: "generic".to_string(),
            unit: String::new(),
            protocol: protocol.to_string(),
            register: None,
            data_type: None,
            scale: None,
            oid: None,
            path: None,
            json_pointer: None,
            bacnet_object: bacnet_object.map(str::to_string),
        }
    }

    #[test]
    fn points_for_device_filters_to_selected_protocol() {
        let config = ExternalDeviceConfig {
            vendor_id: "setra".to_string(),
            model_id: "setra_power_meter_generic".to_string(),
            protocol: "bacnet_ip".to_string(),
            host: Some("192.168.75.40".to_string()),
            port: Some(47808),
            unit_id: None,
            poll_interval_seconds: Some(30),
            snmp_community: None,
            http_base_url: None,
            http_username: None,
            http_password: None,
            lip_username: None,
            lip_password: None,
            lip_integration_report: None,
            leap_client_cert_pem: None,
            leap_client_key_pem: None,
            leap_ca_pem: None,
            leap_verify_ca: None,
            discovered_points: Some(vec![sample_point(
                "line_voltage_ab",
                "bacnet_ip",
                Some("analog_input:7"),
            )]),
            bacnet_device_instance: Some(1234),
            bacnet_vendor_id: Some(42),
            bacnet_bbmd_host: None,
            bacnet_bbmd_port: None,
            bacnet_foreign_ttl_seconds: None,
        };
        let model = DeviceModel {
            id: "setra_power_meter_generic".to_string(),
            name: "Setra".to_string(),
            since_year: Some(2015),
            protocols: vec!["modbus_tcp".to_string(), "bacnet_ip".to_string()],
            points: vec![
                sample_point("voltage_l1_n_v", "modbus_tcp", None),
                sample_point("legacy_bacnet_metric", "bacnet_ip", Some("analog_input:1")),
            ],
        };

        let points = points_for_device(&config, &model);

        assert_eq!(points.len(), 2);
        assert!(points.iter().any(|point| point.metric == "legacy_bacnet_metric"));
        assert!(points.iter().any(|point| point.metric == "line_voltage_ab"));
        assert!(!points.iter().any(|point| point.metric == "voltage_l1_n_v"));
    }

    #[test]
    fn parse_bacnet_object_identifier_round_trips() {
        let parsed = parse_bacnet_object_identifier("analog_input:42").unwrap();
        assert_eq!(parsed.object_type(), ObjectType::ANALOG_INPUT);
        assert_eq!(parsed.instance_number(), 42);
        assert_eq!(format_bacnet_object_identifier(parsed), "analog_input:42");
    }

    #[test]
    fn normalize_http_device_config_accepts_url_in_host_field() {
        let mut config = ExternalDeviceConfig {
            vendor_id: "metasys".to_string(),
            model_id: "metasys_server".to_string(),
            protocol: "http_json".to_string(),
            host: Some("https://192.168.75.18".to_string()),
            port: None,
            unit_id: None,
            poll_interval_seconds: None,
            snmp_community: None,
            http_base_url: None,
            http_username: None,
            http_password: None,
            lip_username: None,
            lip_password: None,
            lip_integration_report: None,
            leap_client_cert_pem: None,
            leap_client_key_pem: None,
            leap_ca_pem: None,
            leap_verify_ca: None,
            discovered_points: None,
            bacnet_device_instance: None,
            bacnet_vendor_id: None,
            bacnet_bbmd_host: None,
            bacnet_bbmd_port: None,
            bacnet_foreign_ttl_seconds: None,
        };

        normalize_http_device_config(&mut config);

        assert_eq!(config.host.as_deref(), Some("192.168.75.18"));
        assert_eq!(config.http_base_url.as_deref(), Some("https://192.168.75.18/metasys"));
    }

    #[test]
    fn hosts_from_range_limits_and_parses() {
        let (label, hosts) = hosts_from_range("192.168.75.40-192.168.75.42").unwrap();
        assert_eq!(label, "192.168.75.40-192.168.75.42");
        assert_eq!(
            hosts,
            vec![
                Ipv4Addr::new(192, 168, 75, 40),
                Ipv4Addr::new(192, 168, 75, 41),
                Ipv4Addr::new(192, 168, 75, 42),
            ]
        );
    }

    #[test]
    fn enumerate_ipv4_hosts_rejects_too_large_ranges() {
        let err = enumerate_ipv4_hosts(Ipv4Addr::new(192, 168, 75, 0), 21).unwrap_err();
        assert!(err.to_string().contains("scan range too large"));
    }

    #[test]
    fn ipv4_broadcast_uses_mask() {
        assert_eq!(
            ipv4_broadcast(Ipv4Addr::new(192, 168, 75, 0), Ipv4Addr::new(255, 255, 255, 0)),
            Ipv4Addr::new(192, 168, 75, 255)
        );
    }

    async fn discover_live_bacnet_points(host: &str) -> Result<DeviceDiscoveryResult> {
        let config = ExternalDeviceConfig {
            vendor_id: "setra".to_string(),
            model_id: "setra_power_meter_generic".to_string(),
            protocol: "bacnet_ip".to_string(),
            host: Some(host.to_string()),
            port: Some(47808),
            unit_id: None,
            poll_interval_seconds: Some(30),
            snmp_community: None,
            http_base_url: None,
            http_username: None,
            http_password: None,
            lip_username: None,
            lip_password: None,
            lip_integration_report: None,
            leap_client_cert_pem: None,
            leap_client_key_pem: None,
            leap_ca_pem: None,
            leap_verify_ca: None,
            discovered_points: None,
            bacnet_device_instance: None,
            bacnet_vendor_id: None,
            bacnet_bbmd_host: std::env::var("ID_LIVE_BACNET_BBMD_HOST").ok(),
            bacnet_bbmd_port: None,
            bacnet_foreign_ttl_seconds: None,
        };
        discover_bacnet_points(&config).await
    }

    #[tokio::test]
    #[ignore = "requires a reachable live Setra BACnet/IP device"]
    async fn live_setra_legacy_meter_discovers_points() {
        let host = std::env::var("ID_LIVE_BACNET_HOST_LEGACY")
            .unwrap_or_else(|_| "192.168.75.40".to_string());
        let discovery = discover_live_bacnet_points(&host).await.unwrap();
        let points = discovery.points.unwrap_or_default();
        assert!(discovery.bacnet_device_instance.is_some());
        assert!(points.len() >= 20, "expected at least 20 BACnet points, got {}", points.len());
        assert!(points.iter().all(|point| point.protocol == "bacnet_ip"));
    }

    #[tokio::test]
    #[ignore = "requires a reachable live Setra BACnet/IP device"]
    async fn live_setra_newer_meter_discovers_points() {
        let host = std::env::var("ID_LIVE_BACNET_HOST_NEWER")
            .unwrap_or_else(|_| "192.168.75.101".to_string());
        let discovery = discover_live_bacnet_points(&host).await.unwrap();
        let points = discovery.points.unwrap_or_default();
        assert!(discovery.bacnet_device_instance.is_some());
        assert!(points.len() >= 20, "expected at least 20 BACnet points, got {}", points.len());
        assert!(points.iter().all(|point| point.protocol == "bacnet_ip"));
    }

    #[tokio::test]
    #[ignore = "diagnostic: inspects live BBMD/FDT state"]
    async fn live_bacnet_gateway_diagnostic() {
        let gateway = std::env::var("ID_LIVE_BACNET_BBMD_HOST")
            .unwrap_or_else(|_| "192.168.75.18".to_string());
        let target = std::env::var("ID_LIVE_BACNET_HOST_NEWER")
            .unwrap_or_else(|_| "192.168.75.101".to_string());
        let host_ip = parse_ipv4_host(&target).unwrap();
        let gateway_ip = parse_ipv4_host(&gateway).unwrap();
        let config = ExternalDeviceConfig {
            vendor_id: "setra".to_string(),
            model_id: "setra_power_meter_generic".to_string(),
            protocol: "bacnet_ip".to_string(),
            host: Some(target),
            port: Some(47808),
            unit_id: None,
            poll_interval_seconds: Some(30),
            snmp_community: None,
            http_base_url: None,
            http_username: None,
            http_password: None,
            lip_username: None,
            lip_password: None,
            lip_integration_report: None,
            leap_client_cert_pem: None,
            leap_client_key_pem: None,
            leap_ca_pem: None,
            leap_verify_ca: None,
            discovered_points: None,
            bacnet_device_instance: None,
            bacnet_vendor_id: None,
            bacnet_bbmd_host: Some(gateway),
            bacnet_bbmd_port: Some(47808),
            bacnet_foreign_ttl_seconds: Some(300),
        };
        let mut client = create_bacnet_client(&config, host_ip, 47808).await.unwrap();
        let gateway_mac = bip_mac(gateway_ip, 47808);
        let register = client.register_foreign_device_bvlc(&gateway_mac, 60).await;
        eprintln!("foreign-device registration: {register:?}");
        let fdt = client.read_fdt(&gateway_mac).await;
        eprintln!("fdt: {fdt:?}");
        client.who_is(None, None).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        let devices = client.discovered_devices().await;
        eprintln!("discovered devices: {devices:#?}");
        client.stop().await.ok();
    }

    #[tokio::test]
    #[ignore = "diagnostic: direct BACnet unicast with known instance"]
    async fn live_bacnet_direct_known_instance_diagnostic() {
        let host = std::env::var("ID_LIVE_BACNET_HOST")
            .unwrap_or_else(|_| "192.168.75.103".to_string());
        let instance = std::env::var("ID_LIVE_BACNET_INSTANCE")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(394000);
        let config = ExternalDeviceConfig {
            vendor_id: "setra".to_string(),
            model_id: "setra_power_meter_generic".to_string(),
            protocol: "bacnet_ip".to_string(),
            host: Some(host.clone()),
            port: Some(47808),
            unit_id: None,
            poll_interval_seconds: Some(30),
            snmp_community: None,
            http_base_url: None,
            http_username: None,
            http_password: None,
            lip_username: None,
            lip_password: None,
            lip_integration_report: None,
            leap_client_cert_pem: None,
            leap_client_key_pem: None,
            leap_ca_pem: None,
            leap_verify_ca: None,
            discovered_points: None,
            bacnet_device_instance: Some(instance),
            bacnet_vendor_id: None,
            bacnet_bbmd_host: None,
            bacnet_bbmd_port: None,
            bacnet_foreign_ttl_seconds: None,
        };
        let host_ip = parse_ipv4_host(&host).unwrap();
        let mut client = create_bacnet_client(&config, host_ip, 47808).await.unwrap();
        let object = bacnet_read_property_value(
            &client,
            instance,
            Some(&bip_mac(host_ip, 47808)),
            ObjectIdentifier::new(ObjectType::DEVICE, instance).unwrap(),
            PropertyIdentifier::OBJECT_NAME,
            None,
        )
        .await;
        eprintln!("direct read result: {object:?}");
        client.stop().await.ok();
    }

    #[tokio::test]
    #[ignore = "requires a reachable live BACnet device with known instance"]
    async fn live_setra_direct_instance_discovers_points() {
        let config = ExternalDeviceConfig {
            vendor_id: "setra".to_string(),
            model_id: "setra_power_meter_generic".to_string(),
            protocol: "bacnet_ip".to_string(),
            host: Some(
                std::env::var("ID_LIVE_BACNET_HOST")
                    .unwrap_or_else(|_| "192.168.75.103".to_string()),
            ),
            port: Some(47808),
            unit_id: None,
            poll_interval_seconds: Some(30),
            snmp_community: None,
            http_base_url: None,
            http_username: None,
            http_password: None,
            lip_username: None,
            lip_password: None,
            lip_integration_report: None,
            leap_client_cert_pem: None,
            leap_client_key_pem: None,
            leap_ca_pem: None,
            leap_verify_ca: None,
            discovered_points: None,
            bacnet_device_instance: Some(
                std::env::var("ID_LIVE_BACNET_INSTANCE")
                    .ok()
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(394000),
            ),
            bacnet_vendor_id: None,
            bacnet_bbmd_host: None,
            bacnet_bbmd_port: None,
            bacnet_foreign_ttl_seconds: None,
        };
        let discovery = discover_bacnet_points(&config).await.unwrap();
        let points = discovery.points.unwrap_or_default();
        assert!(points.len() >= 10, "expected at least 10 points, got {}", points.len());
    }

    #[tokio::test]
    #[ignore = "requires a reachable live BACnet device with known instance"]
    async fn live_megatron_direct_instance_discovers_points() {
        let config = ExternalDeviceConfig {
            vendor_id: "megatron".to_string(),
            model_id: "megatron_controller".to_string(),
            protocol: "bacnet_ip".to_string(),
            host: Some(
                std::env::var("ID_LIVE_BACNET_HOST")
                    .unwrap_or_else(|_| "192.168.75.80".to_string()),
            ),
            port: Some(47808),
            unit_id: None,
            poll_interval_seconds: Some(30),
            snmp_community: None,
            http_base_url: None,
            http_username: None,
            http_password: None,
            lip_username: None,
            lip_password: None,
            lip_integration_report: None,
            leap_client_cert_pem: None,
            leap_client_key_pem: None,
            leap_ca_pem: None,
            leap_verify_ca: None,
            discovered_points: None,
            bacnet_device_instance: Some(
                std::env::var("ID_LIVE_BACNET_INSTANCE")
                    .ok()
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(40),
            ),
            bacnet_vendor_id: None,
            bacnet_bbmd_host: None,
            bacnet_bbmd_port: None,
            bacnet_foreign_ttl_seconds: None,
        };
        let discovery = discover_bacnet_points(&config).await.unwrap();
        let points = discovery.points.unwrap_or_default();
        assert!(!points.is_empty(), "expected at least 1 point");
    }
}
