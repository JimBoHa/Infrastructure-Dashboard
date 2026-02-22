"use client";

import { useMemo, useState } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  AnalyticsRangeSelect,
  type AnalyticsHistoryRangeHours,
  historyIntervalSeconds,
  historyRangeLabel,
  AnalyticsChart,
  Metric,
  type ChartSeriesConfig,
} from "@/features/analytics/components/AnalyticsShared";
import { useAnalyticsData } from "@/features/analytics/hooks/useAnalyticsData";
import { ANALYTICS_COLORS as COLORS, BATTERY_COLORS } from "@/features/analytics/utils/colors";
import { filterSeriesByHours } from "@/features/analytics/utils/series";
import { formatGallons } from "@/lib/format";
import { useMetricsQuery } from "@/lib/queries";
import { sensorSource } from "@/lib/sensorOrigin";
import type { AnalyticsWater, DemoSensor } from "@/types/dashboard";

export function WaterSection({ water }: { water: AnalyticsWater }) {
  const [rangeHours, setRangeHours] = useState<AnalyticsHistoryRangeHours>(24);

  const domesticSeries24h = water.domestic_series_24h ?? water.domestic_series;
  const agSeries24h = water.ag_series_24h ?? water.ag_series;
  const domesticSeries168h = water.domestic_series_168h ?? [];
  const agSeries168h = water.ag_series_168h ?? [];
  const seriesForRange = <T extends { timestamp: Date }>(shortSeries: T[], longSeries: T[]): T[] => {
    const base = longSeries.length ? longSeries : shortSeries;
    return filterSeriesByHours(base, rangeHours);
  };

  const domesticRate = seriesForRange(domesticSeries24h, domesticSeries168h);
  const agRate = seriesForRange(agSeries24h, agSeries168h);
  const usageSeries: ChartSeriesConfig[] = [{ label: "Domestic", series: domesticRate, color: COLORS.domestic }];
  if (agRate.length) {
    usageSeries.push({ label: "Agriculture", series: agRate, color: COLORS.ag });
  }

  return (
    <CollapsibleCard
      title="Water"
      description="Track domestic and agricultural usage alongside reservoir depth sensors."
      defaultOpen
      bodyClassName="space-y-4"
      actions={<AnalyticsRangeSelect value={rangeHours} onChange={setRangeHours} />}
    >
      <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-[2fr_1fr]">
        <AnalyticsChart title={`Usage — ${historyRangeLabel(rangeHours)}`} series={usageSeries} unit="gpm" rangeHours={rangeHours} />
        <Card>
          <CardHeader>
 <CardTitle className="text-sm uppercase tracking-wide text-muted-foreground">
              Totals
            </CardTitle>
          </CardHeader>
          <CardContent>
 <dl className="grid grid-cols-2 gap-2 text-sm text-muted-foreground">
              <Metric label="Domestic 24h" value={formatGallons(water.domestic_gal_24h ?? 0)} />
              <Metric label="Domestic 7d" value={formatGallons(water.domestic_gal_168h ?? 0)} />
              <Metric label="Ag 24h" value={formatGallons(water.ag_gal_24h ?? 0)} />
              <Metric label="Ag 7d" value={formatGallons(water.ag_gal_168h ?? 0)} />
            </dl>
          </CardContent>
        </Card>
      </div>

      <WaterDepthSection rangeHours={rangeHours} />
    </CollapsibleCard>
  );
}

function isWaterDepthSensor(sensor: DemoSensor): boolean {
  if (sensorSource(sensor) === "forecast_points") return false;

  const config = sensor.config ?? {};
  const metric = String(config.metric ?? "").toLowerCase();
  const category = String(config.category ?? "").toLowerCase();
  const unit = String(sensor.unit ?? "").toLowerCase();
  const name = String(sensor.name ?? "").toLowerCase();
  const type = String(sensor.type ?? "").toLowerCase();

  if (type === "water_level" || type.includes("water_level")) return true;
  if (metric.includes("water_level") || metric.includes("reservoir_depth") || metric.includes("depth")) return true;
  if (category === "water" && (unit === "ft" || unit === "in" || unit === "m" || unit === "cm")) return true;
  return name.includes("reservoir") && name.includes("depth");
}

type DepthGaugeDatum = {
  sensor: DemoSensor;
  nodeLabel: string;
  current: number | null;
  min: number | null;
  max: number | null;
  fullScaleMax: number;
  deltaWindow: number | null;
};

function defaultFullScaleMaxForUnit(unitRaw: string): number {
  const unit = unitRaw.trim().toLowerCase();
  if (unit === "ft") return 15;
  if (unit === "in") return 180;
  if (unit === "m") return 5;
  if (unit === "cm") return 500;
  return 15;
}

