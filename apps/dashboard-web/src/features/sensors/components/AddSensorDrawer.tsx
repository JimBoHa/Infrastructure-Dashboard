"use client";

import { useEffect } from "react";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodeTypeBadge from "@/features/nodes/components/NodeTypeBadge";
import NodeSensorsConfigSection from "@/features/nodes/components/NodeSensorsConfigSection";
import DerivedSensorBuilder from "@/features/sensors/components/DerivedSensorBuilder";
import Ws2902SensorBuilder from "@/features/sensors/components/Ws2902SensorBuilder";
import SegmentedControl from "@/components/SegmentedControl";
import { Sheet, SheetContent, SheetHeader, SheetBody } from "@/components/ui/sheet";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

export type AddSensorMode = "hardware" | "weather_station" | "derived";

export default function AddSensorDrawer({
  node,
  nodes,
  sensors,
  canEdit,
  mode,
  hardwareSupported,
  weatherStationSupported,
  onModeChange,
  onClose,
}: {
  node: DemoNode;
  nodes: DemoNode[];
  sensors: DemoSensor[];
  canEdit: boolean;
  mode: AddSensorMode;
  hardwareSupported: boolean;
  weatherStationSupported: boolean;
  onModeChange: (next: AddSensorMode) => void;
  onClose: () => void;
}) {
  const effectiveMode: AddSensorMode = (() => {
    if (mode === "hardware" && !hardwareSupported) {
      return weatherStationSupported ? "weather_station" : "derived";
    }
    if (mode === "weather_station" && !weatherStationSupported) {
      return "derived";
    }
    return mode;
  })();

  const modeOptions = [
    { value: "hardware", label: "Hardware", disabled: !hardwareSupported },
    ...(weatherStationSupported ? [{ value: "weather_station", label: "WS-2902" }] : []),
    { value: "derived", label: "Derived" },
  ] as const;

  useEffect(() => {
    if (mode === "hardware" && !hardwareSupported) {
      onModeChange(weatherStationSupported ? "weather_station" : "derived");
      return;
    }
    if (mode === "weather_station" && !weatherStationSupported) {
      onModeChange("derived");
    }
  }, [hardwareSupported, mode, onModeChange, weatherStationSupported]);

  return (
    <Sheet open onOpenChange={(open) => { if (!open) onClose(); }}>
      <SheetContent aria-label="Add sensor">
        <SheetHeader>
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
 <h3 className="text-lg font-semibold text-foreground">
                Add sensor
              </h3>
              <NodeTypeBadge node={node} size="sm" />
            </div>
 <p className="mt-0.5 truncate text-sm text-muted-foreground">
              {node.name}
            </p>
 <p className="mt-1 text-xs text-muted-foreground">
              {weatherStationSupported
                ? "WS\u20112902 sensors are ingested from station uploads. Derived sensors are computed by the controller."
                : "Hardware sensors are read on Pi nodes. Derived sensors are computed by the controller."}
              {!hardwareSupported ? " Hardware mode is unavailable for this node." : null}
            </p>
          </div>

          <div className="flex flex-wrap items-center justify-end gap-2">
            <SegmentedControl
              value={effectiveMode}
              size="xs"
              onChange={(next) => onModeChange(next as AddSensorMode)}
              options={modeOptions}
            />
            <NodeButton onClick={onClose} size="sm">
              Close
            </NodeButton>
          </div>
        </SheetHeader>

        <SheetBody>
          {effectiveMode === "derived" ? (
            <DerivedSensorBuilder ownerNodeId={node.id} nodes={nodes} sensors={sensors} canEdit={canEdit} />
          ) : effectiveMode === "weather_station" ? (
            <Ws2902SensorBuilder nodeId={node.id} sensors={sensors} canEdit={canEdit} />
          ) : (
            <NodeSensorsConfigSection
              nodeId={node.id}
              canEdit={canEdit}
              openByDefault
              initialAction="add"
              variant="drawer"
            />
          )}
        </SheetBody>
      </SheetContent>
    </Sheet>
  );
}
