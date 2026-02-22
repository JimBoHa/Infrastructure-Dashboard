use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json as SqlJson;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisJobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Canceled,
}

impl AnalysisJobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AnalysisJobStatus::Pending => "pending",
            AnalysisJobStatus::Running => "running",
            AnalysisJobStatus::Completed => "completed",
            AnalysisJobStatus::Failed => "failed",
            AnalysisJobStatus::Canceled => "canceled",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            AnalysisJobStatus::Completed | AnalysisJobStatus::Failed | AnalysisJobStatus::Canceled
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobProgress {
    pub phase: String,
    pub completed: u64,
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Default for AnalysisJobProgress {
    fn default() -> Self {
        Self {
            phase: "queued".to_string(),
            completed: 0,
            total: None,
            message: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobPublic {
    pub id: String,
    pub job_type: String,
    pub status: AnalysisJobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canceled_at: Option<String>,
    pub progress: AnalysisJobProgress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AnalysisJobError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobEventPublic {
    pub id: i64,
    pub created_at: String,
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobEventsResponse {
    pub events: Vec<AnalysisJobEventPublic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_after: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobCreateRequest {
    pub job_type: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub job_key: Option<String>,
    #[serde(default)]
    pub dedupe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobCreateResponse {
    pub job: AnalysisJobPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobStatusResponse {
    pub job: AnalysisJobPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobCancelResponse {
    pub job: AnalysisJobPublic,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AnalysisJobResultResponse {
    pub job_id: String,
    pub result: serde_json::Value,
}

#[derive(sqlx::FromRow, Debug, Clone)]
pub struct AnalysisJobRow {
    pub id: Uuid,
    pub job_type: String,
    pub status: String,
    pub job_key: Option<String>,
    pub created_by: Option<Uuid>,
    pub params: SqlJson<serde_json::Value>,
    pub progress: SqlJson<AnalysisJobProgress>,
    pub error: Option<SqlJson<AnalysisJobError>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub canceled_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl AnalysisJobRow {
    pub fn status_enum(&self) -> AnalysisJobStatus {
        match self.status.as_str() {
            "pending" => AnalysisJobStatus::Pending,
            "running" => AnalysisJobStatus::Running,
            "completed" => AnalysisJobStatus::Completed,
            "failed" => AnalysisJobStatus::Failed,
            "canceled" => AnalysisJobStatus::Canceled,
            other => {
                tracing::warn!(status = %other, job_id = %self.id, "unknown analysis job status; treating as failed");
                AnalysisJobStatus::Failed
            }
        }
    }

    pub fn to_public(&self) -> AnalysisJobPublic {
        AnalysisJobPublic {
            id: self.id.to_string(),
            job_type: self.job_type.clone(),
            status: self.status_enum(),
            job_key: self.job_key.clone(),
            created_by: self.created_by.map(|id| id.to_string()),
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.updated_at.to_rfc3339(),
            started_at: self.started_at.map(|ts| ts.to_rfc3339()),
            completed_at: self.completed_at.map(|ts| ts.to_rfc3339()),
            canceled_at: self.canceled_at.map(|ts| ts.to_rfc3339()),
            progress: self.progress.0.clone(),
            error: self.error.as_ref().map(|value| value.0.clone()),
        }
    }
}
