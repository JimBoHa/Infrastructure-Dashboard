"use client";

import { useMemo, useRef, useState } from "react";
import type { Options as HighchartsOptions } from "highcharts";
import { HighchartsPanel, type HighchartsChartRef } from "@/components/charts/HighchartsPanel";
import type { TrendSeriesEntry } from "@/types/dashboard";
import { formatNumber } from "@/lib/format";
import { CHART_PALETTE, CHART_HEIGHTS } from "@/lib/chartTokens";
import { createBaseOptions, createHistogramOptions } from "@/lib/chartFactories";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";

type VoltageSeriesMeta = {
  sensorId: string;
  label: string;
  nominalV: number | null;
  stats: {
    min: number | null;
    max: number | null;
    mean: number | null;
    stddev: number | null;
    p5: number | null;
    p50: number | null;
    p95: number | null;
    within5Pct: number | null;
    within10Pct: number | null;
    sagCount: number;
    swellCount: number;
    flicker95: number | null;
  };
  histogram: { bins: string[]; counts: number[] };
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const percentile = (sorted: number[], p: number): number | null => {
  if (!sorted.length) return null;
  const idx = (sorted.length - 1) * clamp(p, 0, 1);
  const lo = Math.floor(idx);
  const hi = Math.ceil(idx);
  if (lo === hi) return sorted[lo] ?? null;
  const a = sorted[lo];
  const b = sorted[hi];
  if (a == null || b == null) return null;
  return a + (b - a) * (idx - lo);
};

const mean = (values: number[]): number | null => {
  if (!values.length) return null;
  const sum = values.reduce((acc, v) => acc + v, 0);
  return sum / values.length;
};

const stddev = (values: number[], mu: number): number | null => {
  if (values.length < 2) return null;
  const variance =
    values.reduce((acc, v) => acc + (v - mu) * (v - mu), 0) / (values.length - 1);
  return Math.sqrt(variance);
};

type VoltageQualityMode = "ac" | "dc";

const inferNominalAc = (values: number[]): number | null => {
  const sorted = values.slice().sort((a, b) => a - b);
  const p50 = percentile(sorted, 0.5);
  if (p50 == null) return null;
  if (p50 > 180 && p50 < 280) return 240;
  if (p50 > 90 && p50 < 150) return 120;
  return Math.round(p50 / 5) * 5;
};

const inferNominalDc = (values: number[]): number | null => {
  const sorted = values.slice().sort((a, b) => a - b);
  const p50 = percentile(sorted, 0.5);
  if (p50 == null) return null;
  if (p50 > 30 && p50 < 70) return 48;
  if (p50 > 16 && p50 < 35) return 24;
  if (p50 > 7 && p50 < 18) return 12;
  return Math.round(p50 * 2) / 2;
};

const computeSagSwellCounts = ({
  values,
  nominal,
  intervalSeconds,
}: {
  values: Array<{ t: number; v: number }>;
  nominal: number | null;
  intervalSeconds: number;
}): { sagCount: number; swellCount: number } => {
  if (!nominal || nominal <= 0) return { sagCount: 0, swellCount: 0 };
  const sagThreshold = nominal * 0.9;
  const swellThreshold = nominal * 1.1;

  let sagCount = 0;
  let swellCount = 0;
  let sagActive = false;
  let swellActive = false;
  let lastT: number | null = null;

  for (const pt of values) {
    const v = pt.v;
    const t = pt.t;
    const gapSeconds = lastT == null ? 0 : (t - lastT) / 1000;
    const isGap = lastT != null && gapSeconds > intervalSeconds * 2;

    if (isGap) {
      sagActive = false;
      swellActive = false;
    }

    const isSag = v < sagThreshold;
    const isSwell = v > swellThreshold;

    if (isSag && !sagActive) sagCount += 1;
    if (!isSag) sagActive = false;
    else sagActive = true;

    if (isSwell && !swellActive) swellCount += 1;
    if (!isSwell) swellActive = false;
    else swellActive = true;

    lastT = t;
  }

  return { sagCount, swellCount };
};

const computeHistogram = ({
  values,
  bins,
}: {
  values: number[];
  bins: number;
}): { labels: string[]; counts: number[] } => {
  if (!values.length) return { labels: [], counts: [] };
  const minV = Math.min(...values);
  const maxV = Math.max(...values);
  if (!Number.isFinite(minV) || !Number.isFinite(maxV) || minV === maxV) {
    return { labels: [formatNumber(minV)], counts: [values.length] };
  }
  const width = (maxV - minV) / bins;
  const counts = new Array(bins).fill(0);
  for (const v of values) {
    const idx = clamp(Math.floor((v - minV) / width), 0, bins - 1);
    counts[idx] += 1;
  }
  const labels = counts.map((_, idx) => {
    const lo = minV + idx * width;
    const hi = lo + width;
    return `${formatNumber(lo, { maximumFractionDigits: 1 })}–${formatNumber(hi, { maximumFractionDigits: 1 })}`;
  });
  return { labels, counts };
};

const formatV = (value: number | null, decimals = 1) => {
  if (value == null || !Number.isFinite(value)) return "—";
  return `${formatNumber(value, { minimumFractionDigits: decimals, maximumFractionDigits: decimals })} V`;
};

export default function VoltageQualityPanel({
  series,
  intervalSeconds,
  mode = "ac",
  title,
}: {
  series: TrendSeriesEntry[];
  intervalSeconds: number;
  mode?: VoltageQualityMode;
  title?: string;
}) {
  const timeSeriesRef = useRef<HighchartsChartRef | null>(null);
  const histogramRef = useRef<HighchartsChartRef | null>(null);

  const eligibleSeries = useMemo(() => {
    return series.filter((s) => (s.unit ?? "").toLowerCase() === "v");
  }, [series]);

  const [activeSensorId, setActiveSensorId] = useState<string>(() => eligibleSeries[0]?.sensor_id ?? "");

  const metaBySensorId = useMemo(() => {
    const out = new Map<string, VoltageSeriesMeta>();
    for (const entry of eligibleSeries) {
      const points = entry.points
        .map((pt) => {
          const t = pt.timestamp instanceof Date ? pt.timestamp.getTime() : new Date(pt.timestamp).getTime();
          const v = pt.value;
          return typeof v === "number" && Number.isFinite(v) && Number.isFinite(t) ? { t, v } : null;
        })
        .filter((v): v is { t: number; v: number } => Boolean(v))
        .sort((a, b) => a.t - b.t);

      const values = points.map((p) => p.v);
      const sorted = values.slice().sort((a, b) => a - b);
      const nominalV = mode === "dc" ? inferNominalDc(values) : inferNominalAc(values);
      const p50 = percentile(sorted, 0.5);
      const mu = mean(values);
      const sigma = mu == null ? null : stddev(values, mu);

      const within = (pct: number) => {
        if (!nominalV || nominalV <= 0 || !values.length) return null;
        const lo = nominalV * (1 - pct);
        const hi = nominalV * (1 + pct);
        const ok = values.filter((v) => v >= lo && v <= hi).length;
        return ok / values.length;
      };

      const sagSwell = computeSagSwellCounts({ values: points, nominal: nominalV, intervalSeconds });

      const flicker = (() => {
        if (points.length < 2) return null;
        const deltas: number[] = [];
        for (let i = 1; i < points.length; i += 1) {
          const a = points[i - 1];
          const b = points[i];
          if (!a || !b) continue;
          const gapSeconds = (b.t - a.t) / 1000;
          if (gapSeconds > intervalSeconds * 2) continue;
          deltas.push(Math.abs(b.v - a.v));
        }
        deltas.sort((a, b) => a - b);
        return percentile(deltas, 0.95);
      })();

      const histogram = computeHistogram({ values, bins: 18 });

      out.set(entry.sensor_id, {
        sensorId: entry.sensor_id,
        label: entry.label ?? entry.sensor_id,
        nominalV,
        stats: {
          min: values.length ? Math.min(...values) : null,
          max: values.length ? Math.max(...values) : null,
          mean: mu,
          stddev: sigma,
          p5: percentile(sorted, 0.05),
          p50,
          p95: percentile(sorted, 0.95),
          within5Pct: within(0.05),
          within10Pct: within(0.1),
          sagCount: sagSwell.sagCount,
          swellCount: sagSwell.swellCount,
          flicker95: flicker,
        },
        histogram: { bins: histogram.labels, counts: histogram.counts },
      });
    }
    return out;
  }, [eligibleSeries, intervalSeconds, mode]);

  const hasSeries = eligibleSeries.length > 0;
  const active =
    metaBySensorId.get(activeSensorId) ?? metaBySensorId.get(eligibleSeries[0]?.sensor_id ?? "");
  const activeSeries = eligibleSeries.find((s) => s.sensor_id === active?.sensorId) ?? null;
  const activeLabel = active?.label ?? activeSeries?.label ?? activeSeries?.sensor_id ?? "";

  const nominal = active?.nominalV ?? null;
  const effectiveTitle = title ?? (mode === "dc" ? "DC voltage quality" : "AC voltage quality");
  const band5Lo = nominal ? nominal * 0.95 : null;
  const band5Hi = nominal ? nominal * 1.05 : null;
  const band10Lo = nominal ? nominal * 0.9 : null;
  const band10Hi = nominal ? nominal * 1.1 : null;

  const qualitySegments = (() => {
    const within5 = active?.stats.within5Pct ?? null;
    const within10 = active?.stats.within10Pct ?? null;
    if (within5 == null || within10 == null) return null;
    const ok = clamp(within5, 0, 1);
    const warn = clamp(within10 - within5, 0, 1);
    const bad = clamp(1 - within10, 0, 1);
    return { ok, warn, bad };
  })();

  // Time series chart options (Highcharts)
  const timeSeriesOptions = useMemo<HighchartsOptions | null>(() => {
    if (!activeSeries) return null;

    const mainData: Array<[number, number | null]> = activeSeries.points.map((pt) => {
      const t = pt.timestamp instanceof Date ? pt.timestamp.getTime() : new Date(pt.timestamp).getTime();
      return [t, pt.value];
    });

    const series: HighchartsOptions["series"] = [
      {
        type: "line",
        name: activeLabel || activeSeries.sensor_id,
        data: mainData,
        color: CHART_PALETTE.series[0],
        lineWidth: 1.5,
        marker: { enabled: false },
      },
    ];

    // Add band lines if we have nominal voltage
    if (band5Lo != null && band5Hi != null && band10Lo != null && band10Hi != null && mainData.length > 0) {
      const firstT = mainData[0]![0];
      const lastT = mainData[mainData.length - 1]![0];

      series.push(
        {
          type: "line",
          name: "±5%",
          data: [[firstT, band5Hi], [lastT, band5Hi]],
          color: `${CHART_PALETTE.band.good}a6`,
          lineWidth: 1,
          dashStyle: "Dash",
          marker: { enabled: false },
          enableMouseTracking: false,
        },
        {
          type: "line",
          name: "",
          data: [[firstT, band5Lo], [lastT, band5Lo]],
          color: `${CHART_PALETTE.band.good}a6`,
          lineWidth: 1,
          dashStyle: "Dash",
          marker: { enabled: false },
          enableMouseTracking: false,
          showInLegend: false,
        },
        {
          type: "line",
          name: "±10%",
          data: [[firstT, band10Hi], [lastT, band10Hi]],
          color: `${CHART_PALETTE.band.warn}a6`,
          lineWidth: 1,
          dashStyle: "ShortDot",
          marker: { enabled: false },
          enableMouseTracking: false,
        },
        {
          type: "line",
          name: "",
          data: [[firstT, band10Lo], [lastT, band10Lo]],
          color: `${CHART_PALETTE.band.warn}a6`,
          lineWidth: 1,
          dashStyle: "ShortDot",
          marker: { enabled: false },
          enableMouseTracking: false,
          showInLegend: false,
        },
      );
    }

    const base = createBaseOptions({
      chart: {
        type: "line",
        zooming: { type: "x" },
        height: CHART_HEIGHTS.standard,
      },
      title: { text: undefined },
      xAxis: {
        type: "datetime",
        gridLineWidth: 1,
        gridLineColor: "#f3f4f6",
        labels: {
          format: "{value:%b %e, %H:%M}",
        },
      },
      yAxis: {
        title: { text: undefined },
        gridLineColor: "#f3f4f6",
      },
      legend: {
        enabled: true,
        align: "center",
        verticalAlign: "bottom",
        symbolWidth: 10,
        symbolHeight: 10,
      },
      navigator: { enabled: false },
      rangeSelector: { enabled: false },
      scrollbar: { enabled: false },
      tooltip: {
        shared: true,
        xDateFormat: "%b %e, %H:%M",
        pointFormatter: function () {
          const y = this.y;
          const v = typeof y === "number" && Number.isFinite(y) ? formatV(y, 1) : "—";
          return `<span style="color:${this.color}">\u25CF</span> ${this.series.name}: <b>${v}</b><br/>`;
        },
      },
      plotOptions: {
        line: {
          connectNulls: false,
        },
      },
      series,
    });
    return base;
  }, [activeSeries, activeLabel, band5Lo, band5Hi, band10Lo, band10Hi]);

  // Histogram chart options (Highcharts)
  const histogramOptions = useMemo<HighchartsOptions | null>(() => {
    if (!active?.histogram.bins.length) return null;

    const base = createHistogramOptions({
      series: [
        {
          type: "column",
          name: "Samples",
          data: active.histogram.counts,
        },
      ],
      xType: "category",
      height: CHART_HEIGHTS.compact,
    });

    // Override xAxis for custom categories + tick interval, tooltip, and column styling
    return {
      ...base,
      xAxis: {
        categories: active.histogram.bins,
        labels: {
          rotation: 0,
          style: { fontSize: "10px" },
        },
        tickInterval: Math.ceil(active.histogram.bins.length / 6),
      },
      tooltip: {
        formatter: function () {
          return `<b>${this.x}</b><br/>Samples: ${this.y}`;
        },
      },
      plotOptions: {
        column: {
          borderWidth: 1,
          borderColor: `${CHART_PALETTE.series[0]}e6`,
          color: `${CHART_PALETTE.series[0]}a6`,
          pointPadding: 0,
          groupPadding: 0.1,
        },
      },
    };
  }, [active]);

  if (!hasSeries) return null;

  return (
    <Card className="gap-0 p-4 py-4">
      <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
        <div>
 <h3 className="text-sm font-semibold uppercase tracking-wide text-muted-foreground">
            {effectiveTitle}
          </h3>
 <p className="mt-1 text-sm text-foreground">
            {mode === "dc"
              ? "Voltage stability summarizes how a DC rail (battery/PV/load) varies over time (range, ripple, and dips/spikes)."
              : "Power quality focuses on how stable the utility voltage stays around its nominal value (sags, swells, and flicker)."}
          </p>
        </div>
        {eligibleSeries.length > 1 ? (
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Sensor
            <Select
              className="mt-1 min-w-[260px]"
              value={activeSensorId}
              onChange={(e) => setActiveSensorId(e.target.value)}
            >
              {eligibleSeries.map((s) => (
                <option key={s.sensor_id} value={s.sensor_id}>
                  {s.label ?? s.sensor_id}
                </option>
              ))}
            </Select>
          </label>
        ) : null}
      </div>

      {active ? (
        <div className="mt-4 grid gap-4 lg:grid-cols-[1fr_320px]">
          <div className="space-y-4">
            {timeSeriesOptions ? (
              <div className="h-72">
                <HighchartsPanel
                  chartRef={timeSeriesRef}
                  options={timeSeriesOptions}
                  wrapperClassName="h-full w-full"
                  resetZoomOnDoubleClick
                />
              </div>
            ) : null}

            {qualitySegments ? (
              <div>
 <div className="flex items-center justify-between gap-3 text-xs text-muted-foreground">
                  <div>
                    Quality window:{" "}
                    {nominal ? (
                      <span className="font-semibold">
                        nominal {formatNumber(nominal)} V · ±5% ({formatNumber(band5Lo ?? 0)}–{formatNumber(band5Hi ?? 0)} V)
                      </span>
                    ) : (
                      <span className="font-semibold">unknown nominal</span>
                    )}
                  </div>
                  <div className="font-semibold">
                    within ±5%: {formatNumber(qualitySegments.ok * 100, { maximumFractionDigits: 1 })}%
                  </div>
                </div>
                <div className="mt-2 h-3 w-full overflow-hidden rounded-full border border-border bg-card-inset">
                  <div className="flex h-full w-full">
                    <div className="h-full bg-emerald-500" style={{ width: `${qualitySegments.ok * 100}%` }} />
                    <div className="h-full bg-amber-500" style={{ width: `${qualitySegments.warn * 100}%` }} />
                    <div className="h-full bg-rose-500" style={{ width: `${qualitySegments.bad * 100}%` }} />
                  </div>
                </div>
 <div className="mt-2 flex flex-wrap gap-3 text-xs text-muted-foreground">
                  <span className="inline-flex items-center gap-2">
                    <span className="size-2 rounded-full bg-emerald-500" aria-hidden />
                    within ±5%
                  </span>
                  <span className="inline-flex items-center gap-2">
                    <span className="size-2 rounded-full bg-amber-500" aria-hidden />
                    5–10%
                  </span>
                  <span className="inline-flex items-center gap-2">
                    <span className="size-2 rounded-full bg-rose-500" aria-hidden />
                    &gt;10%
                  </span>
                </div>
              </div>
            ) : null}

            {histogramOptions ? (
              <div className="h-56">
                <HighchartsPanel
                  chartRef={histogramRef}
                  options={histogramOptions}
                  wrapperClassName="h-full w-full"
                  resetZoomOnDoubleClick
                />
              </div>
            ) : null}
          </div>

          <Card className="rounded-lg gap-0 bg-card-inset p-3 text-sm text-card-foreground">
 <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Summary
            </div>
            <dl className="mt-2 space-y-2">
              <div className="flex items-center justify-between gap-3">
 <dt className="text-muted-foreground">Nominal</dt>
                <dd className="font-semibold">{nominal ? `${formatNumber(nominal)} V` : "—"}</dd>
              </div>
              <div className="flex items-center justify-between gap-3">
 <dt className="text-muted-foreground">Min / Max</dt>
                <dd className="font-semibold">
                  {formatV(active.stats.min)} / {formatV(active.stats.max)}
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
 <dt className="text-muted-foreground">P5 / P50 / P95</dt>
                <dd className="font-semibold">
                  {formatV(active.stats.p5)} / {formatV(active.stats.p50)} / {formatV(active.stats.p95)}
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
 <dt className="text-muted-foreground">Mean (σ)</dt>
                <dd className="font-semibold">
                  {formatV(active.stats.mean)}{" "}
                  {active.stats.stddev != null ? `(${formatNumber(active.stats.stddev, { maximumFractionDigits: 2 })})` : ""}
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
 <dt className="text-muted-foreground">
                  {mode === "dc" ? "Dips / Spikes" : "Sags / Swells"}
                </dt>
                <dd className="font-semibold">
                  {active.stats.sagCount} / {active.stats.swellCount}
                </dd>
              </div>
              <div className="flex items-center justify-between gap-3">
 <dt className="text-muted-foreground">
                  {mode === "dc" ? "Ripple (ΔV P95)" : "Flicker (ΔV P95)"}
                </dt>
                <dd className="font-semibold">
                  {active.stats.flicker95 != null ? `${formatNumber(active.stats.flicker95, { maximumFractionDigits: 2 })} V` : "—"}
                </dd>
              </div>
            </dl>
 <p className="mt-3 text-xs text-muted-foreground">
              {mode === "dc"
                ? "Dip/spike thresholds default to ±10% of nominal; ripple is the 95th percentile of per-bucket absolute voltage change (gaps excluded)."
                : "Sag/swell thresholds default to ±10% of nominal; flicker is the 95th percentile of per-bucket absolute voltage change (gaps excluded)."}
            </p>
          </Card>
        </div>
      ) : (
 <div className="mt-3 text-sm text-muted-foreground">No voltage data.</div>
      )}
    </Card>
  );
}
