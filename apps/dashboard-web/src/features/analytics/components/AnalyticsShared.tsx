"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import type { Options, SeriesLineOptions, SeriesColumnOptions } from "highcharts";
import { HighchartsPanel, type HighchartsChartRef } from "@/components/charts/HighchartsPanel";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import {
  formatChartTickDate,
  formatChartTickTime,
  formatChartTickTimeOfDay,
  formatChartTooltipTime,
  useControllerTimeZone,
} from "@/lib/siteTime";
import { createTimeSeriesOptions, createHistogramOptions } from "@/lib/chartFactories";
import { cn } from "@/lib/utils";

export type ChartSeriesConfig = {
  label: string;
  series: Array<{ timestamp: Date; value: number | null }>;
  color: string;
};

export type AnalyticsHistoryRangeHours = 24 | 72 | 168;

const ANALYTICS_HISTORY_RANGE_OPTIONS: Array<{ value: AnalyticsHistoryRangeHours; label: string }> = [
  { value: 24, label: "24h" },
  { value: 72, label: "72h" },
  { value: 168, label: "7d" },
];

function useIsCoarsePointer() {
  const [isCoarsePointer, setIsCoarsePointer] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(pointer: coarse)");

    const sync = () => setIsCoarsePointer(mq.matches);
    sync();

    mq.addEventListener?.("change", sync);
    return () => mq.removeEventListener?.("change", sync);
  }, []);

  return isCoarsePointer;
}

export function historyRangeLabel(hours: AnalyticsHistoryRangeHours): string {
  if (hours === 24) return "Past 24 hours";
  if (hours === 72) return "Past 72 hours";
  return "Past 7 days";
}

export function historyIntervalSeconds(hours: AnalyticsHistoryRangeHours): number {
  if (hours <= 24) return 300; // 5 min
  if (hours <= 72) return 900; // 15 min
  return 3600; // 1 hour
}

export function AnalyticsRangeSelect({
  value,
  onChange,
}: {
  value: AnalyticsHistoryRangeHours;
  onChange: (next: AnalyticsHistoryRangeHours) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-xs font-semibold text-muted-foreground">Range</span>
      <div className="inline-flex overflow-hidden rounded-lg border border-border bg-card shadow-xs">
        {ANALYTICS_HISTORY_RANGE_OPTIONS.map((opt) => {
          const active = opt.value === value;
          return (
            <Button
              key={opt.value}
              type="button"
              size="xs"
              variant={active ? "primary" : "ghost"}
              onClick={() => onChange(opt.value)}
              className={cn(
                "rounded-none border-l border-border shadow-none first:border-l-0",
                "focus-visible:ring-2 focus-visible:ring-indigo-500/30 focus-visible:ring-inset",
                !active && "text-muted-foreground hover:text-foreground",
              )}
            >
              {opt.label}
            </Button>
          );
        })}
      </div>
    </div>
  );
}

export function useBaseChartOptions(rangeHours?: AnalyticsHistoryRangeHours) {
  const timeZone = useControllerTimeZone();
  return useMemo(() => {
    const formatTick = (value: unknown) => {
      if (rangeHours === 24) return formatChartTickTimeOfDay(value, timeZone);
      if (rangeHours === 168) return formatChartTickDate(value, timeZone);
      return formatChartTickTime(value, timeZone);
    };
    const formatTooltip = (x: unknown) => formatChartTooltipTime(x, timeZone);

    // Return a Chart.js-compatible options object for backwards compatibility
    // These are used by ZoomableLineChart/ZoomableBarChart internally
    return {
      timeZone,
      formatTick,
      formatTooltip,
      scales: {
        x: {
          type: "time" as const,
          time: { tooltipFormat: "MMM d, HH:mm" },
          ticks: {
            callback: formatTick,
            maxTicksLimit: 8,
          },
        },
        y: {
          type: "linear" as const,
          ticks: { maxTicksLimit: 6 },
        },
      },
      plugins: {
        legend: { display: false },
        tooltip: {
          callbacks: {
            title: (items: Array<{ parsed?: { x?: unknown } }> | undefined) => {
              const first = items?.[0];
              const x = first?.parsed?.x;
              return formatTooltip(x);
            },
          },
        },
      },
      interaction: { mode: "nearest" as const, intersect: false },
      maintainAspectRatio: false,
      responsive: true,
    };
  }, [rangeHours, timeZone]);
}

