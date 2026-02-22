"use client";

import { useCallback, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter } from "next/navigation";
import ErrorState from "@/components/ErrorState";
import LoadingState from "@/components/LoadingState";
import { useAuth } from "@/components/AuthProvider";
import NodeButton from "@/features/nodes/components/NodeButton";
import PageHeaderCard from "@/components/PageHeaderCard";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
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
import {
  applyMapSave,
  createMapFeature,
  createMapLayer,
  deleteMapFeature,
  deleteMapLayer,
  updateMapFeature,
  updateMapLayer,
  updateMapSettings,
} from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import type { GeoJsonGeometry, MapLayer, MapLayerUpsertPayload } from "@/types/map";
import { MapCanvas, type MapCanvasHandle } from "@/features/map/MapCanvas";
import { LayerEditorModal, layerDraftToUpsertPayload } from "@/features/map/LayerEditorModal";
import { FeatureEditorModal } from "@/features/map/FeatureEditorModal";
import MapOfflinePackCard from "@/features/map/components/MapOfflinePackCard";
import MapBaseLayerCard from "@/features/map/components/MapBaseLayerCard";
import MapDevicesPanel from "@/features/map/components/MapDevicesPanel";
import MapMarkupPanel from "@/features/map/components/MapMarkupPanel";
import MapOverlaysPanel from "@/features/map/components/MapOverlaysPanel";
import MapSaveModal from "@/features/map/components/MapSaveModal";
import MapPlacementBanner from "@/features/map/components/MapPlacementBanner";
import { useMapContext } from "@/features/map/hooks/useMapDerivedData";
import { useViewportFillHeight } from "@/features/map/hooks/useViewportFillHeight";
import { useMapSelection } from "@/features/map/hooks/useMapSelection";
import { useMapSidebarFilters } from "@/features/map/hooks/useMapSidebarFilters";
import { useMapModals } from "@/features/map/hooks/useMapModals";

const layerToUpsertPayload = (layer: MapLayer, patch: Partial<MapLayerUpsertPayload> = {}): MapLayerUpsertPayload => ({
  name: patch.name ?? layer.name,
  kind: patch.kind ?? layer.kind,
  source_type: patch.source_type ?? layer.source_type,
  config: patch.config ?? layer.config,
  opacity: patch.opacity ?? layer.opacity,
  enabled: patch.enabled ?? layer.enabled,
  z_index: patch.z_index ?? layer.z_index,
});

