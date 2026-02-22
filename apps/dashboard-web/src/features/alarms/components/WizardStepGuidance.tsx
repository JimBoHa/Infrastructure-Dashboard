"use client";

import type { Options as HighchartsOptions } from "highcharts";
import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import InlineBanner from "@/components/InlineBanner";
import { TrendChart } from "@/components/TrendChart";
import { HighchartsPanel } from "@/components/charts/HighchartsPanel";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";
import { createHistogramOptions } from "@/lib/chartFactories";
import { CHART_HEIGHTS, CHART_PALETTE } from "@/lib/chartTokens";
import { formatNumber } from "@/lib/format";
import { fetchAlarmRuleStats } from "@/lib/api";
import { useTrendPreviewQuery } from "@/lib/queries";
import type { DemoSensor, TrendSeriesEntry, TrendSeriesPoint } from "@/types/dashboard";
import type {
  AlarmRuleStatsBandSet,
  AlarmRuleStatsBucketAggregationMode,
  AlarmRuleStatsSensor,
  AlarmRuleCreateRequest,
  AlarmWizardState,
} from "@/features/alarms/types/alarmTypes";

type RangePreset = "24h" | "7d" | "30d";

type OverlayChoice =
  | "none"
  | "classic_1"
  | "classic_2"
  | "classic_3"
  | "robust_1"
  | "robust_2"
  | "robust_3"
  | "p05_p95"
  | "p01_p99";

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const computeWindow = (preset: RangePreset): { startIso: string; endIso: string } => {
  const end = new Date();
  const hours = preset === "24h" ? 24 : preset === "7d" ? 7 * 24 : 30 * 24;
  const start = new Date(end.getTime() - hours * 60 * 60 * 1000);
  return { startIso: start.toISOString(), endIso: end.toISOString() };
};

function formatScalar(value: number): string {
  if (!Number.isFinite(value)) return "";
  const rounded = Math.round(value * 1_000_000) / 1_000_000;
  return String(rounded);
}

function computeHistogram({
  values,
  bins,
}: {
  values: number[];
  bins: number;
}): { labels: string[]; counts: number[] } {
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
    return `${formatNumber(lo, { maximumFractionDigits: 2 })}–${formatNumber(hi, { maximumFractionDigits: 2 })}`;
  });
  return { labels, counts };
}

function buildConstantSeries(base: TrendSeriesEntry, value: number, label: string): TrendSeriesEntry {
  const points: TrendSeriesPoint[] = base.points.map((pt) => ({
    timestamp: pt.timestamp,
    value: pt.value == null ? null : value,
    samples: 0,
  }));
  return {
    sensor_id: `ref:${label}`,
    label,
    unit: base.unit,
    display_decimals: base.display_decimals,
    points,
  };
}

function resolveBandPair(
  bands: AlarmRuleStatsBandSet,
  sigma: 1 | 2 | 3,
): { lower: number | null; upper: number | null } {
  const lower =
    sigma === 1 ? bands.lower_1 ?? null : sigma === 2 ? bands.lower_2 ?? null : bands.lower_3 ?? null;
  const upper =
    sigma === 1 ? bands.upper_1 ?? null : sigma === 2 ? bands.upper_2 ?? null : bands.upper_3 ?? null;
  return { lower: typeof lower === "number" ? lower : null, upper: typeof upper === "number" ? upper : null };
}

function formatMaybeNumber(value: number | null | undefined, unit?: string): string {
  if (value == null || !Number.isFinite(value)) return "—";
  const suffix = unit ? ` ${unit}` : "";
  return `${formatNumber(value, { maximumFractionDigits: 4 })}${suffix}`;
}

function formatBandRange(pair: { lower: number | null; upper: number | null }, unit?: string): string {
  if (pair.lower == null || pair.upper == null) return "—";
  return `${formatMaybeNumber(pair.lower, unit)} → ${formatMaybeNumber(pair.upper, unit)}`;
}

