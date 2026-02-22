use super::runner::JobFailure;
use crate::services::analysis::lake::AnalysisLakeConfig;
use crate::services::analysis::parquet_duckdb::DuckDbQueryService;
use anyhow::{Context, Result};
use sqlx::PgPool;
use tokio_util::sync::CancellationToken;

/// Dev-only helper wrapper used by offline evaluation harnesses.
///
/// This module deliberately keeps the per-job implementations private while still enabling
/// deterministic local evaluation via `src/bin/*` tooling.
pub async fn execute_related_sensors_unified_v2(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    job: &super::AnalysisJobRow,
    cancel: CancellationToken,
) -> Result<serde_json::Value> {
    match super::related_sensors_unified_v2::execute(db, duckdb, lake, job, cancel).await {
        Ok(value) => Ok(value),
        Err(JobFailure::Canceled) => anyhow::bail!("analysis job canceled"),
        Err(JobFailure::Failed(err)) => {
            Err(anyhow::anyhow!("analysis job failed: {}", err.message))
                .with_context(|| format!("code={}", err.code))
        }
    }
}

