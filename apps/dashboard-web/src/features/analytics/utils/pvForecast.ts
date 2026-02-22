import { addDaysInTimeZone, startOfDayInTimeZone } from "@/lib/siteTime";
import type { ForecastSeriesPoint } from "@/types/forecast";
import type { TrendSeriesPoint } from "@/types/dashboard";
import type { AnalyticsHistoryRangeHours } from "@/features/analytics/components/AnalyticsShared";

export type PvWindowedSeries = {
  start: Date;
  end: Date;
  startMs: number;
  endMs: number;
  measuredWindowPoints: TrendSeriesPoint[];
  forecastWindowPoints: ForecastSeriesPoint[];
};

export type BuildPvWindowedSeriesInput = {
  rangeHours: AnalyticsHistoryRangeHours;
  timeZone: string;
  measuredPoints: TrendSeriesPoint[];
  forecastPoints: ForecastSeriesPoint[];
  now?: Date;
};

export function buildPvWindowedSeries({
  rangeHours,
  timeZone,
  measuredPoints,
  forecastPoints,
  now = new Date(),
}: BuildPvWindowedSeriesInput): PvWindowedSeries {
  const todayStart = startOfDayInTimeZone(now, timeZone);
  const end = addDaysInTimeZone(todayStart, 1, timeZone);
  const daysBack = rangeHours === 24 ? 0 : rangeHours === 72 ? 2 : 6;
  const start = addDaysInTimeZone(todayStart, -daysBack, timeZone);
  const startMs = start.getTime();
  const endMs = end.getTime();

  const measuredWindowPoints = measuredPoints.filter((point) => {
    const ms = point.timestamp.getTime();
    return ms >= startMs && ms <= endMs;
  });

  const forecastWindowPoints = forecastPoints.filter((point) => {
    const ms = new Date(point.timestamp).getTime();
    return ms >= startMs && ms <= endMs;
  });

  return {
    start,
    end,
    startMs,
    endMs,
    measuredWindowPoints,
    forecastWindowPoints,
  };
}