type ZoomableChartProps = {
  wrapperClassName?: string;
  data: {
    datasets: Array<{
      label?: string;
      data: Array<{ x: Date | string | number; y: number | null | [number, number] } | [number | null, number | null]>;
      borderColor?: string;
      backgroundColor?: string;
      borderWidth?: number;
      pointRadius?: number;
      pointHoverRadius?: number;
      tension?: number;
      fill?: boolean | string;
      borderDash?: number[];
      yAxisID?: string;
      borderRadius?: number;
      type?: string;
    }>;
  };
  options?: Partial<Options> & {
    scales?: {
      x?: {
        type?: string;
        min?: number;
        max?: number;
        ticks?: {
          callback?: (value: unknown) => string;
          maxTicksLimit?: number;
          maxRotation?: number;
          minRotation?: number;
          autoSkip?: boolean;
          autoSkipPadding?: number;
          source?: string;
        };
      };
      y?: {
        type?: string;
        beginAtZero?: boolean;
        min?: number;
        max?: number;
        position?: string;
        display?: boolean;
        title?: { display?: boolean; text?: string };
        grid?: { drawOnChartArea?: boolean };
      };
      y1?: {
        type?: string;
        position?: string;
        grid?: { drawOnChartArea?: boolean };
        display?: boolean;
        min?: number;
        max?: number;
        title?: { display?: boolean; text?: string };
        beginAtZero?: boolean;
      };
    };
    plugins?: {
      legend?: { display?: boolean; position?: string };
      tooltip?: {
        callbacks?: {
          title?: (items: Array<{ parsed?: { x?: unknown } }> | undefined) => string;
          label?: (context: unknown) => string;
        };
      };
    };
    interaction?: { mode?: string; intersect?: boolean };
    maintainAspectRatio?: boolean;
    responsive?: boolean;
    animation?: { duration?: number };
  };
};

function convertToHighchartsLineSeries(
  datasets: ZoomableChartProps["data"]["datasets"],
  _options?: ZoomableChartProps["options"],
): SeriesLineOptions[] {
  return datasets.map((ds) => {
    const data = ds.data.map((point) => {
      if (Array.isArray(point)) {
        return point as [number | null, number | null];
      }
      const x = point.x instanceof Date ? point.x.getTime() : typeof point.x === "string" ? new Date(point.x).getTime() : point.x;
      return [x, point.y] as [number, number | null];
    });

    return {
      type: "line" as const,
      name: ds.label ?? "",
      data,
      color: ds.borderColor,
      lineWidth: ds.borderWidth ?? 1.5,
      dashStyle: ds.borderDash?.length ? "Dash" : "Solid",
      marker: {
        enabled: false,
        radius: ds.pointRadius ?? 0,
        states: {
          hover: {
            radius: ds.pointHoverRadius ?? 3,
          },
        },
      },
      fillColor: ds.fill ? ds.backgroundColor : undefined,
      fillOpacity: ds.fill ? 0.3 : 0,
      yAxis: ds.yAxisID === "y1" ? 1 : 0,
    } as SeriesLineOptions;
  });
}

function convertToHighchartsColumnSeries(
  datasets: ZoomableChartProps["data"]["datasets"],
  labels?: string[],
): SeriesColumnOptions[] {
  return datasets.map((ds) => {
    const data = ds.data.map((point) => {
      if (Array.isArray(point)) {
        // Range bar chart [low, high]
        return { y: point[1] ?? 0, low: point[0] ?? 0, high: point[1] ?? 0 };
      }
      const x = point.x instanceof Date ? point.x.getTime() : typeof point.x === "string" ? new Date(point.x).getTime() : point.x;
      // Check if y is an array (range bar)
      const yVal = point.y;
      if (Array.isArray(yVal)) {
        return { x, low: yVal[0], high: yVal[1] };
      }
      return labels ? yVal : [x, yVal];
    });

    return {
      type: "column" as const,
      name: ds.label ?? "",
      data: data as Array<number | [number, number] | { x?: number; y?: number; low?: number; high?: number }>,
      color: ds.backgroundColor ?? ds.borderColor,
      borderColor: ds.borderColor,
      borderWidth: ds.borderWidth ?? 1,
      borderRadius: ds.borderRadius ?? 0,
    } as SeriesColumnOptions;
  });
}

