"use client";

import { useMemo, useState } from "react";
import Link from "next/link";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import CollapsibleCard from "@/components/CollapsibleCard";
import { TrendChart } from "@/components/TrendChart";
import { formatAmps, formatNumber, formatPercent, formatRuntime, formatVolts, formatWatts } from "@/lib/format";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { classifyPowerNode, type PowerNodeKind } from "@/lib/powerSensors";
import { useMetricsQuery, useNodesQuery, useSensorsQuery } from "@/lib/queries";
import { findSensor, sensorMetric, sensorSource } from "@/lib/sensorOrigin";
import { Select } from "@/components/ui/select";
import SegmentedControl from "@/components/SegmentedControl";
import AcVoltageQualityPanel from "@/features/power/components/AcVoltageQualityPanel";
import DcVoltageQualityPanel from "@/features/power/components/DcVoltageQualityPanel";
import AnalyticsHeaderCard from "@/features/analytics/components/AnalyticsHeaderCard";
import { formatDateTimeForTimeZone, useControllerTimeZone } from "@/lib/siteTime";
import { Card } from "@/components/ui/card";
import type { DemoNode, DemoSensor, TrendSeriesEntry } from "@/types/dashboard";

const DEFAULT_RANGE_HOURS = 24;
const DEFAULT_INTERVAL_SECONDS = 300;

function normalizeEmporiaMainsKey(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim().toLowerCase();
  if (!normalized) return null;
  if (normalized.includes("mains_a") || normalized === "a") return "l1";
  if (normalized.includes("mains_b") || normalized === "b") return "l2";
  return null;
}

function emporiaMainsLegSensors(sensors: DemoSensor[], metric: string): DemoSensor[] {
  const mains = sensors
    .filter((sensor) => sensorSource(sensor) === "emporia_cloud")
    .filter((sensor) => sensorMetric(sensor) === metric)
    .filter((sensor) => Boolean(sensor.config?.["is_mains"]));

  const scored = mains.map((sensor) => {
    const key =
      normalizeEmporiaMainsKey(sensor.config?.["channel_num"]) ??
      normalizeEmporiaMainsKey(sensor.config?.["channel_key"]) ??
      normalizeEmporiaMainsKey(sensor.name);
    const score = key === "l1" ? 0 : key === "l2" ? 1 : 2;
    return { sensor, score };
  });
  scored.sort((a, b) => a.score - b.score || a.sensor.name.localeCompare(b.sensor.name));
  return scored.map((entry) => entry.sensor);
}

