use super::cooccurrence_v1;
use super::event_match_v1;
use super::runner::JobFailure;
use super::store;
use super::types::{AnalysisJobError, AnalysisJobProgress, AnalysisJobRow};
use crate::services::analysis::lake::AnalysisLakeConfig;
use crate::services::analysis::parquet_duckdb::{
    bucket_coverage_pct, DuckDbQueryService, MetricsBucketReadOptions,
};
use crate::services::analysis::signal_semantics;
use crate::services::analysis::tsse::types::{
    AdaptiveThresholdConfigV1, CooccurrenceBucketPreferenceModeV1, CooccurrenceBucketV1,
    CooccurrenceJobParamsV1, CooccurrenceResultV1, CooccurrenceScoreModeV1,
    DeseasonModeV1, DirectionLabelV1, EventDetectorModeV1,
    EventEvidenceMonitoringV1, EventMatchJobParamsV1, EventMatchLagScoreV1, EventMatchResultV1,
    EventPolarityV1,
    EventSuppressionModeV1, EventThresholdModeV1, RelatedSensorsUnifiedCandidateV2,
    RelatedSensorsUnifiedEvidenceV2, RelatedSensorsUnifiedJobParamsV2,
    RelatedSensorsUnifiedLimitsUsedV2, RelatedSensorsUnifiedResultV2,
    RelatedSensorsUnifiedStabilityV1,
    RelatedSensorsUnifiedSkippedCandidateV2, SystemWideCooccurrenceBucketV1, TsseCandidateFiltersV1,
    UnifiedCandidateSkipReasonV2, UnifiedCandidateSourceV2, UnifiedConfidenceTierV2,
    UnifiedEvidenceSourceV1, UnifiedRelationshipModeV2, UnifiedStabilityStatusV1,
    UnifiedStabilityTierV1,
    UnifiedStrategyWeightsV2,
};
use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use sqlx::types::Json as SqlJson;
use sqlx::PgPool;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use xxhash_rust::xxh3::xxh3_64_with_seed;

const MAX_DERIVED_DEPENDENCY_DEPTH: usize = 10;
const MAX_DERIVED_DEPENDENCY_PATH_LEN: usize = 8;
const MAX_DERIVED_DEPENDENCY_VISITED: usize = 5000;
const MIN_COVERAGE_BUCKET_ROWS: u64 = 3;
const MIN_COVERAGE_DELTA_COUNT: u64 = 3;
const COVERAGE_PREFILTER_BATCH_SIZE: usize = 250;
const STABILITY_TOP_K: usize = 10;
const STABILITY_MAX_ELIGIBLE: usize = 120;

#[derive(sqlx::FromRow, Clone)]
struct SensorMetaRow {
    sensor_id: String,
    node_id: uuid::Uuid,
    sensor_type: String,
    unit: String,
    interval_seconds: i32,
    source: Option<String>,
}

#[derive(sqlx::FromRow, Clone)]
struct SensorConfigRow {
    sensor_id: String,
    config: Option<SqlJson<serde_json::Value>>,
}

#[derive(Default)]
struct CooccurrenceAggregate {
    score_sum: f64,
    count: u64,
    max_z: f64,
    timestamps: Vec<i64>,
}

#[derive(Default)]
struct UnifiedAccumulator {
    events_score: Option<f64>,
    events_overlap: Option<u64>,
    n_focus: Option<u64>,
    n_candidate: Option<u64>,
    n_focus_up: Option<u64>,
    n_focus_down: Option<u64>,
    n_candidate_up: Option<u64>,
    n_candidate_down: Option<u64>,
    best_lag_sec: Option<i64>,
    top_lags: Option<Vec<EventMatchLagScoreV1>>,
    direction_label: Option<DirectionLabelV1>,
    sign_agreement: Option<f64>,
    delta_corr: Option<f64>,
    direction_n: Option<u64>,
    time_of_day_entropy_norm: Option<f64>,
    time_of_day_entropy_weight: Option<f64>,
    episodes: Option<Vec<crate::services::analysis::tsse::types::TsseEpisodeV1>>,
    why_ranked: Option<crate::services::analysis::tsse::types::TsseWhyRankedV1>,
    cooccurrence_score: Option<f64>,
    cooccurrence_avg: Option<f64>,
    cooccurrence_surprise: Option<f64>,
    cooccurrence_count: Option<u64>,
    cooccurrence_timestamps: Option<Vec<i64>>,
}

fn stable_candidate_order_seed(focus_sensor_id: &str, job_key: &str) -> u64 {
    let focus_hash = xxh3_64_with_seed(focus_sensor_id.trim().as_bytes(), 0);
    xxh3_64_with_seed(job_key.trim().as_bytes(), focus_hash)
}

fn candidate_priority_group(focus: &SensorMetaRow, candidate: &SensorMetaRow) -> u8 {
    if candidate.node_id == focus.node_id {
        0
    } else if candidate.unit == focus.unit {
        1
    } else if candidate.sensor_type == focus.sensor_type {
        2
    } else {
        3
    }
}

fn deterministic_candidate_order(
    focus: &SensorMetaRow,
    candidates: &[SensorMetaRow],
    seed: u64,
) -> Vec<String> {
    let mut keyed: Vec<(u8, u64, &str)> = candidates
        .iter()
        .map(|row| {
            (
                candidate_priority_group(focus, row),
                xxh3_64_with_seed(row.sensor_id.as_bytes(), seed),
                row.sensor_id.as_str(),
            )
        })
        .collect();

    keyed.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.cmp(b.2))
    });

    keyed.into_iter().map(|(_, _, id)| id.to_string()).collect()
}

fn candidate_matches_filters(focus: &SensorMetaRow, row: &SensorMetaRow, filters: &TsseCandidateFiltersV1) -> bool {
    if filters.same_node_only && row.node_id != focus.node_id {
        return false;
    }
    if filters.same_unit_only && row.unit != focus.unit {
        return false;
    }
    if filters.same_type_only && row.sensor_type != focus.sensor_type {
        return false;
    }
    if let Some(interval_seconds) = filters.interval_seconds {
        if row.interval_seconds as i64 != interval_seconds {
            return false;
        }
    }
    if let Some(is_derived) = filters.is_derived {
        let derived = row.source.as_deref() == Some("derived");
        if derived != is_derived {
            return false;
        }
    }
    if let Some(is_public_provider) = filters.is_public_provider {
        let provider = row.source.as_deref() == Some("forecast_points");
        if provider != is_public_provider {
            return false;
        }
    }
    if filters.exclude_sensor_ids.iter().any(|id| id == &row.sensor_id) {
        return false;
    }

    true
}

fn should_query_all_candidates(params: &RelatedSensorsUnifiedJobParamsV2) -> bool {
    match params.candidate_source {
        Some(UnifiedCandidateSourceV2::AllSensorsInScope) => true,
        Some(UnifiedCandidateSourceV2::VisibleInTrends) => false,
        None => params.candidate_sensor_ids.is_empty(),
    }
}

fn compute_candidate_limit_requested_base(
    mode: UnifiedRelationshipModeV2,
    quick_suggest: bool,
    candidate_limit: Option<u32>,
) -> usize {
    let mut requested = candidate_limit
        .unwrap_or(if quick_suggest { 80 } else { 200 })
        .clamp(10, 1000) as usize;
    if matches!(mode, UnifiedRelationshipModeV2::Simple) && !quick_suggest {
        requested = requested.min(300);
    }
    requested
}

fn compute_candidate_limit_used(
    mode: UnifiedRelationshipModeV2,
    quick_suggest: bool,
    candidate_limit: Option<u32>,
    pinned_count: usize,
    eligible_count: usize,
    evaluate_all_eligible: bool,
) -> (usize, bool) {
    let requested_base = compute_candidate_limit_requested_base(mode, quick_suggest, candidate_limit);
    let evaluate_all_effective = evaluate_all_eligible;

    let requested_effective = if evaluate_all_effective {
        eligible_count
    } else {
        requested_base
    };

    let used = if evaluate_all_effective {
        requested_effective.max(pinned_count)
    } else {
        requested_effective.max(pinned_count).clamp(10, 1000)
    };

    (used, evaluate_all_effective)
}

fn eligible_candidate_count(candidate_rows: &[SensorMetaRow], pinned_rows: &[SensorMetaRow]) -> usize {
    let mut seen: HashSet<&str> = HashSet::new();
    for row in candidate_rows {
        seen.insert(row.sensor_id.as_str());
    }
    for row in pinned_rows {
        seen.insert(row.sensor_id.as_str());
    }
    seen.len()
}

async fn apply_min_coverage_prefilter(
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    interval_seconds: i64,
    gap_max_buckets: i64,
    ordered_candidate_sensor_ids: &[String],
    candidate_limit: usize,
    cancel: &CancellationToken,
) -> Result<(Vec<String>, Vec<String>, Vec<String>), JobFailure> {
    if ordered_candidate_sensor_ids.is_empty() {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    }

    let mut accepted: Vec<String> = Vec::new();
    let mut prefiltered: Vec<String> = Vec::new();
    let mut cursor: usize = 0;

    while cursor < ordered_candidate_sensor_ids.len() && accepted.len() < candidate_limit {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }

        let batch_end = (cursor + COVERAGE_PREFILTER_BATCH_SIZE).min(ordered_candidate_sensor_ids.len());
        let batch: Vec<String> = ordered_candidate_sensor_ids[cursor..batch_end].to_vec();
        let stats = duckdb
            .read_bucket_coverage_stats_from_lake_with_options(
                lake,
                start,
                end,
                batch.clone(),
                interval_seconds,
                gap_max_buckets,
                MetricsBucketReadOptions::analysis_default(),
            )
            .await
            .map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "coverage_prefilter_failed".to_string(),
                    message: err.to_string(),
                    details: None,
                })
            })?;

        let mut stats_by_id: HashMap<String, (u64, u64)> = HashMap::new();
        for row in stats {
            stats_by_id.insert(row.sensor_id, (row.bucket_rows, row.delta_count));
        }

        for (idx, sensor_id) in batch.iter().enumerate() {
            let (bucket_rows, delta_count) = stats_by_id.get(sensor_id).copied().unwrap_or((0, 0));
            if bucket_rows >= MIN_COVERAGE_BUCKET_ROWS && delta_count >= MIN_COVERAGE_DELTA_COUNT {
                accepted.push(sensor_id.clone());
            } else {
                prefiltered.push(sensor_id.clone());
            }

            if accepted.len() >= candidate_limit {
                let trunc_start = cursor + idx + 1;
                let truncated = ordered_candidate_sensor_ids
                    .get(trunc_start..)
                    .unwrap_or(&[])
                    .to_vec();
                return Ok((accepted, prefiltered, truncated));
            }
        }

        cursor = batch_end;
    }

    Ok((accepted, prefiltered, Vec::new()))
}

fn derived_input_ids_from_config(config: &serde_json::Value) -> Vec<String> {
    let source = config
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if source != "derived" {
        return Vec::new();
    }

    let Some(derived) = config.get("derived").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let Some(inputs) = derived.get("inputs").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut out: Vec<String> = Vec::new();
    for entry in inputs {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        let sensor_id = obj
            .get("sensor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if sensor_id.is_empty() {
            continue;
        }
        out.push(sensor_id.to_string());
    }
    out.sort();
    out.dedup();
    out
}

fn cap_dependency_path(mut path: Vec<String>, max_len: usize) -> Vec<String> {
    if max_len == 0 || path.is_empty() {
        return Vec::new();
    }
    if path.len() <= max_len {
        return path;
    }

    let focus = path.last().cloned();
    path.truncate(max_len.saturating_sub(1));
    if let Some(focus) = focus {
        path.push(focus);
    }
    path
}

fn find_derived_dependency_path(
    candidate_sensor_id: &str,
    focus_sensor_id: &str,
    derived_inputs: &HashMap<String, Vec<String>>,
    max_depth: usize,
    max_visited: usize,
) -> Option<Vec<String>> {
    if max_depth == 0 {
        return None;
    }

    let candidate = candidate_sensor_id.trim();
    let focus = focus_sensor_id.trim();
    if candidate.is_empty() || focus.is_empty() {
        return None;
    }
    if candidate == focus {
        return None;
    }

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(candidate.to_string());

    let mut queue: VecDeque<Vec<String>> = VecDeque::new();
    queue.push_back(vec![candidate.to_string()]);

    while let Some(path) = queue.pop_front() {
        let depth_edges = path.len().saturating_sub(1);
        if depth_edges >= max_depth {
            continue;
        }
        let Some(node) = path.last() else {
            continue;
        };
        let Some(inputs) = derived_inputs.get(node) else {
            continue;
        };

        for input_id in inputs {
            if input_id == focus {
                let mut out = path.clone();
                out.push(input_id.clone());
                return Some(out);
            }
            if visited.len() >= max_visited {
                continue;
            }
            if !visited.insert(input_id.clone()) {
                continue;
            }
            let mut next = path.clone();
            next.push(input_id.clone());
            queue.push_back(next);
        }
    }

    None
}

