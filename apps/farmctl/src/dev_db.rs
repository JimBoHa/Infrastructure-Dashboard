use anyhow::{bail, Context, Result};
use chrono::{Duration, Timelike, Utc};
use postgres::{Client, NoTls};
use rand::Rng;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::{DbArgs, DbCommands, DbMigrateArgs, DbSeedDemoArgs};
use crate::config::postgres_connection_string;
use crate::migrations::apply_migrations_url;

#[derive(Debug, Clone, Deserialize)]
struct SetupConfigFile {
    database_url: Option<String>,
    backup_root: Option<String>,
}

pub fn handle(args: DbArgs) -> Result<()> {
    match args.command {
        DbCommands::Migrate(args) => migrate(args),
        DbCommands::SeedDemo(args) => seed_demo(args),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn resolve_existing_setup_config_path(config_arg: Option<PathBuf>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(path) = config_arg {
        candidates.push(path);
    }
    if let Ok(path) = env::var("FARM_SETUP_CONFIG") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(trimmed));
        }
    }
    if let Ok(state_dir) = env::var("FARM_SETUP_STATE_DIR") {
        let trimmed = state_dir.trim();
        if !trimmed.is_empty() {
            candidates.push(PathBuf::from(trimmed).join("config.json"));
        }
    }

    let last_state = repo_root()
        .join("reports")
        .join("e2e-setup-smoke")
        .join("last_state.json");
    if let Ok(raw) = fs::read_to_string(&last_state) {
        if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&raw) {
            let preserved = payload.get("preserved").and_then(|v| v.as_bool()).unwrap_or(false);
            if preserved {
                if let Some(path) = payload.get("config_path").and_then(|v| v.as_str()) {
                    if !path.trim().is_empty() {
                        candidates.push(PathBuf::from(path.trim()));
                    }
                }
            }
        }
    }

    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn load_setup_config(path: &Path) -> Result<SetupConfigFile> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("Failed to read setup config at {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse setup config at {}", path.display()))
}

fn resolve_database_url(
    database_url: Option<String>,
    config_arg: Option<PathBuf>,
    allow_config_fallback: bool,
) -> Result<String> {
    if let Some(url) = database_url.map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        return Ok(url);
    }
    if let Ok(value) = env::var("CORE_DATABASE_URL") {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }
    if let Ok(value) = env::var("DATABASE_URL") {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    if !allow_config_fallback {
        bail!("database_url not provided (set CORE_DATABASE_URL/DATABASE_URL or pass --database-url)");
    }

    let Some(config_path) = resolve_existing_setup_config_path(config_arg) else {
        bail!("database_url not provided and no setup config.json found (set CORE_DATABASE_URL or pass --database-url)");
    };
    let config = load_setup_config(&config_path)?;
    let url = config
        .database_url
        .unwrap_or_default()
        .trim()
        .to_string();
    if url.is_empty() {
        bail!(
            "database_url missing/empty in {} (set CORE_DATABASE_URL or pass --database-url)",
            config_path.display()
        );
    }
    Ok(url)
}

fn resolve_backup_root(
    backup_root: Option<PathBuf>,
    config_arg: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(path) = backup_root {
        return Ok(path);
    }
    if let Ok(value) = env::var("CORE_BACKUP_STORAGE_PATH") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Some(config_path) = resolve_existing_setup_config_path(config_arg) {
        if let Ok(config) = load_setup_config(&config_path) {
            if let Some(root) = config.backup_root.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                return Ok(PathBuf::from(root));
            }
        }
    }
    Ok(PathBuf::from("/Users/Shared/FarmDashboard/storage/backups"))
}

fn migrate(args: DbMigrateArgs) -> Result<()> {
    let database_url = resolve_database_url(args.database_url, args.config, true)?;
    apply_migrations_url(&database_url, &args.migrations_root)?;
    println!("Migrations applied.");
    Ok(())
}

#[derive(Debug, Clone)]
struct NodeSpec {
    id: &'static str,
    name: &'static str,
    status: &'static str,
    mac_eth: &'static str,
    mac_wifi: &'static str,
    ip_last: &'static str,
    uptime_seconds: i64,
    cpu_percent: f32,
    storage_used_bytes: i64,
    config: serde_json::Value,
}

#[derive(Debug, Clone)]
struct SensorSpec {
    node_name: &'static str,
    sensor_id: &'static str,
    name: &'static str,
    sensor_type: &'static str,
    unit: &'static str,
    interval_seconds: i32,
    rolling_avg_seconds: i32,
    config: serde_json::Value,
}

