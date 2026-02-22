"use client";

import { useCallback, useState, type RefObject } from "react";
import type { QueryClient } from "@tanstack/react-query";
import { createMapFeature, deleteMapFeature } from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { GeoJsonGeometry } from "@/types/map";
import type { MapCanvasHandle, MapClickLngLat } from "@/features/map/MapCanvas";

export type PlaceTarget =
  | { kind: "node"; nodeId: string }
  | { kind: "sensor"; sensorId: string }
  | null;

type UseMapSelectionOptions = {
  canEdit: boolean;
  nodesById: Map<string, DemoNode>;
  sensorsById: Map<string, DemoSensor>;
  queryClient: QueryClient;
  mapRef: RefObject<MapCanvasHandle | null>;
};

export function useMapSelection({ canEdit, nodesById, sensorsById, queryClient, mapRef }: UseMapSelectionOptions) {
  const [placeTarget, setPlaceTarget] = useState<PlaceTarget>(null);

  const scrollMapIntoView = useCallback(() => {
    const el = document.getElementById("map-canvas");
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
  }, []);

  const startPlacement = useCallback(
    (target: Exclude<PlaceTarget, null>) => {
      mapRef.current?.stopDraw();
      setPlaceTarget(target);
      scrollMapIntoView();
    },
    [mapRef, scrollMapIntoView],
  );

  const clearPlacement = useCallback(() => {
    setPlaceTarget(null);
  }, []);

  const handleMapClick = useCallback(
    async (pos: MapClickLngLat) => {
      if (!placeTarget || !canEdit) return;

      const geometry: GeoJsonGeometry = { type: "Point", coordinates: [pos.lng, pos.lat] };

      try {
        if (placeTarget.kind === "node") {
          const node = nodesById.get(placeTarget.nodeId);
          await createMapFeature({
            node_id: placeTarget.nodeId,
            geometry,
            properties: {
              kind: "node",
              name: node?.name ?? "Node",
            },
          });
        } else if (placeTarget.kind === "sensor") {
          const sensor = sensorsById.get(placeTarget.sensorId);
          await createMapFeature({
            sensor_id: placeTarget.sensorId,
            geometry,
            properties: {
              kind: "sensor",
              name: sensor?.name ?? "Sensor",
              node_id: sensor?.node_id ?? null,
            },
          });
        }

        setPlaceTarget(null);
        await queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures });
      } catch (error) {
        console.error(error);
      }
    },
    [canEdit, nodesById, placeTarget, queryClient, sensorsById],
  );

  const handleUpsertEntityLocation = useCallback(
    async (payload: { nodeId?: string; sensorId?: string; lng: number; lat: number }) => {
      if (!canEdit) return;
      const geometry: GeoJsonGeometry = { type: "Point", coordinates: [payload.lng, payload.lat] };
      if (payload.nodeId) {
        const node = nodesById.get(payload.nodeId);
        await createMapFeature({
          node_id: payload.nodeId,
          geometry,
          properties: {
            kind: "node",
            name: node?.name ?? "Node",
          },
        });
      } else if (payload.sensorId) {
        const sensor = sensorsById.get(payload.sensorId);
        await createMapFeature({
          sensor_id: payload.sensorId,
          geometry,
          properties: {
            kind: "sensor",
            name: sensor?.name ?? "Sensor",
            node_id: sensor?.node_id ?? null,
          },
        });
      }
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures });
    },
    [canEdit, nodesById, sensorsById, queryClient],
  );

  const handleUnplaceFeature = useCallback(
    async (featureId: number) => {
      if (!canEdit) return;
      await deleteMapFeature(featureId);
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures });
    },
    [canEdit, queryClient],
  );

  return {
    placeTarget,
    setPlaceTarget,
    startPlacement,
    clearPlacement,
    scrollMapIntoView,
    handleMapClick,
    handleUpsertEntityLocation,
    handleUnplaceFeature,
  };
}
