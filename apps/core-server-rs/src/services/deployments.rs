mod bootstrap;
mod db;
mod jobs;
mod manager;
mod runner;
mod ssh;
mod types;
mod util;

use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub use types::{
    DeploymentJob, DeploymentNodeInfo, DeploymentStep, DeploymentUserRef, HostKeyScanRequest,
    HostKeyScanResponse, JobStatus, PiDeploymentRequest, StepStatus,
};

#[derive(Clone)]
pub struct DeploymentManager {
    db: PgPool,
    overlay_path: PathBuf,
    ssh_known_hosts_path: PathBuf,
    node_agent_port: u16,
    default_mqtt_url: String,
    default_mqtt_username: Option<String>,
    default_mqtt_password: Option<String>,
    jobs: Arc<Mutex<HashMap<String, DeploymentJob>>>,
}
