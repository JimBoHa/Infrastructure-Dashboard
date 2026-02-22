use anyhow::Result;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Instant;

use crate::services::analysis::qdrant::QdrantService;

use super::embeddings::{
    TsseEmbeddingConfig, TsseEmbeddingsV1, TSSE_PAYLOAD_EMBEDDING_VERSION,
    TSSE_PAYLOAD_INTERVAL_SECONDS, TSSE_PAYLOAD_IS_DERIVED, TSSE_PAYLOAD_IS_PUBLIC_PROVIDER,
    TSSE_PAYLOAD_NODE_ID, TSSE_PAYLOAD_SENSOR_ID, TSSE_PAYLOAD_SENSOR_TYPE, TSSE_PAYLOAD_UNIT,
};
use super::types::{TsseAnnInfoV1, TsseCandidateFiltersV1, TsseEmbeddingHitV1};

#[derive(Debug, Clone)]
pub struct Candidate {
    pub sensor_id: String,
    pub ann: TsseAnnInfoV1,
}

#[derive(Debug, Clone)]
pub struct FocusSensorMeta {
    pub node_id: Option<String>,
    pub sensor_type: Option<String>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CandidateGenStats {
    pub qdrant_search_ms: u64,
    pub qdrant_requests: u64,
}

pub async fn generate_candidates(
    qdrant: &QdrantService,
    focus_sensor_id: &str,
    focus_meta: &FocusSensorMeta,
    focus_embeddings: &TsseEmbeddingsV1,
    filters: &TsseCandidateFiltersV1,
    min_pool: u32,
    candidate_limit: u32,
    config: &TsseEmbeddingConfig,
) -> Result<Vec<Candidate>> {
    let (candidates, _) = generate_candidates_with_stats(
        qdrant,
        focus_sensor_id,
        focus_meta,
        focus_embeddings,
        filters,
        min_pool,
        candidate_limit,
        config,
    )
    .await?;
    Ok(candidates)
}

pub async fn generate_candidates_with_stats(
    qdrant: &QdrantService,
    focus_sensor_id: &str,
    focus_meta: &FocusSensorMeta,
    focus_embeddings: &TsseEmbeddingsV1,
    filters: &TsseCandidateFiltersV1,
    min_pool: u32,
    candidate_limit: u32,
    config: &TsseEmbeddingConfig,
) -> Result<(Vec<Candidate>, CandidateGenStats)> {
    let target_pool = min_pool.max(candidate_limit).max(10) as usize;
    let mut pool = CandidatePool::new();
    let mut stats = CandidateGenStats::default();

    let plan = build_widen_plan(filters);
    let base_limit = (target_pool / 2).clamp(25, 1000) as u32;

    let mut exclude_set: HashSet<String> = filters
        .exclude_sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    exclude_set.insert(focus_sensor_id.to_string());

    for (stage_idx, stage_filters) in plan.into_iter().enumerate() {
        let vector_limit = (base_limit as usize * (stage_idx + 1)).clamp(25, 2_000) as u32;
        let filter_json = build_qdrant_filter(&stage_filters, focus_meta, config, &exclude_set);
        for (vector_name, vector) in [
            ("value", focus_embeddings.value.clone()),
            ("delta", focus_embeddings.delta.clone()),
            ("event", focus_embeddings.event.clone()),
        ] {
            let search_start = Instant::now();
            let hits = qdrant
                .search_named_vector(vector_name, vector, filter_json.clone(), vector_limit)
                .await?;
            stats.qdrant_search_ms = stats
                .qdrant_search_ms
                .saturating_add(search_start.elapsed().as_millis() as u64);
            stats.qdrant_requests = stats.qdrant_requests.saturating_add(1);
            for (rank, hit) in hits.iter().enumerate() {
                let Some(sensor_id) = sensor_id_from_payload(&hit.payload) else {
                    continue;
                };
                if exclude_set.contains(&sensor_id) {
                    continue;
                }
                pool.record_hit(
                    &sensor_id,
                    stage_idx as u32,
                    stage_filters.clone(),
                    vector_name,
                    (rank + 1) as u32,
                    hit.score,
                );
            }
        }

        if pool.len() >= target_pool {
            break;
        }
    }

    let union_pool_size = pool.len() as u32;
    let mut candidates: Vec<CandidateEntry> = pool.into_entries();
    candidates.sort_by(|a, b| {
        a.widen_stage
            .cmp(&b.widen_stage)
            .then_with(|| b.hits.len().cmp(&a.hits.len()))
            .then_with(|| b.best_score.total_cmp(&a.best_score))
            .then_with(|| a.sensor_id.cmp(&b.sensor_id))
    });

    let out = candidates
        .into_iter()
        .take(candidate_limit as usize)
        .map(|entry| Candidate {
            sensor_id: entry.sensor_id,
            ann: TsseAnnInfoV1 {
                widen_stage: entry.widen_stage,
                union_pool_size,
                embedding_hits: {
                    let mut hits: Vec<TsseEmbeddingHitV1> = entry.hits.into_values().collect();
                    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
                    hits
                },
                filters_applied: entry.filters_applied,
            },
        })
        .collect();

    Ok((out, stats))
}

fn build_widen_plan(filters: &TsseCandidateFiltersV1) -> Vec<TsseCandidateFiltersV1> {
    let mut plan = vec![filters.clone()];
    let mut current = filters.clone();

    // Widen in a "safety-first" order: keep semantic constraints (unit/type) longest,
    // relax node scoping and metadata filters first to grow the pool with minimal semantic drift.
    let relax = |f: &mut TsseCandidateFiltersV1,
                 plan: &mut Vec<TsseCandidateFiltersV1>,
                 apply: fn(&mut TsseCandidateFiltersV1) -> bool| {
        if apply(f) {
            plan.push(f.clone());
        }
    };

    relax(&mut current, &mut plan, |c| {
        if c.same_node_only {
            c.same_node_only = false;
            return true;
        }
        false
    });
    relax(&mut current, &mut plan, |c| {
        if c.interval_seconds.is_some() {
            c.interval_seconds = None;
            return true;
        }
        false
    });
    relax(&mut current, &mut plan, |c| {
        if c.is_derived.is_some() {
            c.is_derived = None;
            return true;
        }
        false
    });
    relax(&mut current, &mut plan, |c| {
        if c.is_public_provider.is_some() {
            c.is_public_provider = None;
            return true;
        }
        false
    });
    relax(&mut current, &mut plan, |c| {
        if c.same_type_only {
            c.same_type_only = false;
            return true;
        }
        false
    });
    relax(&mut current, &mut plan, |c| {
        if c.same_unit_only {
            c.same_unit_only = false;
            return true;
        }
        false
    });

    if plan.len() == 1 {
        let mut relaxed = filters.clone();
        relaxed.same_node_only = false;
        relaxed.same_type_only = false;
        relaxed.same_unit_only = false;
        relaxed.interval_seconds = None;
        relaxed.is_derived = None;
        relaxed.is_public_provider = None;
        plan.push(relaxed);
    }

    plan
}

fn build_qdrant_filter(
    filters: &TsseCandidateFiltersV1,
    focus_meta: &FocusSensorMeta,
    config: &TsseEmbeddingConfig,
    exclude_set: &HashSet<String>,
) -> Option<serde_json::Value> {
    let mut must: Vec<serde_json::Value> = Vec::new();
    must.push(serde_json::json!({
        "key": TSSE_PAYLOAD_EMBEDDING_VERSION,
        "match": { "value": config.version }
    }));

    if filters.same_node_only {
        if let Some(node_id) = &focus_meta.node_id {
            must.push(serde_json::json!({
                "key": TSSE_PAYLOAD_NODE_ID,
                "match": { "value": node_id }
            }));
        }
    }

    if filters.same_unit_only {
        if let Some(unit) = &focus_meta.unit {
            must.push(serde_json::json!({
                "key": TSSE_PAYLOAD_UNIT,
                "match": { "value": unit }
            }));
        }
    }

    if filters.same_type_only {
        if let Some(sensor_type) = &focus_meta.sensor_type {
            must.push(serde_json::json!({
                "key": TSSE_PAYLOAD_SENSOR_TYPE,
                "match": { "value": sensor_type }
            }));
        }
    }

    if let Some(interval_seconds) = filters.interval_seconds {
        must.push(serde_json::json!({
            "key": TSSE_PAYLOAD_INTERVAL_SECONDS,
            "match": { "value": interval_seconds }
        }));
    }

    if let Some(is_derived) = filters.is_derived {
        must.push(serde_json::json!({
            "key": TSSE_PAYLOAD_IS_DERIVED,
            "match": { "value": is_derived }
        }));
    }

    if let Some(is_public_provider) = filters.is_public_provider {
        must.push(serde_json::json!({
            "key": TSSE_PAYLOAD_IS_PUBLIC_PROVIDER,
            "match": { "value": is_public_provider }
        }));
    }

    let mut must_not: Vec<serde_json::Value> = Vec::new();
    for sensor_id in exclude_set.iter() {
        must_not.push(serde_json::json!({
            "key": TSSE_PAYLOAD_SENSOR_ID,
            "match": { "value": sensor_id }
        }));
    }

    if must.is_empty() && must_not.is_empty() {
        return None;
    }

    Some(serde_json::json!({
        "must": must,
        "must_not": must_not,
    }))
}

fn sensor_id_from_payload(payload: &serde_json::Value) -> Option<String> {
    payload
        .get(TSSE_PAYLOAD_SENSOR_ID)
        .and_then(|v| v.as_str())
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .map(|v| v.to_string())
}

#[derive(Debug)]
struct CandidateEntry {
    sensor_id: String,
    widen_stage: u32,
    filters_applied: TsseCandidateFiltersV1,
    hits: BTreeMap<String, TsseEmbeddingHitV1>,
    best_score: f64,
}

#[derive(Debug, Default)]
struct CandidatePool {
    entries: HashMap<String, CandidateEntry>,
}

impl CandidatePool {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn record_hit(
        &mut self,
        sensor_id: &str,
        widen_stage: u32,
        filters_applied: TsseCandidateFiltersV1,
        vector: &str,
        rank: u32,
        score: f64,
    ) {
        let filters_for_entry = filters_applied.clone();
        let entry = self
            .entries
            .entry(sensor_id.to_string())
            .or_insert_with(|| CandidateEntry {
                sensor_id: sensor_id.to_string(),
                widen_stage,
                filters_applied: filters_for_entry,
                hits: BTreeMap::new(),
                best_score: score,
            });

        if entry.hits.get(vector).map(|v| v.rank).unwrap_or(u32::MAX) > rank {
            entry.hits.insert(
                vector.to_string(),
                TsseEmbeddingHitV1 {
                    vector: vector.to_string(),
                    rank,
                    score,
                },
            );
        }

        if score > entry.best_score {
            entry.best_score = score;
        }
        if widen_stage < entry.widen_stage {
            entry.widen_stage = widen_stage;
            entry.filters_applied = filters_applied;
        }
    }

