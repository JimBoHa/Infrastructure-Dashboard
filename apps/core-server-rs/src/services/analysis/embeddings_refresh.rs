use crate::config::CoreConfig;
use crate::services::analysis::jobs::{AnalysisJobCreateRequest, AnalysisJobService};
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct EmbeddingsRefreshService {
    analysis_jobs: Arc<AnalysisJobService>,
    refresh_interval: Duration,
    refresh_horizon_days: i64,
    full_rebuild_interval: Duration,
    full_rebuild_horizon_days: i64,
}

impl EmbeddingsRefreshService {
    pub fn new(analysis_jobs: Arc<AnalysisJobService>, config: &CoreConfig) -> Self {
        Self {
            analysis_jobs,
            refresh_interval: Duration::from_secs(
                config.analysis_embeddings_refresh_interval_seconds.max(60),
            ),
            refresh_horizon_days: config.analysis_embeddings_refresh_horizon_days.max(1),
            full_rebuild_interval: Duration::from_secs(
                config
                    .analysis_embeddings_full_rebuild_interval_hours
                    .saturating_mul(3600)
                    .max(3600),
            ),
            full_rebuild_horizon_days: config.analysis_embeddings_full_rebuild_horizon_days.max(1),
        }
    }

    pub fn start(self, cancel: CancellationToken) {
        let refresh = self.clone();
        let refresh_cancel = cancel.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(refresh.refresh_interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = refresh_cancel.cancelled() => break,
                    _ = ticker.tick() => {}
                }

                if let Err(err) = refresh
                    .schedule_job(EmbeddingsRefreshKind::Incremental)
                    .await
                {
                    tracing::warn!(error = %err, "embeddings refresh tick failed");
                }
            }
        });

        let full = self.clone();
        tokio::spawn(async move {
            let start = tokio::time::Instant::now() + full.full_rebuild_interval;
            let mut ticker = tokio::time::interval_at(start, full.full_rebuild_interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = ticker.tick() => {}
                }

                if let Err(err) = full.schedule_job(EmbeddingsRefreshKind::FullRebuild).await {
                    tracing::warn!(error = %err, "embeddings full rebuild tick failed");
                }
            }
        });
    }

    async fn schedule_job(&self, kind: EmbeddingsRefreshKind) -> Result<()> {
        let (prefix, horizon_days, interval) = match kind {
            EmbeddingsRefreshKind::Incremental => {
                ("refresh", self.refresh_horizon_days, self.refresh_interval)
            }
            EmbeddingsRefreshKind::FullRebuild => (
                "full_rebuild",
                self.full_rebuild_horizon_days,
                self.full_rebuild_interval,
            ),
        };
        let bucket = (Utc::now().timestamp() as u64) / interval.as_secs().max(1);
        let job_key = format!("embeddings_{}_{}", prefix, bucket);
        let params = json!({
            "horizon_days": horizon_days,
        });
        let request = AnalysisJobCreateRequest {
            job_type: "embeddings_build_v1".to_string(),
            params,
            job_key: Some(job_key.clone()),
            dedupe: true,
        };

        let (job, created) = self.analysis_jobs.create_job(&request, None).await?;
        if created {
            tracing::info!(
                job_id = %job.id,
                job_type = %job.job_type,
                job_key = %job_key,
                horizon_days,
                "scheduled embeddings build job"
            );
        } else {
            tracing::debug!(
                job_id = %job.id,
                job_type = %job.job_type,
                job_key = %job_key,
                "embeddings build job already scheduled"
            );
        }
        Ok(())
    }
}

enum EmbeddingsRefreshKind {
    Incremental,
    FullRebuild,
}
