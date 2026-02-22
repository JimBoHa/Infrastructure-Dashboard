use crate::config::Config;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio::time::{Duration, MissedTickBehavior};

#[derive(Clone)]
pub struct PredictiveFeed {
    tx: mpsc::Sender<PredictiveFeedItem>,
}

#[derive(Clone, Debug)]
pub struct PredictiveFeedItem {
    pub sensor_id: String,
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub quality: i32,
}

#[derive(Serialize)]
struct PredictiveIngestItem {
    sensor_id: String,
    timestamp: DateTime<Utc>,
    value: f64,
    quality: i32,
}

#[derive(Serialize)]
struct PredictiveIngestPayload {
    items: Vec<PredictiveIngestItem>,
}

impl PredictiveFeed {
    pub fn new(config: &Config) -> Option<Self> {
        let url = config.predictive_feed_url.clone()?;
        let (tx, rx) = mpsc::channel(config.predictive_feed_queue.max(1));
        let token = config.predictive_feed_token.clone();
        let batch_size = config.predictive_feed_batch_size.max(1);
        let flush_interval = config.predictive_feed_flush_interval();

        tokio::spawn(async move {
            run_predictive_feed(rx, url, token, batch_size, flush_interval).await;
        });

        Some(Self { tx })
    }

    pub fn enqueue(&self, item: PredictiveFeedItem) {
        if let Err(err) = self.tx.try_send(item) {
            tracing::warn!(error=%err, "predictive feed queue full; dropping sample");
        }
    }
}

async fn run_predictive_feed(
    mut rx: mpsc::Receiver<PredictiveFeedItem>,
    url: String,
    token: Option<String>,
    batch_size: usize,
    flush_interval: Duration,
) {
    let client = Client::new();
    let mut ticker = tokio::time::interval(flush_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut buffer: Vec<PredictiveFeedItem> = Vec::with_capacity(batch_size);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Err(err) = flush_predictive_feed(&client, &url, token.as_deref(), &mut buffer).await {
                    tracing::warn!(error=%err, "predictive feed flush failed");
                }
            }
            msg = rx.recv() => {
                match msg {
                    Some(item) => {
                        buffer.push(item);
                        if buffer.len() >= batch_size {
                            if let Err(err) = flush_predictive_feed(&client, &url, token.as_deref(), &mut buffer).await {
                                tracing::warn!(error=%err, "predictive feed flush failed");
                            }
                        }
                    }
                    None => {
                        let _ = flush_predictive_feed(&client, &url, token.as_deref(), &mut buffer).await;
                        break;
                    }
                }
            }
        }
    }
}

async fn flush_predictive_feed(
    client: &Client,
    url: &str,
    token: Option<&str>,
    buffer: &mut Vec<PredictiveFeedItem>,
) -> Result<(), reqwest::Error> {
    if buffer.is_empty() {
        return Ok(());
    }

    let items: Vec<PredictiveIngestItem> = buffer
        .drain(..)
        .map(|item| PredictiveIngestItem {
            sensor_id: item.sensor_id,
            timestamp: item.timestamp,
            value: item.value,
            quality: item.quality,
        })
        .collect();
    let payload = PredictiveIngestPayload { items };

    let mut request = client.post(url).json(&payload);
    if let Some(token) = token {
        request = request.header("X-Predictive-Ingest-Token", token);
    }

    let response = request.send().await?;
    if !response.status().is_success() {
        tracing::warn!(status=%response.status(), "predictive feed returned non-success");
    }
    Ok(())
}
