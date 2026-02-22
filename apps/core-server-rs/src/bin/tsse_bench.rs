use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use clap::Parser;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration as StdDuration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

#[derive(Debug, Parser)]
#[command(
    about = "TSSE benchmark harness (runs real analysis jobs against a controller API and writes a report)."
)]
struct Args {
    /// Base URL for the core-server API (installed controller is typically http://127.0.0.1:8000)
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    base_url: String,

    /// API token string (Bearer). Prefer --auth-token-file.
    #[arg(long)]
    auth_token: Option<String>,

    /// File containing a single-line API token string (Bearer).
    #[arg(long)]
    auth_token_file: Option<PathBuf>,

    /// Focus sensor id for related_sensors_v1. If omitted, the harness will try to pick the first sensor.
    #[arg(long)]
    focus_sensor_id: Option<String>,

    /// RFC3339 start timestamp. Default: now - 24h.
    #[arg(long)]
    start: Option<String>,

    /// RFC3339 end timestamp (inclusive). Default: now.
    #[arg(long)]
    end: Option<String>,

    /// Bucket interval (seconds) used by the job.
    #[arg(long, default_value_t = 60)]
    interval_seconds: i64,

    /// Candidate limit for related_sensors_v1 (higher increases work).
    #[arg(long, default_value_t = 150)]
    candidate_limit: u32,

    /// Minimum ANN pool size for candidate gen widening.
    #[arg(long, default_value_t = 150)]
    min_pool: u32,

    /// Require same-unit candidates (recommended for stable benchmarks).
    #[arg(long, default_value_t = true)]
    same_unit_only: bool,

    /// Number of runs to collect for p50/p95.
    #[arg(long, default_value_t = 7)]
    runs: usize,

    /// Output report path (Markdown). Recommended under reports/.
    #[arg(long)]
    report: PathBuf,

    /// Skip preview endpoint benchmarks (only run the job itself).
    #[arg(long)]
    skip_preview: bool,

    /// Enable server-side profiling for each TSSE job run.
    #[arg(long, default_value_t = false)]
    profile: bool,

    /// Output directory for profiling artifacts on the controller (optional).
    #[arg(long)]
    profile_output_dir: Option<PathBuf>,

    /// Target p50 latency (ms) for the end-to-end related_sensors_v1 job (optional gate).
    #[arg(long)]
    job_p50_target_ms: Option<u64>,

    /// Target p95 latency (ms) for the end-to-end related_sensors_v1 job (optional gate).
    #[arg(long)]
    job_p95_target_ms: Option<u64>,

    /// Target p50 scoring throughput (candidates/sec) for exact scoring (optional gate).
    #[arg(long)]
    scoring_throughput_p50_target: Option<f64>,

    /// Target p95 scoring throughput (candidates/sec) for exact scoring (optional gate).
    #[arg(long)]
    scoring_throughput_p95_target: Option<f64>,

    /// PID of the process to monitor for resource budgets (overrides process name matching).
    #[arg(long)]
    resource_pid: Option<u32>,

