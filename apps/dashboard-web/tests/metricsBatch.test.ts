import { describe, expect, it, vi } from "vitest";

import { fetchMetricsSeriesBatched } from "@/features/trends/utils/metricsBatch";
import type { TrendSeriesEntry } from "@/types/dashboard";
import { fetchMetricsSeries } from "@/lib/api";

vi.mock("@/lib/api", () => ({
  fetchMetricsSeries: vi.fn(),
}));

describe("fetchMetricsSeriesBatched", () => {
  it("fetches in batches and reports progress", async () => {
    const mockFetch = fetchMetricsSeries as unknown as ReturnType<typeof vi.fn>;
    mockFetch.mockImplementation(async (...args: unknown[]) => {
      const sensorIds = args[0] as string[];
      return sensorIds.map(
        (sensor_id): TrendSeriesEntry =>
          ({
            sensor_id,
            points: [],
          }) as TrendSeriesEntry,
      );
    });

    const progressUpdates: Array<{ processedSensors: number; totalSensors: number; completedRequests: number }> = [];

    const result = await fetchMetricsSeriesBatched({
      sensorIds: ["a", "b", "c", "d", "e", "f"],
      start: new Date("2026-01-01T00:00:00.000Z").toISOString(),
      end: new Date("2026-01-02T00:00:00.000Z").toISOString(),
      interval: 60,
      batchSize: 6,
      onProgress: (update) => progressUpdates.push(update),
    });

    expect(result.map((entry) => entry.sensor_id)).toEqual(["a", "b", "c", "d", "e", "f"]);
    expect(mockFetch).toHaveBeenCalledTimes(1);
    expect(progressUpdates.length).toBeGreaterThan(1);
    expect(progressUpdates.at(-1)).toMatchObject({
      processedSensors: 6,
      totalSensors: 6,
    });
  });
});
