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
import NodeButton from "@/features/nodes/components/NodeButton";
import type { MapFeature } from "@/types/map";
import { featureDraftFromMapFeature } from "@/features/map/FeatureEditorModal";
import type { FeatureModalState } from "@/features/map/hooks/useMapModals";

type MapMarkupPanelProps = {
  canEdit: boolean;
  featureSearch: string;
  onFeatureSearchChange: (value: string) => void;
  filteredCustomFeatures: MapFeature[];
  onStartDraw: (mode: "point" | "polygon" | "line") => void;
  onFocusGeometry: (geometry: unknown) => void;
  onOpenFeatureModal: (payload: FeatureModalState) => void;
  onDeleteFeature: (featureId: number) => Promise<void>;
};

export default function MapMarkupPanel({
  canEdit,
  featureSearch,
  onFeatureSearchChange,
  filteredCustomFeatures,
  onStartDraw,
  onFocusGeometry,
  onOpenFeatureModal,
  onDeleteFeature,
}: MapMarkupPanelProps) {
  const [pendingDelete, setPendingDelete] = useState<{ id: number; name: string } | null>(null);

  return (
    <Card className="gap-0 p-4 shadow-xs">
      <div className="flex items-start justify-between gap-3">
        <div>
          <div className="text-sm font-semibold text-card-foreground">Markup</div>
 <div className="mt-1 text-xs text-muted-foreground">
            Draw polygons/lines and add hardware markers to document fields, ditches, and utilities. Markup does not change node locations used
            for weather/forecasting.
          </div>
        </div>
        <div className="flex items-center gap-2">
          <NodeButton size="xs" onClick={() => onStartDraw("point")} disabled={!canEdit}>
            Marker
          </NodeButton>
          <NodeButton size="xs" onClick={() => onStartDraw("polygon")} disabled={!canEdit}>
            Polygon
          </NodeButton>
          <NodeButton size="xs" onClick={() => onStartDraw("line")} disabled={!canEdit}>
            Line
          </NodeButton>
        </div>
      </div>

      <div className="mt-4">
        <Input
          value={featureSearch}
          onChange={(event) => onFeatureSearchChange(event.target.value)}
          placeholder="Search markup (name, kind, id)…"
        />
      </div>

      {filteredCustomFeatures.length ? (
        <div className="mt-3 space-y-2">
          {filteredCustomFeatures
            .slice()
            .sort((a, b) => a.id - b.id)
            .slice(0, 24)
            .map((feature) => {
              const props = (feature.properties ?? {}) as Record<string, unknown>;
              const name =
                typeof props.name === "string" && props.name.trim().length
                  ? props.name.trim()
                  : `Feature #${feature.id}`;
              const kind = typeof props.kind === "string" ? props.kind : "";
              const geometryType = typeof feature.geometry?.type === "string" ? feature.geometry.type : "";
              const badge = geometryType === "Point" ? "marker" : geometryType === "LineString" ? "line" : "polygon";

              return (
                <Card
                  key={feature.id}
                  className="flex-row items-start justify-between gap-3 rounded-lg bg-card-inset px-3 py-2"
                >
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
 <Badge tone="neutral" size="md">
                        {badge}
                      </Badge>
 <div className="truncate text-sm font-semibold text-foreground">
                        {name}
                      </div>
                    </div>
 {kind ? <div className="truncate text-xs text-muted-foreground">{kind}</div> : null}
                  </div>

                  <div className="flex shrink-0 items-center gap-1">
                    <NodeButton size="xs" onClick={() => onFocusGeometry(feature.geometry)}>
                      Focus
                    </NodeButton>
                    <NodeButton
                      size="xs"
                      onClick={() =>
                        onOpenFeatureModal({
                          mode: "edit",
                          featureId: feature.id,
                          draft: featureDraftFromMapFeature(feature),
                        })
                      }
                      disabled={!canEdit}
                    >
                      Edit
                    </NodeButton>
                    <NodeButton
                      size="xs"
                      onClick={() => setPendingDelete({ id: feature.id, name })}
                      disabled={!canEdit}
                    >
                      Delete
                    </NodeButton>
                  </div>
                </Card>
              );
            })}
          {filteredCustomFeatures.length > 24 ? (
 <div className="text-xs text-muted-foreground">
              Showing 24 of {filteredCustomFeatures.length} results. Refine your search to narrow the list.
            </div>
          ) : null}
        </div>
      ) : (
        <Card className="mt-3 rounded-lg gap-0 border-dashed px-3 py-4 text-center text-sm text-muted-foreground">
          No markup yet. Use the buttons above to draw polygons/lines or add hardware markers.
        </Card>
      )}

      <AlertDialog open={!!pendingDelete} onOpenChange={(open) => { if (!open) setPendingDelete(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete markup</AlertDialogTitle>
            <AlertDialogDescription>
              Delete &ldquo;{pendingDelete?.name}&rdquo;? This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (pendingDelete) void onDeleteFeature(pendingDelete.id);
                setPendingDelete(null);
              }}
            >
              Delete
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  );
}
