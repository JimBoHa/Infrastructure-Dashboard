use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::robust;
use crate::services::analysis::parquet_duckdb::MetricsBucketRow;

pub const TSSE_EMBEDDING_VERSION_V1: &str = "tsse_embeddings_v1";

pub const TSSE_VECTOR_VALUE: &str = "value";
pub const TSSE_VECTOR_DELTA: &str = "delta";
pub const TSSE_VECTOR_EVENT: &str = "event";

pub const TSSE_PAYLOAD_SENSOR_ID: &str = "sensor_id";
pub const TSSE_PAYLOAD_NODE_ID: &str = "node_id";
pub const TSSE_PAYLOAD_SENSOR_TYPE: &str = "sensor_type";
pub const TSSE_PAYLOAD_UNIT: &str = "unit";
pub const TSSE_PAYLOAD_INTERVAL_SECONDS: &str = "interval_seconds";
pub const TSSE_PAYLOAD_EMBEDDING_VERSION: &str = "embedding_version";
pub const TSSE_PAYLOAD_WINDOW_SECONDS: &str = "window_seconds";
pub const TSSE_PAYLOAD_UPDATED_AT: &str = "updated_at";
pub const TSSE_PAYLOAD_COMPUTED_THROUGH_TS: &str = "computed_through_ts";
pub const TSSE_PAYLOAD_IS_DERIVED: &str = "is_derived";
pub const TSSE_PAYLOAD_IS_PUBLIC_PROVIDER: &str = "is_public_provider";

