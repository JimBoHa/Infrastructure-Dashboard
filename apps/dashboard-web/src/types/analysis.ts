export type AnalysisJobStatus = "pending" | "running" | "completed" | "failed" | "canceled";

export type AnalysisJobProgress = {
  phase: string;
  completed: number;
  total?: number | null;
  message?: string | null;
};

export type AnalysisJobError = {
  code: string;
  message: string;
  details?: unknown;
};

export type AnalysisJobPublic = {
  id: string;
  job_type: string;
  status: AnalysisJobStatus;
  job_key?: string | null;
  created_by?: string | null;
  created_at: string;
  updated_at: string;
  started_at?: string | null;
  completed_at?: string | null;
  canceled_at?: string | null;
  progress: AnalysisJobProgress;
  error?: AnalysisJobError | null;
};

export type AnalysisJobCreateRequest = {
  job_type: string;
  params: unknown;
  job_key?: string | null;
  dedupe?: boolean;
};

export type AnalysisJobCreateResponse = {
  job: AnalysisJobPublic;
};

export type AnalysisJobStatusResponse = {
  job: AnalysisJobPublic;
};

export type AnalysisJobCancelResponse = {
  job: AnalysisJobPublic;
};

export type AnalysisJobEventPublic = {
  id: number;
  created_at: string;
  kind: string;
  payload: unknown;
};

export type AnalysisJobEventsResponse = {
  events: AnalysisJobEventPublic[];
  next_after?: number | null;
};

export type AnalysisJobResultResponse<T = unknown> = {
  job_id: string;
  result: T;
};

export type EventPolarityV1 = "both" | "up" | "down";
export type DirectionLabelV1 = "same" | "opposite" | "unknown";

export type EventThresholdModeV1 = "fixed_z" | "adaptive_rate";
export type EventSuppressionModeV1 = "greedy_min_separation" | "nms_window";
export type EventDetectorModeV1 =
  | "bucket_deltas"
  | "bucket_second_deltas"
  | "bucket_levels";

export type CooccurrenceBucketPreferenceModeV1 =
  | "prefer_specific_matches"
  | "prefer_system_wide_matches";

export type CooccurrenceScoreModeV1 = "avg_product" | "surprise";

export type AdaptiveThresholdConfigV1 = {
  target_min_events?: number | null;
  target_max_events?: number | null;
  min_z?: number | null;
};

export type TsseCandidateFiltersV1 = {
  same_node_only?: boolean;
  same_unit_only?: boolean;
  same_type_only?: boolean;
  interval_seconds?: number | null;
  is_derived?: boolean | null;
  is_public_provider?: boolean | null;
  exclude_sensor_ids?: string[];
};

export type BucketAggregationModeV1 =
  | "auto"
  | "avg"
  | "last"
  | "sum"
  | "min"
  | "max";

export type RelatedSensorsJobParamsV1 = {
  focus_sensor_id: string;
  start: string;
  end: string;
  interval_seconds?: number;
  candidate_limit?: number | null;
  min_pool?: number | null;
  lag_max_seconds?: number | null;
  min_significant_n?: number | null;
  significance_alpha?: number | null;
  min_abs_r?: number | null;
  bucket_aggregation_mode?: BucketAggregationModeV1 | null;
  filters: TsseCandidateFiltersV1;
};

export type TsseEmbeddingHitV1 = {
  vector: string;
  rank: number;
  score: number;
};

export type TsseAnnInfoV1 = {
  widen_stage: number;
  union_pool_size: number;
  embedding_hits: TsseEmbeddingHitV1[];
  filters_applied: TsseCandidateFiltersV1;
};

export type TsseEpisodeV1 = {
  start_ts: string;
  end_ts: string;
  window_sec: number;
  lag_sec: number;
  lag_iqr_sec: number;
  score_mean: number;
  score_peak: number;
  coverage: number;
  num_points: number;
};

export type TsseWhyRankedV1 = {
  episode_count: number;
  best_window_sec?: number | null;
  best_lag_sec?: number | null;
  best_lag_r_ci_low?: number | null;
  best_lag_r_ci_high?: number | null;
  coverage_pct?: number | null;
  score_components?: Record<string, number>;
  penalties?: string[];
  bonuses?: string[];
};

export type RelatedSensorCandidateV1 = {
  sensor_id: string;
  rank: number;
  score: number;
  ann: TsseAnnInfoV1;
  episodes: TsseEpisodeV1[];
  why_ranked: TsseWhyRankedV1;
};

