use crate::config::Config;
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use crc32c::crc32c;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

const SEGMENT_MAGIC: &[u8; 8] = b"FDSPOOL1";
const SEGMENT_VERSION: u32 = 1;
const SEGMENT_HEADER_LEN: usize = 64;
const SAMPLE_RECORD_LEN: usize = 40;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeQuality {
    Unknown = 0,
    Good = 1,
    Unsynced = 2,
}

impl TimeQuality {
    fn as_flag_bits(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossRange {
    pub start_seq: u64,
    pub end_seq: u64,
    pub dropped_at: String,
}

#[derive(Debug, Clone)]
pub struct LossEvent {
    pub stream_id: Uuid,
    pub range: LossRange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpoolStateDisk {
    stream_id: String,
    next_seq: u64,
    acked_seq: u64,
    open_segment_start_seq: Option<u64>,
    losses: Vec<LossRange>,
}

#[derive(Debug, Clone)]
pub struct PublishSample {
    pub sensor_id: String,
    pub timestamp_ms: i64,
    pub value: f64,
    pub quality: i16,
    pub seq: u64,
    pub stream_id: Uuid,
    pub time_quality: TimeQuality,
    pub monotonic_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpoolStatus {
    pub stream_id: String,
    pub next_seq: u64,
    pub acked_seq: u64,
    pub spool_bytes: u64,
    pub max_spool_bytes: u64,
    pub keep_free_bytes: u64,
    pub free_bytes: Option<u64>,
    pub closed_segments: usize,
    pub open_segment_start_seq: Option<u64>,
    pub open_segment_bytes: u64,
    pub backlog_samples: u64,
    pub replay_msgs_per_sec: u32,
    pub replay_bytes_per_sec: u32,
    pub estimated_drain_seconds: Option<u64>,
    pub losses_pending: usize,
    pub losses: Vec<LossRange>,
    pub oldest_unacked_timestamp_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AppendResult {
    pub accepted: u64,
}

#[derive(Debug)]
pub enum SpoolCommand {
    AppendSamples {
        samples: Vec<IncomingSample>,
        respond_to: oneshot::Sender<Result<AppendResult>>,
    },
    UpdateAck {
        stream_id: Uuid,
        acked_seq: u64,
    },
    GetStatus {
        respond_to: oneshot::Sender<SpoolStatus>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncomingSample {
    pub sensor_id: String,
    pub timestamp_ms: i64,
    pub value: f64,
    pub quality: i16,
    #[serde(default)]
    pub time_quality: Option<TimeQuality>,
}

#[derive(Clone)]
pub struct SpoolHandle {
    tx: mpsc::UnboundedSender<SpoolCommand>,
}

impl SpoolHandle {
    pub fn new(tx: mpsc::UnboundedSender<SpoolCommand>) -> Self {
        Self { tx }
    }

    pub async fn append_samples(&self, samples: Vec<IncomingSample>) -> Result<AppendResult> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(SpoolCommand::AppendSamples {
                samples,
                respond_to: tx,
            })
            .map_err(|_| anyhow!("spool thread stopped"))?;
        rx.await.context("spool thread dropped response")?
    }

    pub fn update_ack(&self, stream_id: Uuid, acked_seq: u64) {
        let _ = self.tx.send(SpoolCommand::UpdateAck { stream_id, acked_seq });
    }

    pub async fn status(&self) -> Result<SpoolStatus> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(SpoolCommand::GetStatus { respond_to: tx })
            .map_err(|_| anyhow!("spool thread stopped"))?;
        Ok(rx.await.context("spool thread dropped response")?)
    }
}

#[derive(Debug, Clone)]
pub struct SensorMap {
    pub by_id: HashMap<String, u32>,
    pub by_index: HashMap<u32, String>,
    pub next_index: u32,
}

impl SensorMap {
    fn empty() -> Self {
        Self {
            by_id: HashMap::new(),
            by_index: HashMap::new(),
            next_index: 1,
        }
    }

    fn get_or_insert(&mut self, sensor_id: &str) -> u32 {
        if let Some(existing) = self.by_id.get(sensor_id) {
            return *existing;
        }
        let idx = self.next_index;
        self.next_index = self.next_index.saturating_add(1);
        self.by_id.insert(sensor_id.to_string(), idx);
        self.by_index.insert(idx, sensor_id.to_string());
        idx
    }
}

pub fn spawn_spool_thread(
    config: Config,
    publish_tx: mpsc::Sender<PublishSample>,
    loss_tx: mpsc::Sender<LossEvent>,
) -> Result<SpoolHandle> {
    let (tx, mut rx) = mpsc::unbounded_channel::<SpoolCommand>();

    std::thread::Builder::new()
        .name("spool-writer".to_string())
        .spawn(move || {
            if let Err(err) = run_spool_thread(config, publish_tx, loss_tx, &mut rx) {
                tracing::error!(error=%err, "spool thread exited");
            }
        })
        .context("failed to spawn spool thread")?;

    Ok(SpoolHandle::new(tx))
}

struct SegmentWriter {
    path: PathBuf,
    file: fs::File,
    start_seq: u64,
    created_at: Instant,
}

struct SpoolRuntime {
    config: Config,
    state_path: PathBuf,
    sensor_map_path: PathBuf,
    stream_id: Uuid,
    next_seq: u64,
    acked_seq: u64,
    losses: Vec<LossRange>,
    sensor_map: SensorMap,
    spool_bytes: u64,
    segment: SegmentWriter,
    last_sync_at: Instant,
    publish_tx: mpsc::Sender<PublishSample>,
    loss_tx: mpsc::Sender<LossEvent>,
}

fn run_spool_thread(
    mut config: Config,
    publish_tx: mpsc::Sender<PublishSample>,
    loss_tx: mpsc::Sender<LossEvent>,
    rx: &mut mpsc::UnboundedReceiver<SpoolCommand>,
) -> Result<()> {
    fs::create_dir_all(&config.spool_dir)
        .with_context(|| format!("failed to create {}", config.spool_dir.display()))?;

    // If the user did not override max_spool_bytes, apply the default policy using filesystem size.
    if env::var("NODE_FORWARDER_MAX_SPOOL_BYTES").is_err() {
        if let Ok(dynamic) = compute_default_spool_budget(&config.spool_dir, config.keep_free_bytes) {
            config.max_spool_bytes = dynamic;
        }
    }

    let state_path = config.spool_dir.join("state.json");
    let sensor_map_path = config.spool_dir.join("sensor_map.json");

    let sensor_map = load_sensor_map(&sensor_map_path).unwrap_or_else(|err| {
        tracing::warn!(error=%err, "failed to load sensor map; starting fresh");
        SensorMap::empty()
    });

    let (stream_id, next_seq, acked_seq, open_start_seq, losses) =
        load_or_init_state(&state_path)?;

    let (segment, initial_spool_bytes) = open_or_create_segment(
        &config,
        stream_id,
        next_seq,
        open_start_seq,
    )?;

    let mut runtime = SpoolRuntime {
        config,
        state_path,
        sensor_map_path,
        stream_id,
        next_seq,
        acked_seq,
        losses,
        sensor_map,
        spool_bytes: initial_spool_bytes,
        segment,
        last_sync_at: Instant::now(),
        publish_tx,
        loss_tx,
    };

    // Best-effort: delete ACKed segments on startup.
    runtime.delete_acked_segments()?;
    runtime.enforce_caps()?;
    runtime.persist_state()?;

    // Best-effort: emit pending loss ranges so the controller can ACK past gaps after reconnect.
    for loss in runtime.losses.clone() {
        let _ = runtime.loss_tx.try_send(LossEvent {
            stream_id: runtime.stream_id,
            range: loss,
        });
    }

    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            SpoolCommand::AppendSamples { samples, respond_to } => {
                let res = runtime.append_samples(samples);
                let _ = respond_to.send(res);
            }
            SpoolCommand::UpdateAck { stream_id, acked_seq } => {
                if stream_id == runtime.stream_id && acked_seq > runtime.acked_seq {
                    runtime.acked_seq = acked_seq;
                    if let Err(err) = runtime.delete_acked_segments() {
                        tracing::warn!(error=%err, "failed to delete acked segments");
                    }
                    runtime.prune_losses();
                    let _ = runtime.persist_state();
                }
            }
            SpoolCommand::GetStatus { respond_to } => {
                let status = runtime.status();
                let _ = respond_to.send(status);
            }
        }
    }

    Ok(())
}

fn compute_default_spool_budget(spool_dir: &Path, keep_free_bytes: u64) -> Result<u64> {
    let (total, free) = statvfs_bytes(spool_dir)?;
    let five_percent = total / 20;
    let min = 1_u64 * 1024 * 1024 * 1024;
    let max = 25_u64 * 1024 * 1024 * 1024;
    let mut budget = five_percent.clamp(min, max);

    // Keep a floor of free space for the OS/logs/etc. If free is already below the floor, fall back
    // to the minimum budget and let cap enforcement drop oldest segments.
    if free > keep_free_bytes {
        budget = budget.min(free.saturating_sub(keep_free_bytes));
    }

    Ok(budget.max(min))
}

fn compute_free_bytes(spool_dir: &Path) -> Result<u64> {
    let (_total, free) = statvfs_bytes(spool_dir)?;
    Ok(free)
}

fn statvfs_bytes(spool_dir: &Path) -> Result<(u64, u64)> {
    use std::ffi::CString;
    let cpath = CString::new(spool_dir.as_os_str().to_string_lossy().as_bytes().to_vec())
        .context("invalid spool path")?;

    let mut out: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(cpath.as_ptr(), &mut out as *mut libc::statvfs) };
    if rc != 0 {
        return Err(anyhow!("statvfs failed"));
    }

    let block = if out.f_frsize > 0 {
        out.f_frsize as u64
    } else {
        out.f_bsize as u64
    };
    let total = (out.f_blocks as u64).saturating_mul(block);
    // f_bavail is free blocks for unprivileged users, which matches our service user behavior.
    let free = (out.f_bavail as u64).saturating_mul(block);
    Ok((total, free))
}

fn load_sensor_map(path: &Path) -> Result<SensorMap> {
    if !path.exists() {
        return Ok(SensorMap::empty());
    }
    let data = fs::read_to_string(path).context("read sensor map")?;
    let decoded: HashMap<String, u32> = serde_json::from_str(&data).context("parse sensor map")?;
    let mut map = SensorMap::empty();
    for (sensor_id, idx) in decoded {
        map.by_id.insert(sensor_id.clone(), idx);
        map.by_index.insert(idx, sensor_id);
        map.next_index = map.next_index.max(idx.saturating_add(1));
    }
    Ok(map)
}

fn persist_sensor_map(path: &Path, map: &SensorMap) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&map.by_id)?;
    fs::write(&tmp, json).context("write sensor map tmp")?;
    fs::rename(&tmp, path).context("rename sensor map")?;
    Ok(())
}

