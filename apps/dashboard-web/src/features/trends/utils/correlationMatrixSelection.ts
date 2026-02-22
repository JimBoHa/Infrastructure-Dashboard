import type { NormalizedCandidate } from "../types/relationshipFinder";

const DEFAULT_MAX_MATRIX_SENSORS = 25;

function normalizeCutoff(value: number): number {
  if (!Number.isFinite(value)) return 0.35;
  return Math.min(1, Math.max(0, value));
}

export function buildRelatedMatrixSensorIds(params: {
  focusSensorId: string | null;
  candidates: NormalizedCandidate[];
  scoreCutoff: number;
  maxSensors?: number;
}): string[] {
  const { focusSensorId, candidates } = params;
  if (!focusSensorId) return [];

  const cutoff = normalizeCutoff(params.scoreCutoff);
  const maxSensors = Math.max(
    1,
    Math.trunc(params.maxSensors ?? DEFAULT_MAX_MATRIX_SENSORS),
  );

  const filtered = candidates
    .filter((candidate) => candidate.sensor_id !== focusSensorId)
    .filter((candidate) => Number.isFinite(candidate.score) && candidate.score >= cutoff)
    .slice(0, maxSensors)
    .map((candidate) => candidate.sensor_id);

  return [focusSensorId, ...filtered];
}