export type RelatedSensorsResultV1 = {
  job_type: string;
  focus_sensor_id: string;
  computed_through_ts?: string | null;
  params: RelatedSensorsJobParamsV1;
  candidates: RelatedSensorCandidateV1[];
  timings_ms?: Record<string, number>;
  versions?: Record<string, string>;
};

export type UnifiedRelationshipModeV2 = "simple" | "advanced";
export type UnifiedConfidenceTierV2 = "high" | "medium" | "low";
export type UnifiedCandidateSourceV2 = "visible_in_trends" | "all_sensors_in_scope";

export type UnifiedStabilityTierV1 = "high" | "medium" | "low";
export type UnifiedStabilityStatusV1 = "computed" | "skipped";
export type UnifiedEvidenceSourceV1 = "delta_z" | "pattern" | "blend";

export type ExplicitFocusEventV1 = {
  ts: string;
  severity?: number | null;
};

export type RelatedSensorsUnifiedStabilityV1 = {
  status: UnifiedStabilityStatusV1;
  k: number;
  window_count: number;
  score?: number | null;
  tier?: UnifiedStabilityTierV1 | null;
  overlaps?: number[];
  reason?: string | null;
};

export type EventEvidenceMonitoringV1 = {
  peak_abs_dz_p50?: number | null;
  peak_abs_dz_p90?: number | null;
  peak_abs_dz_p95?: number | null;
  peak_abs_dz_p99?: number | null;
  z_cap: number;
  events_total: number;
  z_clipped_events: number;
  z_clipped_pct: number;
  delta_points_total: number;
  gap_skipped_deltas_total: number;
  gap_skipped_pct: number;
};

export type UnifiedStrategyWeightsV2 = {
  events: number;
  cooccurrence: number;
  delta_corr?: number | null;
};

export type DeseasonModeV1 = "none" | "hour_of_day_mean";

export type RelatedSensorsUnifiedJobParamsV2 = {
  focus_sensor_id: string;
  start: string;
  end: string;
  focus_events?: ExplicitFocusEventV1[];
  interval_seconds?: number | null;
  mode?: UnifiedRelationshipModeV2 | null;
  candidate_source?: UnifiedCandidateSourceV2 | null;
  candidate_sensor_ids?: string[];
  pinned_sensor_ids?: string[];
  evaluate_all_eligible?: boolean | null;
  candidate_limit?: number | null;
  max_results?: number | null;
  include_low_confidence?: boolean | null;
  quick_suggest?: boolean | null;
  stability_enabled?: boolean | null;
  exclude_system_wide_buckets?: boolean | null;
  filters: TsseCandidateFiltersV1;
  weights?: UnifiedStrategyWeightsV2 | null;
  polarity?: EventPolarityV1 | null;
  z_threshold?: number | null;
  threshold_mode?: EventThresholdModeV1 | null;
  adaptive_threshold?: AdaptiveThresholdConfigV1 | null;
  detector_mode?: EventDetectorModeV1 | null;
  suppression_mode?: EventSuppressionModeV1 | null;
  exclude_boundary_events?: boolean | null;
  sparse_point_events_enabled?: boolean | null;
  z_cap?: number | null;
  min_separation_buckets?: number | null;
  gap_max_buckets?: number | null;
  max_lag_buckets?: number | null;
  max_events?: number | null;
  max_episodes?: number | null;
  episode_gap_buckets?: number | null;
  tolerance_buckets?: number | null;
  min_sensors?: number | null;
  include_delta_corr_signal?: boolean | null;
  deseason_mode?: DeseasonModeV1 | null;
  periodic_penalty_enabled?: boolean | null;
  cooccurrence_score_mode?: CooccurrenceScoreModeV1 | null;
  cooccurrence_bucket_preference_mode?: CooccurrenceBucketPreferenceModeV1 | null;
};

export type RelatedSensorsUnifiedCandidateV2 = {
  sensor_id: string;
  derived_from_focus?: boolean | null;
  derived_dependency_path?: string[] | null;
  rank: number;
  blended_score: number;
  confidence_tier: UnifiedConfidenceTierV2;
  episodes?: TsseEpisodeV1[] | null;
  top_bucket_timestamps?: number[] | null;
  why_ranked?: TsseWhyRankedV1 | null;
  evidence: {
    events_score?: number | null;
    cooccurrence_score?: number | null;
    cooccurrence_avg?: number | null;
    cooccurrence_surprise?: number | null;
    cooccurrence_strength?: number | null;
    events_overlap?: number | null;
    n_focus?: number | null;
    n_candidate?: number | null;
    n_focus_up?: number | null;
    n_focus_down?: number | null;
    n_candidate_up?: number | null;
    n_candidate_down?: number | null;
    cooccurrence_count?: number | null;
    focus_bucket_coverage_pct?: number | null;
    candidate_bucket_coverage_pct?: number | null;
    best_lag_sec?: number | null;
    top_lags?: EventMatchLagScoreV1[];
    direction_label?: DirectionLabelV1 | null;
    sign_agreement?: number | null;
    delta_corr?: number | null;
    direction_n?: number | null;
    time_of_day_entropy_norm?: number | null;
    time_of_day_entropy_weight?: number | null;
    summary?: string[];
  };
};