export function ZoomableLineChart({
  wrapperClassName,
  data,
  options,
}: ZoomableChartProps) {
  const chartRef = useRef<HighchartsChartRef | null>(null);
  const isCoarsePointer = useIsCoarsePointer();
  const baseOptions = useBaseChartOptions();

  const highchartsOptions: Options = useMemo(() => {
    const series = convertToHighchartsLineSeries(data.datasets, options);

    const yAxes: Options["yAxis"] = [
      {
        title: options?.scales?.y?.title?.display ? { text: options.scales.y.title.text } : { text: undefined },
        min: options?.scales?.y?.min,
        max: options?.scales?.y?.max,
        startOnTick: options?.scales?.y?.beginAtZero !== false,
        endOnTick: true,
        opposite: options?.scales?.y?.position === "right",
        visible: options?.scales?.y?.display !== false,
        gridLineWidth: 1,
      },
    ];

    if (options?.scales?.y1) {
      yAxes.push({
        title: options.scales.y1.title?.display ? { text: options.scales.y1.title.text } : { text: undefined },
        min: options.scales.y1.min,
        max: options.scales.y1.max,
        startOnTick: options.scales.y1.beginAtZero !== false,
        endOnTick: true,
        opposite: options.scales.y1.position === "right",
        visible: options.scales.y1.display !== false,
        gridLineWidth: 0,
      });
    }

    const base = createTimeSeriesOptions({
      series,
      timeZone: baseOptions.timeZone,
      navigator: false,
      zoom: !isCoarsePointer,
      yAxis: yAxes,
    });

    // Override with adapter-specific customizations
    return {
      ...base,
      chart: {
        ...base.chart,
        animation: { duration: options?.animation?.duration ?? 0 },
      },
      xAxis: {
        ...(base.xAxis as Record<string, unknown>),
        min: options?.scales?.x?.min,
        max: options?.scales?.x?.max,
        labels: {
          formatter: function () {
            if (options?.scales?.x?.ticks?.callback) {
              return options.scales.x.ticks.callback(this.value);
            }
            return baseOptions.formatTick(this.value);
          },
        },
      },
      tooltip: {
        shared: true,
        formatter: function () {
          const x = this.x as number;
          let header: string;
          if (options?.plugins?.tooltip?.callbacks?.title) {
            header = options.plugins.tooltip.callbacks.title([{ parsed: { x } }]);
          } else {
            header = baseOptions.formatTooltip(x);
          }
          let html = `<b>${header}</b><br/>`;
          this.points?.forEach((point) => {
            const y = point.y;
            const value = typeof y === "number" && Number.isFinite(y) ? y.toFixed(2) : "—";
            html += `<span style="color:${point.color}">\u25CF</span> ${point.series.name}: <b>${value}</b><br/>`;
          });
          return html;
        },
      },
      legend: {
        enabled: options?.plugins?.legend?.display !== false,
        align: "center",
        verticalAlign: "bottom",
      },
    };
  }, [baseOptions, data.datasets, isCoarsePointer, options]);

  return (
    <HighchartsPanel
      chartRef={chartRef}
      options={highchartsOptions}
      wrapperClassName={wrapperClassName}
      resetZoomOnDoubleClick
    />
  );
}

