use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow, AnalysisJobStatus};
use crate::config::CoreConfig;
use crate::services::analysis::lake::AnalysisLakeConfig;
use crate::services::analysis::parquet_duckdb::DuckDbQueryService;
use crate::services::analysis::profiling::{JobProfileRequest, JobProfiler};
use crate::services::analysis::qdrant::QdrantService;
use anyhow::Result;
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::Instrument;
use uuid::Uuid;

pub struct AnalysisJobService {
    db: PgPool,
    lake: AnalysisLakeConfig,
    duckdb: DuckDbQueryService,
    qdrant: Arc<QdrantService>,
    semaphore: Arc<Semaphore>,
    running: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
    poll_interval: Duration,
    profile_enabled: bool,
    profile_output_path: PathBuf,
}

impl AnalysisJobService {
    pub fn new(db: PgPool, config: &CoreConfig, qdrant: Arc<QdrantService>) -> Self {
        let lake = AnalysisLakeConfig {
            hot_path: config.analysis_lake_hot_path.clone(),
            cold_path: config.analysis_lake_cold_path.clone(),
            tmp_path: config.analysis_tmp_path.clone(),
            shards: config.analysis_lake_shards,
            hot_retention_days: config.analysis_hot_retention_days,
            late_window_hours: config.analysis_late_window_hours,
            replication_interval: Duration::from_secs(config.analysis_replication_interval_seconds),
            replication_lag: Duration::from_secs(config.analysis_replication_lag_seconds),
        };
        let duckdb = DuckDbQueryService::new(config.analysis_tmp_path.clone(), 2);
        let max_concurrency = config.analysis_max_concurrent_jobs;
        let poll_interval = Duration::from_millis(config.analysis_poll_interval_ms);
        let profile_enabled = config.analysis_profile_enabled;
        let profile_output_path = config.analysis_profile_output_path.clone();
        Self {
            db,
            lake,
            duckdb,
            qdrant,
            semaphore: Arc::new(Semaphore::new(max_concurrency.max(1))),
            running: Arc::new(Mutex::new(HashMap::new())),
            poll_interval,
            profile_enabled,
            profile_output_path,
        }
    }

    pub fn lake_config(&self) -> &AnalysisLakeConfig {
        &self.lake
    }

    pub fn duckdb(&self) -> &DuckDbQueryService {
        &self.duckdb
    }

    pub fn qdrant(&self) -> Arc<QdrantService> {
        self.qdrant.clone()
    }

    pub async fn create_job(
        &self,
        request: &super::types::AnalysisJobCreateRequest,
        created_by: Option<Uuid>,
    ) -> Result<(AnalysisJobRow, bool), sqlx::Error> {
        store::create_job(&self.db, request, created_by).await
    }

    pub async fn get_job(&self, job_id: Uuid) -> Result<Option<AnalysisJobRow>, sqlx::Error> {
        store::get_job(&self.db, job_id).await
    }

    pub async fn get_job_by_key(
        &self,
        job_type: &str,
        job_key: &str,
    ) -> Result<Option<AnalysisJobRow>, sqlx::Error> {
        store::get_job_by_key(&self.db, job_type, job_key).await
    }

    pub async fn list_events(
        &self,
        job_id: Uuid,
        after: i64,
        limit: i64,
    ) -> Result<Vec<super::types::AnalysisJobEventPublic>, sqlx::Error> {
        store::list_events(&self.db, job_id, after, limit).await
    }

    pub async fn get_result(&self, job_id: Uuid) -> Result<Option<serde_json::Value>, sqlx::Error> {
        store::get_result(&self.db, job_id).await
    }

    pub async fn request_cancel(
        &self,
        job_id: Uuid,
    ) -> Result<Option<AnalysisJobRow>, sqlx::Error> {
        let updated = store::request_cancel(&self.db, job_id).await?;
        if updated.is_some() {
            let token_opt = { self.running.lock().await.get(&job_id).cloned() };
            if let Some(token) = token_opt {
                token.cancel();
            }
        }
        Ok(updated)
    }

    pub async fn count_active_jobs_for_user(&self, user_id: Uuid) -> Result<i64, sqlx::Error> {
        store::count_active_jobs_for_user(&self.db, user_id).await
    }

