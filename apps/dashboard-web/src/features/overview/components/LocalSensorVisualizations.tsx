"use client";

import Link from "next/link";
import { useMemo, useState, useSyncExternalStore } from "react";
import { Card } from "@/components/ui/card";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import LoadingState from "@/components/LoadingState";
import { useMetricsQuery, useNodesQuery, useSensorsQuery } from "@/lib/queries";
import { formatSensorValueWithUnit } from "@/lib/sensorFormat";
import NodeButton from "@/features/nodes/components/NodeButton";
import { nonLocalSensorBadgeMeta } from "@/features/sensors/components/SensorOriginBadge";
import LocalSensorsConfigModal, {
  type OverviewLocalSensorsPrefsV1,
  type OverviewLocalSensorsPrefsV2,
} from "@/features/overview/components/LocalSensorsConfigModal";
import type { DemoSensor, TrendSeriesEntry, TrendSeriesPoint } from "@/types/dashboard";
import { HighchartsPanel } from "@/components/charts/HighchartsPanel";
import { Select } from "@/components/ui/select";
import { createSparklineOptions } from "@/lib/chartFactories";

type RangeHoursOption = 1 | 6 | 24;

type HoverCell = {
  sensorId: string;
  bucketIndex: number;
} | null;

function isLocalSensor(sensor: DemoSensor): boolean {
  return nonLocalSensorBadgeMeta(sensor) == null;
}

function bucketMinutesForRange(rangeHours: RangeHoursOption): number {
  if (rangeHours <= 1) return 5;
  if (rangeHours <= 6) return 15;
  return 60;
}

function intervalSecondsForRange(rangeHours: RangeHoursOption): number {
  if (rangeHours <= 1) return 30;
  if (rangeHours <= 6) return 60;
  return 300;
}

function lastNonNull(points: TrendSeriesPoint[]): TrendSeriesPoint | null {
  for (let idx = points.length - 1; idx >= 0; idx -= 1) {
    const point = points[idx];
    if (point?.value == null || !Number.isFinite(point.value)) continue;
    return point;
  }
  return null;
}

function minMax(values: Array<number | null>): { min: number | null; max: number | null } {
  let min: number | null = null;
  let max: number | null = null;
  for (const v of values) {
    if (v == null || !Number.isFinite(v)) continue;
    min = min == null ? v : Math.min(min, v);
    max = max == null ? v : Math.max(max, v);
  }
  return { min, max };
}

function rgba(rgb: string, alpha: number): string {
  const a = Math.max(0, Math.min(1, alpha));
  return `rgb(${rgb} / ${a})`;
}

function cellColor({
  value,
  min,
  max,
}: {
  value: number | null;
  min: number | null;
  max: number | null;
}): string {
  if (value == null || min == null || max == null || !Number.isFinite(min) || !Number.isFinite(max)) {
    return rgba("229 231 235", 1);
  }
  const span = max - min;
  const norm = span <= 1e-9 ? 0.75 : Math.max(0, Math.min(1, (value - min) / span));
  const base = "79 70 229"; // indigo-600
  const alpha = 0.12 + norm * 0.78;
  return rgba(base, alpha);
}

function formatTimeLabel(ts: Date): string {
  return new Intl.DateTimeFormat(undefined, { hour: "2-digit", minute: "2-digit" }).format(ts);
}

function buildBuckets({
  points,
  start,
  end,
  bucketMinutes,
}: {
  points: TrendSeriesPoint[];
  start: Date;
  end: Date;
  bucketMinutes: number;
}): Array<number | null> {
  const bucketMs = bucketMinutes * 60 * 1000;
  const bucketCount = Math.max(1, Math.ceil((end.getTime() - start.getTime()) / bucketMs));
  const buckets: Array<number | null> = Array.from({ length: bucketCount }, () => null);

  for (const point of points) {
    if (!point?.timestamp) continue;
    if (point.value == null || !Number.isFinite(point.value)) continue;
    const ts = point.timestamp instanceof Date ? point.timestamp : new Date(point.timestamp);
    const idx = Math.floor((ts.getTime() - start.getTime()) / bucketMs);
    if (idx < 0 || idx >= bucketCount) continue;
    buckets[idx] = point.value;
  }

  return buckets;
}


function seriesForSensor(series: TrendSeriesEntry[], sensorId: string): TrendSeriesEntry | null {
  return series.find((s) => s.sensor_id === sensorId) ?? null;
}

