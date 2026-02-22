/**
 * Relationship Finder Panel - Unified Types
 *
 * These types normalize results from 4 different analysis strategies
 * (Similarity, Correlation, Events, Co-occurrence) into a unified interface.
 */

import type { NodePillTone } from "@/features/nodes/components/NodePill";
import type {
  AnalysisJobProgress,
  AnalysisJobStatus,
  CooccurrenceBucketPreferenceModeV1,
  CooccurrenceScoreModeV1,
  CorrelationMatrixCellV1,
  DeseasonModeV1,
  EventMatchCandidateV1,
  RelatedSensorsUnifiedCandidateV2,
  RelatedSensorCandidateV1,
  TsseEpisodeV1,
} from "@/types/analysis";

// ─────────────────────────────────────────────────────────────────────────────
// Core Strategy Types
// ─────────────────────────────────────────────────────────────────────────────

export type Strategy =
  | "unified"
  | "similarity"
  | "correlation"
  | "events"
  | "cooccurrence";

export const STRATEGY_LABELS: Record<Strategy, string> = {
  unified: "Unified",
  similarity: "Similarity",
  correlation: "Correlation",
  events: "Events",
  cooccurrence: "Co-occurrence",
};

export const STRATEGY_DESCRIPTIONS: Record<Strategy, string> = {
  unified: "Auto-ranked related sensors blending event alignment and co-occurrence evidence",
  similarity: "Find sensors with similar time-series patterns (TSSE)",
  correlation: "Find sensors correlated with the focus sensor",
  events: "Find sensors whose spikes align with the focus sensor",
  cooccurrence: "Find sensors that co-occur with the focus sensor in anomaly buckets",
};

export const STRATEGY_JOB_TYPES: Record<Strategy, string> = {
  unified: "related_sensors_unified_v2",
  similarity: "related_sensors_v1",
  correlation: "correlation_matrix_v1",
  events: "event_match_v1",
  cooccurrence: "cooccurrence_v1",
};

// ─────────────────────────────────────────────────────────────────────────────
// Badge Types
// ─────────────────────────────────────────────────────────────────────────────

export type BadgeType =
  | "score"
  | "coverage"
  | "lag"
  | "overlap"
  | "correlation"
  | "p_value"
  | "q_value"
  | "n_eff"
  | "episodes"
  | "status";

export type Badge = {
  type: BadgeType;
  label: string;
  value: string;
  tone: NodePillTone;
  tooltip?: string;
};

// ─────────────────────────────────────────────────────────────────────────────
// Normalized Candidate (unified result format)
// ─────────────────────────────────────────────────────────────────────────────

export type NormalizedCandidate = {
  sensor_id: string;
  label: string;
  node_name: string | null;
  node_id: string | null;
  sensor_type: string | null;
  unit: string | null;
  rank: number;
  score: number;
  score_label: string;
  badges: Badge[];
  strategy: Strategy;
  status: CandidateStatus;
  raw: RawCandidate;
};

export type CandidateStatus =
  | "ok"
  | "not_significant"
  | "insufficient_overlap"
  | "not_computed";

// Raw candidate type preserves original backend data for strategy-specific preview
export type RawCandidate =
  | { type: "unified"; data: RelatedSensorsUnifiedCandidateV2 }
  | { type: "similarity"; data: RelatedSensorCandidateV1 }
  | { type: "correlation"; data: CorrelationMatrixCellV1 & { sensor_id: string } }
  | { type: "events"; data: EventMatchCandidateV1 }
  | { type: "cooccurrence"; data: CooccurrenceAggregatedSensor };

// ─────────────────────────────────────────────────────────────────────────────
// Co-occurrence Aggregated Sensor (computed from buckets)
// ─────────────────────────────────────────────────────────────────────────────

export type CooccurrenceAggregatedSensor = {
  sensor_id: string;
  score: number;
  co_occurrence_count: number;
  max_bucket_z: number;
  avg_bucket_z: number;
  total_severity_contribution: number;
  top_bucket_timestamps: number[];
};

// ─────────────────────────────────────────────────────────────────────────────
// Preview Model Types
// ─────────────────────────────────────────────────────────────────────────────

export type PreviewModel = {
  focus_sensor_id: string;
  focus_label: string;
  candidate_sensor_id: string;
  candidate_label: string;
  strategy: Strategy;
  computed_through_ts: string | null;
  content: PreviewContent;
};

export type PreviewContent =
  | SimilarityPreviewContent
  | CorrelationPreviewContent
  | EventsPreviewContent
  | CooccurrencePreviewContent;

export type SimilarityPreviewContent = {
  type: "similarity";
  episodes: TsseEpisodeV1[];
  selected_episode_index: number;
  why_ranked: WhyRankedSummary;
};