export type RelatedSensorsUnifiedLimitsUsedV2 = {
  candidate_limit_used: number;
  max_results_used: number;
  max_sensors_used: number;
};

export type AlarmRuleBacktestTransitionV1 = {
  timestamp: string;
  transition: "fired" | "resolved";
  observed_value?: number | null;
};

export type AlarmRuleBacktestIntervalV1 = {
  start_ts: string;
  end_ts: string;
  duration_seconds: number;
};

export type AlarmRuleBacktestTargetSummaryV1 = {
  fired_count: number;
  resolved_count: number;
  interval_count: number;
  time_firing_seconds: number;
  min_interval_seconds?: number | null;
  max_interval_seconds?: number | null;
  median_interval_seconds?: number | null;
  p95_interval_seconds?: number | null;
  mean_interval_seconds?: number | null;
};

export type AlarmRuleBacktestTargetResultV1 = {
  target_key: string;
  sensor_ids: string[];
  transitions: AlarmRuleBacktestTransitionV1[];
  firing_intervals: AlarmRuleBacktestIntervalV1[];
  summary: AlarmRuleBacktestTargetSummaryV1;
};

export type AlarmRuleBacktestSummaryV1 = {
  target_count: number;
  total_fired: number;
  total_resolved: number;
  total_time_firing_seconds: number;
};

export type AlarmRuleBacktestJobParamsNormalizedV1 = {
  target_selector: unknown;
  condition_ast: unknown;
  timing: unknown;
  start: string;
  end: string;
  interval_seconds: number;
  bucket_aggregation_mode: BucketAggregationModeV1;
  eval_step_seconds: number;
};

export type AlarmRuleBacktestResultV1 = {
  job_type: "alarm_rule_backtest_v1";
  computed_through_ts: string;
  params: AlarmRuleBacktestJobParamsNormalizedV1;
  summary: AlarmRuleBacktestSummaryV1;
  targets: AlarmRuleBacktestTargetResultV1[];
  timings_ms: Record<string, number>;
};

export type UnifiedCandidateSkipReasonV2 = "no_lake_history";

export type RelatedSensorsUnifiedSkippedCandidateV2 = {
  sensor_id: string;
  reason: UnifiedCandidateSkipReasonV2;
};

export type SystemWideCooccurrenceBucketV1 = {
  ts: number;
  group_size: number;
  severity_sum: number;
};

export type RelatedSensorsUnifiedResultV2 = {
  job_type: string;
  focus_sensor_id: string;
  evidence_source?: UnifiedEvidenceSourceV1 | null;
  computed_through_ts?: string | null;
  interval_seconds?: number | null;
  bucket_count?: number | null;
  params: RelatedSensorsUnifiedJobParamsV2;
  limits_used: RelatedSensorsUnifiedLimitsUsedV2;
  candidates: RelatedSensorsUnifiedCandidateV2[];
  skipped_candidates: RelatedSensorsUnifiedSkippedCandidateV2[];
  system_wide_buckets?: SystemWideCooccurrenceBucketV1[];
  prefiltered_candidate_sensor_ids: string[];
  truncated_candidate_sensor_ids: string[];
  truncated_result_sensor_ids: string[];
  gap_skipped_deltas?: Record<string, number>;
  timings_ms?: Record<string, number>;
  counts?: Record<string, number>;
  versions?: Record<string, string>;
  monitoring?: EventEvidenceMonitoringV1 | null;
  stability?: RelatedSensorsUnifiedStabilityV1 | null;
};

export type PreviewEventOverlayParamsV1 = {
  z_threshold?: number | null;
  min_separation_buckets?: number | null;
  gap_max_buckets?: number | null;
  polarity?: EventPolarityV1 | null;
  max_events?: number | null;
  tolerance_seconds?: number | null;
};

