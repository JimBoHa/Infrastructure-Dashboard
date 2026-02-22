use crate::ingest::TelemetryIngestor;
use crate::pipeline::IngestStats;
use crate::telemetry::MetricRow;
use anyhow::Result;
use chrono::TimeZone;
use std::path::Path;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{transport::Server, Request, Response, Status};
use tonic_health::server::health_reporter;

pub mod proto {
    tonic::include_proto!("telemetry.ingest");
}

use proto::ingestor_server::{Ingestor, IngestorServer};
use proto::{
    HealthRequest, HealthResponse, Metric as RpcMetric, WriteBatchRequest, WriteBatchResponse,
};

#[derive(Clone)]
pub struct IngestService {
    ingestor: TelemetryIngestor,
}

impl IngestService {
    pub fn new(ingestor: TelemetryIngestor) -> Self {
        Self { ingestor }
    }

    fn map_metric(metric: RpcMetric) -> Result<MetricRow, Status> {
        let ts = chrono::Utc
            .timestamp_millis_opt(metric.timestamp_ms)
            .single()
            .ok_or_else(|| Status::invalid_argument("invalid timestamp"))?;

        Ok(MetricRow {
            sensor_id: metric.sensor_id,
            timestamp: ts,
            value: metric.value,
            quality: metric.quality,
            source: Some(metric.source).filter(|s| !s.is_empty()),
            seq: None,
            stream_id: None,
            backfill: false,
        })
    }

    fn to_health(&self, stats: &Arc<IngestStats>) -> HealthResponse {
        let queue_depth = stats.queue_depth.load(std::sync::atomic::Ordering::Relaxed);
        let last_flush_unix_ms = stats
            .last_flush_unix_ms
            .load(std::sync::atomic::Ordering::Relaxed);
        let last_batch_len = stats
            .last_batch_len
            .load(std::sync::atomic::Ordering::Relaxed);
        let average_flush_ms = stats
            .average_flush_micros
            .load(std::sync::atomic::Ordering::Relaxed) as f64
            / 1000.0;
        let inflight_flushes = stats
            .inflight_flushes
            .load(std::sync::atomic::Ordering::Relaxed);
        let mqtt_connected = stats
            .mqtt_connected
            .load(std::sync::atomic::Ordering::Relaxed);
        let last_error = stats
            .last_error
            .lock()
            .ok()
            .and_then(|e| e.clone())
            .unwrap_or_default();

        HealthResponse {
            queue_depth,
            last_flush_unix_ms,
            last_batch_len,
            average_flush_ms,
            mqtt_connected,
            inflight_flushes,
            last_error,
            build: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[tonic::async_trait]
impl Ingestor for IngestService {
    async fn get_health(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let stats = self.ingestor.stats();
        Ok(Response::new(self.to_health(&stats)))
    }

    async fn push_metrics(
        &self,
        request: Request<WriteBatchRequest>,
    ) -> Result<Response<WriteBatchResponse>, Status> {
        let payload = request.into_inner();
        let mut accepted = 0u64;
        let mut rows = Vec::with_capacity(payload.metrics.len());

        for metric in payload.metrics {
            let mapped = Self::map_metric(metric)?;
            rows.push(mapped);
        }

        if !rows.is_empty() {
            accepted = self
                .ingestor
                .ingest_metrics(rows)
                .await
                .map_err(|err| Status::unavailable(format!("failed to ingest: {err}")))?;
        }

        if payload.force_flush {
            let _ = self.ingestor.flush().await;
        }

        let stats = self.ingestor.stats();
        Ok(Response::new(WriteBatchResponse {
            accepted,
            queued: stats.queue_depth.load(std::sync::atomic::Ordering::Relaxed),
            flushed: stats
                .last_batch_len
                .load(std::sync::atomic::Ordering::Relaxed),
        }))
    }

    async fn flush(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let _ = self.ingestor.flush().await;
        let stats = self.ingestor.stats();
        Ok(Response::new(self.to_health(&stats)))
    }
}

pub async fn serve_uds(socket_path: &str, service: IngestService) -> Result<()> {
    if Path::new(socket_path).exists() {
        tokio::fs::remove_file(socket_path).await.ok();
    }

    let uds = UnixListener::bind(socket_path)?;
    let (mut health_reporter, health_service) = health_reporter();
    health_reporter
        .set_serving::<IngestorServer<IngestService>>()
        .await;

    let incoming = UnixListenerStream::new(uds);

    Server::builder()
        .add_service(health_service)
        .add_service(IngestorServer::new(service))
        .serve_with_incoming(incoming)
        .await?;

    Ok(())
}
