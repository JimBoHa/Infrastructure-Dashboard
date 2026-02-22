"use client";

import type { Options, SeriesLineOptions, SeriesScatterOptions } from "highcharts";
import { useEffect, useMemo, useState } from "react";
import { HighchartsPanel } from "@/components/charts/HighchartsPanel";
import { ZoomableLineChart, useBaseChartOptions } from "@/features/analytics/components/AnalyticsShared";
import { createScatterOptions } from "@/lib/chartFactories";
import { seriesColor } from "@/lib/chartTokens";
import { formatNumber } from "@/lib/format";
import type { TempCompensationFitResult, AlignedTempCompensationPoint } from "@/lib/tempCompensation";
import { computeTemperatureDriftCorrection, applyTemperatureCompensation } from "@/lib/tempCompensation";

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

export function TemperatureCompensationTimeSeriesChart({
  points,
  fit,
  rawLabel,
  compensatedLabel,
  rawUnit,
  temperatureLabel = "Temperature",
  temperatureUnit,
}: {
  points: AlignedTempCompensationPoint[];
  fit: Pick<TempCompensationFitResult, "centerTemp" | "coefficients"> | null;
  rawLabel: string;
  compensatedLabel: string;
  rawUnit: string;
  temperatureLabel?: string;
  temperatureUnit?: string;
}) {
  const baseChartOptions = useBaseChartOptions();

  const { data, options } = useMemo(() => {
    const tempSeries = points.map((pt) => ({ x: pt.timestamp, y: pt.temperature }));
    const rawSeries = points.map((pt) => ({ x: pt.timestamp, y: pt.raw }));
    const compensatedSeries = points.map((pt) => ({
      x: pt.timestamp,
      y: fit ? applyTemperatureCompensation(pt.raw, pt.temperature, fit) : null,
    }));

    const datasets = [
      {
        label: rawLabel,
        data: rawSeries,
        borderColor: seriesColor(0),
        backgroundColor: seriesColor(0),
        borderWidth: 2,
        pointRadius: 0,
      },
      {
        label: compensatedLabel,
        data: compensatedSeries,
        borderColor: seriesColor(1),
        backgroundColor: seriesColor(1),
        borderWidth: 2,
        pointRadius: 0,
      },
      {
        label: temperatureLabel,
        data: tempSeries,
        borderColor: seriesColor(2),
        backgroundColor: seriesColor(2),
        borderWidth: 1.5,
        pointRadius: 0,
        yAxisID: "y1",
      },
    ];

    const options = {
      scales: {
        x: {
          type: "time",
          ticks: {
            callback: baseChartOptions.formatTick,
            maxTicksLimit: 8,
          },
        },
        y: {
          type: "linear",
          title: rawUnit ? { display: true, text: rawUnit } : undefined,
          beginAtZero: false,
        },
        y1: {
          type: "linear",
          position: "right",
          grid: { drawOnChartArea: false },
          title: temperatureUnit ? { display: true, text: temperatureUnit } : undefined,
          beginAtZero: false,
        },
      },
      plugins: {
        legend: { display: true, position: "bottom" },
      },
    } as const;

    return { data: { datasets }, options };
  }, [baseChartOptions.formatTick, fit, points, rawLabel, compensatedLabel, rawUnit, temperatureLabel, temperatureUnit]);

  return <ZoomableLineChart wrapperClassName="h-[420px]" data={data} options={options} />;
}

function buildFittedRawLinePoints({
  fit,
  minTemp,
  maxTemp,
  count = 60,
}: {
  fit: TempCompensationFitResult;
  minTemp: number;
  maxTemp: number;
  count?: number;
}): Array<[number, number]> {
  const out: Array<[number, number]> = [];
  const steps = Math.max(2, Math.floor(count));
  const span = maxTemp - minTemp;
  for (let i = 0; i < steps; i += 1) {
    const t = minTemp + (span * i) / (steps - 1);
    const x = t - fit.centerTemp;
    let y = fit.coefficients[0] ?? 0;
    let xPow = x;
    for (let k = 1; k < fit.coefficients.length; k += 1) {
      y += (fit.coefficients[k] ?? 0) * xPow;
      xPow *= x;
    }
    out.push([t, y]);
  }
  return out;
}

