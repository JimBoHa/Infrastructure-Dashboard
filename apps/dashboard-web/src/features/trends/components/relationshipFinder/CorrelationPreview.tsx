"use client";

import { useMemo, useRef, useState } from "react";
import type { Chart as HighchartsChart, Options as HighchartsOptions } from "highcharts";
import { HighchartsPanel, type HighchartsChartRef } from "@/components/charts/HighchartsPanel";
import NodePill from "@/features/nodes/components/NodePill";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { TrendChart } from "@/components/TrendChart";
import { formatNumber } from "@/lib/format";
import { CHART_PALETTE, CHART_HEIGHTS } from "@/lib/chartTokens";
import SegmentedControl from "@/components/SegmentedControl";
import { createScatterOptions, createBaseOptions } from "@/lib/chartFactories";
import type { TrendSeriesEntry } from "@/types/dashboard";
import type { CorrelationMatrixCellV1 } from "@/types/analysis";
import {
  type CorrelationMethod,
  alignSeriesPair,
  computeLagCorrelationSeries,
  linearRegression,
  rollingPearsonCorrelation,
} from "../../utils/correlation";

const CORR_NUMBER_OPTIONS: Intl.NumberFormatOptions = {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
};

/** Resize chart height so the plot area is a perfect square. */
function enforceSquarePlot(this: HighchartsChart) {
  // marginBottom is a runtime property not in the public type defs.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const mb = (this as any).marginBottom as number ?? 80;
  const targetHeight = this.plotWidth + this.plotTop + mb;
  if (Math.abs(this.chartHeight - targetHeight) > 2) {
    this.setSize(undefined, targetHeight, false);
  }
}

function shortLabel(label: string): string {
  const trimmed = label.trim();
  if (!trimmed) return trimmed;
  const parts = trimmed.split(" — ");
  const tail = parts.length > 1 ? parts.slice(1).join(" — ") : trimmed;
  return tail.replace(/\s*\([^)]*\)\s*$/, "").trim() || trimmed;
}

function formatLagLabel(lagBuckets: number, intervalSeconds: number): string {
  const seconds = lagBuckets * intervalSeconds;
  const absSeconds = Math.abs(seconds);
  const sign = seconds < 0 ? "-" : seconds > 0 ? "+" : "";
  if (absSeconds >= 3600) {
    const hours = absSeconds / 3600;
    return `${sign}${formatNumber(hours, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}h`;
  }
  if (absSeconds >= 60) {
    const minutes = absSeconds / 60;
    return `${sign}${formatNumber(minutes, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}m`;
  }
  return `${sign}${absSeconds}s`;
}

type CorrelationPreviewProps = {
  focusSeries: TrendSeriesEntry | null;
  candidateSeries: TrendSeriesEntry | null;
  cell: CorrelationMatrixCellV1 | null;
  method: CorrelationMethod;
  intervalSeconds: number;
  maxLagBuckets?: number;
  rollingWindowBuckets?: number;
  focusColor?: string;
  candidateColor?: string;
  timeZone?: string;
};