export function ZoomableBarChart({
  wrapperClassName,
  data,
  options,
}: ZoomableChartProps & { data: { labels?: string[]; datasets: ZoomableChartProps["data"]["datasets"] } }) {
  const chartRef = useRef<HighchartsChartRef | null>(null);
  const baseOptions = useBaseChartOptions();

  const highchartsOptions: Options = useMemo(() => {
    const hasLabels = Boolean(data.labels?.length);
    const series = convertToHighchartsColumnSeries(data.datasets, data.labels);

    // Check if this is a range bar chart (data points have [low, high] arrays)
    const isRangeBar = data.datasets.some((ds) =>
      ds.data.some((pt) => {
        if (!Array.isArray(pt) && typeof pt === "object" && "y" in pt) {
          return Array.isArray((pt as { y?: unknown }).y);
        }
        return false;
      }),
    );

    const base = createHistogramOptions({
      series,
      timeZone: hasLabels ? undefined : baseOptions.timeZone,
      xType: hasLabels ? "category" : "datetime",
      yAxisTitle: options?.scales?.y?.title?.display ? options.scales.y.title.text : undefined,
    });

    // Override with adapter-specific customizations
    return {
      ...base,
      chart: {
        ...base.chart,
        type: isRangeBar ? "columnrange" : "column",
      },
      xAxis: hasLabels
        ? {
            categories: data.labels,
            labels: {
              rotation: options?.scales?.x?.ticks?.maxRotation ?? 0,
              style: { fontSize: "11px" },
            },
          }
        : {
            type: "datetime" as const,
            min: options?.scales?.x?.min,
            max: options?.scales?.x?.max,
            labels: {
              formatter: function () {
                if (options?.scales?.x?.ticks?.callback) {
                  return options.scales.x.ticks.callback(this.value);
                }
                return baseOptions.formatTick(this.value);
              },
            },
          },
      yAxis: {
        title: options?.scales?.y?.title?.display ? { text: options.scales.y.title.text } : { text: undefined },
        min: options?.scales?.y?.min,
        max: options?.scales?.y?.max,
        startOnTick: options?.scales?.y?.beginAtZero !== false,
      },
      tooltip: {
        shared: true,
        formatter: function () {
          const x = this.x;
          let header: string;
          if (typeof x === "string") {
            header = x;
          } else if (options?.plugins?.tooltip?.callbacks?.title) {
            header = options.plugins.tooltip.callbacks.title([{ parsed: { x } }]);
          } else {
            header = baseOptions.formatTooltip(x);
          }

          let html = `<b>${header}</b><br/>`;
          this.points?.forEach((pt) => {
            const y = pt.y;
            // For range bars, show low-high (cast to access columnrange properties)
            const pointData = pt as unknown as { low?: number; high?: number; color?: string; series: { name: string } };
            if (pointData.low != null && pointData.high != null) {
              html += `<span style="color:${pt.color}">\u25CF</span> ${pt.series.name}: <b>${pointData.low.toFixed(1)} – ${pointData.high.toFixed(1)}</b><br/>`;
            } else {
              const value = typeof y === "number" && Number.isFinite(y) ? y.toFixed(2) : "—";
              html += `<span style="color:${pt.color}">\u25CF</span> ${pt.series.name}: <b>${value}</b><br/>`;
            }
          });
          return html;
        },
      },
      legend: {
        enabled: options?.plugins?.legend?.display !== false,
        align: "center",
        verticalAlign: "bottom",
      },
    };
  }, [baseOptions, data.datasets, data.labels, options]);

  return (
    <HighchartsPanel
      chartRef={chartRef}
      options={highchartsOptions}
      wrapperClassName={wrapperClassName}
      resetZoomOnDoubleClick
    />
  );
}

export function buildChartData(series: ChartSeriesConfig[]) {
  return {
    datasets: series.map((item) => ({
      label: item.label,
      data: item.series.map((point) => ({ x: point.timestamp, y: point.value })),
      borderColor: item.color,
      backgroundColor: item.color,
      borderWidth: 1.5,
      pointRadius: 0,
      pointHoverRadius: 3,
      tension: 0,
      fill: false,
    })),
  };
}

export function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground leading-tight">
        {label}
      </dt>
 <dd className="mt-0.5 font-medium text-foreground tabular-nums leading-snug">
        {value}
      </dd>
    </div>
  );
}

export function AnalyticsChart({
  title,
  series,
  unit,
  rangeHours,
}: {
  title: string;
  series: ChartSeriesConfig[];
  unit?: string;
  rangeHours?: AnalyticsHistoryRangeHours;
}) {
  const baseChartOptions = useBaseChartOptions(rangeHours);
  const data = buildChartData(series);
  const hasData = series.some((entry) => entry.series.some((point) => point.value != null));

  const options: ZoomableChartProps["options"] = {
    scales: {
      x: {
        type: "time",
        ticks: {
          callback: baseChartOptions.formatTick,
          maxTicksLimit: 7,
        },
      },
      y: {
        type: "linear",
        beginAtZero: true,
        title: unit ? { display: true, text: unit } : undefined,
      },
    },
    plugins: {
      legend: { display: series.length > 1, position: "bottom" },
    },
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-sm">{title}</CardTitle>
      </CardHeader>
      <CardContent>
        {hasData ? (
          <ZoomableLineChart wrapperClassName="h-[400px]" data={data} options={options} />
        ) : (
 <p className="text-sm text-muted-foreground">
            No data available for the selected range.
          </p>
        )}
      </CardContent>
    </Card>
  );
}