    /// Process name(s) to monitor for resource budgets (comma-separated).
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "core-server-rs,core-server"
    )]
    resource_process_name: Vec<String>,

    /// Resource sample interval (ms) while jobs are running.
    #[arg(long, default_value_t = 500)]
    resource_sample_ms: u64,

    /// CPU peak percent target for the monitored process (optional gate).
    #[arg(long)]
    resource_cpu_peak_target: Option<f32>,

    /// RAM peak target for the monitored process (MB, optional gate).
    #[arg(long)]
    resource_ram_peak_mb_target: Option<u64>,

    /// Disk read target for the monitored process (MB, optional gate).
    #[arg(long)]
    resource_disk_read_mb_target: Option<u64>,

    /// Disk write target for the monitored process (MB, optional gate).
    #[arg(long)]
    resource_disk_write_mb_target: Option<u64>,

    /// Optional JSON file containing threshold targets (fields mirror CLI flag names).
    #[arg(long)]
    targets_file: Option<PathBuf>,

    /// Fail if any throughput/job/resource targets are missing after applying targets_file.
    #[arg(long, default_value_t = false)]
    require_targets: bool,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct BenchTargets {
    job_p50_target_ms: Option<u64>,
    job_p95_target_ms: Option<u64>,
    scoring_throughput_p50_target: Option<f64>,
    scoring_throughput_p95_target: Option<f64>,
    resource_cpu_peak_target: Option<f32>,
    resource_ram_peak_mb_target: Option<u64>,
    resource_disk_read_mb_target: Option<u64>,
    resource_disk_write_mb_target: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SensorsResponse {
    sensors: Vec<SensorRow>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct SensorRow {
    sensor_id: String,
    #[serde(default)]
    deleted_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AnalysisJobCreateResponse {
    job: AnalysisJobPublic,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AnalysisJobPublic {
    id: String,
    status: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AnalysisJobStatusResponse {
    job: AnalysisJobPublic,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AnalysisJobResultResponse {
    result: serde_json::Value,
}

#[derive(Debug, Clone)]
struct ResourceUsage {
    process_label: String,
    peak_cpu_pct: Option<f32>,
    peak_rss_bytes: Option<u64>,
    disk_read_bytes: Option<u64>,
    disk_write_bytes: Option<u64>,
}

struct ResourceMonitor {
    pid: Pid,
    pid_raw: u32,
    label: String,
    sample_every: StdDuration,
    last_sample: Instant,
    sys: System,
    peak_cpu_pct: f32,
    peak_rss_bytes: u64,
    start_disk: Option<(u64, u64)>,
    last_disk: Option<(u64, u64)>,
    active: bool,
}

impl ResourceMonitor {
    fn new(pid: Pid, label: String, sample_every: StdDuration) -> Self {
        let refresh = RefreshKind::nothing().with_processes(
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage(),
        );
        let mut sys = System::new_with_specifics(refresh);
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let now = Instant::now();
        Self {
            pid,
            pid_raw: pid.as_u32(),
            label,
            sample_every,
            last_sample: now - sample_every,
            sys,
            peak_cpu_pct: 0.0,
            peak_rss_bytes: 0,
            start_disk: None,
            last_disk: None,
            active: true,
        }
    }

    fn sample(&mut self) {
        if !self.active {
            return;
        }
        if self.last_sample.elapsed() < self.sample_every {
            return;
        }
        self.last_sample = Instant::now();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage(),
        );
        if let Some(process) = self.sys.process(self.pid) {
            let cpu = process.cpu_usage();
            if cpu.is_finite() {
                self.peak_cpu_pct = self.peak_cpu_pct.max(cpu);
            }
            let rss_bytes = process.memory().saturating_mul(1024);
            self.peak_rss_bytes = self.peak_rss_bytes.max(rss_bytes);

            let disk = process.disk_usage();
            let read_bytes = disk.read_bytes;
            let write_bytes = disk.written_bytes;
            if self.start_disk.is_none() {
                self.start_disk = Some((read_bytes, write_bytes));
            }
            self.last_disk = Some((read_bytes, write_bytes));
        }

        // macOS: sysinfo process stats can be unavailable for other service users.
        // Fall back to `ps` for CPU/RSS so we can still record resource peaks.
        if let Some((cpu_pct, rss_bytes)) = sample_ps_cpu_rss(self.pid_raw) {
            if cpu_pct.is_finite() {
                self.peak_cpu_pct = self.peak_cpu_pct.max(cpu_pct);
            }
            self.peak_rss_bytes = self.peak_rss_bytes.max(rss_bytes);
        }
    }

    fn finish(&self) -> Option<ResourceUsage> {
        if !self.active && self.start_disk.is_none() && self.peak_rss_bytes == 0 {
            return None;
        }
        let (disk_read_bytes, disk_write_bytes) = self
            .start_disk
            .zip(self.last_disk)
            .map(|(start, end)| (end.0.saturating_sub(start.0), end.1.saturating_sub(start.1)))
            .unwrap_or((0, 0));
        Some(ResourceUsage {
            process_label: self.label.clone(),
            peak_cpu_pct: if self.peak_cpu_pct > 0.0 {
                Some(self.peak_cpu_pct)
            } else {
                None
            },
            peak_rss_bytes: if self.peak_rss_bytes > 0 {
                Some(self.peak_rss_bytes)
            } else {
                None
            },
            disk_read_bytes: if disk_read_bytes > 0 {
                Some(disk_read_bytes)
            } else {
                None
            },
            disk_write_bytes: if disk_write_bytes > 0 {
                Some(disk_write_bytes)
            } else {
                None
            },
        })
    }
}

fn read_token(args: &Args) -> Result<String> {
    if let Some(token) = args
        .auth_token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        return Ok(token.to_string());
    }
    if let Some(path) = args.auth_token_file.as_ref() {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let token = raw.lines().next().unwrap_or("").trim();
        anyhow::ensure!(
            !token.is_empty(),
            "auth token file is empty: {}",
            path.display()
        );
        return Ok(token.to_string());
    }
    anyhow::bail!("missing auth token (provide --auth-token or --auth-token-file)");
}

fn headers(token: &str) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", token)).context("invalid auth token")?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    Ok(headers)
}

fn load_targets(path: &PathBuf) -> Result<BenchTargets> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read targets file {}", path.display()))?;
    let targets: BenchTargets = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse targets file {}", path.display()))?;
    Ok(targets)
}

fn apply_targets(args: &mut Args, targets: BenchTargets) {
    if args.job_p50_target_ms.is_none() {
        args.job_p50_target_ms = targets.job_p50_target_ms;
    }
    if args.job_p95_target_ms.is_none() {
        args.job_p95_target_ms = targets.job_p95_target_ms;
    }
    if args.scoring_throughput_p50_target.is_none() {
        args.scoring_throughput_p50_target = targets.scoring_throughput_p50_target;
    }
    if args.scoring_throughput_p95_target.is_none() {
        args.scoring_throughput_p95_target = targets.scoring_throughput_p95_target;
    }
    if args.resource_cpu_peak_target.is_none() {
        args.resource_cpu_peak_target = targets.resource_cpu_peak_target;
    }
    if args.resource_ram_peak_mb_target.is_none() {
        args.resource_ram_peak_mb_target = targets.resource_ram_peak_mb_target;
    }
    if args.resource_disk_read_mb_target.is_none() {
        args.resource_disk_read_mb_target = targets.resource_disk_read_mb_target;
    }
    if args.resource_disk_write_mb_target.is_none() {
        args.resource_disk_write_mb_target = targets.resource_disk_write_mb_target;
    }
}

fn ensure_required_targets(args: &Args) -> Result<()> {
    if !args.require_targets {
        return Ok(());
    }
    let mut missing: Vec<&str> = Vec::new();
    if args.job_p50_target_ms.is_none() {
        missing.push("--job-p50-target-ms");
    }
    if args.job_p95_target_ms.is_none() {
        missing.push("--job-p95-target-ms");
    }
    if args.scoring_throughput_p50_target.is_none() {
        missing.push("--scoring-throughput-p50-target");
    }
    if args.scoring_throughput_p95_target.is_none() {
        missing.push("--scoring-throughput-p95-target");
    }
    if args.resource_cpu_peak_target.is_none() {
        missing.push("--resource-cpu-peak-target");
    }
    if args.resource_ram_peak_mb_target.is_none() {
        missing.push("--resource-ram-peak-mb-target");
    }
    if args.resource_disk_read_mb_target.is_none() {
        missing.push("--resource-disk-read-mb-target");
    }
    if args.resource_disk_write_mb_target.is_none() {
        missing.push("--resource-disk-write-mb-target");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("missing required targets: {}", missing.join(", "))
    }
}

fn parse_ts(label: &str, value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim())
        .with_context(|| format!("invalid {} timestamp: {}", label, value))?
        .with_timezone(&Utc)
        .pipe(Ok)
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}

fn percentile_ms(samples: &[u64], pct: f64) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let pct = pct.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted.get(idx).copied()
}

