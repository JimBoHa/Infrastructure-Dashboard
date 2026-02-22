use crate::config::Config;
use crate::spool::{LossEvent, LossRange, PublishSample, SpoolHandle, TimeQuality};
use anyhow::{anyhow, Context, Result};
use crc32c::crc32c;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Sleep};
use uuid::Uuid;

const SEGMENT_HEADER_LEN: u64 = 64;
const FRAME_HEADER_LEN: usize = 8;
const MAX_FRAME_LEN: usize = 1024 * 1024;
const SAMPLE_RECORD_LEN: usize = 40;

#[derive(Debug, Deserialize)]
struct AckPayload {
    stream_id: String,
    acked_seq: u64,
}

#[derive(Debug, Deserialize)]
struct SpoolStateDisk {
    stream_id: String,
    losses: Vec<LossRange>,
}

pub async fn run_mqtt_forwarder(
    config: Config,
    spool: SpoolHandle,
    live_rx: mpsc::Receiver<PublishSample>,
    loss_rx: mpsc::Receiver<LossEvent>,
) -> Result<()> {
    let ack_topic = format!("{}/{}/ack", config.mqtt_topic_prefix, config.node_id);
    let loss_topic = format!("{}/{}/loss", config.mqtt_topic_prefix, config.node_id);

    let mut live_rx = live_rx;
    let mut loss_rx = loss_rx;

    loop {
        let mut fast_opts = mqtt_options(&config, &config.mqtt_client_id);
        fast_opts.set_keep_alive(Duration::from_secs(15));

        let replay_id = format!("{}-replay", config.mqtt_client_id);
        let mut replay_opts = mqtt_options(&config, &replay_id);
        replay_opts.set_keep_alive(Duration::from_secs(15));

        let (fast_client, fast_eventloop) = AsyncClient::new(fast_opts, 256);
        let (replay_client, replay_eventloop) = AsyncClient::new(replay_opts, 256);

        if let Err(err) = fast_client.subscribe(ack_topic.clone(), QoS::AtLeastOnce).await {
            tracing::warn!(error=%err, "failed to subscribe to ack topic; retrying");
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        tracing::info!("MQTT connected; publishing live telemetry + replay");

        // Best-effort: publish any pending loss ranges on connect.
        publish_pending_losses_from_state(&fast_client, &config, &loss_topic).await;

        let mut replay = ReplayState::new(config.clone())?;

        let (ack_notify_tx, mut ack_notify_rx) = mpsc::unbounded_channel::<()>();
        let mut fast_poller = spawn_fast_poller(
            fast_eventloop,
            spool.clone(),
            ack_topic.clone(),
            ack_notify_tx,
        );
        let mut replay_poller = spawn_replay_poller(replay_eventloop);

        let replay_sleep: Sleep = sleep(Duration::from_millis(0));
        tokio::pin!(replay_sleep);

        let mut loss_republish = tokio::time::interval(Duration::from_secs(30));
        loss_republish.tick().await;

        let mut last_err: Option<anyhow::Error> = None;

        loop {
            tokio::select! {
                res = &mut fast_poller => {
                    match res {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => last_err = Some(err),
                        Err(err) => last_err = Some(err.into()),
                    }
                    break;
                }

                res = &mut replay_poller => {
                    match res {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => last_err = Some(err),
                        Err(err) => last_err = Some(err.into()),
                    }
                    break;
                }

                maybe = ack_notify_rx.recv() => {
                    if maybe.is_none() {
                        break;
                    }
                    // Hint to the replay loop that ACK progressed; it will refresh status on next tick.
                    replay_sleep.as_mut().reset(tokio::time::Instant::now());
                }

                maybe = loss_rx.recv() => {
                    let Some(event) = maybe else { break; };
                    if let Err(err) = publish_loss(&fast_client, &loss_topic, &event).await {
                        tracing::debug!(error=%err, "failed to publish loss event");
                    }
                }

                maybe = live_rx.recv() => {
                    let Some(sample) = maybe else { break; };
                    if let Err(err) = publish_sample(&fast_client, &config, sample, false).await {
                        tracing::debug!(error=%err, "failed to publish live sample");
                    }
                }

                _ = loss_republish.tick() => {
                    publish_pending_losses_from_state(&fast_client, &config, &loss_topic).await;
                }

                _ = &mut replay_sleep => {
                    match replay.step(&replay_client, &spool).await {
                        Ok(next_delay) => {
                            replay_sleep.as_mut().reset(tokio::time::Instant::now() + next_delay);
                        }
                        Err(err) => {
                            tracing::debug!(error=%err, "replay loop step failed");
                            replay_sleep.as_mut().reset(tokio::time::Instant::now() + Duration::from_millis(500));
                        }
                    }
                }
            }
        }

        fast_poller.abort();
        replay_poller.abort();

        tracing::warn!(error=?last_err, "MQTT connection loop restarting");
        sleep(Duration::from_secs(1)).await;
    }
}

fn spawn_fast_poller(
    mut eventloop: rumqttc::EventLoop,
    spool: SpoolHandle,
    ack_topic: String,
    notify: mpsc::UnboundedSender<()>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        loop {
            match eventloop.poll().await {
                Ok(Event::Incoming(Incoming::Publish(publish))) => {
                    if publish.topic != ack_topic {
                        continue;
                    }
                    let ack: AckPayload = match serde_json::from_slice(&publish.payload) {
                        Ok(parsed) => parsed,
                        Err(_) => continue,
                    };
                    let Ok(stream_id) = Uuid::parse_str(ack.stream_id.trim()) else {
                        continue;
                    };
                    spool.update_ack(stream_id, ack.acked_seq);
                    let _ = notify.send(());
                }
                Ok(_) => {}
                Err(err) => return Err(err.into()),
            }
        }
    })
}

