/**
 * Similarity Strategy (TSSE - Time Series Similarity Engine)
 *
 * Backend: related_sensors_v1
 * Finds sensors with similar time-series patterns using embedding-based search.
 */

import type {
  RelatedSensorsJobParamsV1,
  RelatedSensorsResultV1,
  TsseCandidateFiltersV1,
} from "@/types/analysis";
import type { DemoSensor } from "@/types/dashboard";
import type {
  SimilarityControlState,
  SharedControlState,
  WindowRange,
} from "../types/relationshipFinder";
import { generateJobKey } from "../hooks/useAnalysisJob";

export const JOB_TYPE = "related_sensors_v1";

export type SimilarityParams = RelatedSensorsJobParamsV1;
export type SimilarityResult = RelatedSensorsResultV1;

/**
 * Build job parameters for similarity analysis
 */
export function buildSimilarityParams(
  shared: SharedControlState,
  controls: SimilarityControlState,
  window: WindowRange,
  intervalSeconds: number,
): SimilarityParams {
  const filters: TsseCandidateFiltersV1 = {
    same_node_only: shared.scope === "same_node",
    same_unit_only: shared.sameUnitOnly,
    same_type_only: shared.sameTypeOnly,
    is_public_provider: shared.includeProviderSensors ? null : false,
    exclude_sensor_ids: [],
  };

  return {
    focus_sensor_id: shared.focusSensorId!,
    start: window.startIso,
    end: window.endIso,
    interval_seconds: intervalSeconds,
    candidate_limit: controls.maxCandidates,
    lag_max_seconds: controls.maxLagBuckets * intervalSeconds,
    min_significant_n: controls.minSignificantN,
    significance_alpha: controls.significanceAlpha,
    min_abs_r: 0.2,
    bucket_aggregation_mode: controls.bucketAggregationMode,
    filters,
  };
}

/**
 * Generate a stable job key for deduplication
 */
export function generateSimilarityJobKey(
  params: SimilarityParams,
): string {
  return generateJobKey({
    v: 1,
    strategy: "similarity",
    focus: params.focus_sensor_id,
    start: params.start,
    end: params.end,
    interval: params.interval_seconds,
    candidateLimit: params.candidate_limit,
    lagMaxSeconds: params.lag_max_seconds,
    minSignificantN: params.min_significant_n,
    significanceAlpha: params.significance_alpha,
    minAbsR: params.min_abs_r,
    bucketAggregationMode: params.bucket_aggregation_mode,
    filters: params.filters,
  });
}

/**
 * Validate that we can run similarity analysis
 */
export function validateSimilarityParams(
  shared: SharedControlState,
  window: WindowRange | null,
  candidateCount: number,
): { valid: boolean; error?: string } {
  if (!shared.focusSensorId) {
    return { valid: false, error: "Select a focus sensor to run similarity analysis." };
  }

  if (!window) {
    return { valid: false, error: "Select a valid time range to run analysis." };
  }

  if (candidateCount === 0) {
    return { valid: false, error: "No candidate sensors match the current filters." };
  }

  return { valid: true };
}

/**
 * Count eligible candidate sensors based on filters
 */
export function countSimilarityCandidates(
  sensors: DemoSensor[],
  focusSensorId: string,
  shared: SharedControlState,
  selectedSensorIds: string[],
): number {
  const focusSensor = sensors.find((s) => s.sensor_id === focusSensorId);
  if (!focusSensor) return 0;

  const selectedSet = new Set(selectedSensorIds);

  return sensors.filter((sensor) => {
    if (sensor.sensor_id === focusSensorId) return false;
    if (selectedSet.has(sensor.sensor_id)) return false;
    if (shared.scope === "same_node" && sensor.node_id !== focusSensor.node_id) return false;
    if (shared.sameUnitOnly && focusSensor.unit && sensor.unit !== focusSensor.unit) return false;
    if (shared.sameTypeOnly && focusSensor.type && sensor.type !== focusSensor.type) return false;
    return true;
  }).length;
}

/**
 * Progress phase labels for similarity analysis
 */
export const SIMILARITY_PROGRESS_LABELS: Record<string, string> = {
  candidates: "Finding candidates…",
  inference: "Running inference…",
  scoring: "Scoring results…",
};

/**
 * Get human-readable progress message
 */
export function getSimilarityProgressMessage(
  phase: string,
  completed: number,
  total: number | null | undefined,
): string {
  const label = SIMILARITY_PROGRESS_LABELS[phase] ?? "Processing…";
  if (total && total > 0) {
    const pct = Math.round((completed / total) * 100);
    return `${label} (${pct}%)`;
  }
  return label;
}
