use super::rolling::RollingAverager;
use super::types::SensorMeta;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub(in crate::ingest) struct IngestState {
    pub(in crate::ingest) sensor_meta: HashMap<String, SensorMeta>,
    pub(in crate::ingest) rolling: HashMap<String, RollingAverager>,
    pub(in crate::ingest) cov_last: HashMap<String, (f64, i32)>,
    pub(in crate::ingest) cov_initialized: HashSet<String>,
    pub(in crate::ingest) sensor_last_seen: HashMap<String, DateTime<Utc>>,
    pub(in crate::ingest) sensor_last_sample_ts: HashMap<String, DateTime<Utc>>,
    pub(in crate::ingest) node_last_seen: HashMap<String, DateTime<Utc>>,
    pub(in crate::ingest) node_last_metric_seen: HashMap<String, DateTime<Utc>>,
    pub(in crate::ingest) node_last_sample_ts: HashMap<String, DateTime<Utc>>,
    pub(in crate::ingest) sensor_status: HashMap<String, String>,
    pub(in crate::ingest) node_status: HashMap<String, String>,
    pub(in crate::ingest) node_heartbeat_interval_seconds: HashMap<String, f64>,
    pub(in crate::ingest) node_aliases: HashMap<String, String>,
}

impl IngestState {
    pub(in crate::ingest) fn new() -> Self {
        Self {
            sensor_meta: HashMap::new(),
            rolling: HashMap::new(),
            cov_last: HashMap::new(),
            cov_initialized: HashSet::new(),
            sensor_last_seen: HashMap::new(),
            sensor_last_sample_ts: HashMap::new(),
            node_last_seen: HashMap::new(),
            node_last_metric_seen: HashMap::new(),
            node_last_sample_ts: HashMap::new(),
            sensor_status: HashMap::new(),
            node_status: HashMap::new(),
            node_heartbeat_interval_seconds: HashMap::new(),
            node_aliases: HashMap::new(),
        }
    }
}
