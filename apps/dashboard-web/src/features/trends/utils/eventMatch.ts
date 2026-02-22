"use client";

import type { TrendSeriesEntry } from "@/types/dashboard";

export type EventPolarity = "both" | "up" | "down";

export type DetectedEvent = {
  ts: number;
  z: number;
  direction: "up" | "down";
  delta: number;
};

export type EventMatchSuggestion = {
  sensorId: string;
  score0: number | null;
  overlap0: number;
  nFocus: number;
  nCandidate0: number;
  bestLag: { lagBuckets: number; score: number | null; overlap: number; nCandidate: number } | null;
  best: { lagBuckets: number; score: number | null; overlap: number; nCandidate: number };
  score: number | null;
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

function median(values: number[]): number | null {
  const sorted = values.filter(Number.isFinite).slice().sort((a, b) => a - b);
  if (!sorted.length) return null;
  const mid = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 1) return sorted[mid]!;
  return (sorted[mid - 1]! + sorted[mid]!) / 2;
}

function medianAbsoluteDeviation(values: number[], med: number): number | null {
  const deviations = values
    .filter(Number.isFinite)
    .map((v) => Math.abs(v - med))
    .filter(Number.isFinite);
  return median(deviations);
}

function mean(values: number[]): number | null {
  const finite = values.filter(Number.isFinite);
  if (finite.length === 0) return null;
  return finite.reduce((sum, value) => sum + value, 0) / finite.length;
}

function robustScale(values: number[]): { center: number; scale: number } | null {
  if (values.length < 3) return null;
  const center = median(values);
  if (center == null) return null;

  // Standard robust scale uses MAD * 1.4826 (~std dev under Normal).
  // For quantized signals, many deltas can be exactly 0, making MAD = 0.
  // In that degenerate case, fall back to a scale estimate that still
  // preserves "delta-based" event detection without dividing by ~0.
  const mad = medianAbsoluteDeviation(values, center);
  const epsilon = 1e-9;
  if (mad != null && mad > epsilon) {
    return { center, scale: mad * 1.4826 };
  }

  const deviations = values
    .filter(Number.isFinite)
    .map((v) => Math.abs(v - center))
    .filter(Number.isFinite);

  const nonZeroDeviations = deviations.filter((d) => d > epsilon);
  if (nonZeroDeviations.length >= 10) {
    const nonZeroMad = median(nonZeroDeviations);
    if (nonZeroMad != null && nonZeroMad > epsilon) {
      return { center, scale: nonZeroMad * 1.4826 };
    }
  }

  const meanAbs = mean(deviations);
  if (meanAbs != null && meanAbs > epsilon) {
    // Convert mean absolute deviation to a std-like scale:
    // mean(|X - μ|) ≈ σ * sqrt(2/π) => σ ≈ meanAbs * sqrt(π/2)
    const MEAN_ABS_TO_STD = 1.2533141373155001;
    return { center, scale: meanAbs * MEAN_ABS_TO_STD };
  }

  return null;
}

export function detectChangeEvents(params: {
  series: TrendSeriesEntry;
  intervalSeconds: number;
  zThreshold: number;
  minSeparationBuckets: number;
  polarity: EventPolarity;
  timestampAllowed?: (ts: number) => boolean;
}): DetectedEvent[] {
  const threshold = Math.max(0, params.zThreshold);
  const minSepBuckets = Math.max(0, Math.floor(params.minSeparationBuckets));
  const minSepMs =
    minSepBuckets > 0 && params.intervalSeconds > 0
      ? Math.round(minSepBuckets * params.intervalSeconds * 1000)
      : 0;

  const deltas: Array<{ ts: number; delta: number }> = [];

  const points = params.series.points ?? [];
  for (let idx = 1; idx < points.length; idx += 1) {
    const prev = points[idx - 1];
    const curr = points[idx];
    if (!prev || !curr) continue;
    const prevV = prev.value;
    const currV = curr.value;
    if (typeof prevV !== "number" || typeof currV !== "number") continue;
    if (!Number.isFinite(prevV) || !Number.isFinite(currV)) continue;
    const ts = curr.timestamp instanceof Date ? curr.timestamp.getTime() : Number.NaN;
    if (!Number.isFinite(ts)) continue;
    if (params.timestampAllowed && !params.timestampAllowed(ts)) continue;
    deltas.push({ ts, delta: currV - prevV });
  }

  if (deltas.length < 3 || threshold <= 0) return [];

  const stats = robustScale(deltas.map((d) => d.delta));
  if (!stats) return [];

  const rawEvents: DetectedEvent[] = [];
  deltas.forEach(({ ts, delta }) => {
    const z = (delta - stats.center) / stats.scale;
    if (!Number.isFinite(z)) return;
    if (Math.abs(z) < threshold) return;
    const direction = z >= 0 ? "up" : "down";
    if (params.polarity === "up" && direction !== "up") return;
    if (params.polarity === "down" && direction !== "down") return;
    rawEvents.push({ ts, z, direction, delta });
  });

  if (rawEvents.length === 0) return [];
  rawEvents.sort((a, b) => a.ts - b.ts);

  if (!minSepMs) return rawEvents;

  const merged: DetectedEvent[] = [];
  for (const evt of rawEvents) {
    const last = merged[merged.length - 1];
    if (!last || evt.ts - last.ts > minSepMs) {
      merged.push(evt);
      continue;
    }
    if (Math.abs(evt.z) > Math.abs(last.z)) {
      merged[merged.length - 1] = evt;
    }
  }
  return merged;
}

