import { describe, expect, it } from "vitest";
import { buildRelatedMatrixSensorIds } from "@/features/trends/utils/correlationMatrixSelection";
import type { NormalizedCandidate } from "@/features/trends/types/relationshipFinder";

function candidate(sensorId: string, score: number, rank: number): NormalizedCandidate {
  return {
    sensor_id: sensorId,
    label: sensorId,
    node_name: null,
    node_id: null,
    sensor_type: null,
    unit: null,
    rank,
    score,
    score_label: String(score),
    badges: [],
    strategy: "unified",
    status: "ok",
    raw: {
      type: "unified",
      data: {
        sensor_id: sensorId,
        rank,
        blended_score: score,
        confidence_tier: "high",
        evidence: {},
      },
    },
  };
}

describe("buildRelatedMatrixSensorIds", () => {
  it("returns empty when focus is missing", () => {
    expect(
      buildRelatedMatrixSensorIds({
        focusSensorId: null,
        candidates: [candidate("a", 0.9, 1)],
        scoreCutoff: 0.35,
      }),
    ).toEqual([]);
  });

  it("includes focus + candidates at or above cutoff", () => {
    const ids = buildRelatedMatrixSensorIds({
      focusSensorId: "focus",
      candidates: [
        candidate("a", 0.7, 1),
        candidate("b", 0.35, 2),
        candidate("c", 0.349, 3),
      ],
      scoreCutoff: 0.35,
    });
    expect(ids).toEqual(["focus", "a", "b"]);
  });

  it("caps to maxSensors after cutoff filtering", () => {
    const many = Array.from({ length: 40 }, (_, idx) =>
      candidate(`sensor-${idx}`, 0.8, idx + 1),
    );
    const ids = buildRelatedMatrixSensorIds({
      focusSensorId: "focus",
      candidates: many,
      scoreCutoff: 0.35,
      maxSensors: 25,
    });
    expect(ids.length).toBe(26);
    expect(ids[0]).toBe("focus");
    expect(ids.at(-1)).toBe("sensor-24");
  });
});

