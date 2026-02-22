import type { RelatedSensorsUnifiedResultV2 } from "@/types/analysis";

export const PROVIDER_NO_HISTORY_LABEL =
  "Not available for relationship analysis (no stored history).";

function normalizeSensorId(sensorId: string): string {
  return sensorId.trim();
}

function containsId(list: string[] | undefined | null, target: string): boolean {
  if (!list || list.length === 0) return false;
  return list.includes(target);
}

export function diagnoseUnifiedCandidateAbsence(params: {
  focusSensorId: string | null;
  sensorId: string;
  eligibleSensorIds: string[];
  result: RelatedSensorsUnifiedResultV2;
}): string {
  const sensorId = normalizeSensorId(params.sensorId);
  if (!sensorId) return "Select a sensor id to explain its outcome.";

  if (params.focusSensorId && sensorId === params.focusSensorId) {
    return "Not eligible: this sensor is the focus sensor for the run.";
  }

  if (!params.eligibleSensorIds.includes(sensorId)) {
    return "Not eligible: this sensor did not match the scope/unit/type/source filters used for the run.";
  }

  const requestedCandidateIds = params.result.params.candidate_sensor_ids ?? [];
  const requestedPinnedIds = params.result.params.pinned_sensor_ids ?? [];
  const candidateSource = params.result.params.candidate_source;
  const isBackendCandidateQuery =
    candidateSource === "all_sensors_in_scope" ||
    (!candidateSource && requestedCandidateIds.length === 0);
  if (!isBackendCandidateQuery) {
    const submittedIds = new Set([...requestedCandidateIds, ...requestedPinnedIds]);
    if (submittedIds.size > 0 && !submittedIds.has(sensorId)) {
      return "Eligible but not evaluated: this sensor was not included in the submitted candidate/pinned list for the run (the eligible pool may have changed before/after the request).";
    }
  }

  const skipped = params.result.skipped_candidates.find((entry) => entry.sensor_id === sensorId);
  if (skipped?.reason === "no_lake_history") {
    return PROVIDER_NO_HISTORY_LABEL;
  }

  if (containsId(params.result.prefiltered_candidate_sensor_ids, sensorId)) {
    return "Eligible but not evaluated: insufficient bucket coverage in this time window (minimum history/continuity prefilter).";
  }

  if (containsId(params.result.truncated_candidate_sensor_ids, sensorId)) {
    if (containsId(requestedPinnedIds, sensorId)) {
      return `Pinned but not evaluated: truncated because the pinned set exceeded the hard cap (limit used ${params.result.limits_used.candidate_limit_used}).`;
    }
    const requestedLimitValue = params.result.params.candidate_limit;
    const evaluated = params.result.counts?.evaluated_count ?? 0;
    const eligible = params.result.counts?.eligible_count ?? 0;
    const capped =
      requestedLimitValue != null && requestedLimitValue !== params.result.limits_used.candidate_limit_used
        ? ` (backend cap: ${requestedLimitValue} → ${params.result.limits_used.candidate_limit_used})`
        : "";
    return `Eligible but not evaluated: truncated by candidate limit (evaluated ${evaluated} of ${eligible}; limit used ${params.result.limits_used.candidate_limit_used}${capped}).`;
  }

  const ranked = params.result.candidates.find((candidate) => candidate.sensor_id === sensorId);
  if (ranked) {
    return `Ranked #${ranked.rank} (evidence: ${ranked.confidence_tier}).`;
  }

  if (containsId(params.result.truncated_result_sensor_ids, sensorId)) {
    return `Evaluated and exceeded the evidence threshold, but was truncated by max results (max: ${params.result.limits_used.max_results_used}).`;
  }

  return "Evaluated but did not exceed the evidence threshold.";
}
