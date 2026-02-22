"use client";

import { useMemo, useState } from "react";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import {
  useForecastStatusQuery,
  useMetricsQuery,
  usePvForecastConfigQuery,
  usePvForecastHourlyQuery,
} from "@/lib/queries";
import { sensorSource } from "@/lib/sensorOrigin";
import { formatDateTimeForTimeZone, useControllerTimeZone } from "@/lib/siteTime";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  AnalyticsRangeSelect,
  type AnalyticsHistoryRangeHours,
  historyIntervalSeconds,
  useBaseChartOptions,
  ZoomableLineChart,
} from "@/features/analytics/components/AnalyticsShared";
import { useAnalyticsData } from "@/features/analytics/hooks/useAnalyticsData";
import { buildPvWindowedSeries } from "@/features/analytics/utils/pvForecast";

export function PvForecastSection() {
  const timeZone = useControllerTimeZone();
  const { nodesById, sensorsByNodeId, sensors, isLoading, error } = useAnalyticsData();
  const statusQuery = useForecastStatusQuery();
  const [rangeHours, setRangeHours] = useState<AnalyticsHistoryRangeHours>(24);

  const renogyNodeIds = useMemo(() => {
    const ids = new Set<string>();
    sensors.forEach((sensor) => {
      if (sensorSource(sensor) === "renogy_bt2") ids.add(sensor.node_id);
    });
    return Array.from(ids).filter(Boolean).sort();
  }, [sensors]);

  const forecastSolarStatus = statusQuery.data?.providers?.["Forecast.Solar"];
  const errorMessage = error instanceof Error ? error.message : null;

  return (
    <Card>
      <CardHeader>
        <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
          <div className="space-y-1">
            <CardTitle className="text-lg">PV forecast vs measured</CardTitle>
 <p className="text-sm text-muted-foreground">
              Compare Renogy measured PV power (W) against persisted Forecast.Solar predictions. The range selector is
              day-aligned: it always includes the entire current day and can include prior days.
            </p>
          </div>
          <AnalyticsRangeSelect value={rangeHours} onChange={setRangeHours} />
        </div>
      </CardHeader>
      <CardContent>
 <div className="flex flex-col gap-2 text-xs text-muted-foreground md:flex-row md:items-center md:justify-between">
        <div>
          Forecast.Solar status {forecastSolarStatus?.status ?? "unknown"}
          {forecastSolarStatus?.last_seen
            ? ` · Last poll ${formatDateTimeForTimeZone(new Date(forecastSolarStatus.last_seen), timeZone, {
                dateStyle: "medium",
                timeStyle: "short",
              })}`
            : ""}
        </div>
        <div>Forecast.Solar Public horizon is limited (today + next day).</div>
      </div>

        {errorMessage ? (
          <p className="mt-4 text-sm text-rose-600">Failed to load PV nodes: {errorMessage}</p>
        ) : isLoading ? (
 <p className="mt-4 text-sm text-muted-foreground">Loading PV nodes…</p>
        ) : renogyNodeIds.length === 0 ? (
 <p className="mt-4 text-sm text-muted-foreground">
            No solar charge controller telemetry detected yet. Apply a Renogy BT‑2 preset on a node to enable PV sensors.
          </p>
        ) : (
          <div className="mt-4 space-y-3">
            {renogyNodeIds.map((nodeId, idx) => (
              <PvForecastNodePanel
                key={nodeId}
                node={nodesById.get(nodeId) ?? null}
                nodeId={nodeId}
                sensors={sensorsByNodeId.get(nodeId) ?? []}
                defaultOpen={renogyNodeIds.length === 1 && idx === 0}
                rangeHours={rangeHours}
              />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function PvForecastNodePanel({
  node,
  nodeId,
  sensors,
  defaultOpen,
  rangeHours,
}: {
  node: DemoNode | null;
  nodeId: string;
  sensors: DemoSensor[];
  defaultOpen: boolean;
  rangeHours: AnalyticsHistoryRangeHours;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const timeZone = useControllerTimeZone();
  const baseChartOptions = useBaseChartOptions(rangeHours);

  const pvConfigQuery = usePvForecastConfigQuery(nodeId, { enabled: open });
  const configured = Boolean(pvConfigQuery.data?.enabled);

  const pvSensor = useMemo(() => pickRenogyPvSensor(sensors), [sensors]);

  // eslint-disable-next-line react-hooks/exhaustive-deps -- rangeHours/timeZone are intentional invalidation triggers to recompute "now" when the view changes
  const windowNow = useMemo(() => new Date(), [rangeHours, timeZone]);

  const historyHours = rangeHours;
  const forecastHours = 24;
  const intervalSeconds = historyIntervalSeconds(rangeHours);

  const measuredQuery = useMetricsQuery({
    sensorIds: pvSensor ? [pvSensor.sensor_id] : [],
    rangeHours: historyHours,
    interval: intervalSeconds,
    enabled: open && Boolean(pvSensor?.sensor_id),
    refetchInterval: 30_000,
  });

  const forecastQuery = usePvForecastHourlyQuery(nodeId, forecastHours, {
    enabled: open && configured,
    historyHours,
  });

  const forecastSeries = forecastQuery.data?.metrics?.pv_power_w ?? null;
  const measuredPoints = useMemo(() => measuredQuery.data?.[0]?.points ?? [], [measuredQuery.data]);

  const { startMs, endMs, measuredWindowPoints, forecastWindowPoints } = useMemo(() => {
    return buildPvWindowedSeries({
      rangeHours,
      timeZone,
      measuredPoints,
      forecastPoints: forecastSeries?.points ?? [],
      now: windowNow,
    });
  }, [forecastSeries, measuredPoints, rangeHours, timeZone, windowNow]);

  const chartData = useMemo(() => {
    const datasets: Array<{
      label: string;
      data: Array<{ x: string | Date; y: number | null }>;
      borderColor: string;
      backgroundColor: string;
      borderWidth: number;
      pointRadius: number;
      pointHoverRadius: number;
      tension: number;
      borderDash?: number[];
    }> = [];
    if (measuredWindowPoints.length) {
      datasets.push({
        label: "Measured PV (Renogy)",
        data: measuredWindowPoints.map((point) => ({ x: point.timestamp, y: point.value })),
        borderColor: "#16a34a",
        backgroundColor: "#16a34a",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
      });
    }
    if (forecastWindowPoints.length) {
      datasets.push({
        label: "Forecast PV (Forecast.Solar)",
        data: forecastWindowPoints.map((point) => ({ x: new Date(point.timestamp), y: point.value })),
        borderColor: "#16a34a",
        backgroundColor: "#16a34a",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        borderDash: [6, 4],
      });
    }
    return { datasets };
  }, [forecastWindowPoints, measuredWindowPoints]);

  const hasChartData = useMemo(() => {
    return (
      measuredWindowPoints.some((point) => point.value != null && Number.isFinite(point.value)) ||
      forecastWindowPoints.some((point) => point.value != null && Number.isFinite(point.value))
    );
  }, [forecastWindowPoints, measuredWindowPoints]);

  const nodeName = node?.name ?? nodeId;
  const statusLabel = node ? formatNodeStatusLabel(node.status ?? "unknown", node.last_seen) : "unknown";
  const configChip = pvConfigQuery.data ? (
    <span
      className={`rounded-full px-3 py-1 text-xs font-semibold ${
        pvConfigQuery.data.enabled
          ? "bg-success-surface text-success-surface-foreground"
          : "bg-card-inset text-card-foreground"
      }`}
    >
      {pvConfigQuery.data.enabled ? "Forecast enabled" : "Forecast disabled"}
    </span>
  ) : null;

  return (
    <CollapsibleCard
      title={nodeName}
      description={statusLabel}
      actions={<>{configChip}{pvConfigQuery.isLoading && open ? <span className="text-xs text-muted-foreground">Loading…</span> : null}</>}
      open={open}
      onOpenChange={setOpen}
      density="sm"
    >
          {!configured ? (
 <Card className="rounded-lg gap-0 bg-card-inset p-4 text-sm text-muted-foreground">
              PV forecast is not enabled for {nodeName}. Configure it in{" "}
              <a
                className="font-semibold text-indigo-600 hover:underline"
                href={`/setup?pvNode=${encodeURIComponent(nodeId)}#pv-forecast`}
              >
                Setup Center
              </a>
              .
            </Card>
          ) : measuredQuery.error || forecastQuery.error ? (
            <p className="text-sm text-rose-600">
              Failed to load PV chart:{" "}
              {(measuredQuery.error instanceof Error && measuredQuery.error.message) ||
                (forecastQuery.error instanceof Error && forecastQuery.error.message) ||
                "Unknown error"}
            </p>
          ) : measuredQuery.isLoading || forecastQuery.isLoading ? (
 <p className="text-sm text-muted-foreground">Loading PV power (W) series…</p>
          ) : !hasChartData ? (
            <div className="space-y-2">
 <p className="text-sm text-muted-foreground">
                No PV forecast or measured samples found in this range yet.
              </p>
 <p className="text-xs text-muted-foreground">
                Try a wider range, confirm the node is publishing PV telemetry, and ensure Forecast.Solar is enabled.
              </p>
 <p className="text-xs text-muted-foreground">
                {forecastQuery.data?.issued_at
                  ? `Latest forecast issued ${formatDateTimeForTimeZone(
                      new Date(forecastQuery.data.issued_at),
                      timeZone,
                      {
                        dateStyle: "medium",
                        timeStyle: "short",
                      },
                    )}.`
                  : "Forecast issue time unknown."}
              </p>
            </div>
          ) : (
            <>
              <ZoomableLineChart
                wrapperClassName="h-[400px]"
                data={chartData}
                options={{
                  ...baseChartOptions,
                  plugins: {
                    ...baseChartOptions.plugins,
                    legend: { display: true, position: "bottom" as const },
                  },
                  scales: {
                    x: {
                      ...baseChartOptions.scales.x,
                      min: startMs,
                      max: endMs,
                    },
                    y: {
                      ...baseChartOptions.scales.y,
                      beginAtZero: true,
                      title: { display: true, text: forecastSeries?.unit ?? "W" },
                    },
                  },
                }}
              />

 <p className="mt-2 text-xs text-muted-foreground">
                {forecastQuery.data?.issued_at
                  ? `Latest forecast issued ${formatDateTimeForTimeZone(
                      new Date(forecastQuery.data.issued_at),
                      timeZone,
                      {
                        dateStyle: "medium",
                        timeStyle: "short",
                      },
                    )} · Past hours use stored historical forecasts.`
                  : "Forecast issue time unknown."}
              </p>
            </>
          )}
    </CollapsibleCard>
  );
}

function isPvPowerSensor(sensor: DemoSensor): boolean {
  const config = sensor.config ?? {};
  const metric = String(config.metric ?? "").toLowerCase();
  const category = String(config.category ?? "").toLowerCase();
  const unit = String(sensor.unit ?? "").toLowerCase();
  const name = String(sensor.name ?? "").toLowerCase();

  if (metric.includes("pv_power")) {
    return true;
  }
  if (category === "solar" && unit === "w") {
    return true;
  }
  return name.includes("pv power") && unit.startsWith("w");
}

function pickRenogyPvSensor(sensors: DemoSensor[]): DemoSensor | null {
  const renogySensors = sensors.filter((sensor) => sensorSource(sensor) === "renogy_bt2");
  const candidates = renogySensors.filter(isPvPowerSensor);
  const exact = candidates.find((sensor) => String(sensor.config?.metric ?? "") === "pv_power_w");
  return exact ?? candidates[0] ?? null;
}