async fn compute_derived_dependency_paths(
    db: &PgPool,
    focus_sensor_id: &str,
    candidate_sensor_ids: &[String],
) -> Result<HashMap<String, Vec<String>>, JobFailure> {
    if focus_sensor_id.trim().is_empty() || candidate_sensor_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut derived_inputs: HashMap<String, Vec<String>> = HashMap::new();

    let mut visited: HashSet<String> = HashSet::new();
    let mut frontier: HashSet<String> = candidate_sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();

    for _depth in 0..MAX_DERIVED_DEPENDENCY_DEPTH {
        if frontier.is_empty() {
            break;
        }

        let mut to_fetch: Vec<String> = Vec::new();
        for sensor_id in frontier.drain() {
            if visited.len() >= MAX_DERIVED_DEPENDENCY_VISITED {
                break;
            }
            if visited.insert(sensor_id.clone()) {
                to_fetch.push(sensor_id);
            }
        }

        if to_fetch.is_empty() {
            break;
        }

        let rows: Vec<SensorConfigRow> = sqlx::query_as(
            r#"
            SELECT sensor_id, config
            FROM sensors
            WHERE sensor_id = ANY($1) AND deleted_at IS NULL
            "#,
        )
        .bind(&to_fetch)
        .fetch_all(db)
        .await
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "derived_dependency_lookup_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;

        for row in rows {
            let config = row
                .config
                .map(|c| c.0)
                .unwrap_or_else(|| serde_json::json!({}));
            let inputs = derived_input_ids_from_config(&config);
            if inputs.is_empty() {
                continue;
            }
            for input_id in inputs.iter() {
                if visited.len() >= MAX_DERIVED_DEPENDENCY_VISITED {
                    break;
                }
                if !visited.contains(input_id) {
                    frontier.insert(input_id.clone());
                }
            }
            derived_inputs.insert(row.sensor_id, inputs);
        }
    }

    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for candidate_id in candidate_sensor_ids.iter() {
        let Some(path) = find_derived_dependency_path(
            candidate_id,
            focus_sensor_id,
            &derived_inputs,
            MAX_DERIVED_DEPENDENCY_DEPTH,
            MAX_DERIVED_DEPENDENCY_VISITED,
        ) else {
            continue;
        };
        out.insert(
            candidate_id.clone(),
            cap_dependency_path(path, MAX_DERIVED_DEPENDENCY_PATH_LEN),
        );
    }

    Ok(out)
}