export type CorrelationPreviewContent = {
  type: "correlation";
  r: number | null;
  r_ci_low: number | null;
  r_ci_high: number | null;
  p_value: number | null;
  q_value: number | null;
  n: number;
  n_eff: number | null;
  status: CandidateStatus;
  method: "pearson" | "spearman";
};

export type EventsPreviewContent = {
  type: "events";
  episodes: TsseEpisodeV1[];
  selected_episode_index: number;
  overlap: number;
  n_focus: number;
  n_candidate: number;
  best_lag_sec: number | null;
};

export type CooccurrencePreviewContent = {
  type: "cooccurrence";
  top_bucket_timestamps: number[];
  co_occurrence_count: number;
  max_bucket_z: number;
  total_severity_contribution: number;
};

export type WhyRankedSummary = {
  episode_count: number;
  best_window_sec: number | null;
  best_lag_sec: number | null;
  best_lag_r_ci_low: number | null;
  best_lag_r_ci_high: number | null;
  coverage_pct: number | null;
  p_raw: number | null;
  p_lag: number | null;
  q_value: number | null;
  n_eff: number | null;
  m_lag: number | null;
  bonuses: string[];
  penalties: string[];
};

// ─────────────────────────────────────────────────────────────────────────────
// Strategy Controls Types
// ─────────────────────────────────────────────────────────────────────────────

export type Scope = "same_node" | "all_nodes";

export type SensorSourceFilter = {
  local: boolean;
  remote: boolean;
  derived: boolean;
};

export type SharedControlState = {
  focusSensorId: string | null;
  scope: Scope;
  sameUnitOnly: boolean;
  sameTypeOnly: boolean;
  candidateSource: "visible_in_trends" | "all_sensors_in_scope";
  evaluateAllEligible: boolean;
  includeDerivedFromFocus: boolean;
  excludeSystemWideBuckets: boolean;
  includeProviderSensors: boolean;
};

export type RelationshipFinderMode = "simple" | "advanced";

export type UnifiedControlState = {
  includeLowConfidence: boolean;
  stabilityEnabled: boolean;
  candidateLimit: number;
  maxResults: number;
  matrixScoreCutoff: number;
  eventsWeight: number;
  cooccurrenceWeight: number;
  cooccurrenceScoreMode: CooccurrenceScoreModeV1;
  cooccurrenceBucketPreferenceMode: CooccurrenceBucketPreferenceModeV1;
  includeDeltaCorrSignal: boolean;
  deltaCorrWeight: number;
  polarity: "both" | "up" | "down";
  zThreshold: number;
  zCap: number;
  minSeparationBuckets: number;
  gapMaxBuckets: number;
  maxLagBuckets: number;
  maxEvents: number;
  maxEpisodes: number;
  episodeGapBuckets: number;
  toleranceBuckets: number;
  minSensors: number;
  deseasonMode: DeseasonModeV1;
  periodicPenaltyEnabled: boolean;
};

export type SimilarityControlState = {
  maxCandidates: number;
  maxLagBuckets: number;
  minSignificantN: number;
  significanceAlpha: number;
  bucketAggregationMode: "auto" | "avg" | "last" | "sum" | "min" | "max";
};

export type CorrelationControlState = {
  method: "pearson" | "spearman";
  minOverlap: number;
  minSignificantN: number;
  significanceAlpha: number;
  minAbsR: number;
  bucketAggregationMode: "auto" | "avg" | "last" | "sum" | "min" | "max";
};

export type EventsControlState = {
  polarity: "both" | "up" | "down";
  zThreshold: number;
  minSeparationBuckets: number;
  maxLagBuckets: number;
  maxEvents: number;
  maxEpisodes: number;
  episodeGapBuckets: number;
  candidateLimit: number;
  sourceFilter: SensorSourceFilter;
};

export type CooccurrenceControlState = {
  polarity: "both" | "up" | "down";
  zThreshold: number;
  toleranceBuckets: number;
  minSeparationBuckets: number;
  minSensors: number;
  maxResults: number;
  sourceFilter: SensorSourceFilter;
};

export type StrategyControlState =
  | { strategy: "unified"; controls: UnifiedControlState }
  | { strategy: "similarity"; controls: SimilarityControlState }
  | { strategy: "correlation"; controls: CorrelationControlState }
  | { strategy: "events"; controls: EventsControlState }
  | { strategy: "cooccurrence"; controls: CooccurrenceControlState };

// ─────────────────────────────────────────────────────────────────────────────
// Job State Types
// ─────────────────────────────────────────────────────────────────────────────