export type TssePreviewRequestV1 = {
  focus_sensor_id: string;
  candidate_sensor_id: string;
  episode_start_ts?: string | null;
  episode_end_ts?: string | null;
  lag_seconds?: number | null;
  max_points?: number | null;
  event_overlay?: PreviewEventOverlayParamsV1 | null;
};

export type TssePreviewSeriesPointV1 = {
  timestamp: string;
  value: number;
  samples: number;
};

export type TssePreviewSeriesV1 = {
  sensor_id: string;
  sensor_name?: string | null;
  unit?: string | null;
  bucket_coverage_pct?: number | null;
  points: TssePreviewSeriesPointV1[];
};

export type PreviewEventOverlaysV1 = {
  focus_event_ts_ms: number[];
  candidate_event_ts_ms: number[];
  matched_focus_event_ts_ms: number[];
  matched_candidate_event_ts_ms: number[];
  tolerance_seconds?: number | null;
};

export type TssePreviewResponseV1 = {
  focus: TssePreviewSeriesV1;
  candidate: TssePreviewSeriesV1;
  candidate_aligned?: TssePreviewSeriesV1 | null;
  selected_episode?: TsseEpisodeV1 | null;
  bucket_seconds: number;
  event_overlays?: PreviewEventOverlaysV1 | null;
};

export type CorrelationValueModeV1 = "levels" | "deltas";
export type CorrelationLagModeV1 = "aligned" | "best_within_max";

export type CorrelationMatrixJobParamsV1 = {
  sensor_ids: string[];
  start: string;
  end: string;
  interval_seconds?: number;
  method?: "pearson" | "spearman";
  min_overlap?: number | null;
  min_significant_n?: number | null;
  significance_alpha?: number | null;
  min_abs_r?: number | null;
  bucket_aggregation_mode?: BucketAggregationModeV1 | null;
  value_mode?: CorrelationValueModeV1 | null;
  lag_mode?: CorrelationLagModeV1 | null;
  max_lag_buckets?: number | null;
  max_buckets?: number | null;
  max_sensors?: number | null;
};

export type CorrelationMatrixSensorV1 = {
  sensor_id: string;
  name?: string | null;
  unit?: string | null;
  node_id?: string | null;
  sensor_type?: string | null;
};

export type CorrelationMatrixCellV1 = {
  r: number | null;
  r_ci_low?: number | null;
  r_ci_high?: number | null;
  p_value?: number | null;
  q_value?: number | null;
  n_eff?: number | null;
  status?: "ok" | "insufficient_overlap" | "not_significant" | "not_computed";
  lag_sec?: number | null;
  n: number;
};

export type CorrelationMatrixResultV1 = {
  job_type: string;
  params: CorrelationMatrixJobParamsV1;
  sensor_ids: string[];
  sensors?: CorrelationMatrixSensorV1[];
  matrix: CorrelationMatrixCellV1[][];
  computed_through_ts?: string | null;
  interval_seconds?: number;
  bucket_count?: number;
  truncated_sensor_ids?: string[];
  timings_ms?: Record<string, number>;
  versions?: Record<string, string>;
};

export type EventMatchJobParamsV1 = {
  focus_sensor_id: string;
  start: string;
  end: string;
  focus_events?: ExplicitFocusEventV1[];
  interval_seconds?: number | null;
  candidate_sensor_ids?: string[];
  candidate_limit?: number | null;
  max_buckets?: number | null;
  max_events?: number | null;
  z_threshold?: number | null;
  threshold_mode?: EventThresholdModeV1 | null;
  adaptive_threshold?: AdaptiveThresholdConfigV1 | null;
  detector_mode?: EventDetectorModeV1 | null;
  suppression_mode?: EventSuppressionModeV1 | null;
  exclude_boundary_events?: boolean | null;
  sparse_point_events_enabled?: boolean | null;
  min_separation_buckets?: number | null;
  max_lag_buckets?: number | null;
  top_k_lags?: number | null;
  tolerance_buckets?: number | null;
  max_episodes?: number | null;
  episode_gap_buckets?: number | null;
  gap_max_buckets?: number | null;
  polarity?: EventPolarityV1 | null;
  z_cap?: number | null;
  deseason_mode?: DeseasonModeV1 | null;
  periodic_penalty_enabled?: boolean | null;
  filters: TsseCandidateFiltersV1;
};

export type EventMatchLagScoreV1 = {
  lag_sec: number;
  score?: number | null;
  overlap: number;
  n_candidate: number;
};

