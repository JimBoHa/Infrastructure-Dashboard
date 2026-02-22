"use client";

import { useMemo } from "react";

import { Card, CardContent } from "@/components/ui/card";
import { useAnalyticsData } from "@/features/analytics/hooks/useAnalyticsData";
import { formatKwh, formatKw, formatAmps, formatPercent, formatRuntime, formatVolts } from "@/lib/format";
import { sensorMetric, sensorSource } from "@/lib/sensorOrigin";
import type { AnalyticsStatus } from "@/types/dashboard";

export function StatusSection({ status }: { status: AnalyticsStatus }) {
  const totalNodes = status.nodes_online + status.nodes_offline;
  const remoteNodesOnline = status.remote_nodes_online ?? 0;
  const remoteNodesOffline = status.remote_nodes_offline ?? 0;
  const remoteNodes = remoteNodesOnline + remoteNodesOffline;
  const { sensors, nodes } = useAnalyticsData();
  const batteryVoltageSensors = useMemo(
    () =>
      sensors.filter(
        (sensor) =>
          sensorSource(sensor) === "renogy_bt2" && sensorMetric(sensor) === "battery_voltage_v",
      ),
    [sensors],
  );
  const batteryCurrentSensors = useMemo(
    () =>
      sensors.filter(
        (sensor) =>
          sensorSource(sensor) === "renogy_bt2" && sensorMetric(sensor) === "battery_current_a",
      ),
    [sensors],
  );
  const nodeLabels = useMemo(() => new Map(nodes.map((node) => [node.id, node.name])), [nodes]);
  const batteryPrimary = useMemo(() => {
    const voltageValues = batteryVoltageSensors
      .map((sensor) => sensor.latest_value)
      .filter((value): value is number => value != null && Number.isFinite(value));
    if (voltageValues.length === 0) {
      return status.battery_soc != null ? formatPercent(status.battery_soc) : "—";
    }

    if (batteryVoltageSensors.length === 1 && voltageValues.length === 1) {
      const voltageSensor = batteryVoltageSensors[0];
      const currentSensor =
        batteryCurrentSensors.find((sensor) => sensor.node_id === voltageSensor.node_id) ?? null;
      const volts = voltageSensor.latest_value;
      const amps = currentSensor?.latest_value ?? null;
      if (volts != null && amps != null) {
        return `${formatVolts(volts)} · ${formatAmps(amps)}`;
      }
      if (volts != null) return formatVolts(volts);
      return status.battery_soc != null ? formatPercent(status.battery_soc) : "—";
    }

    const avgVolts = voltageValues.reduce((sum, value) => sum + value, 0) / voltageValues.length;
    return `${formatVolts(avgVolts)} · ${voltageValues.length} nodes`;
  }, [batteryCurrentSensors, batteryVoltageSensors, status.battery_soc]);
  const batterySecondary = useMemo(() => {
    const parts: string[] = [];
    if (batteryVoltageSensors.length === 1) {
      const nodeName = nodeLabels.get(batteryVoltageSensors[0].node_id);
      if (nodeName) parts.push(`Node ${nodeName}`);
    }
    if (status.battery_soc != null) {
      parts.push(`SOC (Renogy) ${formatPercent(status.battery_soc)}`);
    }
    return parts.join(" · ");
  }, [batteryVoltageSensors, nodeLabels, status.battery_soc]);
  const solarKw = status.solar_kw ?? 0;
  const currentLoadKw = status.current_load_kw ?? 0;
  const runtimeHours = status.estimated_runtime_hours ?? 0;
  const storageCapacity = status.storage_capacity_kwh ?? 0;
  const alarmsLast = status.alarms_last_168h ?? 0;

  return (
    <section className="space-y-4">
      <header className="space-y-1">
 <h3 className="text-lg font-semibold text-foreground">
          Fleet status
        </h3>
 <p className="text-sm text-muted-foreground">
          Battery health, solar availability, and alarm counts across the past week.
        </p>
      </header>
      <div className="grid gap-4 sm:grid-cols-2">
        <StatusCard
          title="Battery"
          primary={batteryPrimary}
          secondary={[batterySecondary, `Solar ${formatKw(solarKw)}`].filter(Boolean).join(" · ")}
        />
        <StatusCard
          title="Load / Runtime"
          primary={formatKw(currentLoadKw)}
          secondary={`Runtime ${formatRuntime(runtimeHours)}`}
        />
        <StatusCard
          title="Nodes"
          primary={`${status.nodes_online}/${totalNodes} online`}
          secondary={`Remote ${remoteNodesOnline}/${remoteNodes}`}
        />
        <StatusCard
          title="Alarms"
          primary={`${alarmsLast} in 7 days`}
          secondary={`Storage ${formatKwh(storageCapacity)}`}
        />
      </div>
    </section>
  );
}

function StatusCard({
  title,
  primary,
  secondary,
}: {
  title: string;
  primary: string;
  secondary: string;
}) {
  return (
    <Card>
      <CardContent className="pt-6">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          {title}
        </p>
 <p className="mt-2 break-words text-lg font-semibold leading-tight text-foreground">
          {primary}
        </p>
 <p className="text-sm text-muted-foreground">{secondary}</p>
      </CardContent>
    </Card>
  );
}
