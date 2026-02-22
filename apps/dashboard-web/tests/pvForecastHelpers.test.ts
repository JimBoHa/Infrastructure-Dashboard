import { describe, expect, it } from "vitest";
import { buildPvWindowedSeries } from "@/features/analytics/utils/pvForecast";
import type { ForecastSeriesPoint } from "@/types/forecast";
import type { TrendSeriesPoint } from "@/types/dashboard";

const TIME_ZONE = "UTC";
const NOW = new Date("2025-01-10T15:23:00Z");

const measuredPoints: TrendSeriesPoint[] = [
  { timestamp: new Date("2025-01-09T23:59:59Z"), value: 1 },
  { timestamp: new Date("2025-01-10T00:00:00Z"), value: 2 },
  { timestamp: new Date("2025-01-10T12:00:00Z"), value: 3 },
  { timestamp: new Date("2025-01-11T00:00:00Z"), value: 4 },
  { timestamp: new Date("2025-01-11T00:00:01Z"), value: 5 },
];

const forecastPoints: ForecastSeriesPoint[] = [
  { timestamp: "2025-01-10T00:00:00Z", value: 10 },
  { timestamp: "2025-01-10T18:00:00Z", value: 12 },
  { timestamp: "2025-01-11T00:00:00Z", value: 13 },
  { timestamp: "2025-01-11T00:00:01Z", value: 14 },
];

describe("buildPvWindowedSeries", () => {
  it("builds a day-aligned 24h window and filters points inclusively", () => {
    const result = buildPvWindowedSeries({
      rangeHours: 24,
      timeZone: TIME_ZONE,
      measuredPoints,
      forecastPoints,
      now: NOW,
    });

    expect(result.start.toISOString()).toBe("2025-01-10T00:00:00.000Z");
    expect(result.end.toISOString()).toBe("2025-01-11T00:00:00.000Z");
    expect(result.startMs).toBe(result.start.getTime());
    expect(result.endMs).toBe(result.end.getTime());
    expect(result.measuredWindowPoints.map((point) => point.value)).toEqual([2, 3, 4]);
    expect(result.forecastWindowPoints.map((point) => point.value)).toEqual([10, 12, 13]);
  });

  it("expands the window to cover prior days for longer ranges", () => {
    const range72 = buildPvWindowedSeries({
      rangeHours: 72,
      timeZone: TIME_ZONE,
      measuredPoints: [],
      forecastPoints: [],
      now: NOW,
    });

    expect(range72.start.toISOString()).toBe("2025-01-08T00:00:00.000Z");
    expect(range72.end.toISOString()).toBe("2025-01-11T00:00:00.000Z");

    const range168 = buildPvWindowedSeries({
      rangeHours: 168,
      timeZone: TIME_ZONE,
      measuredPoints: [],
      forecastPoints: [],
      now: NOW,
    });

    expect(range168.start.toISOString()).toBe("2025-01-04T00:00:00.000Z");
    expect(range168.end.toISOString()).toBe("2025-01-11T00:00:00.000Z");
  });
});
