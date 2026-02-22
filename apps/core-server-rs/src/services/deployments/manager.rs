use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::ssh::{fingerprint_sha256, handshake_ssh, host_key_type_to_name, known_hosts_entry};
use super::types::{
    DeploymentJob, DeploymentNodeInfo, DeploymentStep, DeploymentUserRef, HostKeyScanRequest,
    HostKeyScanResponse, JobStatus, PiDeploymentRequest, StepStatus, STEP_NAMES,
};
use super::util::{random_hex, validate_username};
use super::DeploymentManager;

impl DeploymentManager {
    pub fn new(
        db: sqlx::PgPool,
        overlay_path: std::path::PathBuf,
        ssh_known_hosts_path: std::path::PathBuf,
        node_agent_port: u16,
        default_mqtt_url: String,
        default_mqtt_username: Option<String>,
        default_mqtt_password: Option<String>,
    ) -> Self {
        Self {
            db,
            overlay_path,
            ssh_known_hosts_path,
            node_agent_port,
            default_mqtt_url,
            default_mqtt_username,
            default_mqtt_password,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn create_pi5_job(
        &self,
        request: PiDeploymentRequest,
        user: DeploymentUserRef,
    ) -> Result<DeploymentJob> {
        validate_username(&request.username)?;
        let job_id = random_hex(8);
        let job = DeploymentJob {
            id: job_id.clone(),
            status: JobStatus::Queued,
            created_at: Utc::now().to_rfc3339(),
            started_at: None,
            finished_at: None,
            steps: STEP_NAMES
                .iter()
                .map(|name| DeploymentStep {
                    name: name.to_string(),
                    status: StepStatus::Pending,
                    started_at: None,
                    finished_at: None,
                    logs: Vec::new(),
                })
                .collect(),
            error: None,
            node: Some(DeploymentNodeInfo {
                host: Some(request.host.clone()),
                ..DeploymentNodeInfo::default()
            }),
            outcome: None,
        };
        {
            let mut store = self.jobs_lock();
            store.insert(job_id.clone(), job.clone());
        }

        let manager = self.clone();
        tokio::spawn(async move {
            let _ = tokio::task::spawn_blocking(move || {
                manager.run_pi5_deployment(job_id, request, user)
            })
            .await;
        });

        Ok(job)
    }

    pub fn get_job(&self, job_id: &str) -> Option<DeploymentJob> {
        let store = self.jobs_lock();
        store.get(job_id).cloned()
    }

    pub fn scan_host_key(&self, request: HostKeyScanRequest) -> Result<HostKeyScanResponse> {
        let session = handshake_ssh(&request.host, request.port)?;
        let (host_key, host_key_type) = session
            .host_key()
            .ok_or_else(|| anyhow::anyhow!("SSH host key unavailable"))?;
        let fingerprint = fingerprint_sha256(host_key);
        Ok(HostKeyScanResponse {
            host: request.host.clone(),
            port: request.port,
            key_type: host_key_type_to_name(host_key_type),
            fingerprint_sha256: fingerprint.clone(),
            known_hosts_entry: known_hosts_entry(
                &request.host,
                request.port,
                host_key_type,
                host_key,
            ),
        })
    }
}
