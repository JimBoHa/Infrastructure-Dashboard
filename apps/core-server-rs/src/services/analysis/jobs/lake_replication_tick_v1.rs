use super::runner::JobFailure;
use super::types::{AnalysisJobError, AnalysisJobRow};
use crate::services::analysis::lake::{
    read_replication_state, AnalysisLakeConfig, ReplicationState,
};
use crate::services::analysis::replication::run_replication_tick;
use serde::Serialize;
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize)]
struct LakeReplicationTickResultV1 {
    job_type: String,
    status: String,
    replication: ReplicationState,
}

pub async fn execute(
    db: &PgPool,
    lake: &AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    match run_replication_tick(db, lake).await {
        Ok(state) => {
            let result = LakeReplicationTickResultV1 {
                job_type: "lake_replication_tick_v1".to_string(),
                status: "ok".to_string(),
                replication: state,
            };
            serde_json::to_value(result).map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "serialize_failed".to_string(),
                    message: err.to_string(),
                    details: None,
                })
            })
        }
        Err(err) => {
            let replication = read_replication_state(lake).unwrap_or_default();
            Err(JobFailure::Failed(AnalysisJobError {
                code: "replication_tick_failed".to_string(),
                message: err.to_string(),
                details: Some(serde_json::json!({
                    "job_id": job.id.to_string(),
                    "replication": replication,
                })),
            }))
        }
    }
}