fn load_or_init_state(
    state_path: &Path,
) -> Result<(Uuid, u64, u64, Option<u64>, Vec<LossRange>)> {
    if state_path.exists() {
        let raw = fs::read_to_string(state_path).context("read state")?;
        let parsed: SpoolStateDisk = serde_json::from_str(&raw).context("parse state")?;
        let stream_id = Uuid::parse_str(&parsed.stream_id).context("invalid stream_id")?;
        return Ok((
            stream_id,
            parsed.next_seq.max(1),
            parsed.acked_seq,
            parsed.open_segment_start_seq,
            parsed.losses,
        ));
    }
    Ok((Uuid::new_v4(), 1, 0, None, Vec::new()))
}

fn persist_state(
    state_path: &Path,
    stream_id: Uuid,
    next_seq: u64,
    acked_seq: u64,
    open_segment_start_seq: Option<u64>,
    losses: &[LossRange],
) -> Result<()> {
    let tmp = state_path.with_extension("json.tmp");
    let disk = SpoolStateDisk {
        stream_id: stream_id.to_string(),
        next_seq,
        acked_seq,
        open_segment_start_seq,
        losses: losses.to_vec(),
    };
    fs::write(&tmp, serde_json::to_string_pretty(&disk)?).context("write state tmp")?;
    fs::rename(&tmp, state_path).context("rename state")?;
    Ok(())
}