function f1Score(overlap: number, nFocus: number, nCandidate: number): number | null {
  const a = Math.max(0, Math.floor(nFocus));
  const b = Math.max(0, Math.floor(nCandidate));
  const m = Math.max(0, Math.floor(overlap));
  if (a === 0 && b === 0) return null;
  if (a === 0 || b === 0) return 0;
  return clamp((2 * m) / (a + b), 0, 1);
}

function countOverlap(params: { focus: number[]; candidateSet: Set<number>; lagMs: number }): number {
  let matches = 0;
  const lagMs = params.lagMs;
  for (const ts of params.focus) {
    if (params.candidateSet.has(ts + lagMs)) matches += 1;
  }
  return matches;
}

export function computeEventMatchSuggestions(params: {
  focus: TrendSeriesEntry;
  candidates: TrendSeriesEntry[];
  intervalSeconds: number;
  maxLagBuckets: number;
  lagRefineTopK?: number;
  zThreshold: number;
  minSeparationBuckets: number;
  polarity: EventPolarity;
  timestampAllowed?: (ts: number) => boolean;
}): EventMatchSuggestion[] {
  const candidateSeries = params.candidates.filter((series) => (series.points?.length ?? 0) > 0);
  if (candidateSeries.length === 0) return [];

  const focusEvents = detectChangeEvents({
    series: params.focus,
    intervalSeconds: params.intervalSeconds,
    zThreshold: params.zThreshold,
    minSeparationBuckets: params.minSeparationBuckets,
    polarity: params.polarity,
    timestampAllowed: params.timestampAllowed,
  });
  const focus = focusEvents.map((e) => e.ts);
  const nFocus = focus.length;
  if (nFocus === 0) return [];

  const cached = new Map<string, { set: Set<number>; n: number }>();
  candidateSeries.forEach((series) => {
    const events = detectChangeEvents({
      series,
      intervalSeconds: params.intervalSeconds,
      zThreshold: params.zThreshold,
      minSeparationBuckets: params.minSeparationBuckets,
      polarity: params.polarity,
      timestampAllowed: params.timestampAllowed,
    });
    const set = new Set<number>(events.map((e) => e.ts));
    cached.set(series.sensor_id, { set, n: set.size });
  });

  const base = candidateSeries
    .map((series) => {
      const cachedEntry = cached.get(series.sensor_id);
      const candidateSet = cachedEntry?.set ?? new Set<number>();
      const nCandidate0 = cachedEntry?.n ?? 0;
      const overlap0 = nCandidate0 ? countOverlap({ focus, candidateSet, lagMs: 0 }) : 0;
      const score0 = f1Score(overlap0, nFocus, nCandidate0);
      return {
        sensorId: series.sensor_id,
        score0,
        overlap0,
        nFocus,
        nCandidate0,
      };
    })
    .filter((entry) => entry.score0 != null && (entry.nCandidate0 ?? 0) > 0);

  if (base.length === 0) return [];

  base.sort((a, b) => {
    const scoreA = a.score0 ?? 0;
    const scoreB = b.score0 ?? 0;
    if (scoreB !== scoreA) return scoreB - scoreA;
    if (b.overlap0 !== a.overlap0) return b.overlap0 - a.overlap0;
    return a.sensorId.localeCompare(b.sensorId);
  });

  const refineTopK = Math.max(0, Math.floor(params.lagRefineTopK ?? 10));
  const refineIds = new Set<string>();
  if (params.maxLagBuckets > 0 && refineTopK > 0) {
    base.slice(0, refineTopK).forEach((entry) => refineIds.add(entry.sensorId));
  }

  const maxLag = Math.max(0, Math.floor(params.maxLagBuckets));
  const bestById = new Map<string, EventMatchSuggestion>();

  base.forEach((entry) => {
    const best: EventMatchSuggestion["best"] = {
      lagBuckets: 0,
      score: entry.score0,
      overlap: entry.overlap0,
      nCandidate: entry.nCandidate0,
    };
    let bestLag: EventMatchSuggestion["bestLag"] = null;

    if (refineIds.has(entry.sensorId) && maxLag > 0) {
      const cachedEntry = cached.get(entry.sensorId);
      const candidateSet = cachedEntry?.set ?? new Set<number>();
      const nCandidate = cachedEntry?.n ?? 0;
      for (let lag = -maxLag; lag <= maxLag; lag += 1) {
        const lagMs = lag * params.intervalSeconds * 1000;
        const overlap = nCandidate ? countOverlap({ focus, candidateSet, lagMs }) : 0;
        const score = f1Score(overlap, nFocus, nCandidate);
        const currentScore = score ?? 0;
        const bestScore = best.score ?? 0;
        if (currentScore > bestScore || (currentScore === bestScore && overlap > best.overlap)) {
          best.lagBuckets = lag;
          best.score = score;
          best.overlap = overlap;
          best.nCandidate = nCandidate;
        }
      }

      bestLag = best.lagBuckets
        ? { lagBuckets: best.lagBuckets, score: best.score, overlap: best.overlap, nCandidate: best.nCandidate }
        : null;
    }

    bestById.set(entry.sensorId, {
      sensorId: entry.sensorId,
      score0: entry.score0,
      overlap0: entry.overlap0,
      nFocus,
      nCandidate0: entry.nCandidate0,
      bestLag,
      best,
      score: best.score,
    });
  });

  const suggestions = Array.from(bestById.values());
  suggestions.sort((a, b) => {
    const scoreA = a.score ?? 0;
    const scoreB = b.score ?? 0;
    if (scoreB !== scoreA) return scoreB - scoreA;
    if (b.best.overlap !== a.best.overlap) return b.best.overlap - a.best.overlap;
    return a.sensorId.localeCompare(b.sensorId);
  });
  return suggestions;
}
