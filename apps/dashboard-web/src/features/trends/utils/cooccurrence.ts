"use client";

import type { TrendSeriesEntry } from "@/types/dashboard";
import {
  detectChangeEvents,
  type DetectedEvent,
  type EventPolarity,
} from "@/features/trends/utils/eventMatch";

export type CooccurrenceSensorEvent = DetectedEvent & { sensorId: string };

export type CooccurrenceBucket = {
  ts: number;
  sensors: CooccurrenceSensorEvent[];
  groupSize: number;
  severitySum: number;
  pairWeight: number;
  score: number;
};

export type CooccurrenceResult = {
  buckets: CooccurrenceBucket[];
  perSensorEvents: Map<string, DetectedEvent[]>;
  timeline: number[];
};

const clampInt = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, Math.floor(value)));

function countNumericPoints(series: TrendSeriesEntry): number {
  let count = 0;
  for (const pt of series.points ?? []) {
    const v = pt?.value;
    if (typeof v === "number" && Number.isFinite(v)) count += 1;
  }
  return count;
}

function buildTimeline(series: TrendSeriesEntry[]): number[] {
  const set = new Set<number>();
  series.forEach((entry) => {
    entry.points?.forEach((pt) => {
      if (!(pt.timestamp instanceof Date)) return;
      const ts = pt.timestamp.getTime();
      if (!Number.isFinite(ts)) return;
      set.add(ts);
    });
  });
  return Array.from(set).sort((a, b) => a - b);
}

export function computeCooccurrenceBuckets(params: {
  series: TrendSeriesEntry[];
  intervalSeconds: number;
  zThreshold: number;
  minSeparationBuckets: number;
  polarity: EventPolarity;
  minSensors: number;
  toleranceBuckets: number;
  focusSensorId?: string;
  maxResults?: number;
}): CooccurrenceResult {
  const inputSeries = params.series.filter((s) => (s.points?.length ?? 0) > 0);
  const perSensorEvents = new Map<string, DetectedEvent[]>();
  if (inputSeries.length === 0) return { buckets: [], perSensorEvents, timeline: [] };

  const usableSeries = inputSeries.filter((s) => countNumericPoints(s) >= 3);
  if (usableSeries.length === 0) return { buckets: [], perSensorEvents, timeline: [] };

  const timeline = buildTimeline(usableSeries);
  if (timeline.length === 0) return { buckets: [], perSensorEvents, timeline: [] };

  const indexByTs = new Map<number, number>();
  timeline.forEach((ts, idx) => indexByTs.set(ts, idx));

  const tol = clampInt(params.toleranceBuckets, 0, 60);
  const minSensors = clampInt(params.minSensors, 2, usableSeries.length);
  const maxResults = clampInt(params.maxResults ?? 32, 1, 256);

  const bucketsByIndex: Array<Map<string, DetectedEvent> | undefined> = new Array(timeline.length);

  usableSeries.forEach((series) => {
    const events = detectChangeEvents({
      series,
      intervalSeconds: params.intervalSeconds,
      zThreshold: params.zThreshold,
      minSeparationBuckets: params.minSeparationBuckets,
      polarity: params.polarity,
    });
    perSensorEvents.set(series.sensor_id, events);

    events.forEach((evt) => {
      const idx = indexByTs.get(evt.ts);
      if (idx == null) return;

      const start = Math.max(0, idx - tol);
      const end = Math.min(timeline.length - 1, idx + tol);
      for (let i = start; i <= end; i += 1) {
        const map = bucketsByIndex[i] ?? new Map<string, DetectedEvent>();
        const prev = map.get(series.sensor_id);
        if (!prev || Math.abs(evt.z) > Math.abs(prev.z)) {
          map.set(series.sensor_id, evt);
        }
        bucketsByIndex[i] = map;
      }
    });
  });

  const candidates: Array<CooccurrenceBucket & { idx: number }> = [];
  bucketsByIndex.forEach((entry, idx) => {
    if (!entry || entry.size < minSensors) return;
    if (params.focusSensorId && !entry.has(params.focusSensorId)) return;
    const sensors: CooccurrenceSensorEvent[] = [];
    let severitySum = 0;
    entry.forEach((evt, sensorId) => {
      sensors.push({ ...evt, sensorId });
      severitySum += Math.abs(evt.z);
    });
    sensors.sort((a, b) => Math.abs(b.z) - Math.abs(a.z));

    const groupSize = entry.size;
    const pairWeight = (groupSize * (groupSize - 1)) / 2;
    const score = pairWeight * severitySum;
    if (!Number.isFinite(score) || score <= 0) return;
    candidates.push({
      idx,
      ts: timeline[idx] ?? 0,
      sensors,
      groupSize,
      severitySum,
      pairWeight,
      score,
    });
  });

  if (candidates.length === 0) {
    return { buckets: [], perSensorEvents, timeline };
  }

  candidates.sort((a, b) => {
    if (b.score !== a.score) return b.score - a.score;
    if (b.groupSize !== a.groupSize) return b.groupSize - a.groupSize;
    return b.ts - a.ts;
  });

  const blocked = new Array<boolean>(timeline.length).fill(false);
  const suppression = tol;

  const selected: CooccurrenceBucket[] = [];
  for (const candidate of candidates) {
    if (selected.length >= maxResults) break;
    if (blocked[candidate.idx]) continue;
    selected.push(candidate);

    if (suppression > 0) {
      const start = Math.max(0, candidate.idx - suppression);
      const end = Math.min(timeline.length - 1, candidate.idx + suppression);
      for (let i = start; i <= end; i += 1) blocked[i] = true;
    } else {
      blocked[candidate.idx] = true;
    }
  }

  return { buckets: selected, perSensorEvents, timeline };
}