fn open_or_create_segment(
    config: &Config,
    stream_id: Uuid,
    next_seq: u64,
    open_start_seq: Option<u64>,
) -> Result<(SegmentWriter, u64)> {
    let spool_bytes = compute_spool_bytes(&config.spool_dir, stream_id)?;
    if let Some(start_seq) = open_start_seq {
        let open_path = segment_open_path(&config.spool_dir, stream_id, start_seq);
        if open_path.exists() {
            let mut file = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&open_path)
                .with_context(|| format!("open {}", open_path.display()))?;
            recover_truncate_tail(&mut file)?;
            return Ok((
                SegmentWriter {
                    path: open_path,
                    file,
                    start_seq,
                    created_at: Instant::now(),
                },
                spool_bytes,
            ));
        }
    }

    // If we have an existing .open segment for the current stream, prefer it.
    if let Some(open_path) = find_any_open_segment(&config.spool_dir, stream_id)? {
        let start_seq = parse_open_start_seq(&open_path).unwrap_or(next_seq);
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&open_path)
            .with_context(|| format!("open {}", open_path.display()))?;
        recover_truncate_tail(&mut file)?;
        return Ok((
            SegmentWriter {
                path: open_path,
                file,
                start_seq,
                created_at: Instant::now(),
            },
            spool_bytes,
        ));
    }

    let (writer, _) = create_new_segment(config, stream_id, next_seq)?;
    Ok((writer, spool_bytes))
}

