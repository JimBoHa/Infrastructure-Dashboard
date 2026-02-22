use anyhow::{Context, Result};
use reqwest::StatusCode;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::services::analysis::tsse::embeddings::{
    TSSE_PAYLOAD_COMPUTED_THROUGH_TS, TSSE_PAYLOAD_EMBEDDING_VERSION,
    TSSE_PAYLOAD_INTERVAL_SECONDS, TSSE_PAYLOAD_IS_DERIVED, TSSE_PAYLOAD_IS_PUBLIC_PROVIDER,
    TSSE_PAYLOAD_NODE_ID, TSSE_PAYLOAD_SENSOR_ID, TSSE_PAYLOAD_SENSOR_TYPE, TSSE_PAYLOAD_UNIT,
    TSSE_PAYLOAD_UPDATED_AT, TSSE_PAYLOAD_WINDOW_SECONDS,
};

pub const COLLECTION_SENSOR_SIMILARITY_V1: &str = "sensor_similarity_v1";

#[derive(Clone)]
pub struct QdrantService {
    base_url: String,
    http: reqwest::Client,
}

impl QdrantService {
    pub fn new(base_url: String, http: reqwest::Client) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        }
    }

    pub fn start(self: Arc<Self>, cancel: CancellationToken) {
        tokio::spawn(async move {
            let mut delay = Duration::from_secs(2);
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(delay) => {}
                }

                match self.ensure_schema().await {
                    Ok(()) => {
                        tracing::info!("qdrant schema ensured");
                        break;
                    }
                    Err(err) => {
                        tracing::warn!(error = %err, "qdrant schema ensure failed (will retry)");
                        delay = std::cmp::min(delay * 2, Duration::from_secs(60));
                    }
                }
            }
        });
    }

    pub async fn healthz(&self) -> Result<bool> {
        let url = format!("{}/healthz", self.base_url);
        let resp = self
            .http
            .get(url)
            .timeout(Duration::from_secs(2))
            .send()
            .await;
        match resp {
            Ok(r) => Ok(r.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    pub async fn ensure_schema(&self) -> Result<()> {
        if !self.healthz().await? {
            anyhow::bail!("qdrant healthz not OK at {}", self.base_url);
        }
        self.ensure_collection_sensor_similarity_v1().await?;
        Ok(())
    }

    async fn ensure_collection_sensor_similarity_v1(&self) -> Result<()> {
        let url = format!(
            "{}/collections/{}",
            self.base_url, COLLECTION_SENSOR_SIMILARITY_V1
        );
        let resp = self.http.get(&url).send().await?;
        if resp.status().is_success() {
            self.ensure_payload_indexes().await?;
            return Ok(());
        }
        if resp.status() != StatusCode::NOT_FOUND {
            anyhow::bail!("qdrant get collection returned {}", resp.status());
        }

        // Minimal multi-vector schema for TSSE candidate generation.
        // Dimensions and exact embedding strategy are refined in later tickets; keep stable defaults.
        let create_body = json!({
            "vectors": {
                "value": { "size": 128, "distance": "Cosine" },
                "delta": { "size": 128, "distance": "Cosine" },
                "event": { "size": 64, "distance": "Cosine" }
            },
            "hnsw_config": {
                "m": 16,
                "ef_construct": 128,
                "full_scan_threshold": 10000
            },
            "optimizers_config": {
                "default_segment_number": 4
            }
        });

        let resp = self
            .http
            .put(&url)
            .json(&create_body)
            .send()
            .await
            .with_context(|| format!("qdrant create collection request failed for {url}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("qdrant create collection failed: {} {}", status, body);
        }
        self.ensure_payload_indexes().await?;
        Ok(())
    }

    async fn ensure_payload_indexes(&self) -> Result<()> {
        for spec in payload_index_specs() {
            self.ensure_payload_index(&spec).await?;
        }
        Ok(())
    }

    async fn ensure_payload_index(&self, spec: &PayloadIndexSpec) -> Result<()> {
        let url = format!(
            "{}/collections/{}/index?wait=true",
            self.base_url, COLLECTION_SENSOR_SIMILARITY_V1
        );
        let body = json!({
            "field_name": spec.field_name,
            "field_schema": spec.field_schema,
        });
        let resp = self
            .http
            .put(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("qdrant create payload index request failed for {url}"))?;
        if resp.status().is_success() {
            return Ok(());
        }
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let body_lower = body.to_lowercase();
        if status == StatusCode::CONFLICT || body_lower.contains("already exists") {
            tracing::debug!(
                field_name = %spec.field_name,
                "qdrant payload index already exists"
            );
            return Ok(());
        }
        anyhow::bail!(
            "qdrant create payload index failed for {}: {} {}",
            spec.field_name,
            status,
            body
        );
    }

    pub async fn upsert_points(&self, points: Vec<serde_json::Value>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }
        let url = format!(
            "{}/collections/{}/points?wait=true",
            self.base_url, COLLECTION_SENSOR_SIMILARITY_V1
        );
        let body = json!({ "points": points });
        let resp = self
            .http
            .put(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("qdrant upsert request failed for {url}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("qdrant upsert failed: {} {}", status, body);
        }
        Ok(())
    }

    pub async fn search_named_vector(
        &self,
        vector_name: &str,
        vector: Vec<f32>,
        filter: Option<serde_json::Value>,
        limit: u32,
    ) -> Result<Vec<QdrantScoredPoint>> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.base_url, COLLECTION_SENSOR_SIMILARITY_V1
        );
        let mut body = json!({
            "vector": { "name": vector_name, "vector": vector },
            "limit": limit.max(1),
            "with_payload": true,
            "with_vectors": false
        });
        if let Some(filter) = filter {
            body["filter"] = filter;
        }
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("qdrant search request failed for {url}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("qdrant search failed: {} {}", status, body);
        }
        let parsed: serde_json::Value = resp.json().await?;
        let result = parsed
            .get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let mut out = Vec::new();
        for item in result {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let payload = item.get("payload").cloned().unwrap_or_else(|| json!({}));
            if id.trim().is_empty() {
                continue;
            }
            out.push(QdrantScoredPoint { id, score, payload });
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct QdrantScoredPoint {
    pub id: String,
    pub score: f64,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
struct PayloadIndexSpec {
    field_name: &'static str,
    field_schema: serde_json::Value,
}

fn payload_index_specs() -> Vec<PayloadIndexSpec> {
    vec![
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_SENSOR_ID,
            field_schema: json!("keyword"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_NODE_ID,
            field_schema: json!("keyword"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_SENSOR_TYPE,
            field_schema: json!("keyword"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_UNIT,
            field_schema: json!("keyword"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_EMBEDDING_VERSION,
            field_schema: json!("keyword"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_INTERVAL_SECONDS,
            field_schema: json!("integer"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_WINDOW_SECONDS,
            field_schema: json!("integer"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_IS_DERIVED,
            field_schema: json!("bool"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_IS_PUBLIC_PROVIDER,
            field_schema: json!("bool"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_UPDATED_AT,
            field_schema: json!("keyword"),
        },
        PayloadIndexSpec {
            field_name: TSSE_PAYLOAD_COMPUTED_THROUGH_TS,
            field_schema: json!("keyword"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::collections::HashSet;

    #[test]
    fn payload_index_specs_include_required_fields() {
        let specs = payload_index_specs();
        let fields: HashSet<&str> = specs.iter().map(|s| s.field_name).collect();
        assert!(fields.contains(TSSE_PAYLOAD_SENSOR_ID));
        assert!(fields.contains(TSSE_PAYLOAD_NODE_ID));
        assert!(fields.contains(TSSE_PAYLOAD_SENSOR_TYPE));
        assert!(fields.contains(TSSE_PAYLOAD_UNIT));
        assert!(fields.contains(TSSE_PAYLOAD_EMBEDDING_VERSION));
        assert!(fields.contains(TSSE_PAYLOAD_INTERVAL_SECONDS));
        assert!(fields.contains(TSSE_PAYLOAD_WINDOW_SECONDS));
        assert!(fields.contains(TSSE_PAYLOAD_IS_DERIVED));
        assert!(fields.contains(TSSE_PAYLOAD_IS_PUBLIC_PROVIDER));
        assert!(fields.contains(TSSE_PAYLOAD_UPDATED_AT));
        assert!(fields.contains(TSSE_PAYLOAD_COMPUTED_THROUGH_TS));
        assert_eq!(fields.len(), specs.len());

        let schema_by_field: HashMap<&str, &serde_json::Value> = specs
            .iter()
            .map(|s| (s.field_name, &s.field_schema))
            .collect();
        assert_eq!(
            schema_by_field
                .get(TSSE_PAYLOAD_INTERVAL_SECONDS)
                .and_then(|v| v.as_str()),
            Some("integer")
        );
        assert_eq!(
            schema_by_field
                .get(TSSE_PAYLOAD_IS_DERIVED)
                .and_then(|v| v.as_str()),
            Some("bool")
        );
        assert_eq!(
            schema_by_field
                .get(TSSE_PAYLOAD_IS_PUBLIC_PROVIDER)
                .and_then(|v| v.as_str()),
            Some("bool")
        );
    }
}
