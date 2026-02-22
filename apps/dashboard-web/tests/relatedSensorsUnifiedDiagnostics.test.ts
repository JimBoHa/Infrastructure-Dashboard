import { describe, expect, it } from "vitest";
import type { RelatedSensorsUnifiedResultV2 } from "@/types/analysis";
import {
  diagnoseUnifiedCandidateAbsence,
  PROVIDER_NO_HISTORY_LABEL,
} from "@/features/trends/utils/relatedSensorsUnifiedDiagnostics";

function baseResult(): RelatedSensorsUnifiedResultV2 {
  return {
    job_type: "related_sensors_unified_v2",
    focus_sensor_id: "focus",
    computed_through_ts: null,
    interval_seconds: 60,
    bucket_count: 10,
    params: {
      focus_sensor_id: "focus",
      start: "2026-02-09T00:00:00Z",
      end: "2026-02-10T00:00:00Z",
      interval_seconds: 60,
      candidate_sensor_ids: ["a", "b"],
      candidate_limit: 2,
      filters: { exclude_sensor_ids: ["focus"] },
    },
    limits_used: {
      candidate_limit_used: 2,
      max_results_used: 2,
      max_sensors_used: 2,
    },
    candidates: [],
    skipped_candidates: [],
    prefiltered_candidate_sensor_ids: [],
    truncated_candidate_sensor_ids: [],
    truncated_result_sensor_ids: [],
    timings_ms: {},
    counts: { candidate_pool: 2, eligible_count: 2, evaluated_count: 2, ranked: 0 },
    versions: {},
  };
}

describe("relatedSensorsUnifiedDiagnostics (ticket 54)", () => {
  it("reports not-eligible sensors deterministically", () => {
    const result = baseResult();
    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "not-in-pool",
        eligibleSensorIds: ["a", "b"],
        result,
      }),
    ).toMatch(/Not eligible/i);
  });

  it("reports eligible-but-not-evaluated when candidate list truncates eligibility", () => {
    const result = baseResult();
    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "c",
        eligibleSensorIds: ["a", "b", "c"],
        result,
      }),
    ).toMatch(/Eligible but not evaluated/i);
  });

  it("reports backend clamping using truncated_candidate_sensor_ids", () => {
    const result = {
      ...baseResult(),
      params: {
        ...baseResult().params,
        candidate_sensor_ids: ["a", "b", "c"],
        candidate_limit: 100,
      },
      limits_used: {
        ...baseResult().limits_used,
        candidate_limit_used: 2,
      },
      truncated_candidate_sensor_ids: ["c"],
      counts: { candidate_pool: 2, eligible_count: 3, evaluated_count: 2, ranked: 0 },
    };

    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "c",
        eligibleSensorIds: ["a", "b", "c"],
        result,
      }),
    ).toMatch(/backend cap/i);
  });

  it("reports coverage prefilter drops deterministically", () => {
    const result = { ...baseResult(), prefiltered_candidate_sensor_ids: ["b"] };
    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "b",
        eligibleSensorIds: ["a", "b"],
        result,
      }),
    ).toMatch(/insufficient bucket coverage/i);
  });

  it("reports evaluated-below-threshold when it was in the submitted candidates but not ranked", () => {
    const result = baseResult();
    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "a",
        eligibleSensorIds: ["a", "b"],
        result,
      }),
    ).toBe("Evaluated but did not exceed the evidence threshold.");
  });

  it("treats pinned candidates as submitted even when they were not in candidate_sensor_ids", () => {
    const result = {
      ...baseResult(),
      params: {
        ...baseResult().params,
        candidate_sensor_ids: ["a"],
        pinned_sensor_ids: ["pinned"],
      },
    };

    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "pinned",
        eligibleSensorIds: ["a", "pinned"],
        result,
      }),
    ).toBe("Evaluated but did not exceed the evidence threshold.");
  });

  it("reports pinned truncation distinctly when a pinned sensor is truncated", () => {
    const result = {
      ...baseResult(),
      params: {
        ...baseResult().params,
        candidate_sensor_ids: ["a"],
        pinned_sensor_ids: ["pinned"],
      },
      truncated_candidate_sensor_ids: ["pinned"],
      limits_used: {
        ...baseResult().limits_used,
        candidate_limit_used: 1000,
      },
    };

    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "pinned",
        eligibleSensorIds: ["a", "pinned"],
        result,
      }),
    ).toMatch(/Pinned but not evaluated/i);
  });
});

describe("relatedSensorsUnifiedDiagnostics (ticket 72)", () => {
  it("labels provider/forecast no-history sensors with the exact not-available copy", () => {
    const result = {
      ...baseResult(),
      skipped_candidates: [{ sensor_id: "provider", reason: "no_lake_history" }],
      params: {
        ...baseResult().params,
        candidate_sensor_ids: ["a", "provider"],
        candidate_limit: 2,
      },
      limits_used: { ...baseResult().limits_used, candidate_limit_used: 2 },
    };

    expect(
      diagnoseUnifiedCandidateAbsence({
        focusSensorId: "focus",
        sensorId: "provider",
        eligibleSensorIds: ["a", "provider"],
        result,
      }),
    ).toBe(PROVIDER_NO_HISTORY_LABEL);
  });
});