fn spawn_replay_poller(mut eventloop: rumqttc::EventLoop) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        loop {
            eventloop.poll().await.map_err(|err| anyhow!(err))?;
        }
    })
}

fn mqtt_options(config: &Config, client_id: &str) -> MqttOptions {
    let mut mqttoptions = MqttOptions::new(client_id, config.mqtt_host.clone(), config.mqtt_port);
    if let Some(username) = &config.mqtt_username {
        mqttoptions.set_credentials(
            username.clone(),
            config.mqtt_password.clone().unwrap_or_default(),
        );
    }
    mqttoptions
}

async fn publish_sample(
    client: &AsyncClient,
    config: &Config,
    sample: PublishSample,
    backfill: bool,
) -> Result<usize> {
    let topic = telemetry_topic(config, &sample.sensor_id);
    let encoded = encode_telemetry_payload(&sample, backfill)?;
    client
        .publish(topic, QoS::AtLeastOnce, false, encoded.clone())
        .await?;
    Ok(encoded.len())
}

fn telemetry_topic(config: &Config, sensor_id: &str) -> String {
    format!(
        "{}/{}/{}/telemetry",
        config.mqtt_topic_prefix, config.node_id, sensor_id
    )
}

fn encode_telemetry_payload(sample: &PublishSample, backfill: bool) -> Result<Vec<u8>> {
    let time_quality = match sample.time_quality {
        TimeQuality::Good => "good",
        TimeQuality::Unsynced => "unsynced",
        TimeQuality::Unknown => "unknown",
    };

    let payload = json!({
        "timestamp": sample.timestamp_ms,
        "value": sample.value,
        "quality": sample.quality,
        "seq": sample.seq,
        "stream_id": sample.stream_id.to_string(),
        "backfill": backfill,
        "time_quality": time_quality,
        "mono_ms": sample.monotonic_ms,
    });

    Ok(serde_json::to_vec(&payload)?)
}

