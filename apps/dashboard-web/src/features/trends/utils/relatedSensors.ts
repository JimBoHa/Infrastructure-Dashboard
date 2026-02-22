import type { TrendSeriesEntry, TrendSeriesPoint } from "@/types/dashboard";
import {
  type CorrelationMethod,
  buildSeriesValueMap,
  computeLagCorrelationSeries,
  computePairCorrelation,
} from "@/features/trends/utils/correlation";

export type RelatedSensorSuggestion = {
  sensorId: string;
  r0: number | null;
  n0: number;
  bestLag: { lagBuckets: number; r: number | null; n: number } | null;
  best: { lagBuckets: number; r: number | null; n: number };
  score: number | null;
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

export function computeRelatedSensorSuggestions(params: {
  focus: TrendSeriesEntry;
  candidates: TrendSeriesEntry[];
  method: CorrelationMethod;
  intervalSeconds: number;
  maxLagBuckets: number;
  lagRefineTopK?: number;
}): RelatedSensorSuggestion[] {
  const candidates = params.candidates.filter((series) => series.points.length > 0);
  if (candidates.length === 0) return [];

  const valueMaps = new Map<string, Map<number, number>>();
  candidates.forEach((series) => valueMaps.set(series.sensor_id, buildSeriesValueMap(series)));

  const base = candidates
    .map((series) => {
      const bValueMap = valueMaps.get(series.sensor_id) ?? undefined;
      const { r, n } = computePairCorrelation({
        a: params.focus,
        b: series,
        method: params.method,
        bValueMap,
      });
      return {
        sensorId: series.sensor_id,
        r0: r,
        n0: n,
      };
    })
    .filter((entry) => entry.r0 != null && Number.isFinite(entry.r0));

  if (base.length === 0) return [];

  base.sort((a, b) => {
    const scoreA = Math.abs(a.r0 ?? 0);
    const scoreB = Math.abs(b.r0 ?? 0);
    if (scoreB !== scoreA) return scoreB - scoreA;
    if (b.n0 !== a.n0) return b.n0 - a.n0;
    return a.sensorId.localeCompare(b.sensorId);
  });

  const refineTopK = Math.max(0, Math.floor(params.lagRefineTopK ?? 10));
  const refineIds = new Set<string>();
  if (params.maxLagBuckets > 0 && refineTopK > 0) {
    base.slice(0, refineTopK).forEach((entry) => refineIds.add(entry.sensorId));
  }

  const bestById = new Map<string, RelatedSensorSuggestion>();
  base.forEach((entry) => {
    const best: RelatedSensorSuggestion["best"] = { lagBuckets: 0, r: entry.r0, n: entry.n0 };
    let bestLag: RelatedSensorSuggestion["bestLag"] = null;

    if (refineIds.has(entry.sensorId)) {
      const series = candidates.find((s) => s.sensor_id === entry.sensorId);
      if (series) {
        const lag = computeLagCorrelationSeries({
          a: params.focus,
          b: series,
          method: params.method,
          intervalSeconds: params.intervalSeconds,
          maxLagBuckets: params.maxLagBuckets,
        });
        if (lag.best) {
          bestLag = { lagBuckets: lag.best.lag_buckets, r: lag.best.r, n: lag.best.n };
          if (lag.best.r != null && Number.isFinite(lag.best.r)) {
            const absLag = Math.abs(lag.best.r);
            const absBase = Math.abs(entry.r0 ?? 0);
            if (absLag > absBase) {
              best.lagBuckets = lag.best.lag_buckets;
              best.r = lag.best.r;
              best.n = lag.best.n;
            }
          }
        }
      }
    }

    bestById.set(entry.sensorId, {
      sensorId: entry.sensorId,
      r0: entry.r0,
      n0: entry.n0,
      bestLag,
      best,
      score: best.r != null ? clamp(Math.abs(best.r), 0, 1) : null,
    });
  });

  const suggestions = Array.from(bestById.values());
  suggestions.sort((a, b) => {
    const scoreA = a.score ?? 0;
    const scoreB = b.score ?? 0;
    if (scoreB !== scoreA) return scoreB - scoreA;
    if ((b.best.n ?? 0) !== (a.best.n ?? 0)) return (b.best.n ?? 0) - (a.best.n ?? 0);
    return a.sensorId.localeCompare(b.sensorId);
  });
  return suggestions;
}

function numericValues(series: TrendSeriesEntry): number[] {
  const values: number[] = [];
  series.points.forEach((point) => {
    const v = point.value;
    if (typeof v === "number" && Number.isFinite(v)) values.push(v);
  });
  return values;
}

export function zScoreNormalizeSeries(series: TrendSeriesEntry): TrendSeriesEntry {
  const values = numericValues(series);
  if (values.length < 2) {
    return {
      ...series,
      unit: "z",
      display_decimals: 2,
      label: `${series.label ?? series.sensor_id} (z-score)`,
    };
  }

  const mean = values.reduce((sum, v) => sum + v, 0) / values.length;
  const variance =
    values.reduce((sum, v) => sum + (v - mean) * (v - mean), 0) / (values.length - 1);
  const std = Math.sqrt(Math.max(0, variance));
  const safeStd = std > 0 ? std : 1;

  const points: TrendSeriesPoint[] = series.points.map((point) => {
    const v = point.value;
    if (typeof v !== "number" || !Number.isFinite(v)) return point;
    return { ...point, value: (v - mean) / safeStd };
  });

  return {
    ...series,
    unit: "z",
    display_decimals: 2,
    label: `${series.label ?? series.sensor_id} (z-score)`,
    points,
  };
}

export function shiftSeriesTimestamps(series: TrendSeriesEntry, shiftSeconds: number): TrendSeriesEntry {
  const ms = Math.round(shiftSeconds * 1000);
  if (!Number.isFinite(ms) || ms === 0) return series;
  return {
    ...series,
    points: series.points.map((point) => ({
      ...point,
      timestamp: new Date(point.timestamp.getTime() + ms),
    })),
  };
}

