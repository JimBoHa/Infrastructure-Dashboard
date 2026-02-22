mod ack;
mod config;
mod core_status;
mod grpc;
mod ingest;
mod mqtt;
mod pipeline;
mod predictive_feed;
mod telemetry;

use crate::config::Config;
use crate::grpc::{serve_uds, IngestService};
use crate::ingest::TelemetryIngestor;
use crate::pipeline::{build_pool, spawn_worker, BatchCommand, IngestStats, PipelineHandle};
use crate::predictive_feed::PredictiveFeed;
use anyhow::Result;
use futures::future;
use std::sync::Arc;
use tokio::sync::mpsc;

fn init_tracing(config: &Config) -> Result<()> {
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::{runtime::Tokio, trace::Config as OTelTraceConfig, Resource};
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,telemetry_sidecar=info".into());
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true);

    if let Some(endpoint) = &config.otlp_endpoint {
        let endpoint = normalize_otlp_http_endpoint(endpoint);
        let exporter = opentelemetry_otlp::new_exporter()
            .http()
            .with_endpoint(endpoint);
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(OTelTraceConfig::default().with_resource(Resource::new(vec![
                KeyValue::new("service.name", "telemetry-sidecar"),
            ])))
            .install_batch(Tokio)?;

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .try_init()?;
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .try_init()?;
    }

    Ok(())
}

fn normalize_otlp_http_endpoint(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.contains("/v1/traces") {
        return trimmed.to_string();
    }
    format!("{}/v1/traces", trimmed.trim_end_matches('/'))
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    init_tracing(&config)?;

    let pool = build_pool(&config.database_url, config.db_pool_size).await?;
    let stats = Arc::new(IngestStats::new());
    let (tx, rx) = mpsc::channel::<BatchCommand>(config.max_queue);
    let pipeline = PipelineHandle::new(tx, stats.clone());

    let (ack_tx, ack_rx) = ack::channel();
    let ack_tx = if config.enable_mqtt_listener {
        Some(ack_tx)
    } else {
        None
    };

    let ack_handle = if config.enable_mqtt_listener {
        let ack_config = config.clone();
        let ack_pool = pool.clone();
        Some(tokio::spawn(async move {
            if let Err(err) = ack::run_ack_manager(ack_config, ack_pool, ack_rx).await {
                tracing::error!(error=%err, "ack manager exited");
            }
        }))
    } else {
        None
    };
    let predictive_feed = PredictiveFeed::new(&config);
    let ingestor = TelemetryIngestor::new(
        pool.clone(),
        pipeline.clone(),
        config.offline_threshold(),
        predictive_feed,
        ack_tx.clone(),
    );

    let _worker_handle = spawn_worker(
        pool,
        rx,
        stats.clone(),
        config.batch_size,
        config.flush_interval(),
        ack_tx.clone(),
    );
    let grpc_service = IngestService::new(ingestor.clone());
    let grpc_path = config.grpc_socket_path.clone();

    let grpc_handle = tokio::spawn(async move { serve_uds(&grpc_path, grpc_service).await });

    let mqtt_handle = if config.enable_mqtt_listener {
        let config_clone = config.clone();
        let ingestor_clone = ingestor.clone();
        Some(tokio::spawn(async move {
            mqtt::run_listener(config_clone, ingestor_clone).await
        }))
    } else {
        None
    };
    let core_status_handle = {
        let config_clone = config.clone();
        let ingestor_clone = ingestor.clone();
        tokio::spawn(async move {
            if let Err(err) = core_status::run(config_clone, ingestor_clone).await {
                tracing::error!(error=%err, "core status task exited");
            }
        })
    };

    let status_handle = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(config.status_poll_interval());
        loop {
            ticker.tick().await;
            if let Err(err) = ingestor.check_offline().await {
                tracing::warn!(error=%err, "failed to run offline monitor");
            }
        }
    });

    tokio::select! {
        res = grpc_handle => {
            if let Err(err) = res { tracing::error!(error=%err, "gRPC task failed"); }
        }
        _ = async {
            if let Some(handle) = mqtt_handle {
                if let Err(err) = handle.await { tracing::warn!(error=%err, "MQTT task failed"); }
            } else {
                future::pending::<()>().await;
            }
        } => {}
        _ = async {
            if let Some(handle) = ack_handle {
                let _ = handle.await;
            } else {
                future::pending::<()>().await;
            }
        } => {}
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown signal received");
        }
    }

    status_handle.abort();
    core_status_handle.abort();
    drop(pipeline);

    Ok(())
}
