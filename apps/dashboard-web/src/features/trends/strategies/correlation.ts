/**
 * Correlation Strategy
 *
 * Backend: correlation_matrix_v1
 * Computes pairwise correlation coefficients between sensors.
 */

import type {
  CorrelationMatrixJobParamsV1,
  CorrelationMatrixResultV1,
} from "@/types/analysis";
import type {
  CorrelationControlState,
  SharedControlState,
  WindowRange,
} from "../types/relationshipFinder";
import { generateJobKey } from "../hooks/useAnalysisJob";

export const JOB_TYPE = "correlation_matrix_v1";

export type CorrelationParams = CorrelationMatrixJobParamsV1;
export type CorrelationResult = CorrelationMatrixResultV1;

/**
 * Build job parameters for correlation analysis
 */
export function buildCorrelationParams(
  shared: SharedControlState,
  controls: CorrelationControlState,
  window: WindowRange,
  intervalSeconds: number,
  sensorIds: string[],
): CorrelationParams {
  return {
    sensor_ids: sensorIds,
    start: window.startIso,
    end: window.endIso,
    interval_seconds: intervalSeconds,
    method: controls.method,
    min_overlap: controls.minOverlap,
    min_significant_n: controls.minSignificantN,
    significance_alpha: controls.significanceAlpha,
    min_abs_r: controls.minAbsR,
    bucket_aggregation_mode: controls.bucketAggregationMode,
  };
}

/**
 * Generate a stable job key for deduplication
 */
export function generateCorrelationJobKey(
  params: CorrelationParams,
): string {
  return generateJobKey({
    v: 2,
    strategy: "correlation",
    sensorIds: params.sensor_ids.slice().sort(),
    start: params.start,
    end: params.end,
    interval: params.interval_seconds,
    method: params.method,
    minOverlap: params.min_overlap,
    minSignificantN: params.min_significant_n,
    significanceAlpha: params.significance_alpha,
    minAbsR: params.min_abs_r,
    bucketAggregationMode: params.bucket_aggregation_mode,
    valueMode: params.value_mode,
    lagMode: params.lag_mode,
    maxLagBuckets: params.max_lag_buckets,
  });
}

/**
 * Validate that we can run correlation analysis
 */
export function validateCorrelationParams(
  shared: SharedControlState,
  window: WindowRange | null,
  sensorIds: string[],
): { valid: boolean; error?: string } {
  if (!shared.focusSensorId) {
    return { valid: false, error: "Select a focus sensor to run correlation analysis." };
  }

  if (!window) {
    return { valid: false, error: "Select a valid time range to run analysis." };
  }

  if (sensorIds.length < 2) {
    return { valid: false, error: "Select at least two sensors to compute correlations." };
  }

  return { valid: true };
}

/**
 * Build sensor IDs list for correlation
 * Includes focus sensor plus selected sensors, or filtered candidates
 */
export function buildCorrelationSensorIds(
  focusSensorId: string,
  selectedSensorIds: string[],
): string[] {
  // Always include focus sensor first
  const ids = [focusSensorId];

  // Add other selected sensors
  for (const id of selectedSensorIds) {
    if (id !== focusSensorId && !ids.includes(id)) {
      ids.push(id);
    }
  }

  return ids;
}

/**
 * Progress phase labels for correlation analysis
 */
export const CORRELATION_PROGRESS_LABELS: Record<string, string> = {
  load_series: "Loading time series…",
  correlate: "Computing correlations…",
};

/**
 * Get human-readable progress message
 */
export function getCorrelationProgressMessage(
  phase: string,
  completed: number,
  total: number | null | undefined,
): string {
  const label = CORRELATION_PROGRESS_LABELS[phase] ?? "Processing…";
  if (total && total > 0) {
    return `${label} (${completed} of ${total} pairs)`;
  }
  return label;
}

/**
 * Get correlation tone based on r value
 */
export function getCorrelationTone(r: number | null): "success" | "danger" | "neutral" | "muted" {
  if (r == null || !Number.isFinite(r)) return "muted";
  if (Math.abs(r) >= 0.7) return r > 0 ? "success" : "danger";
  if (Math.abs(r) >= 0.4) return r > 0 ? "success" : "danger";
  return "neutral";
}

/**
 * Get significance indicator
 */
export function getSignificanceIndicator(
  pValue: number | null | undefined,
  alpha: number = 0.05,
): { significant: boolean; stars: string } {
  if (pValue == null || !Number.isFinite(pValue)) {
    return { significant: false, stars: "" };
  }

  if (pValue < 0.001) return { significant: true, stars: "***" };
  if (pValue < 0.01) return { significant: true, stars: "**" };
  if (pValue < alpha) return { significant: true, stars: "*" };
  return { significant: false, stars: "" };
}
