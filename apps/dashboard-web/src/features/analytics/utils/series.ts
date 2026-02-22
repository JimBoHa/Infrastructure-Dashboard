import type { AnalyticsHistoryRangeHours } from "@/features/analytics/components/AnalyticsShared";

export function filterSeriesByHours<T extends { timestamp: Date }>(
  series: T[],
  hours: AnalyticsHistoryRangeHours,
): T[] {
  if (series.length === 0) return series;

  const valid = series.filter((point) => {
    if (!(point.timestamp instanceof Date)) return false;
    const ms = point.timestamp.getTime();
    return Number.isFinite(ms);
  });
  if (valid.length === 0) return [];

  const sorted = [...valid].sort((a, b) => a.timestamp.getTime() - b.timestamp.getTime());
  if (hours >= 168) return sorted;

  const endMs = sorted[sorted.length - 1]!.timestamp.getTime();
  const minMs = endMs - hours * 60 * 60 * 1000;
  return sorted.filter((point) => point.timestamp.getTime() >= minMs);
}

