use crate::auth::AuthManager;
use crate::config::CoreConfig;
use crate::services::analysis::jobs::AnalysisJobService;
use crate::services::analysis::qdrant::QdrantService;
use crate::services::deployments::DeploymentManager;
use crate::services::mqtt::MqttPublisher;
use axum::extract::FromRef;
use reqwest::Client;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: CoreConfig,
    pub db: PgPool,
    pub auth: Arc<AuthManager>,
    pub mqtt: Arc<MqttPublisher>,
    pub deployments: Arc<DeploymentManager>,
    pub analysis_jobs: Arc<AnalysisJobService>,
    pub qdrant: Arc<QdrantService>,
    pub http: Client,
}

impl FromRef<AppState> for Arc<AuthManager> {
    fn from_ref(state: &AppState) -> Arc<AuthManager> {
        state.auth.clone()
    }
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> PgPool {
        state.db.clone()
    }
}