pub async fn execute(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
) -> std::result::Result<serde_json::Value, JobFailure> {
    let mut params: RelatedSensorsUnifiedJobParamsV2 = serde_json::from_value(job.params.0.clone())
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: err.to_string(),
                details: None,
            })
        })?;

    let focus_sensor_id = params.focus_sensor_id.trim().to_string();
    if focus_sensor_id.is_empty() {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "focus_sensor_id is required".to_string(),
            details: None,
        }));
    }

    let start = DateTime::parse_from_rfc3339(params.start.trim())
        .map_err(|_| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: "Invalid start timestamp".to_string(),
                details: None,
            })
        })?
        .with_timezone(&Utc);

    let end_inclusive = DateTime::parse_from_rfc3339(params.end.trim())
        .map_err(|_| {
            JobFailure::Failed(AnalysisJobError {
                code: "invalid_params".to_string(),
                message: "Invalid end timestamp".to_string(),
                details: None,
            })
        })?
        .with_timezone(&Utc);

    if end_inclusive <= start {
        return Err(JobFailure::Failed(AnalysisJobError {
            code: "invalid_params".to_string(),
            message: "end must be after start".to_string(),
            details: None,
        }));
    }
    let end_exclusive = end_inclusive + Duration::microseconds(1);

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let mode = params.mode.unwrap_or(UnifiedRelationshipModeV2::Simple);
    let quick_suggest = params.quick_suggest.unwrap_or(false);
    let query_all_candidates = should_query_all_candidates(&params);

    let max_results = params
        .max_results
        .unwrap_or(if quick_suggest { 20 } else { 60 })
        .clamp(5, 300) as usize;

    let focus_meta = fetch_focus_sensor_meta(db, &focus_sensor_id).await?;
    let mut interval_seconds = params.interval_seconds.unwrap_or(60).max(1);
    let horizon_seconds = (end_inclusive - start).num_seconds().max(1);
    let max_buckets = if quick_suggest { 8_000 } else { 16_000 };
    let expected_buckets = (horizon_seconds as f64 / interval_seconds as f64).ceil() as i64;
    if expected_buckets > max_buckets {
        interval_seconds = ((horizon_seconds as f64) / (max_buckets as f64)).ceil() as i64;
    }
    params.interval_seconds = Some(interval_seconds);
    let gap_max_buckets = params.gap_max_buckets.unwrap_or(5).max(0);

    let mut pinned_requested_ids: Vec<String> = params
        .pinned_sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty() && id != &focus_sensor_id)
        .collect();
    pinned_requested_ids.sort();
    pinned_requested_ids.dedup();

    let mut pinned_rows: Vec<SensorMetaRow> = if !pinned_requested_ids.is_empty() {
        fetch_sensor_meta_rows(db, &pinned_requested_ids).await?
    } else {
        Vec::new()
    };
    pinned_rows.retain(|row| {
        let id = row.sensor_id.trim();
        !id.is_empty() && id != focus_sensor_id
    });

    let mut requested_candidate_ids: Vec<String> = params
        .candidate_sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty() && id != &focus_sensor_id)
        .collect();
    requested_candidate_ids.sort();
    requested_candidate_ids.dedup();

    let mut candidate_rows: Vec<SensorMetaRow> = if !query_all_candidates && !requested_candidate_ids.is_empty() {
        fetch_sensor_meta_rows(db, &requested_candidate_ids).await?
    } else {
        fetch_candidate_sensor_meta_rows_by_filters(db, &focus_meta, &params.filters).await?
    };

    candidate_rows.retain(|row| {
        let id = row.sensor_id.trim();
        !id.is_empty()
            && id != focus_sensor_id
            && candidate_matches_filters(&focus_meta, row, &params.filters)
    });

    let eligible_count = eligible_candidate_count(&candidate_rows, &pinned_rows);

    let mut skipped_candidates: Vec<RelatedSensorsUnifiedSkippedCandidateV2> = Vec::new();
    let mut provider_candidate_ids: HashSet<String> = HashSet::new();
    for row in &candidate_rows {
        if row.source.as_deref() == Some("forecast_points") {
            provider_candidate_ids.insert(row.sensor_id.clone());
        }
    }
    for row in &pinned_rows {
        if row.source.as_deref() == Some("forecast_points") {
            provider_candidate_ids.insert(row.sensor_id.clone());
        }
    }
    if !provider_candidate_ids.is_empty() {
        for sensor_id in provider_candidate_ids.iter() {
            skipped_candidates.push(RelatedSensorsUnifiedSkippedCandidateV2 {
                sensor_id: sensor_id.clone(),
                reason: UnifiedCandidateSkipReasonV2::NoLakeHistory,
            });
        }
        skipped_candidates.sort_by(|a, b| a.sensor_id.cmp(&b.sensor_id));
        candidate_rows.retain(|row| row.source.as_deref() != Some("forecast_points"));
        pinned_rows.retain(|row| row.source.as_deref() != Some("forecast_points"));
    }

    let (candidate_limit_used, evaluate_all_eligible_effective) = compute_candidate_limit_used(
        mode,
        quick_suggest,
        params.candidate_limit,
        pinned_rows.len(),
        eligible_count,
        params.evaluate_all_eligible.unwrap_or(false),
    );
    let max_sensors_used = ((candidate_limit_used + 1) as u32).clamp(2, 10_000);

    let seed = stable_candidate_order_seed(&focus_sensor_id, job.job_key.as_deref().unwrap_or(""));
    let mut pinned_included_ids = deterministic_candidate_order(&focus_meta, &pinned_rows, seed);
    let mut pinned_truncated_ids: Vec<String> = Vec::new();
        if pinned_included_ids.len() > candidate_limit_used {
            pinned_truncated_ids = pinned_included_ids.split_off(candidate_limit_used);
        }
    let pinned_included: HashSet<String> = pinned_included_ids.iter().cloned().collect();

    let non_pinned_rows: Vec<SensorMetaRow> = candidate_rows
        .into_iter()
        .filter(|row| !pinned_included.contains(&row.sensor_id))
        .collect();
    let ordered_candidate_sensor_ids = deterministic_candidate_order(&focus_meta, &non_pinned_rows, seed);

    let remaining_slots = candidate_limit_used.saturating_sub(pinned_included_ids.len());
    let start_prefilter = Instant::now();
    let (non_pinned_candidate_sensor_ids, prefiltered, truncated) = if remaining_slots == 0 {
        (Vec::new(), Vec::new(), ordered_candidate_sensor_ids.clone())
    } else if evaluate_all_eligible_effective {
        if ordered_candidate_sensor_ids.len() > remaining_slots {
            let mut selected = ordered_candidate_sensor_ids.clone();
            let truncated = selected.split_off(remaining_slots);
            (selected, Vec::new(), truncated)
        } else {
            (ordered_candidate_sensor_ids.clone(), Vec::new(), Vec::new())
        }
    } else {
        apply_min_coverage_prefilter(
            duckdb,
            lake,
            start,
            end_exclusive,
            interval_seconds,
            gap_max_buckets,
            &ordered_candidate_sensor_ids,
            remaining_slots,
            &cancel,
        )
        .await?
    };
    let prefilter_ms = start_prefilter.elapsed().as_millis() as u64;
    let prefiltered_candidate_sensor_ids = prefiltered;
    let mut truncated_candidate_sensor_ids: Vec<String> = Vec::new();
    truncated_candidate_sensor_ids.extend(pinned_truncated_ids.clone());
    truncated_candidate_sensor_ids.extend(truncated);

    let mut candidate_sensor_ids: Vec<String> =
        Vec::with_capacity(pinned_included_ids.len() + non_pinned_candidate_sensor_ids.len());
    candidate_sensor_ids.extend(pinned_included_ids.clone());
    candidate_sensor_ids.extend(non_pinned_candidate_sensor_ids);

    if candidate_sensor_ids.is_empty() {
        let pinned_requested_count = pinned_requested_ids.len() as u64;
        let pinned_included_count = pinned_included_ids.len() as u64;
        let pinned_truncated_count = pinned_truncated_ids.len() as u64;
        let prefiltered_count = prefiltered_candidate_sensor_ids.len() as u64;
        let truncated_count = truncated_candidate_sensor_ids.len() as u64;
        let stability = if params.stability_enabled.unwrap_or(false) {
            Some(RelatedSensorsUnifiedStabilityV1 {
                status: UnifiedStabilityStatusV1::Skipped,
                k: STABILITY_TOP_K as u32,
                window_count: 3,
                score: None,
                tier: None,
                overlaps: Vec::new(),
                reason: Some("No candidates evaluated".to_string()),
            })
        } else {
            None
        };
        let empty = RelatedSensorsUnifiedResultV2 {
            job_type: "related_sensors_unified_v2".to_string(),
            focus_sensor_id,
            evidence_source: Some(if params.focus_events.is_empty() {
                UnifiedEvidenceSourceV1::DeltaZ
            } else {
                UnifiedEvidenceSourceV1::Pattern
            }),
            computed_through_ts: None,
            interval_seconds: params.interval_seconds,
            bucket_count: None,
            params,
            limits_used: RelatedSensorsUnifiedLimitsUsedV2 {
                candidate_limit_used: candidate_limit_used as u32,
                max_results_used: max_results as u32,
                max_sensors_used,
            },
            candidates: Vec::new(),
            skipped_candidates,
            system_wide_buckets: Vec::new(),
            prefiltered_candidate_sensor_ids,
            truncated_candidate_sensor_ids,
            truncated_result_sensor_ids: Vec::new(),
            timings_ms: BTreeMap::new(),
            counts: BTreeMap::from([
                ("candidate_pool".to_string(), 0),
                ("eligible_count".to_string(), eligible_count as u64),
                ("evaluated_count".to_string(), 0),
                ("ranked".to_string(), 0),
                (
                    "evaluate_all_eligible_effective".to_string(),
                    if evaluate_all_eligible_effective { 1 } else { 0 },
                ),
                ("pinned_requested".to_string(), pinned_requested_count),
                ("pinned_included".to_string(), pinned_included_count),
                ("pinned_truncated".to_string(), pinned_truncated_count),
                ("candidate_prefiltered".to_string(), prefiltered_count),
                ("candidate_truncated".to_string(), truncated_count),
            ]),
            versions: BTreeMap::from([("unified".to_string(), "v2".to_string())]),
            monitoring: None,
            stability,
            gap_skipped_deltas: BTreeMap::new(),
        };
        return serde_json::to_value(&empty)
            .context("failed to serialize related_sensors_unified_v2 empty result")
            .map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "result_encode_failed".to_string(),
                    message: err.to_string(),
                    details: None,
                })
            });
    }

    let derived_dependency_paths =
        compute_derived_dependency_paths(db, &focus_sensor_id, &candidate_sensor_ids).await?;

    let mut progress = AnalysisJobProgress {
        phase: "unified_prepare".to_string(),
        completed: 0,
        total: Some(3),
        message: Some("Preparing related sensor analysis".to_string()),
    };
    let _ = store::update_progress(db, job.id, &progress).await;

    let polarity = params.polarity.unwrap_or(EventPolarityV1::Both);
    let z_threshold = params
        .z_threshold
        .unwrap_or(if quick_suggest { 3.5 } else { 3.0 });
    let threshold_mode = params
        .threshold_mode
        .unwrap_or(EventThresholdModeV1::FixedZ);
    params.threshold_mode = Some(threshold_mode);
    let adaptive_threshold: Option<AdaptiveThresholdConfigV1> = params.adaptive_threshold.clone();
    let detector_mode = params
        .detector_mode
        .unwrap_or(EventDetectorModeV1::BucketDeltas);
    params.detector_mode = Some(detector_mode);
    let suppression_mode = params
        .suppression_mode
        .unwrap_or(EventSuppressionModeV1::NmsWindow);
    params.suppression_mode = Some(suppression_mode);
    let exclude_boundary_events = params.exclude_boundary_events.unwrap_or(false);
    params.exclude_boundary_events = Some(exclude_boundary_events);
    let sparse_point_events_enabled = params.sparse_point_events_enabled.unwrap_or(false);
    params.sparse_point_events_enabled = Some(sparse_point_events_enabled);
    let z_cap = params.z_cap.unwrap_or(15.0).clamp(1.0, 1_000.0);
    let min_separation_buckets = params.min_separation_buckets.unwrap_or(2).max(0);
    let max_lag_buckets = params
        .max_lag_buckets
        .unwrap_or(if quick_suggest { 8 } else { 12 });
    let max_events = params
        .max_events
        .unwrap_or(if quick_suggest { 1200 } else { 2000 });
    let max_episodes = params
        .max_episodes
        .unwrap_or(if quick_suggest { 12 } else { 24 });
    let episode_gap_buckets = params.episode_gap_buckets.unwrap_or(6).max(1);
    let tolerance_buckets = params
        .tolerance_buckets
        .unwrap_or(if quick_suggest { 1 } else { 2 });
    let min_sensors = params.min_sensors.unwrap_or(2).max(2);
    let include_delta_corr_signal = match (mode, params.include_delta_corr_signal) {
        (_, Some(value)) => value,
        (UnifiedRelationshipModeV2::Simple, None) => {
            signal_semantics::is_level_like_sensor_type(&focus_meta.sensor_type)
        }
        (UnifiedRelationshipModeV2::Advanced, None) => false,
    };
    params.include_delta_corr_signal = Some(include_delta_corr_signal);
    let weights = normalize_weights(params.weights.clone(), include_delta_corr_signal);
    params.weights = Some(weights.clone());
    let deseason_mode = params.deseason_mode.unwrap_or(DeseasonModeV1::None);
    let periodic_penalty_enabled = params
        .periodic_penalty_enabled
        .unwrap_or(matches!(deseason_mode, DeseasonModeV1::None));
    params.deseason_mode = Some(deseason_mode);
    params.periodic_penalty_enabled = Some(periodic_penalty_enabled);
    let cooccurrence_score_mode = params
        .cooccurrence_score_mode
        .unwrap_or(CooccurrenceScoreModeV1::AvgProduct);
    params.cooccurrence_score_mode = Some(cooccurrence_score_mode);
    let cooccurrence_bucket_preference_mode = params
        .cooccurrence_bucket_preference_mode
        .unwrap_or(CooccurrenceBucketPreferenceModeV1::PreferSpecificMatches);
    params.cooccurrence_bucket_preference_mode = Some(cooccurrence_bucket_preference_mode);
    let deseasoning_applied =
        matches!(deseason_mode, DeseasonModeV1::HourOfDayMean) && horizon_seconds >= 2 * 86_400;
    let deseasoning_skipped_insufficient_window =
        matches!(deseason_mode, DeseasonModeV1::HourOfDayMean) && !deseasoning_applied;

    let event_params = EventMatchJobParamsV1 {
        focus_sensor_id: focus_sensor_id.clone(),
        start: params.start.clone(),
        end: params.end.clone(),
        focus_events: params.focus_events.clone(),
        interval_seconds: Some(interval_seconds),
        candidate_sensor_ids: candidate_sensor_ids.clone(),
        candidate_limit: Some(candidate_limit_used as u32),
        max_buckets: Some(if quick_suggest { 8_000 } else { 16_000 }),
        max_events: Some(max_events),
        z_threshold: Some(z_threshold),
        threshold_mode: Some(threshold_mode),
        adaptive_threshold,
        detector_mode: Some(detector_mode),
        suppression_mode: Some(suppression_mode),
        exclude_boundary_events: Some(exclude_boundary_events),
        sparse_point_events_enabled: Some(sparse_point_events_enabled),
        min_separation_buckets: Some(min_separation_buckets),
        max_lag_buckets: Some(max_lag_buckets),
        top_k_lags: if matches!(mode, UnifiedRelationshipModeV2::Advanced) {
            Some(3)
        } else {
            None
        },
        tolerance_buckets: Some(tolerance_buckets),
        max_episodes: Some(max_episodes),
        episode_gap_buckets: Some(episode_gap_buckets),
        gap_max_buckets: Some(gap_max_buckets),
        polarity: Some(polarity),
        z_cap: Some(z_cap),
        deseason_mode: Some(deseason_mode),
        periodic_penalty_enabled: Some(periodic_penalty_enabled),
        filters: params.filters.clone(),
    };
    let event_params_base = event_params.clone();

    progress.phase = "events".to_string();
    progress.completed = 1;
    progress.message = Some("Running event alignment".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let start_events = Instant::now();
    let event_job = rewritten_job(job, "event_match_v1", serde_json::json!(event_params));
    let event_value = event_match_v1::execute(db, duckdb, lake, &event_job, cancel.clone()).await?;
    let event_result: EventMatchResultV1 = serde_json::from_value(event_value).map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "result_decode_failed".to_string(),
            message: format!("Failed to decode event result: {err}"),
            details: None,
        })
    })?;
    let events_ms = start_events.elapsed().as_millis() as u64;

    progress.phase = "cooccurrence".to_string();
    progress.completed = 2;
    progress.message = Some("Running co-occurrence analysis".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let mut co_sensor_ids: Vec<String> = Vec::with_capacity(candidate_sensor_ids.len() + 1);
    co_sensor_ids.push(focus_sensor_id.clone());
    let co_candidate_max = (max_sensors_used as usize).saturating_sub(1);
    co_sensor_ids.extend(candidate_sensor_ids.iter().take(co_candidate_max).cloned());
    co_sensor_ids.sort();
    co_sensor_ids.dedup();

    let coocc_params = CooccurrenceJobParamsV1 {
        sensor_ids: co_sensor_ids,
        start: params.start.clone(),
        end: params.end.clone(),
        interval_seconds: Some(interval_seconds),
        max_buckets: Some(if quick_suggest { 8_000 } else { 16_000 }),
        z_threshold: Some(z_threshold),
        threshold_mode: Some(threshold_mode),
        adaptive_threshold: params.adaptive_threshold.clone(),
        detector_mode: Some(detector_mode),
        suppression_mode: Some(suppression_mode),
        exclude_boundary_events: Some(exclude_boundary_events),
        sparse_point_events_enabled: Some(sparse_point_events_enabled),
        gap_max_buckets: Some(gap_max_buckets),
        min_separation_buckets: Some(min_separation_buckets),
        tolerance_buckets: Some(tolerance_buckets),
        min_sensors: Some(min_sensors),
        max_results: Some((max_results * 4).min(256) as u32),
        max_sensors: Some(max_sensors_used),
        max_events: Some(max_events),
        focus_sensor_id: Some(focus_sensor_id.clone()),
        polarity: Some(polarity),
        z_cap: Some(z_cap),
        deseason_mode: Some(deseason_mode),
        periodic_penalty_enabled: Some(periodic_penalty_enabled),
        bucket_preference_mode: Some(cooccurrence_bucket_preference_mode),
    };
    let coocc_params_base = coocc_params.clone();

    let start_coocc = Instant::now();
    let coocc_job = rewritten_job(job, "cooccurrence_v1", serde_json::json!(coocc_params));
    let coocc_value =
        cooccurrence_v1::execute(db, duckdb, lake, &coocc_job, cancel.clone()).await?;
    let coocc_result: CooccurrenceResultV1 =
        serde_json::from_value(coocc_value).map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "result_decode_failed".to_string(),
                message: format!("Failed to decode cooccurrence result: {err}"),
                details: None,
            })
        })?;
    let coocc_ms = start_coocc.elapsed().as_millis() as u64;

    let coocc_total_sensors = coocc_result
        .params
        .sensor_ids
        .len()
        .saturating_sub(coocc_result.truncated_sensor_ids.len())
        .max(1);

    let mut system_wide_buckets: Vec<SystemWideCooccurrenceBucketV1> = Vec::new();
    let mut system_wide_ts: HashSet<i64> = HashSet::new();
    for bucket in &coocc_result.buckets {
        if !bucket.severity_sum.is_finite() || bucket.severity_sum <= 0.0 {
            continue;
        }
        let group_size = bucket.group_size.max(1) as usize;
        let ratio = (group_size as f64) / (coocc_total_sensors as f64);
        let is_system_wide = group_size >= 10 || ratio >= 0.5;
        if !is_system_wide {
            continue;
        }
        system_wide_ts.insert(bucket.ts);
        system_wide_buckets.push(SystemWideCooccurrenceBucketV1 {
            ts: bucket.ts,
            group_size: bucket.group_size,
            severity_sum: bucket.severity_sum,
        });
    }

    system_wide_buckets.sort_by(|a, b| {
        b.severity_sum
            .total_cmp(&a.severity_sum)
            .then_with(|| b.group_size.cmp(&a.group_size))
            .then_with(|| b.ts.cmp(&a.ts))
    });
    if system_wide_buckets.len() > 24 {
        system_wide_buckets.truncate(24);
    }

    let mut coocc_for_merge = coocc_result.clone();
    if params.exclude_system_wide_buckets.unwrap_or(false) && !system_wide_ts.is_empty() {
        coocc_for_merge
            .buckets
            .retain(|bucket| !system_wide_ts.contains(&bucket.ts));
    }

    progress.phase = "merge".to_string();
    progress.completed = 3;
    progress.message = Some("Merging relationship evidence".to_string());
    let _ = store::update_progress(db, job.id, &progress).await;

    if cancel.is_cancelled() {
        return Err(JobFailure::Canceled);
    }

    let include_low = params
        .include_low_confidence
        .unwrap_or(matches!(mode, UnifiedRelationshipModeV2::Advanced));
    let evidence_source = compute_evidence_source(&params, &weights);

    let (mut candidates, truncated_result_sensor_ids, cooccurrence_sensors) = merge_unified_candidates(
        &focus_sensor_id,
        &event_result,
        &coocc_for_merge,
        weights.clone(),
        cooccurrence_score_mode,
        include_low,
        max_results,
    );
    for candidate in candidates.iter_mut() {
        if let Some(path) = derived_dependency_paths.get(&candidate.sensor_id) {
            candidate.derived_from_focus = true;
            candidate.derived_dependency_path = Some(path.clone());
        }
    }

    let (focus_bucket_coverage_pct, candidate_bucket_coverage_pct) = {
        let mut ids: Vec<String> = Vec::with_capacity(candidate_sensor_ids.len() + 1);
        ids.push(focus_sensor_id.clone());
        ids.extend(candidate_sensor_ids.iter().cloned());
        ids.sort();
        ids.dedup();

        let stats = duckdb
            .read_bucket_coverage_stats_from_lake_with_options(
                lake,
                start,
                end_exclusive,
                ids.clone(),
                interval_seconds,
                gap_max_buckets,
                MetricsBucketReadOptions::analysis_default(),
            )
            .await
            .map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "missingness_stats_failed".to_string(),
                    message: err.to_string(),
                    details: None,
                })
            })?;

        let mut bucket_rows_by_id: HashMap<String, u64> = HashMap::new();
        for row in stats {
            bucket_rows_by_id.insert(row.sensor_id, row.bucket_rows);
        }

        let focus_rows = bucket_rows_by_id
            .get(&focus_sensor_id)
            .copied()
            .unwrap_or(0);
        let focus_pct = bucket_coverage_pct(focus_rows, start, end_exclusive, interval_seconds);

        let mut pct_by_sensor_id: HashMap<String, f64> = HashMap::new();
        for sensor_id in ids {
            let bucket_rows = bucket_rows_by_id.get(&sensor_id).copied().unwrap_or(0);
            if let Some(pct) = bucket_coverage_pct(bucket_rows, start, end_exclusive, interval_seconds) {
                pct_by_sensor_id.insert(sensor_id, pct);
            }
        }

        (focus_pct, pct_by_sensor_id)
    };

    for candidate in candidates.iter_mut() {
        candidate.evidence.focus_bucket_coverage_pct = focus_bucket_coverage_pct;
        candidate.evidence.candidate_bucket_coverage_pct =
            candidate_bucket_coverage_pct.get(&candidate.sensor_id).copied();
    }

    let computed_through_ts = choose_latest_iso(
        event_result.computed_through_ts.clone(),
        coocc_result.computed_through_ts.clone(),
    );

    let mut timings_ms = BTreeMap::new();
    timings_ms.insert("prefilter_ms".to_string(), prefilter_ms);
    timings_ms.insert("events_ms".to_string(), events_ms);
    timings_ms.insert("cooccurrence_ms".to_string(), coocc_ms);
    timings_ms.insert(
        "job_total_ms".to_string(),
        (prefilter_ms + events_ms + coocc_ms).max(
            event_result
                .timings_ms
                .get("job_total_ms")
                .copied()
                .unwrap_or(0)
                + coocc_result
                    .timings_ms
                    .get("job_total_ms")
                    .copied()
                    .unwrap_or(0),
        ),
    );

    let mut counts = BTreeMap::new();
    let evaluated_count = candidate_sensor_ids.len() as u64;
    counts.insert(
        "candidate_pool".to_string(),
        evaluated_count,
    );
    counts.insert("eligible_count".to_string(), eligible_count as u64);
    counts.insert("evaluated_count".to_string(), evaluated_count);
    counts.insert("ranked".to_string(), candidates.len() as u64);
    counts.insert(
        "evaluate_all_eligible_effective".to_string(),
        if evaluate_all_eligible_effective { 1 } else { 0 },
    );
    counts.insert(
        "pinned_requested".to_string(),
        pinned_requested_ids.len() as u64,
    );
    counts.insert(
        "pinned_included".to_string(),
        pinned_included_ids.len() as u64,
    );
    counts.insert(
        "pinned_truncated".to_string(),
        pinned_truncated_ids.len() as u64,
    );
    counts.insert(
        "candidate_prefiltered".to_string(),
        prefiltered_candidate_sensor_ids.len() as u64,
    );
    counts.insert(
        "candidate_truncated".to_string(),
        truncated_candidate_sensor_ids.len() as u64,
    );
    counts.insert(
        "event_candidates".to_string(),
        event_result.candidates.len() as u64,
    );
    counts.insert(
        "cooccurrence_sensors".to_string(),
        cooccurrence_sensors as u64,
    );
    counts.insert(
        "cooccurrence_total_sensors".to_string(),
        coocc_total_sensors as u64,
    );
    counts.insert(
        "deseasoning_applied".to_string(),
        if deseasoning_applied { 1 } else { 0 },
    );
    counts.insert(
        "deseasoning_skipped_insufficient_window".to_string(),
        if deseasoning_skipped_insufficient_window { 1 } else { 0 },
    );

    let monitoring: Option<EventEvidenceMonitoringV1> = event_result.monitoring.clone();
    let stability_enabled = params.stability_enabled.unwrap_or(false);
    let stability = if stability_enabled {
        Some(
            compute_rank_stability(
                db,
                duckdb,
                lake,
                job,
                cancel.clone(),
                &focus_sensor_id,
                &candidates,
                start,
                end_inclusive,
                eligible_count,
                &event_params_base,
                &coocc_params_base,
                weights,
                cooccurrence_score_mode,
                include_low,
                max_results,
                params.exclude_system_wide_buckets.unwrap_or(false),
            )
            .await?,
        )
    } else {
        None
    };
    if let Some(stability) = stability.as_ref() {
        counts.insert(
            "stability_computed".to_string(),
            if matches!(stability.status, UnifiedStabilityStatusV1::Computed) {
                1
            } else {
                0
            },
        );
    }

    let result = RelatedSensorsUnifiedResultV2 {
        job_type: "related_sensors_unified_v2".to_string(),
        focus_sensor_id,
        evidence_source: Some(evidence_source),
        computed_through_ts,
        interval_seconds: event_result
            .interval_seconds
            .or(coocc_result.interval_seconds),
        bucket_count: event_result.bucket_count.or(coocc_result.bucket_count),
        params,
        limits_used: RelatedSensorsUnifiedLimitsUsedV2 {
            candidate_limit_used: candidate_limit_used as u32,
            max_results_used: max_results as u32,
            max_sensors_used,
        },
        candidates,
        skipped_candidates,
        system_wide_buckets,
        prefiltered_candidate_sensor_ids,
        truncated_candidate_sensor_ids,
        truncated_result_sensor_ids,
        timings_ms,
        counts,
        versions: BTreeMap::from([
            ("unified".to_string(), "v2".to_string()),
            ("event_match".to_string(), "v1".to_string()),
            ("cooccurrence".to_string(), "v1".to_string()),
        ]),
        monitoring,
        stability,
        gap_skipped_deltas: event_result.gap_skipped_deltas.clone(),
    };

    serde_json::to_value(&result)
        .context("failed to serialize related_sensors_unified_v2 result")
        .map_err(|err| {
            JobFailure::Failed(AnalysisJobError {
                code: "result_encode_failed".to_string(),
                message: err.to_string(),
                details: None,
            })
        })
}

