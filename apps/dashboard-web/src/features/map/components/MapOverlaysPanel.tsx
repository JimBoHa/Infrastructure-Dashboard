"use client";

import { Card } from "@/components/ui/card";
import NodeButton from "@/features/nodes/components/NodeButton";
import type { MapLayer } from "@/types/map";

type MapOverlaysPanelProps = {
  canEdit: boolean;
  overlayLayers: MapLayer[];
  onAddLayer: () => void;
  onEditLayer: (layer: MapLayer) => void;
  onToggleLayerEnabled: (layer: MapLayer, enabled: boolean) => void;
  onUpdateLayerOpacity: (layer: MapLayer, opacity: number) => void;
  onReorderLayer: (layer: MapLayer, direction: "up" | "down") => void;
  onDeleteLayer: (layer: MapLayer) => void;
};

export default function MapOverlaysPanel({
  canEdit,
  overlayLayers,
  onAddLayer,
  onEditLayer,
  onToggleLayerEnabled,
  onUpdateLayerOpacity,
  onReorderLayer,
  onDeleteLayer,
}: MapOverlaysPanelProps) {
  return (
    <Card className="gap-0 p-4 shadow-xs">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-semibold text-card-foreground">Overlays</div>
 <div className="mt-1 text-xs text-muted-foreground">
            Add topo/survey overlays (WMS/ArcGIS/XYZ), adjust opacity, and reorder stacking.
          </div>
        </div>
        <NodeButton size="sm" onClick={onAddLayer} disabled={!canEdit}>
          Add
        </NodeButton>
      </div>

      {overlayLayers.length ? (
        <div className="mt-4 space-y-3">
          {overlayLayers
            .slice()
            .sort((a, b) => a.z_index - b.z_index || a.id - b.id)
            .map((layer) => (
              <Card
                key={layer.id}
                className="rounded-lg gap-0 bg-card-inset px-3 py-2"
              >
                <div className="flex items-start justify-between gap-2">
                  <label className="flex min-w-0 items-start gap-2">
                    <input
                      type="checkbox"
                      checked={layer.enabled}
                      disabled={!canEdit}
                      onChange={(event) => onToggleLayerEnabled(layer, event.target.checked)}
 className="mt-1 rounded border-input text-indigo-600 focus:ring-indigo-500 disabled:opacity-50"
                    />
                    <div className="min-w-0">
 <div className="truncate text-sm font-semibold text-foreground">
                        {layer.name}
                      </div>
 <div className="truncate text-xs text-muted-foreground">
                        {layer.source_type.toUpperCase()}
                      </div>
                    </div>
                  </label>

                  <div className="flex shrink-0 items-center gap-1">
                    <NodeButton size="xs" onClick={() => onReorderLayer(layer, "up")} disabled={!canEdit}>
                      ↑
                    </NodeButton>
                    <NodeButton size="xs" onClick={() => onReorderLayer(layer, "down")} disabled={!canEdit}>
                      ↓
                    </NodeButton>
                    <NodeButton size="xs" onClick={() => onEditLayer(layer)} disabled={!canEdit}>
                      Edit
                    </NodeButton>
                    <NodeButton size="xs" onClick={() => onDeleteLayer(layer)} disabled={!canEdit}>
                      Delete
                    </NodeButton>
                  </div>
                </div>

                <div className="mt-2 flex items-center gap-3">
 <div className="text-xs font-semibold text-muted-foreground">Opacity</div>
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.05}
                    value={layer.opacity}
                    disabled={!canEdit}
                    onChange={(event) => onUpdateLayerOpacity(layer, Number.parseFloat(event.target.value))}
                    className="flex-1"
                  />
 <div className="w-10 text-right text-xs text-muted-foreground">
                    {(layer.opacity * 100).toFixed(0)}%
                  </div>
                </div>
              </Card>
            ))}
        </div>
      ) : (
        <Card className="mt-4 rounded-lg gap-0 border-dashed px-3 py-4 text-center text-sm text-muted-foreground">
          No overlays yet. Add WMS/ArcGIS/XYZ layers (or upload GeoJSON/KML) to overlay topo/survey data.
        </Card>
      )}
    </Card>
  );
}