fn compute_spool_bytes(spool_dir: &Path, stream_id: Uuid) -> Result<u64> {
    let mut total = 0u64;
    for entry in fs::read_dir(spool_dir).context("read spool dir")? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if !name.contains(&stream_id.to_string()) {
            continue;
        }
        total = total.saturating_add(entry.metadata()?.len());
    }
    Ok(total)
}

fn segment_open_path(spool_dir: &Path, stream_id: Uuid, start_seq: u64) -> PathBuf {
    spool_dir.join(format!("seg-{}-{}.open", stream_id, start_seq))
}

fn segment_closed_path(spool_dir: &Path, stream_id: Uuid, start_seq: u64, end_seq: u64) -> PathBuf {
    spool_dir.join(format!("seg-{}-{}-{}.seg", stream_id, start_seq, end_seq))
}

fn find_any_open_segment(spool_dir: &Path, stream_id: Uuid) -> Result<Option<PathBuf>> {
    let prefix = format!("seg-{}-", stream_id);
    for entry in fs::read_dir(spool_dir).context("read spool dir")? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if name.starts_with(&prefix) && name.ends_with(".open") {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn parse_open_start_seq(path: &Path) -> Option<u64> {
    let name = path.file_name()?.to_str()?;
    // seg-<uuid>-<start>.open
    let parts: Vec<&str> = name.split('-').collect();
    if parts.len() < 3 {
        return None;
    }
    let last = parts.last()?.trim_end_matches(".open");
    last.parse().ok()
}

fn recover_truncate_tail(file: &mut fs::File) -> Result<()> {
    let end = file.metadata()?.len();
    if end < SEGMENT_HEADER_LEN as u64 {
        return Err(anyhow!("segment too small"));
    }
    file.seek(SeekFrom::Start(SEGMENT_HEADER_LEN as u64))?;
    let mut pos = SEGMENT_HEADER_LEN as u64;
    loop {
        let mut header = [0u8; 8];
        if file.read_exact(&mut header).is_err() {
            break;
        }
        let len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as u64;
        let crc = u32::from_le_bytes(header[4..8].try_into().unwrap());
        if len == 0 || len > 1024 * 1024 {
            break;
        }
        let mut payload = vec![0u8; len as usize];
        if file.read_exact(&mut payload).is_err() {
            break;
        }
        if crc32c(&payload) != crc {
            break;
        }
        pos = pos.saturating_add(8 + len);
    }
    file.set_len(pos)?;
    file.seek(SeekFrom::End(0))?;
    Ok(())
}

fn create_new_segment(config: &Config, stream_id: Uuid, start_seq: u64) -> Result<(SegmentWriter, u64)> {
    let path = segment_open_path(&config.spool_dir, stream_id, start_seq);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("create {}", path.display()))?;
    write_segment_header(&mut file, stream_id, start_seq)?;
    file.sync_data().ok();
    Ok((
        SegmentWriter {
            path,
            file,
            start_seq,
            created_at: Instant::now(),
        },
        SEGMENT_HEADER_LEN as u64,
    ))
}

fn write_segment_header(file: &mut fs::File, stream_id: Uuid, start_seq: u64) -> Result<()> {
    let created_wall_ms = Utc::now().timestamp_millis();
    let mut header = vec![0u8; SEGMENT_HEADER_LEN];
    header[0..8].copy_from_slice(SEGMENT_MAGIC);
    header[8..12].copy_from_slice(&SEGMENT_VERSION.to_le_bytes());
    header[12..16].copy_from_slice(&(SEGMENT_HEADER_LEN as u32).to_le_bytes());
    header[16..32].copy_from_slice(stream_id.as_bytes());
    header[32..40].copy_from_slice(&(created_wall_ms as i64).to_le_bytes());
    header[40..48].copy_from_slice(&start_seq.to_le_bytes());
    file.write_all(&header)?;
    Ok(())
}

impl SpoolRuntime {
    fn append_samples(&mut self, samples: Vec<IncomingSample>) -> Result<AppendResult> {
        if samples.is_empty() {
            return Ok(AppendResult {
                accepted: 0,
            });
        }

        let mut published = Vec::with_capacity(samples.len());
        for sample in samples {
            let sensor_id = sample.sensor_id.trim();
            if sensor_id.is_empty() {
                continue;
            }
            let sensor_idx = self.sensor_map.get_or_insert(sensor_id);
            if sensor_idx + 1 == self.sensor_map.next_index {
                let _ = persist_sensor_map(&self.sensor_map_path, &self.sensor_map);
            }

            let seq = self.next_seq;
            self.next_seq = self.next_seq.saturating_add(1);

            let time_quality = sample.time_quality.unwrap_or(TimeQuality::Unknown);
            let mono_ms = monotonic_ms();

            let payload = encode_sample_record(
                sensor_idx,
                seq,
                sample.timestamp_ms,
                sample.value,
                sample.quality,
                time_quality,
                mono_ms,
            );
            let len = payload.len() as u32;
            let crc = crc32c(&payload);
            self.segment.file.write_all(&len.to_le_bytes())?;
            self.segment.file.write_all(&crc.to_le_bytes())?;
            self.segment.file.write_all(&payload)?;
            self.spool_bytes = self.spool_bytes.saturating_add(8 + payload.len() as u64);

            published.push(PublishSample {
                sensor_id: sensor_id.to_string(),
                timestamp_ms: sample.timestamp_ms,
                value: sample.value,
                quality: sample.quality,
                seq,
                stream_id: self.stream_id,
                time_quality,
                monotonic_ms: mono_ms,
            });

            if self.segment.created_at.elapsed() >= self.config.segment_roll_duration
                || self.segment.file.metadata()?.len() >= self.config.segment_roll_bytes
            {
                self.roll_segment(seq)?;
            }

            if self.last_sync_at.elapsed() >= self.config.sync_interval {
                self.segment.file.sync_data().ok();
                self.last_sync_at = Instant::now();
                let _ = self.persist_state();
            }
        }

        // Best-effort publish of live samples (bounded by channel capacity).
        for item in &published {
            if self.publish_tx.try_send(item.clone()).is_err() {
                break;
            }
        }

        self.enforce_caps()?;
        self.persist_state()?;

        Ok(AppendResult {
            accepted: published.len() as u64,
        })
    }

    fn roll_segment(&mut self, last_seq: u64) -> Result<()> {
        // Close current .open segment as .seg.
        self.segment.file.sync_data().ok();
        let start_seq = self.segment.start_seq;
        let closed_path = segment_closed_path(&self.config.spool_dir, self.stream_id, start_seq, last_seq);
        fs::rename(&self.segment.path, &closed_path)
            .with_context(|| format!("rename {} -> {}", self.segment.path.display(), closed_path.display()))?;

        // Create new open segment at next_seq.
        let (new_seg, added_bytes) = create_new_segment(&self.config, self.stream_id, self.next_seq)?;
        self.spool_bytes = self.spool_bytes.saturating_add(added_bytes);
        self.segment = new_seg;
        Ok(())
    }

    fn delete_acked_segments(&mut self) -> Result<()> {
        let mut deleted_bytes = 0u64;
        let prefix = format!("seg-{}-", self.stream_id);
        for entry in fs::read_dir(&self.config.spool_dir).context("read spool dir")? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if !name.starts_with(&prefix) || !name.ends_with(".seg") {
                continue;
            }
            if let Some((_start, end)) = parse_closed_segment_range(name) {
                if end <= self.acked_seq {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    fs::remove_file(&path).ok();
                    deleted_bytes = deleted_bytes.saturating_add(size);
                }
            }
        }
        self.spool_bytes = self.spool_bytes.saturating_sub(deleted_bytes);
        Ok(())
    }

    fn enforce_caps(&mut self) -> Result<()> {
        let max_bytes = self.config.max_spool_bytes;
        let free_bytes = compute_free_bytes(&self.config.spool_dir).ok();
        let needs_free_space = free_bytes
            .map(|free| free < self.config.keep_free_bytes)
            .unwrap_or(false);

        if self.spool_bytes <= max_bytes && self.config.max_spool_age.is_none() && !needs_free_space {
            return Ok(());
        }

        let mut segments = list_closed_segments(&self.config.spool_dir, self.stream_id)?;
        segments.sort_by_key(|seg| seg.start_seq);

        let mut now = Utc::now();
        while self.spool_bytes > max_bytes
            || compute_free_bytes(&self.config.spool_dir)
                .map(|free| free < self.config.keep_free_bytes)
                .unwrap_or(false)
        {
            let Some(seg) = segments.first().cloned() else {
                break;
            };
            segments.remove(0);
            self.drop_segment(seg, now)?;
            now = Utc::now();
        }

        if let Some(max_age) = self.config.max_spool_age {
            let cutoff = now - chrono::Duration::from_std(max_age).unwrap_or_else(|_| chrono::Duration::hours(72));
            for seg in segments {
                if seg.created_at < cutoff {
                    self.drop_segment(seg, now)?;
                }
            }
        }

        Ok(())
    }

    fn drop_segment(&mut self, seg: ClosedSegment, now: DateTime<Utc>) -> Result<()> {
        let size = seg.size_bytes;
        fs::remove_file(&seg.path).ok();
        self.spool_bytes = self.spool_bytes.saturating_sub(size);

        if seg.end_seq > self.acked_seq {
            let loss = LossRange {
                start_seq: seg.start_seq,
                end_seq: seg.end_seq,
                dropped_at: now.to_rfc3339(),
            };
            self.losses.push(loss);
            if let Some(last) = self.losses.last().cloned() {
                let _ = self.loss_tx.try_send(LossEvent {
                    stream_id: self.stream_id,
                    range: last,
                });
            }
        }
        Ok(())
    }

    fn prune_losses(&mut self) {
        // Once the controller ACK has advanced beyond a loss range, it must have accepted the skip.
        self.losses.retain(|loss| loss.end_seq > self.acked_seq);
    }

    fn persist_state(&self) -> Result<()> {
        persist_state(
            &self.state_path,
            self.stream_id,
            self.next_seq,
            self.acked_seq,
            Some(self.segment.start_seq),
            &self.losses,
        )
    }

    fn status(&self) -> SpoolStatus {
        let open_segment_bytes = self
            .segment
            .file
            .metadata()
            .map(|m| m.len())
            .unwrap_or(0);
        let closed_segments = list_closed_segments(&self.config.spool_dir, self.stream_id)
            .map(|v| v.len())
            .unwrap_or(0);
        let oldest_unacked_timestamp_ms =
            find_oldest_unacked_timestamp_ms(&self.config.spool_dir, self.stream_id, self.acked_seq)
                .unwrap_or(None);
        let free_bytes = compute_free_bytes(&self.config.spool_dir).ok();
        let backlog_samples = self
            .next_seq
            .saturating_sub(self.acked_seq.saturating_add(1));
        let estimated_drain_seconds = if backlog_samples > 0 && self.config.replay_msgs_per_sec > 0 {
            Some(
                backlog_samples
                    .saturating_add(self.config.replay_msgs_per_sec as u64 - 1)
                    / self.config.replay_msgs_per_sec as u64,
            )
        } else {
            None
        };
        let mut losses = self.losses.clone();
        const LOSS_STATUS_LIMIT: usize = 20;
        if losses.len() > LOSS_STATUS_LIMIT {
            losses = losses.split_off(losses.len().saturating_sub(LOSS_STATUS_LIMIT));
        }
        SpoolStatus {
            stream_id: self.stream_id.to_string(),
            next_seq: self.next_seq,
            acked_seq: self.acked_seq,
            spool_bytes: self.spool_bytes,
            max_spool_bytes: self.config.max_spool_bytes,
            keep_free_bytes: self.config.keep_free_bytes,
            free_bytes,
            closed_segments,
            open_segment_start_seq: Some(self.segment.start_seq),
            open_segment_bytes,
            backlog_samples,
            replay_msgs_per_sec: self.config.replay_msgs_per_sec,
            replay_bytes_per_sec: self.config.replay_bytes_per_sec,
            estimated_drain_seconds,
            losses_pending: self.losses.len(),
            losses,
            oldest_unacked_timestamp_ms,
        }
    }
}

