import { formatNumber } from "@/lib/format";
import { formatChartTooltipTime } from "@/lib/siteTime";

/**
 * Ordered palette — index 0-9 for series, named keys for semantic use.
 * Every chart in the dashboard pulls colors from this single source.
 */
export const CHART_PALETTE = {
  series: [
    "#2563eb", // blue-600
    "#16a34a", // green-600
    "#f97316", // orange-500
    "#a855f7", // purple-500
    "#0ea5e9", // sky-500
    "#ec4899", // pink-500
    "#14b8a6", // teal-500
    "#facc15", // yellow-400
    "#dc2626", // red-600
    "#475569", // slate-600
  ],
  // Semantic colors
  solar: "#f59e0b",
  grid: "#6366f1",
  battery: "#10b981",
  anomaly: "#e11d48",
  regression: "#6366f1",
  reference: "#94a3b8",
  band: {
    good: "#10b981",
    warn: "#f59e0b",
    bad: "#dc2626",
  },
} as const;

/** Standard chart heights (px) */
export const CHART_HEIGHTS = {
  sparkline: 48,
  compact: 200,
  standard: 320,
  tall: 480,
} as const;

/** Return the palette color at a given series index (cycles) */
export function seriesColor(index: number): string {
  return CHART_PALETTE.series[index % CHART_PALETTE.series.length]!;
}

/**
 * Shared tooltip formatter for time-series charts.
 * Produces a consistent HTML tooltip across all chart types.
 */
export function formatTooltipHtml(
  points: ReadonlyArray<{
    x: number;
    y: number | null | undefined;
    color: string;
    seriesName: string;
    unit?: string;
    decimals?: number;
  }>,
  timeZone: string,
): string {
  if (!points.length) return "";
  const header = formatChartTooltipTime(points[0]!.x, timeZone);
  let html = `<b>${header}</b><br/>`;

  for (const point of points) {
    const rawY = point.y;
    const value =
      typeof rawY === "number" && Number.isFinite(rawY)
        ? point.decimals != null
          ? formatNumber(rawY, {
              minimumFractionDigits: point.decimals,
              maximumFractionDigits: point.decimals,
            })
          : formatNumber(rawY, { minimumFractionDigits: 0, maximumFractionDigits: 2 })
        : "\u2014";
    const suffix = point.unit && !point.seriesName.includes(point.unit) ? ` ${point.unit}` : "";
    html += `<span style="color:${point.color}">\u25CF</span> ${point.seriesName}: <b>${value}${suffix}</b><br/>`;
  }

  return html;
}