export default function CorrelationPreview({
  focusSeries,
  candidateSeries,
  cell,
  method,
  intervalSeconds,
  maxLagBuckets = 24,
  rollingWindowBuckets = 48,
  focusColor = CHART_PALETTE.regression,
  candidateColor = CHART_PALETTE.series[0]!,
  timeZone,
}: CorrelationPreviewProps) {
  const scatterChartRef = useRef<HighchartsChartRef | null>(null);
  const lagChartRef = useRef<HighchartsChartRef | null>(null);
  const [pairTab, setPairTab] = useState<"scatter" | "lag" | "rolling">("scatter");

  // Aligned pair data
  const alignedPair = useMemo(() => {
    if (!focusSeries || !candidateSeries) return null;
    return alignSeriesPair(focusSeries, candidateSeries, 0);
  }, [focusSeries, candidateSeries]);

  // Regression line
  const regression = useMemo(() => {
    if (!alignedPair || alignedPair.x.length < 3) return null;
    return linearRegression(alignedPair.x, alignedPair.y);
  }, [alignedPair]);

  // Lag correlation series
  const lagCorrelation = useMemo(() => {
    if (!focusSeries || !candidateSeries || !intervalSeconds) return null;
    return computeLagCorrelationSeries({
      a: focusSeries,
      b: candidateSeries,
      method,
      intervalSeconds,
      maxLagBuckets,
    });
  }, [focusSeries, candidateSeries, method, intervalSeconds, maxLagBuckets]);

  // Rolling correlation
  const rollingCorrelation = useMemo(() => {
    if (!alignedPair || alignedPair.x.length < rollingWindowBuckets) return null;
    const points = rollingPearsonCorrelation(alignedPair, rollingWindowBuckets);
    if (points.length === 0) return null;
    return {
      sensor_id: "rolling_correlation",
      label: `Rolling r (${rollingWindowBuckets} buckets)`,
      unit: undefined,
      points: points.map((p) => ({
        timestamp: p.timestamp,
        value: p.value,
        samples: 1,
      })),
    } as TrendSeriesEntry;
  }, [alignedPair, rollingWindowBuckets]);

  // Labels
  const focusLabel = focusSeries ? shortLabel(focusSeries.label ?? focusSeries.sensor_id) : "Focus";
  const candidateLabel = candidateSeries
    ? shortLabel(candidateSeries.label ?? candidateSeries.sensor_id)
    : "Candidate";

  // Scatter plot options (Highcharts)
  const scatterOptions = useMemo<HighchartsOptions | null>(() => {
    if (!alignedPair || !focusSeries || !candidateSeries || alignedPair.x.length < 3) return null;

    // Limit points for readability
    const maxPoints = 500;
    const step = alignedPair.x.length > maxPoints ? Math.ceil(alignedPair.x.length / maxPoints) : 1;
    const points: Array<[number, number]> = [];
    for (let idx = 0; idx < alignedPair.x.length; idx += step) {
      points.push([alignedPair.x[idx]!, alignedPair.y[idx]!]);
    }

    // Calculate regression line endpoints
    let regressionData: Array<[number, number]> = [];
    if (regression) {
      let minX = Number.POSITIVE_INFINITY;
      let maxX = Number.NEGATIVE_INFINITY;
      alignedPair.x.forEach((value) => {
        if (Number.isFinite(value)) {
          if (value < minX) minX = value;
          if (value > maxX) maxX = value;
        }
      });

      if (Number.isFinite(minX) && Number.isFinite(maxX)) {
        regressionData = [
          [minX, regression.intercept + regression.slope * minX],
          [maxX, regression.intercept + regression.slope * maxX],
        ];
      }
    }

    const xAxisTitle = focusSeries.unit ? `${focusLabel} (${focusSeries.unit})` : focusLabel;
    const yAxisTitle = candidateSeries.unit ? `${candidateLabel} (${candidateSeries.unit})` : candidateLabel;

    const base = createScatterOptions({
      xAxisTitle,
      yAxisTitle,
      height: 400,
      series: [
        {
          type: "scatter",
          name: `${focusLabel} vs ${candidateLabel}`,
          data: points,
          color: candidateColor + "99",
          marker: {
            radius: 4,
            symbol: "circle",
            lineWidth: 1,
            lineColor: candidateColor,
          },
        },
        ...(regressionData.length > 0
          ? [
              {
                type: "line" as const,
                name: "Best fit",
                data: regressionData,
                color: focusColor,
                lineWidth: 2,
                marker: { enabled: false },
                enableMouseTracking: false,
              },
            ]
          : []),
      ],
    });

    return {
      ...base,
      chart: {
        ...base.chart,
        events: {
          ...(base.chart as Record<string, unknown>)?.events as Record<string, unknown>,
          load: enforceSquarePlot,
          redraw: enforceSquarePlot,
        },
      },
      tooltip: {
        formatter: function () {
          const xValue = this.x != null ? formatNumber(this.x as number, { maximumFractionDigits: 3 }) : "—";
          const yValue = this.y != null ? formatNumber(this.y as number, { maximumFractionDigits: 3 }) : "—";
          return `<b>${focusLabel}:</b> ${xValue}<br/><b>${candidateLabel}:</b> ${yValue}`;
        },
      },
    };
  }, [
    alignedPair,
    candidateColor,
    candidateLabel,
    candidateSeries,
    focusColor,
    focusLabel,
    focusSeries,
    regression,
  ]);

  // Lag correlation chart options (Highcharts)
  const lagChartOptions = useMemo<HighchartsOptions | null>(() => {
    if (!lagCorrelation) return null;

    const points: Array<[number, number]> = lagCorrelation.points
      .filter((point) => point.r != null && Number.isFinite(point.r))
      .map((point) => [point.lag_buckets, point.r as number]);

    return createBaseOptions({
      chart: {
        type: "line",
        zooming: { type: "xy" },
        height: CHART_HEIGHTS.compact,
      },
      title: { text: undefined },
      xAxis: {
        title: { text: "Lag (intervals)", style: { color: "#6b7280", fontSize: "11px" } },
        gridLineWidth: 1,
        gridLineColor: "#f3f4f6",
        labels: {
          formatter: function () {
            return formatLagLabel(this.value as number, intervalSeconds);
          },
        },
      },
      yAxis: {
        title: { text: "Correlation", style: { color: "#6b7280", fontSize: "11px" } },
        min: -1,
        max: 1,
        gridLineColor: "#f3f4f6",
      },
      legend: { enabled: false },
      navigator: { enabled: false },
      rangeSelector: { enabled: false },
      scrollbar: { enabled: false },
      tooltip: {
        formatter: function () {
          const lag = this.x as number;
          const value = this.y != null ? formatNumber(this.y as number, CORR_NUMBER_OPTIONS) : "—";
          return `<b>Lag ${formatLagLabel(lag, intervalSeconds)}</b> (${lag} buckets)<br/>r = ${value}`;
        },
      },
      plotOptions: {
        line: {
          marker: {
            enabled: true,
            radius: 2,
          },
        },
      },
      series: [
        {
          type: "line",
          name: "Correlation",
          data: points,
          color: focusColor,
          lineWidth: 2,
        },
      ],
    });
  }, [focusColor, intervalSeconds, lagCorrelation]);

  // If no data available
  if (!focusSeries || !candidateSeries) {
    return (
      <Card className="rounded-lg gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
        Select a sensor with data to view correlation analysis.
      </Card>
    );
  }

  const hasData = alignedPair && alignedPair.x.length >= 3;

  return (
    <div className="space-y-4">
      {/* Correlation summary badges */}
      {cell && (
        <div className="flex flex-wrap gap-2">
          {cell.r != null && (
            <NodePill
              tone={Math.abs(cell.r) >= 0.7 ? "success" : Math.abs(cell.r) >= 0.4 ? "warning" : "muted"}
              size="md"
              title={`Correlation coefficient (${method})`}
            >
              r = {formatNumber(cell.r, CORR_NUMBER_OPTIONS)}
            </NodePill>
          )}
          {cell.n != null && (
            <NodePill tone="muted" size="md" title="Number of overlapping data points">
              n = {cell.n}
            </NodePill>
          )}
          {cell.n_eff != null && (
            <NodePill
              tone="muted"
              size="md"
              title="Effective sample size after autocorrelation adjustment"
            >
              n_eff = {formatNumber(cell.n_eff, { maximumFractionDigits: 0 })}
            </NodePill>
          )}
          {cell.p_value != null && (
            <NodePill
              tone={cell.p_value < 0.05 ? "success" : "warning"}
              size="md"
              title="Per-test p-value (time-series adjusted)"
            >
              p = {cell.p_value < 0.001 ? "<0.001" : formatNumber(cell.p_value, { maximumFractionDigits: 3 })}
            </NodePill>
          )}
          {cell.q_value != null && (
            <NodePill
              tone={cell.q_value < 0.05 ? "success" : "warning"}
              size="md"
              title="FDR-adjusted q-value (Benjamini-Hochberg)"
            >
              q = {cell.q_value < 0.001 ? "<0.001" : formatNumber(cell.q_value, { maximumFractionDigits: 3 })}
            </NodePill>
          )}
          {regression?.r2 != null && (
            <NodePill tone="neutral" size="md" title="Coefficient of determination">
              R² = {formatNumber(regression.r2, CORR_NUMBER_OPTIONS)}
            </NodePill>
          )}
        </div>
      )}

      {/* Best lag indicator */}
      {lagCorrelation?.best && lagCorrelation.best.lag_buckets !== 0 && (
        <InlineBanner tone="warning" className="px-3 py-2 text-sm">
          <strong>Best lag:</strong> {formatLagLabel(lagCorrelation.best.lag_buckets, intervalSeconds)} (r ={" "}
          {formatNumber(lagCorrelation.best.r ?? 0, CORR_NUMBER_OPTIONS)})
        </InlineBanner>
      )}

      {/* Tab switcher */}
      <SegmentedControl
        value={pairTab}
        onChange={(next) => setPairTab(next as "scatter" | "lag" | "rolling")}
        options={[
          { value: "scatter", label: "Scatter" },
          { value: "lag", label: "Lag/Lead" },
          { value: "rolling", label: "Rolling" },
        ]}
        size="xs"
      />

      {/* Chart area */}
      <div className="min-h-[200px]">
        {pairTab === "scatter" && (
          <>
            {!hasData ? (
              <Card className="rounded-lg gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
                Insufficient overlapping data for scatter plot (need at least 3 points).
              </Card>
            ) : scatterOptions ? (
              <div>
                <HighchartsPanel chartRef={scatterChartRef} options={scatterOptions} resetZoomOnDoubleClick />
 <p className="mt-1 text-xs text-muted-foreground">
                  {alignedPair.x.length} aligned points · double-click to reset zoom
                </p>
              </div>
            ) : null}
          </>
        )}

        {pairTab === "lag" && (
          <>
            {!lagChartOptions ? (
              <Card className="rounded-lg gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
                Insufficient data for lag correlation analysis.
              </Card>
            ) : (
              <div>
                <HighchartsPanel chartRef={lagChartRef} options={lagChartOptions} resetZoomOnDoubleClick />
 <p className="mt-1 text-xs text-muted-foreground">
                  Correlation at different time lags · positive lag = candidate lags behind focus
                </p>
              </div>
            )}
          </>
        )}

        {pairTab === "rolling" && (
          <>
            {!rollingCorrelation ? (
              <Card className="rounded-lg gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
                Insufficient data for rolling correlation (need at least {rollingWindowBuckets} points).
              </Card>
            ) : (
              <>
                <TrendChart
                  data={[rollingCorrelation]}
                  timeZone={timeZone}
                  heightPx={200}
                  yDomain={{ min: -1, max: 1 }}
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Rolling {rollingWindowBuckets}-bucket Pearson correlation over time
                </p>
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