#[derive(Debug, Clone)]
struct OutputSpec {
    id: &'static str,
    node_name: &'static str,
    name: &'static str,
    output_type: &'static str,
    state: &'static str,
    supported_states: serde_json::Value,
    config: serde_json::Value,
}

fn seed_demo(args: DbSeedDemoArgs) -> Result<()> {
    let database_url = resolve_database_url(args.database_url, args.config.clone(), false)?;
    let backup_root = resolve_backup_root(args.backup_root, args.config)?;

    let mut client = Client::connect(&postgres_connection_string(&database_url), NoTls)
        .context("Failed to connect for seed-demo")?;

    let node_specs = vec![
        NodeSpec {
            id: "11111111-1111-1111-1111-111111111111",
            name: "North Field Controller",
            status: "online",
            mac_eth: "40:16:7E:AA:01:01",
            mac_wifi: "40:16:7E:AA:01:02",
            ip_last: "127.0.0.1",
            uptime_seconds: 72_600,
            cpu_percent: 32.4,
            storage_used_bytes: 78_956_371_200,
            config: serde_json::json!({
                "hardware": "Pi 5",
                "firmware": "1.4.2",
                "mesh_role": "coordinator",
                "tags": ["controller", "north"],
                "buffer": {"pending_messages": 2},
                "retention_days": 21,
            }),
        },
        NodeSpec {
            id: "22222222-2222-2222-2222-222222222222",
            name: "Irrigation Pump House",
            status: "online",
            mac_eth: "40:16:7E:AA:02:01",
            mac_wifi: "40:16:7E:AA:02:02",
            ip_last: "127.0.0.1",
            uptime_seconds: 54_100,
            cpu_percent: 28.9,
            storage_used_bytes: 41_270_149_120,
            config: serde_json::json!({
                "hardware": "Pi 5",
                "firmware": "1.4.2",
                "mesh_role": "router",
                "tags": ["pump", "irrigation"],
                "buffer": {"pending_messages": 1},
            }),
        },
        NodeSpec {
            id: "33333333-3333-3333-3333-333333333333",
            name: "Greenhouse South",
            status: "maintenance",
            mac_eth: "40:16:7E:AA:03:01",
            mac_wifi: "40:16:7E:AA:03:02",
            ip_last: "127.0.0.1",
            uptime_seconds: 12_400,
            cpu_percent: 12.1,
            storage_used_bytes: 19_430_400_000,
            config: serde_json::json!({
                "hardware": "Pi Zero 2 W",
                "firmware": "1.3.9",
                "mesh_role": "router",
                "tags": ["greenhouse"],
                "retention_days": 7,
            }),
        },
    ];

    let sensor_specs = vec![
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "soil-moisture-north",
            name: "Soil Moisture - North",
            sensor_type: "moisture",
            unit: "%",
            interval_seconds: 1800,
            rolling_avg_seconds: 600,
            config: serde_json::json!({
                "default_interval_seconds": 1800,
                "category": "moisture",
                "rolling_enabled": true,
            }),
        },
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "soil-temp-north",
            name: "Soil Temperature",
            sensor_type: "temperature",
            unit: "°C",
            interval_seconds: 1800,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 1800,
                "category": "temperature",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "Irrigation Pump House",
            sensor_id: "pump-load",
            name: "Pump Load",
            sensor_type: "power",
            unit: "kW",
            interval_seconds: 1,
            rolling_avg_seconds: 60,
            config: serde_json::json!({
                "default_interval_seconds": 1,
                "category": "power",
                "rolling_enabled": true,
            }),
        },
        SensorSpec {
            node_name: "Irrigation Pump House",
            sensor_id: "water-pressure",
            name: "Water Pressure",
            sensor_type: "pressure",
            unit: "psi",
            interval_seconds: 30,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 30,
                "category": "pressure",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "Irrigation Pump House",
            sensor_id: "flow-meter-domestic",
            name: "Domestic Flow",
            sensor_type: "flow",
            unit: "gpm",
            interval_seconds: 0,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 0,
                "category": "flow",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "Irrigation Pump House",
            sensor_id: "reservoir-level",
            name: "Reservoir Level",
            sensor_type: "water_level",
            unit: "in",
            interval_seconds: 1800,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 1800,
                "category": "water_level",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "Greenhouse South",
            sensor_id: "greenhouse-humidity",
            name: "Greenhouse Humidity",
            sensor_type: "humidity",
            unit: "%",
            interval_seconds: 600,
            rolling_avg_seconds: 300,
            config: serde_json::json!({
                "default_interval_seconds": 600,
                "category": "humidity",
                "rolling_enabled": true,
            }),
        },
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "wind-speed-north",
            name: "Wind Speed",
            sensor_type: "wind",
            unit: "mph",
            interval_seconds: 30,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 30,
                "category": "wind",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "Greenhouse South",
            sensor_id: "lux-greenhouse",
            name: "Greenhouse Lux",
            sensor_type: "lux",
            unit: "lux",
            interval_seconds: 300,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 300,
                "category": "light",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "rain-gauge",
            name: "Rain Gauge",
            sensor_type: "rain",
            unit: "mm",
            interval_seconds: 0,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 0,
                "category": "rain",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "Irrigation Pump House",
            sensor_id: "fert-level",
            name: "Fertilizer Level",
            sensor_type: "chemical_level",
            unit: "%",
            interval_seconds: 1800,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 1800,
                "category": "level",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "Irrigation Pump House",
            sensor_id: "solar-irradiance",
            name: "Solar Irradiance",
            sensor_type: "solar",
            unit: "W/m²",
            interval_seconds: 300,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "default_interval_seconds": 300,
                "category": "solar",
                "rolling_enabled": false,
            }),
        },
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "renogy-pv-power",
            name: "Renogy PV Power",
            sensor_type: "renogy_bt2",
            unit: "W",
            interval_seconds: 5,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "metric": "pv_power_w",
                "default_interval_seconds": 5,
                "category": "power",
            }),
        },
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "renogy-battery-soc",
            name: "Renogy Battery SOC",
            sensor_type: "renogy_bt2",
            unit: "%",
            interval_seconds: 5,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "metric": "battery_soc_percent",
                "default_interval_seconds": 5,
                "category": "battery",
            }),
        },
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "renogy-load-power",
            name: "Renogy Load Power",
            sensor_type: "renogy_bt2",
            unit: "W",
            interval_seconds: 5,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "metric": "load_power_w",
                "default_interval_seconds": 5,
                "category": "power",
            }),
        },
        SensorSpec {
            node_name: "North Field Controller",
            sensor_id: "renogy-runtime",
            name: "Renogy Runtime",
            sensor_type: "renogy_bt2",
            unit: "hrs",
            interval_seconds: 5,
            rolling_avg_seconds: 0,
            config: serde_json::json!({
                "metric": "runtime_hours",
                "default_interval_seconds": 5,
                "category": "battery",
            }),
        },
    ];

    let output_specs = vec![
        OutputSpec {
            id: "out-pump-1",
            node_name: "Irrigation Pump House",
            name: "Pump 1",
            output_type: "relay",
            state: "off",
            supported_states: serde_json::json!(["off", "on", "auto"]),
            config: serde_json::json!({"command_topic": "iot/pump-house/pump1/command"}),
        },
        OutputSpec {
            id: "out-greenhouse-fan",
            node_name: "Greenhouse South",
            name: "Greenhouse Fan",
            output_type: "relay",
            state: "auto",
            supported_states: serde_json::json!(["off", "on", "auto"]),
            config: serde_json::json!({"command_topic": "iot/greenhouse/fan/command"}),
        },
    ];

    let retention_overrides: HashMap<&str, i32> = HashMap::from([
        ("North Field Controller", 21),
        ("Greenhouse South", 7),
    ]);

    let mut tx = client.transaction().context("Failed to begin transaction")?;
    reset_demo_tables(&mut tx)?;

    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let mut node_ids: HashMap<&str, String> = HashMap::new();
    for spec in &node_specs {
        tx.execute(
            r#"
            INSERT INTO nodes (id, name, status, mac_eth, mac_wifi, ip_last, uptime_seconds, cpu_percent, storage_used_bytes, config, last_seen)
            VALUES ($1::uuid, $2, $3, $4::macaddr, $5::macaddr, $6::inet, $7, $8, $9, $10::jsonb, $11::timestamptz)
            "#,
            &[
                &spec.id,
                &spec.name,
                &spec.status,
                &spec.mac_eth,
                &spec.mac_wifi,
                &spec.ip_last,
                &spec.uptime_seconds,
                &spec.cpu_percent,
                &spec.storage_used_bytes,
                &spec.config.to_string(),
                &now_rfc3339,
            ],
        )
        .with_context(|| format!("Failed to insert node {}", spec.name))?;
        node_ids.insert(spec.name, spec.id.to_string());
    }

    for (name, keep_days) in retention_overrides {
        if let Some(node_id) = node_ids.get(name) {
            tx.execute(
                r#"INSERT INTO backup_retention (node_id, keep_days) VALUES ($1::uuid, $2)"#,
                &[node_id, &keep_days],
            )
            .with_context(|| format!("Failed to insert backup_retention for {}", name))?;
        }
    }

    for spec in &sensor_specs {
        let node_id = node_ids
            .get(spec.node_name)
            .with_context(|| format!("Missing node mapping for {}", spec.node_name))?;
        tx.execute(
            r#"
            INSERT INTO sensors (sensor_id, node_id, name, type, unit, interval_seconds, rolling_avg_seconds, config)
            VALUES ($1, $2::uuid, $3, $4, $5, $6, $7, $8::jsonb)
            "#,
            &[
                &spec.sensor_id,
                node_id,
                &spec.name,
                &spec.sensor_type,
                &spec.unit,
                &spec.interval_seconds,
                &spec.rolling_avg_seconds,
                &spec.config.to_string(),
            ],
        )
        .with_context(|| format!("Failed to insert sensor {}", spec.sensor_id))?;
    }

    let last_command = now - Duration::minutes(15);
    let last_command_rfc3339 = last_command.to_rfc3339();
    for spec in &output_specs {
        let node_id = node_ids
            .get(spec.node_name)
            .with_context(|| format!("Missing node mapping for {}", spec.node_name))?;
        tx.execute(
            r#"
            INSERT INTO outputs (id, node_id, name, type, state, supported_states, last_command, config)
            VALUES ($1, $2::uuid, $3, $4, $5, $6::jsonb, $7::timestamptz, $8::jsonb)
            "#,
            &[
                &spec.id,
                node_id,
                &spec.name,
                &spec.output_type,
                &spec.state,
                &spec.supported_states.to_string(),
                &last_command_rfc3339,
                &spec.config.to_string(),
            ],
        )
        .with_context(|| format!("Failed to insert output {}", spec.id))?;
    }

    insert_metrics(&mut tx, &sensor_specs)?;
    insert_analytics(&mut tx)?;

    tx.commit().context("Failed to commit seed transaction")?;

    if !args.skip_backup_fixtures {
        write_backup_fixtures(&backup_root, &node_specs)?;
    }

    println!("Demo data seeded.");
    Ok(())
}

