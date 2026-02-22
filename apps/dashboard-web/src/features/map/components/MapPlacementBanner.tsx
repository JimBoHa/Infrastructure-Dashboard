"use client";

import InlineBanner from "@/components/InlineBanner";
import NodeButton from "@/features/nodes/components/NodeButton";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { PlaceTarget } from "@/features/map/hooks/useMapSelection";

type MapPlacementBannerProps = {
  placeTarget: PlaceTarget;
  nodesById: Map<string, DemoNode>;
  sensorsById: Map<string, DemoSensor>;
  onCancel: () => void;
};

export default function MapPlacementBanner({
  placeTarget,
  nodesById,
  sensorsById,
  onCancel,
}: MapPlacementBannerProps) {
  if (!placeTarget) return null;

  const label =
    placeTarget.kind === "node"
      ? nodesById.get(placeTarget.nodeId)?.name ?? "node"
      : sensorsById.get(placeTarget.sensorId)?.name ?? "sensor";

  return (
    <InlineBanner tone="info" className="px-4 py-3">
      <div className="font-semibold">Placement mode</div>
      <div className="mt-1 flex flex-wrap items-center gap-2">
 <span className="text-indigo-700">Click on the map to place {label}.</span>
        <NodeButton size="sm" onClick={onCancel}>
          Cancel
        </NodeButton>
      </div>
    </InlineBanner>
  );
}
