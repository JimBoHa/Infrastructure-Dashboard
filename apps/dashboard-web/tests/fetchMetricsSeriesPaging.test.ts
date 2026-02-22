import { describe, expect, it, vi } from "vitest";

import { fetchMetricsSeries } from "@/lib/api";
import { fetchBinary } from "@/lib/http";

vi.mock("@/lib/http", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/http")>();
  return {
    ...actual,
    fetchBinary: vi.fn(),
  };
});

/** Build a valid binary-v1 buffer for testing. */
function buildBinaryBuffer(
  series: Array<{
    sensor_id: string;
    sensor_name: string;
    points: Array<{ offset_seconds: number; value: number }>;
    base_timestamp_ms: number;
  }>,
): ArrayBuffer {
  const parts: number[] = [];
  const encoder = new TextEncoder();

  // Magic "FDB1"
  parts.push(0x46, 0x44, 0x42, 0x31);

  // series_count (u16 LE)
  const sc = series.length;
  parts.push(sc & 0xff, (sc >> 8) & 0xff);

  // total_point_count (u32 LE)
  const totalPoints = series.reduce((sum, s) => sum + s.points.length, 0);
  parts.push(
    totalPoints & 0xff,
    (totalPoints >> 8) & 0xff,
    (totalPoints >> 16) & 0xff,
    (totalPoints >> 24) & 0xff,
  );

  // Headers
  for (const s of series) {
    const idBytes = encoder.encode(s.sensor_id);
    parts.push(idBytes.length & 0xff, (idBytes.length >> 8) & 0xff);
    parts.push(...idBytes);

    const nameBytes = encoder.encode(s.sensor_name);
    parts.push(nameBytes.length & 0xff, (nameBytes.length >> 8) & 0xff);
    if (nameBytes.length > 0) parts.push(...nameBytes);

    // point_count (u32 LE)
    const pc = s.points.length;
    parts.push(pc & 0xff, (pc >> 8) & 0xff, (pc >> 16) & 0xff, (pc >> 24) & 0xff);

    // base_timestamp_ms (f64 LE)
    const f64buf = new ArrayBuffer(8);
    new DataView(f64buf).setFloat64(0, s.base_timestamp_ms, true);
    parts.push(...new Uint8Array(f64buf));
  }

  // Bulk data
  for (const s of series) {
    for (const pt of s.points) {
      // offset_seconds (u32 LE)
      const os = pt.offset_seconds;
      parts.push(os & 0xff, (os >> 8) & 0xff, (os >> 16) & 0xff, (os >> 24) & 0xff);
      // value (f32 LE)
      const f32buf = new ArrayBuffer(4);
      new DataView(f32buf).setFloat32(0, pt.value, true);
      parts.push(...new Uint8Array(f32buf));
    }
  }

  return new Uint8Array(parts).buffer;
}

describe("fetchMetricsSeries binary", () => {
  it("decodes a single-series binary response", async () => {
    const mockFetchBinary = fetchBinary as unknown as ReturnType<typeof vi.fn>;
    const baseMs = new Date("2026-01-01T00:00:00.000Z").getTime();

    mockFetchBinary.mockResolvedValueOnce(
      buildBinaryBuffer([
        {
          sensor_id: "sensor-a",
          sensor_name: "Temperature",
          base_timestamp_ms: baseMs,
          points: [
            { offset_seconds: 0, value: 23.5 },
            { offset_seconds: 1, value: 24.0 },
            { offset_seconds: 2, value: 24.5 },
            { offset_seconds: 3, value: 25.0 },
          ],
        },
      ]),
    );

    const result = await fetchMetricsSeries(
      ["sensor-a"],
      "2026-01-01T00:00:00.000Z",
      "2026-01-01T00:00:03.000Z",
      1,
    );

    expect(mockFetchBinary).toHaveBeenCalledTimes(1);
    const url = String(mockFetchBinary.mock.calls[0]?.[0] ?? "");
    expect(url).toContain("format=binary");

    expect(result).toHaveLength(1);
    expect(result[0]?.sensor_id).toBe("sensor-a");
    expect(result[0]?.label).toBe("Temperature");
    expect(result[0]?.points).toHaveLength(4);

    const values = result[0]?.points.map((p) => p.value);
    expect(values?.[0]).toBeCloseTo(23.5, 1);
    expect(values?.[1]).toBeCloseTo(24.0, 1);
    expect(values?.[2]).toBeCloseTo(24.5, 1);
    expect(values?.[3]).toBeCloseTo(25.0, 1);

    expect(result[0]?.points[0]?.timestamp.getTime()).toBe(baseMs);
    expect(result[0]?.points[1]?.timestamp.getTime()).toBe(baseMs + 1000);
  });

  it("returns empty array for empty sensor list", async () => {
    const result = await fetchMetricsSeries(
      [],
      "2026-01-01T00:00:00.000Z",
      "2026-01-01T00:00:03.000Z",
      1,
    );
    expect(result).toEqual([]);
  });
});