#[derive(Debug, Clone)]
struct ClosedSegment {
    path: PathBuf,
    start_seq: u64,
    end_seq: u64,
    created_at: DateTime<Utc>,
    size_bytes: u64,
}

fn list_closed_segments(spool_dir: &Path, stream_id: Uuid) -> Result<Vec<ClosedSegment>> {
    let mut out = Vec::new();
    let prefix = format!("seg-{}-", stream_id);
    for entry in fs::read_dir(spool_dir).context("read spool dir")? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
            continue;
        };
        if !name.starts_with(&prefix) || !name.ends_with(".seg") {
            continue;
        }
        let Some((start_seq, end_seq)) = parse_closed_segment_range(name) else {
            continue;
        };
        let meta = entry.metadata()?;
        let created_at = meta
            .modified()
            .ok()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(Utc::now);
        out.push(ClosedSegment {
            path,
            start_seq,
            end_seq,
            created_at,
            size_bytes: meta.len(),
        });
    }
    Ok(out)
}

fn parse_closed_segment_range(name: &str) -> Option<(u64, u64)> {
    // seg-<uuid>-<start>-<end>.seg
    let trimmed = name.trim_end_matches(".seg");
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() < 4 {
        return None;
    }
    let start = parts.get(parts.len() - 2)?.parse().ok()?;
    let end = parts.get(parts.len() - 1)?.parse().ok()?;
    Some((start, end))
}

