use std::fmt;

pub(super) const STEP_NAMES: [&str; 7] = [
    "Prepare bundle",
    "Connect via SSH",
    "Inspect node",
    "Upload bundle",
    "Install node-agent",
    "Start services",
    "Verify health",
];

pub(super) const MAX_LOG_LINES: usize = 200;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct DeploymentStep {
    pub name: String,
    pub status: StepStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, Default)]
pub struct DeploymentNodeInfo {
    pub node_id: Option<String>,
    pub node_name: Option<String>,
    pub adoption_token: Option<String>,
    pub mac_eth: Option<String>,
    pub mac_wifi: Option<String>,
    pub host: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct DeploymentJob {
    pub id: String,
    pub status: JobStatus,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub steps: Vec<DeploymentStep>,
    pub error: Option<String>,
    pub node: Option<DeploymentNodeInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outcome: Option<String>,
}

#[derive(Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct PiDeploymentRequest {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_private_key_pem: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ssh_private_key_passphrase: Option<String>,
    pub node_name: Option<String>,
    pub node_id: Option<String>,
    pub mqtt_url: Option<String>,
    pub mqtt_username: Option<String>,
    pub mqtt_password: Option<String>,
    pub adoption_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_key_fingerprint: Option<String>,
}

impl fmt::Debug for PiDeploymentRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PiDeploymentRequest")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field(
                "ssh_private_key_pem",
                &self.ssh_private_key_pem.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "ssh_private_key_passphrase",
                &self
                    .ssh_private_key_passphrase
                    .as_ref()
                    .map(|_| "<redacted>"),
            )
            .field("node_name", &self.node_name)
            .field("node_id", &self.node_id)
            .field("mqtt_url", &self.mqtt_url)
            .field("mqtt_username", &self.mqtt_username)
            .field(
                "mqtt_password",
                &self.mqtt_password.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "adoption_token",
                &self.adoption_token.as_ref().map(|_| "<redacted>"),
            )
            .field("host_key_fingerprint", &self.host_key_fingerprint)
            .finish()
    }
}

#[derive(Debug, Clone, serde::Deserialize, utoipa::ToSchema)]
pub struct HostKeyScanRequest {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct HostKeyScanResponse {
    pub host: String,
    pub port: u16,
    pub key_type: String,
    pub fingerprint_sha256: String,
    pub known_hosts_entry: String,
}

#[derive(Debug, Clone)]
pub struct DeploymentUserRef {
    pub id: String,
    pub email: String,
    pub role: String,
}
