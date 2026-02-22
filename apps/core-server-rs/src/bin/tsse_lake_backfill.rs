use anyhow::{Context, Result};
use clap::Parser;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration as StdDuration, Instant};

#[derive(Debug, Parser)]
#[command(about = "Trigger a TSSE lake backfill job via the core-server API.")]
struct Args {
    /// Base URL for the core-server API.
    #[arg(long, default_value = "http://127.0.0.1:8000")]
    base_url: String,

    /// API token string (Bearer). Prefer --auth-token-file.
    #[arg(long)]
    auth_token: Option<String>,

    /// File containing a single-line API token string (Bearer).
    #[arg(long)]
    auth_token_file: Option<PathBuf>,

    /// Backfill range in days.
    #[arg(long, default_value_t = 90)]
    days: u32,

    /// Replace existing Parquet partitions in the backfill window.
    #[arg(long, default_value_t = true)]
    replace_existing: bool,

    /// Wait for the job to finish.
    #[arg(long, default_value_t = false)]
    wait: bool,

    /// Poll interval seconds when --wait is enabled.
    #[arg(long, default_value_t = 5)]
    poll_seconds: u64,

    /// Timeout seconds when --wait is enabled (0 = no timeout).
    #[arg(long, default_value_t = 0)]
    timeout_seconds: u64,

    /// Fetch and print the job result after completion.
    #[arg(long, default_value_t = false)]
    show_result: bool,
}

#[derive(Debug, Deserialize)]
struct AnalysisJobProgress {
    phase: String,
    completed: u64,
    total: Option<u64>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnalysisJobPublic {
    id: String,
    status: String,
    progress: AnalysisJobProgress,
    #[serde(default)]
    error: Option<AnalysisJobError>,
}

#[derive(Debug, Deserialize)]
struct AnalysisJobError {
    code: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct AnalysisJobCreateResponse {
    job: AnalysisJobPublic,
}

#[derive(Debug, Deserialize)]
struct AnalysisJobStatusResponse {
    job: AnalysisJobPublic,
}

#[derive(Debug, Deserialize)]
struct AnalysisJobResultResponse {
    result: serde_json::Value,
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

fn render_progress(progress: &AnalysisJobProgress) -> String {
    let total = progress
        .total
        .map(|total| format!("{}/{}", progress.completed, total))
        .unwrap_or_else(|| progress.completed.to_string());
    match progress.message.as_deref() {
        Some(message) if !message.trim().is_empty() => {
            format!("{} ({})", message.trim(), total)
        }
        _ => format!("{} ({})", progress.phase, total),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let token = read_token(&args)?;
    let http = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(30))
        .build()
        .context("failed to build HTTP client")?;
    let headers = headers(&token)?;

    let payload = json!({
        "job_type": "lake_backfill_v1",
        "params": {
            "days": args.days,
            "replace_existing": args.replace_existing,
        },
        "dedupe": false,
    });

    let create: AnalysisJobCreateResponse = http
        .post(format!("{}/api/analysis/jobs", args.base_url))
        .headers(headers.clone())
        .json(&payload)
        .send()
        .await
        .context("failed to create backfill job")?
        .error_for_status()
        .context("backfill job request failed")?
        .json()
        .await
        .context("failed to decode backfill response")?;

    let job_id = create.job.id.clone();
    println!("Backfill job created: {}", job_id);
    println!(
        "status={} progress={}",
        create.job.status,
        render_progress(&create.job.progress)
    );

    if !args.wait {
        return Ok(());
    }

    let poll_every = StdDuration::from_secs(args.poll_seconds.max(1));
    let deadline = if args.timeout_seconds > 0 {
        Some(Instant::now() + StdDuration::from_secs(args.timeout_seconds))
    } else {
        None
    };

    loop {
        if let Some(deadline) = deadline {
            if Instant::now() > deadline {
                anyhow::bail!("backfill job timed out after {}s", args.timeout_seconds);
            }
        }

        tokio::time::sleep(poll_every).await;
        let status: AnalysisJobStatusResponse = http
            .get(format!("{}/api/analysis/jobs/{}", args.base_url, job_id))
            .headers(headers.clone())
            .send()
            .await
            .context("failed to poll backfill job")?
            .error_for_status()
            .context("backfill job poll failed")?
            .json()
            .await
            .context("failed to decode backfill status")?;

        println!(
            "status={} progress={}",
            status.job.status,
            render_progress(&status.job.progress)
        );

        match status.job.status.as_str() {
            "completed" | "failed" | "canceled" => {
                if status.job.status != "completed" {
                    if let Some(error) = status.job.error.as_ref() {
                        anyhow::bail!("backfill job {} ({})", error.code, error.message.trim());
                    }
                    anyhow::bail!("backfill job {}", status.job.status);
                }
                if args.show_result {
                    let result: AnalysisJobResultResponse = http
                        .get(format!(
                            "{}/api/analysis/jobs/{}/result",
                            args.base_url, job_id
                        ))
                        .headers(headers.clone())
                        .send()
                        .await
                        .context("failed to fetch backfill result")?
                        .error_for_status()
                        .context("backfill result request failed")?
                        .json()
                        .await
                        .context("failed to decode backfill result")?;
                    println!("result: {}", serde_json::to_string_pretty(&result.result)?);
                }
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
