import { describe, expect, it } from "vitest";
import {
  alignSeriesByTimestamp,
  applyTemperatureCompensation,
  buildTempCompensationExpression,
  fitTemperatureDriftModel,
  suggestTemperatureLagSeconds,
  type AlignedTempCompensationPoint,
} from "@/lib/tempCompensation";
import type { TrendSeriesEntry } from "@/types/dashboard";

function makeSeries(
  sensorId: string,
  points: Array<[number, number | null]>,
): TrendSeriesEntry {
  return {
    sensor_id: sensorId,
    label: sensorId,
    unit: undefined,
    display_decimals: undefined,
    points: points.map(([ms, value]) => ({
      timestamp: new Date(ms),
      value,
      samples: undefined,
    })),
  };
}

describe("tempCompensation", () => {
  it("alignSeriesByTimestamp joins on matching timestamps and ignores nulls", () => {
    const base = 1_700_000_000_000;
    const raw = makeSeries("raw", [
      [base, 10],
      [base + 1000, null],
      [base + 2000, 12],
    ]);
    const temp = makeSeries("temp", [
      [base, 20],
      [base + 2000, 22],
      [base + 3000, 23],
    ]);

    const aligned = alignSeriesByTimestamp(raw, temp);
    expect(aligned).toHaveLength(2);
    expect(aligned[0]?.raw).toBe(10);
    expect(aligned[0]?.temperature).toBe(20);
    expect(aligned[1]?.raw).toBe(12);
    expect(aligned[1]?.temperature).toBe(22);
  });

  it("alignSeriesByTimestamp can apply a positive temperature lag (use past temperature)", () => {
    const base = 1_700_000_000_000;
    const raw = makeSeries("raw", [
      [base + 1000, 10],
      [base + 2000, 12],
    ]);
    const temp = makeSeries("temp", [
      [base + 0, 20],
      [base + 1000, 22],
    ]);

    // With a 1s lag, raw[t] aligns with temp[t-1s]
    const aligned = alignSeriesByTimestamp(raw, temp, { temperatureLagSeconds: 1 });
    expect(aligned).toHaveLength(2);
    expect(aligned[0]?.raw).toBe(10);
    expect(aligned[0]?.temperature).toBe(20);
    expect(aligned[1]?.raw).toBe(12);
    expect(aligned[1]?.temperature).toBe(22);
  });

  it("fitTemperatureDriftModel recovers polynomial coefficients (degree 2) for noiseless data", () => {
    const points: AlignedTempCompensationPoint[] = [];
    const centerTemp = 12.5;
    const b0 = 100;
    const b1 = 2;
    const b2 = 0.5;

    for (let i = 0; i < 80; i += 1) {
      const temperature = 5 + (25 - 5) * (i / 79);
      const x = temperature - centerTemp;
      const raw = b0 + b1 * x + b2 * x * x;
      points.push({ timestamp: new Date(1_700_000_000_000 + i * 60_000), temperature, raw });
    }

    const fit = fitTemperatureDriftModel({ points, degree: 2, centerTemp });
    expect(fit).not.toBeNull();
    if (!fit) return;

    expect(fit.coefficients).toHaveLength(3);
    expect(fit.coefficients[0]).toBeCloseTo(b0, 6);
    expect(fit.coefficients[1]).toBeCloseTo(b1, 6);
    expect(fit.coefficients[2]).toBeCloseTo(b2, 6);
    expect(fit.r2).toBeCloseTo(1, 6);
  });

  it("fitTemperatureDriftModel can detrend a slow time drift without biasing temperature coefficients", () => {
    const points: AlignedTempCompensationPoint[] = [];
    const baseTs = 1_700_000_000_000;
    const centerTemp = 20;
    const b0 = 100;
    const b1 = 2.0; // raw units per °C around centerTemp
    const timeSlopePerDay = 10.0; // raw units per day

    // 72 hours @ 1h resolution, daily temperature cycle + slow linear time drift.
    for (let hour = 0; hour < 72; hour += 1) {
      const timeDays = hour / 24;
      const temperature = centerTemp + 5 * Math.sin(2 * Math.PI * timeDays);
      const x = temperature - centerTemp;
      const raw = b0 + b1 * x + timeSlopePerDay * timeDays;
      points.push({ timestamp: new Date(baseTs + hour * 3_600_000), temperature, raw });
    }

    const fit = fitTemperatureDriftModel({ points, degree: 1, centerTemp, includeTimeSlope: true });
    expect(fit).not.toBeNull();
    if (!fit) return;

    // Temperature slope recovered (intercept depends on center time).
    expect(fit.coefficients).toHaveLength(2);
    expect(fit.coefficients[1]).toBeCloseTo(b1, 6);

    // Time slope recovered.
    expect(fit.timeSlopePerDay).not.toBeNull();
    expect(fit.timeSlopePerDay ?? 0).toBeCloseTo(timeSlopePerDay, 6);
    expect(fit.r2).toBeCloseTo(1, 6);
  });

  it("applyTemperatureCompensation subtracts drift so corrected equals raw at center temperature", () => {
    const fit = {
      centerTemp: 10,
      coefficients: [50, 2] as number[],
    };
    // At centerTemp, drift term is zero, so corrected == raw
    expect(applyTemperatureCompensation(123, 10, fit)).toBeCloseTo(123, 8);
    // At +1 C, drift is +2, so corrected is -2
    expect(applyTemperatureCompensation(123, 11, fit)).toBeCloseTo(121, 8);
  });

  it("buildTempCompensationExpression creates a derived-sensor expression", () => {
    const built = buildTempCompensationExpression({
      rawVar: "raw",
      temperatureVar: "t",
      centerTemp: 10,
      coefficients: [50, 2, 0.5],
      clampAbs: 5,
    });
    expect(built).not.toBeNull();
    expect(built?.expression).toContain("raw -");
    expect(built?.expression).toContain("clamp(");
    expect(built?.expression).toContain("pow(");
  });

  it("suggestTemperatureLagSeconds can recover a simple fixed lag", () => {
    const base = 1_700_000_000_000;
    const intervalSeconds = 60;
    const lagSeconds = 600; // 10 minutes (multiple of 5 minutes scan step)
    const b1 = 2.0;

    const tempPoints: Array<[number, number | null]> = [];
    const rawPoints: Array<[number, number | null]> = [];

    const tempByMs = new Map<number, number>();
    for (let i = 0; i < 180; i += 1) {
      const ms = base + i * intervalSeconds * 1000;
      const t = 20 + 5 * Math.sin((2 * Math.PI * i) / 60);
      tempPoints.push([ms, t]);
      tempByMs.set(ms, t);
    }

    for (let i = 0; i < 180; i += 1) {
      const ms = base + i * intervalSeconds * 1000;
      const lagMs = lagSeconds * 1000;
      const tLag = tempByMs.get(ms - lagMs) ?? tempByMs.get(base) ?? 20;
      const raw = 100 + b1 * (tLag - 20);
      rawPoints.push([ms, raw]);
    }

    const tempSeries = makeSeries("temp", tempPoints);
    const rawSeries = makeSeries("raw", rawPoints);

    const suggested = suggestTemperatureLagSeconds({
      rawSeries,
      temperatureSeries: tempSeries,
      intervalSeconds,
      degree: 1,
      includeTimeSlope: false,
    });

    expect(suggested.lagSeconds).toBe(lagSeconds);
    expect(suggested.best?.reductionPct).toBeGreaterThan(80);
  });
});
