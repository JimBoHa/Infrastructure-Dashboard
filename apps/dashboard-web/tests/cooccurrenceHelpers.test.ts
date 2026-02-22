import { describe, expect, it } from "vitest";
import { computeCooccurrenceBuckets } from "@/features/trends/utils/cooccurrence";
import type { TrendSeriesEntry } from "@/types/dashboard";

function makeSeries(sensorId: string, values: number[]): TrendSeriesEntry {
  const start = new Date("2026-01-20T00:00:00Z").getTime();
  return {
    sensor_id: sensorId,
    label: sensorId,
    unit: "",
    points: values.map((value, idx) => ({
      timestamp: new Date(start + idx * 60_000),
      value,
    })),
  };
}

describe("computeCooccurrenceBuckets", () => {
  it("detects co-occurring events across multiple sensors at the same bucket", () => {
    const a = makeSeries("a", [0, 1, 0, 2, 0, 10, 9, 11, 10, 9, 10]);
    const b = makeSeries("b", [0, 1, 0, 2, 0, 10, 9, 11, 10, 9, 10]);

    const result = computeCooccurrenceBuckets({
      series: [a, b],
      intervalSeconds: 60,
      zThreshold: 4,
      minSeparationBuckets: 0,
      polarity: "both",
      minSensors: 2,
      toleranceBuckets: 0,
      maxResults: 8,
    });

    expect(result.buckets.length).toBeGreaterThan(0);
    const first = result.buckets[0];
    expect(first?.groupSize).toBe(2);
    expect(first?.sensors.map((s) => s.sensorId).sort()).toEqual(["a", "b"]);
  });

  it("ranks larger co-occurring groups above smaller ones", () => {
    // Two events in A/B (idx 5 and idx 9), but only idx 9 is shared with C.
    const a = makeSeries("a", [0, 1, 0, 2, 0, 10, 9, 11, 10, 20, 19, 21, 20]);
    const b = makeSeries("b", [0, 1, 0, 2, 0, 10, 9, 11, 10, 20, 19, 21, 20]);
    const c = makeSeries("c", [0, 1, 0, 2, 0, 2, 1, 3, 2, 20, 19, 21, 20]);

    const result = computeCooccurrenceBuckets({
      series: [a, b, c],
      intervalSeconds: 60,
      zThreshold: 4,
      minSeparationBuckets: 0,
      polarity: "both",
      minSensors: 2,
      toleranceBuckets: 0,
      maxResults: 8,
    });

    expect(result.buckets.length).toBeGreaterThan(0);
    const top = result.buckets[0];
    expect(top?.groupSize).toBe(3);
    expect(new Set(top?.sensors.map((s) => s.sensorId))).toEqual(new Set(["a", "b", "c"]));
  });
});

