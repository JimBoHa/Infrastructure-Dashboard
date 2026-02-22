"use client";

import type { OfflineMapPack } from "@/types/map";

export type OfflinePackProgressSummary = {
  pct: number;
  downloaded: number;
  total: number;
  failed: number;
};

const asFiniteNumber = (value: unknown): number | null => {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim().length) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
};

export function summarizeOfflinePackProgress(pack: OfflineMapPack | null): OfflinePackProgressSummary | null {
  if (!pack) return null;
  const progress = pack.progress as Record<string, unknown> | null | undefined;
  const layers = progress?.layers;
  if (!layers || typeof layers !== "object") return null;

  const entries = Object.entries(layers as Record<string, unknown>);
  if (!entries.length) return null;

  let total = 0;
  let downloaded = 0;
  let failed = 0;

  for (const [, layer] of entries) {
    if (!layer || typeof layer !== "object") continue;
    const layerProgress = layer as Record<string, unknown>;
    const layerTotal = asFiniteNumber(layerProgress.total);
    const layerDownloaded = asFiniteNumber(layerProgress.downloaded);
    const layerFailed = asFiniteNumber(layerProgress.failed);

    if (layerTotal != null) total += layerTotal;
    if (layerDownloaded != null) downloaded += layerDownloaded;
    if (layerFailed != null) failed += layerFailed;
  }

  if (!total) return null;
  const pct = Math.min(100, Math.max(0, (downloaded / total) * 100));
  return { pct, downloaded, total, failed };
}