    fn into_entries(self) -> Vec<CandidateEntry> {
        self.entries.into_values().collect()
    }
}

pub fn empty_ann(filters: &TsseCandidateFiltersV1) -> TsseAnnInfoV1 {
    TsseAnnInfoV1 {
        widen_stage: 0,
        union_pool_size: 0,
        embedding_hits: vec![TsseEmbeddingHitV1 {
            vector: "value".to_string(),
            rank: 0,
            score: 0.0,
        }],
        filters_applied: filters.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn widen_plan_relaxes_filters() {
        let filters = TsseCandidateFiltersV1 {
            same_node_only: true,
            same_unit_only: true,
            same_type_only: true,
            interval_seconds: Some(60),
            is_derived: Some(true),
            is_public_provider: Some(false),
            exclude_sensor_ids: vec![],
        };
        let plan = build_widen_plan(&filters);
        assert!(plan.len() >= 2);
        let last = plan.last().unwrap();
        assert!(!last.same_node_only);
        assert!(!last.same_unit_only);
        assert!(!last.same_type_only);
        assert!(last.interval_seconds.is_none());
        assert!(last.is_derived.is_none());
        assert!(last.is_public_provider.is_none());
    }

    #[test]
    fn widen_plan_relaxes_optional_filters_in_order() {
        let filters = TsseCandidateFiltersV1 {
            same_node_only: true,
            same_unit_only: true,
            same_type_only: true,
            interval_seconds: Some(60),
            is_derived: Some(true),
            is_public_provider: Some(false),
            exclude_sensor_ids: vec![],
        };
        let plan = build_widen_plan(&filters);
        assert_eq!(plan.len(), 7);

        assert!(plan[0].same_node_only);
        assert!(plan[0].interval_seconds.is_some());
        assert!(plan[0].is_derived.is_some());
        assert!(plan[0].is_public_provider.is_some());
        assert!(plan[0].same_type_only);
        assert!(plan[0].same_unit_only);

        assert!(!plan[1].same_node_only);
        assert!(plan[1].interval_seconds.is_some());
        assert!(plan[1].is_derived.is_some());
        assert!(plan[1].is_public_provider.is_some());
        assert!(plan[1].same_type_only);
        assert!(plan[1].same_unit_only);

        assert!(!plan[2].same_node_only);
        assert!(plan[2].interval_seconds.is_none());
        assert!(plan[2].is_derived.is_some());
        assert!(plan[2].is_public_provider.is_some());
        assert!(plan[2].same_type_only);
        assert!(plan[2].same_unit_only);

        assert!(plan[3].interval_seconds.is_none());
        assert!(plan[3].is_derived.is_none());
        assert!(plan[3].is_public_provider.is_some());
        assert!(plan[3].same_type_only);
        assert!(plan[3].same_unit_only);

        assert!(plan[4].interval_seconds.is_none());
        assert!(plan[4].is_derived.is_none());
        assert!(plan[4].is_public_provider.is_none());
        assert!(plan[4].same_type_only);
        assert!(plan[4].same_unit_only);

        assert!(plan[5].interval_seconds.is_none());
        assert!(plan[5].is_derived.is_none());
        assert!(plan[5].is_public_provider.is_none());
        assert!(!plan[5].same_type_only);
        assert!(plan[5].same_unit_only);

        assert!(plan[6].interval_seconds.is_none());
        assert!(plan[6].is_derived.is_none());
        assert!(plan[6].is_public_provider.is_none());
        assert!(!plan[6].same_type_only);
        assert!(!plan[6].same_unit_only);
    }

    #[test]
    fn candidate_pool_tracks_best_hit_and_stage() {
        let mut pool = CandidatePool::new();
        let filters = TsseCandidateFiltersV1::default();
        pool.record_hit("sensor-1", 1, filters.clone(), "value", 5, 0.4);
        pool.record_hit("sensor-1", 0, filters.clone(), "event", 2, 0.6);
        let entries = pool.into_entries();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.widen_stage, 0);
        assert!(entry.best_score >= 0.6);
        assert_eq!(entry.hits.len(), 2);
    }

    #[test]
    fn qdrant_filter_includes_optional_fields() {
        let filters = TsseCandidateFiltersV1 {
            same_node_only: false,
            same_unit_only: false,
            same_type_only: false,
            interval_seconds: Some(60),
            is_derived: Some(true),
            is_public_provider: Some(false),
            exclude_sensor_ids: vec![],
        };
        let focus_meta = FocusSensorMeta {
            node_id: None,
            sensor_type: None,
            unit: None,
        };
        let config = TsseEmbeddingConfig::default();
        let filter = build_qdrant_filter(&filters, &focus_meta, &config, &HashSet::new()).unwrap();
        let must = filter
            .get("must")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let keys: HashSet<String> = must
            .iter()
            .filter_map(|v| v.get("key").and_then(|k| k.as_str()).map(|s| s.to_string()))
            .collect();
        assert!(keys.contains(TSSE_PAYLOAD_INTERVAL_SECONDS));
        assert!(keys.contains(TSSE_PAYLOAD_IS_DERIVED));
        assert!(keys.contains(TSSE_PAYLOAD_IS_PUBLIC_PROVIDER));
    }

    #[test]
    fn sensor_id_from_payload_reads_sensor_id() {
        let payload = json!({ TSSE_PAYLOAD_SENSOR_ID: "sensor-abc" });
        assert_eq!(
            sensor_id_from_payload(&payload).as_deref(),
            Some("sensor-abc")
        );

        let missing = json!({});
        assert!(sensor_id_from_payload(&missing).is_none());
    }
}