fn overlap_at_k(left: &[String], right: &[String], k: usize) -> f64 {
    let kk = k.max(1);
    let right_set: HashSet<&str> = right
        .iter()
        .take(kk)
        .map(|id| id.as_str())
        .collect();

    let hits = left
        .iter()
        .take(kk)
        .filter(|id| right_set.contains(id.as_str()))
        .count();

    (hits as f64) / (kk as f64)
}

fn stability_tier(score: f64) -> UnifiedStabilityTierV1 {
    if score >= 0.8 {
        UnifiedStabilityTierV1::High
    } else if score >= 0.5 {
        UnifiedStabilityTierV1::Medium
    } else {
        UnifiedStabilityTierV1::Low
    }
}

fn compute_evidence_source(
    params: &RelatedSensorsUnifiedJobParamsV2,
    weights: &UnifiedStrategyWeightsV2,
) -> UnifiedEvidenceSourceV1 {
    let events_enabled = weights.events.is_finite() && weights.events > 0.0;
    if !events_enabled {
        return UnifiedEvidenceSourceV1::DeltaZ;
    }

    let explicit_focus_events = !params.focus_events.is_empty();
    if !explicit_focus_events {
        return UnifiedEvidenceSourceV1::DeltaZ;
    }

    let coocc_enabled = weights.cooccurrence.is_finite() && weights.cooccurrence > 0.0;
    if coocc_enabled {
        UnifiedEvidenceSourceV1::Blend
    } else {
        UnifiedEvidenceSourceV1::Pattern
    }
}

fn strip_system_wide_buckets(coocc_result: &CooccurrenceResultV1) -> CooccurrenceResultV1 {
    let coocc_total_sensors = coocc_result
        .params
        .sensor_ids
        .len()
        .saturating_sub(coocc_result.truncated_sensor_ids.len())
        .max(1);

    let mut system_wide_ts: HashSet<i64> = HashSet::new();
    for bucket in &coocc_result.buckets {
        if !bucket.severity_sum.is_finite() || bucket.severity_sum <= 0.0 {
            continue;
        }
        let group_size = bucket.group_size.max(1) as usize;
        let ratio = (group_size as f64) / (coocc_total_sensors as f64);
        let is_system_wide = group_size >= 10 || ratio >= 0.5;
        if is_system_wide {
            system_wide_ts.insert(bucket.ts);
        }
    }

    if system_wide_ts.is_empty() {
        return coocc_result.clone();
    }

    let mut filtered = coocc_result.clone();
    filtered
        .buckets
        .retain(|bucket| !system_wide_ts.contains(&bucket.ts));
    filtered
}

async fn compute_rank_stability(
    db: &PgPool,
    duckdb: &DuckDbQueryService,
    lake: &crate::services::analysis::lake::AnalysisLakeConfig,
    job: &AnalysisJobRow,
    cancel: CancellationToken,
    focus_sensor_id: &str,
    candidates: &[RelatedSensorsUnifiedCandidateV2],
    start: DateTime<Utc>,
    end_inclusive: DateTime<Utc>,
    eligible_count: usize,
    event_params_base: &EventMatchJobParamsV1,
    coocc_params_base: &CooccurrenceJobParamsV1,
    weights: UnifiedStrategyWeightsV2,
    cooccurrence_score_mode: CooccurrenceScoreModeV1,
    include_low: bool,
    max_results: usize,
    exclude_system_wide_buckets: bool,
) -> std::result::Result<RelatedSensorsUnifiedStabilityV1, JobFailure> {
    let window_count = 3_u32;

    if eligible_count > STABILITY_MAX_ELIGIBLE {
        return Ok(RelatedSensorsUnifiedStabilityV1 {
            status: UnifiedStabilityStatusV1::Skipped,
            k: STABILITY_TOP_K as u32,
            window_count,
            score: None,
            tier: None,
            overlaps: Vec::new(),
            reason: Some(format!(
                "Eligible pool too large ({eligible_count} > {STABILITY_MAX_ELIGIBLE})"
            )),
        });
    }

    let main_top_ids: Vec<String> = candidates
        .iter()
        .take(STABILITY_TOP_K)
        .map(|c| c.sensor_id.clone())
        .collect();

    let total_ms = (end_inclusive - start).num_milliseconds();
    if total_ms <= 0 {
        return Ok(RelatedSensorsUnifiedStabilityV1 {
            status: UnifiedStabilityStatusV1::Skipped,
            k: STABILITY_TOP_K as u32,
            window_count,
            score: None,
            tier: None,
            overlaps: Vec::new(),
            reason: Some("Invalid time window".to_string()),
        });
    }

    let third_ms = total_ms / 3;
    if third_ms <= 0 {
        return Ok(RelatedSensorsUnifiedStabilityV1 {
            status: UnifiedStabilityStatusV1::Skipped,
            k: STABILITY_TOP_K as u32,
            window_count,
            score: None,
            tier: None,
            overlaps: Vec::new(),
            reason: Some("Window too small for stability split".to_string()),
        });
    }

    let w1_end = start + Duration::milliseconds(third_ms);
    let w2_end = start + Duration::milliseconds(third_ms.saturating_mul(2));
    let windows = vec![(start, w1_end), (w1_end, w2_end), (w2_end, end_inclusive)];

    let mut overlaps: Vec<f64> = Vec::with_capacity(windows.len());

    for (window_start, window_end) in windows {
        if cancel.is_cancelled() {
            return Err(JobFailure::Canceled);
        }
        if window_end <= window_start {
            continue;
        }

        let mut event_params = event_params_base.clone();
        event_params.start = window_start.to_rfc3339();
        event_params.end = window_end.to_rfc3339();

        let event_job = rewritten_job(job, "event_match_v1", serde_json::json!(event_params));
        let event_value =
            event_match_v1::execute(db, duckdb, lake, &event_job, cancel.clone()).await?;
        let event_result: EventMatchResultV1 =
            serde_json::from_value(event_value).map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "result_decode_failed".to_string(),
                    message: format!("Failed to decode event result: {err}"),
                    details: None,
                })
            })?;

        let mut coocc_params = coocc_params_base.clone();
        coocc_params.start = window_start.to_rfc3339();
        coocc_params.end = window_end.to_rfc3339();

        let coocc_job = rewritten_job(job, "cooccurrence_v1", serde_json::json!(coocc_params));
        let coocc_value =
            cooccurrence_v1::execute(db, duckdb, lake, &coocc_job, cancel.clone()).await?;
        let coocc_result: CooccurrenceResultV1 =
            serde_json::from_value(coocc_value).map_err(|err| {
                JobFailure::Failed(AnalysisJobError {
                    code: "result_decode_failed".to_string(),
                    message: format!("Failed to decode cooccurrence result: {err}"),
                    details: None,
                })
            })?;

        let coocc_for_merge = if exclude_system_wide_buckets {
            strip_system_wide_buckets(&coocc_result)
        } else {
            coocc_result.clone()
        };

        let (sub_candidates, _, _) = merge_unified_candidates(
            focus_sensor_id,
            &event_result,
            &coocc_for_merge,
            weights.clone(),
            cooccurrence_score_mode,
            include_low,
            max_results,
        );

        let sub_top_ids: Vec<String> = sub_candidates
            .iter()
            .take(STABILITY_TOP_K)
            .map(|c| c.sensor_id.clone())
            .collect();

        overlaps.push(overlap_at_k(
            &main_top_ids,
            &sub_top_ids,
            STABILITY_TOP_K,
        ));
    }

    if overlaps.is_empty() {
        return Ok(RelatedSensorsUnifiedStabilityV1 {
            status: UnifiedStabilityStatusV1::Skipped,
            k: STABILITY_TOP_K as u32,
            window_count,
            score: None,
            tier: None,
            overlaps: Vec::new(),
            reason: Some("No valid subwindows for stability scoring".to_string()),
        });
    }

    let score = overlaps.iter().copied().sum::<f64>() / (overlaps.len() as f64);
    let score = if score.is_finite() {
        score.clamp(0.0, 1.0)
    } else {
        0.0
    };

    Ok(RelatedSensorsUnifiedStabilityV1 {
        status: UnifiedStabilityStatusV1::Computed,
        k: STABILITY_TOP_K as u32,
        window_count,
        score: Some(score),
        tier: Some(stability_tier(score)),
        overlaps,
        reason: None,
    })
}

fn rewritten_job(
    job: &AnalysisJobRow,
    job_type: &str,
    params: serde_json::Value,
) -> AnalysisJobRow {
    AnalysisJobRow {
        id: job.id,
        job_type: job_type.to_string(),
        status: job.status.clone(),
        job_key: job.job_key.clone(),
        created_by: job.created_by,
        params: SqlJson(params),
        progress: job.progress.clone(),
        error: job.error.clone(),
        created_at: job.created_at,
        updated_at: job.updated_at,
        started_at: job.started_at,
        completed_at: job.completed_at,
        cancel_requested_at: job.cancel_requested_at,
        canceled_at: job.canceled_at,
        expires_at: job.expires_at,
    }
}

fn normalize_weights(
    weights: Option<UnifiedStrategyWeightsV2>,
    include_delta_corr: bool,
) -> UnifiedStrategyWeightsV2 {
    let defaults = if include_delta_corr {
        UnifiedStrategyWeightsV2 {
            events: 0.6,
            cooccurrence: 0.4,
            delta_corr: Some(0.2),
        }
    } else {
        UnifiedStrategyWeightsV2 {
            events: 0.6,
            cooccurrence: 0.4,
            delta_corr: None,
        }
    };

    let Some(mut weights) = weights else {
        return defaults;
    };

    if !weights.events.is_finite() || weights.events < 0.0 {
        weights.events = defaults.events;
    }
    if !weights.cooccurrence.is_finite() || weights.cooccurrence < 0.0 {
        weights.cooccurrence = defaults.cooccurrence;
    }

    let delta_raw = if include_delta_corr {
        weights
            .delta_corr
            .unwrap_or_else(|| defaults.delta_corr.unwrap_or(0.0))
    } else {
        0.0
    };
    let delta = if include_delta_corr {
        if delta_raw.is_finite() && delta_raw >= 0.0 {
            delta_raw
        } else {
            defaults.delta_corr.unwrap_or(0.0)
        }
    } else {
        0.0
    };

    let sum = weights.events + weights.cooccurrence + delta;
    if sum <= 0.0 || !sum.is_finite() {
        return defaults;
    }

    UnifiedStrategyWeightsV2 {
        events: weights.events / sum,
        cooccurrence: weights.cooccurrence / sum,
        delta_corr: if include_delta_corr {
            Some(delta / sum)
        } else {
            None
        },
    }
}

fn normalize_component(value: Option<f64>, max_value: f64) -> f64 {
    let Some(value) = value else {
        return 0.0;
    };
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    if !max_value.is_finite() || max_value <= 0.0 {
        return 0.0;
    }
    (value / max_value).clamp(0.0, 1.0)
}

fn classify_confidence(
    blended_score: f64,
    events_overlap: u64,
    cooccurrence_count: u64,
) -> UnifiedConfidenceTierV2 {
    if blended_score >= 0.75 && (events_overlap >= 2 || cooccurrence_count >= 2) {
        UnifiedConfidenceTierV2::High
    } else if blended_score >= 0.35 && (events_overlap >= 1 || cooccurrence_count >= 1) {
        UnifiedConfidenceTierV2::Medium
    } else {
        UnifiedConfidenceTierV2::Low
    }
}

fn confidence_weight(tier: UnifiedConfidenceTierV2) -> u8 {
    match tier {
        UnifiedConfidenceTierV2::High => 3,
        UnifiedConfidenceTierV2::Medium => 2,
        UnifiedConfidenceTierV2::Low => 1,
    }
}

