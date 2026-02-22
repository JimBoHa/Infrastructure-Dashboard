import {
  alignSeriesPair,
  computeLagCorrelationSeries,
  linearRegression,
  pearsonCorrelation,
  spearmanCorrelation,
} from "@/features/trends/utils/correlation";
import type { TrendSeriesEntry } from "@/types/dashboard";

function makeSeries(sensor_id: string, points: Array<[string, number]>): TrendSeriesEntry {
  return {
    sensor_id,
    points: points.map(([timestamp, value]) => ({ timestamp: new Date(timestamp), value })),
  };
}

describe("correlation helpers", () => {
  it("computes Pearson correlations", () => {
    expect(pearsonCorrelation([1, 2, 3], [1, 2, 3])).toBeCloseTo(1, 6);
    expect(pearsonCorrelation([1, 2, 3], [3, 2, 1])).toBeCloseTo(-1, 6);
    expect(pearsonCorrelation([1, 1, 1], [2, 2, 2])).toBeNull();
  });

  it("computes Spearman correlations", () => {
    expect(spearmanCorrelation([1, 2, 3, 4], [10, 100, 1000, 10_000])).toBeCloseTo(1, 6);
    expect(spearmanCorrelation([1, 2, 3, 4], [4, 3, 2, 1])).toBeCloseTo(-1, 6);
  });

  it("aligns series with exact buckets and lag", () => {
    const a = makeSeries("a", [
      ["2026-01-01T00:00:00Z", 1],
      ["2026-01-01T00:01:00Z", 2],
    ]);
    const b = makeSeries("b", [
      ["2026-01-01T00:00:00Z", 10],
      ["2026-01-01T00:01:00Z", 20],
    ]);

    const aligned = alignSeriesPair(a, b, 0);
    expect(aligned.x).toEqual([1, 2]);
    expect(aligned.y).toEqual([10, 20]);

    const shifted = alignSeriesPair(a, b, 60);
    expect(shifted.x).toEqual([1]);
    expect(shifted.y).toEqual([20]);
  });

  it("computes lag correlation points", () => {
    const intervalSeconds = 60;
    const a = makeSeries("a", [
      ["2026-01-01T00:00:00Z", 0.2],
      ["2026-01-01T00:01:00Z", -1.3],
      ["2026-01-01T00:02:00Z", 2.5],
      ["2026-01-01T00:03:00Z", -0.7],
      ["2026-01-01T00:04:00Z", 1.1],
    ]);
    const b = makeSeries("b", [
      ["2026-01-01T00:01:00Z", 0.2],
      ["2026-01-01T00:02:00Z", -1.3],
      ["2026-01-01T00:03:00Z", 2.5],
      ["2026-01-01T00:04:00Z", -0.7],
      ["2026-01-01T00:05:00Z", 1.1],
    ]);

    const lagSeries = computeLagCorrelationSeries({
      a,
      b,
      method: "pearson",
      intervalSeconds,
      maxLagBuckets: 2,
    });

    const lagOne = lagSeries.points.find((point) => point.lag_buckets === 1);
    expect(lagOne?.n).toBe(5);
    expect(lagOne?.r).not.toBeNull();
    expect(lagOne?.r as number).toBeCloseTo(1, 6);
  });

  it("computes linear regression with r²", () => {
    const regression = linearRegression([1, 2, 3], [2, 4, 6]);
    expect(regression).not.toBeNull();
    expect(regression?.slope).toBeCloseTo(2, 6);
    expect(regression?.intercept).toBeCloseTo(0, 6);
    expect(regression?.r2).toBeCloseTo(1, 6);
  });
});

