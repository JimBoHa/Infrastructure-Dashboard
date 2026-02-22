use crate::services::analysis::security;
use chrono::Utc;
use serde_json::Value;
use std::fs::File;
use std::path::{Component, PathBuf};
use std::time::Instant;
use uuid::Uuid;

pub struct JobProfileRequest {
    pub enabled: bool,
    pub output_dir: PathBuf,
    pub requested: bool,
}

impl JobProfileRequest {
    pub fn from_job_params(
        default_enabled: bool,
        default_output_dir: PathBuf,
        params: &Value,
    ) -> Self {
        let mut enabled = default_enabled;
        let mut requested = false;
        let mut output_dir = default_output_dir;

        if let Some(value) = params.get("profile").and_then(|v| v.as_bool()) {
            if value {
                enabled = true;
                requested = true;
            }
        }

        if let Some(value) = params.get("profile_output_dir").and_then(|v| v.as_str()) {
            match resolve_profile_output_dir(value) {
                Some(path) => {
                    output_dir = path;
                    enabled = true;
                    requested = true;
                }
                None => {
                    tracing::warn!(
                        profile_output_dir = value,
                        "ignoring invalid profile output dir"
                    );
                }
            }
        }

        Self {
            enabled,
            output_dir,
            requested,
        }
    }
}

pub struct JobProfiler {
    guard: Option<pprof::ProfilerGuard<'static>>,
    output_dir: PathBuf,
    job_id: Uuid,
    job_type: String,
    started: Instant,
}

impl JobProfiler {
    pub fn start(enabled: bool, output_dir: PathBuf, job_id: Uuid, job_type: &str) -> Self {
        let guard = if enabled {
            match pprof::ProfilerGuard::new(100) {
                Ok(guard) => Some(guard),
                Err(err) => {
                    tracing::warn!(
                        job_id = %job_id,
                        job_type = %job_type,
                        error = %err,
                        "failed to start profiler guard"
                    );
                    None
                }
            }
        } else {
            None
        };

        Self {
            guard,
            output_dir,
            job_id,
            job_type: job_type.to_string(),
            started: Instant::now(),
        }
    }

    pub fn finish(self) -> Option<PathBuf> {
        let Some(guard) = self.guard else {
            return None;
        };

        if let Err(err) = security::ensure_dir_mode(&self.output_dir, 0o700) {
            tracing::warn!(
                job_id = %self.job_id,
                job_type = %self.job_type,
                error = %err,
                "failed to prepare profiling output directory"
            );
            return None;
        }

        let report = match guard.report().build() {
            Ok(report) => report,
            Err(err) => {
                tracing::warn!(
                    job_id = %self.job_id,
                    job_type = %self.job_type,
                    error = %err,
                    "failed to build profiling report"
                );
                return None;
            }
        };

        let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
        let filename = format!(
            "analysis_job_{}_{}_{}.svg",
            self.job_type, self.job_id, timestamp
        );
        let path = self.output_dir.join(filename);
        let file = match File::create(&path) {
            Ok(file) => file,
            Err(err) => {
                tracing::warn!(
                    job_id = %self.job_id,
                    job_type = %self.job_type,
                    error = %err,
                    "failed to create profiling output file"
                );
                return None;
            }
        };

        if let Err(err) = report.flamegraph(file) {
            tracing::warn!(
                job_id = %self.job_id,
                job_type = %self.job_type,
                error = %err,
                "failed to write profiling flamegraph"
            );
            return None;
        }

        let _ = security::ensure_file_mode(&path, 0o600);

        tracing::info!(
            job_id = %self.job_id,
            job_type = %self.job_type,
            duration_ms = self.started.elapsed().as_millis() as u64,
            path = %path.display(),
            "analysis job profile written"
        );

        Some(path)
    }
}

fn resolve_profile_output_dir(value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut path = PathBuf::from(trimmed);
    if path.is_relative() {
        let cwd = std::env::current_dir().ok()?;
        path = cwd.join(path);
    }

    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return None;
    }

    if path
        .components()
        .any(|component| component.as_os_str() == "project_management")
    {
        return None;
    }

    if path.to_string_lossy().contains("project_management") {
        return None;
    }

    Some(path)
}
