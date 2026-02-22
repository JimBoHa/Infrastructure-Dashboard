import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import type { DemoAdoptionCandidate, DemoNode } from "@/types/dashboard";

export default function AdoptionSection({
  discovered,
  adoption,
  nodes,
  onRefresh,
  onAdopt,
  onConnectWeatherStation,
  onOpenNode,
  refreshLoading = false,
  refreshLabel,
}: {
  discovered: DemoAdoptionCandidate[];
  adoption: DemoAdoptionCandidate[];
  nodes: DemoNode[];
  onRefresh?: () => void;
  onAdopt: (candidate: DemoAdoptionCandidate) => void;
  onConnectWeatherStation?: () => void;
  onOpenNode?: (nodeId: string) => void;
  refreshLoading?: boolean;
  refreshLabel?: string;
}) {
  const nodeByMac = new Map<string, DemoNode>();
  nodes.forEach((node) => {
    if (node.mac_eth) nodeByMac.set(node.mac_eth.toLowerCase(), node);
    if (node.mac_wifi) nodeByMac.set(node.mac_wifi.toLowerCase(), node);
  });

  const alreadyAdopted = discovered
    .map((candidate) => {
      const macEth = candidate.mac_eth?.toLowerCase();
      const macWifi = candidate.mac_wifi?.toLowerCase();
      const match = (macEth && nodeByMac.get(macEth)) || (macWifi && nodeByMac.get(macWifi)) || null;
      return match ? { candidate, node: match } : null;
    })
    .filter((entry): entry is { candidate: DemoAdoptionCandidate; node: DemoNode } => Boolean(entry));

  return (
    <CollapsibleCard
      title="Discovered nodes"
      description="Nodes broadcasting via mDNS. New nodes can be adopted; already-adopted nodes show up here so “Scan complete” is never confusing."
      actions={
        <>
          {onConnectWeatherStation ? (
            <NodeButton onClick={onConnectWeatherStation} size="sm">
              Add weather station (WS-2902)
            </NodeButton>
          ) : null}
          {onRefresh ? (
            <NodeButton onClick={onRefresh} loading={refreshLoading} size="sm">
              {refreshLabel ?? "Scan again"}
            </NodeButton>
          ) : null}
        </>
      }
    >
      <div className="space-y-6">
        <div>
          <div className="mb-2 flex items-center justify-between">
 <h4 className="text-sm font-semibold text-foreground">Ready to adopt</h4>
 <span className="text-xs text-muted-foreground">
              {adoption.length ? `${adoption.length} found` : "None found"}
            </span>
          </div>
          <div className="grid gap-4 md:grid-cols-2">
            {adoption.length ? (
              adoption.map((service) => {
                const properties = service.properties ?? {};
                return (
                  <Card
                    key={service.service_name}
                    className="gap-0 p-4"
                  >
                    <p className="text-sm font-semibold">
                      {properties.node_name ?? service.service_name}
                    </p>
 <p className="text-xs text-muted-foreground">
                      {service.hostname ?? "(no hostname)"} / {service.ip ?? "?"}:{service.port ?? "?"}
                    </p>
 <div className="mt-2 flex flex-wrap gap-2 text-[11px] text-muted-foreground">
                      <NodePill size="md">MAC eth {service.mac_eth ?? "-"}</NodePill>
                      <NodePill size="md">wifi {service.mac_wifi ?? "-"}</NodePill>
                      {properties.hw ? <NodePill size="md">{String(properties.hw)}</NodePill> : null}
                      {properties.fw ? <NodePill size="md">fw {String(properties.fw)}</NodePill> : null}
                    </div>
 <p className="mt-2 line-clamp-2 text-xs text-muted-foreground">
                      {Object.entries(properties)
                        .filter(([key]) => !key.startsWith("mesh_") && key !== "adoption_token")
                        .map(([key, val]) => `${key}: ${val}`)
                        .join(" / ")}
                    </p>
                    <NodeButton
                      onClick={() => onAdopt(service)}
                      variant="primary"
                      size="xs"
                      fullWidth
                      className="mt-3"
                    >
                      Adopt node
                    </NodeButton>
                  </Card>
                );
              })
            ) : (
 <p className="text-sm text-muted-foreground">
                No new nodes found. If you’re trying to deploy to a fresh host, use the Deployment tab.
              </p>
            )}
          </div>
        </div>

        <div>
          <div className="mb-2 flex items-center justify-between">
 <h4 className="text-sm font-semibold text-foreground">Already adopted</h4>
 <span className="text-xs text-muted-foreground">
              {alreadyAdopted.length ? `${alreadyAdopted.length} seen` : "None seen"}
            </span>
          </div>
          <div className="grid gap-4 md:grid-cols-2">
            {alreadyAdopted.length ? (
              alreadyAdopted.map(({ candidate, node }) => (
                <Card
                  key={`${candidate.service_name}:${node.id}`}
                  className="gap-0 bg-card-inset p-4 shadow-xs"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
 <p className="truncate text-sm font-semibold text-foreground">
                        {node.name}
                      </p>
 <p className="truncate text-xs text-muted-foreground">
                        {candidate.ip ?? "?"}:{candidate.port ?? "?"} · {candidate.hostname ?? "(no hostname)"}
                      </p>
                    </div>
                    <NodePill tone="success" size="md">
                      adopted
                    </NodePill>
                  </div>
 <div className="mt-2 flex flex-wrap gap-2 text-[11px] text-muted-foreground">
                    <NodePill size="md">MAC eth {candidate.mac_eth ?? "-"}</NodePill>
                    <NodePill size="md">wifi {candidate.mac_wifi ?? "-"}</NodePill>
                  </div>
                  {onOpenNode ? (
                    <NodeButton
                      onClick={() => onOpenNode(node.id)}
                      size="xs"
                      fullWidth
                      className="mt-3"
                    >
                      View node details
                    </NodeButton>
                  ) : null}
                </Card>
              ))
            ) : (
 <p className="text-sm text-muted-foreground">
                No adopted nodes were seen during this scan.
              </p>
            )}
          </div>
        </div>
      </div>
    </CollapsibleCard>
  );
}