fn percentile_f64(samples: &[f64], pct: f64) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = pct.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted.get(idx).copied()
}

fn bytes_to_mb(bytes: u64) -> f64 {
    (bytes as f64) / (1024.0 * 1024.0)
}

fn fmt_opt_u64(opt: Option<u64>) -> String {
    opt.map(|v| v.to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn fmt_opt_f64(opt: Option<f64>, decimals: usize) -> String {
    opt.map(|v| format!("{v:.precision$}", precision = decimals))
        .unwrap_or_else(|| "—".to_string())
}

fn fmt_pass(pass: Option<bool>) -> String {
    match pass {
        Some(true) => "PASS".to_string(),
        Some(false) => "FAIL".to_string(),
        None => "N/A".to_string(),
    }
}

fn gate_upper_u64(value: Option<u64>, target: Option<u64>) -> Option<bool> {
    match target {
        Some(target) => Some(value.map(|v| v <= target).unwrap_or(false)),
        None => None,
    }
}

fn gate_lower_f64(value: Option<f64>, target: Option<f64>) -> Option<bool> {
    match target {
        Some(target) => Some(value.map(|v| v >= target).unwrap_or(false)),
        None => None,
    }
}

fn resolve_resource_monitor(args: &Args) -> Option<ResourceMonitor> {
    if let Some(pid_raw) = args.resource_pid {
        let pid = Pid::from_u32(pid_raw);
        return Some(ResourceMonitor::new(
            pid,
            format!("pid {pid_raw}"),
            StdDuration::from_millis(args.resource_sample_ms.max(100)),
        ));
    }

    let refresh = RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing());
    let mut sys = System::new_with_specifics(refresh);
    sys.refresh_processes(ProcessesToUpdate::All, true);

    for name in &args.resource_process_name {
        let needle = name.trim();
        if needle.is_empty() {
            continue;
        }
        if let Some((pid, process)) = sys.processes().iter().find(|(_, process)| {
            process
                .name()
                .to_string_lossy()
                .eq_ignore_ascii_case(needle)
        }) {
            return Some(ResourceMonitor::new(
                *pid,
                process.name().to_string_lossy().to_string(),
                StdDuration::from_millis(args.resource_sample_ms.max(100)),
            ));
        }
    }

    if let Some((pid_raw, label)) = resolve_resource_pid_via_ps(&args.resource_process_name) {
        return Some(ResourceMonitor::new(
            Pid::from_u32(pid_raw),
            label,
            StdDuration::from_millis(args.resource_sample_ms.max(100)),
        ));
    }

    None
}

fn sample_ps_cpu_rss(pid: u32) -> Option<(f32, u64)> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "%cpu=", "-o", "rss="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut cpu_pct: Option<f32> = None;
    let mut rss_kb: Option<u64> = None;
    for token in stdout.split_whitespace() {
        if cpu_pct.is_none() {
            cpu_pct = token.parse::<f32>().ok();
            continue;
        }
        if rss_kb.is_none() {
            rss_kb = token.parse::<u64>().ok();
            continue;
        }
    }
    Some((cpu_pct?, rss_kb?.saturating_mul(1024)))
}