fn encode_sample_record(
    sensor_idx: u32,
    seq: u64,
    timestamp_ms: i64,
    value: f64,
    quality: i16,
    time_quality: TimeQuality,
    monotonic_ms: u64,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(SAMPLE_RECORD_LEN);
    buf.extend_from_slice(&sensor_idx.to_le_bytes());
    buf.extend_from_slice(&seq.to_le_bytes());
    buf.extend_from_slice(&timestamp_ms.to_le_bytes());
    buf.extend_from_slice(&value.to_le_bytes());
    buf.extend_from_slice(&quality.to_le_bytes());
    buf.extend_from_slice(&time_quality.as_flag_bits().to_le_bytes());
    buf.extend_from_slice(&monotonic_ms.to_le_bytes());
    buf
}

#[derive(Debug, Clone)]
struct SampleRecord {
    seq: u64,
    timestamp_ms: i64,
}

fn decode_sample_record(buf: &[u8]) -> Option<SampleRecord> {
    if buf.len() != SAMPLE_RECORD_LEN {
        return None;
    }
    let seq = u64::from_le_bytes(buf[4..12].try_into().ok()?);
    let timestamp_ms = i64::from_le_bytes(buf[12..20].try_into().ok()?);
    Some(SampleRecord {
        seq,
        timestamp_ms,
    })
}