pub fn qdrant_point_id(sensor_id: &str) -> String {
    let name = format!("farmdashboard:tsse:sensor:{}", sensor_id.trim());
    Uuid::new_v5(&Uuid::NAMESPACE_URL, name.as_bytes()).to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsseEmbeddingsV1 {
    pub value: Vec<f32>,
    pub delta: Vec<f32>,
    pub event: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct TsseEmbeddingConfig {
    pub version: String,
    pub windows_sec: Vec<i64>,
    pub value_dim: usize,
    pub delta_dim: usize,
    pub event_dim: usize,
    pub z_clip: f64,
    pub spike_z: f64,
}

impl Default for TsseEmbeddingConfig {
    fn default() -> Self {
        Self {
            version: TSSE_EMBEDDING_VERSION_V1.to_string(),
            windows_sec: vec![300, 3600, 21_600, 86_400, 604_800, 2_592_000, 7_776_000],
            value_dim: 128,
            delta_dim: 128,
            event_dim: 64,
            z_clip: 6.0,
            spike_z: 2.5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TsseEmbeddingMetadata {
    pub sensor_id: String,
    pub node_id: Option<String>,
    pub sensor_type: Option<String>,
    pub unit: Option<String>,
    pub interval_seconds: Option<i64>,
    pub computed_through_ts: Option<String>,
    pub is_derived: Option<bool>,
    pub is_public_provider: Option<bool>,
}

pub fn l2_normalize_in_place(values: &mut [f32]) {
    let norm = values
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    let norm = if norm.is_finite() && norm > 0.0 {
        norm as f32
    } else {
        1.0
    };
    for v in values.iter_mut() {
        *v /= norm;
    }
}

pub fn pad_or_truncate(mut values: Vec<f32>, dim: usize) -> Vec<f32> {
    if values.len() > dim {
        values.truncate(dim);
        return values;
    }
    if values.len() < dim {
        values.resize(dim, 0.0);
    }
    values
}

pub fn compute_sensor_embeddings(
    rows: &[MetricsBucketRow],
    config: &TsseEmbeddingConfig,
) -> Option<TsseEmbeddingsV1> {
    let mut points: Vec<(i64, f64)> = rows
        .iter()
        .filter(|r| r.value.is_finite())
        .map(|r| (r.bucket.timestamp(), r.value))
        .collect();
    if points.len() < 3 {
        return None;
    }
    points.sort_by_key(|(ts, _)| *ts);
    points.dedup_by_key(|(ts, _)| *ts);
    if points.len() < 3 {
        return None;
    }

    let end_ts = points.last().map(|v| v.0).unwrap_or(0);
    let mut value_features: Vec<f32> = Vec::new();
    let mut delta_features: Vec<f32> = Vec::new();
    let mut event_features: Vec<f32> = Vec::new();

    for &window_sec in &config.windows_sec {
        let values = window_values(&points, end_ts, window_sec);
        let deltas = window_deltas(&points, end_ts, window_sec);

        value_features.extend(robust_features(&values, config.z_clip));
        delta_features.extend(robust_features(&deltas, config.z_clip));
        event_features.extend(event_signature(&values, &deltas, config));
    }

    let all_values: Vec<f64> = points.iter().map(|(_, v)| *v).collect();
    let all_deltas = window_deltas(&points, end_ts, i64::MAX / 4);
    event_features.extend(event_signature(&all_values, &all_deltas, config));

    let mut value = pad_or_truncate(value_features, config.value_dim);
    let mut delta = pad_or_truncate(delta_features, config.delta_dim);
    let mut event = pad_or_truncate(event_features, config.event_dim);

    l2_normalize_in_place(&mut value);
    l2_normalize_in_place(&mut delta);
    l2_normalize_in_place(&mut event);

    Some(TsseEmbeddingsV1 {
        value,
        delta,
        event,
    })
}

pub fn build_qdrant_point(
    embeddings: &TsseEmbeddingsV1,
    meta: &TsseEmbeddingMetadata,
    config: &TsseEmbeddingConfig,
    updated_at: DateTime<Utc>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        TSSE_PAYLOAD_SENSOR_ID: meta.sensor_id,
        TSSE_PAYLOAD_EMBEDDING_VERSION: config.version,
        TSSE_PAYLOAD_WINDOW_SECONDS: config.windows_sec,
        TSSE_PAYLOAD_UPDATED_AT: updated_at.to_rfc3339(),
        TSSE_PAYLOAD_COMPUTED_THROUGH_TS: meta.computed_through_ts,
    });

    if let Some(node_id) = &meta.node_id {
        payload[TSSE_PAYLOAD_NODE_ID] = serde_json::Value::String(node_id.clone());
    }
    if let Some(sensor_type) = &meta.sensor_type {
        payload[TSSE_PAYLOAD_SENSOR_TYPE] = serde_json::Value::String(sensor_type.clone());
    }
    if let Some(unit) = &meta.unit {
        payload[TSSE_PAYLOAD_UNIT] = serde_json::Value::String(unit.clone());
    }
    if let Some(interval_seconds) = meta.interval_seconds {
        payload[TSSE_PAYLOAD_INTERVAL_SECONDS] = serde_json::Value::Number(interval_seconds.into());
    }
    if let Some(is_derived) = meta.is_derived {
        payload[TSSE_PAYLOAD_IS_DERIVED] = serde_json::Value::Bool(is_derived);
    }
    if let Some(is_public_provider) = meta.is_public_provider {
        payload[TSSE_PAYLOAD_IS_PUBLIC_PROVIDER] = serde_json::Value::Bool(is_public_provider);
    }

    serde_json::json!({
        "id": qdrant_point_id(&meta.sensor_id),
        "vectors": {
            TSSE_VECTOR_VALUE: embeddings.value,
            TSSE_VECTOR_DELTA: embeddings.delta,
            TSSE_VECTOR_EVENT: embeddings.event
        },
        "payload": payload
    })
}

fn window_values(points: &[(i64, f64)], end_ts: i64, window_sec: i64) -> Vec<f64> {
    let start_ts = end_ts.saturating_sub(window_sec.max(0));
    points
        .iter()
        .filter(|(ts, _)| *ts >= start_ts)
        .map(|(_, v)| *v)
        .collect()
}

fn window_deltas(points: &[(i64, f64)], end_ts: i64, window_sec: i64) -> Vec<f64> {
    let start_ts = end_ts.saturating_sub(window_sec.max(0));
    let mut out = Vec::new();
    let mut prev: Option<(i64, f64)> = None;
    for (ts, value) in points.iter().copied() {
        if ts < start_ts {
            continue;
        }
        if let Some((prev_ts, prev_val)) = prev {
            let dt = (ts - prev_ts).max(1) as f64;
            out.push((value - prev_val) / dt);
        }
        prev = Some((ts, value));
    }
    out
}

fn robust_features(values: &[f64], clip: f64) -> Vec<f32> {
    if values.len() < 3 {
        return vec![0.0; 10];
    }

    let mut scratch = values.to_vec();
    let median = robust::median(&mut scratch).unwrap_or(0.0);
    let mad = robust::mad(&mut scratch, median).unwrap_or(0.0);
    let p05 = robust::quantile(&mut scratch, 0.05).unwrap_or(0.0);
    let p50 = robust::quantile(&mut scratch, 0.50).unwrap_or(0.0);
    let p95 = robust::quantile(&mut scratch, 0.95).unwrap_or(0.0);
    let p25 = robust::quantile(&mut scratch, 0.25).unwrap_or(0.0);
    let p75 = robust::quantile(&mut scratch, 0.75).unwrap_or(0.0);
    let iqr = (p75 - p25).abs();

    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values
        .iter()
        .map(|v| {
            let d = *v - mean;
            d * d
        })
        .sum::<f64>()
        / values.len() as f64;
    let std = var.sqrt();

    let z = robust::zscore_robust(values, clip).unwrap_or_default();
    let mut abs_vals: Vec<f64> = z
        .iter()
        .filter(|v| v.is_finite())
        .map(|v| v.abs())
        .collect();
    let z_mean_abs = if abs_vals.is_empty() {
        0.0
    } else {
        abs_vals.iter().sum::<f64>() / abs_vals.len() as f64
    };
    let z_p95_abs = robust::quantile(&mut abs_vals, 0.95).unwrap_or(0.0);

    vec![
        median as f32,
        mad as f32,
        p05 as f32,
        p50 as f32,
        p95 as f32,
        iqr as f32,
        mean as f32,
        std as f32,
        z_mean_abs as f32,
        z_p95_abs as f32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn qdrant_point_id_is_stable_uuid() {
        let sensor_id = "a1b2c3d4e5f6a7b8c9d0e1f2";
        let first = qdrant_point_id(sensor_id);
        let second = qdrant_point_id(sensor_id);
        assert_eq!(first, second);
        assert!(Uuid::parse_str(&first).is_ok());

        let different = qdrant_point_id("ffffffffffffffffffffffff");
        assert_ne!(first, different);
    }

    #[test]
    fn build_qdrant_point_uses_uuid_id_and_payload_sensor_id() {
        let embeddings = TsseEmbeddingsV1 {
            value: vec![0.1, 0.2],
            delta: vec![0.3],
            event: vec![0.4],
        };
        let meta = TsseEmbeddingMetadata {
            sensor_id: "11223344556677889900aabb".to_string(),
            node_id: None,
            sensor_type: None,
            unit: None,
            interval_seconds: None,
            computed_through_ts: None,
            is_derived: None,
            is_public_provider: None,
        };
        let config = TsseEmbeddingConfig::default();
        let updated_at = Utc.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap();
        let point = build_qdrant_point(&embeddings, &meta, &config, updated_at);

        let id = point.get("id").and_then(|v| v.as_str()).unwrap_or("");
        assert!(Uuid::parse_str(id).is_ok());
        let payload_sensor_id = point
            .get("payload")
            .and_then(|v| v.get(TSSE_PAYLOAD_SENSOR_ID))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert_eq!(payload_sensor_id, meta.sensor_id);
    }
}

fn event_signature(values: &[f64], deltas: &[f64], config: &TsseEmbeddingConfig) -> Vec<f32> {
    let mut out = Vec::with_capacity(8);
    let (spike_rate, spike_rate_hi, max_abs, p95_abs) = spike_features(values, config);
    out.push(spike_rate as f32);
    out.push(spike_rate_hi as f32);
    out.push(max_abs as f32);
    out.push(p95_abs as f32);

    let (spike_rate, spike_rate_hi, max_abs, p95_abs) = spike_features(deltas, config);
    out.push(spike_rate as f32);
    out.push(spike_rate_hi as f32);
    out.push(max_abs as f32);
    out.push(p95_abs as f32);
    out
}

fn spike_features(values: &[f64], config: &TsseEmbeddingConfig) -> (f64, f64, f64, f64) {
    if values.len() < 3 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let z = robust::zscore_robust(values, config.z_clip).unwrap_or_default();
    let mut abs_vals: Vec<f64> = z
        .iter()
        .filter(|v| v.is_finite())
        .map(|v| v.abs())
        .collect();
    if abs_vals.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut spike = 0.0;
    let mut spike_hi = 0.0;
    for v in abs_vals.iter() {
        if *v >= config.spike_z {
            spike += 1.0;
        }
        if *v >= config.spike_z + 1.5 {
            spike_hi += 1.0;
        }
    }
    let denom = abs_vals.len() as f64;
    let max_abs = abs_vals.iter().copied().fold(0.0_f64, |acc, v| acc.max(v));
    let p95_abs = robust::quantile(&mut abs_vals, 0.95).unwrap_or(0.0);
    (spike / denom, spike_hi / denom, max_abs, p95_abs)
}

pub async fn compute_sensor_embeddings_placeholder() -> Result<TsseEmbeddingsV1> {
    Ok(TsseEmbeddingsV1 {
        value: vec![0.0; 128],
        delta: vec![0.0; 128],
        event: vec![0.0; 64],
    })
}
