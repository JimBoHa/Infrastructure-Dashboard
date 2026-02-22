use super::runner::JobFailure;
use super::types::{AnalysisJobError, AnalysisJobRow};
use crate::services::analysis::lake::AnalysisLakeConfig;
use crate::services::analysis::lake_inspector;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize)]
struct LakeInspectResultV1 {
    job_type: String,
    inspection: lake_inspector::LakeInspection,
    #[serde(default)]
    versions: BTreeMap<String, String>,
}

pub async fn execute(
    _db: &PgPool,
    lake: &AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let inspection = lake_inspector::inspect(lake).map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "inspect_failed".to_string(),
            message: err.to_string(),
            details: Some(serde_json::json!({ "job_id": job.id.to_string() })),
        })
    })?;

    let result = LakeInspectResultV1 {
        job_type: "lake_inspect_v1".to_string(),
        inspection,
        versions: BTreeMap::from([("lake_inspector".to_string(), "v1".to_string())]),
    };

    serde_json::to_value(result).map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "serialize_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })
}