const OVERVIEW_LOCAL_SENSORS_PREFS_KEY = "farmdashboard.overview.localSensors.v1";
const OVERVIEW_LOCAL_SENSORS_PREFS_EVENT = "farmdashboard.overview.localSensors.changed";

type OverviewLocalSensorsPrefs = OverviewLocalSensorsPrefsV1 | OverviewLocalSensorsPrefsV2;

function parseOverviewLocalSensorsPrefs(raw: string | null): OverviewLocalSensorsPrefs | null {
  try {
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return null;
    const version = (parsed as { version?: unknown }).version;
    if (version === 1) {
      const order = (parsed as { order?: unknown }).order;
      const hidden = (parsed as { hidden?: unknown }).hidden;
      if (!Array.isArray(order) || !Array.isArray(hidden)) return null;
      return {
        version: 1,
        order: order.map((v) => String(v)).filter(Boolean),
        hidden: hidden.map((v) => String(v)).filter(Boolean),
      };
    }

    if (version === 2) {
      const nodeOrder = (parsed as { node_order?: unknown }).node_order;
      const sensorOrderByNode = (parsed as { sensor_order_by_node?: unknown }).sensor_order_by_node;
      const hidden = (parsed as { hidden?: unknown }).hidden;
      if (!Array.isArray(nodeOrder) || !Array.isArray(hidden)) return null;
      if (!sensorOrderByNode || typeof sensorOrderByNode !== "object") return null;
      const normalizedByNode: Record<string, string[]> = {};
      Object.entries(sensorOrderByNode as Record<string, unknown>).forEach(([nodeId, value]) => {
        if (!Array.isArray(value)) return;
        normalizedByNode[String(nodeId)] = value.map((v) => String(v)).filter(Boolean);
      });
      return {
        version: 2,
        node_order: nodeOrder.map((v) => String(v)).filter(Boolean),
        sensor_order_by_node: normalizedByNode,
        hidden: hidden.map((v) => String(v)).filter(Boolean),
      };
    }

    return null;
  } catch {
    return null;
  }
}

function writeOverviewLocalSensorsPrefs(value: OverviewLocalSensorsPrefsV2 | null): void {
  if (typeof window === "undefined") return;
  try {
    if (value == null) {
      window.localStorage.removeItem(OVERVIEW_LOCAL_SENSORS_PREFS_KEY);
    } else {
      window.localStorage.setItem(OVERVIEW_LOCAL_SENSORS_PREFS_KEY, JSON.stringify(value));
    }
    window.dispatchEvent(new Event(OVERVIEW_LOCAL_SENSORS_PREFS_EVENT));
  } catch {
    // ignore quota/private-mode failures
  }
}

function prefsSnapshot(): string {
  if (typeof window === "undefined") return "";
  return window.localStorage.getItem(OVERVIEW_LOCAL_SENSORS_PREFS_KEY) ?? "";
}

function subscribePrefs(callback: () => void): () => void {
  if (typeof window === "undefined") return () => {};
  const handler = () => callback();
  window.addEventListener("storage", handler);
  window.addEventListener(OVERVIEW_LOCAL_SENSORS_PREFS_EVENT, handler as EventListener);
  return () => {
    window.removeEventListener("storage", handler);
    window.removeEventListener(OVERVIEW_LOCAL_SENSORS_PREFS_EVENT, handler as EventListener);
  };
}

