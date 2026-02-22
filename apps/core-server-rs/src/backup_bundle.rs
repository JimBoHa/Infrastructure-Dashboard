use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::routes::node_sensors::{NodeAds1263SettingsDraft, NodeSensorDraft};

pub(crate) const NODE_BACKUP_SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BackupOutputSnapshot {
    pub(crate) id: String,
    pub(crate) name: String,
    #[serde(rename = "type")]
    pub(crate) output_type: String,
    #[serde(default)]
    pub(crate) supported_states: Vec<String>,
    #[serde(default)]
    pub(crate) config: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NodeBackupBundle {
    pub(crate) schema_version: i32,
    pub(crate) captured_at: String,
    pub(crate) node_id: String,
    pub(crate) node_name: String,
    #[serde(default)]
    pub(crate) desired_sensors: Vec<NodeSensorDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) desired_ads1263: Option<NodeAds1263SettingsDraft>,
    #[serde(default)]
    pub(crate) outputs: Vec<BackupOutputSnapshot>,
}