export default function MapPageClient() {
  const queryClient = useQueryClient();
  const router = useRouter();
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));

  const mapRef = useRef<MapCanvasHandle | null>(null);
  const [baseLayerOverrideId, setBaseLayerOverrideId] = useState<number | null>(null);
  const [pendingDeleteLayer, setPendingDeleteLayer] = useState<MapLayer | null>(null);

  const {
    nodes,
    sensors,
    saves,
    settings,
    layers,
    mapReady,
    mapLoading,
    mapError,
    baseLayers,
    overlayLayers,
    offlinePacksById,
    baseLayerBySystemKey,
    nodesById,
    sensorsById,
    sensorsByNode,
    featuresByNode,
    featuresBySensor,
    customFeatures,
    customFeaturesById,
    nodeFeatureCollection,
    sensorFeatureCollection,
    customFeatureCollection,
  } = useMapContext();

  const swantonPack = useMemo(() => offlinePacksById.get("swanton_ca") ?? null, [offlinePacksById]);
  const swantonInstalled = swantonPack?.status === "installed";

  const selectedBaseLayer = useMemo(() => {
    const activeId = baseLayerOverrideId ?? settings?.active_base_layer_id ?? null;

    const byId = activeId != null ? baseLayers.find((layer) => layer.id === activeId) ?? null : null;
    const fallback = baseLayers.find((layer) => layer.system_key === "streets") ?? baseLayers[0] ?? null;
    const candidate = byId ?? fallback;
    if (!candidate) return null;

    const systemKey = candidate.system_key;
    if (systemKey && systemKey.startsWith("offline_") && !swantonInstalled) {
      const onlineKey = systemKey.replace(/^offline_/, "");
      return baseLayerBySystemKey.get(onlineKey) ?? candidate;
    }

    if (systemKey && swantonInstalled && (systemKey === "streets" || systemKey === "satellite" || systemKey === "topo")) {
      const offline = baseLayerBySystemKey.get(`offline_${systemKey}`);
      return offline ?? candidate;
    }

    return candidate;
  }, [baseLayerBySystemKey, baseLayerOverrideId, baseLayers, settings?.active_base_layer_id, swantonInstalled]);

  const {
    placeTarget,
    startPlacement,
    clearPlacement,
    scrollMapIntoView,
    handleMapClick,
    handleUpsertEntityLocation,
    handleUnplaceFeature,
  } = useMapSelection({ canEdit, nodesById, sensorsById, queryClient, mapRef });

  const {
    deviceSearch,
    setDeviceSearch,
    deviceQuery,
    featureSearch,
    setFeatureSearch,
    expandedNodeIds,
    toggleNodeExpanded,
    showAllBaseLayers,
    setShowAllBaseLayers,
    filteredCustomFeatures,
    unplacedNodes,
    filteredNodes,
    filterSensor,
    filteredUnassignedSensors,
    formatCoords,
  } = useMapSidebarFilters({ nodes, sensors, featuresByNode, featuresBySensor, customFeatures, sensorsByNode });

  const { layerModal, setLayerModal, openCreateLayer, openEditLayer, featureModal, setFeatureModal, saveModal } =
    useMapModals({
      overlayLayers,
      activeSaveName: settings?.active_save_name,
      canEdit,
      settings,
      selectedBaseLayer,
      mapRef,
      queryClient,
    });

  const { viewportFillRef, viewportFillHeightPx } = useViewportFillHeight({
    dependencies: [placeTarget, mapReady],
  });

  const handleStartZoomTo300 = useCallback(() => {
    mapRef.current?.zoomToFeet(300);
  }, []);

	  const handleSaveView = useCallback(async () => {
	    if (!canEdit) return;
	    const view = mapRef.current?.getView();
	    if (!view) return;
	    try {
	      await updateMapSettings({
	        active_base_layer_id: selectedBaseLayer?.id ?? settings?.active_base_layer_id ?? null,
	        center_lat: view.center_lat,
	        center_lng: view.center_lng,
	        zoom: view.zoom,
	        bearing: view.bearing,
	        pitch: view.pitch,
	      });
	      await queryClient.invalidateQueries({ queryKey: queryKeys.mapSettings });
	    } catch (error) {
	      console.error(error);
	    }
	  }, [canEdit, queryClient, selectedBaseLayer, settings]);

  const handleApplySave = useCallback(
    async (saveId: number) => {
      if (!canEdit) return;
      if (!Number.isFinite(saveId) || saveId <= 0) return;
      try {
        const updated = await applyMapSave(saveId);
        queryClient.setQueryData(queryKeys.mapSettings, updated);
        await Promise.all([
          queryClient.invalidateQueries({ queryKey: queryKeys.mapSaves }),
          queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures }),
          queryClient.invalidateQueries({ queryKey: queryKeys.mapSettings }),
        ]);
      } catch (error) {
        console.error(error);
      }
    },
    [canEdit, queryClient],
  );

  const handleSelectBaseLayer = useCallback(
    async (layerId: number) => {
      setBaseLayerOverrideId(layerId);
      if (!canEdit) return;
      if (!settings) return;
      try {
        await updateMapSettings({
          active_base_layer_id: layerId,
          center_lat: settings.center_lat,
          center_lng: settings.center_lng,
          zoom: settings.zoom,
          bearing: settings.bearing,
          pitch: settings.pitch,
        });
        await queryClient.invalidateQueries({ queryKey: queryKeys.mapSettings });
        setBaseLayerOverrideId(null);
      } catch (error) {
        console.error(error);
      }
    },
    [canEdit, settings, queryClient],
  );

  const handleToggleLayerEnabled = useCallback(
    async (layer: MapLayer, enabled: boolean) => {
      if (!canEdit) return;
      await updateMapLayer(layer.id, layerToUpsertPayload(layer, { enabled }));
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapLayers });
    },
    [canEdit, queryClient],
  );

  const handleUpdateLayerOpacity = useCallback(
    async (layer: MapLayer, opacity: number) => {
      if (!canEdit) return;
      await updateMapLayer(layer.id, layerToUpsertPayload(layer, { opacity }));
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapLayers });
    },
    [canEdit, queryClient],
  );

  const handleReorderLayer = useCallback(
    async (layer: MapLayer, dir: "up" | "down") => {
      if (!canEdit) return;
      const sorted = overlayLayers.slice().sort((a, b) => a.z_index - b.z_index || a.id - b.id);
      const idx = sorted.findIndex((l) => l.id === layer.id);
      const swapIdx = dir === "up" ? idx - 1 : idx + 1;
      if (idx < 0 || swapIdx < 0 || swapIdx >= sorted.length) return;
      const a = sorted[idx];
      const b = sorted[swapIdx];
      await Promise.all([
        updateMapLayer(a.id, layerToUpsertPayload(a, { z_index: b.z_index })),
        updateMapLayer(b.id, layerToUpsertPayload(b, { z_index: a.z_index })),
      ]);
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapLayers });
    },
    [canEdit, overlayLayers, queryClient],
  );

  const handleDeleteLayer = useCallback(
    (layer: MapLayer) => {
      if (!canEdit) return;
      setPendingDeleteLayer(layer);
    },
    [canEdit],
  );

  const confirmDeleteLayer = useCallback(async () => {
    if (!pendingDeleteLayer) return;
    await deleteMapLayer(pendingDeleteLayer.id);
    setPendingDeleteLayer(null);
    await queryClient.invalidateQueries({ queryKey: queryKeys.mapLayers });
  }, [pendingDeleteLayer, queryClient]);

  const handleCreateLayer = useCallback(
    async (draft: Parameters<typeof layerDraftToUpsertPayload>[0]) => {
      if (!canEdit) return;
      const payload = layerDraftToUpsertPayload(draft);
      if (!payload) return;
      const created = await createMapLayer(payload);
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapLayers });
      return created;
    },
    [canEdit, queryClient],
  );

  const handleUpdateLayer = useCallback(
    async (layerId: number, draft: Parameters<typeof layerDraftToUpsertPayload>[0]) => {
      if (!canEdit) return;
      const payload = layerDraftToUpsertPayload(draft);
      if (!payload) return;
      await updateMapLayer(layerId, payload);
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapLayers });
    },
    [canEdit, queryClient],
  );

  const handleCreateCustomFeature = useCallback(
    async (geometry: unknown, properties: Record<string, unknown>) => {
      if (!canEdit) return null;
      const created = await createMapFeature({ geometry: geometry as GeoJsonGeometry, properties });
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures });
      return created;
    },
    [canEdit, queryClient],
  );

  const handleUpdateCustomFeature = useCallback(
    async (featureId: number, geometry: unknown, properties: Record<string, unknown>) => {
      if (!canEdit) return;
      await updateMapFeature(featureId, { geometry, properties });
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures });
    },
    [canEdit, queryClient],
  );

  const handleDeleteCustomFeature = useCallback(
    async (featureId: number) => {
      if (!canEdit) return;
      await deleteMapFeature(featureId);
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures });
    },
    [canEdit, queryClient],
  );

  const handleStartDraw = useCallback(
    (mode: "point" | "polygon" | "line") => {
      mapRef.current?.startDraw(mode);
      scrollMapIntoView();
    },
    [scrollMapIntoView],
  );

  const handleFocusGeometry = useCallback((geometry: unknown) => {
    mapRef.current?.focusGeometry(geometry);
  }, []);

  const handleOpenNode = useCallback(
    (nodeId: string) => {
      router.push(`/nodes/detail?id=${encodeURIComponent(nodeId)}`);
    },
    [router],
  );

  const handleOpenSensor = useCallback(
    (nodeId: string, sensorId: string) => {
      router.push(`/sensors?node=${encodeURIComponent(nodeId)}&sensor=${encodeURIComponent(sensorId)}`);
    },
    [router],
  );

  const handleOpenSensorsList = useCallback(() => {
    router.push("/sensors");
  }, [router]);

  if (mapLoading) {
    return <LoadingState label="Loading map…" />;
  }

  if (mapError) {
    return <ErrorState message={mapError instanceof Error ? mapError.message : "Failed to load map data."} />;
  }

  if (!mapReady || !settings) {
    return <LoadingState label="Preparing map…" />;
  }

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Map"
        description="Place nodes and document the farm with markup layers. Node pins drive location-based features (weather, PV forecasts); sensors inherit their node placement unless you override them."
        actions={
          <>
            <Card className="flex-row items-center gap-2 px-3 py-2 shadow-xs">
 <label className="text-xs font-semibold text-foreground">Saved map</label>
              <Select
                className="h-8 max-w-[220px] px-2 text-xs text-foreground"
                value={settings.active_save_id}
                onChange={(event) => void handleApplySave(Number(event.target.value))}
                disabled={!canEdit || !saves.length}
              >
                {saves.map((save) => (
                  <option key={save.id} value={save.id}>
                    {save.name}
                  </option>
                ))}
              </Select>
            </Card>

            <NodeButton size="sm" onClick={saveModal.open} disabled={!canEdit}>
              Save as…
            </NodeButton>
            <NodeButton size="sm" onClick={handleStartZoomTo300}>
              Zoom to ~300′
            </NodeButton>
            <NodeButton size="sm" onClick={handleSaveView} disabled={!canEdit}>
              Save view
            </NodeButton>
          </>
        }
      >
        <MapPlacementBanner
          placeTarget={placeTarget}
          nodesById={nodesById}
          sensorsById={sensorsById}
          onCancel={clearPlacement}
        />
      </PageHeaderCard>

      <div ref={viewportFillRef} className="grid min-w-0 gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,420px)]">
        <Card
          id="map-canvas"
          className="relative h-[44vh] min-h-[360px] min-w-0 gap-0 overflow-hidden p-0 shadow-xs lg:h-[52vh] lg:min-h-[420px]"
          style={viewportFillHeightPx ? { height: viewportFillHeightPx } : undefined}
        >
          <MapCanvas
            ref={mapRef}
            canEdit={canEdit}
            settings={settings}
            baseLayer={selectedBaseLayer}
            overlayLayers={overlayLayers}
            nodeFeatureCollection={nodeFeatureCollection}
            sensorFeatureCollection={sensorFeatureCollection}
            customFeatureCollection={customFeatureCollection}
            customFeaturesById={customFeaturesById}
            placementActive={Boolean(placeTarget)}
            onMapClick={handleMapClick}
            onUpsertEntityLocation={handleUpsertEntityLocation}
            onUpdateCustomFeature={handleUpdateCustomFeature}
            onDeleteCustomFeature={handleDeleteCustomFeature}
            onOpenFeatureDraft={(payload) => setFeatureModal(payload)}
          />
        </Card>

        <div
          className="min-w-0 space-y-4 lg:h-[52vh] lg:overflow-y-auto lg:pr-1"
          style={viewportFillHeightPx ? { height: viewportFillHeightPx } : undefined}
        >
          <MapOfflinePackCard canEdit={canEdit} swantonPack={swantonPack} />
          <MapBaseLayerCard
            canEdit={canEdit}
            baseLayers={baseLayers}
            baseLayerBySystemKey={baseLayerBySystemKey}
            selectedBaseLayer={selectedBaseLayer}
            swantonInstalled={swantonInstalled}
            showAllBaseLayers={showAllBaseLayers}
            onToggleShowAllBaseLayers={() => setShowAllBaseLayers((prev) => !prev)}
            onSelectBaseLayer={handleSelectBaseLayer}
            onEditLayer={openEditLayer}
          />
          <MapDevicesPanel
            canEdit={canEdit}
            nodes={nodes}
            deviceSearch={deviceSearch}
            onDeviceSearchChange={(value) => setDeviceSearch(value)}
            filteredNodes={filteredNodes}
            deviceQuery={deviceQuery}
            featuresByNode={featuresByNode}
            featuresBySensor={featuresBySensor}
            sensorsByNode={sensorsByNode}
            filterSensor={filterSensor}
            expandedNodeIds={expandedNodeIds}
            onToggleNodeExpanded={toggleNodeExpanded}
            unplacedNodes={unplacedNodes}
            filteredUnassignedSensors={filteredUnassignedSensors}
            formatCoords={formatCoords}
            onFocusGeometry={handleFocusGeometry}
            onOpenNode={handleOpenNode}
            onOpenSensor={handleOpenSensor}
            onOpenSensorsList={handleOpenSensorsList}
            onStartPlaceNode={(nodeId) => startPlacement({ kind: "node", nodeId })}
            onStartPlaceSensor={(sensorId) => startPlacement({ kind: "sensor", sensorId })}
            onClearFeature={handleUnplaceFeature}
          />
          <MapMarkupPanel
            canEdit={canEdit}
            featureSearch={featureSearch}
            onFeatureSearchChange={(value) => setFeatureSearch(value)}
            filteredCustomFeatures={filteredCustomFeatures}
            onStartDraw={handleStartDraw}
            onFocusGeometry={handleFocusGeometry}
            onOpenFeatureModal={(payload) => setFeatureModal(payload)}
            onDeleteFeature={handleDeleteCustomFeature}
          />
          <MapOverlaysPanel
            canEdit={canEdit}
            overlayLayers={overlayLayers}
            onAddLayer={openCreateLayer}
            onEditLayer={openEditLayer}
            onToggleLayerEnabled={handleToggleLayerEnabled}
            onUpdateLayerOpacity={handleUpdateLayerOpacity}
            onReorderLayer={handleReorderLayer}
            onDeleteLayer={handleDeleteLayer}
          />
        </div>
      </div>

      {layerModal ? (
        <LayerEditorModal
          mode={layerModal.mode}
          draft={layerModal.draft}
          existing={layerModal.layerId != null ? layers.find((layer) => layer.id === layerModal.layerId) ?? null : null}
          onClose={() => setLayerModal(null)}
          onCreate={async (draft) => {
            const created = await handleCreateLayer(draft);
            setLayerModal(null);
            return created;
          }}
          onUpdate={async (draft) => {
            if (layerModal.layerId == null) return;
            await handleUpdateLayer(layerModal.layerId, draft);
            setLayerModal(null);
          }}
        />
      ) : null}

      {featureModal ? (
        <FeatureEditorModal
          draft={featureModal.draft}
          onClose={() => {
            if (featureModal.mode === "create" && featureModal.drawId) {
              mapRef.current?.discardDrawFeature(featureModal.drawId);
            }
            setFeatureModal(null);
          }}
          onSave={async (draft) => {
            if (featureModal.mode === "edit" && featureModal.featureId != null) {
              await handleUpdateCustomFeature(featureModal.featureId, draft.geometry, draft.properties);
              setFeatureModal(null);
              return;
            }

            const created = await handleCreateCustomFeature(draft.geometry, draft.properties);
            if (created?.id != null && featureModal.drawId) {
              mapRef.current?.attachBackendId(featureModal.drawId, created.id);
            }
            setFeatureModal(null);
          }}
          onDelete={
            featureModal.mode === "edit" && featureModal.featureId != null
              ? async () => {
                  await handleDeleteCustomFeature(featureModal.featureId as number);
                  setFeatureModal(null);
                }
              : undefined
          }
        />
      ) : null}

      <MapSaveModal
        isOpen={saveModal.isOpen}
        nameDraft={saveModal.nameDraft}
        busy={saveModal.busy}
        error={saveModal.error}
        onNameChange={(value) => saveModal.setNameDraft(value)}
        onCancel={saveModal.close}
        onConfirm={() => void saveModal.confirm()}
      />

      <AlertDialog open={!!pendingDeleteLayer} onOpenChange={(open) => { if (!open) setPendingDeleteLayer(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete layer</AlertDialogTitle>
            <AlertDialogDescription>
              Delete layer &ldquo;{pendingDeleteLayer?.name}&rdquo;? This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={() => void confirmDeleteLayer()}>Delete</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
