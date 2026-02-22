use crate::auth::AuthManager;
use crate::config::CoreConfig;
use crate::db;
use crate::services;
use crate::services::analysis::jobs::AnalysisJobService;
use crate::services::analysis::qdrant::QdrantService;
use crate::services::deployments::DeploymentManager;
use crate::state::AppState;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub fn test_config() -> CoreConfig {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let data_root = tempdir.keep();

    CoreConfig {
        database_url: "postgresql://postgres@localhost/postgres".to_string(),
        mqtt_host: "127.0.0.1".to_string(),
        mqtt_port: 1883,
        mqtt_username: None,
        mqtt_password: None,
        static_root: None,
        setup_daemon_base_url: None,
        data_root: data_root.clone(),
        backup_storage_path: data_root.join("storage/backups"),
        backup_retention_days: 30,
        map_storage_path: data_root.join("storage/map"),
        node_agent_port: 9000,
        node_agent_overlay_path: PathBuf::from("/tmp/node-agent-overlay.tar.gz"),
        ssh_known_hosts_path: data_root.join("storage/ssh/known_hosts"),
        demo_mode: true,
        enable_analytics_feeds: false,
        enable_forecast_ingestion: false,
        analytics_feed_poll_interval_seconds: 300,
        forecast_poll_interval_seconds: 3600,
        schedule_poll_interval_seconds: 15,
        enable_external_devices: false,
        external_device_poll_interval_seconds: 30,
        forecast_api_base_url: None,
        forecast_api_path: None,
        rates_api_base_url: None,
        rates_api_path: None,
        analysis_max_concurrent_jobs: 1,
        analysis_poll_interval_ms: 250,
        analysis_lake_hot_path: data_root.join("storage/analysis/lake/hot"),
        analysis_lake_cold_path: None,
        analysis_tmp_path: data_root.join("storage/analysis/tmp"),
        analysis_lake_shards: 2,
        analysis_hot_retention_days: 90,
        analysis_late_window_hours: 48,
        analysis_replication_interval_seconds: 60,
        analysis_replication_lag_seconds: 300,
        analysis_max_jobs_per_user: 3,
        analysis_preview_max_window_seconds: 7 * 24 * 3600,
        analysis_embeddings_refresh_enabled: false,
        analysis_embeddings_refresh_interval_seconds: 21_600,
        analysis_embeddings_refresh_horizon_days: 30,
        analysis_embeddings_full_rebuild_interval_hours: 168,
        analysis_embeddings_full_rebuild_horizon_days: 365,
        analysis_profile_enabled: false,
        analysis_profile_output_path: data_root.join("storage/analysis/tmp/profiles"),
        qdrant_url: "http://127.0.0.1:6333".to_string(),
    }
}

pub fn test_state() -> AppState {
    let config = test_config();
    let pool = db::connect_lazy(&config.database_url).expect("connect_lazy");
    let auth = Arc::new(AuthManager::new(24));
    let http = reqwest::Client::new();
    let qdrant = Arc::new(QdrantService::new(config.qdrant_url.clone(), http.clone()));

    let (mqtt, _task) = services::mqtt::MqttPublisher::new(
        "core-server-rs-tests",
        &config.mqtt_host,
        config.mqtt_port,
        None,
        None,
    )
    .expect("mqtt publisher");
    let mqtt = Arc::new(mqtt);

    let deployments = Arc::new(DeploymentManager::new(
        pool.clone(),
        config.node_agent_overlay_path.clone(),
        config.ssh_known_hosts_path.clone(),
        config.node_agent_port,
        format!("mqtt://{}:{}", config.mqtt_host, config.mqtt_port),
        config.mqtt_username.clone(),
        config.mqtt_password.clone(),
    ));
    let analysis_jobs = Arc::new(AnalysisJobService::new(
        pool.clone(),
        &config,
        qdrant.clone(),
    ));

    AppState {
        config,
        db: pool,
        auth,
        mqtt,
        deployments,
        analysis_jobs,
        qdrant,
        http,
    }
}

pub fn test_user_with_caps(caps: &[&str]) -> crate::auth::AuthenticatedUser {
    let capabilities: HashSet<String> = caps.iter().map(|cap| cap.to_string()).collect();
    crate::auth::AuthenticatedUser {
        id: Uuid::new_v4().to_string(),
        email: "test-user@example.com".to_string(),
        role: "view".to_string(),
        capabilities,
        source: "test".to_string(),
    }
}
