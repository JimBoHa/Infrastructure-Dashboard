/**
 * Co-occurrence Strategy
 *
 * Backend: cooccurrence_v1
 * Finds sensors that have anomalous events at the same times as the focus sensor.
 */

import type {
  CooccurrenceJobParamsV1,
  CooccurrenceResultV1,
} from "@/types/analysis";
import type { DemoSensor } from "@/types/dashboard";
import type {
  CooccurrenceControlState,
  SharedControlState,
  WindowRange,
  SensorSourceFilter,
} from "../types/relationshipFinder";
import { generateJobKey } from "../hooks/useAnalysisJob";
import { isDerivedSensor, isPublicProviderSensor } from "@/lib/sensorOrigin";

export const JOB_TYPE = "cooccurrence_v1";

export type CooccurrenceParams = CooccurrenceJobParamsV1;
export type CooccurrenceResult = CooccurrenceResultV1;

/**
 * Determine which source bucket a sensor belongs to
 */
function sensorSourceBucket(sensor: Pick<DemoSensor, "config">): keyof SensorSourceFilter {
  if (isPublicProviderSensor(sensor)) return "remote";
  if (isDerivedSensor(sensor)) return "derived";
  return "local";
}

/**
 * Build job parameters for co-occurrence analysis
 */
export function buildCooccurrenceParams(
  shared: SharedControlState,
  controls: CooccurrenceControlState,
  window: WindowRange,
  intervalSeconds: number,
  sensorIds: string[],
): CooccurrenceParams {
  return {
    sensor_ids: sensorIds,
    start: window.startIso,
    end: window.endIso,
    interval_seconds: intervalSeconds,
    z_threshold: controls.zThreshold,
    tolerance_buckets: controls.toleranceBuckets,
    min_separation_buckets: controls.minSeparationBuckets,
    min_sensors: controls.minSensors,
    max_results: controls.maxResults,
    focus_sensor_id: shared.focusSensorId,
    polarity: controls.polarity,
  };
}

/**
 * Generate a stable job key for deduplication
 */
export function generateCooccurrenceJobKey(
  params: CooccurrenceParams,
): string {
  return generateJobKey({
    v: 1,
    strategy: "cooccurrence",
    sensorIds: params.sensor_ids.slice(0, 100).sort(),
    sensorCount: params.sensor_ids.length,
    start: params.start,
    end: params.end,
    interval: params.interval_seconds,
    zThreshold: params.z_threshold,
    toleranceBuckets: params.tolerance_buckets,
    minSeparationBuckets: params.min_separation_buckets,
    minSensors: params.min_sensors,
    maxResults: params.max_results,
    focusSensorId: params.focus_sensor_id,
    polarity: params.polarity,
  });
}

/**
 * Validate that we can run co-occurrence analysis
 */
export function validateCooccurrenceParams(
  shared: SharedControlState,
  window: WindowRange | null,
  sensorIds: string[],
): { valid: boolean; error?: string } {
  if (!shared.focusSensorId) {
    return { valid: false, error: "Select a focus sensor to run co-occurrence analysis." };
  }

  if (!window) {
    return { valid: false, error: "Select a valid time range to run analysis." };
  }

  if (sensorIds.length < 2) {
    return { valid: false, error: "Select at least two sensors to detect co-occurrences." };
  }

  return { valid: true };
}

/**
 * Get sensor IDs for co-occurrence analysis
 */
export function getCooccurrenceSensorIds(
  sensors: DemoSensor[],
  focusSensorId: string,
  shared: SharedControlState,
  selectedSensorIds: string[],
  sourceFilter: SensorSourceFilter,
  mode: "selection" | "focus_scan",
): string[] {
  if (mode === "selection") {
    // Use selected sensors
    return selectedSensorIds;
  }

  // Focus scan mode - filter all sensors
  const focusSensor = sensors.find((s) => s.sensor_id === focusSensorId);
  if (!focusSensor) return [focusSensorId];

  const filtered = sensors.filter((sensor) => {
    if (sensor.sensor_id === focusSensorId) return true; // Always include focus
    if (shared.scope === "same_node" && sensor.node_id !== focusSensor.node_id) return false;
    if (shared.sameUnitOnly && focusSensor.unit && sensor.unit !== focusSensor.unit) return false;
    if (shared.sameTypeOnly && focusSensor.type && sensor.type !== focusSensor.type) return false;
    const bucket = sensorSourceBucket(sensor);
    if (!sourceFilter[bucket]) return false;
    return true;
  });

  return filtered.map((s) => s.sensor_id);
}

/**
 * Progress phase labels for co-occurrence analysis
 */
export const COOCCURRENCE_PROGRESS_LABELS: Record<string, string> = {
  load_series: "Loading time series…",
  detect_events: "Detecting events…",
  score_buckets: "Scoring co-occurrences…",
};

/**
 * Get human-readable progress message
 */
export function getCooccurrenceProgressMessage(
  phase: string,
  completed: number,
  total: number | null | undefined,
): string {
  const label = COOCCURRENCE_PROGRESS_LABELS[phase] ?? "Processing…";
  if (total && total > 0) {
    return `${label} (${completed} of ${total})`;
  }
  if (completed > 0 && phase === "detect_events") {
    return `${label} (${completed} events found)`;
  }
  return label;
}

/**
 * Get severity tone based on z-score
 */
export function getCooccurrenceSeverityTone(
  z: number,
): "danger" | "warning" | "accent" | "info" | "muted" {
  const absZ = Math.abs(z);
  if (absZ >= 5) return "danger";
  if (absZ >= 4) return "warning";
  if (absZ >= 3) return "accent";
  if (absZ >= 2) return "info";
  return "muted";
}
