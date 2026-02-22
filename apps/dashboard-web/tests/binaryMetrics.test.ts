import { describe, expect, it } from "vitest";
import { decodeBinaryMetrics, readBinaryPoint } from "@/lib/binaryMetrics";

/** Build a valid binary-v1 buffer for testing. */
function buildBuffer(
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

  for (const s of series) {
    const idBytes = encoder.encode(s.sensor_id);
    parts.push(idBytes.length & 0xff, (idBytes.length >> 8) & 0xff);
    parts.push(...idBytes);

    const nameBytes = encoder.encode(s.sensor_name);
    parts.push(nameBytes.length & 0xff, (nameBytes.length >> 8) & 0xff);
    if (nameBytes.length > 0) parts.push(...nameBytes);

    const pc = s.points.length;
    parts.push(pc & 0xff, (pc >> 8) & 0xff, (pc >> 16) & 0xff, (pc >> 24) & 0xff);

    const f64buf = new ArrayBuffer(8);
    new DataView(f64buf).setFloat64(0, s.base_timestamp_ms, true);
    parts.push(...new Uint8Array(f64buf));
  }

  for (const s of series) {
    for (const pt of s.points) {
      const os = pt.offset_seconds;
      parts.push(os & 0xff, (os >> 8) & 0xff, (os >> 16) & 0xff, (os >> 24) & 0xff);
      const f32buf = new ArrayBuffer(4);
      new DataView(f32buf).setFloat32(0, pt.value, true);
      parts.push(...new Uint8Array(f32buf));
    }
  }

  return new Uint8Array(parts).buffer;
}

describe("decodeBinaryMetrics", () => {
  it("decodes a valid buffer with known values", () => {
    const baseMs = 1735689600000; // 2025-01-01T00:00:00Z
    const buffer = buildBuffer([
      {
        sensor_id: "temp_1",
        sensor_name: "Temperature",
        base_timestamp_ms: baseMs,
        points: [
          { offset_seconds: 0, value: 22.5 },
          { offset_seconds: 60, value: 23.0 },
          { offset_seconds: 120, value: 23.5 },
        ],
      },
    ]);

    const result = decodeBinaryMetrics(buffer);
    expect(result).toHaveLength(1);
    expect(result[0].sensor_id).toBe("temp_1");
    expect(result[0].sensor_name).toBe("Temperature");
    expect(result[0].base_timestamp_ms).toBe(baseMs);
    expect(result[0].point_count).toBe(3);

    // Verify point data
    const [ts0, val0] = readBinaryPoint(result[0], 0);
    expect(ts0).toBe(baseMs);
    expect(val0).toBeCloseTo(22.5, 1);

    const [ts1, val1] = readBinaryPoint(result[0], 1);
    expect(ts1).toBe(baseMs + 60000);
    expect(val1).toBeCloseTo(23.0, 1);

    const [ts2, val2] = readBinaryPoint(result[0], 2);
    expect(ts2).toBe(baseMs + 120000);
    expect(val2).toBeCloseTo(23.5, 1);
  });

  it("rejects invalid magic", () => {
    const buf = new ArrayBuffer(10);
    const view = new DataView(buf);
    view.setUint32(0, 0x00000000, true); // wrong magic
    view.setUint16(4, 0, true);
    view.setUint32(6, 0, true);

    expect(() => decodeBinaryMetrics(buf)).toThrow("Invalid binary metrics magic");
  });

  it("handles empty series (0 points)", () => {
    const buffer = buildBuffer([
      {
        sensor_id: "empty_sensor",
        sensor_name: "",
        base_timestamp_ms: 0,
        points: [],
      },
    ]);

    const result = decodeBinaryMetrics(buffer);
    expect(result).toHaveLength(1);
    expect(result[0].sensor_id).toBe("empty_sensor");
    expect(result[0].sensor_name).toBeNull();
    expect(result[0].point_count).toBe(0);
  });

  it("handles zero series", () => {
    const buffer = buildBuffer([]);
    const result = decodeBinaryMetrics(buffer);
    expect(result).toHaveLength(0);
  });

  it("decodes multiple series correctly", () => {
    const baseMs = 1735689600000;
    const buffer = buildBuffer([
      {
        sensor_id: "sensor_a",
        sensor_name: "Sensor A",
        base_timestamp_ms: baseMs,
        points: [
          { offset_seconds: 0, value: 10.0 },
          { offset_seconds: 1, value: 11.0 },
        ],
      },
      {
        sensor_id: "sensor_b",
        sensor_name: "Sensor B",
        base_timestamp_ms: baseMs + 5000,
        points: [
          { offset_seconds: 0, value: 100.0 },
        ],
      },
    ]);

    const result = decodeBinaryMetrics(buffer);
    expect(result).toHaveLength(2);

    expect(result[0].sensor_id).toBe("sensor_a");
    expect(result[0].point_count).toBe(2);
    expect(result[1].sensor_id).toBe("sensor_b");
    expect(result[1].point_count).toBe(1);

    const [tsA0, valA0] = readBinaryPoint(result[0], 0);
    expect(tsA0).toBe(baseMs);
    expect(valA0).toBeCloseTo(10.0, 1);

    const [tsB0, valB0] = readBinaryPoint(result[1], 0);
    expect(tsB0).toBe(baseMs + 5000);
    expect(valB0).toBeCloseTo(100.0, 1);
  });

  it("reconstructs timestamps accurately from offsets", () => {
    const baseMs = 1735689600000;
    const buffer = buildBuffer([
      {
        sensor_id: "s",
        sensor_name: "",
        base_timestamp_ms: baseMs,
        points: [
          { offset_seconds: 0, value: 1 },
          { offset_seconds: 3600, value: 2 },
          { offset_seconds: 86400, value: 3 },
        ],
      },
    ]);

    const result = decodeBinaryMetrics(buffer);
    const [ts0] = readBinaryPoint(result[0], 0);
    const [ts1] = readBinaryPoint(result[0], 1);
    const [ts2] = readBinaryPoint(result[0], 2);

    expect(ts0).toBe(baseMs);
    expect(ts1).toBe(baseMs + 3600 * 1000);
    expect(ts2).toBe(baseMs + 86400 * 1000);
  });
});
