import AlarmOriginBadge from "@/components/alarms/AlarmOriginBadge";
import AnomalyScore from "@/components/alarms/AnomalyScore";
import { isPredictiveOrigin, type AlarmOriginFilter } from "@/lib/alarms/origin";
import filterAlarmsByOrigin from "@/features/sensors/utils/filterAlarmsByOrigin";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { formatSensorInterval, formatSensorValueWithUnit } from "@/lib/sensorFormat";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { Badge } from "@/components/ui/badge";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import type { DemoAlarm, DemoNode, DemoSensor } from "@/types/dashboard";

export default function SensorTable({
  sensors,
  nodes,
  alarms,
  alarmOriginFilter,
  groupByNode = false,
  onSelectSensor,
}: {
  sensors: DemoSensor[];
  nodes: DemoNode[];
  alarms: DemoAlarm[];
  alarmOriginFilter: AlarmOriginFilter;
  groupByNode?: boolean;
  onSelectSensor: (sensorId: string) => void;
}) {
  const nodesById = new Map(nodes.map((node) => [node.id, node]));

  const renderRows = (nodeNameOverride?: string) =>
    sensors.map((sensor) => {
      const nodeName =
        nodeNameOverride ?? nodesById.get(sensor.node_id)?.name ?? "Unknown";
      const interval = formatSensorInterval(sensor.interval_seconds);
      const sensorAlarms = filterAlarmsByOrigin(
        alarms.filter(
          (alarm) => alarm.target_type === "sensor" && alarm.target_id === sensor.sensor_id,
        ),
        alarmOriginFilter,
      );
      const predictiveCount = sensorAlarms.filter((alarm) =>
        isPredictiveOrigin(alarm.origin ?? alarm.type),
      ).length;
      const highestScore = sensorAlarms.reduce<number | null>((max, alarm) => {
        if (alarm.anomaly_score == null) return max;
        return max == null ? alarm.anomaly_score : Math.max(max, alarm.anomaly_score);
      }, null);
      return (
        <tr
          key={sensor.sensor_id}
 className="cursor-pointer hover:bg-muted"
          onClick={() => onSelectSensor(sensor.sensor_id)}
        >
          <td className="px-4 py-3">
            <div className="flex flex-wrap items-center gap-2">
 <div className="font-medium text-foreground">{sensor.name}</div>
              <SensorOriginBadge sensor={sensor} />
            </div>
 <div className="text-xs text-muted-foreground">{sensor.sensor_id}</div>
          </td>
          {!nodeNameOverride ? (
 <td className="px-4 py-3 text-muted-foreground">{nodeName}</td>
          ) : null}
 <td className="px-4 py-3 text-muted-foreground">{sensor.type}</td>
 <td className="px-4 py-3 text-muted-foreground">
            <span title={interval.title}>{interval.label}</span>
          </td>
 <td className="px-4 py-3 text-muted-foreground">
            {(sensor.rolling_avg_seconds ?? 0) > 0 ? `${sensor.rolling_avg_seconds}s` : "-"}
          </td>
 <td className="px-4 py-3 text-muted-foreground">
            {formatSensorValueWithUnit(sensor, sensor.latest_value, "-")}
          </td>
 <td className="px-4 py-3 text-muted-foreground">
            <div className="flex flex-wrap items-center gap-2">
              {sensorAlarms.length ? (
                <>
                  <Badge tone="warning">{sensorAlarms.length} active</Badge>
                  {predictiveCount > 0 && <AlarmOriginBadge origin="predictive" />}
                  {highestScore != null && <AnomalyScore score={highestScore} />}
                </>
              ) : (
 <span className="text-xs text-muted-foreground">None</span>
              )}
            </div>
          </td>
        </tr>
      );
    });

  if (groupByNode) {
    const sensorsByNode = new Map<string, DemoSensor[]>();
    sensors.forEach((sensor) => {
      const list = sensorsByNode.get(sensor.node_id) ?? [];
      list.push(sensor);
      sensorsByNode.set(sensor.node_id, list);
    });

    const nodeGroups: Array<{
      nodeId: string;
      name: string;
      status: string;
      lastSeen: DemoNode["last_seen"];
      sensors: DemoSensor[];
    }> = [];
    const seen = new Set<string>();
    for (const node of nodes) {
      const nodeSensors = sensorsByNode.get(node.id);
      if (!nodeSensors?.length) continue;
      seen.add(node.id);
      nodeGroups.push({
        nodeId: node.id,
        name: node.name,
        status: node.status,
        lastSeen: node.last_seen ?? null,
        sensors: nodeSensors,
      });
    }
    for (const [nodeId, nodeSensors] of sensorsByNode.entries()) {
      if (seen.has(nodeId)) continue;
      const node = nodesById.get(nodeId);
      nodeGroups.push({
        nodeId,
        name: node?.name ?? "Unknown node",
        status: node?.status ?? "unknown",
        lastSeen: node?.last_seen ?? null,
        sensors: nodeSensors,
      });
    }

    return (
      <Card className="gap-0 p-6">
 <h3 className="text-lg font-semibold text-foreground">Sensors</h3>
 <p className="mt-1 text-sm text-muted-foreground">
          Grouped by node for clarity. Click a sensor to inspect details and alarms.
        </p>

        <div className="mt-4 space-y-3">
          {nodeGroups.map((group) => (
            <CollapsibleCard
              key={group.nodeId}
              className="bg-card-inset shadow-xs"
              density="sm"
              title={group.name}
              description={`${group.sensors.length} sensors`}
              actions={
                <span
                  className={`shrink-0 rounded-full px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide ${
                    group.status === "online"
                      ? "bg-emerald-100 text-emerald-800"
                      : group.status === "offline"
                        ? "bg-rose-100 text-rose-800"
                        : "bg-muted text-foreground"
                  }`}
                >
                  {formatNodeStatusLabel(group.status, group.lastSeen)}
                </span>
              }
            >
              <div className="overflow-x-auto md:overflow-x-visible">
                <table className="min-w-full divide-y divide-border text-sm">
 <thead className="bg-muted">
                    <tr>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Name
                      </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Type
                      </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Interval
                      </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Rolling
                      </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Latest
                      </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Alarms
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {group.sensors.map((sensor) => {
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
                      const highestScore = sensorAlarms.reduce<number | null>((max, alarm) => {
                        if (alarm.anomaly_score == null) return max;
                        return max == null ? alarm.anomaly_score : Math.max(max, alarm.anomaly_score);
                      }, null);
                      return (
                        <tr
                          key={sensor.sensor_id}
 className="cursor-pointer hover:bg-muted"
                          onClick={() => onSelectSensor(sensor.sensor_id)}
                        >
                          <td className="px-4 py-3">
                            <div className="flex flex-wrap items-center gap-2">
 <div className="font-medium text-foreground">
                                {sensor.name}
                              </div>
                              <SensorOriginBadge sensor={sensor} />
                            </div>
 <div className="text-xs text-muted-foreground">
                              {sensor.sensor_id}
                            </div>
                          </td>
 <td className="px-4 py-3 text-muted-foreground">
                            {sensor.type}
                          </td>
 <td className="px-4 py-3 text-muted-foreground">
                            {(() => {
                              const interval = formatSensorInterval(sensor.interval_seconds);
                              return <span title={interval.title}>{interval.label}</span>;
                            })()}
                          </td>
 <td className="px-4 py-3 text-muted-foreground">
                            {(sensor.rolling_avg_seconds ?? 0) > 0
                              ? `${sensor.rolling_avg_seconds}s`
                              : "-"}
                          </td>
 <td className="px-4 py-3 text-muted-foreground">
                            {formatSensorValueWithUnit(sensor, sensor.latest_value, "-")}
                          </td>
 <td className="px-4 py-3 text-muted-foreground">
                            <div className="flex flex-wrap items-center gap-2">
                              {sensorAlarms.length ? (
                                <>
                                  <Badge tone="warning">{sensorAlarms.length} active</Badge>
                                  {predictiveCount > 0 && <AlarmOriginBadge origin="predictive" />}
                                  {highestScore != null && <AnomalyScore score={highestScore} />}
                                </>
                              ) : (
 <span className="text-xs text-muted-foreground">
                                  None
                                </span>
                              )}
                            </div>
                          </td>
                        </tr>
                      );
                    })}
                    {!group.sensors.length && (
                      <tr>
                        <td
                          colSpan={6}
 className="px-4 py-6 text-center text-sm text-muted-foreground"
                        >
                          No sensors match the current filters.
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>
              </div>
            </CollapsibleCard>
          ))}

          {!sensors.length && (
            <Card className="gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
              No sensors match the current filters.
            </Card>
          )}
        </div>
      </Card>
    );
  }

  return (
    <Card className="gap-0 p-6">
 <h3 className="text-lg font-semibold text-foreground">Sensors</h3>
      <div className="mt-4 overflow-x-auto md:overflow-x-visible">
        <table className="min-w-full divide-y divide-border text-sm">
 <thead className="bg-card-inset">
            <tr>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Name
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Node
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Type
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Interval
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Rolling
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Latest
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Alarms
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {renderRows()}
            {!sensors.length && (
              <tr>
 <td colSpan={7} className="px-4 py-6 text-center text-sm text-muted-foreground">
                  No sensors match the current filters.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </Card>
  );
}
