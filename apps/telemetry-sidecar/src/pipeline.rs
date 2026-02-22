use crate::ack::AckCommand;
use crate::telemetry::MetricRow;
use anyhow::Result;
use chrono::Utc;
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, QueryBuilder};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub enum BatchCommand {
    Metric(MetricRow),
    Flush(oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct PipelineHandle {
    tx: mpsc::Sender<BatchCommand>,
    stats: Arc<IngestStats>,
}

impl PipelineHandle {
    pub fn new(tx: mpsc::Sender<BatchCommand>, stats: Arc<IngestStats>) -> Self {
        Self { tx, stats }
    }

    pub fn stats(&self) -> Arc<IngestStats> {
        self.stats.clone()
    }

    pub async fn enqueue(&self, metric: MetricRow) -> Result<()> {
        let queue_depth = self.stats.queue_depth.fetch_add(1, Ordering::Relaxed) + 1;
        let source = metric.source.as_deref().unwrap_or("ingest");
        tracing::trace!(queue_depth, sensor = %metric.sensor_id, source, "queued metric");
        if let Err(err) = self.tx.send(BatchCommand::Metric(metric)).await {
            self.stats.queue_depth.fetch_sub(1, Ordering::Relaxed);
            return Err(err.into());
        }
        Ok(())
    }

    pub async fn flush(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let _ = self.tx.send(BatchCommand::Flush(tx)).await;
        let _ = rx.await;
        Ok(())
    }
}

#[derive(Debug)]
pub struct IngestStats {
    pub queue_depth: AtomicU64,
    pub last_flush_unix_ms: AtomicU64,
    pub last_batch_len: AtomicU64,
    pub average_flush_micros: AtomicU64,
    pub inflight_flushes: AtomicU64,
    pub mqtt_connected: AtomicBool,
    pub last_error: Mutex<Option<String>>,
}

impl IngestStats {
    pub fn new() -> Self {
        Self {
            queue_depth: AtomicU64::new(0),
            last_flush_unix_ms: AtomicU64::new(0),
            last_batch_len: AtomicU64::new(0),
            average_flush_micros: AtomicU64::new(0),
            inflight_flushes: AtomicU64::new(0),
            mqtt_connected: AtomicBool::new(false),
            last_error: Mutex::new(None),
        }
    }

    pub fn set_mqtt_connected(&self, connected: bool) {
        self.mqtt_connected.store(connected, Ordering::Relaxed);
    }

    pub fn record_error(&self, err: impl Into<String>) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(err.into());
        }
    }

    pub fn clear_error(&self) {
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
    }
}

pub async fn build_pool(database_url: &str, max_connections: u32) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub fn spawn_worker(
    pool: PgPool,
    mut rx: mpsc::Receiver<BatchCommand>,
    stats: Arc<IngestStats>,
    batch_size: usize,
    flush_interval: Duration,
    ack_tx: Option<mpsc::UnboundedSender<AckCommand>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buffer: Vec<MetricRow> = Vec::with_capacity(batch_size);
        let mut ticker = tokio::time::interval(flush_interval);
        let ack_tx = ack_tx;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(err) = flush(&pool, &mut buffer, &stats, ack_tx.as_ref()).await {
                        tracing::warn!(error=%err, "flush on interval failed");
                    }
                }
                cmd = rx.recv() => {
                    match cmd {
                        Some(BatchCommand::Metric(metric)) => {
                            stats.queue_depth.fetch_sub(1, Ordering::Relaxed);
                            buffer.push(metric);
                            if buffer.len() >= batch_size {
                                if let Err(err) = flush(&pool, &mut buffer, &stats, ack_tx.as_ref()).await {
                                    tracing::warn!(error=%err, "flush on batch size failed");
                                }
                            }
                        }
                        Some(BatchCommand::Flush(done)) => {
                            if let Err(err) = flush(&pool, &mut buffer, &stats, ack_tx.as_ref()).await {
                                tracing::warn!(error=%err, "flush on demand failed");
                            }
                            let _ = done.send(());
                        }
                        None => {
                            if let Err(err) = flush(&pool, &mut buffer, &stats, ack_tx.as_ref()).await {
                                tracing::warn!(error=%err, "flush during shutdown failed");
                            }
                            break;
                        }
                    }
                }
            }
        }
    })
}

async fn flush(
    pool: &PgPool,
    buffer: &mut Vec<MetricRow>,
    stats: &Arc<IngestStats>,
    ack_tx: Option<&mpsc::UnboundedSender<AckCommand>>,
) -> Result<()> {
    if buffer.is_empty() {
        return Ok(());
    }

    let started = Instant::now();
    let inserted_at = Utc::now();
    stats.inflight_flushes.fetch_add(1, Ordering::Relaxed);
    let items = std::mem::take(buffer);
    let len = items.len();

    let mut builder: QueryBuilder<Postgres> =
        QueryBuilder::new("INSERT INTO metrics (sensor_id, ts, value, quality, inserted_at) ");
    builder.push_values(items.iter(), |mut b, metric| {
        b.push_bind(&metric.sensor_id)
            .push_bind(metric.timestamp)
            .push_bind(metric.value)
            .push_bind(metric.quality)
            .push_bind(inserted_at);
    });
    builder.push(" ON CONFLICT DO NOTHING");

    let result = builder.build().execute(pool).await;
    stats.inflight_flushes.fetch_sub(1, Ordering::Relaxed);

    match result {
        Ok(result) => {
            let inserted = result.rows_affected() as usize;
            if inserted < len {
                tracing::warn!(
                    inserted,
                    skipped = len.saturating_sub(inserted),
                    "skipped duplicate metric rows"
                );
            }

            if let Some(ack_tx) = ack_tx {
                let mut grouped: std::collections::HashMap<(String, uuid::Uuid), std::collections::BTreeSet<u64>> =
                    std::collections::HashMap::new();
                for metric in &items {
                    let (Some(node_mqtt_id), Some(stream_id), Some(seq)) =
                        (metric.source.as_ref(), metric.stream_id, metric.seq)
                    else {
                        continue;
                    };
                    grouped
                        .entry((node_mqtt_id.clone(), stream_id))
                        .or_default()
                        .insert(seq);
                }
                for ((node_mqtt_id, stream_id), seqs) in grouped {
                    let seqs: Vec<u64> = seqs.into_iter().collect();
                    let _ = ack_tx.send(AckCommand::Committed {
                        node_mqtt_id,
                        stream_id,
                        seqs,
                    });
                }
            }

            stats.last_batch_len.store(len as u64, Ordering::Relaxed);
            let now = Utc::now().timestamp_millis() as u64;
            stats.last_flush_unix_ms.store(now, Ordering::Relaxed);
            let micros = started.elapsed().as_micros() as u64;
            let prev = stats.average_flush_micros.load(Ordering::Relaxed);
            let avg = if prev == 0 {
                micros
            } else {
                (prev + micros) / 2
            };
            stats.average_flush_micros.store(avg, Ordering::Relaxed);
            stats.clear_error();
            tracing::debug!(len, micros, "flushed metrics batch");
        }
        Err(err) => {
            stats.record_error(err.to_string());
            tracing::error!(error=%err, "failed to flush metrics");
            buffer.extend(items);
            return Err(err.into());
        }
    }

    Ok(())
}
