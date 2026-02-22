use anyhow::Result;
use chrono::Utc;
use postgres::{Client, NoTls};
use reqwest::blocking::Client as HttpClient;
use serde::{Deserialize, Serialize};

use crate::config::{postgres_connection_string, SetupConfig};

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
    let core_url = format!("http://127.0.0.1:{}/healthz", config.core_port);
    let dashboard_url = format!("http://127.0.0.1:{}/", config.core_port);
    let http_client = HttpClient::builder()
        .timeout(std::time::Duration::from_secs(5))
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

    let mqtt = match std::net::TcpStream::connect((config.mqtt_host.as_str(), config.mqtt_port)) {
        Ok(_) => HealthCheck {
            status: "ok".to_string(),
            message: "MQTT reachable".to_string(),
        },
        Err(err) => HealthCheck {
            status: "error".to_string(),
            message: format!("MQTT connection failed: {err}"),
        },
    };

    let database = match Client::connect(&postgres_connection_string(&config.database_url), NoTls) {
        Ok(_) => HealthCheck {
            status: "ok".to_string(),
            message: "Database reachable".to_string(),
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
