import { describe, expect, it } from "vitest";
import { buildMetricsQuery } from "@/lib/api";

describe("buildMetricsQuery", () => {
  it("builds metrics query with sensor ids and params", () => {
    const start = "2025-10-01T00:00:00Z";
    const end = "2025-10-02T00:00:00Z";
    const url = buildMetricsQuery(["sensor-a", "sensor-b"], start, end, 300);
    expect(url).toContain("/api/metrics/query?");
    expect(url).toContain("sensor_ids%5B%5D=sensor-a");
    expect(url).toContain("sensor_ids%5B%5D=sensor-b");
    expect(url).toContain("start=2025-10-01T00%3A00%3A00Z");
    expect(url).toContain("end=2025-10-02T00%3A00%3A00Z");
    expect(url).toContain("interval=300");
    expect(url).toContain("format=binary");
  });
});
