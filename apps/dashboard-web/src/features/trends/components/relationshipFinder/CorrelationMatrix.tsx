"use client";

import { useMemo, useRef } from "react";
import type { ReactNode } from "react";
import type { Options as HighchartsOptions, PointOptionsObject } from "highcharts";
import { HighchartsPanel, type HighchartsChartRef } from "@/components/charts/HighchartsPanel";
import { createHeatmapOptions } from "@/lib/chartFactories";
import { formatNumber } from "@/lib/format";
import { Card } from "@/components/ui/card";
import type { CorrelationMatrixResultV1 } from "@/types/analysis";

function shortLabel(label: string): string {
  const trimmed = label.trim();
  if (!trimmed) return trimmed;
  const parts = trimmed.split(" — ");
  const tail = parts.length > 1 ? parts.slice(1).join(" — ") : trimmed;
  return tail.replace(/\s*\([^)]*\)\s*$/, "").trim() || trimmed;
}

function formatLagSeconds(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "0";
  if (seconds === 0) return "0";
  const sign = seconds < 0 ? "-" : "+";
  const absSeconds = Math.abs(seconds);
  if (absSeconds >= 3600) {
    return `${sign}${formatNumber(absSeconds / 3600, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}h`;
  }
  if (absSeconds >= 60) {
    return `${sign}${formatNumber(absSeconds / 60, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}m`;
  }
  return `${sign}${absSeconds}s`;
}

type CorrelationMatrixProps = {
  result: CorrelationMatrixResultV1;
  labelMap: Map<string, string>;
  focusSensorId?: string | null;
  onSelectPair?: (rowSensorId: string, colSensorId: string) => void;
  title?: ReactNode;
  description?: ReactNode;
  emptyMessage?: string;
  showHeader?: boolean;
};

