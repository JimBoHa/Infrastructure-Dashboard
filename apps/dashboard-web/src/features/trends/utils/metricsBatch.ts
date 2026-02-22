"use client";

import { fetchMetricsSeries } from "@/lib/api";
import type { TrendSeriesEntry } from "@/types/dashboard";

function chunk<T>(items: T[], size: number): T[][] {
  const batchSize = Math.max(1, Math.floor(size));
  if (items.length <= batchSize) return [items];
  const chunks: T[][] = [];
  for (let i = 0; i < items.length; i += batchSize) {
    chunks.push(items.slice(i, i + batchSize));
  }
  return chunks;
}

export async function fetchMetricsSeriesBatched(params: {
  sensorIds: string[];
  start: string;
  end: string;
  interval: number;
  batchSize?: number;
  signal?: AbortSignal;
  onProgress?: (progress: {
    processedSensors: number;
    totalSensors: number;
    completedRequests: number;
  }) => void;
}): Promise<TrendSeriesEntry[]> {
  const cleanIds = Array.from(
    new Set(params.sensorIds.map((id) => id.trim()).filter((id) => id.length > 0)),
  );
  if (cleanIds.length === 0) return [];

  const totalSensors = cleanIds.length;
  let processedSensors = 0;
  let completedRequests = 0;

  const reportProgress = () => {
    params.onProgress?.({
      processedSensors,
      totalSensors,
      completedRequests,
    });
  };

  reportProgress();

  const baseBatchSize = params.batchSize ?? 24;
  const batches = chunk(cleanIds, baseBatchSize);
  const output: TrendSeriesEntry[] = [];

  for (const ids of batches) {
    if (params.signal?.aborted) {
      throw new DOMException("Aborted", "AbortError");
    }
    const series = await fetchMetricsSeries(ids, params.start, params.end, params.interval, {
      signal: params.signal,
    });
    output.push(...series);
    processedSensors += ids.length;
    completedRequests += 1;
    reportProgress();
  }

  output.sort((a, b) => a.sensor_id.localeCompare(b.sensor_id));
  return output;
}