export type AnalysisJobState = {
  jobId: string | null;
  status: AnalysisJobStatus | null;
  progress: AnalysisJobProgress | null;
  error: string | null;
  requestedAt: Date | null;
  completedAt: Date | null;
  computedThroughTs: string | null;
};

export type WindowRange = {
  startIso: string;
  endIso: string;
};

// ─────────────────────────────────────────────────────────────────────────────
// Result State (per-strategy cache)
// ─────────────────────────────────────────────────────────────────────────────

export type StrategyResultCache = {
  unified: NormalizedCandidate[] | null;
  similarity: NormalizedCandidate[] | null;
  correlation: NormalizedCandidate[] | null;
  events: NormalizedCandidate[] | null;
  cooccurrence: NormalizedCandidate[] | null;
};

export type StrategyJobState = {
  unified: AnalysisJobState;
  similarity: AnalysisJobState;
  correlation: AnalysisJobState;
  events: AnalysisJobState;
  cooccurrence: AnalysisJobState;
};

// ─────────────────────────────────────────────────────────────────────────────
// Props Types
// ─────────────────────────────────────────────────────────────────────────────

export type SelectedBadge = {
  sensorId: string;
  label: string;
  color: string;
  hasData: boolean;
};

// ─────────────────────────────────────────────────────────────────────────────
// Default Values
// ─────────────────────────────────────────────────────────────────────────────

export const DEFAULT_SHARED_CONTROLS: SharedControlState = {
  focusSensorId: null,
  scope: "all_nodes",
  sameUnitOnly: false,
  sameTypeOnly: false,
  candidateSource: "all_sensors_in_scope",
  evaluateAllEligible: true,
  includeDerivedFromFocus: false,
  excludeSystemWideBuckets: false,
  includeProviderSensors: false,
};

export const DEFAULT_UNIFIED_CONTROLS: UnifiedControlState = {
  includeLowConfidence: false,
  stabilityEnabled: false,
  candidateLimit: 200,
  maxResults: 60,
  matrixScoreCutoff: 0.35,
  eventsWeight: 0.6,
  cooccurrenceWeight: 0.4,
  cooccurrenceScoreMode: "avg_product",
  cooccurrenceBucketPreferenceMode: "prefer_specific_matches",
  includeDeltaCorrSignal: false,
  deltaCorrWeight: 0.2,
  polarity: "both",
  zThreshold: 3,
  zCap: 15,
  minSeparationBuckets: 2,
  gapMaxBuckets: 5,
  maxLagBuckets: 12,
  maxEvents: 2000,
  maxEpisodes: 24,
  episodeGapBuckets: 6,
  toleranceBuckets: 2,
  minSensors: 2,
  deseasonMode: "none",
  periodicPenaltyEnabled: true,
};

export const DEFAULT_SIMILARITY_CONTROLS: SimilarityControlState = {
  maxCandidates: 100,
  maxLagBuckets: 24,
  minSignificantN: 10,
  significanceAlpha: 0.05,
  bucketAggregationMode: "auto",
};

export const DEFAULT_CORRELATION_CONTROLS: CorrelationControlState = {
  method: "pearson",
  minOverlap: 10,
  minSignificantN: 10,
  significanceAlpha: 0.05,
  minAbsR: 0.2,
  bucketAggregationMode: "auto",
};

export const DEFAULT_EVENTS_CONTROLS: EventsControlState = {
  polarity: "both",
  zThreshold: 3,
  minSeparationBuckets: 2,
  maxLagBuckets: 12,
  maxEvents: 2000,
  maxEpisodes: 24,
  episodeGapBuckets: 6,
  candidateLimit: 50,
  sourceFilter: { local: true, remote: true, derived: true },
};

export const DEFAULT_COOCCURRENCE_CONTROLS: CooccurrenceControlState = {
  polarity: "both",
  zThreshold: 4,
  toleranceBuckets: 0,
  minSeparationBuckets: 2,
  minSensors: 2,
  maxResults: 24,
  sourceFilter: { local: true, remote: true, derived: true },
};

export const DEFAULT_SOURCE_FILTER: SensorSourceFilter = {
  local: true,
  remote: true,
  derived: true,
};

export function createInitialJobState(): AnalysisJobState {
  return {
    jobId: null,
    status: null,
    progress: null,
    error: null,
    requestedAt: null,
    completedAt: null,
    computedThroughTs: null,
  };
}

export function createInitialStrategyJobState(): StrategyJobState {
  return {
    unified: createInitialJobState(),
    similarity: createInitialJobState(),
    correlation: createInitialJobState(),
    events: createInitialJobState(),
    cooccurrence: createInitialJobState(),
  };
}

export function createInitialResultCache(): StrategyResultCache {
  return {
    unified: null,
    similarity: null,
    correlation: null,
    events: null,
    cooccurrence: null,
  };
}
