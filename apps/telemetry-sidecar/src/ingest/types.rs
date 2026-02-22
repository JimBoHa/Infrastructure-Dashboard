use chrono::{DateTime, Utc};

#[derive(Clone, Debug)]
pub(in crate::ingest) struct SensorMeta {
    pub(in crate::ingest) sensor_id: String,
    pub(in crate::ingest) node_id: String,
    pub(in crate::ingest) interval_seconds: i64,
    pub(in crate::ingest) rolling_avg_seconds: i64,
}

#[derive(Clone, Debug)]
pub(in crate::ingest) struct Sample {
    pub(in crate::ingest) timestamp: DateTime<Utc>,
    pub(in crate::ingest) value: f64,
    pub(in crate::ingest) quality: i32,
    pub(in crate::ingest) samples: i64,
}