fn monotonic_ms() -> u64 {
    #[cfg(target_os = "linux")]
    unsafe {
        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        if libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) != 0 {
            return 0;
        }
        let secs = ts.tv_sec.max(0) as u64;
        let nanos = ts.tv_nsec.max(0) as u64;
        secs.saturating_mul(1000) + nanos / 1_000_000
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

fn find_oldest_unacked_timestamp_ms(
    spool_dir: &Path,
    stream_id: Uuid,
    acked_seq: u64,
) -> Result<Option<i64>> {
    let mut paths: Vec<(u64, PathBuf)> = Vec::new();

    for seg in list_closed_segments(spool_dir, stream_id).unwrap_or_default() {
        if seg.end_seq <= acked_seq {
            continue;
        }
        paths.push((seg.start_seq, seg.path));
    }

    if let Some(open_path) = find_any_open_segment(spool_dir, stream_id)? {
        let start_seq = parse_open_start_seq(&open_path).unwrap_or(0);
        paths.push((start_seq, open_path));
    }

    paths.sort_by_key(|(start, _)| *start);

    for (_start, path) in paths {
        let mut file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(_) => continue,
        };
        if file.metadata().map(|m| m.len()).unwrap_or(0) < SEGMENT_HEADER_LEN as u64 {
            continue;
        }
        file.seek(SeekFrom::Start(SEGMENT_HEADER_LEN as u64))?;
        loop {
            let Some(payload) = read_next_frame_payload(&mut file)? else {
                break;
            };
            let Some(record) = decode_sample_record(&payload) else {
                break;
            };
            if record.seq > acked_seq {
                return Ok(Some(record.timestamp_ms));
            }
        }
    }

    Ok(None)
}

