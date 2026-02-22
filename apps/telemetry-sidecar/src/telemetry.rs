use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MetricRow {
    pub sensor_id: String,
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub quality: i32,
    pub source: Option<String>,
    pub seq: Option<u64>,
    pub stream_id: Option<Uuid>,
    pub backfill: bool,
}

#[derive(Debug, Deserialize)]
struct BorrowedTelemetry<'a> {
    #[serde(default, borrow)]
    timestamp: Option<BorrowedTimestamp<'a>>,
    value: f64,
    #[serde(default)]
    quality: Option<i32>,
    #[serde(default)]
    seq: Option<u64>,
    #[serde(default, borrow)]
    stream_id: Option<&'a str>,
    #[serde(default)]
    backfill: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BorrowedTimestamp<'a> {
    Str(&'a str),
    Int(i64),
    Float(f64),
}

impl<'a> BorrowedTimestamp<'a> {
    fn to_datetime(&self) -> DateTime<Utc> {
        match self {
            BorrowedTimestamp::Str(s) => DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            BorrowedTimestamp::Int(ms) => millis_to_dt(*ms),
            BorrowedTimestamp::Float(ts) => millis_to_dt((*ts * 1000.0) as i64),
        }
    }
}

fn millis_to_dt(ms: i64) -> DateTime<Utc> {
    let secs = ms / 1000;
    let nanos = ((ms % 1000) * 1_000_000) as u32;
    Utc.timestamp_opt(secs, nanos)
        .single()
        .unwrap_or_else(Utc::now)
}

pub fn parse_mqtt_payload(
    topic_prefix: &str,
    topic: &str,
    payload: &mut [u8],
) -> Result<Option<MetricRow>> {
    let parts: Vec<&str> = topic.split('/').collect();
    if parts.len() != 4 || parts[0] != topic_prefix || parts[3] != "telemetry" {
        return Ok(None);
    }

    let sensor_id = parts[2].to_string();
    let telemetry: BorrowedTelemetry = simd_json::from_slice(payload)?;

    let timestamp = telemetry
        .timestamp
        .as_ref()
        .map(|t| t.to_datetime())
        .unwrap_or_else(Utc::now);

    let quality = telemetry.quality.unwrap_or(0);
    let seq = telemetry.seq;
    let stream_id = telemetry
        .stream_id
        .and_then(|raw| Uuid::parse_str(raw.trim()).ok());
    let backfill = telemetry.backfill.unwrap_or(false);

    Ok(Some(MetricRow {
        sensor_id,
        timestamp,
        value: telemetry.value,
        quality,
        source: Some(parts[1].to_string()),
        seq,
        stream_id,
        backfill,
    }))
}