async fn publish_loss(client: &AsyncClient, topic: &str, event: &LossEvent) -> Result<()> {
    let payload = json!({
        "stream_id": event.stream_id.to_string(),
        "start_seq": event.range.start_seq,
        "end_seq": event.range.end_seq,
        "dropped_at": event.range.dropped_at,
        "reason": "spool_cap_drop_oldest_segment",
    });
    client
        .publish(topic, QoS::AtLeastOnce, false, serde_json::to_vec(&payload)?)
        .await?;
    Ok(())
}

async fn publish_pending_losses_from_state(client: &AsyncClient, config: &Config, topic: &str) {
    let Ok(state) = load_spool_state(&config.spool_dir) else {
        return;
    };
    let Ok(stream_id) = Uuid::parse_str(state.stream_id.trim()) else {
        return;
    };
    for range in state.losses {
        let event = LossEvent { stream_id, range };
        if publish_loss(client, topic, &event).await.is_err() {
            break;
        }
    }
}

fn load_spool_state(spool_dir: &Path) -> Result<SpoolStateDisk> {
    let path = spool_dir.join("state.json");
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).context("parse state.json")
}

#[derive(Debug, Clone)]
struct TokenBucket {
    rate_per_sec: f64,
    capacity: f64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(rate_per_sec: u32) -> Self {
        let rate_per_sec = rate_per_sec.max(1) as f64;
        Self {
            rate_per_sec,
            capacity: rate_per_sec,
            tokens: rate_per_sec,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        if elapsed <= 0.0 {
            return;
        }
        self.tokens = (self.tokens + elapsed * self.rate_per_sec).min(self.capacity);
        self.last_refill = now;
    }

    fn try_take(&mut self, cost: f64) -> bool {
        self.refill();
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    fn delay_for(&mut self, cost: f64) -> Duration {
        self.refill();
        if self.tokens >= cost {
            return Duration::from_millis(0);
        }
        let deficit = (cost - self.tokens).max(0.0);
        let secs = deficit / self.rate_per_sec;
        Duration::from_secs_f64(secs.min(1.0))
    }
}

#[derive(Debug)]
struct ReplayState {
    config: Config,
    stream_id: Option<Uuid>,
    acked_seq: u64,
    next_seq: u64,
    msg_bucket: TokenBucket,
    byte_bucket: TokenBucket,
    cursor: Option<ReplayCursor>,
    pending: Option<PublishSample>,
    last_state_refresh: Instant,
    last_ack_progress: Instant,
    last_stall_reset: Instant,
}

impl ReplayState {
    fn new(config: Config) -> Result<Self> {
        let replay_msgs_per_sec = config.replay_msgs_per_sec;
        let replay_bytes_per_sec = config.replay_bytes_per_sec;
        let now = Instant::now();
        Ok(Self {
            config,
            stream_id: None,
            acked_seq: 0,
            next_seq: 1,
            msg_bucket: TokenBucket::new(replay_msgs_per_sec),
            byte_bucket: TokenBucket::new(replay_bytes_per_sec),
            cursor: None,
            pending: None,
            last_state_refresh: now - Duration::from_secs(60),
            last_ack_progress: now,
            last_stall_reset: now,
        })
    }

    async fn step(&mut self, client: &AsyncClient, spool: &SpoolHandle) -> Result<Duration> {
        let mut backlog_samples = None;
        if self.last_state_refresh.elapsed() >= Duration::from_secs(1) || self.cursor.is_none() {
            let status = spool.status().await?;
            let stream_id = Uuid::parse_str(status.stream_id.trim()).context("invalid stream_id")?;
            if self.stream_id != Some(stream_id) {
                self.stream_id = Some(stream_id);
                self.acked_seq = status.acked_seq;
                self.next_seq = status.acked_seq.saturating_add(1);
                self.cursor = Some(ReplayCursor::new(self.config.spool_dir.clone(), stream_id)?);
                self.pending = None;
                self.last_ack_progress = Instant::now();
                self.last_stall_reset = Instant::now();
            } else if status.acked_seq > self.acked_seq {
                self.acked_seq = status.acked_seq;
                self.next_seq = self.next_seq.max(status.acked_seq.saturating_add(1));
                self.last_ack_progress = Instant::now();
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.seq <= self.acked_seq)
                {
                    self.pending = None;
                }
            }
            backlog_samples = Some(status.backlog_samples);
            self.last_state_refresh = Instant::now();

            // If ACK is not moving but we still have backlog, it's likely we skipped a seq (e.g.,
            // rate-limit deferral) or the controller missed a commit. Reset our cursor to the first
            // unacked seq and try again so the spool can always make forward progress.
            if status.backlog_samples > 0
                && self.next_seq > status.acked_seq.saturating_add(1)
                && self.last_ack_progress.elapsed() >= Duration::from_secs(5)
                && self.last_stall_reset.elapsed() >= Duration::from_secs(5)
            {
                let desired = status.acked_seq.saturating_add(1);
                if let Some(cursor) = self.cursor.as_mut() {
                    let _ = cursor.seek_to_seq(desired);
                }
                self.next_seq = desired;
                self.pending = None;
                self.last_stall_reset = Instant::now();
            }
        }

        let Some(stream_id) = self.stream_id else {
            return Ok(Duration::from_millis(250));
        };
        let Some(cursor) = self.cursor.as_mut() else {
            return Ok(Duration::from_millis(250));
        };

        if self.next_seq <= self.acked_seq {
            self.next_seq = self.acked_seq.saturating_add(1);
        }

        if let Some(pending) = self.pending.as_ref() {
            if pending.seq <= self.acked_seq {
                self.pending = None;
            }
        }

        if self.pending.is_none() {
            let maybe = cursor.next_sample(self.next_seq).await?;
            let Some(sample) = maybe else {
                return Ok(Duration::from_millis(200));
            };
            if sample.seq <= self.acked_seq {
                self.next_seq = self.acked_seq.saturating_add(1);
                return Ok(Duration::from_millis(0));
            }
            self.pending = Some(sample);
        }

        let sample = self.pending.as_ref().expect("pending sample set");
        if sample.seq != self.next_seq {
            // If we see a gap, it means the spool no longer contains `next_seq` (disk cap drop,
            // corruption, or a previous buggy replay). Advance to the observed seq; the controller
            // must have received a corresponding loss range to ACK past the missing region.
            if sample.seq > self.next_seq {
                self.next_seq = sample.seq;
            }
        }

        let topic = telemetry_topic(&self.config, &sample.sensor_id);
        let payload = encode_telemetry_payload(&sample, true)?;
        let payload_len = payload.len();

        if !self.msg_bucket.try_take(1.0) || !self.byte_bucket.try_take(payload_len as f64) {
            let delay = self
                .msg_bucket
                .delay_for(1.0)
                .max(self.byte_bucket.delay_for(payload_len as f64));
            return Ok(delay.max(Duration::from_millis(1)));
        }

        client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await?;
        let bytes = payload_len;
        let published_seq = sample.seq;
        let published_stream_id = sample.stream_id;
        tracing::trace!(seq = published_seq, bytes, backlog_samples, "published replay sample");

        self.pending = None;
        self.next_seq = published_seq.saturating_add(1);
        if stream_id != published_stream_id {
            return Err(anyhow!("replay stream_id changed mid-flight"));
        }

        Ok(Duration::from_millis(0))
    }
}

#[derive(Debug, Clone)]
struct SegmentInfo {
    path: PathBuf,
    start_seq: u64,
    end_seq: Option<u64>,
    is_open: bool,
}

#[derive(Debug)]
struct ReplayCursor {
    spool_dir: PathBuf,
    stream_id: Uuid,
    sensor_by_index: HashMap<u32, String>,
    segments: Vec<SegmentInfo>,
    segment_index: usize,
    file: Option<fs::File>,
    last_seq: Option<u64>,
    last_refresh: Instant,
}

impl ReplayCursor {
    fn new(spool_dir: PathBuf, stream_id: Uuid) -> Result<Self> {
        let sensor_by_index = load_sensor_map_by_index(&spool_dir).unwrap_or_default();
        Ok(Self {
            spool_dir,
            stream_id,
            sensor_by_index,
            segments: Vec::new(),
            segment_index: 0,
            file: None,
            last_seq: None,
            last_refresh: Instant::now() - Duration::from_secs(60),
        })
    }

