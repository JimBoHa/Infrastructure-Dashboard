"use client";

import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import InlineBanner from "@/components/InlineBanner";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodeTypeBadge from "@/features/nodes/components/NodeTypeBadge";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { MapFeature } from "@/types/map";

type MapDevicesPanelProps = {
  canEdit: boolean;
  nodes: DemoNode[];
  deviceSearch: string;
  onDeviceSearchChange: (value: string) => void;
  filteredNodes: DemoNode[];
  deviceQuery: string;
  featuresByNode: Map<string, MapFeature>;
  featuresBySensor: Map<string, MapFeature>;
  sensorsByNode: Map<string, DemoSensor[]>;
  filterSensor: (sensor: DemoSensor) => boolean;
  expandedNodeIds: Set<string>;
  onToggleNodeExpanded: (nodeId: string) => void;
  unplacedNodes: DemoNode[];
  filteredUnassignedSensors: DemoSensor[];
  formatCoords: (geometry: unknown) => string | null;
  onFocusGeometry: (geometry: unknown) => void;
  onOpenNode: (nodeId: string) => void;
  onOpenSensor: (nodeId: string, sensorId: string) => void;
  onOpenSensorsList: () => void;
  onStartPlaceNode: (nodeId: string) => void;
  onStartPlaceSensor: (sensorId: string) => void;
  onClearFeature: (featureId: number) => Promise<void>;
};

