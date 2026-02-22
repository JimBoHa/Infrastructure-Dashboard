mod config;
mod http;
mod mqtt;
mod spool;

use crate::config::Config;
use anyhow::Result;
use tokio::sync::mpsc;

fn init_tracing() -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,node_forwarder=info".into());
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .try_init()
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    init_tracing()?;

    let (publish_tx, publish_rx) = mpsc::channel::<spool::PublishSample>(10_000);
    let (loss_tx, loss_rx) = mpsc::channel::<spool::LossEvent>(256);
    let spool = spool::spawn_spool_thread(config.clone(), publish_tx, loss_tx)?;

    let mqtt_config = config.clone();
    let mqtt_spool = spool.clone();
    let mqtt_handle = tokio::spawn(async move {
        if let Err(err) =
            mqtt::run_mqtt_forwarder(mqtt_config, mqtt_spool, publish_rx, loss_rx).await
        {
            tracing::error!(error=%err, "mqtt forwarder exited");
        }
    });

    let app = http::router(http::HttpState { spool: spool.clone() });
    let listener = tokio::net::TcpListener::bind(&config.http_bind).await?;
    tracing::info!(bind=%config.http_bind, "node-forwarder HTTP listening");
    let http_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown signal received");
        }
        _ = mqtt_handle => {}
        _ = http_handle => {}
    }

    Ok(())
}