fn read_next_frame_payload(file: &mut fs::File) -> Result<Option<Vec<u8>>> {
    let mut header = [0u8; 8];
    match file.read_exact(&mut header) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    let len = u32::from_le_bytes(header[0..4].try_into().unwrap()) as usize;
    let crc = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if len == 0 || len > 1024 * 1024 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(spool_dir: &Path) -> Config {
        Config {
            node_id: "test-node".to_string(),
            mqtt_host: "127.0.0.1".to_string(),
            mqtt_port: 1883,
            mqtt_username: None,
            mqtt_password: None,
            mqtt_topic_prefix: "iot".to_string(),
            mqtt_client_id: "node-forwarder-test".to_string(),
            http_bind: "127.0.0.1:0".to_string(),
            spool_dir: spool_dir.to_path_buf(),
            segment_roll_duration: std::time::Duration::from_secs(3600),
            segment_roll_bytes: 128 * 1024 * 1024,
            sync_interval: std::time::Duration::from_secs(3600),
            max_spool_bytes: 1024 * 1024 * 1024,
            keep_free_bytes: 0,
            max_spool_age: None,
            replay_msgs_per_sec: 2000,
            replay_bytes_per_sec: 10 * 1024 * 1024,
        }
    }

    #[test]
    fn recover_truncate_tail_truncates_partial_frame() {
        let dir = TempDir::new().unwrap();
        let config = test_config(dir.path());
        let stream_id = Uuid::new_v4();
        let start_seq = 1u64;
        let path = segment_open_path(&config.spool_dir, stream_id, start_seq);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();

        write_segment_header(&mut file, stream_id, start_seq).unwrap();

        let payload = encode_sample_record(1, 1, 1000, 1.0, 0, TimeQuality::Good, 1);
        let len = payload.len() as u32;
        let crc = crc32c(&payload);
        file.write_all(&len.to_le_bytes()).unwrap();
        file.write_all(&crc.to_le_bytes()).unwrap();
        file.write_all(&payload).unwrap();

        file.write_all(&len.to_le_bytes()).unwrap();
        file.flush().unwrap();

        drop(file);
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        recover_truncate_tail(&mut file).unwrap();
        let after = file.metadata().unwrap().len();

        let expected = SEGMENT_HEADER_LEN as u64 + 8 + payload.len() as u64;
        assert_eq!(after, expected);
    }

    #[test]
    fn delete_acked_segments_removes_closed_segments() {
        let dir = TempDir::new().unwrap();
        let mut config = test_config(dir.path());
        config.max_spool_bytes = 10 * 1024 * 1024;

        fs::create_dir_all(&config.spool_dir).unwrap();
        let stream_id = Uuid::new_v4();

        let (segment, _) = create_new_segment(&config, stream_id, 1).unwrap();

        let seg1 = segment_closed_path(&config.spool_dir, stream_id, 1, 10);
        fs::write(&seg1, vec![0u8; 1024]).unwrap();
        let seg2 = segment_closed_path(&config.spool_dir, stream_id, 11, 20);
        fs::write(&seg2, vec![0u8; 1024]).unwrap();

        let mut runtime = SpoolRuntime {
            config,
            state_path: dir.path().join("state.json"),
            sensor_map_path: dir.path().join("sensor_map.json"),
            stream_id,
            next_seq: 21,
            acked_seq: 10,
            losses: Vec::new(),
            sensor_map: SensorMap::empty(),
            spool_bytes: 2048,
            segment,
            last_sync_at: Instant::now(),
            publish_tx: mpsc::channel(1).0,
            loss_tx: mpsc::channel(1).0,
        };

        runtime.delete_acked_segments().unwrap();

        assert!(!seg1.exists());
        assert!(seg2.exists());
    }

    #[test]
    fn enforce_caps_drops_oldest_and_records_loss() {
        let dir = TempDir::new().unwrap();
        let mut config = test_config(dir.path());
        config.max_spool_bytes = 1500;

        fs::create_dir_all(&config.spool_dir).unwrap();
        let stream_id = Uuid::new_v4();
        let (segment, _) = create_new_segment(&config, stream_id, 1).unwrap();

        let seg1 = segment_closed_path(&config.spool_dir, stream_id, 1, 10);
        fs::write(&seg1, vec![0u8; 1000]).unwrap();
        let seg2 = segment_closed_path(&config.spool_dir, stream_id, 11, 20);
        fs::write(&seg2, vec![0u8; 1000]).unwrap();

        let mut runtime = SpoolRuntime {
            config,
            state_path: dir.path().join("state.json"),
            sensor_map_path: dir.path().join("sensor_map.json"),
            stream_id,
            next_seq: 21,
            acked_seq: 0,
            losses: Vec::new(),
            sensor_map: SensorMap::empty(),
            spool_bytes: 2000,
            segment,
            last_sync_at: Instant::now(),
            publish_tx: mpsc::channel(1).0,
            loss_tx: mpsc::channel(1).0,
        };

        runtime.enforce_caps().unwrap();

        assert!(!seg1.exists());
        assert!(runtime.losses.iter().any(|loss| loss.start_seq == 1 && loss.end_seq == 10));
    }
}