    async fn next_sample(&mut self, min_seq: u64) -> Result<Option<PublishSample>> {
        if self.last_refresh.elapsed() >= Duration::from_secs(2) || self.segments.is_empty() {
            self.refresh_segments()?;
            self.last_refresh = Instant::now();
        }

        if self.file.is_none() {
            self.seek_to_seq(min_seq)?;
        }

        loop {
            let Some(file) = self.file.as_mut() else {
                return Ok(None);
            };
            let Some(payload) = read_next_frame_payload(file)? else {
                // End of this segment for now.
                let is_open = self
                    .segments
                    .get(self.segment_index)
                    .map(|seg| seg.is_open)
                    .unwrap_or(false);
                if is_open {
                    return Ok(None);
                }
                self.advance_segment()?;
                continue;
            };

            let Some(record) = decode_sample_record(&payload) else {
                // Corrupt tail: stop this segment.
                let is_open = self
                    .segments
                    .get(self.segment_index)
                    .map(|seg| seg.is_open)
                    .unwrap_or(false);
                if is_open {
                    return Ok(None);
                }
                self.advance_segment()?;
                continue;
            };

            self.last_seq = Some(record.seq);
            if record.seq < min_seq {
                continue;
            }

            let sensor_id = match self.sensor_by_index.get(&record.sensor_idx) {
                Some(id) => id.clone(),
                None => {
                    self.sensor_by_index = load_sensor_map_by_index(&self.spool_dir).unwrap_or_default();
                    self.sensor_by_index
                        .get(&record.sensor_idx)
                        .cloned()
                        .unwrap_or_else(|| format!("unknown-{}", record.sensor_idx))
                }
            };

            return Ok(Some(PublishSample {
                sensor_id,
                timestamp_ms: record.timestamp_ms,
                value: record.value,
                quality: record.quality,
                seq: record.seq,
                stream_id: self.stream_id,
                time_quality: record.time_quality,
                monotonic_ms: record.monotonic_ms,
            }));
        }
    }