export default function WizardStepGuidance({
  state,
  onPatch,
  payload,
  sensors,
}: {
  state: AlarmWizardState;
  onPatch: (partial: Partial<AlarmWizardState>) => void;
  payload: AlarmRuleCreateRequest | null;
  sensors: DemoSensor[];
}) {
  const [rangePreset, setRangePreset] = useState<RangePreset>("7d");
  const [bucketMode, setBucketMode] = useState<AlarmRuleStatsBucketAggregationMode>("auto");
  const [overlay, setOverlay] = useState<OverlayChoice>(() => {
    if (state.template === "range") return "robust_2";
    if (state.template === "threshold") return "robust_2";
    return "none";
  });

  const selectorKey = useMemo(() => {
    if (!payload) return null;
    try {
      return JSON.stringify(payload.target_selector);
    } catch {
      return null;
    }
  }, [payload]);

  const window = useMemo(() => computeWindow(rangePreset), [rangePreset]);
  const startIso = window.startIso;
  const endIso = window.endIso;

  const statsQuery = useQuery({
    queryKey: ["alarm-rule-stats", selectorKey ?? "missing", startIso, endIso, bucketMode],
    queryFn: () =>
      fetchAlarmRuleStats({
        target_selector: payload!.target_selector,
        start: startIso,
        end: endIso,
        bucket_aggregation_mode: bucketMode,
      }),
    enabled: Boolean(payload && selectorKey),
    staleTime: 30_000,
  });

  const sensorsById = useMemo(() => {
    const out = new Map<string, DemoSensor>();
    for (const sensor of sensors) {
      out.set(sensor.sensor_id, sensor);
    }
    return out;
  }, [sensors]);

  const statsSensors: AlarmRuleStatsSensor[] = useMemo(() => statsQuery.data?.sensors ?? [], [statsQuery.data?.sensors]);

  const [previewSensorId, setPreviewSensorId] = useState<string>("");
  useEffect(() => {
    if (!statsSensors.length) return;
    const exists = statsSensors.some((s) => s.sensor_id === previewSensorId);
    if (!previewSensorId || !exists) {
      setPreviewSensorId(statsSensors[0]!.sensor_id);
    }
  }, [previewSensorId, statsSensors]);

  const previewStats = useMemo(() => {
    if (!previewSensorId) return null;
    return statsSensors.find((s) => s.sensor_id === previewSensorId) ?? null;
  }, [previewSensorId, statsSensors]);

  const previewSensor = previewSensorId ? sensorsById.get(previewSensorId) ?? null : null;

  const previewWindow = useMemo(() => {
    const start = statsQuery.data?.start;
    const end = statsQuery.data?.end;
    if (!start || !end) return null;
    return { start, end };
  }, [statsQuery.data?.end, statsQuery.data?.start]);

  const previewIntervalSeconds = statsQuery.data?.interval_seconds ?? 60;

  const trendQuery = useTrendPreviewQuery({
    sensorId: previewSensorId,
    start: previewWindow?.start ?? new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
    end: previewWindow?.end ?? new Date().toISOString(),
    interval: previewIntervalSeconds,
    enabled: Boolean(previewWindow && previewSensorId),
  });

  const baseSeries: TrendSeriesEntry | null = useMemo(() => {
    const entry = trendQuery.data?.[0] ?? null;
    if (!entry) return null;
    return {
      ...entry,
      label: previewSensor?.name ?? entry.label ?? previewSensorId,
      unit: previewSensor?.unit ?? previewStats?.unit ?? entry.unit,
    };
  }, [previewSensor?.name, previewSensor?.unit, previewSensorId, previewStats?.unit, trendQuery.data]);

  const overlaySeries: TrendSeriesEntry[] = useMemo(() => {
    if (!baseSeries || !previewStats) return [];
    if (overlay === "none") return [];

    const out: TrendSeriesEntry[] = [];

    const addPair = (labelPrefix: string, lower: number | null, upper: number | null) => {
      if (lower != null && Number.isFinite(lower)) {
        out.push(buildConstantSeries(baseSeries, lower, `${labelPrefix} lower`));
      }
      if (upper != null && Number.isFinite(upper)) {
        out.push(buildConstantSeries(baseSeries, upper, `${labelPrefix} upper`));
      }
    };

    if (overlay === "p05_p95") {
      addPair("p05–p95", previewStats.p05 ?? null, previewStats.p95 ?? null);
      return out;
    }
    if (overlay === "p01_p99") {
      addPair("p01–p99", previewStats.p01 ?? null, previewStats.p99 ?? null);
      return out;
    }

    const [source, sigmaRaw] = overlay.split("_");
    const sigma = Number(sigmaRaw) as 1 | 2 | 3;
    if (source !== "classic" && source !== "robust") return out;
    if (![1, 2, 3].includes(sigma)) return out;
    const bands = source === "classic" ? previewStats.bands.classic : previewStats.bands.robust;
    const pair = resolveBandPair(bands, sigma);
    addPair(`${source} ±${sigma}\u03C3`, pair.lower, pair.upper);

    // Add a centerline for context (mean or median)
    const center = source === "classic" ? previewStats.mean ?? null : previewStats.median ?? null;
    if (center != null && Number.isFinite(center)) {
      out.push(buildConstantSeries(baseSeries, center, `${source} center`));
    }

    return out;
  }, [baseSeries, overlay, previewStats]);

  const histogramOptions = useMemo<HighchartsOptions | null>(() => {
    const values =
      baseSeries?.points
        .map((pt) => pt.value)
        .filter((v): v is number => typeof v === "number" && Number.isFinite(v)) ?? [];
    if (!values.length) return null;

    const histogram = computeHistogram({ values, bins: 24 });
    if (!histogram.labels.length) return null;

    const base = createHistogramOptions({
      series: [
        {
          type: "column",
          name: "Samples",
          data: histogram.counts,
        },
      ],
      xType: "category",
      height: CHART_HEIGHTS.compact,
    });

    return {
      ...base,
      xAxis: {
        categories: histogram.labels,
        labels: { rotation: 0, style: { fontSize: "10px" } },
        tickInterval: Math.ceil(histogram.labels.length / 6),
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
  }, [baseSeries]);

  const applyAllowed = !state.advancedMode;

  const applyThreshold = (op: AlarmWizardState["thresholdOp"], value: number) => {
    if (!applyAllowed) return;
    if (!Number.isFinite(value)) return;
    onPatch({ thresholdOp: op, thresholdValue: formatScalar(value) });
  };

  const applyRange = (low: number, high: number) => {
    if (!applyAllowed) return;
    if (!Number.isFinite(low) || !Number.isFinite(high)) return;
    onPatch({ rangeLow: formatScalar(low), rangeHigh: formatScalar(high) });
  };

  const previewUnit = previewSensor?.unit || previewStats?.unit || baseSeries?.unit || "";

  const classic2 = previewStats ? resolveBandPair(previewStats.bands.classic, 2) : { lower: null, upper: null };
  const robust2 = previewStats ? resolveBandPair(previewStats.bands.robust, 2) : { lower: null, upper: null };

  return (
    <div className="space-y-4">
      {!payload ? (
        <InlineBanner tone="danger">Fix Basics/Condition inputs to see guidance.</InlineBanner>
      ) : null}

      {state.advancedMode ? (
        <InlineBanner tone="info">
          Guidance is read-only in expert JSON mode. Switch back to guided mode in the Review step to use one-click apply.
        </InlineBanner>
      ) : null}

      <Card className="rounded-xl border border-border p-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
          <div className="min-w-0">
            <p className="text-sm font-semibold text-card-foreground">Guidance window</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Stats are computed over the selected baseline so operators can choose sensible thresholds and bands.
            </p>
          </div>
          <div className="grid gap-2 md:grid-cols-2">
            <div>
              <label className="text-[11px] font-semibold text-muted-foreground">Range</label>
              <Select value={rangePreset} onChange={(e) => setRangePreset(e.target.value as RangePreset)}>
                <option value="24h">Last 24h</option>
                <option value="7d">Last 7d</option>
                <option value="30d">Last 30d</option>
              </Select>
            </div>
            <div>
              <label className="text-[11px] font-semibold text-muted-foreground">Aggregation</label>
              <Select
                value={bucketMode}
                onChange={(e) => setBucketMode(e.target.value as AlarmRuleStatsBucketAggregationMode)}
              >
                <option value="auto">auto (recommended)</option>
                <option value="avg">avg</option>
                <option value="last">last</option>
                <option value="min">min</option>
                <option value="max">max</option>
                <option value="sum">sum</option>
              </Select>
            </div>
          </div>
        </div>

        {statsQuery.isLoading ? <p className="mt-3 text-sm text-muted-foreground">Loading stats…</p> : null}
        {statsQuery.error ? (
          <InlineBanner tone="danger" className="mt-3">
            {statsQuery.error instanceof Error ? statsQuery.error.message : "Failed to load stats."}
          </InlineBanner>
        ) : null}

        {!statsQuery.isLoading && !statsQuery.error ? (
          <div className="mt-4 grid gap-3 md:grid-cols-12">
            <div className="md:col-span-6">
              <label className="text-xs font-semibold text-muted-foreground">Preview sensor</label>
              <Select value={previewSensorId} onChange={(e) => setPreviewSensorId(e.target.value)}>
                {statsSensors.length ? null : <option value="">No sensors matched</option>}
                {statsSensors.map((sensor) => {
                  const meta = sensorsById.get(sensor.sensor_id);
                  const label = meta ? `${meta.name} (${meta.unit || sensor.unit || "unitless"})` : sensor.sensor_id;
                  return (
                    <option key={sensor.sensor_id} value={sensor.sensor_id}>
                      {label}
                    </option>
                  );
                })}
              </Select>
            </div>
            <div className="md:col-span-6">
              <label className="text-xs font-semibold text-muted-foreground">Band overlay</label>
              <Select value={overlay} onChange={(e) => setOverlay(e.target.value as OverlayChoice)}>
                <option value="none">None</option>
                <option value="robust_1">Robust ±1σ (MAD)</option>
                <option value="robust_2">Robust ±2σ (MAD)</option>
                <option value="robust_3">Robust ±3σ (MAD)</option>
                <option value="classic_1">Classic ±1σ (std dev)</option>
                <option value="classic_2">Classic ±2σ (std dev)</option>
                <option value="classic_3">Classic ±3σ (std dev)</option>
                <option value="p05_p95">p05–p95</option>
                <option value="p01_p99">p01–p99</option>
              </Select>
            </div>
          </div>
        ) : null}
      </Card>

      {previewStats ? (
        <Card className="rounded-xl border border-border p-4">
          <p className="text-sm font-semibold text-card-foreground">Stats</p>
          <div className="mt-3 grid gap-3 md:grid-cols-3">
            <StatRow label="Samples (n)" value={String(previewStats.n)} />
            <StatRow label="Coverage" value={formatMaybeNumber(previewStats.coverage_pct ?? null, "%")} />
            <StatRow label="Missing" value={formatMaybeNumber(previewStats.missing_pct ?? null, "%")} />

            <StatRow label="Min" value={formatMaybeNumber(previewStats.min ?? null, previewUnit)} />
            <StatRow label="Max" value={formatMaybeNumber(previewStats.max ?? null, previewUnit)} />
            <StatRow label="Median" value={formatMaybeNumber(previewStats.median ?? null, previewUnit)} />

            <StatRow label="Mean" value={formatMaybeNumber(previewStats.mean ?? null, previewUnit)} />
            <StatRow label="Std dev" value={formatMaybeNumber(previewStats.stddev ?? null, previewUnit)} />
            <StatRow label="MAD" value={formatMaybeNumber(previewStats.mad ?? null, previewUnit)} />

            <StatRow label="p05" value={formatMaybeNumber(previewStats.p05 ?? null, previewUnit)} />
            <StatRow label="p95" value={formatMaybeNumber(previewStats.p95 ?? null, previewUnit)} />
            <StatRow label="IQR" value={formatMaybeNumber(previewStats.iqr ?? null, previewUnit)} />
          </div>

          <div className="mt-4 grid gap-3 md:grid-cols-2">
            <div className="rounded-lg border border-border bg-card-inset p-3">
              <p className="text-xs font-semibold text-muted-foreground">Classic bands (mean ± k·σ)</p>
              <div className="mt-2 space-y-1 text-xs text-muted-foreground">
                <p>±1σ: {formatBandRange(resolveBandPair(previewStats.bands.classic, 1), previewUnit)}</p>
                <p>±2σ: {formatBandRange(resolveBandPair(previewStats.bands.classic, 2), previewUnit)}</p>
                <p>±3σ: {formatBandRange(resolveBandPair(previewStats.bands.classic, 3), previewUnit)}</p>
              </div>
            </div>
            <div className="rounded-lg border border-border bg-card-inset p-3">
              <p className="text-xs font-semibold text-muted-foreground">Robust bands (median ± k·σ)</p>
              <div className="mt-2 space-y-1 text-xs text-muted-foreground">
                <p>±1σ: {formatBandRange(resolveBandPair(previewStats.bands.robust, 1), previewUnit)}</p>
                <p>±2σ: {formatBandRange(resolveBandPair(previewStats.bands.robust, 2), previewUnit)}</p>
                <p>±3σ: {formatBandRange(resolveBandPair(previewStats.bands.robust, 3), previewUnit)}</p>
              </div>
            </div>
          </div>

          {state.template === "threshold" ? (
            <div className="mt-4 rounded-xl border border-border bg-card-inset p-3">
              <p className="text-sm font-semibold text-card-foreground">One-click apply: threshold</p>
              <p className="mt-1 text-xs text-muted-foreground">
                These set the operator and value. Always sanity-check against the chart.
              </p>
              <div className="mt-3 grid gap-3 md:grid-cols-2">
                <div className="space-y-2">
                  <p className="text-xs font-semibold text-muted-foreground">Low alarm (value &lt; X)</p>
                  <div className="flex flex-wrap gap-2">
                    <NodeButton
                      size="sm"
                      onClick={() => applyThreshold("lt", robust2.lower ?? NaN)}
                      disabled={!applyAllowed || robust2.lower == null}
                    >
                      robust −2σ
                    </NodeButton>
                    <NodeButton
                      size="sm"
                      onClick={() => applyThreshold("lt", classic2.lower ?? NaN)}
                      disabled={!applyAllowed || classic2.lower == null}
                    >
                      classic −2σ
                    </NodeButton>
                    <NodeButton
                      size="sm"
                      onClick={() => applyThreshold("lt", previewStats.p05 ?? NaN)}
                      disabled={!applyAllowed || previewStats.p05 == null}
                    >
                      p05
                    </NodeButton>
                  </div>
                </div>
                <div className="space-y-2">
                  <p className="text-xs font-semibold text-muted-foreground">High alarm (value &gt; X)</p>
                  <div className="flex flex-wrap gap-2">
                    <NodeButton
                      size="sm"
                      onClick={() => applyThreshold("gt", robust2.upper ?? NaN)}
                      disabled={!applyAllowed || robust2.upper == null}
                    >
                      robust +2σ
                    </NodeButton>
                    <NodeButton
                      size="sm"
                      onClick={() => applyThreshold("gt", classic2.upper ?? NaN)}
                      disabled={!applyAllowed || classic2.upper == null}
                    >
                      classic +2σ
                    </NodeButton>
                    <NodeButton
                      size="sm"
                      onClick={() => applyThreshold("gt", previewStats.p95 ?? NaN)}
                      disabled={!applyAllowed || previewStats.p95 == null}
                    >
                      p95
                    </NodeButton>
                  </div>
                </div>
              </div>
            </div>
          ) : null}

          {state.template === "range" ? (
            <div className="mt-4 rounded-xl border border-border bg-card-inset p-3">
              <p className="text-sm font-semibold text-card-foreground">One-click apply: band</p>
              <p className="mt-1 text-xs text-muted-foreground">Sets low/high. Mode stays {state.rangeMode}.</p>
              <div className="mt-3 flex flex-wrap gap-2">
                <NodeButton
                  size="sm"
                  onClick={() => applyRange(robust2.lower ?? NaN, robust2.upper ?? NaN)}
                  disabled={!applyAllowed || robust2.lower == null || robust2.upper == null}
                >
                  robust ±2σ
                </NodeButton>
                <NodeButton
                  size="sm"
                  onClick={() => applyRange(classic2.lower ?? NaN, classic2.upper ?? NaN)}
                  disabled={!applyAllowed || classic2.lower == null || classic2.upper == null}
                >
                  classic ±2σ
                </NodeButton>
                <NodeButton
                  size="sm"
                  onClick={() => applyRange(previewStats.p05 ?? NaN, previewStats.p95 ?? NaN)}
                  disabled={!applyAllowed || previewStats.p05 == null || previewStats.p95 == null}
                >
                  p05–p95
                </NodeButton>
              </div>
            </div>
          ) : null}
        </Card>
      ) : null}

      <Card className="rounded-xl border border-border p-4">
        <p className="text-sm font-semibold text-card-foreground">Visualization</p>
        <p className="mt-1 text-xs text-muted-foreground">
          {previewWindow
            ? `${new Date(previewWindow.start).toLocaleString()} → ${new Date(previewWindow.end).toLocaleString()} · interval ${previewIntervalSeconds}s`
            : "Select a window to render charts."}
        </p>

        {trendQuery.isLoading ? <p className="mt-3 text-sm text-muted-foreground">Loading preview…</p> : null}
        {trendQuery.error ? (
          <InlineBanner tone="danger" className="mt-3">
            {trendQuery.error instanceof Error ? trendQuery.error.message : "Failed to load preview."}
          </InlineBanner>
        ) : null}

        {baseSeries ? (
          <div className="mt-3 space-y-4">
            <TrendChart
              title="Time-series preview"
              description={
                <span className="text-xs text-muted-foreground">
                  Overlay reflects the selected band option; stats/bands are computed over the baseline window.
                </span>
              }
              data={[baseSeries, ...overlaySeries]}
              independentAxes={false}
              stacked={false}
              navigator={false}
              analysisTools={false}
              heightPx={320}
            />

            {histogramOptions ? (
              <div className="rounded-xl border border-border bg-card-inset p-3">
                <p className="text-sm font-semibold text-card-foreground">Histogram</p>
                <p className="mt-1 text-xs text-muted-foreground">Bucketed from the preview series values.</p>
                <div className="mt-3">
                  <HighchartsPanel options={histogramOptions} enableAutoReflow />
                </div>
              </div>
            ) : (
              <Card className="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
                Histogram unavailable (no numeric samples).
              </Card>
            )}
          </div>
        ) : (
          <Card className="mt-3 rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
            No preview series available.
          </Card>
        )}
      </Card>
    </div>
  );
}

function StatRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border bg-card-inset p-3">
      <p className="text-xs font-semibold text-muted-foreground">{label}</p>
      <p className="mt-1 text-sm font-semibold text-card-foreground">{value}</p>
    </div>
  );
}