    pub fn start(self: Arc<Self>, cancel: CancellationToken) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(self.poll_interval) => {}
                }

                while let Ok(permit) = self.semaphore.clone().try_acquire_owned() {
                    let job = match store::claim_next_pending(&self.db).await {
                        Ok(job) => job,
                        Err(err) => {
                            tracing::warn!(error = %err, "analysis job poller failed to claim job");
                            drop(permit);
                            break;
                        }
                    };

                    let Some(job) = job else {
                        drop(permit);
                        break;
                    };

                    let service = self.clone();
                    let span = tracing::info_span!(
                        "analysis_job",
                        job_id = %job.id,
                        job_type = %job.job_type,
                        job_key = ?job.job_key,
                        created_by = ?job.created_by,
                    );
                    tokio::spawn(async move {
                        let _permit = permit;
                        if let Err(err) = service.run_one(job).instrument(span).await {
                            tracing::warn!(error = %err, "analysis job runner error");
                        }
                    });
                }
            }
        });
    }

    async fn current_phase(&self, job_id: Uuid) -> Option<String> {
        match store::get_job(&self.db, job_id).await {
            Ok(Some(job)) => Some(job.progress.0.phase),
            _ => None,
        }
    }

    async fn run_one(self: Arc<Self>, job: AnalysisJobRow) -> Result<()> {
        let started = Instant::now();
        let job_id = job.id;
        let cancel = CancellationToken::new();
        {
            let mut running = self.running.lock().await;
            running.insert(job_id, cancel.clone());
        }

        let profile_request = JobProfileRequest::from_job_params(
            self.profile_enabled,
            self.profile_output_path.clone(),
            &job.params.0,
        );
        if profile_request.requested {
            tracing::info!(
                job_id = %job_id,
                job_type = %job.job_type,
                profile_output_dir = %profile_request.output_dir.display(),
                "analysis job profiling requested"
            );
        }

        let profiler = JobProfiler::start(
            profile_request.enabled,
            profile_request.output_dir.clone(),
            job_id,
            &job.job_type,
        );

        tracing::info!(phase = "runner_start", "analysis job execution started");

        let outcome = self.execute_job(&job, cancel.clone()).await;

        {
            let mut running = self.running.lock().await;
            running.remove(&job_id);
        }

        match outcome {
            Ok(result) => {
                let duration_ms = started.elapsed().as_millis() as u64;
                let _ = store::append_event(
                    &self.db,
                    job_id,
                    "runner_summary",
                    serde_json::json!({
                        "status": "completed",
                        "duration_ms": duration_ms,
                    }),
                )
                .await;
                store::mark_completed(&self.db, job_id, result).await?;
                tracing::info!(
                    phase = "runner_complete",
                    status = "completed",
                    duration_ms,
                    "analysis job execution finished"
                );
            }
            Err(JobFailure::Canceled) => {
                let duration_ms = started.elapsed().as_millis() as u64;
                let current_phase = self.current_phase(job_id).await;
                let _ = store::append_event(
                    &self.db,
                    job_id,
                    "runner_summary",
                    serde_json::json!({
                        "status": "canceled",
                        "duration_ms": duration_ms,
                    }),
                )
                .await;
                let _ = store::append_event(
                    &self.db,
                    job_id,
                    "phase_timing",
                    serde_json::json!({
                        "phase": "job_total",
                        "duration_ms": duration_ms,
                        "status": "canceled",
                        "current_phase": current_phase,
                    }),
                )
                .await;
                store::mark_canceled(&self.db, job_id).await?;
                tracing::info!(
                    phase = "runner_complete",
                    status = "canceled",
                    duration_ms,
                    "analysis job execution finished"
                );
            }
            Err(JobFailure::Failed(error)) => {
                let duration_ms = started.elapsed().as_millis() as u64;
                let current_phase = self.current_phase(job_id).await;
                let error_code = error.code.clone();
                let error_message = error.message.clone();
                let _ = store::append_event(
                    &self.db,
                    job_id,
                    "runner_summary",
                    serde_json::json!({
                        "status": "failed",
                        "duration_ms": duration_ms,
                        "error_code": error_code,
                    }),
                )
                .await;
                let _ = store::append_event(
                    &self.db,
                    job_id,
                    "phase_timing",
                    serde_json::json!({
                        "phase": "job_total",
                        "duration_ms": duration_ms,
                        "status": "failed",
                        "current_phase": current_phase,
                    }),
                )
                .await;
                store::mark_failed(&self.db, job_id, error).await?;
                tracing::warn!(
                    phase = "runner_complete",
                    status = "failed",
                    duration_ms,
                    error_code = %error_code,
                    error_message = %error_message,
                    "analysis job execution finished"
                );
            }
        }

        let profile_path = profiler.finish();
        if let Some(path) = profile_path.as_ref() {
            let _ = store::append_event(
                &self.db,
                job_id,
                "profile_written",
                serde_json::json!({
                    "path": path.display().to_string(),
                }),
            )
            .await;
        }

        Ok(())
    }

    async fn execute_job(
        &self,
        job: &AnalysisJobRow,
        cancel: CancellationToken,
    ) -> std::result::Result<serde_json::Value, JobFailure> {
        if job.cancel_requested_at.is_some() {
            return Err(JobFailure::Canceled);
        }

        match job.job_type.as_str() {
            "noop_v1" => self.execute_noop(job, cancel).await,
            "lake_backfill_v1" => {
                super::lake_backfill_v1::execute(&self.db, &self.lake, job, cancel).await
            }
            "lake_inspect_v1" => {
                super::lake_inspect_v1::execute(&self.db, &self.lake, job, cancel).await
            }
            "lake_replication_tick_v1" => {
                super::lake_replication_tick_v1::execute(&self.db, &self.lake, job, cancel).await
            }
            "lake_parity_check_v1" => {
                super::lake_parity_check_v1::execute(
                    &self.db,
                    &self.duckdb,
                    &self.lake,
                    job,
                    cancel,
                )
                .await
            }
            "embeddings_build_v1" => {
                super::embeddings_build_v1::execute(
                    &self.db,
                    &self.duckdb,
                    &self.lake,
                    &self.qdrant,
                    job,
                    cancel,
                )
                .await
            }
            "related_sensors_v1" => {
                super::related_sensors_v1::execute(
                    &self.db,
                    &self.duckdb,
                    &self.lake,
                    &self.qdrant,
                    job,
                    cancel,
                )
                .await
            }
            "related_sensors_unified_v2" => {
                super::related_sensors_unified_v2::execute(
                    &self.db,
                    &self.duckdb,
                    &self.lake,
                    job,
                    cancel,
                )
                .await
            }
            "alarm_rule_backtest_v1" => {
                super::alarm_rule_backtest_v1::execute(&self.db, &self.duckdb, &self.lake, job, cancel).await
            }
            "correlation_matrix_v1" => {
                super::correlation_matrix_v1::execute(
                    &self.db,
                    &self.duckdb,
                    &self.lake,
                    job,
                    cancel,
                )
                .await
            }
            "event_match_v1" => {
                super::event_match_v1::execute(&self.db, &self.duckdb, &self.lake, job, cancel)
                    .await
            }
            "cooccurrence_v1" => {
                super::cooccurrence_v1::execute(&self.db, &self.duckdb, &self.lake, job, cancel)
                    .await
            }
            "matrix_profile_v1" => {
                super::matrix_profile_v1::execute(&self.db, &self.duckdb, &self.lake, job, cancel)
                    .await
            }
            "forecast_materialize_v1" => {
                super::forecast_materialize_v1::execute(&self.db, job, cancel).await
            }
            other => Err(JobFailure::Failed(AnalysisJobError {
                code: "unsupported_job_type".to_string(),
                message: format!("Unsupported job type: {other}"),
                details: Some(serde_json::json!({ "job_type": other })),
            })),
        }
    }

    async fn execute_noop(
        &self,
        job: &AnalysisJobRow,
        cancel: CancellationToken,
    ) -> std::result::Result<serde_json::Value, JobFailure> {
        let steps: u64 = job
            .params
            .0
            .get("steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(50)
            .clamp(1, 5_000);

        let mut progress = AnalysisJobProgress {
            phase: "noop".to_string(),
            completed: 0,
            total: Some(steps),
            message: Some("Running noop analysis job".to_string()),
        };
        if let Err(err) = store::update_progress(&self.db, job.id, &progress).await {
            tracing::warn!(error = %err, job_id = %job.id, "failed to write noop progress");
        }

        for idx in 0..steps {
            if cancel.is_cancelled() {
                return Err(JobFailure::Canceled);
            }
            progress.completed = idx + 1;
            if idx == 0 {
                progress.message = Some("Starting".to_string());
            } else if idx + 1 == steps {
                progress.message = Some("Finishing".to_string());
            } else if idx % 25 == 0 {
                progress.message = Some(format!("Step {} of {}", idx + 1, steps));
            }
            if idx % 10 == 0 || idx + 1 == steps {
                if let Err(err) = store::update_progress(&self.db, job.id, &progress).await {
                    tracing::warn!(error = %err, job_id = %job.id, "failed to write noop progress");
                }
            }
            tokio::task::yield_now().await;
        }

        Ok(serde_json::json!({
            "job_type": job.job_type,
            "status": AnalysisJobStatus::Completed.as_str(),
            "steps": steps,
        }))
    }
}

pub(super) enum JobFailure {
    Canceled,
    Failed(AnalysisJobError),
}