fn reset_demo_tables(tx: &mut postgres::Transaction<'_>) -> Result<()> {
    tx.batch_execute(
        r#"
        DELETE FROM analytics_rate_schedules;
        DELETE FROM analytics_integration_status;
        DELETE FROM analytics_status_samples;
        DELETE FROM analytics_soil_field_stats;
        DELETE FROM analytics_soil_samples;
        DELETE FROM analytics_water_samples;
        DELETE FROM analytics_power_samples;
        DELETE FROM metrics;
        DELETE FROM outputs;
        DELETE FROM sensors;
        DELETE FROM backup_retention;
        DELETE FROM nodes;
        "#,
    )
    .context("Failed to clear demo tables")?;
    Ok(())
}

fn insert_metrics(tx: &mut postgres::Transaction<'_>, sensors: &[SensorSpec]) -> Result<()> {
    let now = Utc::now();
    let start = now - Duration::hours(24);
    let mut rng = rand::thread_rng();
    for spec in sensors {
        let base: f64 = rng.gen_range(10.0..30.0);
        let mut current = start;
        while current <= now {
            let noise: f64 = rng.gen_range(-2.0..2.0);
            let value = (base + noise * 1.0).round_to(2);
            let current_rfc3339 = current.to_rfc3339();
            tx.execute(
                r#"
                INSERT INTO metrics (sensor_id, ts, value, quality)
                VALUES ($1, $2::timestamptz, $3, 0)
                "#,
                &[&spec.sensor_id, &current_rfc3339, &value],
            )
            .with_context(|| format!("Failed to insert metric point for {}", spec.sensor_id))?;
            current = current + Duration::minutes(30);
        }
    }
    Ok(())
}