fn aggregate_cooccurrence(
    buckets: &[CooccurrenceBucketV1],
    focus_sensor_id: &str,
    entropy_weights: &HashMap<String, f64>,
    z_cap: f64,
) -> HashMap<String, CooccurrenceAggregate> {
    let mut out: HashMap<String, CooccurrenceAggregate> = HashMap::new();

    for bucket in buckets {
        let focus = bucket
            .sensors
            .iter()
            .find(|sensor| sensor.sensor_id == focus_sensor_id);
        let Some(focus) = focus else {
            continue;
        };

        let focus_z = focus.z.abs().min(z_cap);
        for sensor in &bucket.sensors {
            if sensor.sensor_id == focus_sensor_id {
                continue;
            }
            let entry = out.entry(sensor.sensor_id.clone()).or_default();
            let sensor_z = sensor.z.abs().min(z_cap);
            let weight = entropy_weights.get(&sensor.sensor_id).copied().unwrap_or(1.0);
            entry.score_sum += focus_z * sensor_z * weight;
            entry.count += 1;
            if sensor_z > entry.max_z {
                entry.max_z = sensor_z;
            }
            entry.timestamps.push(bucket.ts);
        }
    }

    for value in out.values_mut() {
        value.timestamps.sort_by(|a, b| b.cmp(a));
        if value.timestamps.len() > 10 {
            value.timestamps.truncate(10);
        }
    }

    out
}

fn prevalence_penalty(n_focus: u64, n_candidate: u64) -> f64 {
    if n_focus == 0 || n_candidate == 0 {
        return 1.0;
    }
    if n_candidate <= n_focus {
        return 1.0;
    }
    ((n_focus as f64) / (n_candidate as f64))
        .sqrt()
        .clamp(0.25, 1.0)
}

fn surprise_ratio(avg_product: f64, focus_mean_abs_z: f64, candidate_mean_abs_z: f64) -> Option<f64> {
    if !avg_product.is_finite() || avg_product <= 0.0 {
        return None;
    }
    if !focus_mean_abs_z.is_finite()
        || !candidate_mean_abs_z.is_finite()
        || focus_mean_abs_z <= 0.0
        || candidate_mean_abs_z <= 0.0
    {
        return None;
    }
    let expected = focus_mean_abs_z * candidate_mean_abs_z;
    if !expected.is_finite() || expected <= 0.0 {
        return None;
    }
    Some((avg_product / expected).clamp(0.0, 10.0))
}

fn merge_unified_candidates(
    focus_sensor_id: &str,
    event_result: &EventMatchResultV1,
    coocc_result: &CooccurrenceResultV1,
    weights: UnifiedStrategyWeightsV2,
    cooccurrence_score_mode: CooccurrenceScoreModeV1,
    include_low: bool,
    max_results: usize,
) -> (Vec<RelatedSensorsUnifiedCandidateV2>, Vec<String>, usize) {
    let mut entropy_weights: HashMap<String, f64> = HashMap::new();
    for candidate in &event_result.candidates {
        if let Some(weight) = candidate.time_of_day_entropy_weight {
            if weight.is_finite() && weight > 0.0 {
                entropy_weights.insert(candidate.sensor_id.clone(), weight);
            }
        }
    }
    let z_cap = event_result.params.z_cap.unwrap_or(15.0).clamp(1.0, 1_000.0);
    let coagg =
        aggregate_cooccurrence(&coocc_result.buckets, focus_sensor_id, &entropy_weights, z_cap);
    let cooccurrence_sensors = coagg.len();
    let focus_mean_abs_z = coocc_result
        .sensor_stats
        .get(focus_sensor_id)
        .map(|s| s.mean_abs_z)
        .unwrap_or(0.0);

    let max_event_score = event_result
        .candidates
        .iter()
        .filter_map(|c| c.score)
        .filter(|v| v.is_finite() && *v > 0.0)
        .fold(0.0_f64, f64::max);

    let mut merged: HashMap<String, UnifiedAccumulator> = HashMap::new();

    for candidate in &event_result.candidates {
        let entry = merged.entry(candidate.sensor_id.clone()).or_default();
        entry.events_score = candidate.score;
        entry.events_overlap = Some(candidate.overlap);
        entry.n_focus = Some(candidate.n_focus);
        entry.n_candidate = Some(candidate.n_candidate);
        entry.n_focus_up = candidate.n_focus_up;
        entry.n_focus_down = candidate.n_focus_down;
        entry.n_candidate_up = candidate.n_candidate_up;
        entry.n_candidate_down = candidate.n_candidate_down;
        entry.best_lag_sec = Some(candidate.best_lag.as_ref().map(|x| x.lag_sec).unwrap_or(0));
        entry.top_lags = Some(candidate.top_lags.clone());
        entry.direction_label = candidate.direction_label;
        entry.sign_agreement = candidate.sign_agreement;
        entry.delta_corr = candidate.delta_corr;
        entry.direction_n = candidate.direction_n;
        entry.time_of_day_entropy_norm = candidate.time_of_day_entropy_norm;
        entry.time_of_day_entropy_weight = candidate.time_of_day_entropy_weight;
        entry.episodes = Some(candidate.episodes.clone());
        entry.why_ranked = candidate.why_ranked.clone();
    }

    for (sensor_id, agg) in &coagg {
        let entry = merged.entry(sensor_id.clone()).or_default();
        entry.cooccurrence_score = Some(agg.score_sum);
        entry.cooccurrence_count = Some(agg.count);
        let avg = if agg.count > 0 && agg.score_sum.is_finite() && agg.score_sum > 0.0 {
            agg.score_sum / (agg.count as f64)
        } else {
            0.0
        };
        let avg = if avg.is_finite() && avg > 0.0 { Some(avg) } else { None };
        entry.cooccurrence_avg = avg;
        let candidate_mean_abs_z = coocc_result
            .sensor_stats
            .get(sensor_id)
            .map(|s| s.mean_abs_z)
            .unwrap_or(0.0);
        entry.cooccurrence_surprise = avg.and_then(|avg| {
            surprise_ratio(avg, focus_mean_abs_z, candidate_mean_abs_z)
        });
        entry.cooccurrence_timestamps = Some(agg.timestamps.clone());
    }

    let mut candidates: Vec<RelatedSensorsUnifiedCandidateV2> = Vec::new();

    let max_coocc_metric = merged
        .values()
        .filter_map(|acc| {
            let metric = match cooccurrence_score_mode {
                CooccurrenceScoreModeV1::AvgProduct => acc.cooccurrence_avg,
                CooccurrenceScoreModeV1::Surprise => acc.cooccurrence_surprise,
            }?;
            let penalty = prevalence_penalty(acc.n_focus.unwrap_or(0), acc.n_candidate.unwrap_or(0));
            let effective = metric * penalty;
            if effective.is_finite() && effective > 0.0 {
                Some(effective)
            } else {
                None
            }
        })
        .fold(0.0_f64, f64::max);

    for (sensor_id, acc) in merged {
        let events_norm = normalize_component(acc.events_score, max_event_score);
        let coocc_metric = match cooccurrence_score_mode {
            CooccurrenceScoreModeV1::AvgProduct => acc.cooccurrence_avg,
            CooccurrenceScoreModeV1::Surprise => acc.cooccurrence_surprise,
        };
        let coocc_metric_effective = coocc_metric.map(|metric| {
            let penalty = prevalence_penalty(acc.n_focus.unwrap_or(0), acc.n_candidate.unwrap_or(0));
            metric * penalty
        });
        let coocc_norm = normalize_component(coocc_metric_effective, max_coocc_metric);
        let delta_weight = weights.delta_corr.unwrap_or(0.0);
        let delta_corr_abs = acc
            .delta_corr
            .map(|v| v.abs().clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let blended = weights.events * events_norm
            + weights.cooccurrence * coocc_norm
            + delta_weight * delta_corr_abs;

        if !blended.is_finite() || blended <= 0.0 {
            continue;
        }

        let confidence = classify_confidence(
            blended,
            acc.events_overlap.unwrap_or(0),
            acc.cooccurrence_count.unwrap_or(0),
        );
        if !include_low && matches!(confidence, UnifiedConfidenceTierV2::Low) {
            continue;
        }

        let mut summary: Vec<String> = Vec::new();
        if let Some(events_score) = acc.events_score {
            if events_score.is_finite() {
                summary.push(format!(
                    "Event match (F1) {:.2} • matched: {}",
                    events_score,
                    acc.events_overlap.unwrap_or(0)
                ));
            }
        }
        if let Some(count) = acc.cooccurrence_count {
            if count > 0 {
                let label = match cooccurrence_score_mode {
                    CooccurrenceScoreModeV1::AvgProduct => "Avg co-occ",
                    CooccurrenceScoreModeV1::Surprise => "Surprise",
                };
            summary.push(format!(
                    "Shared buckets {} • {} {:.2}",
                    count, label, coocc_norm
                ));
            }
        }
        if let Some(best_lag) = acc.best_lag_sec {
            if best_lag != 0 {
                summary.push(format!("Best lag {best_lag}s"));
            }
        }

        candidates.push(RelatedSensorsUnifiedCandidateV2 {
            sensor_id,
            derived_from_focus: false,
            derived_dependency_path: None,
            rank: 0,
            blended_score: blended,
            confidence_tier: confidence,
            episodes: acc.episodes,
            top_bucket_timestamps: acc.cooccurrence_timestamps,
            why_ranked: acc.why_ranked,
            evidence: RelatedSensorsUnifiedEvidenceV2 {
                events_score: acc.events_score,
                cooccurrence_score: acc.cooccurrence_score,
                cooccurrence_avg: acc.cooccurrence_avg,
                cooccurrence_surprise: acc.cooccurrence_surprise,
                cooccurrence_strength: coocc_metric_effective.and(Some(coocc_norm)),
                events_overlap: acc.events_overlap,
                n_focus: acc.n_focus,
                n_candidate: acc.n_candidate,
                n_focus_up: acc.n_focus_up,
                n_focus_down: acc.n_focus_down,
                n_candidate_up: acc.n_candidate_up,
                n_candidate_down: acc.n_candidate_down,
                cooccurrence_count: acc.cooccurrence_count,
                focus_bucket_coverage_pct: None,
                candidate_bucket_coverage_pct: None,
                best_lag_sec: acc.best_lag_sec,
                top_lags: acc.top_lags.unwrap_or_default(),
                direction_label: acc.direction_label,
                sign_agreement: acc.sign_agreement,
                delta_corr: acc.delta_corr,
                direction_n: acc.direction_n,
                time_of_day_entropy_norm: acc.time_of_day_entropy_norm,
                time_of_day_entropy_weight: acc.time_of_day_entropy_weight,
                summary,
            },
        });
    }

    candidates.sort_by(|a, b| {
        b.blended_score
            .total_cmp(&a.blended_score)
            .then_with(|| {
                confidence_weight(b.confidence_tier).cmp(&confidence_weight(a.confidence_tier))
            })
            .then_with(|| {
                b.evidence
                    .cooccurrence_count
                    .unwrap_or(0)
                    .cmp(&a.evidence.cooccurrence_count.unwrap_or(0))
            })
            .then_with(|| {
                b.evidence
                    .events_overlap
                    .unwrap_or(0)
                    .cmp(&a.evidence.events_overlap.unwrap_or(0))
            })
            .then_with(|| a.sensor_id.cmp(&b.sensor_id))
    });

    let mut truncated_result_sensor_ids: Vec<String> = Vec::new();
    if candidates.len() > max_results {
        let remainder = candidates.split_off(max_results);
        truncated_result_sensor_ids = remainder.into_iter().map(|c| c.sensor_id).collect();
    }

    for (idx, candidate) in candidates.iter_mut().enumerate() {
        candidate.rank = (idx + 1) as u32;
    }

    (candidates, truncated_result_sensor_ids, cooccurrence_sensors)
}

fn choose_latest_iso(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let parsed_a = DateTime::parse_from_rfc3339(&a).ok();
            let parsed_b = DateTime::parse_from_rfc3339(&b).ok();
            match (parsed_a, parsed_b) {
                (Some(pa), Some(pb)) => {
                    if pa >= pb {
                        Some(a)
                    } else {
                        Some(b)
                    }
                }
                (Some(_), None) => Some(a),
                (None, Some(_)) => Some(b),
                (None, None) => Some(a),
            }
        }
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

async fn fetch_focus_sensor_meta(
    db: &PgPool,
    focus_sensor_id: &str,
) -> Result<SensorMetaRow, JobFailure> {
    sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            node_id,
            type as sensor_type,
            unit,
            interval_seconds,
            NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        WHERE sensor_id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(focus_sensor_id)
    .fetch_one(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "focus_sensor_not_found".to_string(),
            message: err.to_string(),
            details: None,
        })
    })
}

async fn fetch_sensor_meta_rows(
    db: &PgPool,
    sensor_ids: &[String],
) -> Result<Vec<SensorMetaRow>, JobFailure> {
    if sensor_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            node_id,
            type as sensor_type,
            unit,
            interval_seconds,
            NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        WHERE deleted_at IS NULL
          AND sensor_id = ANY($1)
        ORDER BY sensor_id
        "#,
    )
    .bind(sensor_ids)
    .fetch_all(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "sensor_lookup_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })
}

