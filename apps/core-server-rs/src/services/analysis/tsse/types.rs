use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TsseJobTypeV1 {
    RelatedSensorsV1,
    RelatedSensorsUnifiedV2,
    CorrelationMatrixV1,
    EventMatchV1,
    CooccurrenceV1,
    MatrixProfileV1,
    EmbeddingsBuildV1,
}

impl TsseJobTypeV1 {
    pub fn as_str(&self) -> &'static str {
        match self {
            TsseJobTypeV1::RelatedSensorsV1 => "related_sensors_v1",
            TsseJobTypeV1::RelatedSensorsUnifiedV2 => "related_sensors_unified_v2",
            TsseJobTypeV1::CorrelationMatrixV1 => "correlation_matrix_v1",
            TsseJobTypeV1::EventMatchV1 => "event_match_v1",
            TsseJobTypeV1::CooccurrenceV1 => "cooccurrence_v1",
            TsseJobTypeV1::MatrixProfileV1 => "matrix_profile_v1",
            TsseJobTypeV1::EmbeddingsBuildV1 => "embeddings_build_v1",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TsseCandidateFiltersV1 {
    #[serde(default)]
    pub same_node_only: bool,
    #[serde(default)]
    pub same_unit_only: bool,
    #[serde(default)]
    pub same_type_only: bool,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
    #[serde(default)]
    pub is_derived: Option<bool>,
    #[serde(default)]
    pub is_public_provider: Option<bool>,
    #[serde(default)]
    pub exclude_sensor_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsJobParamsV1 {
    pub focus_sensor_id: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub interval_seconds: i64,
    #[serde(default)]
    pub candidate_limit: Option<u32>,
    #[serde(default)]
    pub min_pool: Option<u32>,
    #[serde(default)]
    pub lag_max_seconds: Option<i64>,
    #[serde(default)]
    pub min_significant_n: Option<u32>,
    #[serde(default)]
    pub significance_alpha: Option<f64>,
    #[serde(default)]
    pub min_abs_r: Option<f64>,
    #[serde(default)]
    pub bucket_aggregation_mode: Option<BucketAggregationModeV1>,
    #[serde(default)]
    pub filters: TsseCandidateFiltersV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EmbeddingsBuildJobParamsV1 {
    #[serde(default)]
    pub start: Option<String>,
    #[serde(default)]
    pub end: Option<String>,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
    #[serde(default)]
    pub horizon_days: Option<i64>,
    #[serde(default)]
    pub batch_size: Option<u32>,
    #[serde(default)]
    pub sensor_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EmbeddingsBuildResultV1 {
    pub job_type: String,
    pub embedding_version: String,
    pub window_seconds: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_through_ts: Option<String>,
    pub sensors_total: u64,
    pub sensors_embedded: u64,
    pub sensors_skipped: u64,
    pub points_upserted: u64,
    #[serde(default)]
    pub timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationMethodV1 {
    Pearson,
    Spearman,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BucketAggregationModeV1 {
    Auto,
    Avg,
    Last,
    Sum,
    Min,
    Max,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationValueModeV1 {
    Levels,
    Deltas,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationLagModeV1 {
    Aligned,
    BestWithinMax,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CorrelationMatrixJobParamsV1 {
    pub sensor_ids: Vec<String>,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
    #[serde(default)]
    pub method: Option<CorrelationMethodV1>,
    #[serde(default)]
    pub max_sensors: Option<u32>,
    #[serde(default)]
    pub max_buckets: Option<u32>,
    #[serde(default)]
    pub min_overlap: Option<u32>,
    #[serde(default)]
    pub min_significant_n: Option<u32>,
    #[serde(default)]
    pub significance_alpha: Option<f64>,
    #[serde(default)]
    pub min_abs_r: Option<f64>,
    #[serde(default)]
    pub bucket_aggregation_mode: Option<BucketAggregationModeV1>,
    #[serde(default)]
    pub value_mode: Option<CorrelationValueModeV1>,
    #[serde(default)]
    pub lag_mode: Option<CorrelationLagModeV1>,
    #[serde(default)]
    pub max_lag_buckets: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CorrelationMatrixSensorV1 {
    pub sensor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensor_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CorrelationMatrixCellV1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_ci_low: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r_ci_high: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_eff: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<CorrelationMatrixCellStatusV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lag_sec: Option<i64>,
    pub n: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CorrelationMatrixCellStatusV1 {
    Ok,
    InsufficientOverlap,
    NotSignificant,
    NotComputed,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CorrelationMatrixResultV1 {
    pub job_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_through_ts: Option<String>,
    pub params: CorrelationMatrixJobParamsV1,
    pub sensor_ids: Vec<String>,
    pub sensors: Vec<CorrelationMatrixSensorV1>,
    pub matrix: Vec<Vec<CorrelationMatrixCellV1>>,
    pub interval_seconds: i64,
    pub bucket_count: u64,
    #[serde(default)]
    pub truncated_sensor_ids: Vec<String>,
    #[serde(default)]
    pub timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventPolarityV1 {
    Both,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventThresholdModeV1 {
    FixedZ,
    AdaptiveRate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventSuppressionModeV1 {
    GreedyMinSeparation,
    NmsWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventDetectorModeV1 {
    /// Robust z-score on bucket-to-bucket deltas (Δ).
    BucketDeltas,
    /// Robust z-score on second differences (Δ²) to emphasize ramp boundaries.
    BucketSecondDeltas,
    /// Robust z-score on bucket levels (point-events).
    BucketLevels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CooccurrenceBucketPreferenceModeV1 {
    PreferSpecificMatches,
    PreferSystemWideMatches,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CooccurrenceScoreModeV1 {
    AvgProduct,
    Surprise,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AdaptiveThresholdConfigV1 {
    #[serde(default)]
    pub target_min_events: Option<u32>,
    #[serde(default)]
    pub target_max_events: Option<u32>,
    #[serde(default)]
    pub min_z: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventDirectionV1 {
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DirectionLabelV1 {
    Same,
    Opposite,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeseasonModeV1 {
    None,
    HourOfDayMean,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ExplicitFocusEventV1 {
    /// RFC3339 timestamp (UTC recommended).
    pub ts: String,
    /// Optional severity/weight for this focus event (higher = more weight).
    #[serde(default)]
    pub severity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EventMatchJobParamsV1 {
    pub focus_sensor_id: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub focus_events: Vec<ExplicitFocusEventV1>,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
    #[serde(default)]
    pub candidate_sensor_ids: Vec<String>,
    #[serde(default)]
    pub candidate_limit: Option<u32>,
    #[serde(default)]
    pub max_buckets: Option<u32>,
    #[serde(default)]
    pub max_events: Option<u32>,
    #[serde(default)]
    pub z_threshold: Option<f64>,
    #[serde(default)]
    pub threshold_mode: Option<EventThresholdModeV1>,
    #[serde(default)]
    pub adaptive_threshold: Option<AdaptiveThresholdConfigV1>,
    #[serde(default)]
    pub detector_mode: Option<EventDetectorModeV1>,
    #[serde(default)]
    pub suppression_mode: Option<EventSuppressionModeV1>,
    #[serde(default)]
    pub exclude_boundary_events: Option<bool>,
    #[serde(default)]
    pub sparse_point_events_enabled: Option<bool>,
    #[serde(default)]
    pub min_separation_buckets: Option<i64>,
    #[serde(default)]
    pub max_lag_buckets: Option<i64>,
    #[serde(default)]
    pub top_k_lags: Option<u32>,
    #[serde(default)]
    pub tolerance_buckets: Option<i64>,
    #[serde(default)]
    pub max_episodes: Option<u32>,
    #[serde(default)]
    pub episode_gap_buckets: Option<i64>,
    #[serde(default)]
    pub gap_max_buckets: Option<i64>,
    #[serde(default)]
    pub polarity: Option<EventPolarityV1>,
    #[serde(default)]
    pub z_cap: Option<f64>,
    #[serde(default)]
    pub deseason_mode: Option<DeseasonModeV1>,
    #[serde(default)]
    pub periodic_penalty_enabled: Option<bool>,
    #[serde(default)]
    pub filters: TsseCandidateFiltersV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EventMatchLagScoreV1 {
    pub lag_sec: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    pub overlap: u64,
    pub n_candidate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EventMatchCandidateV1 {
    pub sensor_id: String,
    pub rank: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    pub overlap: u64,
    pub n_focus: u64,
    pub n_candidate: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_focus_up: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_focus_down: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_candidate_up: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_candidate_down: Option<u64>,
    pub zero_lag: EventMatchLagScoreV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_lag: Option<EventMatchLagScoreV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_lags: Vec<EventMatchLagScoreV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_label: Option<DirectionLabelV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_agreement: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_corr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_n: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_of_day_entropy_norm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_of_day_entropy_weight: Option<f64>,
    #[serde(default)]
    pub episodes: Vec<TsseEpisodeV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why_ranked: Option<TsseWhyRankedV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EventEvidenceMonitoringV1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_abs_dz_p50: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_abs_dz_p90: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_abs_dz_p95: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_abs_dz_p99: Option<f64>,
    pub z_cap: f64,
    pub events_total: u64,
    pub z_clipped_events: u64,
    pub z_clipped_pct: f64,
    pub delta_points_total: u64,
    pub gap_skipped_deltas_total: u64,
    pub gap_skipped_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EventMatchResultV1 {
    pub job_type: String,
    pub focus_sensor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_through_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_count: Option<u64>,
    pub params: EventMatchJobParamsV1,
    pub candidates: Vec<EventMatchCandidateV1>,
    #[serde(default)]
    pub truncated_sensor_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gap_skipped_deltas: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitoring: Option<EventEvidenceMonitoringV1>,
    #[serde(default)]
    pub timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CooccurrenceJobParamsV1 {
    pub sensor_ids: Vec<String>,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
    #[serde(default)]
    pub max_buckets: Option<u32>,
    #[serde(default)]
    pub z_threshold: Option<f64>,
    #[serde(default)]
    pub threshold_mode: Option<EventThresholdModeV1>,
    #[serde(default)]
    pub adaptive_threshold: Option<AdaptiveThresholdConfigV1>,
    #[serde(default)]
    pub detector_mode: Option<EventDetectorModeV1>,
    #[serde(default)]
    pub suppression_mode: Option<EventSuppressionModeV1>,
    #[serde(default)]
    pub exclude_boundary_events: Option<bool>,
    #[serde(default)]
    pub sparse_point_events_enabled: Option<bool>,
    #[serde(default)]
    pub gap_max_buckets: Option<i64>,
    #[serde(default)]
    pub min_separation_buckets: Option<i64>,
    #[serde(default)]
    pub tolerance_buckets: Option<i64>,
    #[serde(default)]
    pub min_sensors: Option<u32>,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub max_sensors: Option<u32>,
    #[serde(default)]
    pub max_events: Option<u32>,
    #[serde(default)]
    pub focus_sensor_id: Option<String>,
    #[serde(default)]
    pub polarity: Option<EventPolarityV1>,
    #[serde(default)]
    pub z_cap: Option<f64>,
    #[serde(default)]
    pub deseason_mode: Option<DeseasonModeV1>,
    #[serde(default)]
    pub periodic_penalty_enabled: Option<bool>,
    #[serde(default)]
    pub bucket_preference_mode: Option<CooccurrenceBucketPreferenceModeV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CooccurrenceEventV1 {
    pub sensor_id: String,
    pub ts: i64,
    pub z: f64,
    pub direction: EventDirectionV1,
    pub delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CooccurrenceBucketV1 {
    pub ts: i64,
    pub sensors: Vec<CooccurrenceEventV1>,
    pub group_size: u32,
    pub severity_sum: f64,
    pub pair_weight: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idf: Option<f64>,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CooccurrenceSensorStatsV1 {
    pub n_events: u64,
    pub mean_abs_z: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CooccurrenceResultV1 {
    pub job_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_through_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_count: Option<u64>,
    pub params: CooccurrenceJobParamsV1,
    pub buckets: Vec<CooccurrenceBucketV1>,
    #[serde(default)]
    pub truncated_sensor_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gap_skipped_deltas: BTreeMap<String, u64>,
    #[serde(default)]
    pub timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub counts: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sensor_stats: BTreeMap<String, CooccurrenceSensorStatsV1>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MatrixProfileJobParamsV1 {
    pub sensor_id: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
    #[serde(default)]
    pub window_points: Option<u32>,
    #[serde(default)]
    pub exclusion_zone: Option<u32>,
    #[serde(default)]
    pub max_points: Option<u32>,
    #[serde(default)]
    pub max_windows: Option<u32>,
    #[serde(default)]
    pub top_k: Option<u32>,
    #[serde(default)]
    pub max_compute_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MatrixProfileWindowV1 {
    pub window_index: u32,
    pub start_ts: String,
    pub end_ts: String,
    pub distance: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_start_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_end_ts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct MatrixProfileResultV1 {
    pub job_type: String,
    pub sensor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensor_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_through_ts: Option<String>,
    pub params: MatrixProfileJobParamsV1,
    pub interval_seconds: i64,
    pub window_points: u32,
    pub window: u32,
    pub exclusion_zone: u32,
    pub timestamps: Vec<String>,
    pub values: Vec<f64>,
    pub window_start_ts: Vec<String>,
    pub profile: Vec<f64>,
    pub profile_index: Vec<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_interval_seconds: Option<i64>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub motifs: Vec<MatrixProfileWindowV1>,
    #[serde(default)]
    pub anomalies: Vec<MatrixProfileWindowV1>,
    pub source_points: u64,
    pub sampled_points: u64,
    #[serde(default)]
    pub timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TsseEmbeddingHitV1 {
    pub vector: String,
    pub rank: u32,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TsseAnnInfoV1 {
    pub widen_stage: u32,
    pub union_pool_size: u32,
    pub embedding_hits: Vec<TsseEmbeddingHitV1>,
    pub filters_applied: TsseCandidateFiltersV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TsseEpisodeV1 {
    pub start_ts: String,
    pub end_ts: String,
    pub window_sec: i64,
    pub lag_sec: i64,
    pub lag_iqr_sec: i64,
    pub score_mean: f64,
    pub score_peak: f64,
    pub coverage: f64,
    pub num_points: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TsseWhyRankedV1 {
    pub episode_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_window_sec: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_lag_sec: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_lag_r_ci_low: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_lag_r_ci_high: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_pct: Option<f64>,
    #[serde(default)]
    pub score_components: BTreeMap<String, f64>,
    #[serde(default)]
    pub penalties: Vec<String>,
    #[serde(default)]
    pub bonuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorCandidateV1 {
    pub sensor_id: String,
    pub rank: u32,
    pub score: f64,
    pub ann: TsseAnnInfoV1,
    pub episodes: Vec<TsseEpisodeV1>,
    pub why_ranked: TsseWhyRankedV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsResultV1 {
    pub job_type: String,
    pub focus_sensor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_through_ts: Option<String>,
    pub params: RelatedSensorsJobParamsV1,
    pub candidates: Vec<RelatedSensorCandidateV1>,
    #[serde(default)]
    pub timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedRelationshipModeV2 {
    Simple,
    Advanced,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedCandidateSourceV2 {
    VisibleInTrends,
    AllSensorsInScope,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedConfidenceTierV2 {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedStabilityTierV1 {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedStabilityStatusV1 {
    Computed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedEvidenceSourceV1 {
    DeltaZ,
    Pattern,
    Blend,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsUnifiedStabilityV1 {
    pub status: UnifiedStabilityStatusV1,
    pub k: u32,
    pub window_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<UnifiedStabilityTierV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlaps: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UnifiedStrategyWeightsV2 {
    pub events: f64,
    pub cooccurrence: f64,
    #[serde(default)]
    pub delta_corr: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsUnifiedJobParamsV2 {
    pub focus_sensor_id: String,
    pub start: String,
    pub end: String,
    #[serde(default)]
    pub focus_events: Vec<ExplicitFocusEventV1>,
    #[serde(default)]
    pub interval_seconds: Option<i64>,
    #[serde(default)]
    pub mode: Option<UnifiedRelationshipModeV2>,
    #[serde(default)]
    pub candidate_source: Option<UnifiedCandidateSourceV2>,
    #[serde(default)]
    pub candidate_sensor_ids: Vec<String>,
    #[serde(default)]
    pub pinned_sensor_ids: Vec<String>,
    #[serde(default)]
    pub evaluate_all_eligible: Option<bool>,
    #[serde(default)]
    pub candidate_limit: Option<u32>,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub include_low_confidence: Option<bool>,
    #[serde(default)]
    pub quick_suggest: Option<bool>,
    #[serde(default)]
    pub stability_enabled: Option<bool>,
    #[serde(default)]
    pub exclude_system_wide_buckets: Option<bool>,
    #[serde(default)]
    pub filters: TsseCandidateFiltersV1,
    #[serde(default)]
    pub weights: Option<UnifiedStrategyWeightsV2>,
    #[serde(default)]
    pub polarity: Option<EventPolarityV1>,
    #[serde(default)]
    pub z_threshold: Option<f64>,
    #[serde(default)]
    pub threshold_mode: Option<EventThresholdModeV1>,
    #[serde(default)]
    pub adaptive_threshold: Option<AdaptiveThresholdConfigV1>,
    #[serde(default)]
    pub detector_mode: Option<EventDetectorModeV1>,
    #[serde(default)]
    pub suppression_mode: Option<EventSuppressionModeV1>,
    #[serde(default)]
    pub exclude_boundary_events: Option<bool>,
    #[serde(default)]
    pub sparse_point_events_enabled: Option<bool>,
    #[serde(default)]
    pub z_cap: Option<f64>,
    #[serde(default)]
    pub min_separation_buckets: Option<i64>,
    #[serde(default)]
    pub gap_max_buckets: Option<i64>,
    #[serde(default)]
    pub max_lag_buckets: Option<i64>,
    #[serde(default)]
    pub max_events: Option<u32>,
    #[serde(default)]
    pub max_episodes: Option<u32>,
    #[serde(default)]
    pub episode_gap_buckets: Option<i64>,
    #[serde(default)]
    pub tolerance_buckets: Option<i64>,
    #[serde(default)]
    pub min_sensors: Option<u32>,
    #[serde(default)]
    pub include_delta_corr_signal: Option<bool>,
    #[serde(default)]
    pub deseason_mode: Option<DeseasonModeV1>,
    #[serde(default)]
    pub periodic_penalty_enabled: Option<bool>,
    #[serde(default)]
    pub cooccurrence_score_mode: Option<CooccurrenceScoreModeV1>,
    #[serde(default)]
    pub cooccurrence_bucket_preference_mode: Option<CooccurrenceBucketPreferenceModeV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsUnifiedEvidenceV2 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooccurrence_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooccurrence_avg: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooccurrence_surprise: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooccurrence_strength: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events_overlap: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_focus: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_candidate: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_focus_up: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_focus_down: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_candidate_up: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_candidate_down: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooccurrence_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focus_bucket_coverage_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_bucket_coverage_pct: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_lag_sec: Option<i64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_lags: Vec<EventMatchLagScoreV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_label: Option<DirectionLabelV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_agreement: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_corr: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_n: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_of_day_entropy_norm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_of_day_entropy_weight: Option<f64>,
    #[serde(default)]
    pub summary: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsUnifiedCandidateV2 {
    pub sensor_id: String,
    #[serde(default)]
    pub derived_from_focus: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_dependency_path: Option<Vec<String>>,
    pub rank: u32,
    pub blended_score: f64,
    pub confidence_tier: UnifiedConfidenceTierV2,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episodes: Option<Vec<TsseEpisodeV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_bucket_timestamps: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why_ranked: Option<TsseWhyRankedV1>,
    pub evidence: RelatedSensorsUnifiedEvidenceV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsUnifiedLimitsUsedV2 {
    pub candidate_limit_used: u32,
    pub max_results_used: u32,
    pub max_sensors_used: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedCandidateSkipReasonV2 {
    NoLakeHistory,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsUnifiedSkippedCandidateV2 {
    pub sensor_id: String,
    pub reason: UnifiedCandidateSkipReasonV2,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SystemWideCooccurrenceBucketV1 {
    pub ts: i64,
    pub group_size: u32,
    pub severity_sum: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RelatedSensorsUnifiedResultV2 {
    pub job_type: String,
    pub focus_sensor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_source: Option<UnifiedEvidenceSourceV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_through_ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_seconds: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_count: Option<u64>,
    pub params: RelatedSensorsUnifiedJobParamsV2,
    pub limits_used: RelatedSensorsUnifiedLimitsUsedV2,
    #[serde(default)]
    pub candidates: Vec<RelatedSensorsUnifiedCandidateV2>,
    #[serde(default)]
    pub skipped_candidates: Vec<RelatedSensorsUnifiedSkippedCandidateV2>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub system_wide_buckets: Vec<SystemWideCooccurrenceBucketV1>,
    #[serde(default)]
    pub prefiltered_candidate_sensor_ids: Vec<String>,
    #[serde(default)]
    pub truncated_candidate_sensor_ids: Vec<String>,
    #[serde(default)]
    pub truncated_result_sensor_ids: Vec<String>,
    #[serde(default)]
    pub timings_ms: BTreeMap<String, u64>,
    #[serde(default)]
    pub counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub versions: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitoring: Option<EventEvidenceMonitoringV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stability: Option<RelatedSensorsUnifiedStabilityV1>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gap_skipped_deltas: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PreviewEventOverlayParamsV1 {
    #[serde(default)]
    pub z_threshold: Option<f64>,
    #[serde(default)]
    pub min_separation_buckets: Option<i64>,
    #[serde(default)]
    pub gap_max_buckets: Option<i64>,
    #[serde(default)]
    pub polarity: Option<EventPolarityV1>,
    #[serde(default)]
    pub max_events: Option<u32>,
    #[serde(default)]
    pub tolerance_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TssePreviewRequestV1 {
    pub focus_sensor_id: String,
    pub candidate_sensor_id: String,
    #[serde(default)]
    pub episode_start_ts: Option<String>,
    #[serde(default)]
    pub episode_end_ts: Option<String>,
    #[serde(default)]
    pub lag_seconds: Option<i64>,
    #[serde(default)]
    pub max_points: Option<u32>,
    #[serde(default)]
    pub event_overlay: Option<PreviewEventOverlayParamsV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TssePreviewSeriesPointV1 {
    pub timestamp: String,
    pub value: f64,
    pub samples: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TssePreviewSeriesV1 {
    pub sensor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensor_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_coverage_pct: Option<f64>,
    pub points: Vec<TssePreviewSeriesPointV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PreviewEventOverlaysV1 {
    #[serde(default)]
    pub focus_event_ts_ms: Vec<i64>,
    #[serde(default)]
    pub candidate_event_ts_ms: Vec<i64>,
    #[serde(default)]
    pub matched_focus_event_ts_ms: Vec<i64>,
    #[serde(default)]
    pub matched_candidate_event_ts_ms: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tolerance_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TssePreviewResponseV1 {
    pub focus: TssePreviewSeriesV1,
    pub candidate: TssePreviewSeriesV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_aligned: Option<TssePreviewSeriesV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_episode: Option<TsseEpisodeV1>,
    pub bucket_seconds: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_overlays: Option<PreviewEventOverlaysV1>,
}
