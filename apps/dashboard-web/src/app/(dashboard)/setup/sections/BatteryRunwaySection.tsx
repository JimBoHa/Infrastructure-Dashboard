"use client";

import Link from "next/link";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

import BatteryRunwayNodeCard from "../components/BatteryRunwayNodeCard";

export default function BatteryRunwaySection({
  batteryConfigNodes,
  requestedNodeId,
  sensors,
  nodes,
  canEdit,
}: {
  batteryConfigNodes: DemoNode[];
  requestedNodeId: string | null;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  canEdit: boolean;
}) {
  return (
    <CollapsibleCard
      title="Battery SOC + power runway"
      description={
        <>
          Estimate battery SOC with coulomb counting (anchored to Renogy SOC when resting) and
          project a conservative runway using Forecast.Solar PV + an hour-of-day load profile
          learned from recent days.
        </>
      }
      defaultOpen={batteryConfigNodes.length > 0}
      bodyClassName="space-y-4"
      id="battery-runway"
    >
      <div className="grid gap-6 lg:grid-cols-2">
        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <p className="text-sm font-semibold text-card-foreground">How it works</p>
          <div className="mt-2 space-y-2 text-sm text-muted-foreground">
            <p>
              Battery SOC uses Renogy BT‑2 battery current integration, anchored when the battery
              is resting (low absolute current) so the estimate doesn&apos;t drift forever.
            </p>
            <p>
              Power runway uses your selected load power sensor(s) (must be{" "}
              <span className="font-semibold">W</span>) and derates PV to stay conservative.
            </p>
          </div>
        </Card>

        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <p className="text-sm font-semibold text-card-foreground">Warnings</p>
          <div className="mt-2 space-y-2 text-sm text-muted-foreground">
            <p>
              Runway projection is conservative beyond the PV forecast horizon (PV assumed{" "}
              <span className="font-semibold">0</span>).
            </p>
            <p>
              To get a warning, create an alarm on{" "}
              <span className="font-semibold">Power runway (conservative, hr)</span> (example:
              runway &lt; 72h). Use <Link className="underline" href="/alarms">Alarms</Link>.
            </p>
          </div>
        </Card>
      </div>

      {!canEdit ? (
        <Card className="rounded-lg gap-0 border-dashed p-4 text-sm text-muted-foreground">
          This section requires the <span className="font-semibold">config.write</span> capability.
        </Card>
      ) : batteryConfigNodes.length === 0 ? (
        <p className="mt-4 text-sm text-muted-foreground">
          No Renogy BT‑2 nodes detected yet. Apply a Renogy BT‑2 preset on a node (Power tab) to
          start ingesting telemetry, then configure SOC + runway here.
        </p>
      ) : (
        <div className="space-y-3">
          {batteryConfigNodes.map((node, idx) => (
            <BatteryRunwayNodeCard
              key={node.id}
              node={node}
              defaultOpen={
                requestedNodeId
                  ? requestedNodeId === node.id
                  : idx === 0 && batteryConfigNodes.length === 1
              }
              sensors={sensors}
              nodes={nodes}
              canEdit={canEdit}
            />
          ))}
        </div>
      )}
    </CollapsibleCard>
  );
}

