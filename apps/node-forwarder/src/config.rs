use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone)]
pub struct Config {
    pub node_id: String,
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub mqtt_username: Option<String>,
    pub mqtt_password: Option<String>,
    pub mqtt_topic_prefix: String,
    pub mqtt_client_id: String,

    pub http_bind: String,

    pub spool_dir: PathBuf,
    pub segment_roll_duration: Duration,
    pub segment_roll_bytes: u64,
    pub sync_interval: Duration,
    pub max_spool_bytes: u64,
    pub keep_free_bytes: u64,
    pub max_spool_age: Option<Duration>,

    pub replay_msgs_per_sec: u32,
    pub replay_bytes_per_sec: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let node_id = env_string("NODE_NODE_ID", Some("pi-node".to_string()))?;
        let mqtt_url = env_string("NODE_MQTT_URL", Some("mqtt://127.0.0.1:1883".to_string()))?;
        let mqtt_username = env_optional("NODE_MQTT_USERNAME");
        let mqtt_password = env_optional("NODE_MQTT_PASSWORD");

        let url = Url::parse(&mqtt_url).context("invalid NODE_MQTT_URL")?;
        let mqtt_host = url
            .host_str()
            .ok_or_else(|| anyhow!("NODE_MQTT_URL missing host"))?
            .to_string();
        let mqtt_port = url.port().unwrap_or(1883) as u16;

        let mqtt_topic_prefix = env_string("NODE_MQTT_TOPIC_PREFIX", Some("iot".to_string()))?;
        let mqtt_client_id = env_string(
            "NODE_FORWARDER_MQTT_CLIENT_ID",
            Some(format!("node-forwarder-{}", node_id)),
        )?;

        let http_bind =
            env_string("NODE_FORWARDER_HTTP_BIND", Some("127.0.0.1:9101".to_string()))?;

        let spool_dir = PathBuf::from(env_string(
            "NODE_FORWARDER_SPOOL_DIR",
            Some("/opt/node-agent/storage/spool".to_string()),
        )?);

        let segment_roll_duration = Duration::from_secs(env_u64(
            "NODE_FORWARDER_SEGMENT_ROLL_SECONDS",
            Some(3600),
        )?);
        let segment_roll_bytes =
            env_u64("NODE_FORWARDER_SEGMENT_ROLL_BYTES", Some(128 * 1024 * 1024))?;

        let sync_interval =
            Duration::from_millis(env_u64("NODE_FORWARDER_SYNC_INTERVAL_MS", Some(1000))?);

        // Default policy: min(max(1GiB, 5% of filesystem), 25GiB), but we may not know filesystem
        // size here. Compute a conservative default and allow overrides.
        let max_spool_bytes = env_u64("NODE_FORWARDER_MAX_SPOOL_BYTES", Some(1 * 1024 * 1024 * 1024))?;

        let keep_free_bytes =
            env_u64("NODE_FORWARDER_KEEP_FREE_BYTES", Some(2 * 1024 * 1024 * 1024))?;

        let max_spool_age = match env_optional("NODE_FORWARDER_MAX_SPOOL_AGE_SECONDS") {
            Some(raw) => {
                let secs = raw
                    .trim()
                    .parse::<u64>()
                    .context("invalid NODE_FORWARDER_MAX_SPOOL_AGE_SECONDS")?;
                Some(Duration::from_secs(secs))
            }
            None => None,
        };

        let replay_msgs_per_sec =
            env_u64("NODE_FORWARDER_REPLAY_MSGS_PER_SEC", Some(2000))? as u32;
        let replay_bytes_per_sec =
            env_u64("NODE_FORWARDER_REPLAY_BYTES_PER_SEC", Some(10 * 1024 * 1024))? as u32;

        Ok(Self {
            node_id,
            mqtt_host,
            mqtt_port,
            mqtt_username,
            mqtt_password,
            mqtt_topic_prefix,
            mqtt_client_id,
            http_bind,
            spool_dir,
            segment_roll_duration,
            segment_roll_bytes,
            sync_interval,
            max_spool_bytes,
            keep_free_bytes,
            max_spool_age,
            replay_msgs_per_sec,
            replay_bytes_per_sec,
        })
    }
}

fn env_string(key: &str, default: Option<String>) -> Result<String> {
    match env::var(key) {
        Ok(value) => Ok(value.trim().to_string()),
        Err(_) => default.ok_or_else(|| anyhow!("missing env var {key}")),
    }
}

fn env_u64(key: &str, default: Option<u64>) -> Result<u64> {
    match env::var(key) {
        Ok(value) => value
            .trim()
            .parse::<u64>()
            .with_context(|| format!("invalid {key}")),
        Err(_) => default.ok_or_else(|| anyhow!("missing env var {key}")),
    }
}

fn env_optional(key: &str) -> Option<String> {
    env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}
