use anyhow::Result;
use chrono::Utc;
use postgres::{Client, NoTls};
use reqwest::blocking::Client as HttpClient;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{postgres_connection_string, SetupConfig};

const STARTUP_GRACE_PERIOD: Duration = Duration::from_secs(30);
const STARTUP_POLL_INTERVAL: Duration = Duration::from_secs(2);
const REQUIRED_DATABASE_TABLES: &[&str] = &[
    "users",
    "nodes",
    "sensors",
    "metrics",
    "setup_credentials",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub core_api: HealthCheck,
    pub dashboard: HealthCheck,
    pub mqtt: HealthCheck,
    pub database: HealthCheck,
    pub redis: HealthCheck,
    pub qdrant: HealthCheck,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub status: String,
    pub message: String,
}

pub fn run_health_checks(config: &SetupConfig) -> Result<HealthReport> {
    let deadline = Instant::now() + STARTUP_GRACE_PERIOD;
    let mut report = build_health_report(config)?;
    while !report_ready(&report) && Instant::now() < deadline {
        thread::sleep(STARTUP_POLL_INTERVAL);
        report = build_health_report(config)?;
    }
    Ok(report)
}

fn build_health_report(config: &SetupConfig) -> Result<HealthReport> {
    let core_url = format!("http://127.0.0.1:{}/healthz", config.core_port);
    let dashboard_url = format!("http://127.0.0.1:{}/", config.core_port);
    let http_client = HttpClient::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let core_api = match http_client.get(core_url).send() {
        Ok(resp) if resp.status().is_success() => HealthCheck {
            status: "ok".to_string(),
            message: "Core API responded".to_string(),
        },
        Ok(resp) => HealthCheck {
            status: "error".to_string(),
            message: format!("Core API returned {}", resp.status()),
        },
        Err(err) => HealthCheck {
            status: "error".to_string(),
            message: format!("Core API unreachable: {err}"),
        },
    };

    let dashboard = match http_client.get(dashboard_url).send() {
        Ok(resp) if resp.status().is_success() || resp.status().is_redirection() => HealthCheck {
            status: "ok".to_string(),
            message: "Dashboard responded".to_string(),
        },
        Ok(resp) => HealthCheck {
            status: "error".to_string(),
            message: format!("Dashboard returned {}", resp.status()),
        },
        Err(err) => HealthCheck {
            status: "error".to_string(),
            message: format!("Dashboard unreachable: {err}"),
        },
    };

    let advertised_host = config.mqtt_host.trim();
    let mqtt_message = if advertised_host.is_empty() || advertised_host == "127.0.0.1" {
        format!("Local MQTT broker reachable on 127.0.0.1:{}", config.mqtt_port)
    } else {
        format!(
            "Local MQTT broker reachable on 127.0.0.1:{} (advertised to nodes as {})",
            config.mqtt_port, advertised_host
        )
    };
    let mqtt = match std::net::TcpStream::connect(("127.0.0.1", config.mqtt_port)) {
        Ok(_) => HealthCheck {
            status: "ok".to_string(),
            message: mqtt_message,
        },
        Err(err) => HealthCheck {
            status: "error".to_string(),
            message: format!("Local MQTT broker connection failed: {err}"),
        },
    };

    let database = match Client::connect(&postgres_connection_string(&config.database_url), NoTls) {
        Ok(mut client) => match missing_database_tables(&mut client) {
            Ok(missing) if missing.is_empty() => HealthCheck {
                status: "ok".to_string(),
                message: "Database reachable and schema is ready".to_string(),
            },
            Ok(missing) => HealthCheck {
                status: "error".to_string(),
                message: format!(
                    "Database reachable but schema is incomplete: missing {}",
                    missing.join(", ")
                ),
            },
            Err(err) => HealthCheck {
                status: "error".to_string(),
                message: format!("Database schema check failed: {err}"),
            },
        },
        Err(err) => HealthCheck {
            status: "error".to_string(),
            message: format!("Database connection failed: {err}"),
        },
    };

    let redis = match std::net::TcpStream::connect(("127.0.0.1", config.redis_port)) {
        Ok(_) => HealthCheck {
            status: "ok".to_string(),
            message: "Redis reachable".to_string(),
        },
        Err(err) => HealthCheck {
            status: "error".to_string(),
            message: format!("Redis connection failed: {err}"),
        },
    };

    let qdrant = match std::net::TcpStream::connect(("127.0.0.1", config.qdrant_port)) {
        Ok(_) => HealthCheck {
            status: "ok".to_string(),
            message: "Qdrant reachable".to_string(),
        },
        Err(err) => HealthCheck {
            status: "error".to_string(),
            message: format!("Qdrant connection failed: {err}"),
        },
    };

    Ok(HealthReport {
        core_api,
        dashboard,
        mqtt,
        database,
        redis,
        qdrant,
        generated_at: Utc::now().to_rfc3339(),
    })
}

fn missing_database_tables(client: &mut Client) -> Result<Vec<&'static str>> {
    let mut missing = Vec::new();
    for table in REQUIRED_DATABASE_TABLES {
        let table_name = format!("public.{table}");
        let exists: bool = client
            .query_one("SELECT to_regclass($1) IS NOT NULL", &[&table_name])?
            .get(0);
        if !exists {
            missing.push(*table);
        }
    }
    Ok(missing)
}

fn report_ready(report: &HealthReport) -> bool {
    [
        &report.core_api,
        &report.dashboard,
        &report.mqtt,
        &report.database,
        &report.redis,
        &report.qdrant,
    ]
    .iter()
    .all(|entry| entry.status == "ok")
}