fn insert_analytics(tx: &mut postgres::Transaction<'_>) -> Result<()> {
    let now = Utc::now()
        .with_minute(0)
        .and_then(|dt| dt.with_second(0))
        .and_then(|dt| dt.with_nanosecond(0))
        .context("Failed to truncate analytics seed timestamp")?;

    for offset in (0..=168).rev() {
        let ts = now - Duration::hours(offset);
        let ts_rfc3339 = ts.to_rfc3339();
        let offset_f = offset as f64;
        let wave = (offset_f / 6.0).sin();
        let solar_wave = (offset_f / 4.5).sin().max(0.0);
        let total_kw = 28.0 + 6.0 * wave;
        let solar_kw = (12.0 + 4.0 * solar_wave).max(0.0);
        let grid_kw = (total_kw - solar_kw).max(0.0);
        let consumption_kwh = (total_kw * 0.9).max(0.0);

        insert_analytics_sample(
            tx,
            "analytics_power_samples",
            &ts_rfc3339,
            "total_kw",
            total_kw.round_to(3),
        )?;
        insert_analytics_sample(
            tx,
            "analytics_power_samples",
            &ts_rfc3339,
            "solar_kw",
            solar_kw.round_to(3),
        )?;
        insert_analytics_sample(
            tx,
            "analytics_power_samples",
            &ts_rfc3339,
            "grid_kw",
            grid_kw.round_to(3),
        )?;
        insert_analytics_sample(
            tx,
            "analytics_power_samples",
            &ts_rfc3339,
            "consumption_kwh",
            consumption_kwh.round_to(3),
        )?;

        let domestic_gal = (30.0 + 10.0 * (offset_f / 3.5).sin()).max(0.0);
        let ag_gal = (85.0 + 18.0 * ((offset + 6) as f64 / 2.8).sin()).max(0.0);
        let reservoir_depth = 62.0 - 0.05 * offset_f + 0.8 * (offset_f / 8.0).sin();

        insert_analytics_sample(
            tx,
            "analytics_water_samples",
            &ts_rfc3339,
            "domestic_gal",
            domestic_gal.round_to(3),
        )?;
        insert_analytics_sample(tx, "analytics_water_samples", &ts_rfc3339, "ag_gal", ag_gal.round_to(3))?;
        insert_analytics_sample(
            tx,
            "analytics_water_samples",
            &ts_rfc3339,
            "reservoir_depth_ft",
            reservoir_depth.round_to(3),
        )?;

        let avg_moisture = 34.0 + 2.5 * (offset_f / 7.0).sin();
        insert_analytics_sample(
            tx,
            "analytics_soil_samples",
            &ts_rfc3339,
            "avg_moisture",
            avg_moisture.round_to(3),
        )?;

        let alarms = 3.0 + ((offset % 4) as f64);
        let battery_soc = 58.0 + 6.0 * (offset_f / 5.5).sin();
        insert_analytics_sample(
            tx,
            "analytics_status_samples",
            &ts_rfc3339,
            "alarms_last_168h",
            alarms.round_to(3),
        )?;
        insert_analytics_sample(tx, "analytics_status_samples", &ts_rfc3339, "nodes_online", 2.0)?;
        insert_analytics_sample(tx, "analytics_status_samples", &ts_rfc3339, "nodes_offline", 1.0)?;
        insert_analytics_sample(
            tx,
            "analytics_status_samples",
            &ts_rfc3339,
            "battery_soc",
            battery_soc.round_to(3),
        )?;
        insert_analytics_sample(
            tx,
            "analytics_status_samples",
            &ts_rfc3339,
            "solar_kw",
            solar_kw.round_to(3),
        )?;
    }

    for offset in (0..=168).rev().step_by(24) {
        let ts = now - Duration::hours(offset);
        let ts_rfc3339 = ts.to_rfc3339();
        let offset_f = offset as f64;
        let north_base = 32.0 + 1.8 * (offset_f / 5.0).sin();
        let south_base = 29.0 + 1.5 * (offset_f / 4.5).cos();
        insert_soil_field_stat(
            tx,
            &ts_rfc3339,
            "North Field",
            (north_base - 2.0).round_to(3),
            (north_base + 2.5).round_to(3),
            north_base.round_to(3),
        )?;
        insert_soil_field_stat(
            tx,
            &ts_rfc3339,
            "South Pasture",
            (south_base - 1.7).round_to(3),
            (south_base + 2.2).round_to(3),
            south_base.round_to(3),
        )?;
    }

    insert_integration_status(
        tx,
        "power",
        "Emporia Vue",
        "connected",
        serde_json::json!({"last_sync_minutes": 5}),
    )?;
    insert_integration_status(
        tx,
        "power",
        "Tesla Gateway",
        "connected",
        serde_json::json!({"last_sync_minutes": 8}),
    )?;
    insert_integration_status(
        tx,
        "power",
        "Enphase",
        "pending",
        serde_json::json!({"last_sync_minutes": serde_json::Value::Null}),
    )?;

    tx.execute(
        r#"
        INSERT INTO analytics_rate_schedules (category, provider, current_rate, est_monthly_cost, details)
        VALUES ($1, $2, $3, $4, $5::jsonb)
        "#,
        &[
            &"power",
            &"PG&E TOU-D",
            &0.27_f64,
            &812.4_f64,
            &serde_json::json!({"tier": "peak", "currency": "USD"}).to_string(),
        ],
    )
    .context("Failed to insert analytics_rate_schedule")?;

    Ok(())
}

