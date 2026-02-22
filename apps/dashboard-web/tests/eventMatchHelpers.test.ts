import { describe, expect, it } from "vitest";
import { detectChangeEvents } from "@/features/trends/utils/eventMatch";
import type { TrendSeriesEntry } from "@/types/dashboard";

function makeSeries(sensorId: string, values: number[]): TrendSeriesEntry {
  const start = new Date("2026-01-20T00:00:00Z").getTime();
  return {
    sensor_id: sensorId,
    points: values.map((value, idx) => ({
      timestamp: new Date(start + idx * 60_000),
      value,
    })),
  };
}

describe("detectChangeEvents", () => {
  it("detects events even when MAD is zero due to quantized deltas", () => {
    // Many flat buckets (0 deltas) + small quantized steps (±1) makes MAD=0 for deltas,
    // but the large jump at the end should still be detected as an event.
    const values = [
      ...Array.from({ length: 30 }, () => 0),
      ...Array.from({ length: 20 }, (_, idx) => (idx % 2 === 0 ? 0 : 1)),
      0,
      20,
    ];
    const series = makeSeries("a", values);

    const events = detectChangeEvents({
      series,
      intervalSeconds: 60,
      zThreshold: 4,
      minSeparationBuckets: 0,
      polarity: "both",
    });

    expect(events.length).toBeGreaterThan(0);
    expect(events.some((evt) => Math.abs(evt.delta) >= 20)).toBe(true);
  });

  it("detects rare step changes when there are only a few non-zero deltas", () => {
    // Only two non-zero deltas: +20 then -20. Robust MAD is degenerate; fallback scale
    // should still allow us to flag the step.
    const values = [
      ...Array.from({ length: 60 }, () => 0),
      20,
      ...Array.from({ length: 60 }, () => 20),
      0,
      ...Array.from({ length: 20 }, () => 0),
    ];
    const series = makeSeries("b", values);

    const events = detectChangeEvents({
      series,
      intervalSeconds: 60,
      zThreshold: 4,
      minSeparationBuckets: 0,
      polarity: "both",
    });

    expect(events.length).toBeGreaterThan(0);
    expect(events.some((evt) => Math.abs(evt.delta) >= 20)).toBe(true);
  });
});