export default function PowerPageClient() {
  const timeZone = useControllerTimeZone();
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();

  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);

  const sensorsByNode = useMemo(() => {
    const map = new Map<string, DemoSensor[]>();
    sensors.forEach((sensor) => {
      const list = map.get(sensor.node_id) ?? [];
      list.push(sensor);
      map.set(sensor.node_id, list);
    });
    return map;
  }, [sensors]);

  const powerNodes = useMemo(() => {
    return nodes
      .map((node) => {
        const nodeSensors = sensorsByNode.get(node.id) ?? [];
        const kind = classifyPowerNode(node, nodeSensors);
        const config = node.config ?? {};
        return kind
          ? {
              node,
              kind,
              sensors: nodeSensors,
              groupLabel: typeof config.group_label === "string" ? (config.group_label as string) : null,
              includeInPowerSummary: config.include_in_power_summary !== false,
            }
          : null;
      })
      .filter(
        (
          entry,
        ): entry is {
          node: DemoNode;
          kind: PowerNodeKind;
          sensors: DemoSensor[];
          groupLabel: string | null;
          includeInPowerSummary: boolean;
        } => Boolean(entry),
      )
      .sort((a, b) => a.node.name.localeCompare(b.node.name));
  }, [nodes, sensorsByNode]);

  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const effectiveSelectedNodeId = selectedNodeId ?? powerNodes[0]?.node.id ?? null;
  const selected = effectiveSelectedNodeId
    ? powerNodes.find((entry) => entry.node.id === effectiveSelectedNodeId) ?? null
    : null;

  const powerWChartSensorIds = useMemo(() => {
    if (!selected) return [];
    if (selected.kind === "emporia") {
      const mains = findSensor(selected.sensors, "emporia_cloud", "mains_power_w");
      const circuits = selected.sensors
        .filter((sensor) => sensorMetric(sensor) === "channel_power_w")
        .sort((a, b) => (b.latest_value ?? 0) - (a.latest_value ?? 0))
        .slice(0, 4);
      return [mains?.sensor_id, ...circuits.map((s) => s.sensor_id)].filter(Boolean) as string[];
    }

    const pv = findSensor(selected.sensors, "renogy_bt2", "pv_power_w");
    const load = findSensor(selected.sensors, "renogy_bt2", "load_power_w");
    return [pv?.sensor_id, load?.sensor_id].filter(Boolean) as string[];
  }, [selected]);

  const voltageChartSensorIds = useMemo(() => {
    if (!selected) return [];
    if (selected.kind === "emporia") {
      const mainsLegs = emporiaMainsLegSensors(selected.sensors, "channel_voltage_v");
      return mainsLegs.map((sensor) => sensor.sensor_id);
    }

    const pv = findSensor(selected.sensors, "renogy_bt2", "pv_voltage_v");
    const load = findSensor(selected.sensors, "renogy_bt2", "load_voltage_v");
    const battery = findSensor(selected.sensors, "renogy_bt2", "battery_voltage_v");
    return [pv?.sensor_id, load?.sensor_id, battery?.sensor_id].filter(Boolean) as string[];
  }, [selected]);

  const currentChartSensorIds = useMemo(() => {
    if (!selected) return [];
    if (selected.kind === "emporia") {
      const mainsLegs = emporiaMainsLegSensors(selected.sensors, "channel_current_a");
      return mainsLegs.map((sensor) => sensor.sensor_id);
    }

    const pv = findSensor(selected.sensors, "renogy_bt2", "pv_current_a");
    const load = findSensor(selected.sensors, "renogy_bt2", "load_current_a");
    const battery = findSensor(selected.sensors, "renogy_bt2", "battery_current_a");
    return [pv?.sensor_id, load?.sensor_id, battery?.sensor_id].filter(Boolean) as string[];
  }, [selected]);

  const metricsSensorIds = useMemo(() => {
    const set = new Set<string>();
    powerWChartSensorIds.forEach((id) => set.add(id));
    voltageChartSensorIds.forEach((id) => set.add(id));
    currentChartSensorIds.forEach((id) => set.add(id));
    return Array.from(set);
  }, [currentChartSensorIds, powerWChartSensorIds, voltageChartSensorIds]);

  const {
    data: series,
    isLoading: metricsLoading,
    error: metricsError,
  } = useMetricsQuery({
    sensorIds: metricsSensorIds,
    rangeHours: DEFAULT_RANGE_HOURS,
    interval: DEFAULT_INTERVAL_SECONDS,
    enabled: metricsSensorIds.length > 0,
    refetchInterval: 30_000,
  });

  const labeledSeries = useMemo<TrendSeriesEntry[]>(() => {
    if (!series || !selected) return [];
    const labelMap = new Map<string, string>();
    const unitMap = new Map<string, string>();
    selected.sensors.forEach((sensor) => {
      labelMap.set(sensor.sensor_id, `${selected.node.name} — ${sensor.name} (${sensor.unit})`);
      unitMap.set(sensor.sensor_id, sensor.unit);
    });
    return series.map((entry) => ({
      ...entry,
      label: labelMap.get(entry.sensor_id) ?? entry.label ?? entry.sensor_id,
      unit: entry.unit ?? unitMap.get(entry.sensor_id) ?? undefined,
    }));
  }, [series, selected]);

  const batteryPowerSeries = useMemo<TrendSeriesEntry[]>(() => {
    if (!selected || selected.kind !== "renogy") return [];
    const batteryVoltageId =
      findSensor(selected.sensors, "renogy_bt2", "battery_voltage_v")?.sensor_id ?? null;
    const batteryCurrentId =
      findSensor(selected.sensors, "renogy_bt2", "battery_current_a")?.sensor_id ?? null;
    if (!batteryVoltageId || !batteryCurrentId) return [];

    const voltageSeries = labeledSeries.find((entry) => entry.sensor_id === batteryVoltageId) ?? null;
    const currentSeries = labeledSeries.find((entry) => entry.sensor_id === batteryCurrentId) ?? null;
    if (!voltageSeries || !currentSeries) return [];
    if (!voltageSeries.points.length || !currentSeries.points.length) return [];

    const currentByTs = new Map<number, number>();
    currentSeries.points.forEach((pt) => {
      if (pt.value == null) return;
      currentByTs.set(pt.timestamp.getTime(), pt.value);
    });

    const points = voltageSeries.points
      .map((pt) => {
        if (pt.value == null) return null;
        const current = currentByTs.get(pt.timestamp.getTime());
        if (current == null) return null;
        return { timestamp: pt.timestamp, value: pt.value * current };
      })
      .filter((pt): pt is { timestamp: Date; value: number } => pt != null);

    if (!points.length) return [];

    return [
      {
        sensor_id: `derived:battery_power_w:${selected.node.id}`,
        label: `${selected.node.name} — Battery power (W)`,
        unit: "W",
        points,
      },
    ];
  }, [labeledSeries, selected]);

  const [dcQualityMetric, setDcQualityMetric] = useState<"battery" | "pv" | "load">("battery");
  const dcVoltageSeries = useMemo<TrendSeriesEntry[]>(() => {
    if (!selected || selected.kind !== "renogy") return [];
    const metric = dcQualityMetric === "pv" ? "pv_voltage_v" : dcQualityMetric === "load" ? "load_voltage_v" : "battery_voltage_v";
    const sensorId = findSensor(selected.sensors, "renogy_bt2", metric)?.sensor_id ?? null;
    if (!sensorId) return [];
    return labeledSeries.filter((entry) => entry.sensor_id === sensorId);
  }, [dcQualityMetric, labeledSeries, selected]);

  const acVoltageSeries = useMemo<TrendSeriesEntry[]>(() => {
    if (!selected || selected.kind !== "emporia") return [];
    const ids = new Set(voltageChartSensorIds);
    return labeledSeries.filter((entry) => ids.has(entry.sensor_id));
  }, [labeledSeries, selected, voltageChartSensorIds]);

  const powerTrendSeries = useMemo(() => {
    const wanted = new Set(powerWChartSensorIds);
    const series = labeledSeries.filter((s) => wanted.has(s.sensor_id) && s.points.length > 0);
    if (selected?.kind === "renogy") {
      return [...series, ...batteryPowerSeries];
    }
    return series;
  }, [batteryPowerSeries, labeledSeries, powerWChartSensorIds, selected?.kind]);

  const voltageTrendSeries = useMemo(() => {
    const wanted = new Set(voltageChartSensorIds);
    return labeledSeries.filter((s) => wanted.has(s.sensor_id) && s.points.length > 0);
  }, [labeledSeries, voltageChartSensorIds]);

  const currentTrendSeries = useMemo(() => {
    const wanted = new Set(currentChartSensorIds);
    return labeledSeries.filter((s) => wanted.has(s.sensor_id) && s.points.length > 0);
  }, [currentChartSensorIds, labeledSeries]);

  if (nodesQuery.isLoading || sensorsQuery.isLoading) {
    return <LoadingState label="Loading power data..." />;
  }
  if (nodesQuery.error || sensorsQuery.error) {
    const message =
      (nodesQuery.error instanceof Error && nodesQuery.error.message) ||
      (sensorsQuery.error instanceof Error && sensorsQuery.error.message) ||
      "Failed to load power data.";
    return <ErrorState message={message} />;
  }

  return (
    <div className="space-y-5">
      <AnalyticsHeaderCard tab="power">
        <div className="flex flex-wrap items-center gap-3">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Power node
          </label>
          <Select
            value={effectiveSelectedNodeId ?? ""}
            onChange={(event) => setSelectedNodeId(event.target.value)}
          >
            {powerNodes.map((entry) => (
              <option key={entry.node.id} value={entry.node.id}>
                {entry.kind === "emporia" && entry.groupLabel?.trim()
                  ? `${entry.groupLabel.trim()} — ${entry.node.name} (${entry.kind})`
                  : `${entry.node.name} (${entry.kind})`}
              </option>
            ))}
          </Select>
          {selected?.kind === "emporia" && selected.groupLabel?.trim() ? (
 <span className="text-xs text-muted-foreground">
              Group {selected.groupLabel.trim()}
              {!selected.includeInPowerSummary ? " · excluded from totals" : ""}
            </span>
          ) : null}
          {selected?.node.last_seen ? (
 <span className="text-xs text-muted-foreground">
              Last update{" "}
              {formatDateTimeForTimeZone(new Date(selected.node.last_seen), timeZone, {
                month: "numeric",
                day: "numeric",
                year: "numeric",
                hour: "numeric",
                minute: "2-digit",
              })}
            </span>
          ) : null}
        </div>
      </AnalyticsHeaderCard>

      {!selected ? (
 <Card className="rounded-lg gap-0 border-dashed p-6 text-sm text-muted-foreground">
          No power nodes detected yet. Configure Renogy/Emporia integrations and wait for telemetry ingestion.
        </Card>
      ) : (
        <div className="space-y-6">
          <PowerNodeSummary entry={selected} />

          <CollapsibleCard
            title={`Power (W) trends (past ${DEFAULT_RANGE_HOURS}h)`}
            description="Bucketed time-series for the selected power sensors (explicit units)."
            defaultOpen
            bodyClassName="space-y-3"
          >
            {metricsError ? (
              <ErrorState
                message={
                  metricsError instanceof Error ? metricsError.message : "Failed to load power metrics."
                }
              />
            ) : metricsLoading ? (
              <LoadingState label="Loading power trends..." />
            ) : (
              <TrendChart data={powerTrendSeries} timeZone={timeZone} />
            )}
          </CollapsibleCard>

          <CollapsibleCard
            title={`Voltage (V) trends (past ${DEFAULT_RANGE_HOURS}h)`}
            description="Voltage trends for the selected node (explicit units)."
            defaultOpen
            bodyClassName="space-y-3"
          >
            {metricsError ? (
              <ErrorState
                message={
                  metricsError instanceof Error ? metricsError.message : "Failed to load voltage metrics."
                }
              />
            ) : metricsLoading ? (
              <LoadingState label="Loading voltage trends..." />
            ) : (
              <TrendChart data={voltageTrendSeries} timeZone={timeZone} />
            )}

            {!metricsError && !metricsLoading && selected.kind === "emporia" && acVoltageSeries.length ? (
              <AcVoltageQualityPanel series={acVoltageSeries} intervalSeconds={DEFAULT_INTERVAL_SECONDS} />
            ) : null}

            {!metricsError && !metricsLoading && selected.kind === "renogy" && dcVoltageSeries.length ? (
              <div className="space-y-3">
                <div className="flex flex-wrap items-center justify-between gap-3">
 <div className="text-sm font-semibold text-foreground">
                    DC voltage quality
                  </div>
                  <SegmentedControl
                    value={dcQualityMetric}
                    onChange={(next) => setDcQualityMetric(next as "battery" | "pv" | "load")}
                    options={[
                      { value: "battery", label: "Battery" },
                      { value: "pv", label: "PV" },
                      { value: "load", label: "Load" },
                    ]}
                    size="xs"
                  />
                </div>
                <DcVoltageQualityPanel
                  series={dcVoltageSeries}
                  intervalSeconds={DEFAULT_INTERVAL_SECONDS}
                  title={`DC voltage quality (${dcQualityMetric})`}
                />
              </div>
            ) : null}
          </CollapsibleCard>

          <CollapsibleCard
            title={`Current (A) trends (past ${DEFAULT_RANGE_HOURS}h)`}
            description="Current trends for the selected node (explicit units)."
            defaultOpen
            bodyClassName="space-y-3"
          >
            {metricsError ? (
              <ErrorState
                message={
                  metricsError instanceof Error ? metricsError.message : "Failed to load current metrics."
                }
              />
            ) : metricsLoading ? (
              <LoadingState label="Loading current trends..." />
            ) : (
              <TrendChart data={currentTrendSeries} timeZone={timeZone} />
            )}
          </CollapsibleCard>

          {selected.kind === "emporia" ? (
            <EmporiaCircuitsTable
              nodeName={selected.node.name}
              lastSeen={selected.node.last_seen ?? null}
              sensors={selected.sensors}
              timeZone={timeZone}
            />
          ) : null}
        </div>
      )}
    </div>
  );
}