export default function LocalSensorVisualizations() {
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();

  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);

  const [rangeHours, setRangeHours] = useState<RangeHoursOption>(6);
  const [sensorLimit, setSensorLimit] = useState<number>(12);
  const [hover, setHover] = useState<HoverCell>(null);
  const [configOpen, setConfigOpen] = useState(false);

  const prefsRaw = useSyncExternalStore(subscribePrefs, prefsSnapshot, () => "");
  const prefs = useMemo(() => parseOverviewLocalSensorsPrefs(prefsRaw), [prefsRaw]);

  const nodesById = useMemo(() => {
    const map = new Map<string, (typeof nodes)[number]>();
    for (const node of nodes) map.set(node.id, node);
    return map;
  }, [nodes]);

  const localSensors = useMemo(() => {
    return sensors.filter(isLocalSensor);
  }, [sensors]);

  const sensorById = useMemo(() => {
    const map = new Map<string, DemoSensor>();
    for (const sensor of localSensors) map.set(sensor.sensor_id, sensor);
    return map;
  }, [localSensors]);

  const defaultNodeOrder = useMemo(() => {
    const nodeIdsWithSensors = new Set(localSensors.map((sensor) => sensor.node_id));
    const orderedFromNodes = nodes
      .map((node) => node.id)
      .filter((id) => nodeIdsWithSensors.has(id));
    const unknownNodes = Array.from(nodeIdsWithSensors).filter((id) => !orderedFromNodes.includes(id));
    unknownNodes.sort();
    return [...orderedFromNodes, ...unknownNodes];
  }, [localSensors, nodes]);

  const defaultSensorOrderByNode = useMemo(() => {
    const byNode: Record<string, string[]> = {};
    localSensors.forEach((sensor) => {
      const nodeId = sensor.node_id;
      if (!nodeId) return;
      if (!byNode[nodeId]) byNode[nodeId] = [];
      byNode[nodeId].push(sensor.sensor_id);
    });
    return byNode;
  }, [localSensors]);

  const ordering = useMemo(() => {
    const validSensorIds = new Set(localSensors.map((sensor) => sensor.sensor_id));
    const validNodeIds = new Set(defaultNodeOrder);

    const prefsNodeOrder: string[] = [];
    const prefsSensorOrderByNode: Record<string, string[]> = {};
    if (prefs?.version === 2) {
      prefs.node_order.forEach((nodeId) => {
        if (!validNodeIds.has(nodeId)) return;
        if (!prefsNodeOrder.includes(nodeId)) prefsNodeOrder.push(nodeId);
      });

      Object.entries(prefs.sensor_order_by_node).forEach(([nodeId, ids]) => {
        if (!validNodeIds.has(nodeId)) return;
        const unique: string[] = [];
        const seen = new Set<string>();
        ids.forEach((id) => {
          if (!validSensorIds.has(id)) return;
          if (seen.has(id)) return;
          const sensor = sensorById.get(id);
          if (!sensor || sensor.node_id !== nodeId) return;
          seen.add(id);
          unique.push(id);
        });
        prefsSensorOrderByNode[nodeId] = unique;
      });
    } else if (prefs?.version === 1) {
      const seenNodes = new Set<string>();
      (prefs.order ?? []).forEach((id) => {
        if (!validSensorIds.has(id)) return;
        const sensor = sensorById.get(id);
        if (!sensor) return;
        const nodeId = sensor.node_id;
        if (!nodeId) return;
        if (!seenNodes.has(nodeId)) {
          seenNodes.add(nodeId);
          prefsNodeOrder.push(nodeId);
        }
        const list = prefsSensorOrderByNode[nodeId] ?? [];
        if (!list.includes(id)) list.push(id);
        prefsSensorOrderByNode[nodeId] = list;
      });
    }

    const mergedNodeOrder: string[] = [];
    const seenMergedNodes = new Set<string>();
    const pushNode = (nodeId: string) => {
      if (!nodeId) return;
      if (!validNodeIds.has(nodeId)) return;
      if (seenMergedNodes.has(nodeId)) return;
      seenMergedNodes.add(nodeId);
      mergedNodeOrder.push(nodeId);
    };

    prefsNodeOrder.forEach(pushNode);
    defaultNodeOrder.forEach(pushNode);

    const mergedSensorOrderByNode: Record<string, string[]> = {};
    mergedNodeOrder.forEach((nodeId) => {
      const base = (prefsSensorOrderByNode[nodeId] ?? []).slice();
      const seen = new Set(base);
      (defaultSensorOrderByNode[nodeId] ?? []).forEach((id) => {
        if (!validSensorIds.has(id)) return;
        if (seen.has(id)) return;
        seen.add(id);
        base.push(id);
      });
      mergedSensorOrderByNode[nodeId] = base;
    });

    const orderIds = mergedNodeOrder.flatMap((nodeId) => mergedSensorOrderByNode[nodeId] ?? []);
    return { nodeOrder: mergedNodeOrder, sensorOrderByNode: mergedSensorOrderByNode, orderIds, validSensorIds };
  }, [defaultNodeOrder, defaultSensorOrderByNode, localSensors, prefs, sensorById]);

  const effectiveOrderIds = ordering.orderIds;

  const hiddenSet = useMemo(() => {
    const set = new Set<string>();
    (prefs?.hidden ?? []).forEach((id) => {
      if (!ordering.validSensorIds.has(id)) return;
      set.add(id);
    });
    return set;
  }, [ordering.validSensorIds, prefs?.hidden]);

  const selectedSensorIds = useMemo(() => {
    const limit = Math.max(4, Math.min(24, Math.floor(sensorLimit)));
    return effectiveOrderIds.filter((id) => !hiddenSet.has(id)).slice(0, limit);
  }, [effectiveOrderIds, hiddenSet, sensorLimit]);

  const selectedSensors = useMemo(() => {
    return selectedSensorIds.map((id) => sensorById.get(id)).filter((sensor): sensor is DemoSensor => Boolean(sensor));
  }, [selectedSensorIds, sensorById]);

  const configInitialHidden = useMemo(
    () => effectiveOrderIds.filter((id) => hiddenSet.has(id)),
    [effectiveOrderIds, hiddenSet],
  );

  const intervalSeconds = intervalSecondsForRange(rangeHours);

  const metricsQuery = useMetricsQuery({
    sensorIds: selectedSensorIds,
    rangeHours,
    interval: intervalSeconds,
    enabled: selectedSensorIds.length > 0,
    refetchInterval: 30_000,
  });

  const metricsSeries = useMemo(() => metricsQuery.data ?? [], [metricsQuery.data]);

  // eslint-disable-next-line react-hooks/exhaustive-deps -- dataUpdatedAt is an intentional invalidation trigger to recompute "now" when data refreshes
  const rangeEnd = useMemo(() => new Date(), [metricsQuery.dataUpdatedAt]);
  const rangeStart = useMemo(
    () => new Date(rangeEnd.getTime() - rangeHours * 60 * 60 * 1000),
    [rangeEnd, rangeHours],
  );
  const bucketMinutes = bucketMinutesForRange(rangeHours);

  const tapestryRows = useMemo(() => {
    return selectedSensors.map((sensor) => {
      const entry = seriesForSensor(metricsSeries, sensor.sensor_id);
      const points = entry?.points ?? [];
      const buckets = buildBuckets({ points, start: rangeStart, end: rangeEnd, bucketMinutes });
      const { min, max } = minMax(buckets);
      const latest = lastNonNull(points);
      return {
        sensor,
        nodeLabel: nodesById.get(sensor.node_id)?.name ?? sensor.node_id,
        unit: entry?.unit ?? sensor.unit ?? "",
        buckets,
        min,
        max,
        latestValue: latest?.value ?? null,
        latestAt: latest?.timestamp ?? null,
      };
    });
  }, [selectedSensors, metricsSeries, rangeStart, rangeEnd, bucketMinutes, nodesById]);

  const bucketCount = tapestryRows[0]?.buckets.length ?? Math.ceil((rangeHours * 60) / bucketMinutes);

  const hoverMeta = useMemo(() => {
    if (!hover) return null;
    const row = tapestryRows.find((r) => r.sensor.sensor_id === hover.sensorId);
    if (!row) return null;
    const bucketMs = bucketMinutes * 60 * 1000;
    const bucketStart = new Date(rangeStart.getTime() + hover.bucketIndex * bucketMs);
    const bucketEnd = new Date(bucketStart.getTime() + bucketMs);
    const value = row.buckets[hover.bucketIndex] ?? null;
    return {
      sensor: row.sensor,
      nodeLabel: row.nodeLabel,
      value,
      bucketLabel: `${formatTimeLabel(bucketStart)}–${formatTimeLabel(bucketEnd)}`,
    };
  }, [hover, tapestryRows, bucketMinutes, rangeStart]);

  const showLoading = nodesQuery.isLoading || sensorsQuery.isLoading || metricsQuery.isLoading;
  const showError = nodesQuery.error || sensorsQuery.error || metricsQuery.error;

  return (
    <CollapsibleCard
      title="Local sensors"
      description="Overview-only visualizations that highlight locally acquired telemetry (excluding forecast/public provider sensors)."
      actions={
        <div className="flex flex-wrap items-center gap-2">
 <label className="text-xs font-semibold text-muted-foreground">
            Range
            <Select
              className="ms-2 h-8 px-2 text-xs text-foreground"
              value={String(rangeHours)}
              onChange={(event) => setRangeHours(Number(event.target.value) as RangeHoursOption)}
            >
              <option value="1">Last 1h</option>
              <option value="6">Last 6h</option>
              <option value="24">Last 24h</option>
            </Select>
          </label>

 <label className="text-xs font-semibold text-muted-foreground">
            Sensors
            <Select
              className="ms-2 h-8 px-2 text-xs text-foreground"
              value={String(sensorLimit)}
              onChange={(event) => setSensorLimit(Number(event.target.value))}
            >
              <option value="8">8</option>
              <option value="12">12</option>
              <option value="16">16</option>
              <option value="24">24</option>
            </Select>
          </label>

          <NodeButton
            size="xs"
            type="button"
            onClick={() => setConfigOpen(true)}
            aria-label="Configure local sensors"
          >
            Configure
          </NodeButton>
        </div>
      }
    >

      {showError ? (
        <InlineBanner tone="danger" className="mt-4">
          Failed to load local sensor telemetry.
        </InlineBanner>
      ) : null}

      {showLoading ? (
        <div className="mt-6">
          <LoadingState label="Loading local sensor visualizations…" />
        </div>
      ) : null}

      {!showLoading && !showError && selectedSensors.length === 0 ? (
        <Card className="mt-4 rounded-lg gap-0 bg-card-inset px-3 py-2 text-sm text-card-foreground">
          No local sensors detected yet.
        </Card>
      ) : null}

      {!showLoading && !showError && selectedSensors.length > 0 ? (
        <div className="mt-6 grid gap-6 lg:grid-cols-2">
          <Card
            data-testid="telemetry-tapestry-card"
            className="gap-0 p-4 shadow-xs"
          >
            <div className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_280px] sm:items-start">
              <div className="min-w-0">
 <div className="text-sm font-semibold text-foreground">Telemetry tapestry</div>
 <div className="mt-1 text-xs text-muted-foreground">
                  Bucketed {bucketMinutes}m heatmap per sensor (low → high per-row).
                </div>
              </div>

              <div
                data-testid="telemetry-tapestry-details"
 className="min-h-[56px] rounded-lg border border-indigo-200 bg-indigo-50 px-3 py-2 text-xs text-indigo-900 shadow-xs"
              >
                {hoverMeta ? (
                  <div className="min-w-0">
                    <div className="truncate font-semibold">
                      {hoverMeta.sensor.name} <span className="font-normal">({hoverMeta.nodeLabel})</span>
                    </div>
                    <div className="mt-1 flex min-w-0 items-center justify-between gap-2">
                      <span className="min-w-0 truncate">{hoverMeta.bucketLabel}</span>
                      <span className="shrink-0 font-semibold">
                        {formatSensorValueWithUnit(hoverMeta.sensor, hoverMeta.value, "—")}
                      </span>
                    </div>
                  </div>
                ) : (
                  <div className="min-w-0">
                    <div className="truncate font-semibold">Hover cells for details</div>
 <div className="mt-1 truncate text-indigo-700/70">
                      Move over a heatmap cell to inspect that bucket.
                    </div>
                  </div>
                )}
              </div>
            </div>

            <div data-testid="telemetry-tapestry-body" className="mt-4 overflow-x-hidden">
 <div className="flex items-center justify-between text-[11px] text-muted-foreground">
                <span>Older</span>
                <span>Now</span>
              </div>

              <div data-testid="telemetry-tapestry-rows" className="mt-2 space-y-3">
                {tapestryRows.map((row) => {
                  const id = row.sensor.sensor_id;
                  return (
                    <div key={id} className="flex min-w-0 flex-col gap-2 sm:flex-row sm:items-center sm:gap-3">
                      <div className="min-w-0 sm:w-[240px] sm:min-w-[240px]">
                        <div className="flex min-w-0 items-center justify-between gap-2">
                          <Link
                            href={`/sensors?sensor=${encodeURIComponent(id)}`}
 className="min-w-0 truncate text-sm font-semibold text-foreground hover:underline"
                            title={`${row.sensor.name} (${row.nodeLabel})`}
                          >
                            {row.sensor.name}
                          </Link>
 <span className="shrink-0 text-xs text-muted-foreground">
                            {row.unit || row.sensor.unit || ""}
                          </span>
                        </div>
 <div className="mt-0.5 flex min-w-0 items-center justify-between gap-2 text-xs text-muted-foreground">
                          <span className="min-w-0 truncate">{row.nodeLabel}</span>
 <span className="shrink-0 font-semibold text-foreground">
                            {formatSensorValueWithUnit(row.sensor, row.latestValue, "—")}
                          </span>
                        </div>
                      </div>

                      <div
                        className="grid h-9 min-w-0 flex-1 items-stretch gap-1"
                        style={{ gridTemplateColumns: `repeat(${bucketCount}, minmax(0, 1fr))` }}
                        role="img"
                        aria-label={`Telemetry heatmap row for ${row.sensor.name}`}
                      >
                        {row.buckets.map((value, idx) => {
                          const isHover = hover?.sensorId === id && hover.bucketIndex === idx;
                          const bg = cellColor({ value, min: row.min, max: row.max });
                          return (
                            <div
                              key={`${id}:${idx}`}
                              className={[
                                "rounded-sm border",
                                isHover ? "border-indigo-500" : "border-transparent",
                              ].join(" ")}
                              style={{ backgroundColor: bg }}
                              onMouseEnter={() => setHover({ sensorId: id, bucketIndex: idx })}
                              onMouseLeave={() => setHover((prev) => (prev?.sensorId === id ? null : prev))}
                              title={
                                value == null
                                  ? "No data"
                                  : `${row.sensor.name}: ${formatSensorValueWithUnit(row.sensor, value)}`
                              }
                            />
                          );
                        })}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </Card>

          <Card className="gap-0 p-4 shadow-xs">
            <div className="flex items-start justify-between gap-3">
              <div>
 <div className="text-sm font-semibold text-foreground">Sparkline mosaic</div>
 <div className="mt-1 text-xs text-muted-foreground">
                  Compact “small multiples” (no Trends charts) using the same telemetry window.
                </div>
              </div>
            </div>

            <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
              {selectedSensors.map((sensor) => {
                const entry = seriesForSensor(metricsSeries, sensor.sensor_id);
                const points = entry?.points ?? [];
                const numericPoints = points.map((p) => (p?.value != null && Number.isFinite(p.value) ? p.value : null));
                const { min, max } = minMax(numericPoints);
                const latest = lastNonNull(points);
                const sparkData: Array<[number, number | null]> = points.map((p) => {
                  const t = p?.timestamp instanceof Date ? p.timestamp.getTime() : new Date(p?.timestamp ?? 0).getTime();
                  return [t, p?.value ?? null];
                });
                const hasData = sparkData.some(([, v]) => v != null);
                const sparkOpts = hasData ? createSparklineOptions({ data: sparkData, color: "#4f46e5", height: 56 }) : null;

                return (
                  <Card
                    key={sensor.sensor_id}
                    className="gap-0 p-3 shadow-xs"
                  >
                    <div className="flex items-start justify-between gap-2">
                      <Link
                        href={`/sensors?sensor=${encodeURIComponent(sensor.sensor_id)}`}
 className="min-w-0 truncate text-sm font-semibold text-foreground hover:underline"
                        title={sensor.name}
                      >
                        {sensor.name}
                      </Link>
 <span className="shrink-0 text-[11px] font-semibold text-muted-foreground">
                        {formatSensorValueWithUnit(sensor, latest?.value ?? null, "—")}
                      </span>
                    </div>
 <div className="mt-0.5 truncate text-xs text-muted-foreground">
                      {nodesById.get(sensor.node_id)?.name ?? sensor.node_id}
                    </div>

 <Card className="mt-2 overflow-hidden rounded-lg gap-0 bg-gradient-to-b from-indigo-50/70 to-white">
                      {sparkOpts ? (
                        <HighchartsPanel
                          options={sparkOpts}
                          wrapperClassName="h-[56px] w-full"
                        />
                      ) : (
                        <div className="flex h-[56px] items-center justify-center text-xs text-muted-foreground">
                          No data
                        </div>
                      )}
                    </Card>

 <div className="mt-2 flex items-center justify-between text-[11px] text-muted-foreground">
                      <span>
                        Min{" "}
 <span className="font-semibold text-foreground">
                          {min == null ? "—" : formatSensorValueWithUnit(sensor, min, "—")}
                        </span>
                      </span>
                      <span>
                        Max{" "}
 <span className="font-semibold text-foreground">
                          {max == null ? "—" : formatSensorValueWithUnit(sensor, max, "—")}
                        </span>
                      </span>
                    </div>
                  </Card>
                );
              })}
            </div>
          </Card>
        </div>
      ) : null}

      {configOpen ? (
        <LocalSensorsConfigModal
          sensorLimit={sensorLimit}
          nodes={nodes}
          sensors={localSensors}
          initialOrder={effectiveOrderIds}
          initialHidden={configInitialHidden}
          onClose={() => setConfigOpen(false)}
          onSave={(next) => writeOverviewLocalSensorsPrefs(next)}
        />
      ) : null}
    </CollapsibleCard>
  );
}
