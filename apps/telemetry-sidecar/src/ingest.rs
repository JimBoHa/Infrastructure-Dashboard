mod db;
mod ingestor;
mod rolling;
mod state;
mod types;

#[cfg(test)]
mod tests;

use crate::pipeline::PipelineHandle;
use crate::predictive_feed::PredictiveFeed;
use chrono::Duration as ChronoDuration;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

const COV_TOLERANCE: f64 = 1e-6;
const STATUS_ONLINE: &str = "online";
const STATUS_OFFLINE: &str = "offline";

#[derive(Clone)]
pub struct TelemetryIngestor {
    pool: PgPool,
    pipeline: PipelineHandle,
    state: Arc<Mutex<state::IngestState>>,
    offline_threshold: ChronoDuration,
    predictive_feed: Option<PredictiveFeed>,
    ack_tx: Option<mpsc::UnboundedSender<crate::ack::AckCommand>>,
}