fn insert_analytics_sample(
    tx: &mut postgres::Transaction<'_>,
    table: &str,
    recorded_at_rfc3339: &str,
    metric: &str,
    value: f64,
) -> Result<()> {
    let sql = format!(
        "INSERT INTO {table} (recorded_at, metric, value, metadata) VALUES ($1::timestamptz, $2, $3, '{{}}'::jsonb) ON CONFLICT (recorded_at, metric) DO UPDATE SET value = EXCLUDED.value",
    );
    tx.execute(&sql, &[&recorded_at_rfc3339, &metric, &value]).with_context(|| {
        format!(
            "Failed to insert {} {} at {}",
            table, metric, recorded_at_rfc3339
        )
    })?;
    Ok(())
}

fn insert_soil_field_stat(
    tx: &mut postgres::Transaction<'_>,
    recorded_at_rfc3339: &str,
    field_name: &str,
    min_pct: f64,
    max_pct: f64,
    avg_pct: f64,
) -> Result<()> {
    tx.execute(
        r#"
        INSERT INTO analytics_soil_field_stats (recorded_at, field_name, min_pct, max_pct, avg_pct, metadata)
        VALUES ($1::timestamptz, $2, $3, $4, $5, '{}'::jsonb)
        ON CONFLICT (recorded_at, field_name) DO UPDATE
          SET min_pct = EXCLUDED.min_pct,
              max_pct = EXCLUDED.max_pct,
              avg_pct = EXCLUDED.avg_pct
        "#,
        &[
            &recorded_at_rfc3339,
            &field_name,
            &min_pct,
            &max_pct,
            &avg_pct,
        ],
    )
    .with_context(|| format!("Failed to insert soil field stat {}", field_name))?;
    Ok(())
}

