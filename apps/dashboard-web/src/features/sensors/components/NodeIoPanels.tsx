"use client";

import { useMemo, useState } from "react";
import { formatDistanceToNow } from "date-fns";
import { useAuth } from "@/components/AuthProvider";
import AlarmOriginBadge from "@/components/alarms/AlarmOriginBadge";
import { Badge } from "@/components/ui/badge";
import { isPredictiveOrigin, type AlarmOriginFilter } from "@/lib/alarms/origin";
import filterAlarmsByOrigin from "@/features/sensors/utils/filterAlarmsByOrigin";
import LiveWeatherPanel from "@/features/nodes/components/LiveWeatherPanel";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import NodeTypeBadge from "@/features/nodes/components/NodeTypeBadge";
import AddSensorDrawer, { type AddSensorMode } from "@/features/sensors/components/AddSensorDrawer";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { formatSensorInterval, formatSensorValueWithUnit } from "@/lib/sensorFormat";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import type { DemoAlarm, DemoNode, DemoOutput, DemoSchedule, DemoSensor } from "@/types/dashboard";

export default function NodeIoPanels({
  nodes,
  sensors,
  outputs,
  schedules,
  alarms,
  alarmOriginFilter,
  nodeFilter,
  typeFilter,
  onSelectSensor,
  onCommandOutput,
}: {
  nodes: DemoNode[];
  sensors: DemoSensor[];
  outputs: DemoOutput[];
  schedules: DemoSchedule[];
  alarms: DemoAlarm[];
  alarmOriginFilter: AlarmOriginFilter;
  nodeFilter: string;
  typeFilter: string;
  onSelectSensor: (sensorId: string) => void;
  onCommandOutput: (outputId: string) => void;
}) {
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const canCommand = Boolean(me?.capabilities?.includes("outputs.command"));
  const [sensorConfigNodeId, setSensorConfigNodeId] = useState<string | null>(null);
  const [sensorConfigMode, setSensorConfigMode] = useState<AddSensorMode>("hardware");
  const [expandedNodeIds, setExpandedNodeIds] = useState<Set<string>>(() => new Set());

  const scheduleNameById = useMemo(() => new Map(schedules.map((s) => [s.id, s.name])), [schedules]);

  const visibleNodes = useMemo(() => {
    return nodeFilter === "all" ? nodes : nodes.filter((node) => node.id === nodeFilter);
  }, [nodes, nodeFilter]);

  const sensorsByNode = useMemo(() => {
    const byNode = new Map<string, DemoSensor[]>();
    for (const sensor of sensors) {
      if (nodeFilter !== "all" && sensor.node_id !== nodeFilter) continue;
      if (typeFilter !== "all" && sensor.type !== typeFilter) continue;
      const list = byNode.get(sensor.node_id) ?? [];
      list.push(sensor);
      byNode.set(sensor.node_id, list);
    }
    return byNode;
  }, [sensors, nodeFilter, typeFilter]);

  const outputsByNode = useMemo(() => {
    const byNode = new Map<string, DemoOutput[]>();
    for (const output of outputs) {
      if (nodeFilter !== "all" && output.node_id !== nodeFilter) continue;
      const list = byNode.get(output.node_id) ?? [];
      list.push(output);
      byNode.set(output.node_id, list);
    }
    for (const list of byNode.values()) {
      list.sort((a, b) => a.name.localeCompare(b.name));
    }
    return byNode;
  }, [outputs, nodeFilter]);

  const sensorConfigNode = useMemo(() => {
    if (!sensorConfigNodeId) return null;
    return nodes.find((node) => node.id === sensorConfigNodeId) ?? null;
  }, [nodes, sensorConfigNodeId]);

  const supportsHardwareSensors = (node: DemoNode) => {
    const agentNodeId = node.config?.["agent_node_id"];
    return typeof agentNodeId === "string" && agentNodeId.trim().length > 0;
  };

  const supportsWeatherStationSensors = (node: DemoNode) => {
    const kind = node.config?.["kind"];
    return typeof kind === "string" && kind.trim().toLowerCase() === "ws-2902";
  };

  const sensorConfigNodeSupportsHardwareSensors = sensorConfigNode ? supportsHardwareSensors(sensorConfigNode) : false;
  const sensorConfigNodeSupportsWeatherStationSensors = sensorConfigNode ? supportsWeatherStationSensors(sensorConfigNode) : false;

  return (
    <>
      <CollapsibleCard
        title="Nodes"
        description={
          <div className="space-y-1">
            <p>Inspect public provider data, review sensors and alarms, and manage outputs per node.</p>
            {!canCommand ? (
              <p>
                Read-only outputs: you need <code className="px-1">outputs.command</code> to send commands.
              </p>
            ) : null}
          </div>
        }
        defaultOpen
        bodyClassName="space-y-4"
      >
        {visibleNodes.map((node) => {
          const nodeSensors = sensorsByNode.get(node.id) ?? [];
          const nodeOutputs = outputsByNode.get(node.id) ?? [];
          const canConfigureHardwareSensors = supportsHardwareSensors(node);
          const expanded = expandedNodeIds.has(node.id);

          return (
            <CollapsibleCard
              key={node.id}
              density="sm"
 className="bg-card-inset shadow-xs"
              title={
                <span className="inline-flex min-w-0 items-center gap-2">
                  <span className="min-w-0 truncate">{node.name}</span>
                  <NodeTypeBadge node={node} size="sm" className="shrink-0" />
                </span>
              }
              description={
 <span className="text-xs text-muted-foreground">
                  {nodeSensors.length} sensors · {nodeOutputs.length} outputs
                </span>
              }
              actions={
                <div className="flex items-center gap-2">
                  <NodePill tone={node.status === "online" ? "success" : "muted"} size="sm" caps>
                    {formatNodeStatusLabel(node.status, node.last_seen)}
                  </NodePill>
                  {node.last_seen ? (
 <span className="text-xs text-muted-foreground">
                      Last seen{" "}
                      {(() => {
                        const ts =
                          node.last_seen instanceof Date ? node.last_seen : new Date(node.last_seen);
                        if (Number.isNaN(ts.getTime())) return "—";
                        return formatDistanceToNow(ts, { addSuffix: true });
                      })()}
                    </span>
                  ) : null}
                </div>
              }
              open={expanded}
              onOpenChange={(nextOpen) => {
                setExpandedNodeIds((prev) => {
                  const next = new Set(prev);
                  if (nextOpen) {
                    next.add(node.id);
                  } else {
                    next.delete(node.id);
                  }
                  return next;
                });
              }}
            >
              <div className="space-y-4">
                {(() => {
                  const config = (node.config || {}) as Record<string, unknown>;
                  const hideLiveWeather = config["hide_live_weather"] === true;
                  if (hideLiveWeather) return null;
                  return <LiveWeatherPanel nodeId={node.id} />;
                })()}

                <div className="space-y-4">
                  <CollapsibleCard
                    density="sm"
                    title="Sensors"
                    description="Click a sensor to inspect details, alarms, and history preview."
                    defaultOpen
                    actions={
 <span className="text-xs text-muted-foreground">
                        {nodeSensors.length} shown
                      </span>
                    }
                  >
                    <div className="overflow-x-auto md:overflow-x-visible">
                      <table className="min-w-full divide-y divide-border text-sm">
 <thead className="bg-card-inset">
                          <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Name
                            </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Type
                            </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Interval
                            </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Latest
                            </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                              Alarms
                            </th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-border">
                          {nodeSensors.map((sensor) => {
                            const interval = formatSensorInterval(sensor.interval_seconds);
                            const sensorAlarms = filterAlarmsByOrigin(
                              alarms.filter(
                                (alarm) =>
                                  alarm.target_type === "sensor" &&
                                  alarm.target_id === sensor.sensor_id,
                              ),
                              alarmOriginFilter,
                            );
                            const predictiveCount = sensorAlarms.filter((alarm) =>
                              isPredictiveOrigin(alarm.origin ?? alarm.type),
                            ).length;
                            return (
                              <tr
                                key={sensor.sensor_id}
 className="cursor-pointer hover:bg-muted"
                                onClick={() => onSelectSensor(sensor.sensor_id)}
                              >
                                <td className="px-3 py-2">
                                  <div className="flex flex-wrap items-center gap-2">
 <div className="font-medium text-foreground">
                                      {sensor.name}
                                    </div>
                                    <SensorOriginBadge sensor={sensor} size="xs" />
                                  </div>
 <div className="text-[11px] text-muted-foreground">
                                    {sensor.sensor_id}
                                  </div>
                                </td>
 <td className="px-3 py-2 text-muted-foreground">
                                  {sensor.type}
                                </td>
 <td className="px-3 py-2 text-muted-foreground">
                                  <span title={interval.title}>{interval.label}</span>
                                </td>
 <td className="px-3 py-2 text-muted-foreground">
                                  {formatSensorValueWithUnit(sensor, sensor.latest_value, "-")}
                                </td>
 <td className="px-3 py-2 text-muted-foreground">
                                  {sensorAlarms.length ? (
                                    <div className="flex flex-wrap items-center gap-2">
                                      <Badge tone="warning">{sensorAlarms.length}</Badge>
                                      {predictiveCount > 0 ? <AlarmOriginBadge origin="predictive" /> : null}
                                    </div>
                                  ) : (
 <span className="text-xs text-muted-foreground">—</span>
                                  )}
                                </td>
                              </tr>
                              );
                            })}
                          {!nodeSensors.length ? (
                            <tr>
                              <td
                                colSpan={5}
 className="px-3 py-5 text-center text-sm text-muted-foreground"
                              >
                                No sensors match the current filters.
                              </td>
                            </tr>
                          ) : null}
                          <tr>
                            <td colSpan={5} className="px-3 py-3">
                              <NodeButton
                                variant="dashed"
                                fullWidth
                                size="sm"
                                onClick={() => {
                                  setSensorConfigNodeId(node.id);
                                  setSensorConfigMode(
                                    supportsWeatherStationSensors(node)
                                      ? "weather_station"
                                      : canConfigureHardwareSensors
                                        ? "hardware"
                                        : "derived",
                                  );
                                }}
                                disabled={!canEdit}
                              >
                                Add sensor
                                {!canEdit ? (
                                  <span className="text-xs font-normal">(requires config.write)</span>
                                ) : null}
                              </NodeButton>
                            </td>
                          </tr>
                        </tbody>
                      </table>
                    </div>
                  </CollapsibleCard>

                  <CollapsibleCard
                    density="sm"
                    title="Outputs"
                    description="Send commands and review linked schedules."
                    defaultOpen
                    actions={
 <span className="text-xs text-muted-foreground">
                        {nodeOutputs.length} shown
                      </span>
                    }
                  >
                    <div className="space-y-2">
                      {nodeOutputs.map((output) => {
                        const scheduleNames = (output.schedule_ids ?? []).map(
                          (id) => scheduleNameById.get(id) ?? id,
                        );
                        return (
                          <Card
                            key={output.id}
                            className="flex flex-col gap-2 rounded-lg bg-card-inset p-3"
                          >
                            <div className="flex items-center justify-between gap-2">
                              <div className="min-w-0">
 <p className="truncate text-sm font-semibold text-foreground">
                                  {output.name}
                                </p>
 <p className="text-xs text-muted-foreground">
                                  {output.type} · Supported: {output.supported_states?.join(", ") ?? "n/a"}
                                </p>
                              </div>
                              <NodePill tone="neutral" size="sm" caps weight="normal">
                                {output.state}
                              </NodePill>
                            </div>
                            {scheduleNames.length ? (
 <p className="text-xs text-muted-foreground">
                                Schedules: {scheduleNames.join(", ")}
                              </p>
                            ) : null}
                            <div className="flex items-center justify-end">
                              <NodeButton
                                size="sm"
                                onClick={() => onCommandOutput(output.id)}
                                disabled={!canCommand}
                              >
                                Send command
                              </NodeButton>
                            </div>
                          </Card>
                        );
                      })}
                      {!nodeOutputs.length ? (
 <p className="text-sm text-muted-foreground">
                          No outputs configured.
                        </p>
                      ) : null}
                    </div>
                  </CollapsibleCard>
                </div>
              </div>
            </CollapsibleCard>
          );
        })}

        {nodes.length === 0 ? (
          <Card className="gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
            No nodes found.
          </Card>
        ) : null}
      </CollapsibleCard>

      {sensorConfigNode ? (
        <AddSensorDrawer
          node={sensorConfigNode}
          nodes={nodes}
          sensors={sensors}
          canEdit={canEdit}
          mode={sensorConfigMode}
          hardwareSupported={sensorConfigNodeSupportsHardwareSensors}
          weatherStationSupported={sensorConfigNodeSupportsWeatherStationSensors}
          onModeChange={setSensorConfigMode}
          onClose={() => setSensorConfigNodeId(null)}
        />
      ) : null}
    </>
  );
}