export default function MapDevicesPanel({
  canEdit,
  nodes,
  deviceSearch,
  onDeviceSearchChange,
  filteredNodes,
  deviceQuery,
  featuresByNode,
  featuresBySensor,
  sensorsByNode,
  filterSensor,
  expandedNodeIds,
  onToggleNodeExpanded,
  unplacedNodes,
  filteredUnassignedSensors,
  formatCoords,
  onFocusGeometry,
  onOpenNode,
  onOpenSensor,
  onOpenSensorsList,
  onStartPlaceNode,
  onStartPlaceSensor,
  onClearFeature,
}: MapDevicesPanelProps) {
  const [pendingClear, setPendingClear] = useState<{ id: number; label: string } | null>(null);

  return (
    <Card className="gap-0 p-4 shadow-xs">
      <div className="text-sm font-semibold text-card-foreground">Devices</div>
 <div className="mt-1 text-xs text-muted-foreground">
        Click a node to expand its sensors. Click “Place”/“Move”, then click the map. Use Markup for markers/polygons/lines (they don’t affect
        weather/forecasting).
      </div>

      <div className="mt-4">
        <Input
          value={deviceSearch}
          onChange={(event) => onDeviceSearchChange(event.target.value)}
          placeholder="Search nodes/sensors…"
        />
      </div>

      <div className="mt-4 space-y-3 text-sm">
        <Card className="rounded-lg gap-0 bg-card-inset p-3">
          <div className="flex items-center justify-between gap-2">
            <div className="font-semibold text-card-foreground">Nodes</div>
 <div className="text-xs text-muted-foreground">
              {nodes.length - unplacedNodes.length}/{nodes.length} placed
            </div>
          </div>

          <div className="mt-3 max-h-[520px] space-y-2 overflow-auto pr-1">
            {filteredNodes.length ? (
              filteredNodes.map((node) => {
                const nodeFeature = featuresByNode.get(node.id) ?? null;
                const coords = nodeFeature ? formatCoords(nodeFeature.geometry) : null;
                const nodeSensors = (sensorsByNode.get(node.id) ?? []).filter(filterSensor);
                const expanded = expandedNodeIds.has(node.id);
                return (
                  <Card
                    key={node.id}
                    className="rounded-lg gap-0 px-2 py-2"
                  >
                    <div className="flex items-start justify-between gap-2">
                      <button
                        type="button"
                        onClick={() => onToggleNodeExpanded(node.id)}
                        className="min-w-0 flex-1 text-left"
                        aria-expanded={expanded}
                      >
                        <div className="flex min-w-0 items-center gap-2">
 <div className="min-w-0 truncate text-xs font-semibold text-foreground">
                            <span title={node.name}>{node.name}</span>
                          </div>
                          <NodeTypeBadge node={node} size="sm" className="shrink-0" />
                        </div>
 <div className="truncate text-[11px] text-muted-foreground">
                          {nodeFeature ? "Placed" : "Not placed"}
                          {coords ? ` · ${coords}` : ""} · {nodeSensors.length} sensors
                        </div>
                      </button>
                      <div className="flex shrink-0 items-center gap-1">
                        {nodeFeature ? (
                          <NodeButton size="xs" onClick={() => onFocusGeometry(nodeFeature.geometry)}>
                            Focus
                          </NodeButton>
                        ) : null}
                        <NodeButton size="xs" onClick={() => onOpenNode(node.id)}>
                          Open
                        </NodeButton>
                        <NodeButton size="xs" onClick={() => onStartPlaceNode(node.id)} disabled={!canEdit}>
                          {nodeFeature ? "Move" : "Place"}
                        </NodeButton>
                        {nodeFeature ? (
                          <NodeButton
                            size="xs"
                            onClick={() => setPendingClear({ id: nodeFeature.id, label: node.name })}
                            disabled={!canEdit}
                          >
                            Clear
                          </NodeButton>
                        ) : null}
                      </div>
                    </div>

                    {expanded ? (
                      <div className="mt-2 border-t border-border pt-2">
                        {nodeSensors.length ? (
                          <div className="space-y-2">
                            {nodeSensors.slice(0, 12).map((sensor) => {
                              const sensorFeature = featuresBySensor.get(sensor.sensor_id) ?? null;
                              const badge = sensorFeature ? "Custom" : nodeFeature ? "Node" : "Needs node";
                              return (
                                <div key={sensor.sensor_id} className="flex items-start justify-between gap-2">
                                  <div className="min-w-0">
                                    <div className="flex items-center gap-2">
                                      <div className="truncate text-xs font-semibold text-card-foreground">
                                        <span title={sensor.name}>{sensor.name}</span>
                                      </div>
                                      <SensorOriginBadge sensor={sensor} size="xs" />
                                      <Badge tone="muted" size="sm" className="border border-border bg-card-inset text-[10px]">
                                        {badge}
                                      </Badge>
                                    </div>
 <div className="truncate text-[11px] text-muted-foreground">
                                      <span title={sensor.sensor_id}>{sensor.sensor_id}</span>
                                    </div>
                                  </div>
                                  <div className="flex shrink-0 items-center gap-1">
                                    {sensorFeature ? (
                                      <NodeButton size="xs" onClick={() => onFocusGeometry(sensorFeature.geometry)}>
                                        Focus
                                      </NodeButton>
                                    ) : null}
                                    <NodeButton size="xs" onClick={() => onOpenSensor(sensor.node_id, sensor.sensor_id)}>
                                      Open
                                    </NodeButton>
                                    <NodeButton
                                      size="xs"
                                      onClick={() => onStartPlaceSensor(sensor.sensor_id)}
                                      disabled={!canEdit}
                                    >
                                      {sensorFeature ? "Move" : "Place"}
                                    </NodeButton>
                                    {sensorFeature ? (
                                      <NodeButton
                                        size="xs"
                                        onClick={() => setPendingClear({ id: sensorFeature.id, label: sensor.name })}
                                        disabled={!canEdit}
                                      >
                                        Clear
                                      </NodeButton>
                                    ) : null}
                                  </div>
                                </div>
                              );
                            })}
                            {nodeSensors.length > 12 ? (
 <div className="text-[11px] text-muted-foreground">
                                Showing 12 of {nodeSensors.length} sensors. Use search to narrow.
                              </div>
                            ) : null}
                          </div>
                        ) : (
 <div className="text-xs text-muted-foreground">No sensors attached to this node.</div>
                        )}
                      </div>
                    ) : null}
                  </Card>
                );
              })
            ) : (
 <div className="text-xs text-muted-foreground">
                {deviceQuery ? "No nodes match the filter." : "No nodes found."}
              </div>
            )}
          </div>
        </Card>

        {filteredUnassignedSensors.length ? (
          <InlineBanner tone="warning" className="px-3 py-2 text-xs">
            <div className="font-semibold">Unassigned sensors</div>
            <div className="mt-1">
              {filteredUnassignedSensors.length} sensors are not assigned to any node, so they cannot inherit a location.
            </div>
            <div className="mt-2">
              <NodeButton size="xs" onClick={onOpenSensorsList}>
                Open Sensors &amp; Outputs
              </NodeButton>
            </div>
          </InlineBanner>
        ) : null}
      </div>

      {!canEdit ? (
        <InlineBanner tone="warning" className="mt-4 px-3 py-2 text-xs">
          You are in read-only mode. Ask an admin for `config.write` to place devices, draw markup, or edit map layers.
        </InlineBanner>
      ) : null}

      <AlertDialog open={!!pendingClear} onOpenChange={(open) => { if (!open) setPendingClear(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Remove location</AlertDialogTitle>
            <AlertDialogDescription>
              Remove location for &ldquo;{pendingClear?.label}&rdquo;?
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (pendingClear) void onClearFeature(pendingClear.id);
                setPendingClear(null);
              }}
            >
              Remove
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  );
}