fn insert_integration_status(
    tx: &mut postgres::Transaction<'_>,
    category: &str,
    name: &str,
    status: &str,
    metadata: serde_json::Value,
) -> Result<()> {
    tx.execute(
        r#"
        INSERT INTO analytics_integration_status (category, name, status, metadata)
        VALUES ($1, $2, $3, $4::jsonb)
        "#,
        &[&category, &name, &status, &metadata.to_string()],
    )
    .with_context(|| format!("Failed to insert analytics_integration_status {name}"))?;
    Ok(())
}

fn write_backup_fixtures(backup_root: &Path, nodes: &[NodeSpec]) -> Result<()> {
    fs::create_dir_all(backup_root)
        .with_context(|| format!("Failed to create backup root {}", backup_root.display()))?;
    let backup_date = Utc::now().format("%Y-%m-%d").to_string();
    let fetched_at = Utc::now().to_rfc3339();

    for node in nodes {
        let node_dir = backup_root.join(node.id);
        fs::create_dir_all(&node_dir)
            .with_context(|| format!("Failed to create {}", node_dir.display()))?;
        let backup_path = node_dir.join(format!("{backup_date}.json"));
        if backup_path.exists() {
            continue;
        }
        let payload = serde_json::json!({
            "fetched_at": fetched_at,
            "config": {
                "node": {
                    "node_id": node.id,
                    "node_name": node.name,
                }
            },
            "node": {
                "id": node.id,
                "name": node.name,
                "ip": node.ip_last,
            },
        });
        fs::write(&backup_path, serde_json::to_string_pretty(&payload)?)
            .with_context(|| format!("Failed to write {}", backup_path.display()))?;
    }
    Ok(())
}

trait RoundTo {
    fn round_to(self, decimals: u32) -> f64;
}

impl RoundTo for f64 {
    fn round_to(self, decimals: u32) -> f64 {
        let scale = 10_f64.powi(decimals as i32);
        (self * scale).round() / scale
    }
}