fn resolve_resource_pid_via_ps(names: &[String]) -> Option<(u32, String)> {
    let needles: Vec<String> = names
        .iter()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .collect();
    if needles.is_empty() {
        return None;
    }

    let output = Command::new("ps")
        .args(["-A", "-o", "pid=", "-o", "comm="])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut best: Option<(u32, String)> = None;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let Some(pid_str) = parts.next() else {
            continue;
        };
        let Some(pid_raw) = pid_str.parse::<u32>().ok() else {
            continue;
        };
        let comm = parts.next().unwrap_or("").to_string();
        if comm.is_empty() {
            continue;
        }
        let comm_lower = comm.to_lowercase();
        let matched = needles
            .iter()
            .any(|needle| comm_lower == *needle || comm_lower.contains(needle.as_str()));
        if !matched {
            continue;
        }
        if best
            .as_ref()
            .map(|(existing_pid, _)| *existing_pid >= pid_raw)
            == Some(true)
        {
            continue;
        }
        best = Some((pid_raw, comm));
    }
    best
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = Args::parse();
    if let Some(path) = args.targets_file.as_ref() {
        let targets = load_targets(path)?;
        apply_targets(&mut args, targets);
    }
    ensure_required_targets(&args)?;
    let token = read_token(&args)?;
    let base_url = args.base_url.trim_end_matches('/').to_string();
    let http = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()?;
    let headers = headers(&token)?;

    let end_inclusive = args
        .end
        .as_deref()
        .map(|v| parse_ts("end", v))
        .transpose()?
        .unwrap_or_else(Utc::now);
    let start = args
        .start
        .as_deref()
        .map(|v| parse_ts("start", v))
        .transpose()?
        .unwrap_or_else(|| end_inclusive - Duration::hours(24));
    anyhow::ensure!(end_inclusive > start, "end must be after start");

    let focus_sensor_id = if let Some(id) = args
        .focus_sensor_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        id.to_string()
    } else {
        let url = format!("{}/api/sensors", base_url);
        let resp = http
            .get(url)
            .headers(headers.clone())
            .send()
            .await?
            .error_for_status()?;
        let parsed: SensorsResponse = resp.json().await.context("failed to parse /api/sensors")?;
        let picked = parsed
            .sensors
            .into_iter()
            .find(|s| s.deleted_at.is_none())
            .map(|s| s.sensor_id)
            .context("no sensors available for benchmarking; pass --focus-sensor-id")?;
        picked
    };

    let mut resource_monitor = resolve_resource_monitor(&args);
    if let Some(monitor) = resource_monitor.as_mut() {
        monitor.sample();
    } else if args.resource_pid.is_some() || !args.resource_process_name.is_empty() {
        println!(
            "tsse_bench: resource monitor disabled (process not found). Use --resource-pid to override."
        );
    }

    let mut job_wall_ms: Vec<u64> = Vec::new();
    let mut candidate_gen_ms: Vec<u64> = Vec::new();
    let mut qdrant_search_ms: Vec<u64> = Vec::new();
    let mut scoring_ms: Vec<u64> = Vec::new();
    let mut duckdb_candidate_ms: Vec<u64> = Vec::new();
    let mut episode_extract_ms: Vec<u64> = Vec::new();
    let mut exact_stage_ms: Vec<u64> = Vec::new();
    let mut scoring_throughput: Vec<f64> = Vec::new();
    let mut preview_wall_ms: Vec<u64> = Vec::new();
    let mut effective_intervals: Vec<u64> = Vec::new();

    for run_idx in 0..args.runs {
        println!(
            "tsse_bench: run {}/{} (related_sensors_v1)",
            run_idx + 1,
            args.runs
        );

        let mut params = json!({
            "focus_sensor_id": focus_sensor_id,
            "start": start.to_rfc3339(),
            "end": end_inclusive.to_rfc3339(),
            "interval_seconds": args.interval_seconds,
            "candidate_limit": args.candidate_limit,
            "min_pool": args.min_pool,
            "lag_max_seconds": 0,
            "filters": {
                "same_node_only": false,
                "same_unit_only": args.same_unit_only,
                "same_type_only": false,
                "exclude_sensor_ids": [],
            }
        });
        if args.profile {
            params["profile"] = json!(true);
        }
        if let Some(dir) = args.profile_output_dir.as_ref() {
            let value = dir.to_string_lossy();
            if !value.is_empty() {
                params["profile_output_dir"] = json!(value.as_ref());
            }
        }

        let create_body = json!({
            "job_type": "related_sensors_v1",
            "params": params,
            "dedupe": false
        });

        let create_started = Instant::now();
        let created: AnalysisJobCreateResponse = http
            .post(format!("{}/api/analysis/jobs", base_url))
            .headers(headers.clone())
            .json(&create_body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("failed to parse create_job response")?;
        let job_id = created.job.id;

        // Poll for completion.
        let poll_started = Instant::now();
        loop {
            if let Some(monitor) = resource_monitor.as_mut() {
                monitor.sample();
            }
            let status: AnalysisJobStatusResponse = http
                .get(format!("{}/api/analysis/jobs/{}", base_url, job_id))
                .headers(headers.clone())
                .send()
                .await?
                .error_for_status()?
                .json()
                .await
                .context("failed to parse get_job response")?;
            match status.job.status.as_str() {
                "completed" => break,
                "failed" => anyhow::bail!("job failed (job_id={})", job_id),
                "canceled" => anyhow::bail!("job canceled (job_id={})", job_id),
                _ => {}
            }
            tokio::time::sleep(StdDuration::from_millis(300)).await;
        }

        let job_total_ms = create_started.elapsed().as_millis() as u64;
        job_wall_ms.push(job_total_ms);

        let result: AnalysisJobResultResponse = http
            .get(format!("{}/api/analysis/jobs/{}/result", base_url, job_id))
            .headers(headers.clone())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("failed to parse job result response")?;

        if let Some(interval) = result
            .result
            .get("params")
            .and_then(|v| v.get("interval_seconds"))
            .and_then(|v| v.as_u64())
        {
            effective_intervals.push(interval);
        }

        let timings = result
            .result
            .get("timings_ms")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();

        let timing_candidate_gen = timings.get("candidate_gen_ms").and_then(|v| v.as_u64());
        let timing_qdrant_search = timings.get("qdrant_search_ms").and_then(|v| v.as_u64());
        let timing_scoring = timings.get("scoring_ms").and_then(|v| v.as_u64());
        let timing_duckdb_candidates = timings.get("duckdb_candidate_ms").and_then(|v| v.as_u64());
        let timing_episode_extract = timings.get("episode_extract_ms").and_then(|v| v.as_u64());

        if let Some(ms) = timing_candidate_gen {
            candidate_gen_ms.push(ms);
        }
        if let Some(ms) = timing_qdrant_search {
            qdrant_search_ms.push(ms);
        }
        if let Some(ms) = timing_scoring {
            scoring_ms.push(ms);
        }
        if let Some(ms) = timing_duckdb_candidates {
            duckdb_candidate_ms.push(ms);
        }
        if let Some(ms) = timing_episode_extract {
            episode_extract_ms.push(ms);
        }

        let candidate_count = result
            .result
            .get("candidates")
            .and_then(|v| v.as_array())
            .map(|arr| arr.len() as u64)
            .unwrap_or(0);
        if let (Some(duckdb_ms), Some(scoring_ms), Some(episode_ms)) = (
            timing_duckdb_candidates,
            timing_scoring,
            timing_episode_extract,
        ) {
            let exact_ms = duckdb_ms
                .saturating_add(scoring_ms)
                .saturating_add(episode_ms);
            exact_stage_ms.push(exact_ms);
            if exact_ms > 0 && candidate_count > 0 {
                let throughput = (candidate_count as f64) / (exact_ms as f64 / 1000.0);
                scoring_throughput.push(throughput);
            }
        }

        if args.skip_preview {
            continue;
        }

        // Preview: pick the top candidate and its best episode if present.
        let top_candidate = result
            .result
            .get("candidates")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .cloned();

        let Some(candidate) = top_candidate else {
            println!(
                "tsse_bench: no candidates returned (job_id={}), skipping preview run",
                job_id
            );
            continue;
        };

        let candidate_sensor_id = candidate
            .get("sensor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if candidate_sensor_id.is_empty() {
            continue;
        }

        let best_episode = candidate
            .get("episodes")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .cloned();

        let mut preview_body = json!({
            "focus_sensor_id": focus_sensor_id,
            "candidate_sensor_id": candidate_sensor_id,
            "max_points": 5000
        });
        if let Some(ep) = best_episode {
            if let Some(start_ts) = ep.get("start_ts").and_then(|v| v.as_str()) {
                preview_body["episode_start_ts"] = json!(start_ts);
            }
            if let Some(end_ts) = ep.get("end_ts").and_then(|v| v.as_str()) {
                preview_body["episode_end_ts"] = json!(end_ts);
            }
            if let Some(lag_sec) = ep.get("lag_sec").and_then(|v| v.as_i64()) {
                preview_body["lag_seconds"] = json!(lag_sec);
            }
        }

        let preview_started = Instant::now();
        let _preview_resp: serde_json::Value = http
            .post(format!("{}/api/analysis/preview", base_url))
            .headers(headers.clone())
            .json(&preview_body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("failed to parse preview response")?;
        preview_wall_ms.push(preview_started.elapsed().as_millis() as u64);

        // Make it easier to separate runs in logs.
        let job_poll_ms = poll_started.elapsed().as_millis() as u64;
        println!(
            "tsse_bench: run complete job_id={} job_total_ms={} poll_ms={}",
            job_id, job_total_ms, job_poll_ms
        );
    }

    if let Some(monitor) = resource_monitor.as_mut() {
        monitor.sample();
    }

    let job_p50 = percentile_ms(&job_wall_ms, 0.50);
    let job_p95 = percentile_ms(&job_wall_ms, 0.95);
    let cand_p50 = percentile_ms(&candidate_gen_ms, 0.50);
    let cand_p95 = percentile_ms(&candidate_gen_ms, 0.95);
    let qdrant_p50 = percentile_ms(&qdrant_search_ms, 0.50);
    let qdrant_p95 = percentile_ms(&qdrant_search_ms, 0.95);
    let exact_stage_p50 = percentile_ms(&exact_stage_ms, 0.50);
    let exact_stage_p95 = percentile_ms(&exact_stage_ms, 0.95);
    let scoring_tp_p50 = percentile_f64(&scoring_throughput, 0.50);
    let scoring_tp_p95 = percentile_f64(&scoring_throughput, 0.95);
    let preview_p50 = percentile_ms(&preview_wall_ms, 0.50);
    let preview_p95 = percentile_ms(&preview_wall_ms, 0.95);
    let effective_interval_min = effective_intervals.iter().min().copied();
    let effective_interval_max = effective_intervals.iter().max().copied();

    // ADR 0006 targets (Jan 2026)
    let target_candidate_p50_ms: u64 = 250;
    let target_candidate_p95_ms: u64 = 750;
    let target_preview_p50_ms: u64 = 250;
    let target_preview_p95_ms: u64 = 750;

    let candidate_pass = cand_p50
        .zip(cand_p95)
        .map(|(p50, p95)| p50 <= target_candidate_p50_ms && p95 <= target_candidate_p95_ms)
        .unwrap_or(false);
    let preview_pass = args.skip_preview
        || preview_p50
            .zip(preview_p95)
            .map(|(p50, p95)| p50 <= target_preview_p50_ms && p95 <= target_preview_p95_ms)
            .unwrap_or(false);

    let job_p50_pass = gate_upper_u64(job_p50, args.job_p50_target_ms);
    let job_p95_pass = gate_upper_u64(job_p95, args.job_p95_target_ms);
    let scoring_tp_p50_pass = gate_lower_f64(scoring_tp_p50, args.scoring_throughput_p50_target);
    let scoring_tp_p95_pass = gate_lower_f64(scoring_tp_p95, args.scoring_throughput_p95_target);

    let resource_usage = resource_monitor.and_then(|monitor| monitor.finish());
    let resource_cpu_peak = resource_usage.as_ref().and_then(|r| r.peak_cpu_pct);
    let resource_ram_peak_mb = resource_usage
        .as_ref()
        .and_then(|r| r.peak_rss_bytes)
        .map(|bytes| bytes_to_mb(bytes).round() as u64);
    let resource_disk_read_mb = resource_usage
        .as_ref()
        .and_then(|r| r.disk_read_bytes)
        .map(|bytes| bytes_to_mb(bytes).round() as u64);
    let resource_disk_write_mb = resource_usage
        .as_ref()
        .and_then(|r| r.disk_write_bytes)
        .map(|bytes| bytes_to_mb(bytes).round() as u64);

    let resource_ram_pass = gate_upper_u64(resource_ram_peak_mb, args.resource_ram_peak_mb_target);
    let resource_disk_read_pass =
        gate_upper_u64(resource_disk_read_mb, args.resource_disk_read_mb_target);
    let resource_disk_write_pass =
        gate_upper_u64(resource_disk_write_mb, args.resource_disk_write_mb_target);

    let mut report = String::new();
    report.push_str("# TSSE Bench Report\n\n");
    report.push_str("## Run metadata\n\n");
    report.push_str(&format!(
        "- Date (UTC): {}\n- Base URL: `{}`\n- Focus sensor: `{}`\n- Window: `{}` → `{}`\n- Interval (requested): `{}` seconds\n- Interval (effective): {}\n- Runs: `{}`\n- Candidate limit: `{}` (min_pool `{}`)\n- same_unit_only: `{}`\n- skip_preview: `{}`\n- targets_file: {}\n",
        Utc::now().to_rfc3339(),
        base_url,
        focus_sensor_id,
        start.to_rfc3339(),
        end_inclusive.to_rfc3339(),
        args.interval_seconds,
        match (effective_interval_min, effective_interval_max) {
            (Some(min), Some(max)) if min == max => format!("{} seconds", min),
            (Some(min), Some(max)) => format!("{}-{} seconds", min, max),
            _ => "(unavailable)".to_string(),
        },
        args.runs,
        args.candidate_limit,
        args.min_pool,
        args.same_unit_only,
        args.skip_preview,
        args.targets_file
            .as_ref()
            .map(|path| format!("`{}`", path.display()))
            .unwrap_or_else(|| "(none)".to_string()),
    ));
    if let Some(usage) = resource_usage.as_ref() {
        report.push_str(&format!(
            "- Resource monitor: `{}` (sample {} ms)\n",
            usage.process_label, args.resource_sample_ms
        ));
    } else {
        report.push_str(&format!(
            "- Resource monitor: (unavailable) (sample {} ms)\n",
            args.resource_sample_ms
        ));
    }
    report.push_str("\n## Thresholds (targets + pass/fail)\n\n");
    report.push_str("| Metric | Target | Source | Result | Pass |\n");
    report.push_str("| --- | --- | --- | --- | --- |\n");
    report.push_str(&format!(
        "| Candidate generation latency p50 | <= {} ms | ADR 0006 | {} ms | {} |\n",
        target_candidate_p50_ms,
        fmt_opt_u64(cand_p50),
        fmt_pass(Some(candidate_pass)),
    ));
    report.push_str(&format!(
        "| Candidate generation latency p95 | <= {} ms | TSE-0019 | {} ms | {} |\n",
        target_candidate_p95_ms,
        fmt_opt_u64(cand_p95),
        fmt_pass(Some(candidate_pass)),
    ));
    report.push_str(&format!(
        "| Qdrant search time p50/p95 | — | TSE-0007/obs | {} / {} ms | N/A |\n",
        fmt_opt_u64(qdrant_p50),
        fmt_opt_u64(qdrant_p95),
    ));
    report.push_str(&format!(
        "| Preview endpoint latency p50 | <= {} ms | ADR 0006 | {} ms | {} |\n",
        target_preview_p50_ms,
        if args.skip_preview {
            "(skipped)".to_string()
        } else {
            fmt_opt_u64(preview_p50)
        },
        fmt_pass(if args.skip_preview {
            None
        } else {
            Some(preview_pass)
        }),
    ));
    report.push_str(&format!(
        "| Preview endpoint latency p95 | <= {} ms | TSE-0019 | {} ms | {} |\n",
        target_preview_p95_ms,
        if args.skip_preview {
            "(skipped)".to_string()
        } else {
            fmt_opt_u64(preview_p95)
        },
        fmt_pass(if args.skip_preview {
            None
        } else {
            Some(preview_pass)
        }),
    ));

    report.push_str(&format!(
        "| Exact stage latency p50 | TBD | TSE-0019/CLI | {} ms | N/A |\n",
        fmt_opt_u64(exact_stage_p50)
    ));
    report.push_str(&format!(
        "| Exact stage latency p95 | TBD | TSE-0019/CLI | {} ms | N/A |\n",
        fmt_opt_u64(exact_stage_p95)
    ));
    report.push_str(&format!(
        "| Exact stage throughput p50 (candidates/sec) | {} | TSE-0019/CLI | {} | {} |\n",
        args.scoring_throughput_p50_target
            .map(|v| format!(">= {v:.2}"))
            .unwrap_or_else(|| "TBD".to_string()),
        fmt_opt_f64(scoring_tp_p50, 2),
        fmt_pass(scoring_tp_p50_pass),
    ));
    report.push_str(&format!(
        "| Exact stage throughput p95 (candidates/sec) | {} | TSE-0019/CLI | {} | {} |\n",
        args.scoring_throughput_p95_target
            .map(|v| format!(">= {v:.2}"))
            .unwrap_or_else(|| "TBD".to_string()),
        fmt_opt_f64(scoring_tp_p95, 2),
        fmt_pass(scoring_tp_p95_pass),
    ));
    report.push_str(&format!(
        "| End-to-end related sensors job latency p50 | {} | TSE-0019/CLI | {} ms | {} |\n",
        args.job_p50_target_ms
            .map(|v| format!("<= {v} ms"))
            .unwrap_or_else(|| "TBD".to_string()),
        fmt_opt_u64(job_p50),
        fmt_pass(job_p50_pass),
    ));
    report.push_str(&format!(
        "| End-to-end related sensors job latency p95 | {} | TSE-0019/CLI | {} ms | {} |\n",
        args.job_p95_target_ms
            .map(|v| format!("<= {v} ms"))
            .unwrap_or_else(|| "TBD".to_string()),
        fmt_opt_u64(job_p95),
        fmt_pass(job_p95_pass),
    ));
    report.push_str(&format!(
        "| CPU peak | {} | TSE-0019/CLI | {} % | {} |\n",
        args.resource_cpu_peak_target
            .map(|v| format!("<= {v:.1}%"))
            .unwrap_or_else(|| "TBD".to_string()),
        resource_cpu_peak
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "—".to_string()),
        fmt_pass(args.resource_cpu_peak_target.map(|_| {
            resource_cpu_peak
                .map(|v| v <= args.resource_cpu_peak_target.unwrap_or(0.0))
                .unwrap_or(false)
        })),
    ));
    report.push_str(&format!(
        "| RAM peak | {} | TSE-0019/CLI | {} MB | {} |\n",
        args.resource_ram_peak_mb_target
            .map(|v| format!("<= {v} MB"))
            .unwrap_or_else(|| "TBD".to_string()),
        resource_ram_peak_mb
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        fmt_pass(resource_ram_pass),
    ));
    report.push_str(&format!(
        "| Disk IO read | {} | TSE-0019/CLI | {} MB | {} |\n",
        args.resource_disk_read_mb_target
            .map(|v| format!("<= {v} MB"))
            .unwrap_or_else(|| "TBD".to_string()),
        resource_disk_read_mb
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        fmt_pass(resource_disk_read_pass),
    ));
    report.push_str(&format!(
        "| Disk IO write | {} | TSE-0019/CLI | {} MB | {} |\n",
        args.resource_disk_write_mb_target
            .map(|v| format!("<= {v} MB"))
            .unwrap_or_else(|| "TBD".to_string()),
        resource_disk_write_mb
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        fmt_pass(resource_disk_write_pass),
    ));

    report.push_str("\n## Results\n\n");
    report.push_str("### Candidate generation\n");
    report.push_str(&format!(
        "- p50: {} ms\n- p95: {} ms\n- Notes: `candidate_gen_ms` from job timings.\n\n",
        fmt_opt_u64(cand_p50),
        fmt_opt_u64(cand_p95)
    ));

    report.push_str("### Qdrant search\n");
    report.push_str(&format!(
        "- p50: {} ms\n- p95: {} ms\n- Notes: sum of `qdrant_search_ms` across widening stages/vectors.\n\n",
        fmt_opt_u64(qdrant_p50),
        fmt_opt_u64(qdrant_p95)
    ));

    report.push_str("### Preview endpoint\n");
    if args.skip_preview {
        report.push_str(
            "- p50: (skipped)\n- p95: (skipped)\n- Notes: skipped via --skip-preview.\n\n",
        );
    } else {
        report.push_str(&format!(
            "- p50: {} ms\n- p95: {} ms\n- Notes: preview wall time for top candidate/episode.\n\n",
            fmt_opt_u64(preview_p50),
            fmt_opt_u64(preview_p95)
        ));
    }

    report.push_str("### Exact stage throughput\n");
    report.push_str(&format!(
        "- p50: {} candidates/sec\n- p95: {} candidates/sec\n- exact_stage_ms p50/p95: {} / {} ms\n- Notes: candidates/sec based on `duckdb_candidate_ms + scoring_ms + episode_extract_ms`.\n\n",
        fmt_opt_f64(scoring_tp_p50, 2),
        fmt_opt_f64(scoring_tp_p95, 2),
        fmt_opt_u64(exact_stage_p50),
        fmt_opt_u64(exact_stage_p95)
    ));

    report.push_str("### End-to-end job latency\n");
    report.push_str(&format!(
        "- p50: {} ms\n- p95: {} ms\n- Notes: wall time from job submission to completion.\n\n",
        fmt_opt_u64(job_p50),
        fmt_opt_u64(job_p95)
    ));

    report.push_str("### Resource usage\n");
    if let Some(usage) = resource_usage.as_ref() {
        report.push_str(&format!("- Process: `{}`\n", usage.process_label));
    } else {
        report.push_str("- Process: (unavailable)\n");
    }
    report.push_str(&format!(
        "- CPU peak: {} %\n- RAM peak: {} MB\n- Disk IO read/write: {} / {} MB\n- Notes: best-effort sampling of monitored process.\n\n",
        resource_cpu_peak
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "—".to_string()),
        resource_ram_peak_mb
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        resource_disk_read_mb
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        resource_disk_write_mb
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string())
    ));

    report.push_str("## Artifacts\n\n");
    report.push_str("- Raw logs: (not captured)\n");
    report.push_str("- JSON summary: (not captured)\n");

    report.push_str("\n## Notes\n\n");
    report.push_str("- `candidate_gen_ms`, `duckdb_candidate_ms`, `scoring_ms`, and `episode_extract_ms` come from the job’s `timings_ms` payload.\n");
    report.push_str(
        "- Targets are sourced from `docs/ADRs/0006-time-series-similarity-engine-(tsse)-on-controller-similarity-search.md` when available; other targets are CLI-provided.\n",
    );
    report.push_str(
        "- Resource sampling uses process statistics and may be unavailable on some systems.\n",
    );
    report.push_str(
        "- Interval (effective) is read from job result params (auto-clamped horizons may override the requested interval).\n",
    );

    if let Some(parent) = args.report.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.report, report)
        .with_context(|| format!("failed to write {}", args.report.display()))?;

    println!(
        "tsse_bench: wrote report to {} (candidate_p50={}ms, preview_p50={}ms)",
        args.report.display(),
        cand_p50
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
        preview_p50
            .map(|v| v.to_string())
            .unwrap_or_else(|| "—".to_string()),
    );

    let mut failed = false;
    if !candidate_pass {
        failed = true;
    }
    if !preview_pass {
        failed = true;
    }
    if matches!(job_p50_pass, Some(false)) || matches!(job_p95_pass, Some(false)) {
        failed = true;
    }
    if matches!(scoring_tp_p50_pass, Some(false)) || matches!(scoring_tp_p95_pass, Some(false)) {
        failed = true;
    }
    if matches!(resource_ram_pass, Some(false))
        || matches!(resource_disk_read_pass, Some(false))
        || matches!(resource_disk_write_pass, Some(false))
    {
        failed = true;
    }
    if let Some(target) = args.resource_cpu_peak_target {
        if let Some(cpu_peak) = resource_cpu_peak {
            if cpu_peak > target {
                failed = true;
            }
        } else {
            failed = true;
        }
    }

    if failed {
        anyhow::bail!("TSSE bench failed targets (see report)");
    }

    Ok(())
}