function clampNumber(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function pickFirstFinite(points: Array<{ value: number | null }>): number | null {
  for (const p of points) {
    if (p.value == null || !Number.isFinite(p.value)) continue;
    return p.value;
  }
  return null;
}

function pickLastFinite(points: Array<{ value: number | null }>): number | null {
  for (let idx = points.length - 1; idx >= 0; idx -= 1) {
    const v = points[idx]?.value ?? null;
    if (v == null || !Number.isFinite(v)) continue;
    return v;
  }
  return null;
}

function minMaxFinite(points: Array<{ value: number | null }>): { min: number | null; max: number | null } {
  let min: number | null = null;
  let max: number | null = null;
  for (const p of points) {
    const v = p.value;
    if (v == null || !Number.isFinite(v)) continue;
    min = min == null ? v : Math.min(min, v);
    max = max == null ? v : Math.max(max, v);
  }
  return { min, max };
}

function formatDepth(value: number | null, unit: string): string {
  if (value == null || !Number.isFinite(value)) return "—";
  const decimals = unit.toLowerCase() === "ft" ? 2 : 1;
  return `${value.toFixed(decimals)} ${unit}`.trim();
}

function DepthGaugeCard({ datum }: { datum: DepthGaugeDatum }) {
  const unit = datum.sensor.unit?.trim() || "";
  const current = datum.current;
  const fullMax = datum.fullScaleMax;
  const frac = current == null || fullMax <= 0 ? 0 : clampNumber(current / fullMax, 0, 1);
  const fillPct = frac * 100;
  const rangeStart = datum.min == null ? null : clampNumber(datum.min / fullMax, 0, 1) * 100;
  const rangeEnd = datum.max == null ? null : clampNumber(datum.max / fullMax, 0, 1) * 100;

  const delta = datum.deltaWindow;
  const deltaLabel =
    delta == null || !Number.isFinite(delta)
      ? "—"
      : `${delta >= 0 ? "+" : ""}${delta.toFixed(unit.toLowerCase() === "ft" ? 2 : 1)} ${unit}`;
  const deltaTone =
    delta == null
 ? "text-muted-foreground"
      : Math.abs(delta) < 1e-6
 ? "text-muted-foreground"
        : delta > 0
 ? "text-emerald-700"
 : "text-rose-700";

  return (
    <Card className="p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
 <p className="truncate text-sm font-semibold text-foreground" title={datum.sensor.name}>
            {datum.sensor.name}
          </p>
 <p className="mt-0.5 truncate text-xs text-muted-foreground" title={datum.nodeLabel}>
            {datum.nodeLabel}
          </p>
        </div>
        <div className="shrink-0 text-right">
 <p className="text-lg font-semibold text-foreground">
            {formatDepth(current, unit)}
          </p>
          <p className={`mt-0.5 text-xs font-semibold ${deltaTone}`} title="Change over the chart window (approx)">
            Δ {deltaLabel}
          </p>
        </div>
      </div>

      <div className="mt-4 grid grid-cols-[80px_1fr] gap-4">
        <div className="relative h-40">
 <div className="absolute inset-0 rounded-xl bg-gradient-to-b from-indigo-50 to-white" />
          <div className="absolute inset-0 rounded-xl border border-border" />

 <div className="absolute inset-x-3 top-3 bottom-3 overflow-hidden rounded-lg bg-muted">
            <div
              className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-sky-500 via-indigo-500 to-indigo-400 opacity-90"
              style={{ height: `${fillPct}%` }}
            />
            <div
              className="absolute inset-x-0 bottom-0 h-8 opacity-60"
              style={{
                transform: `translateY(${-(fillPct / 100) * 8}px)`,
                background:
                  "radial-gradient(circle at 30% 30%, rgb(255 255 255 / 0.45), transparent 60%), radial-gradient(circle at 70% 60%, rgb(255 255 255 / 0.25), transparent 60%)",
              }}
            />

            {rangeStart != null && rangeEnd != null ? (
              <div
 className="absolute inset-x-1 rounded-sm border border-indigo-300 bg-indigo-200/40"
                style={{
                  bottom: `${rangeStart}%`,
                  height: `${Math.max(2, rangeEnd - rangeStart)}%`,
                }}
                title="Observed min/max in window"
              />
            ) : null}
          </div>

 <div className="pointer-events-none absolute inset-y-3 right-1 flex flex-col justify-between text-[10px] text-muted-foreground">
            <span>{fullMax.toFixed(unit.toLowerCase() === "ft" ? 1 : 0)}</span>
            <span>{(fullMax * 0.75).toFixed(unit.toLowerCase() === "ft" ? 1 : 0)}</span>
            <span>{(fullMax * 0.5).toFixed(unit.toLowerCase() === "ft" ? 1 : 0)}</span>
            <span>{(fullMax * 0.25).toFixed(unit.toLowerCase() === "ft" ? 1 : 0)}</span>
            <span>0</span>
          </div>
        </div>

        <div className="min-w-0 space-y-2">
 <div className="grid grid-cols-2 gap-2 text-xs text-muted-foreground">
            <Card className="rounded-lg gap-0 bg-card-inset px-3 py-2">
 <p className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">Min</p>
 <p className="mt-0.5 font-semibold text-foreground">{formatDepth(datum.min, unit)}</p>
            </Card>
            <Card className="rounded-lg gap-0 bg-card-inset px-3 py-2">
 <p className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">Max</p>
 <p className="mt-0.5 font-semibold text-foreground">{formatDepth(datum.max, unit)}</p>
            </Card>
          </div>
 <p className="text-xs text-muted-foreground">
            Full-scale view (0 → {fullMax.toFixed(unit.toLowerCase() === "ft" ? 1 : 0)} {unit}) to avoid over-zooming.
          </p>
        </div>
      </div>
    </Card>
  );
}

function WaterDepthSection({ rangeHours }: { rangeHours: AnalyticsHistoryRangeHours }) {
  const { sensors, nodeLabelsById, isLoading, error } = useAnalyticsData();

  const depthSensors = useMemo(() => sensors.filter(isWaterDepthSensor), [sensors]);
  const depthSensorIds = useMemo(
    () => depthSensors.map((sensor) => sensor.sensor_id).filter(Boolean).sort(),
    [depthSensors],
  );

  const intervalSeconds = historyIntervalSeconds(rangeHours);
  const { data: depthSeries, isLoading: metricsLoading, error: metricsError } = useMetricsQuery({
    sensorIds: depthSensorIds,
    rangeHours,
    interval: intervalSeconds,
    enabled: depthSensorIds.length > 0,
    refetchInterval: 60_000,
  });

  const sensorById = useMemo(() => new Map(depthSensors.map((sensor) => [sensor.sensor_id, sensor])), [depthSensors]);

  const chartSeries = useMemo<ChartSeriesConfig[]>(() => {
    if (!depthSeries?.length) return [];
    return depthSeries
      .flatMap((entry, index) => {
        const sensor = sensorById.get(entry.sensor_id);
        const label = sensor
          ? `${nodeLabelsById.get(sensor.node_id) ?? sensor.node_id} — ${sensor.name}`
          : entry.label ?? entry.sensor_id;
        const series = entry.points;
        if (!series.length) return [];
        return [{
          label,
          series,
          color: BATTERY_COLORS[index % BATTERY_COLORS.length],
        }];
      })
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [depthSeries, nodeLabelsById, sensorById]);

  const unit = depthSensors[0]?.unit?.trim() || "ft";

  const gauges = useMemo<DepthGaugeDatum[]>(() => {
    const seriesById = new Map((depthSeries ?? []).map((entry) => [entry.sensor_id, entry.points]));
    return depthSensors
      .map((sensor) => {
        const points = seriesById.get(sensor.sensor_id) ?? [];
        const { min, max } = minMaxFinite(points);
        const current = sensor.latest_value != null && Number.isFinite(sensor.latest_value)
          ? sensor.latest_value
          : pickLastFinite(points);

        const first = pickFirstFinite(points);
        const deltaWindow =
          current != null && first != null && Number.isFinite(current) && Number.isFinite(first)
            ? current - first
            : null;

        const maxObserved = Math.max(max ?? 0, current ?? 0);
        const baseMax = defaultFullScaleMaxForUnit(sensor.unit ?? "");
        const fullScaleMax = Math.max(baseMax, maxObserved * 1.25, 1);

        return {
          sensor,
          nodeLabel: nodeLabelsById.get(sensor.node_id) ?? sensor.node_id,
          current,
          min,
          max,
          fullScaleMax,
          deltaWindow,
        };
      })
      .sort((a, b) => {
        const nodeCmp = a.nodeLabel.localeCompare(b.nodeLabel);
        if (nodeCmp !== 0) return nodeCmp;
        return a.sensor.name.localeCompare(b.sensor.name);
      });
  }, [depthSensors, depthSeries, nodeLabelsById]);

  const loading = isLoading || metricsLoading;
  const toMessage = (error: unknown) => (error instanceof Error ? error.message : typeof error === "string" ? error : null);
  const errorMessage = toMessage(error) ?? toMessage(metricsError);

  if (loading) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Water depths</CardTitle>
        </CardHeader>
        <CardContent>
 <p className="text-sm text-muted-foreground">Loading depth sensors…</p>
        </CardContent>
      </Card>
    );
  }

  if (errorMessage) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Water depths</CardTitle>
        </CardHeader>
        <CardContent>
 <p className="text-sm text-rose-700">{errorMessage}</p>
        </CardContent>
      </Card>
    );
  }

  if (!depthSensors.length) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Water depths</CardTitle>
        </CardHeader>
        <CardContent>
 <p className="text-sm text-muted-foreground">No depth sensors detected yet.</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-4">
      <AnalyticsChart
        title={`Water depths — ${historyRangeLabel(rangeHours).toLowerCase()}`}
        series={chartSeries}
        unit={unit}
        rangeHours={rangeHours}
      />
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Live reservoir depths</CardTitle>
 <p className="text-sm text-muted-foreground">
            Side-by-side full-range gauges (0 → full scale) so small fluctuations don&apos;t dominate the view.
          </p>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
            {gauges.map((datum) => (
              <DepthGaugeCard key={datum.sensor.sensor_id} datum={datum} />
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