function PowerNodeSummary({
  entry,
}: {
  entry: {
    node: DemoNode;
    kind: PowerNodeKind;
    sensors: DemoSensor[];
    groupLabel: string | null;
    includeInPowerSummary: boolean;
  };
}) {
  if (entry.kind === "emporia") {
    const mains = findSensor(entry.sensors, "emporia_cloud", "mains_power_w");
    const mainsVLegs = emporiaMainsLegSensors(entry.sensors, "channel_voltage_v");
    const mainsALegs = emporiaMainsLegSensors(entry.sensors, "channel_current_a");
    const mainsValue = mains?.latest_value;
    const circuits = entry.sensors.filter((sensor) => sensorMetric(sensor) === "channel_power_w");
    const activeCircuits = circuits.filter((sensor) => (sensor.latest_value ?? 0) > 0);

    const voltageLabel =
      mainsVLegs.length >= 2
        ? `L1 ${mainsVLegs[0]?.latest_value != null ? formatVolts(mainsVLegs[0].latest_value) : "—"} · L2 ${mainsVLegs[1]?.latest_value != null ? formatVolts(mainsVLegs[1].latest_value) : "—"}`
        : mainsVLegs.length === 1 && mainsVLegs[0]?.latest_value != null
          ? formatVolts(mainsVLegs[0].latest_value)
          : null;
    const currentLabel =
      mainsALegs.length >= 2
        ? `L1 ${mainsALegs[0]?.latest_value != null ? formatAmps(mainsALegs[0].latest_value) : "—"} · L2 ${mainsALegs[1]?.latest_value != null ? formatAmps(mainsALegs[1].latest_value) : "—"}`
        : mainsALegs.length === 1 && mainsALegs[0]?.latest_value != null
          ? formatAmps(mainsALegs[0].latest_value)
          : null;
    const secondaryReadbacks = [voltageLabel, currentLabel].filter(Boolean).join(" · ") || "Voltage/current unavailable";
    return (
      <section className="grid gap-4 md:grid-cols-3">
        <SummaryCard
          label="Emporia mains power (W)"
          primary={mainsValue != null ? formatWatts(mainsValue) : "—"}
          secondary={secondaryReadbacks !== "Voltage/current unavailable" ? secondaryReadbacks : `Node ${entry.node.name}`}
        />
        <SummaryCard
          label="Circuits reporting"
          primary={`${activeCircuits.length}/${circuits.length}`}
          secondary="Channels with recent samples"
        />
        <SummaryCard
          label="Node status"
          primary={formatNodeStatusLabel(entry.node.status ?? "unknown", entry.node.last_seen)}
          secondary={entry.node.last_seen ? `Last seen ${new Date(entry.node.last_seen).toLocaleTimeString()}` : "No samples yet"}
        />
      </section>
    );
  }

  const pv = findSensor(entry.sensors, "renogy_bt2", "pv_power_w");
  const load = findSensor(entry.sensors, "renogy_bt2", "load_power_w");
  const batterySoc = findSensor(entry.sensors, "renogy_bt2", "battery_soc_percent");
  const batteryV = findSensor(entry.sensors, "renogy_bt2", "battery_voltage_v");
  const batteryA = findSensor(entry.sensors, "renogy_bt2", "battery_current_a");

  const socEst = findSensor(entry.sensors, "battery_model", "battery_soc_est_percent");
  const remainingAh = findSensor(entry.sensors, "battery_model", "battery_remaining_ah");
  const capacityEstAh = findSensor(entry.sensors, "battery_model", "battery_capacity_est_ah");

  const runwayHours = findSensor(entry.sensors, "power_runway", "power_runway_hours_conservative");
  const runwayMinSoc = findSensor(entry.sensors, "power_runway", "power_runway_min_soc_projected_percent");

  const configuredLoadSensorIds = (() => {
    const cfg = (entry.node.config?.["power_runway"] ?? null) as unknown;
    if (!cfg || typeof cfg !== "object") return [];
    const raw = (cfg as Record<string, unknown>)["load_sensor_ids"];
    if (!Array.isArray(raw)) return [];
    return raw
      .filter((value): value is string => typeof value === "string")
      .map((value) => value.trim())
      .filter(Boolean);
  })();

  const configuredLoadPowerW = (() => {
    if (!configuredLoadSensorIds.length) return null;
    const byId = new Map(entry.sensors.map((sensor) => [sensor.sensor_id, sensor]));
    let sum = 0;
    let found = 0;
    configuredLoadSensorIds.forEach((id) => {
      const sensor = byId.get(id);
      const value = sensor?.latest_value;
      if (typeof value === "number" && Number.isFinite(value)) {
        sum += value;
        found += 1;
      }
    });
    return found > 0 ? sum : null;
  })();

  const formatAh = (value: number) =>
    `${formatNumber(value, { minimumFractionDigits: 0, maximumFractionDigits: 1 })} Ah`;

  const stickerCapacityAh = (() => {
    const rawBatteryCfg = entry.node.config?.["battery_model"];
    if (rawBatteryCfg && typeof rawBatteryCfg === "object") {
      const cap = (rawBatteryCfg as Record<string, unknown>)["sticker_capacity_ah"];
      if (typeof cap === "number" && Number.isFinite(cap) && cap > 0) return cap;
    }

    const remaining = remainingAh?.latest_value ?? null;
    const soc = socEst?.latest_value ?? null;
    if (typeof remaining === "number" && Number.isFinite(remaining) && typeof soc === "number" && Number.isFinite(soc) && soc > 0.5) {
      return remaining / (soc / 100);
    }

    return null;
  })();

  const remainingLabel = (() => {
    const remaining = remainingAh?.latest_value ?? null;
    if (typeof remaining !== "number" || !Number.isFinite(remaining)) return "—";
    if (typeof stickerCapacityAh === "number" && Number.isFinite(stickerCapacityAh) && stickerCapacityAh > 0) {
      return `${formatAh(remaining)} / ${formatAh(stickerCapacityAh)}`;
    }
    return formatAh(remaining);
  })();

  const socPrimary =
    socEst?.latest_value != null
      ? `${formatPercent(socEst.latest_value)} (est)`
      : batterySoc?.latest_value != null
        ? `${formatPercent(batterySoc.latest_value)} (Renogy)`
        : "—";

  const socSecondaryParts: string[] = [];
  if (batterySoc?.latest_value != null) {
    socSecondaryParts.push(`Renogy SOC ${formatPercent(batterySoc.latest_value)}`);
  }
  socSecondaryParts.push(remainingLabel !== "—" ? `Remaining ${remainingLabel}` : "Remaining —");
  if (capacityEstAh?.latest_value != null && Number.isFinite(capacityEstAh.latest_value)) {
    socSecondaryParts.push(`Capacity est ${formatAh(capacityEstAh.latest_value)}`);
  }
  if (batteryV?.latest_value != null && batteryA?.latest_value != null) {
    socSecondaryParts.push(`${formatVolts(batteryV.latest_value)} · ${formatAmps(batteryA.latest_value)}`);
  } else if (batteryV?.latest_value != null) {
    socSecondaryParts.push(formatVolts(batteryV.latest_value));
  }
  const socSecondary = socSecondaryParts.join(" · ");

  const runwayPrimary =
    runwayHours?.latest_value != null && Number.isFinite(runwayHours.latest_value)
      ? formatRuntime(runwayHours.latest_value)
      : "—";
  const runwaySecondaryParts: string[] = [];
  if (runwayHours?.latest_value != null && Number.isFinite(runwayHours.latest_value)) {
    runwaySecondaryParts.push(`${formatNumber(runwayHours.latest_value, { minimumFractionDigits: 0, maximumFractionDigits: 1 })} hr`);
  }
  if (runwayMinSoc?.latest_value != null && Number.isFinite(runwayMinSoc.latest_value)) {
    runwaySecondaryParts.push(`min SOC ${formatPercent(runwayMinSoc.latest_value)}`);
  }
  runwaySecondaryParts.push("PV=0 beyond horizon");
  const runwaySecondary = runwaySecondaryParts.join(" · ");

  const loadPrimary =
    configuredLoadPowerW != null
      ? formatWatts(configuredLoadPowerW)
      : load?.latest_value != null
        ? formatWatts(load.latest_value)
        : "—";
  const loadSecondaryParts: string[] = [];
  if (configuredLoadSensorIds.length > 0) {
    loadSecondaryParts.push(`Using ${configuredLoadSensorIds.length} configured sensor(s)`);
    if (configuredLoadPowerW == null) {
      loadSecondaryParts.push("configured sensors missing recent samples");
    }
    if (load?.latest_value != null) {
      loadSecondaryParts.push(`Renogy ${formatWatts(load.latest_value)}`);
    }
  } else {
    loadSecondaryParts.push("Configure true load sensors in Setup Center");
  }
  const loadSecondary = loadSecondaryParts.join(" · ");

  return (
    <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <SummaryCard
        label="Renogy PV power (W)"
        primary={pv?.latest_value != null ? formatWatts(pv.latest_value) : "—"}
        secondary={`Node ${entry.node.name}`}
      />
      <SummaryCard
        label="Load power (W)"
        primary={loadPrimary}
        secondary={loadSecondary}
      />
      <SummaryCard
        label="Battery SOC"
        primary={socPrimary}
        secondary={socSecondary}
      />

      <SummaryCard label="Runway (conservative)" primary={runwayPrimary} secondary={runwaySecondary} />
    </section>
  );
}