    fn refresh_segments(&mut self) -> Result<()> {
        let mut segments: Vec<SegmentInfo> = Vec::new();
        let prefix = format!("seg-{}-", self.stream_id);
        for entry in fs::read_dir(&self.spool_dir).context("read spool dir")? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if !name.starts_with(&prefix) {
                continue;
            }
            if name.ends_with(".seg") {
                if let Some((start, end)) = parse_closed_segment_range(name) {
                    segments.push(SegmentInfo {
                        path,
                        start_seq: start,
                        end_seq: Some(end),
                        is_open: false,
                    });
                }
            } else if name.ends_with(".open") {
                let start = parse_open_start_seq(name).unwrap_or(0);
                segments.push(SegmentInfo {
                    path,
                    start_seq: start,
                    end_seq: None,
                    is_open: true,
                });
            }
        }
        segments.sort_by_key(|seg| seg.start_seq);

        // Keep our position if possible.
        if let Some(last_seq) = self.last_seq {
            if let Some((idx, _)) = segments
                .iter()
                .enumerate()
                .find(|(_, seg)| seg.start_seq <= last_seq && seg.end_seq.map(|end| last_seq <= end).unwrap_or(true))
            {
                self.segment_index = idx;
            }
        }

        self.segments = segments;
        Ok(())
    }

    fn seek_to_seq(&mut self, seq: u64) -> Result<()> {
        if self.segments.is_empty() {
            self.refresh_segments()?;
        }

        let Some((idx, _)) = self
            .segments
            .iter()
            .enumerate()
            .find(|(_, seg)| seg.start_seq <= seq && seg.end_seq.map(|end| seq <= end).unwrap_or(true))
            .or_else(|| self.segments.iter().enumerate().find(|(_, seg)| seg.start_seq >= seq))
        else {
            self.file = None;
            return Ok(());
        };

        self.segment_index = idx;
        self.open_current_segment()
    }

