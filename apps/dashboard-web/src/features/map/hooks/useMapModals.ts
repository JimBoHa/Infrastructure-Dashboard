"use client";

import { useCallback, useState, type RefObject } from "react";
import type { QueryClient } from "@tanstack/react-query";
import { createMapSave } from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import type { MapLayer, MapSettings } from "@/types/map";
import { type LayerDraft, layerDraftFromLayer } from "@/features/map/LayerEditorModal";
import type { FeatureDraft } from "@/features/map/FeatureEditorModal";
import type { MapCanvasHandle } from "@/features/map/MapCanvas";

export type LayerModalState = {
  mode: "create" | "edit";
  layerId?: number;
  draft: LayerDraft;
};

export type FeatureModalState = {
  mode: "create" | "edit";
  draft: FeatureDraft;
  drawId?: string;
  featureId?: number;
};

export type SaveModalState = {
  isOpen: boolean;
  nameDraft: string;
  busy: boolean;
  error: string | null;
  open: () => void;
  close: () => void;
  confirm: () => Promise<void>;
  setNameDraft: (value: string) => void;
};

type UseMapModalsOptions = {
  overlayLayers: MapLayer[];
  activeSaveName?: string | null;
  canEdit: boolean;
  settings: MapSettings | null | undefined;
  selectedBaseLayer: MapLayer | null;
  mapRef: RefObject<MapCanvasHandle | null>;
  queryClient: QueryClient;
};

export function useMapModals({
  overlayLayers,
  activeSaveName,
  canEdit,
  settings,
  selectedBaseLayer,
  mapRef,
  queryClient,
}: UseMapModalsOptions) {
  const [layerModal, setLayerModal] = useState<LayerModalState | null>(null);
  const [featureModal, setFeatureModal] = useState<FeatureModalState | null>(null);
  const [saveModalOpen, setSaveModalOpen] = useState(false);
  const [saveNameDraft, setSaveNameDraft] = useState("");
  const [saveBusy, setSaveBusy] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const openCreateLayer = useCallback(() => {
    setLayerModal({
      mode: "create",
      draft: {
        name: "",
        kind: "overlay",
        source_type: "xyz",
        enabled: true,
        opacity: 0.8,
        z_index: overlayLayers.length ? Math.max(...overlayLayers.map((layer) => layer.z_index)) + 1 : 0,
        xyz: { url_template: "" },
        wms: { base_url: "", layers: "" },
        arcgis: { base_url: "", mode: "tile" },
        geojson: { file_name: "", data: null },
      },
    });
  }, [overlayLayers]);

  const openEditLayer = useCallback((layer: MapLayer) => {
    setLayerModal({
      mode: "edit",
      layerId: layer.id,
      draft: layerDraftFromLayer(layer),
    });
  }, []);

  const openSaveAs = useCallback(() => {
    const base = activeSaveName?.trim() || "Map";
    const suggestion = `${base} copy`;
    setSaveNameDraft(suggestion);
    setSaveError(null);
    setSaveModalOpen(true);
  }, [activeSaveName]);

  const closeSaveAs = useCallback(() => {
    if (saveBusy) return;
    setSaveModalOpen(false);
  }, [saveBusy]);

  const confirmSaveAs = useCallback(async () => {
    if (!canEdit) return;
    const name = saveNameDraft.trim();
    if (!name) {
      setSaveError("Save name is required.");
      return;
    }
    const view = mapRef.current?.getView();
    if (!view) {
      setSaveError("Map view is not ready yet.");
      return;
    }

    setSaveBusy(true);
    setSaveError(null);
    try {
      await createMapSave({
        name,
        active_base_layer_id: settings?.active_base_layer_id ?? selectedBaseLayer?.id ?? null,
        center_lat: view.center_lat,
        center_lng: view.center_lng,
        zoom: view.zoom,
        bearing: view.bearing,
        pitch: view.pitch,
      });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.mapSaves }),
        queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures }),
        queryClient.invalidateQueries({ queryKey: queryKeys.mapSettings }),
      ]);
      setSaveModalOpen(false);
    } catch (error) {
      const message = error instanceof Error ? error.message : "Save failed.";
      setSaveError(message);
    } finally {
      setSaveBusy(false);
    }
  }, [
    canEdit,
    queryClient,
    saveNameDraft,
    selectedBaseLayer?.id,
    settings?.active_base_layer_id,
    mapRef,
  ]);

  const saveModal: SaveModalState = {
    isOpen: saveModalOpen,
    nameDraft: saveNameDraft,
    busy: saveBusy,
    error: saveError,
    open: openSaveAs,
    close: closeSaveAs,
    confirm: confirmSaveAs,
    setNameDraft: setSaveNameDraft,
  };

  return {
    layerModal,
    setLayerModal,
    openCreateLayer,
    openEditLayer,
    featureModal,
    setFeatureModal,
    saveModal,
  };
}