function SummaryCard({ label, primary, secondary }: { label: string; primary: string; secondary: string }) {
  return (
    <Card className="p-6">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </p>
 <p className="mt-2 text-xl font-semibold leading-tight text-foreground">
        {primary}
      </p>
 <p className="text-sm text-muted-foreground">{secondary}</p>
    </Card>
  );
}

function EmporiaCircuitsTable({
  nodeName,
  lastSeen,
  sensors,
  timeZone,
}: {
  nodeName: string;
  lastSeen: string | Date | null;
  sensors: DemoSensor[];
  timeZone: string;
}) {
  const nodeId = sensors[0]?.node_id ?? null;
  const circuits = useMemo(() => {
    const rows = new Map<
      string,
      {
        key: string;
        name: string;
        power_w: number | null;
        power_is_derived: boolean;
        voltage_v: number | null;
        voltage_is_derived: boolean;
        current_a: number | null;
        power_sensor_id: string | null;
        voltage_sensor_id: string | null;
        current_sensor_id: string | null;
      }
    >();

    const stripSuffix = (value: string) => value.replace(/\s+(Voltage|Current)$/i, "").trim();

    for (const sensor of sensors) {
      const metric = sensorMetric(sensor);
      if (!metric) continue;
      if (!["channel_power_w", "channel_voltage_v", "channel_current_a"].includes(metric)) continue;
      const config = sensor.config ?? {};
      const channelKey =
        (typeof config.channel_key === "string" && config.channel_key.trim()) ||
        (typeof config.channel_num === "string" && config.channel_num.trim()) ||
        sensor.sensor_id;
      const nameHint =
        (typeof config.channel_name === "string" && config.channel_name.trim()) ||
        stripSuffix(sensor.name);
      const row = rows.get(channelKey) ?? {
        key: channelKey,
        name: nameHint,
        power_w: null,
        power_is_derived: false,
        voltage_v: null,
        voltage_is_derived: false,
        current_a: null,
        power_sensor_id: null,
        voltage_sensor_id: null,
        current_sensor_id: null,
      };

      if (metric === "channel_power_w") {
        row.power_w = sensor.latest_value ?? null;
        row.power_sensor_id = sensor.sensor_id;
        row.power_is_derived = Boolean(config.derived_from_va);
      } else if (metric === "channel_voltage_v") {
        row.voltage_v = sensor.latest_value ?? null;
        row.voltage_sensor_id = sensor.sensor_id;
      } else if (metric === "channel_current_a") {
        row.current_a = sensor.latest_value ?? null;
        row.current_sensor_id = sensor.sensor_id;
      }

      rows.set(channelKey, row);
    }

    for (const row of rows.values()) {
      if (row.power_w != null) continue;
      if (row.voltage_v == null || row.current_a == null) continue;
      row.power_w = row.voltage_v * row.current_a;
      row.power_is_derived = true;
    }

    for (const row of rows.values()) {
      if (row.voltage_v != null) continue;
      if (row.current_a == null || row.current_a === 0) continue;
      if (row.power_w == null) continue;
      row.voltage_v = row.power_w / row.current_a;
      row.voltage_is_derived = true;
    }

    return Array.from(rows.values()).sort((a, b) => (b.power_w ?? 0) - (a.power_w ?? 0));
  }, [sensors]);

  return (
    <CollapsibleCard
      title="Emporia circuits"
      description={`Circuit readbacks for node ${nodeName}. Values are instantaneous power (W/kW) with explicit units.`}
      defaultOpen={false}
    >
      <div className="overflow-x-auto">
 <table className="min-w-full divide-y divide-border text-sm">
          <thead className="bg-card-inset">
            <tr>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Circuit
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Power (W)
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Voltage (V)
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Current (A)
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Last update
              </th>
            </tr>
          </thead>
 <tbody className="divide-y divide-border">
            {circuits.map((circuit) => (
              <tr key={circuit.key}>
                <td className="px-4 py-3">
 <div className="font-medium text-foreground">
                    {circuit.power_sensor_id || circuit.voltage_sensor_id || circuit.current_sensor_id ? (
                      <Link
                        className="hover:underline"
                        href={`/sensors?${nodeId ? `node=${encodeURIComponent(nodeId)}&` : ""}sensor=${encodeURIComponent(
                          circuit.power_sensor_id ?? circuit.voltage_sensor_id ?? circuit.current_sensor_id ?? "",
                        )}`}
                      >
                        {circuit.name}
                      </Link>
                    ) : (
                      circuit.name
                    )}
                  </div>
 <div className="text-xs text-muted-foreground">{circuit.key}</div>
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {circuit.power_w != null ? (
                    <span className="inline-flex items-baseline gap-2">
                      <span>{formatWatts(circuit.power_w)}</span>
                      {circuit.power_is_derived ? (
 <span className="text-xs text-muted-foreground" title="Computed from Voltage × Current">
                          calc
                        </span>
                      ) : null}
                    </span>
                  ) : (
                    "—"
                  )}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {circuit.voltage_v != null ? (
                    <span className="inline-flex items-baseline gap-2">
                      <span>{formatVolts(circuit.voltage_v)}</span>
                      {circuit.voltage_is_derived ? (
                        <span
 className="text-xs text-muted-foreground"
                          title="Computed from Power ÷ Current"
                        >
                          calc
                        </span>
                      ) : null}
                    </span>
                  ) : (
                    "—"
                  )}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {circuit.current_a != null ? formatAmps(circuit.current_a) : "—"}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {lastSeen
                    ? formatDateTimeForTimeZone(new Date(lastSeen), timeZone, {
                        month: "numeric",
                        day: "numeric",
                        year: "numeric",
                        hour: "numeric",
                        minute: "2-digit",
                      })
                    : "—"}
                </td>
              </tr>
            ))}
            {!circuits.length ? (
              <tr>
 <td colSpan={5} className="px-4 py-6 text-center text-sm text-muted-foreground">
                  No Emporia circuit sensors detected yet.
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>
    </CollapsibleCard>
  );
}