export default function CorrelationMatrix({
  result,
  labelMap,
  focusSensorId,
  onSelectPair,
  title,
  description,
  emptyMessage,
  showHeader = true,
}: CorrelationMatrixProps) {
  const chartRef = useRef<HighchartsChartRef | null>(null);

  const { sensor_ids: sensorIds, matrix, sensors } = result;

  const labels = useMemo(() => {
    const sensorMap = new Map(
      (sensors ?? []).map((s) => [s.sensor_id, s.name ?? s.sensor_id]),
    );
    return sensorIds.map((id) => {
      const label = labelMap.get(id) ?? sensorMap.get(id) ?? id;
      return shortLabel(label);
    });
  }, [sensorIds, sensors, labelMap]);

  const chartOptions = useMemo<HighchartsOptions | null>(() => {
    if (!matrix.length || sensorIds.length < 2) return null;

    const data: PointOptionsObject[] = [];
    for (let row = 0; row < matrix.length; row++) {
      for (let col = 0; col < (matrix[row]?.length ?? 0); col++) {
        const cell = matrix[row]![col]!;
        const r = cell.r;
        if (r == null) continue;
        data.push({ x: col, y: row, value: r });
      }
    }

    // Compute height based on matrix size — square aspect ratio
    const cellSize = 40;
    const marginBottom = 100;
    const size = sensorIds.length;
    const chartHeight = Math.max(300, size * cellSize + marginBottom + 40);

    return createHeatmapOptions({
      xCategories: labels,
      yCategories: labels,
      colorAxisMin: -1,
      colorAxisMax: 1,
      height: chartHeight,
      series: [
        {
          type: "heatmap",
          name: "Correlation",
          data,
          borderWidth: 1,
          borderColor: "#ffffff",
          dataLabels: {
            enabled: size <= 15,
            format: "{point.value:.2f}",
            style: {
              fontSize: "10px",
              fontWeight: "normal",
              textOutline: "none",
            },
          },
          cursor: onSelectPair ? "pointer" : undefined,
          point: onSelectPair
            ? {
                events: {
                  click: function () {
                    const rowIdx = (this as unknown as { y: number }).y;
                    const colIdx = (this as unknown as { x: number }).x;
                    if (rowIdx !== colIdx) {
                      onSelectPair(sensorIds[rowIdx]!, sensorIds[colIdx]!);
                    }
                  },
                },
              }
            : undefined,
        },
      ],
      tooltip: {
        formatter: function () {
          // eslint-disable-next-line @typescript-eslint/no-explicit-any
          const point = (this as any).point as {
            x: number;
            y: number;
            value: number;
          };
          const rowLabel = labels[point.y] ?? "—";
          const colLabel = labels[point.x] ?? "—";
          const r = formatNumber(point.value, {
            minimumFractionDigits: 3,
            maximumFractionDigits: 3,
          });
          const cell = matrix[point.y]?.[point.x];
          const n = cell?.n ?? "—";
          const p = cell?.p_value != null
            ? cell.p_value < 0.001
              ? "<0.001"
              : formatNumber(cell.p_value, { maximumFractionDigits: 3 })
            : "—";
          const q = cell?.q_value != null
            ? cell.q_value < 0.001
              ? "<0.001"
              : formatNumber(cell.q_value, { maximumFractionDigits: 3 })
            : "—";
          const nEff = cell?.n_eff ?? "—";
          const lag = cell?.lag_sec != null && Number.isFinite(cell.lag_sec)
            ? cell.lag_sec
            : null;
          const lagLabel = lag != null ? formatLagSeconds(lag) : null;
          const status = (cell?.status ?? "not_computed").replaceAll("_", " ");
          const method = (result.params.method ?? "pearson").toUpperCase();
          const lagLine = lagLabel ? `<br/>lag = ${lagLabel}` : "";
          return `<b>${rowLabel}</b> × <b>${colLabel}</b><br/>r (${method}) = ${r}${lagLine}<br/>n = ${n}, n_eff = ${nEff}<br/>p = ${p}, q = ${q}<br/>status = ${status}`;
        },
      },
    });

    // Override colorAxis for diverging palette (blue-white-red)
  }, [matrix, sensorIds, labels, onSelectPair, result.params.method]);

  // Patch colorAxis after factory creates the options
  const finalOptions = useMemo<HighchartsOptions | null>(() => {
    if (!chartOptions) return null;
    return {
      ...chartOptions,
      chart: {
        ...chartOptions.chart,
        marginLeft: 120,
        marginBottom: 100,
      },
      xAxis: {
        ...((chartOptions.xAxis ?? {}) as Record<string, unknown>),
        labels: {
          rotation: -45,
          style: { fontSize: "10px" },
        },
      },
      colorAxis: {
        min: -1,
        max: 1,
        stops: [
          [0, "#2563eb"],     // blue-600 (strong negative)
          [0.25, "#93c5fd"],  // blue-300
          [0.5, "#f5f5f5"],   // neutral (zero)
          [0.75, "#fca5a5"],  // red-300
          [1, "#dc2626"],     // red-600 (strong positive)
        ],
      },
      legend: {
        align: "right",
        layout: "vertical",
        margin: 0,
        verticalAlign: "middle",
        symbolHeight: 200,
      },
    };
  }, [chartOptions]);

  if (!finalOptions) {
    return (
      <Card className="rounded-lg gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
        {emptyMessage ?? "Correlation matrix requires at least 2 sensors."}
      </Card>
    );
  }

  const heading = title ?? "Correlation Matrix";
  const body =
    description ??
    (
      <>
        Click a cell to inspect that sensor pair. Blue = negative correlation, Red = positive.
        Tooltips show <code>p</code> (per-test) and <code>q</code> (FDR-adjusted), plus{" "}
        <code>n_eff</code> and optional <code>lag</code>.
        {focusSensorId && (
          <> Focus sensor highlighted.</>
        )}
      </>
    );

  if (!showHeader) {
    return (
      <div className="overflow-x-auto">
        <HighchartsPanel chartRef={chartRef} options={finalOptions} />
      </div>
    );
  }

  return (
    <div className="space-y-2">
 <h4 className="text-sm font-semibold text-foreground">
        {heading}
      </h4>
      <p className="text-xs text-muted-foreground">
        {body}
      </p>
      <div className="overflow-x-auto">
        <HighchartsPanel chartRef={chartRef} options={finalOptions} />
      </div>
    </div>
  );
}
