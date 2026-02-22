use anyhow::{Context, Result};
use clap::Parser;
use core_server_rs::{
    auth, cli, config, core_node, db, openapi, routes, services, state, static_assets,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};

async fn bind_listener(addr: &str) -> Result<TcpListener> {
    match TcpListener::bind(addr).await {
        Ok(listener) => Ok(listener),
        Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
            anyhow::bail!(
                "Failed to bind core-server-rs listener on {addr}: port already in use. Stop the other service using this port or re-run with --port to choose another port.",
            );
        }
        Err(err) => {
            Err(err).with_context(|| format!("failed to bind core-server-rs listener on {addr}"))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::Args::parse();
    if args.print_openapi {
        println!(
            "{}",
            serde_json::to_string_pretty(&openapi::openapi_json())?
        );
        return Ok(());
    }

    services::analysis::security::apply_umask();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = config::CoreConfig::from_env(args.static_root)?;
    let pool = db::connect_lazy(&config.database_url)?;

    let auth = Arc::new(auth::AuthManager::new(24));
    let http = reqwest::Client::new();
    let qdrant = Arc::new(services::analysis::qdrant::QdrantService::new(
        config.qdrant_url.clone(),
        http.clone(),
    ));
    let (mqtt, _mqtt_task) = services::mqtt::MqttPublisher::new(
        "farmdashboard-core",
        &config.mqtt_host,
        config.mqtt_port,
        config.mqtt_username.as_deref(),
        config.mqtt_password.as_deref(),
    )?;
    let mqtt = Arc::new(mqtt);

    let deployments = Arc::new(services::deployments::DeploymentManager::new(
        pool.clone(),
        config.node_agent_overlay_path.clone(),
        config.ssh_known_hosts_path.clone(),
        config.node_agent_port,
        format!("mqtt://{}:{}", config.mqtt_host, config.mqtt_port),
        config.mqtt_username.clone(),
        config.mqtt_password.clone(),
    ));

    let analysis_jobs = Arc::new(services::analysis::jobs::AnalysisJobService::new(
        pool.clone(),
        &config,
        qdrant.clone(),
    ));

    let state = state::AppState {
        config: config.clone(),
        db: pool.clone(),
        auth,
        mqtt: mqtt.clone(),
        deployments,
        analysis_jobs: analysis_jobs.clone(),
        qdrant: qdrant.clone(),
        http,
    };

    if let Err(err) = core_node::ensure_core_node(&state.db).await {
        tracing::warn!("failed to ensure core node exists: {err:#}");
    }

    if let Err(err) = services::map_offline::resume_installing_packs(state.clone()).await {
        tracing::warn!("failed to resume offline map installs: {err:#}");
    }

    let cancel = CancellationToken::new();
    if let Some(supervisor) = services::analysis::local_qdrant::LocalQdrantSupervisor::maybe_new(
        &config,
        state.http.clone(),
    ) {
        supervisor.start(cancel.clone());
    }
    analysis_jobs.clone().start(cancel.clone());
    qdrant.clone().start(cancel.clone());
    if config.analysis_embeddings_refresh_enabled {
        services::analysis::embeddings_refresh::EmbeddingsRefreshService::new(
            analysis_jobs.clone(),
            &config,
        )
        .start(cancel.clone());
    }
    services::analysis::replication::AnalysisReplicationService::new(pool.clone(), &config)
        .start(cancel.clone());
    services::schedule_engine::ScheduleEngine::new(pool, mqtt, config.clone())
        .start(cancel.clone());
    services::alarm_engine::AlarmEngineService::new(state.db.clone(), 10).start(cancel.clone());
    services::mqtt_status_ingest::MqttStatusIngestService::new(state.clone()).start(cancel.clone());
    services::restore_worker::RestoreWorkerService::new(state.clone()).start(cancel.clone());
    if config.enable_analytics_feeds {
        let feeds = services::analytics_feeds::AnalyticsFeedService::new(
            state.clone(),
            Duration::from_secs(config.analytics_feed_poll_interval_seconds),
        );
        feeds.start(cancel.clone());
    }
    if config.enable_forecast_ingestion {
        let forecasts = services::forecasts::ForecastService::new(
            state.clone(),
            Duration::from_secs(config.forecast_poll_interval_seconds),
        );
        forecasts.start(cancel.clone());
    }
    if config.enable_external_devices {
        let external_devices = services::external_devices::ExternalDeviceService::new(
            state.clone(),
            Duration::from_secs(config.external_device_poll_interval_seconds),
        );
        external_devices.start(cancel.clone());
    }

    services::battery_model::BatteryEstimatorService::new(state.clone(), Duration::from_secs(30))
        .start(cancel.clone());
    services::power_runway::PowerRunwayService::new(state.clone(), Duration::from_secs(10 * 60))
        .start(cancel.clone());

    // Applies queued Renogy BT-2 settings writes when nodes come online (best-effort; no resets).
    services::renogy_settings_apply::RenogySettingsApplyService::new(
        state.clone(),
        Duration::from_secs(20),
    )
    .start(cancel.clone());

    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(20)
            .burst_size(60)
            .methods(vec![
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
            ])
            .use_headers()
            .finish()
            .context("failed to build rate limiter config")?,
    );

    let governor_limiter = governor_conf.limiter().clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        governor_limiter.retain_recent();
    });

    let app = routes::router(state)
        .layer(GovernorLayer::new(governor_conf))
        .fallback_service(static_assets::service(config.static_root.clone())?);
    let addr = format!("{}:{}", args.host, args.port);
    let listener = bind_listener(&addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    cancel.cancel();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::bind_listener;
    use anyhow::Result;

    #[tokio::test]
    async fn reports_port_in_use_with_actionable_message() -> Result<()> {
        let listener = match std::net::TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                // Sandbox environments can block binding attempts.
                return Ok(());
            }
            Err(err) => return Err(err.into()),
        };
        let addr = listener.local_addr()?;

        let err = bind_listener(&addr.to_string()).await.unwrap_err();
        if err.to_string().to_lowercase().contains("operation not permitted") {
            // Sandbox environments can block binding attempts; skip assertions in that case.
            return Ok(());
        }
        let message = err.to_string().to_lowercase();

        assert!(message.contains(&addr.to_string()));
        assert!(message.contains("port already in use"));
        assert!(message.contains("--port"));

        drop(listener);
        Ok(())
    }
}