export type EventMatchCandidateV1 = {
  sensor_id: string;
  rank: number;
  score?: number | null;
  overlap: number;
  n_focus: number;
  n_candidate: number;
  n_focus_up?: number | null;
  n_focus_down?: number | null;
  n_candidate_up?: number | null;
  n_candidate_down?: number | null;
  zero_lag: EventMatchLagScoreV1;
  best_lag?: EventMatchLagScoreV1 | null;
  top_lags?: EventMatchLagScoreV1[];
  direction_label?: DirectionLabelV1 | null;
  sign_agreement?: number | null;
  delta_corr?: number | null;
  direction_n?: number | null;
  time_of_day_entropy_norm?: number | null;
  time_of_day_entropy_weight?: number | null;
  episodes?: TsseEpisodeV1[];
  why_ranked?: TsseWhyRankedV1 | null;
};

export type EventMatchResultV1 = {
  job_type: string;
  focus_sensor_id: string;
  computed_through_ts?: string | null;
  interval_seconds?: number | null;
  bucket_count?: number | null;
  params: EventMatchJobParamsV1;
  candidates: EventMatchCandidateV1[];
  truncated_sensor_ids?: string[];
  gap_skipped_deltas?: Record<string, number>;
  monitoring?: EventEvidenceMonitoringV1 | null;
  timings_ms?: Record<string, number>;
  versions?: Record<string, string>;
};

export type CooccurrenceJobParamsV1 = {
  sensor_ids: string[];
  start: string;
  end: string;
  interval_seconds?: number | null;
  max_buckets?: number | null;
  z_threshold?: number | null;
  threshold_mode?: EventThresholdModeV1 | null;
  adaptive_threshold?: AdaptiveThresholdConfigV1 | null;
  detector_mode?: EventDetectorModeV1 | null;
  suppression_mode?: EventSuppressionModeV1 | null;
  exclude_boundary_events?: boolean | null;
  sparse_point_events_enabled?: boolean | null;
  gap_max_buckets?: number | null;
  min_separation_buckets?: number | null;
  tolerance_buckets?: number | null;
  min_sensors?: number | null;
  max_results?: number | null;
  max_sensors?: number | null;
  max_events?: number | null;
  focus_sensor_id?: string | null;
  polarity?: "both" | "up" | "down" | null;
  z_cap?: number | null;
  deseason_mode?: DeseasonModeV1 | null;
  periodic_penalty_enabled?: boolean | null;
  bucket_preference_mode?: CooccurrenceBucketPreferenceModeV1 | null;
};

export type CooccurrenceBucketV1 = {
  ts: number;
  sensors: Array<{
    sensor_id: string;
    ts: number;
    z: number;
    direction: "up" | "down";
    delta: number;
  }>;
  group_size: number;
  severity_sum: number;
  pair_weight: number;
  idf?: number | null;
  score: number;
};

export type CooccurrenceResultV1 = {
  job_type: string;
  params: CooccurrenceJobParamsV1;
  buckets: CooccurrenceBucketV1[];
  truncated_sensor_ids?: string[];
  gap_skipped_deltas?: Record<string, number>;
  interval_seconds?: number | null;
  bucket_count?: number | null;
  computed_through_ts?: string | null;
  timings_ms?: Record<string, number>;
  counts?: Record<string, number>;
  sensor_stats?: Record<string, { n_events: number; mean_abs_z: number }>;
  versions?: Record<string, string>;
};

export type MatrixProfileJobParamsV1 = {
  sensor_id: string;
  start: string;
  end: string;
  interval_seconds?: number;
  max_points?: number;
  window_points?: number;
  exclusion_zone?: number | null;
  max_windows?: number | null;
  top_k?: number | null;
};

export type MatrixProfileWindowV1 = {
  window_index: number;
  start_ts: string;
  end_ts: string;
  distance: number;
  match_index?: number | null;
  match_start_ts?: string | null;
  match_end_ts?: string | null;
};

export type MatrixProfileResultV1 = {
  job_type: string;
  params: MatrixProfileJobParamsV1;
  sensor_id: string;
  sensor_label?: string | null;
  unit?: string | null;
  timestamps: string[];
  values: number[];
  window_start_ts?: string[];
  profile: number[];
  profile_index: number[];
  window: number;
  exclusion_zone: number;
  step?: number | null;
  effective_interval_seconds?: number | null;
  computed_through_ts?: string | null;
  interval_seconds?: number;
  window_points?: number;
  warnings?: string[];
  motifs?: MatrixProfileWindowV1[];
  anomalies?: MatrixProfileWindowV1[];
  source_points?: number;
  sampled_points?: number;
  timings_ms?: Record<string, number>;
  versions?: Record<string, string>;
};