async fn fetch_candidate_sensor_meta_rows_by_filters(
    db: &PgPool,
    focus: &SensorMetaRow,
    filters: &TsseCandidateFiltersV1,
) -> Result<Vec<SensorMetaRow>, JobFailure> {
    let node_id_filter: Option<uuid::Uuid> = if filters.same_node_only {
        Some(focus.node_id)
    } else {
        None
    };
    let unit_filter: Option<String> = if filters.same_unit_only {
        Some(focus.unit.clone())
    } else {
        None
    };
    let type_filter: Option<String> = if filters.same_type_only {
        Some(focus.sensor_type.clone())
    } else {
        None
    };
    let interval_seconds_filter = filters.interval_seconds;
    let derived_filter = filters.is_derived;
    let provider_filter = filters.is_public_provider;

    let mut exclude_ids: Vec<String> = filters
        .exclude_sensor_ids
        .iter()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    exclude_ids.sort();
    exclude_ids.dedup();

    sqlx::query_as(
        r#"
        SELECT
            sensor_id,
            node_id,
            type as sensor_type,
            unit,
            interval_seconds,
            NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') as source
        FROM sensors
        WHERE deleted_at IS NULL
          AND sensor_id <> $1
          AND ($2::uuid IS NULL OR node_id = $2)
          AND ($3::text IS NULL OR unit = $3)
          AND ($4::text IS NULL OR type = $4)
          AND ($5::bigint IS NULL OR interval_seconds::bigint = $5)
          AND (
            $6::boolean IS NULL
            OR COALESCE(
              NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') = 'derived',
              false
            ) = $6
          )
          AND (
            $7::boolean IS NULL
            OR COALESCE(
              NULLIF(TRIM(COALESCE(config, '{}'::jsonb)->>'source'), '') = 'forecast_points',
              false
            ) = $7
          )
          AND NOT (sensor_id = ANY($8))
        ORDER BY sensor_id
        "#,
    )
    .bind(&focus.sensor_id)
    .bind(node_id_filter)
    .bind(unit_filter)
    .bind(type_filter)
    .bind(interval_seconds_filter)
    .bind(derived_filter)
    .bind(provider_filter)
    .bind(exclude_ids)
    .fetch_all(db)
    .await
    .map_err(|err| {
        JobFailure::Failed(AnalysisJobError {
            code: "sensor_lookup_failed".to_string(),
            message: err.to_string(),
            details: None,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{
        cap_dependency_path, classify_confidence, deterministic_candidate_order,
        compute_candidate_limit_used, compute_evidence_source, eligible_candidate_count,
        find_derived_dependency_path, merge_unified_candidates, normalize_weights,
        should_query_all_candidates,
        stable_candidate_order_seed, SensorMetaRow,
    };
    use super::super::event_utils::{detect_change_events, EventPoint};
    use crate::services::analysis::parquet_duckdb::MetricsBucketRow;
    use crate::services::analysis::tsse::types::{
        CooccurrenceBucketV1, CooccurrenceEventV1, CooccurrenceJobParamsV1, CooccurrenceResultV1,
        CooccurrenceScoreModeV1, EventDirectionV1, EventMatchCandidateV1, EventMatchJobParamsV1,
        EventMatchLagScoreV1, EventMatchResultV1, EventPolarityV1,
        RelatedSensorsUnifiedJobParamsV2, TsseCandidateFiltersV1, UnifiedConfidenceTierV2,
        UnifiedEvidenceSourceV1, UnifiedRelationshipModeV2, UnifiedStrategyWeightsV2,
    };
    use chrono::{DateTime, Utc};
    use std::collections::{BTreeMap, HashMap};
    use uuid::Uuid;

    #[test]
    fn backend_query_mode_is_selected_by_param_or_empty_candidate_list() {
        let params: RelatedSensorsUnifiedJobParamsV2 = serde_json::from_value(serde_json::json!({
            "focus_sensor_id": "focus",
            "start": "2026-02-09T00:00:00Z",
            "end": "2026-02-10T00:00:00Z",
            "candidate_source": "all_sensors_in_scope",
            "candidate_sensor_ids": ["a"]
        }))
        .expect("params");
        assert!(should_query_all_candidates(&params));

        let params: RelatedSensorsUnifiedJobParamsV2 = serde_json::from_value(serde_json::json!({
            "focus_sensor_id": "focus",
            "start": "2026-02-09T00:00:00Z",
            "end": "2026-02-10T00:00:00Z",
            "candidate_source": "visible_in_trends",
            "candidate_sensor_ids": []
        }))
        .expect("params");
        assert!(!should_query_all_candidates(&params));

        let params: RelatedSensorsUnifiedJobParamsV2 = serde_json::from_value(serde_json::json!({
            "focus_sensor_id": "focus",
            "start": "2026-02-09T00:00:00Z",
            "end": "2026-02-10T00:00:00Z",
            "candidate_sensor_ids": []
        }))
        .expect("params");
        assert!(should_query_all_candidates(&params));
    }

    #[test]
    fn evidence_source_respects_explicit_focus_events_and_weights() {
        let params_with_focus: RelatedSensorsUnifiedJobParamsV2 =
            serde_json::from_value(serde_json::json!({
                "focus_sensor_id": "focus",
                "start": "2026-02-09T00:00:00Z",
                "end": "2026-02-10T00:00:00Z",
                "focus_events": [
                    { "ts": "2026-02-09T12:00:00Z", "severity": 2.0 }
                ]
            }))
            .expect("params");

        let weights_blend = UnifiedStrategyWeightsV2 {
            events: 0.6,
            cooccurrence: 0.4,
            delta_corr: None,
        };
        assert_eq!(
            compute_evidence_source(&params_with_focus, &weights_blend),
            UnifiedEvidenceSourceV1::Blend
        );

        let weights_pattern = UnifiedStrategyWeightsV2 {
            events: 1.0,
            cooccurrence: 0.0,
            delta_corr: None,
        };
        assert_eq!(
            compute_evidence_source(&params_with_focus, &weights_pattern),
            UnifiedEvidenceSourceV1::Pattern
        );

        let weights_delta_only = UnifiedStrategyWeightsV2 {
            events: 0.0,
            cooccurrence: 1.0,
            delta_corr: None,
        };
        assert_eq!(
            compute_evidence_source(&params_with_focus, &weights_delta_only),
            UnifiedEvidenceSourceV1::DeltaZ
        );

        let params_no_focus: RelatedSensorsUnifiedJobParamsV2 =
            serde_json::from_value(serde_json::json!({
                "focus_sensor_id": "focus",
                "start": "2026-02-09T00:00:00Z",
                "end": "2026-02-10T00:00:00Z",
                "focus_events": []
            }))
            .expect("params");
        assert_eq!(
            compute_evidence_source(&params_no_focus, &weights_blend),
            UnifiedEvidenceSourceV1::DeltaZ
        );
    }

    #[test]
    fn evaluate_all_eligible_uses_pool_size_when_small() {
        let (used, effective) = compute_candidate_limit_used(
            UnifiedRelationshipModeV2::Advanced,
            false,
            Some(200),
            0,
            5,
            true,
        );
        assert_eq!(used, 5);
        assert!(effective);

        let (used, effective) = compute_candidate_limit_used(
            UnifiedRelationshipModeV2::Advanced,
            false,
            Some(200),
            3,
            5,
            true,
        );
        assert_eq!(used, 5);
        assert!(effective);
    }

    #[test]
    fn evaluate_all_eligible_uses_pool_size_when_large() {
        let (used, effective) = compute_candidate_limit_used(
            UnifiedRelationshipModeV2::Advanced,
            false,
            Some(200),
            0,
            501,
            true,
        );
        assert_eq!(used, 501);
        assert!(effective);
    }

    #[test]
    fn eligible_count_includes_deduped_pins() {
        let focus_node = Uuid::new_v4();
        let candidate_rows = vec![
            SensorMetaRow {
                sensor_id: "a".to_string(),
                node_id: focus_node,
                sensor_type: "temp".to_string(),
                unit: "C".to_string(),
                interval_seconds: 60,
                source: None,
            },
            SensorMetaRow {
                sensor_id: "b".to_string(),
                node_id: focus_node,
                sensor_type: "temp".to_string(),
                unit: "C".to_string(),
                interval_seconds: 60,
                source: None,
            },
        ];
        let pinned_rows = vec![
            SensorMetaRow {
                sensor_id: "b".to_string(),
                node_id: focus_node,
                sensor_type: "temp".to_string(),
                unit: "C".to_string(),
                interval_seconds: 60,
                source: None,
            },
            SensorMetaRow {
                sensor_id: "c".to_string(),
                node_id: focus_node,
                sensor_type: "temp".to_string(),
                unit: "C".to_string(),
                interval_seconds: 60,
                source: None,
            },
        ];

        assert_eq!(eligible_candidate_count(&candidate_rows, &pinned_rows), 3);
    }

    #[test]
    fn normalize_weights_rescales_positive_values() {
        let weights = normalize_weights(Some(UnifiedStrategyWeightsV2 {
            events: 3.0,
            cooccurrence: 1.0,
            delta_corr: None,
        }), false);
        assert!((weights.events - 0.75).abs() < 0.0001);
        assert!((weights.cooccurrence - 0.25).abs() < 0.0001);
    }

    #[test]
    fn normalize_weights_falls_back_for_invalid_input() {
        let weights = normalize_weights(Some(UnifiedStrategyWeightsV2 {
            events: -1.0,
            cooccurrence: f64::NAN,
            delta_corr: None,
        }), false);
        assert!((weights.events - 0.6).abs() < 0.0001);
        assert!((weights.cooccurrence - 0.4).abs() < 0.0001);
    }

    #[test]
    fn delta_corr_signal_improves_rank_order_when_enabled() {
        let focus_sensor_id = "sensor-focus";
        let mut event_result = make_event_result(
            focus_sensor_id,
            vec![("sensor-a", 0.27), ("sensor-b", 0.30)],
        );
        for candidate in event_result.candidates.iter_mut() {
            if candidate.sensor_id == "sensor-a" {
                candidate.delta_corr = Some(1.0);
            }
        }

        let coocc_result =
            make_cooccurrence_result(focus_sensor_id, &[("sensor-a", 9.0), ("sensor-b", 10.0)]);

        let (candidates_base, _, _) = merge_unified_candidates(
            focus_sensor_id,
            &event_result,
            &coocc_result,
            normalize_weights(None, false),
            CooccurrenceScoreModeV1::AvgProduct,
            true,
            10,
        );
        assert_eq!(candidates_base.first().unwrap().sensor_id, "sensor-b");

        let (candidates_delta, _, _) = merge_unified_candidates(
            focus_sensor_id,
            &event_result,
            &coocc_result,
            normalize_weights(None, true),
            CooccurrenceScoreModeV1::AvgProduct,
            true,
            10,
        );
        assert_eq!(candidates_delta.first().unwrap().sensor_id, "sensor-a");
    }

    #[test]
    fn confidence_classification_matches_thresholds() {
        assert!(matches!(
            classify_confidence(0.8, 2, 0),
            UnifiedConfidenceTierV2::High
        ));
        assert!(matches!(
            classify_confidence(0.5, 1, 0),
            UnifiedConfidenceTierV2::Medium
        ));
        assert!(matches!(
            classify_confidence(0.2, 1, 0),
            UnifiedConfidenceTierV2::Low
        ));
    }

    #[test]
    fn deterministic_candidate_order_respects_priority_groups() {
        let focus_node = Uuid::from_u128(1);
        let other_node = Uuid::from_u128(2);
        let focus = SensorMetaRow {
            sensor_id: "focus".to_string(),
            node_id: focus_node,
            sensor_type: "temperature".to_string(),
            unit: "C".to_string(),
            interval_seconds: 60,
            source: None,
        };

        let same_node = SensorMetaRow {
            sensor_id: "same-node".to_string(),
            node_id: focus_node,
            sensor_type: "pressure".to_string(),
            unit: "kPa".to_string(),
            interval_seconds: 60,
            source: None,
        };
        let same_unit = SensorMetaRow {
            sensor_id: "same-unit".to_string(),
            node_id: other_node,
            sensor_type: "pressure".to_string(),
            unit: "C".to_string(),
            interval_seconds: 60,
            source: None,
        };
        let same_type = SensorMetaRow {
            sensor_id: "same-type".to_string(),
            node_id: other_node,
            sensor_type: "temperature".to_string(),
            unit: "kPa".to_string(),
            interval_seconds: 60,
            source: None,
        };
        let other = SensorMetaRow {
            sensor_id: "other".to_string(),
            node_id: other_node,
            sensor_type: "pressure".to_string(),
            unit: "kPa".to_string(),
            interval_seconds: 60,
            source: None,
        };

        let order = deterministic_candidate_order(&focus, &[other, same_type, same_unit, same_node], 42);
        assert_eq!(order, vec!["same-node", "same-unit", "same-type", "other"]);
    }

    #[test]
    fn deterministic_candidate_order_is_stable_and_not_lexicographic_prefix_truncated() {
        let focus_node = Uuid::from_u128(1);
        let other_node = Uuid::from_u128(2);
        let focus = SensorMetaRow {
            sensor_id: "focus".to_string(),
            node_id: focus_node,
            sensor_type: "temperature".to_string(),
            unit: "C".to_string(),
            interval_seconds: 60,
            source: None,
        };

        let mut candidates: Vec<SensorMetaRow> = Vec::new();
        for idx in 0..128_u32 {
            candidates.push(SensorMetaRow {
                sensor_id: format!("a-{idx:04}"),
                node_id: other_node,
                sensor_type: "pressure".to_string(),
                unit: "kPa".to_string(),
                interval_seconds: 60,
                source: None,
            });
            candidates.push(SensorMetaRow {
                sensor_id: format!("z-{idx:04}"),
                node_id: other_node,
                sensor_type: "pressure".to_string(),
                unit: "kPa".to_string(),
                interval_seconds: 60,
                source: None,
            });
        }

        let seed = stable_candidate_order_seed(&focus.sensor_id, "job-key");
        let order1 = deterministic_candidate_order(&focus, &candidates, seed);
        let order2 = deterministic_candidate_order(&focus, &candidates, seed);
        assert_eq!(order1, order2);

        let mut lex = order1.clone();
        lex.sort();
        assert_ne!(order1, lex, "candidate ordering must not be lexicographic");

        let limit = 50;
        let selected = &order1[..limit];
        assert!(
            selected.iter().any(|id| id.starts_with("z-")),
            "hashed truncation should not deterministically select only lexicographic prefixes"
        );
    }

    #[test]
    fn derived_dependency_path_detects_transitive_chain_and_bounds() {
        let mut derived_inputs: HashMap<String, Vec<String>> = HashMap::new();
        derived_inputs.insert("d1".to_string(), vec!["focus".to_string()]);
        derived_inputs.insert("d2".to_string(), vec!["d1".to_string()]);
        derived_inputs.insert("cycle_a".to_string(), vec!["cycle_b".to_string()]);
        derived_inputs.insert("cycle_b".to_string(), vec!["cycle_a".to_string()]);

        let path = find_derived_dependency_path("d2", "focus", &derived_inputs, 10, 100)
            .expect("expected dependency path");
        assert_eq!(path, vec!["d2", "d1", "focus"]);

        assert!(find_derived_dependency_path("cycle_a", "focus", &derived_inputs, 10, 100).is_none());
        assert!(find_derived_dependency_path("d2", "focus", &derived_inputs, 1, 100).is_none());
        assert!(find_derived_dependency_path("focus", "focus", &derived_inputs, 10, 100).is_none());
        assert!(find_derived_dependency_path("", "focus", &derived_inputs, 10, 100).is_none());
    }

    #[test]
    fn derived_dependency_path_is_capped_and_preserves_focus() {
        let path = vec![
            "candidate".to_string(),
            "mid1".to_string(),
            "mid2".to_string(),
            "mid3".to_string(),
            "mid4".to_string(),
            "focus".to_string(),
        ];
        let capped = cap_dependency_path(path, 4);
        assert_eq!(capped, vec!["candidate", "mid1", "mid2", "focus"]);
    }

    fn make_event_result(
        focus_sensor_id: &str,
        candidates: Vec<(&str, f64)>,
    ) -> EventMatchResultV1 {
        let interval_seconds = 60;
        EventMatchResultV1 {
            job_type: "event_match_v1".to_string(),
            focus_sensor_id: focus_sensor_id.to_string(),
            computed_through_ts: None,
            interval_seconds: Some(interval_seconds),
            bucket_count: Some(100),
            params: EventMatchJobParamsV1 {
                focus_sensor_id: focus_sensor_id.to_string(),
                start: "2026-02-09T00:00:00Z".to_string(),
                end: "2026-02-10T00:00:00Z".to_string(),
                focus_events: Vec::new(),
                interval_seconds: Some(interval_seconds),
                candidate_sensor_ids: candidates.iter().map(|(id, _)| (*id).to_string()).collect(),
                candidate_limit: None,
                max_buckets: None,
                max_events: None,
                z_threshold: None,
                threshold_mode: None,
                adaptive_threshold: None,
                detector_mode: None,
                suppression_mode: None,
                exclude_boundary_events: None,
                sparse_point_events_enabled: None,
                min_separation_buckets: None,
                max_lag_buckets: None,
                top_k_lags: None,
                tolerance_buckets: None,
                max_episodes: None,
                episode_gap_buckets: None,
                gap_max_buckets: None,
                polarity: None,
                z_cap: None,
                deseason_mode: None,
                periodic_penalty_enabled: None,
                filters: TsseCandidateFiltersV1::default(),
            },
            candidates: candidates
                .into_iter()
                .enumerate()
                .map(|(idx, (sensor_id, score))| EventMatchCandidateV1 {
                    sensor_id: sensor_id.to_string(),
                    rank: (idx + 1) as u32,
                    score: Some(score),
                    overlap: 2,
                    n_focus: 10,
                    n_candidate: 10,
                    n_focus_up: None,
                    n_focus_down: None,
                    n_candidate_up: None,
                    n_candidate_down: None,
                    zero_lag: EventMatchLagScoreV1 {
                        lag_sec: 0,
                        score: Some(0.0),
                        overlap: 0,
                        n_candidate: 10,
                    },
                    best_lag: None,
                    top_lags: Vec::new(),
                    direction_label: None,
                    sign_agreement: None,
                    delta_corr: None,
                    direction_n: None,
                    time_of_day_entropy_norm: None,
                    time_of_day_entropy_weight: None,
                    episodes: Vec::new(),
                    why_ranked: None,
                })
                .collect(),
            truncated_sensor_ids: Vec::new(),
            gap_skipped_deltas: BTreeMap::new(),
            monitoring: None,
            timings_ms: BTreeMap::new(),
            versions: BTreeMap::new(),
        }
    }

    fn make_cooccurrence_result(
        focus_sensor_id: &str,
        candidate_z: &[(&str, f64)],
    ) -> CooccurrenceResultV1 {
        let interval_seconds = 60;
        let mut buckets: Vec<CooccurrenceBucketV1> = Vec::new();
        for (idx, (sensor_id, z)) in candidate_z.iter().enumerate() {
            let ts = (idx as i64) * interval_seconds * 1000;
            buckets.push(CooccurrenceBucketV1 {
                ts,
                sensors: vec![
                    CooccurrenceEventV1 {
                        sensor_id: focus_sensor_id.to_string(),
                        ts,
                        z: 1.0,
                        direction: EventDirectionV1::Up,
                        delta: 1.0,
                    },
                    CooccurrenceEventV1 {
                        sensor_id: (*sensor_id).to_string(),
                        ts,
                        z: *z,
                        direction: if *z >= 0.0 {
                            EventDirectionV1::Up
                        } else {
                            EventDirectionV1::Down
                        },
                        delta: 1.0,
                    },
                ],
                group_size: 2,
                severity_sum: 2.0,
                pair_weight: 1.0,
                idf: Some(1.0),
                score: 1.0,
            });
        }

        CooccurrenceResultV1 {
            job_type: "cooccurrence_v1".to_string(),
            computed_through_ts: None,
            interval_seconds: Some(interval_seconds),
            bucket_count: Some(100),
            params: CooccurrenceJobParamsV1 {
                sensor_ids: std::iter::once(focus_sensor_id.to_string())
                    .chain(candidate_z.iter().map(|(id, _)| (*id).to_string()))
                    .collect(),
                start: "2026-02-09T00:00:00Z".to_string(),
                end: "2026-02-10T00:00:00Z".to_string(),
                interval_seconds: Some(interval_seconds),
                max_buckets: None,
                z_threshold: None,
                threshold_mode: None,
                adaptive_threshold: None,
                detector_mode: None,
                suppression_mode: None,
                exclude_boundary_events: None,
                sparse_point_events_enabled: None,
                gap_max_buckets: None,
                min_separation_buckets: None,
                tolerance_buckets: None,
                min_sensors: None,
                max_results: None,
                max_sensors: None,
                max_events: None,
                focus_sensor_id: Some(focus_sensor_id.to_string()),
                polarity: Some(EventPolarityV1::Both),
                z_cap: None,
                deseason_mode: None,
                periodic_penalty_enabled: None,
                bucket_preference_mode: None,
            },
            buckets,
            truncated_sensor_ids: Vec::new(),
            gap_skipped_deltas: BTreeMap::new(),
            timings_ms: BTreeMap::new(),
            counts: BTreeMap::new(),
            sensor_stats: BTreeMap::new(),
            versions: BTreeMap::new(),
        }
    }

    #[test]
    fn rank_score_is_pool_relative_and_changes_when_candidate_pool_changes() {
        let focus_sensor_id = "sensor-focus";
        let weights = normalize_weights(None, false);

        let event_result = make_event_result(focus_sensor_id, vec![("sensor-a", 1.0), ("sensor-b", 1.0)]);
        let coocc_result = make_cooccurrence_result(focus_sensor_id, &[("sensor-a", 10.0), ("sensor-b", 5.0)]);
        let (candidates_base, _truncated, _coocc_sensors) = merge_unified_candidates(
            focus_sensor_id,
            &event_result,
            &coocc_result,
            weights.clone(),
            CooccurrenceScoreModeV1::AvgProduct,
            true,
            60,
        );
        let b_base = candidates_base
            .iter()
            .find(|c| c.sensor_id == "sensor-b")
            .expect("base b");

        let event_result = make_event_result(
            focus_sensor_id,
            vec![("sensor-a", 1.0), ("sensor-b", 1.0), ("sensor-c", 1.0)],
        );
        let coocc_result = make_cooccurrence_result(
            focus_sensor_id,
            &[("sensor-a", 10.0), ("sensor-b", 5.0), ("sensor-c", 100.0)],
        );
        let (candidates_more, _truncated, _coocc_sensors) = merge_unified_candidates(
            focus_sensor_id,
            &event_result,
            &coocc_result,
            weights,
            CooccurrenceScoreModeV1::AvgProduct,
            true,
            60,
        );
        let b_more = candidates_more
            .iter()
            .find(|c| c.sensor_id == "sensor-b")
            .expect("more b");

        assert!(
            (b_base.blended_score - b_more.blended_score).abs() > 1e-9,
            "expected pool-relative normalization to change rank score when the pool changes"
        );
    }

    #[test]
    fn cooccurrence_avg_normalization_prefers_stronger_average_over_more_buckets() {
        let focus_sensor_id = "sensor-focus";
        let weights = normalize_weights(None, false);

        let event_result =
            make_event_result(focus_sensor_id, vec![("sensor-a", 1.0), ("sensor-b", 1.0)]);

        let interval_seconds = 60;
        let mut buckets: Vec<CooccurrenceBucketV1> = Vec::new();
        for idx in 0..2_i64 {
            let ts = idx * interval_seconds * 1000;
            buckets.push(CooccurrenceBucketV1 {
                ts,
                sensors: vec![
                    CooccurrenceEventV1 {
                        sensor_id: focus_sensor_id.to_string(),
                        ts,
                        z: 10.0,
                        direction: EventDirectionV1::Up,
                        delta: 1.0,
                    },
                    CooccurrenceEventV1 {
                        sensor_id: "sensor-a".to_string(),
                        ts,
                        z: 1.0,
                        direction: EventDirectionV1::Up,
                        delta: 1.0,
                    },
                ],
                group_size: 2,
                severity_sum: 0.0,
                pair_weight: 1.0,
                idf: None,
                score: 1.0,
            });
        }

        for idx in 2..6_i64 {
            let ts = idx * interval_seconds * 1000;
            buckets.push(CooccurrenceBucketV1 {
                ts,
                sensors: vec![
                    CooccurrenceEventV1 {
                        sensor_id: focus_sensor_id.to_string(),
                        ts,
                        z: 3.0,
                        direction: EventDirectionV1::Up,
                        delta: 1.0,
                    },
                    CooccurrenceEventV1 {
                        sensor_id: "sensor-b".to_string(),
                        ts,
                        z: 2.0,
                        direction: EventDirectionV1::Up,
                        delta: 1.0,
                    },
                ],
                group_size: 2,
                severity_sum: 0.0,
                pair_weight: 1.0,
                idf: None,
                score: 1.0,
            });
        }

        let coocc_result = CooccurrenceResultV1 {
            job_type: "cooccurrence_v1".to_string(),
            computed_through_ts: None,
            interval_seconds: Some(interval_seconds),
            bucket_count: Some(100),
            params: CooccurrenceJobParamsV1 {
                sensor_ids: vec![
                    focus_sensor_id.to_string(),
                    "sensor-a".to_string(),
                    "sensor-b".to_string(),
                ],
                start: "2026-02-09T00:00:00Z".to_string(),
                end: "2026-02-10T00:00:00Z".to_string(),
                interval_seconds: Some(interval_seconds),
                max_buckets: None,
                z_threshold: None,
                threshold_mode: None,
                adaptive_threshold: None,
                detector_mode: None,
                suppression_mode: None,
                exclude_boundary_events: None,
                sparse_point_events_enabled: None,
                gap_max_buckets: None,
                min_separation_buckets: None,
                tolerance_buckets: None,
                min_sensors: None,
                max_results: None,
                max_sensors: None,
                max_events: None,
                focus_sensor_id: Some(focus_sensor_id.to_string()),
                polarity: Some(EventPolarityV1::Both),
                z_cap: None,
                deseason_mode: None,
                periodic_penalty_enabled: None,
                bucket_preference_mode: None,
            },
            buckets,
            truncated_sensor_ids: Vec::new(),
            gap_skipped_deltas: BTreeMap::new(),
            timings_ms: BTreeMap::new(),
            counts: BTreeMap::new(),
            sensor_stats: BTreeMap::new(),
            versions: BTreeMap::new(),
        };

        let (candidates, _truncated, _coocc_sensors) = merge_unified_candidates(
            focus_sensor_id,
            &event_result,
            &coocc_result,
            weights,
            CooccurrenceScoreModeV1::AvgProduct,
            true,
            10,
        );
        assert_eq!(candidates.first().unwrap().sensor_id, "sensor-a");
    }

    fn row_at(sensor_id: &str, epoch: i64, value: f64) -> MetricsBucketRow {
        MetricsBucketRow {
            sensor_id: sensor_id.to_string(),
            bucket: DateTime::<Utc>::from_timestamp(epoch, 0).expect("ts"),
            value,
            samples: 1,
        }
    }

    fn stepped_series_with_offset(
        sensor_id: &str,
        interval_seconds: i64,
        buckets: usize,
        spike_every: usize,
        spike_delta: f64,
        offset_buckets: usize,
    ) -> Vec<MetricsBucketRow> {
        let mut rows: Vec<MetricsBucketRow> = Vec::with_capacity(buckets);
        let mut value = 0.0;
        for idx in 0..buckets {
            if idx > 0 && spike_every > 0 {
                let shifted = idx.saturating_sub(offset_buckets);
                if shifted > 0 && shifted % spike_every == 0 {
                    value += spike_delta;
                }
            }
            rows.push(row_at(sensor_id, (idx as i64) * interval_seconds, value));
        }
        rows
    }

    fn count_tolerant_overlap(
        focus_times: &[i64],
        candidate_times: &[i64],
        lag_sec: i64,
        tolerance_sec: i64,
    ) -> u64 {
        if focus_times.is_empty() || candidate_times.is_empty() {
            return 0;
        }
        let tol = tolerance_sec.max(0);
        let mut overlap: u64 = 0;
        let mut focus_idx: usize = 0;
        let mut candidate_idx: usize = 0;
        while focus_idx < focus_times.len() && candidate_idx < candidate_times.len() {
            let target = focus_times[focus_idx] + lag_sec;
            let candidate = candidate_times[candidate_idx];
            if candidate < target - tol {
                candidate_idx += 1;
            } else if candidate > target + tol {
                focus_idx += 1;
            } else {
                overlap += 1;
                focus_idx += 1;
                candidate_idx += 1;
            }
        }
        overlap
    }

    fn f1_score(overlap: u64, n_focus: u64, n_candidate: u64) -> Option<f64> {
        if n_focus == 0 && n_candidate == 0 {
            return None;
        }
        if n_focus == 0 || n_candidate == 0 {
            return Some(0.0);
        }
        Some((2.0 * overlap as f64) / (n_focus + n_candidate) as f64)
    }

    fn best_lag_score(
        focus_times: &[i64],
        candidate_times: &[i64],
        max_lag_buckets: i64,
        interval_seconds: i64,
        tolerance_sec: i64,
        n_candidate: u64,
        n_focus: u64,
    ) -> Option<EventMatchLagScoreV1> {
        if max_lag_buckets <= 0 {
            return None;
        }
        let mut best: Option<EventMatchLagScoreV1> = None;
        for lag in -max_lag_buckets..=max_lag_buckets {
            let lag_sec = lag.saturating_mul(interval_seconds);
            let overlap = count_tolerant_overlap(focus_times, candidate_times, lag_sec, tolerance_sec);
            let score = f1_score(overlap, n_focus, n_candidate);
            let candidate = EventMatchLagScoreV1 {
                lag_sec,
                score,
                overlap,
                n_candidate,
            };
            let candidate_score = candidate.score.unwrap_or(0.0);
            let best_score = best.as_ref().and_then(|b| b.score).unwrap_or(0.0);
            if candidate_score > best_score
                || (candidate_score == best_score
                    && candidate.overlap > best.as_ref().map(|b| b.overlap).unwrap_or(0))
            {
                best = Some(candidate);
            }
        }
        best
    }

    fn compute_bucket_score(
        severity_sum: f64,
        group_size: usize,
        total_sensors: usize,
    ) -> Option<(f64, f64, f64)> {
        if !severity_sum.is_finite() || severity_sum <= 0.0 {
            return None;
        }
        let total_sensors = total_sensors.max(1) as f64;
        let group_size = group_size.max(1) as f64;
        let pair_weight = 1.0 / (2.0 + group_size).ln();
        let idf = ((total_sensors + 1.0) / (group_size + 1.0)).ln();
        let score = severity_sum * pair_weight * idf;
        if !pair_weight.is_finite() || !idf.is_finite() || !score.is_finite() {
            return None;
        }
        Some((pair_weight, idf, score))
    }

    fn build_cooccurrence_buckets(
        focus_sensor_id: &str,
        events_by_sensor: &HashMap<String, Vec<EventPoint>>,
        interval_seconds: i64,
        tolerance_buckets: i64,
        z_cap: f64,
        max_results: usize,
    ) -> Vec<CooccurrenceBucketV1> {
        let mut buckets_by_index: HashMap<i64, HashMap<String, EventPoint>> = HashMap::new();
        for (sensor_id, events) in events_by_sensor {
            for evt in events {
                let bucket_index = evt.ts_epoch / interval_seconds.max(1);
                for offset in -tolerance_buckets..=tolerance_buckets {
                    let idx = bucket_index + offset;
                    let entry = buckets_by_index.entry(idx).or_default();
                    let replace = entry
                        .get(sensor_id)
                        .map(|prev| evt.z.abs() > prev.z.abs())
                        .unwrap_or(true);
                    if replace {
                        entry.insert(sensor_id.clone(), evt.clone());
                    }
                }
            }
        }

        let total_sensors = events_by_sensor.len().max(1);
        let mut candidates: Vec<(i64, CooccurrenceBucketV1, usize)> = Vec::new();
        for (bucket_idx, entry) in buckets_by_index.iter() {
            if entry.len() < 2 {
                continue;
            }
            if !entry.contains_key(focus_sensor_id) {
                continue;
            }
            let mut sensors: Vec<CooccurrenceEventV1> = Vec::new();
            let mut severity_sum = 0.0;
            for (sensor_id, evt) in entry.iter() {
                sensors.push(CooccurrenceEventV1 {
                    sensor_id: sensor_id.clone(),
                    ts: evt.ts.timestamp_millis(),
                    z: evt.z,
                    direction: evt.direction,
                    delta: evt.delta,
                });
                severity_sum += evt.z.abs().min(z_cap);
            }
            sensors.sort_by(|a, b| b.z.abs().total_cmp(&a.z.abs()));

            let group_size = sensors.len();
            let Some((pair_weight, idf, score)) =
                compute_bucket_score(severity_sum, group_size, total_sensors)
            else {
                continue;
            };
            if score <= 0.0 {
                continue;
            }
            let ts_epoch = bucket_idx * interval_seconds.max(1);
            let ts = ts_epoch.saturating_mul(1000);
            candidates.push((
                *bucket_idx,
                CooccurrenceBucketV1 {
                    ts,
                    sensors,
                    group_size: group_size as u32,
                    severity_sum,
                    pair_weight,
                    idf: Some(idf),
                    score,
                },
                group_size,
            ));
        }

        candidates.sort_by(|a, b| {
            b.1.score
                .total_cmp(&a.1.score)
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| b.0.cmp(&a.0))
        });

        let suppression = tolerance_buckets;
        let mut blocked: HashMap<i64, bool> = HashMap::new();
        let mut selected: Vec<CooccurrenceBucketV1> = Vec::new();
        for (bucket_idx, bucket, _group_size) in candidates {
            if selected.len() >= max_results {
                break;
            }
            if *blocked.get(&bucket_idx).unwrap_or(&false) {
                continue;
            }
            selected.push(bucket);
            let start = bucket_idx.saturating_sub(suppression);
            let end = bucket_idx.saturating_add(suppression);
            for idx in start..=end {
                blocked.insert(idx, true);
            }
        }
        selected
    }

    #[test]
    fn contract_harness_runs_unified_over_synthetic_buckets_and_preserves_lag_sign() {
        let focus_sensor_id = "focus";
        let candidate_coocc_sensor_id = "cand_coocc";
        let candidate_lag_sensor_id = "cand_lag";
        let interval_seconds = 60;
        let tolerance_buckets = 0;
        let tolerance_sec = tolerance_buckets * interval_seconds;
        let max_lag_buckets = 2;
        let z_threshold = 0.8;
        let z_cap = 15.0;

        let focus_rows = stepped_series_with_offset(focus_sensor_id, interval_seconds, 25, 4, 100.0, 0);
        let candidate_coocc_rows = stepped_series_with_offset(
            candidate_coocc_sensor_id,
            interval_seconds,
            25,
            4,
            100.0,
            0,
        );
        let candidate_lag_rows =
            stepped_series_with_offset(candidate_lag_sensor_id, interval_seconds, 25, 4, 100.0, 1);

        let focus_detected = detect_change_events(
            &focus_rows,
            interval_seconds,
            z_threshold,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );
        let cand_coocc_detected = detect_change_events(
            &candidate_coocc_rows,
            interval_seconds,
            z_threshold,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );
        let cand_lag_detected = detect_change_events(
            &candidate_lag_rows,
            interval_seconds,
            z_threshold,
            0,
            0,
            EventPolarityV1::Both,
            10_000,
        );

        let mut focus_times: Vec<i64> = focus_detected.events.iter().map(|e| e.ts_epoch).collect();
        focus_times.sort();
        let mut cand_coocc_times: Vec<i64> = cand_coocc_detected
            .events
            .iter()
            .map(|e| e.ts_epoch)
            .collect();
        cand_coocc_times.sort();
        let mut cand_lag_times: Vec<i64> = cand_lag_detected.events.iter().map(|e| e.ts_epoch).collect();
        cand_lag_times.sort();

        let n_focus = focus_times.len() as u64;
        let mk_event_candidate = |sensor_id: &str,
                                  cand_times: &[i64],
                                  rank: u32|
         -> EventMatchCandidateV1 {
            let n_candidate = cand_times.len() as u64;
            let zero_overlap = count_tolerant_overlap(&focus_times, cand_times, 0, tolerance_sec);
            let zero_score = f1_score(zero_overlap, n_focus, n_candidate);
            let zero_lag = EventMatchLagScoreV1 {
                lag_sec: 0,
                score: zero_score,
                overlap: zero_overlap,
                n_candidate,
            };
            let best_lag = best_lag_score(
                &focus_times,
                cand_times,
                max_lag_buckets,
                interval_seconds,
                tolerance_sec,
                n_candidate,
                n_focus,
            )
            .expect("best lag");
            EventMatchCandidateV1 {
                sensor_id: sensor_id.to_string(),
                rank,
                score: best_lag.score,
                overlap: best_lag.overlap,
                n_focus,
                n_candidate,
                n_focus_up: None,
                n_focus_down: None,
                n_candidate_up: None,
                n_candidate_down: None,
                zero_lag,
                best_lag: Some(best_lag),
                top_lags: Vec::new(),
                direction_label: None,
                sign_agreement: None,
                delta_corr: None,
                direction_n: None,
                time_of_day_entropy_norm: None,
                time_of_day_entropy_weight: None,
                episodes: Vec::new(),
                why_ranked: None,
            }
        };

        let event_result = EventMatchResultV1 {
            job_type: "event_match_v1".to_string(),
            focus_sensor_id: focus_sensor_id.to_string(),
            computed_through_ts: None,
            interval_seconds: Some(interval_seconds),
            bucket_count: Some(25),
            params: EventMatchJobParamsV1 {
                focus_sensor_id: focus_sensor_id.to_string(),
                start: "2026-02-09T00:00:00Z".to_string(),
                end: "2026-02-10T00:00:00Z".to_string(),
                focus_events: Vec::new(),
                interval_seconds: Some(interval_seconds),
                candidate_sensor_ids: vec![
                    candidate_coocc_sensor_id.to_string(),
                    candidate_lag_sensor_id.to_string(),
                ],
                candidate_limit: None,
                max_buckets: None,
                max_events: None,
                z_threshold: Some(z_threshold),
                threshold_mode: None,
                adaptive_threshold: None,
                detector_mode: None,
                suppression_mode: None,
                exclude_boundary_events: None,
                sparse_point_events_enabled: None,
                min_separation_buckets: None,
                max_lag_buckets: Some(max_lag_buckets),
                top_k_lags: None,
                tolerance_buckets: Some(tolerance_buckets),
                max_episodes: None,
                episode_gap_buckets: None,
                gap_max_buckets: None,
                polarity: Some(EventPolarityV1::Both),
                z_cap: Some(z_cap),
                deseason_mode: None,
                periodic_penalty_enabled: None,
                filters: TsseCandidateFiltersV1::default(),
            },
            candidates: vec![
                mk_event_candidate(candidate_coocc_sensor_id, &cand_coocc_times, 1),
                mk_event_candidate(candidate_lag_sensor_id, &cand_lag_times, 2),
            ],
            truncated_sensor_ids: Vec::new(),
            gap_skipped_deltas: BTreeMap::new(),
            monitoring: None,
            timings_ms: BTreeMap::new(),
            versions: BTreeMap::new(),
        };

        let mut events_by_sensor: HashMap<String, Vec<EventPoint>> = HashMap::new();
        events_by_sensor.insert(focus_sensor_id.to_string(), focus_detected.events);
        events_by_sensor.insert(candidate_coocc_sensor_id.to_string(), cand_coocc_detected.events);
        events_by_sensor.insert(candidate_lag_sensor_id.to_string(), cand_lag_detected.events);

        let coocc_buckets = build_cooccurrence_buckets(
            focus_sensor_id,
            &events_by_sensor,
            interval_seconds,
            tolerance_buckets,
            z_cap,
            128,
        );

        let coocc_result = CooccurrenceResultV1 {
            job_type: "cooccurrence_v1".to_string(),
            computed_through_ts: None,
            interval_seconds: Some(interval_seconds),
            bucket_count: Some(25),
            params: CooccurrenceJobParamsV1 {
                sensor_ids: vec![
                    focus_sensor_id.to_string(),
                    candidate_coocc_sensor_id.to_string(),
                    candidate_lag_sensor_id.to_string(),
                ],
                start: "2026-02-09T00:00:00Z".to_string(),
                end: "2026-02-10T00:00:00Z".to_string(),
                interval_seconds: Some(interval_seconds),
                max_buckets: None,
                z_threshold: Some(z_threshold),
                threshold_mode: None,
                adaptive_threshold: None,
                detector_mode: None,
                suppression_mode: None,
                exclude_boundary_events: None,
                sparse_point_events_enabled: None,
                gap_max_buckets: None,
                min_separation_buckets: None,
                tolerance_buckets: Some(tolerance_buckets),
                min_sensors: Some(2),
                max_results: None,
                max_sensors: None,
                max_events: None,
                focus_sensor_id: Some(focus_sensor_id.to_string()),
                polarity: Some(EventPolarityV1::Both),
                z_cap: Some(z_cap),
                deseason_mode: None,
                periodic_penalty_enabled: None,
                bucket_preference_mode: None,
            },
            buckets: coocc_buckets,
            truncated_sensor_ids: Vec::new(),
            gap_skipped_deltas: BTreeMap::new(),
            timings_ms: BTreeMap::new(),
            counts: BTreeMap::new(),
            sensor_stats: BTreeMap::new(),
            versions: BTreeMap::new(),
        };

        let weights = normalize_weights(None, false);
        let (candidates, _truncated, _coocc_sensors) = merge_unified_candidates(
            focus_sensor_id,
            &event_result,
            &coocc_result,
            weights,
            CooccurrenceScoreModeV1::AvgProduct,
            true,
            60,
        );
        assert_eq!(candidates.len(), 2);

        let cand_coocc = candidates
            .iter()
            .find(|c| c.sensor_id == candidate_coocc_sensor_id)
            .expect("cand_coocc");
        assert_eq!(cand_coocc.evidence.best_lag_sec, Some(0));
        assert_eq!(cand_coocc.evidence.cooccurrence_strength, Some(1.0));

        let cand_lag = candidates
            .iter()
            .find(|c| c.sensor_id == candidate_lag_sensor_id)
            .expect("cand_lag");
        assert_eq!(cand_lag.evidence.best_lag_sec, Some(60));
        assert!(
            cand_lag
                .evidence
                .summary
                .iter()
                .any(|line| line.contains("Best lag 60s")),
            "expected best lag summary line"
        );
        assert_eq!(cand_lag.evidence.cooccurrence_strength, None);
    }
}
