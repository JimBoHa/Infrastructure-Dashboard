import type { TrendSeriesEntry, TrendSeriesPoint } from "@/types/dashboard";

export type CorrelationMethod = "pearson" | "spearman";

export type CorrelationResult = {
  r: number | null;
  n: number;
};

export type AlignedPair = {
  timestamps: number[];
  x: number[];
  y: number[];
};

export type RegressionResult = {
  slope: number;
  intercept: number;
  r2: number | null;
};

export type LagCorrelationPoint = {
  lag_buckets: number;
  r: number | null;
  n: number;
};

export type LagCorrelationSeries = {
  points: LagCorrelationPoint[];
  best: LagCorrelationPoint | null;
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const toTimestampMs = (timestamp: TrendSeriesPoint["timestamp"]): number => {
  if (timestamp instanceof Date) return timestamp.getTime();
  if (typeof timestamp !== "string" && typeof timestamp !== "number") return Number.NaN;
  const parsed = new Date(timestamp);
  const ms = parsed.getTime();
  return Number.isFinite(ms) ? ms : Number.NaN;
};

export function buildSeriesValueMap(series: TrendSeriesEntry): Map<number, number> {
  const map = new Map<number, number>();
  series.points.forEach((point) => {
    const ts = toTimestampMs(point.timestamp);
    if (!Number.isFinite(ts)) return;
    const value = point.value;
    if (typeof value !== "number" || !Number.isFinite(value)) return;
    map.set(ts, value);
  });
  return map;
}

/**
 * Aligns `a(t)` with `b(t + lagSeconds)` using exact bucket timestamps.
 * Positive lag means `b` occurs later in time (i.e., `a` leads `b`).
 */
export function alignSeriesPair(
  a: TrendSeriesEntry,
  b: TrendSeriesEntry,
  lagSeconds = 0,
  bValueMap?: Map<number, number>,
): AlignedPair {
  const lagMs = Math.round(lagSeconds * 1000);
  const mapB = bValueMap ?? buildSeriesValueMap(b);
  const timestamps: number[] = [];
  const x: number[] = [];
  const y: number[] = [];

  a.points.forEach((point) => {
    const ts = toTimestampMs(point.timestamp);
    if (!Number.isFinite(ts)) return;
    const bValue = mapB.get(ts + lagMs);
    if (bValue == null) return;
    const aValue = point.value;
    if (typeof aValue !== "number" || !Number.isFinite(aValue) || !Number.isFinite(bValue)) return;
    timestamps.push(ts);
    x.push(aValue);
    y.push(bValue);
  });

  return { timestamps, x, y };
}

export function pearsonCorrelation(x: number[], y: number[]): number | null {
  if (x.length !== y.length || x.length < 3) return null;
  const n = x.length;
  let sumX = 0;
  let sumY = 0;
  let sumXX = 0;
  let sumYY = 0;
  let sumXY = 0;

  for (let idx = 0; idx < n; idx += 1) {
    const xv = x[idx];
    const yv = y[idx];
    sumX += xv;
    sumY += yv;
    sumXX += xv * xv;
    sumYY += yv * yv;
    sumXY += xv * yv;
  }

  const numerator = n * sumXY - sumX * sumY;
  const denomX = n * sumXX - sumX * sumX;
  const denomY = n * sumYY - sumY * sumY;
  if (denomX <= 0 || denomY <= 0) return null;
  const r = numerator / Math.sqrt(denomX * denomY);
  if (!Number.isFinite(r)) return null;
  return clamp(r, -1, 1);
}

function rank(values: number[]): number[] {
  const indexed = values.map((value, idx) => ({ value, idx }));
  indexed.sort((a, b) => a.value - b.value);
  const ranks = new Array<number>(values.length);
  let i = 0;
  while (i < indexed.length) {
    let j = i + 1;
    while (j < indexed.length && indexed[j].value === indexed[i].value) {
      j += 1;
    }
    const averageRank = (i + 1 + j) / 2;
    for (let k = i; k < j; k += 1) {
      ranks[indexed[k].idx] = averageRank;
    }
    i = j;
  }
  return ranks;
}

export function spearmanCorrelation(x: number[], y: number[]): number | null {
  if (x.length !== y.length || x.length < 3) return null;
  const rx = rank(x);
  const ry = rank(y);
  return pearsonCorrelation(rx, ry);
}

export function computePairCorrelation(params: {
  a: TrendSeriesEntry;
  b: TrendSeriesEntry;
  method: CorrelationMethod;
  lagSeconds?: number;
  bValueMap?: Map<number, number>;
}): CorrelationResult {
  const aligned = alignSeriesPair(
    params.a,
    params.b,
    params.lagSeconds ?? 0,
    params.bValueMap,
  );
  const n = aligned.x.length;
  const r =
    params.method === "spearman"
      ? spearmanCorrelation(aligned.x, aligned.y)
      : pearsonCorrelation(aligned.x, aligned.y);
  return { r, n };
}

export function linearRegression(x: number[], y: number[]): RegressionResult | null {
  if (x.length !== y.length || x.length < 2) return null;
  const n = x.length;
  let sumX = 0;
  let sumY = 0;
  let sumXX = 0;
  let sumXY = 0;

  for (let idx = 0; idx < n; idx += 1) {
    const xv = x[idx];
    const yv = y[idx];
    sumX += xv;
    sumY += yv;
    sumXX += xv * xv;
    sumXY += xv * yv;
  }

  const denom = n * sumXX - sumX * sumX;
  if (denom === 0) return null;
  const slope = (n * sumXY - sumX * sumY) / denom;
  const intercept = (sumY - slope * sumX) / n;
  if (!Number.isFinite(slope) || !Number.isFinite(intercept)) return null;
  const r = pearsonCorrelation(x, y);
  const r2 = r != null ? clamp(r * r, 0, 1) : null;
  return { slope, intercept, r2 };
}

export function rollingPearsonCorrelation(
  aligned: AlignedPair,
  windowPoints: number,
): Array<{ timestamp: Date; value: number }> {
  const n = aligned.x.length;
  const window = Math.floor(windowPoints);
  if (window < 3 || n < window) return [];

  let sumX = 0;
  let sumY = 0;
  let sumXX = 0;
  let sumYY = 0;
  let sumXY = 0;

  for (let idx = 0; idx < window; idx += 1) {
    const xv = aligned.x[idx]!;
    const yv = aligned.y[idx]!;
    sumX += xv;
    sumY += yv;
    sumXX += xv * xv;
    sumYY += yv * yv;
    sumXY += xv * yv;
  }

  const points: Array<{ timestamp: Date; value: number }> = [];
  for (let end = window - 1; end < n; end += 1) {
    const numerator = window * sumXY - sumX * sumY;
    const denomX = window * sumXX - sumX * sumX;
    const denomY = window * sumYY - sumY * sumY;
    const r =
      denomX > 0 && denomY > 0
        ? clamp(numerator / Math.sqrt(denomX * denomY), -1, 1)
        : Number.NaN;
    const ts = aligned.timestamps[end]!;
    if (Number.isFinite(ts) && Number.isFinite(r)) {
      points.push({ timestamp: new Date(ts), value: r });
    }

    const next = end + 1;
    if (next >= n) break;
    const removeIdx = end - window + 1;
    const removeX = aligned.x[removeIdx]!;
    const removeY = aligned.y[removeIdx]!;
    sumX -= removeX;
    sumY -= removeY;
    sumXX -= removeX * removeX;
    sumYY -= removeY * removeY;
    sumXY -= removeX * removeY;

    const addX = aligned.x[next]!;
    const addY = aligned.y[next]!;
    sumX += addX;
    sumY += addY;
    sumXX += addX * addX;
    sumYY += addY * addY;
    sumXY += addX * addY;
  }

  return points;
}

export function computeLagCorrelationSeries(params: {
  a: TrendSeriesEntry;
  b: TrendSeriesEntry;
  method: CorrelationMethod;
  intervalSeconds: number;
  maxLagBuckets: number;
}): LagCorrelationSeries {
  const maxLag = Math.max(0, Math.floor(params.maxLagBuckets));
  const points: LagCorrelationPoint[] = [];
  const bValueMap = buildSeriesValueMap(params.b);
  for (let lag = -maxLag; lag <= maxLag; lag += 1) {
    const { r, n } = computePairCorrelation({
      a: params.a,
      b: params.b,
      method: params.method,
      lagSeconds: lag * params.intervalSeconds,
      bValueMap,
    });
    points.push({ lag_buckets: lag, r, n });
  }

  let best: LagCorrelationPoint | null = null;
  points.forEach((point) => {
    if (point.r == null || point.n < 3) return;
    if (!best) {
      best = point;
      return;
    }
    if (Math.abs(point.r) > Math.abs(best.r ?? 0)) {
      best = point;
    }
  });

  return { points, best };
}
