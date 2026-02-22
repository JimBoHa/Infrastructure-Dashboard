/**
 * Events (Spike Matching) Strategy
 *
 * Backend: event_match_v1
 * Finds sensors whose spikes (events) align with the focus sensor.
 */

import type {
  EventMatchJobParamsV1,
  EventMatchResultV1,
  TsseCandidateFiltersV1,
} from "@/types/analysis";
import type { DemoSensor } from "@/types/dashboard";
import type {
  EventsControlState,
  SharedControlState,
  WindowRange,
  SensorSourceFilter,
} from "../types/relationshipFinder";
import { generateJobKey } from "../hooks/useAnalysisJob";
import { isDerivedSensor, isPublicProviderSensor } from "@/lib/sensorOrigin";

export const JOB_TYPE = "event_match_v1";

export type EventsParams = EventMatchJobParamsV1;
export type EventsResult = EventMatchResultV1;

/**
 * Determine which source bucket a sensor belongs to
 */
function sensorSourceBucket(sensor: Pick<DemoSensor, "config">): keyof SensorSourceFilter {
  if (isPublicProviderSensor(sensor)) return "remote";
  if (isDerivedSensor(sensor)) return "derived";
  return "local";
}

/**
 * Build job parameters for events analysis
 */
export function buildEventsParams(
  shared: SharedControlState,
  controls: EventsControlState,
  window: WindowRange,
  intervalSeconds: number,
  candidateSensorIds: string[],
): EventsParams {
  const filters: TsseCandidateFiltersV1 = {
    same_node_only: shared.scope === "same_node",
    same_unit_only: shared.sameUnitOnly,
    same_type_only: shared.sameTypeOnly,
    exclude_sensor_ids: [],
  };

  return {
    focus_sensor_id: shared.focusSensorId!,
    start: window.startIso,
    end: window.endIso,
    interval_seconds: intervalSeconds,
    candidate_sensor_ids: candidateSensorIds,
    candidate_limit: Math.max(5, Math.min(500, Math.floor(controls.candidateLimit))),
    z_threshold: controls.zThreshold,
    min_separation_buckets: controls.minSeparationBuckets,
    max_lag_buckets: controls.maxLagBuckets,
    max_events: controls.maxEvents,
    max_episodes: controls.maxEpisodes,
    episode_gap_buckets: controls.episodeGapBuckets,
    polarity: controls.polarity,
    filters,
  };
}

/**
 * Generate a stable job key for deduplication
 */
export function generateEventsJobKey(
  params: EventsParams,
): string {
  const candidateIds = params.candidate_sensor_ids?.slice(0, 250).sort() ?? [];

  return generateJobKey({
    v: 1,
    strategy: "events",
    focus: params.focus_sensor_id,
    start: params.start,
    end: params.end,
    interval: params.interval_seconds,
    candidateLimit: params.candidate_limit,
    polarity: params.polarity,
    zThreshold: params.z_threshold,
    minSeparationBuckets: params.min_separation_buckets,
    maxLagBuckets: params.max_lag_buckets,
    maxEvents: params.max_events,
    maxEpisodes: params.max_episodes,
    episodeGapBuckets: params.episode_gap_buckets,
    filters: params.filters,
    candidates: candidateIds,
    candidateCount: params.candidate_sensor_ids?.length ?? 0,
  });
}

/**
 * Validate that we can run events analysis
 */
export function validateEventsParams(
  shared: SharedControlState,
  window: WindowRange | null,
  candidateCount: number,
): { valid: boolean; error?: string } {
  if (!shared.focusSensorId) {
    return { valid: false, error: "Select a focus sensor to run event matching." };
  }

  if (!window) {
    return { valid: false, error: "Select a valid time range to run analysis." };
  }

  if (candidateCount === 0) {
    return { valid: false, error: "No candidate sensors to scan. Adjust scope/filters or select more sensors." };
  }

  return { valid: true };
}

/**
 * Get candidate sensor IDs for event matching
 */
export function getEventsCandidateSensorIds(
  sensors: DemoSensor[],
  focusSensorId: string,
  shared: SharedControlState,
  selectedSensorIds: string[],
  sourceFilter: SensorSourceFilter,
  mode: "selection" | "focus_scan",
): string[] {
  const focusSensor = sensors.find((s) => s.sensor_id === focusSensorId);

  if (mode === "selection") {
    // Compare only within selected sensors
    return selectedSensorIds.filter((id) => id !== focusSensorId);
  }

  // Focus scan mode - filter all sensors
  if (!focusSensor) return [];

  return sensors
    .filter((sensor) => {
      if (sensor.sensor_id === focusSensorId) return false;
      if (shared.scope === "same_node" && sensor.node_id !== focusSensor.node_id) return false;
      if (shared.sameUnitOnly && focusSensor.unit && sensor.unit !== focusSensor.unit) return false;
      if (shared.sameTypeOnly && focusSensor.type && sensor.type !== focusSensor.type) return false;
      const bucket = sensorSourceBucket(sensor);
      if (!sourceFilter[bucket]) return false;
      return true;
    })
    .map((sensor) => sensor.sensor_id);
}

/**
 * Progress phase labels for events analysis
 */
export const EVENTS_PROGRESS_LABELS: Record<string, string> = {
  load_series: "Loading time series…",
  detect_events: "Detecting events…",
  match_candidates: "Matching candidates…",
};

/**
 * Get human-readable progress message
 */
export function getEventsProgressMessage(
  phase: string,
  completed: number,
  total: number | null | undefined,
): string {
  const label = EVENTS_PROGRESS_LABELS[phase] ?? "Processing…";
  if (total && total > 0) {
    const pct = Math.round((completed / total) * 100);
    return `${label} (${pct}%)`;
  }
  if (completed > 0 && phase === "detect_events") {
    return `${label} (${completed} events found)`;
  }
  return label;
}