    fn open_current_segment(&mut self) -> Result<()> {
        let Some(seg) = self.segments.get(self.segment_index).cloned() else {
            self.file = None;
            return Ok(());
        };
        let mut file = fs::File::open(&seg.path).with_context(|| format!("open {}", seg.path.display()))?;
        if file.metadata().map(|m| m.len()).unwrap_or(0) < SEGMENT_HEADER_LEN {
            self.file = None;
            return Ok(());
        }
        file.seek(SeekFrom::Start(SEGMENT_HEADER_LEN))?;
        self.file = Some(file);
        Ok(())
    }

    fn advance_segment(&mut self) -> Result<()> {
        self.segment_index = self.segment_index.saturating_add(1);
        self.file = None;
        if self.segment_index >= self.segments.len() {
            self.refresh_segments()?;
            if self.segment_index >= self.segments.len() {
                return Ok(());
            }
        }
        self.open_current_segment()
    }
}

fn load_sensor_map_by_index(spool_dir: &Path) -> Result<HashMap<u32, String>> {
    let path = spool_dir.join("sensor_map.json");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let decoded: HashMap<String, u32> = serde_json::from_str(&raw).context("parse sensor_map.json")?;
    let mut out = HashMap::new();
    for (sensor_id, idx) in decoded {
        out.insert(idx, sensor_id);
    }
    Ok(out)
}

fn parse_closed_segment_range(name: &str) -> Option<(u64, u64)> {
    let trimmed = name.trim_end_matches(".seg");
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() < 4 {
        return None;
    }
    let start = parts.get(parts.len() - 2)?.parse().ok()?;
    let end = parts.get(parts.len() - 1)?.parse().ok()?;
    Some((start, end))
}

fn parse_open_start_seq(name: &str) -> Option<u64> {
    let trimmed = name.trim_end_matches(".open");
    let parts: Vec<&str> = trimmed.split('-').collect();
    parts.last()?.parse().ok()
}

#[derive(Debug, Clone)]
struct SampleRecord {
    sensor_idx: u32,
    seq: u64,
    timestamp_ms: i64,
    value: f64,
    quality: i16,
    time_quality: TimeQuality,
    monotonic_ms: u64,
}

fn decode_sample_record(buf: &[u8]) -> Option<SampleRecord> {
    if buf.len() != SAMPLE_RECORD_LEN {
        return None;
    }
    let sensor_idx = u32::from_le_bytes(buf[0..4].try_into().ok()?);
    let seq = u64::from_le_bytes(buf[4..12].try_into().ok()?);
    let timestamp_ms = i64::from_le_bytes(buf[12..20].try_into().ok()?);
    let value = f64::from_le_bytes(buf[20..28].try_into().ok()?);
    let quality = i16::from_le_bytes(buf[28..30].try_into().ok()?);
    let time_quality_bits = u16::from_le_bytes(buf[30..32].try_into().ok()?);
    let monotonic_ms = u64::from_le_bytes(buf[32..40].try_into().ok()?);
    let time_quality = match time_quality_bits {
        1 => TimeQuality::Good,
        2 => TimeQuality::Unsynced,
        _ => TimeQuality::Unknown,
    };
    Some(SampleRecord {
        sensor_idx,
        seq,
        timestamp_ms,
        value,
        quality,
        time_quality,
        monotonic_ms,
    })
}

fn read_next_frame_payload(file: &mut fs::File) -> Result<Option<Vec<u8>>> {
    let mut header = [0u8; FRAME_HEADER_LEN];
    match file.read_exact(&mut header) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    let len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if len == 0 || len > MAX_FRAME_LEN {
        return Ok(None);
    }

    let mut payload = vec![0u8; len];
    match file.read_exact(&mut payload) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    if crc32c(&payload) != crc {
        return Ok(None);
    }

    Ok(Some(payload))
}
