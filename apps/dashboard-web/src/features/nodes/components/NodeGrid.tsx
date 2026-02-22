import { formatDistanceToNow } from "date-fns";
import { formatBytes, formatDuration, formatPercent } from "@/lib/format";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { formatSensorInterval } from "@/lib/sensorFormat";
import { isCoreNode } from "@/lib/coreNode";
import CollapsibleCard from "@/components/CollapsibleCard";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodeInfoList from "@/features/nodes/components/NodeInfoList";
import NodePill, { type NodePillTone } from "@/features/nodes/components/NodePill";
import NodeTypeBadge from "@/features/nodes/components/NodeTypeBadge";
import SensorOriginBadge, {
  sensorOriginBadgeMeta,
} from "@/features/sensors/components/SensorOriginBadge";
import type { DemoBackup, DemoNode, DemoOutput, DemoSensor } from "@/types/dashboard";

export default function NodeGrid({
  nodes,
  sensors,
  outputs,
  backupMap,
  onOpenNode,
}: {
  nodes: DemoNode[];
  sensors: DemoSensor[];
  outputs: DemoOutput[];
  backupMap: Record<string, DemoBackup[]>;
  onOpenNode: (id: string) => void;
}) {
  return (
    <div className="grid gap-6 lg:grid-cols-2">
      {nodes.map((node) => {
        const nodeSensors = sensors.filter((sensor) => sensor.node_id === node.id);
        const nodeOutputs = outputs.filter((output) => output.node_id === node.id);
        const config = (node.config || {}) as Record<string, unknown>;
        const lastBackup = backupMap[node.id]?.[0];
        const coreNode = isCoreNode(node);
        const meshRole = typeof config["mesh_role"] === "string" ? config["mesh_role"] : undefined;
        const lastBackupTimestamp = (() => {
          if (!lastBackup?.captured_at) return null;
          const timestamp = new Date(lastBackup.captured_at);
          if (Number.isNaN(timestamp.getTime())) return null;
          return formatDistanceToNow(timestamp, { addSuffix: true });
        })();
        const lastBackupDisplay = lastBackupTimestamp ? `Backup ${lastBackupTimestamp}` : "No backup yet";
        const ipLast =
          typeof node.ip_last === "string"
            ? node.ip_last
            : node.ip_last
              ? JSON.stringify(node.ip_last)
              : null;
        const statusTone: NodePillTone =
          node.status === "online"
            ? "success"
            : node.status === "maintenance"
            ? "warning"
            : "muted";

        return (
          <CollapsibleCard
            key={node.id}
            title={
              <div className="flex min-w-0 items-center gap-2">
                <span className="min-w-0 truncate">{node.name}</span>
                <NodeTypeBadge node={node} size="md" className="shrink-0" />
              </div>
            }
            description={
              <div className="space-y-0.5">
                <div>
                  {coreNode
                    ? "Controller / Core services"
                    : `${(config["hardware"] as string | undefined) ?? "Unknown"} / ${(config["firmware"] as string | undefined) ?? "n/a"}`}
                </div>
 <div className="text-xs text-muted-foreground">{lastBackupDisplay}</div>
              </div>
            }
            actions={
              <NodePill tone={statusTone} size="lg" caps>
                {formatNodeStatusLabel(node.status, node.last_seen)}
              </NodePill>
            }
          >
            <dl className="grid grid-cols-3 gap-4 text-sm">
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Uptime
                  </dt>
 <dd className="font-semibold text-foreground">
                    {formatDuration(node.uptime_seconds)}
                  </dd>
                </div>
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    CPU
                  </dt>
 <dd className="font-semibold text-foreground">
                    {formatPercent(node.cpu_percent)}
                  </dd>
                </div>
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    RAM
                  </dt>
 <dd className="font-semibold text-foreground">
                    {formatPercent(node.memory_percent ?? null)}
                  </dd>
                </div>
              </dl>

              <dl className="mt-3 grid grid-cols-3 gap-4 text-sm">
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Storage
                  </dt>
 <dd className="font-semibold text-foreground">
                    {formatBytes(node.storage_used_bytes)}
                  </dd>
                </div>
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    RAM used
                  </dt>
 <dd className="font-semibold text-foreground">
                    {formatBytes(node.memory_used_bytes ?? null)}
                  </dd>
                </div>
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Ping
                  </dt>
 <dd className="font-semibold text-foreground">
                    {(() => {
                      const ping = node.ping_ms ?? node.network_latency_ms;
                      if (typeof ping !== "number" || !Number.isFinite(ping)) return "—";
                      return `${Math.round(ping)}ms`;
                    })()}
                  </dd>
                </div>
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Ping (30m)
                  </dt>
 <dd className="font-semibold text-foreground">
                    {(() => {
                      const p50 = node.ping_p50_30m_ms;
                      if (typeof p50 !== "number" || !Number.isFinite(p50)) return "—";
                      return `${Math.round(p50)}ms`;
                    })()}
                  </dd>
                </div>
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Jitter
                  </dt>
 <dd className="font-semibold text-foreground">
                    {(() => {
                      const jitter = node.ping_jitter_ms ?? node.network_jitter_ms;
                      if (typeof jitter !== "number" || !Number.isFinite(jitter)) return "—";
                      return `${Math.round(jitter)}ms`;
                    })()}
                  </dd>
                </div>
                <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Link (24h)
                  </dt>
 <dd className="font-semibold text-foreground">
                    {formatPercent(node.uptime_percent_24h ?? null)}
                  </dd>
                </div>
              </dl>

              <div className="mt-4 grid gap-4 md:grid-cols-2">
                <NodeInfoList
                  title={`Sensors (${nodeSensors.length})`}
                  emptyLabel="No sensors linked."
                  maxItems={3}
                  items={nodeSensors.map((sensor) => ({
                    id: sensor.sensor_id,
                    name: sensor.name,
                    badges: sensorOriginBadgeMeta(sensor) ? (
                      <SensorOriginBadge sensor={sensor} size="xs" />
                    ) : null,
                    description: `${sensor.type} / interval ${formatSensorInterval(sensor.interval_seconds).label}${
                      (sensor.rolling_avg_seconds ?? 0) > 0
                        ? ` / rolling ${sensor.rolling_avg_seconds}s`
                        : ""
                    }`,
                  }))}
                />
                <NodeInfoList
                  title={`Outputs (${nodeOutputs.length})`}
                  emptyLabel="No outputs configured."
                  maxItems={3}
                  items={nodeOutputs.map((output) => ({
                    id: output.id,
                    name: output.name,
                    pill: output.state,
                    description: output.last_command
                      ? `Last command ${formatDistanceToNow(new Date(output.last_command), {
                          addSuffix: true,
                        })}`
                      : "No commands yet",
                  }))}
                />
              </div>

 <div className="mt-4 flex flex-wrap gap-3 text-xs text-muted-foreground">
              {ipLast && (
                <NodePill tone="muted" size="lg" weight="normal">
                  IP {ipLast}
                </NodePill>
              )}
              {meshRole && (
                <NodePill tone="muted" size="lg" weight="normal">
                  Mesh {meshRole}
                </NodePill>
              )}
              {lastBackupTimestamp && (
                <NodePill tone="muted" size="lg" weight="normal">
                  Last backup {lastBackupTimestamp}
                </NodePill>
              )}
            </div>

            <div className="mt-6">
              <NodeButton onClick={() => onOpenNode(node.id)} fullWidth>
                More details
              </NodeButton>
            </div>
          </CollapsibleCard>
        );
      })}
    </div>
  );
}