export function TemperatureCompensationScatterChart({
  points,
  fit,
  rawLabel,
  rawUnit,
  temperatureLabel = "Temperature",
  temperatureUnit,
}: {
  points: AlignedTempCompensationPoint[];
  fit: TempCompensationFitResult | null;
  rawLabel: string;
  rawUnit: string;
  temperatureLabel?: string;
  temperatureUnit?: string;
}) {
  const isCoarsePointer = useIsCoarsePointer();
  const options = useMemo<Options>(() => {
    const scatterSeries: SeriesScatterOptions = {
      type: "scatter",
      name: rawLabel,
      data: points.map((pt) => [pt.temperature, pt.raw]),
      color: seriesColor(0),
      marker: { radius: 2 },
      tooltip: {
        pointFormatter: function () {
          const x = typeof this.x === "number" ? this.x : null;
          const y = typeof this.y === "number" ? this.y : null;
          const temp = x != null ? formatNumber(x, { maximumFractionDigits: 2 }) : "\u2014";
          const raw = y != null ? formatNumber(y, { maximumFractionDigits: 3 }) : "\u2014";
          return `<span style="color:${this.color}">\u25CF</span> ${temperatureLabel}: <b>${temp}${temperatureUnit ? ` ${temperatureUnit}` : ""}</b><br/>${rawLabel}: <b>${raw}${rawUnit ? ` ${rawUnit}` : ""}</b><br/>`;
        },
      },
    };

    const fittedSeries: SeriesLineOptions | null =
      fit && Number.isFinite(fit.tempMin) && Number.isFinite(fit.tempMax)
        ? {
            type: "line",
            name: "Fitted drift model",
            data: buildFittedRawLinePoints({ fit, minTemp: fit.tempMin, maxTemp: fit.tempMax }),
            color: seriesColor(1),
            lineWidth: 2,
            marker: { enabled: false },
            tooltip: { valueDecimals: 3 },
          }
        : null;

    const base = createScatterOptions({
      xAxisTitle: temperatureUnit ? `${temperatureLabel} (${temperatureUnit})` : temperatureLabel,
      yAxisTitle: rawUnit ? `${rawLabel} (${rawUnit})` : rawLabel,
      zoom: !isCoarsePointer,
      series: fittedSeries ? [scatterSeries, fittedSeries] : [scatterSeries],
    });

    return {
      ...base,
      tooltip: {
        shared: false,
        useHTML: true,
      },
      legend: { enabled: true, align: "center", verticalAlign: "bottom" },
    };
  }, [fit, isCoarsePointer, points, rawLabel, rawUnit, temperatureLabel, temperatureUnit]);

  return (
    <div className="h-[420px]">
      <HighchartsPanel options={options} wrapperClassName="h-full w-full" />
    </div>
  );
}

export function TemperatureCompensationCorrectionChart({
  fit,
  temperatureUnit,
  rawUnit,
}: {
  fit: TempCompensationFitResult | null;
  temperatureUnit?: string;
  rawUnit?: string;
}) {
  const isCoarsePointer = useIsCoarsePointer();
  const options = useMemo<Options>(() => {
    const series: SeriesLineOptions = {
      type: "line",
      name: "Adjustment",
      data: [],
      color: seriesColor(1),
      lineWidth: 2,
      marker: { enabled: false },
    };

    const base = createScatterOptions({
      xAxisTitle: temperatureUnit ? `Temperature (${temperatureUnit})` : "Temperature",
      yAxisTitle: rawUnit ? `Adjustment (${rawUnit})` : "Adjustment",
      zoom: !isCoarsePointer,
      series: [series],
    });

    if (fit) {
      const data: Array<[number, number]> = [];
      const steps = 60;
      const span = fit.tempMax - fit.tempMin;
      for (let i = 0; i < steps; i += 1) {
        const t = fit.tempMin + (span * i) / (steps - 1);
        const correction = computeTemperatureDriftCorrection(t, fit);
        if (correction == null) continue;
        data.push([t, correction]);
      }
      series.data = data;
    }

    return {
      ...base,
      legend: { enabled: false },
      tooltip: {
        formatter: function () {
          const x = typeof this.x === "number" ? this.x : null;
          const y = typeof this.y === "number" ? this.y : null;
          const t = x != null ? formatNumber(x, { maximumFractionDigits: 2 }) : "\u2014";
          const adj = y != null ? formatNumber(y, { maximumFractionDigits: 4 }) : "\u2014";
          return `<b>${t}${temperatureUnit ? ` ${temperatureUnit}` : ""}</b><br/>Adjustment: <b>${adj}${rawUnit ? ` ${rawUnit}` : ""}</b>`;
        },
        useHTML: true,
      },
    };
  }, [fit, isCoarsePointer, rawUnit, temperatureUnit]);

  return (
    <div className="h-[260px]">
      <HighchartsPanel options={options} wrapperClassName="h-full w-full" />
    </div>
  );
}

export function formatLiveCorrectionPreview({
  rawLatest,
  temperatureLatest,
  fit,
}: {
  rawLatest: number | null;
  temperatureLatest: number | null;
  fit: Pick<TempCompensationFitResult, "centerTemp" | "coefficients"> | null;
}): { corrected: number | null; correction: number | null } {
  if (rawLatest == null || temperatureLatest == null || !fit) return { corrected: null, correction: null };
  const correction = computeTemperatureDriftCorrection(temperatureLatest, fit);
  const corrected = correction == null ? null : rawLatest - correction;
  return { corrected, correction };
}
