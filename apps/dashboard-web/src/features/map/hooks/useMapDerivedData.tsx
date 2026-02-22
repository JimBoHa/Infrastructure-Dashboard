"use client";

import { useMemo } from "react";

import {
  useMapFeaturesQuery,
  useMapLayersQuery,
  useMapOfflinePacksQuery,
  useMapSavesQuery,
  useMapSettingsQuery,
  useNodesQuery,
  useSensorsQuery,
} from "@/lib/queries";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { MapFeature, MapLayer, MapSave, MapSettings, OfflineMapPack } from "@/types/map";
import { pointFromGeometry } from "@/features/map/utils/geometry";

export type MapFeatureCollection = {
  type: "FeatureCollection";
  features: Array<{
    type: "Feature";
    id: string;
    properties: Record<string, unknown>;
    geometry: MapFeature["geometry"];
  }>;
};

export type MapDerivedData = {
  baseLayers: MapLayer[];
  overlayLayers: MapLayer[];
  offlinePacksById: Map<string, OfflineMapPack>;
  baseLayerBySystemKey: Map<string, MapLayer>;
  nodesById: Map<string, DemoNode>;
  sensorsById: Map<string, DemoSensor>;
  sensorsByNode: Map<string, DemoSensor[]>;
  featuresByNode: Map<string, MapFeature>;
  featuresBySensor: Map<string, MapFeature>;
  customFeatures: MapFeature[];
  customFeaturesById: Map<number, MapFeature>;
  nodeFeatureCollection: MapFeatureCollection;
  sensorFeatureCollection: MapFeatureCollection;
  customFeatureCollection: MapFeatureCollection;
};

export type MapDerivedDataInput = {
  nodes?: DemoNode[];
  sensors?: DemoSensor[];
  layers?: MapLayer[];
  features?: MapFeature[];
  offlinePacks?: OfflineMapPack[];
};

export type MapContextData = MapDerivedData & {
  nodes: DemoNode[];
  sensors: DemoSensor[];
  saves: MapSave[];
  settings: MapSettings | undefined;
  layers: MapLayer[];
  features: MapFeature[];
  offlinePacks: OfflineMapPack[];
  mapReady: boolean;
  mapLoading: boolean;
  mapError: unknown;
};

export function useMapDerivedData({
  nodes = [],
  sensors = [],
  layers = [],
  features = [],
  offlinePacks = [],
}: MapDerivedDataInput): MapDerivedData {
  const baseLayers = useMemo(() => layers.filter((layer) => layer.kind === "base"), [layers]);
  const overlayLayers = useMemo(() => layers.filter((layer) => layer.kind === "overlay"), [layers]);

  const offlinePacksById = useMemo(() => {
    const map = new Map<string, OfflineMapPack>();
    for (const pack of offlinePacks) map.set(pack.id, pack);
    return map;
  }, [offlinePacks]);

  const baseLayerBySystemKey = useMemo(() => {
    const map = new Map<string, MapLayer>();
    for (const layer of baseLayers) {
      if (layer.system_key) map.set(layer.system_key, layer);
    }
    return map;
  }, [baseLayers]);

  const nodesById = useMemo(() => new Map(nodes.map((node) => [node.id, node])), [nodes]);
  const sensorsById = useMemo(
    () => new Map(sensors.map((sensor) => [sensor.sensor_id, sensor])),
    [sensors],
  );

  const sensorsByNode = useMemo(() => {
    const map = new Map<string, DemoSensor[]>();
    for (const sensor of sensors) {
      const nodeId = sensor.node_id;
      if (!nodeId) continue;
      const list = map.get(nodeId) ?? [];
      list.push(sensor);
      map.set(nodeId, list);
    }
    return map;
  }, [sensors]);

  const featuresByNode = useMemo(() => {
    const map = new Map<string, MapFeature>();
    for (const feature of features) {
      if (feature.node_id) map.set(feature.node_id, feature);
    }
    return map;
  }, [features]);

  const featuresBySensor = useMemo(() => {
    const map = new Map<string, MapFeature>();
    for (const feature of features) {
      if (feature.sensor_id) map.set(feature.sensor_id, feature);
    }
    return map;
  }, [features]);

  const customFeatures = useMemo(
    () => features.filter((feature) => !feature.node_id && !feature.sensor_id),
    [features],
  );

  const customFeaturesById = useMemo(() => {
    const map = new Map<number, MapFeature>();
    for (const feature of customFeatures) map.set(feature.id, feature);
    return map;
  }, [customFeatures]);

  const nodeFeatureCollection = useMemo<MapFeatureCollection>(() => {
    const placedNodeFeatures = features.filter((feature) =>
      Boolean(feature.node_id && pointFromGeometry(feature.geometry)),
    );
    return {
      type: "FeatureCollection",
      features: placedNodeFeatures.map((feature) => {
        const nodeId = String(feature.node_id);
        const node = nodesById.get(nodeId);
        return {
          type: "Feature",
          id: nodeId,
          properties: {
            kind: "node",
            node_id: nodeId,
            name: node?.name ?? (feature.properties?.name as string | undefined) ?? "Node",
            status: node?.status ?? "unknown",
            last_seen: node?.last_seen ?? null,
          },
          geometry: feature.geometry,
        };
      }),
    };
  }, [features, nodesById]);

  const sensorFeatureCollection = useMemo<MapFeatureCollection>(() => {
    const sensorOverrides = features.filter((feature) =>
      Boolean(feature.sensor_id && pointFromGeometry(feature.geometry)),
    );
    return {
      type: "FeatureCollection",
      features: sensorOverrides.map((feature) => {
        const sensorId = String(feature.sensor_id);
        const sensor = sensorsById.get(sensorId);
        return {
          type: "Feature",
          id: sensorId,
          properties: {
            kind: "sensor",
            sensor_id: sensorId,
            node_id: sensor?.node_id ?? (feature.properties?.node_id as string | undefined) ?? null,
            name: sensor?.name ?? (feature.properties?.name as string | undefined) ?? "Sensor",
          },
          geometry: feature.geometry,
        };
      }),
    };
  }, [features, sensorsById]);

  const customFeatureCollection = useMemo<MapFeatureCollection>(() => {
    return {
      type: "FeatureCollection",
      features: customFeatures.map((feature) => ({
        type: "Feature",
        id: String(feature.id),
        properties: {
          ...(feature.properties ?? {}),
          backend_id: feature.id,
        },
        geometry: feature.geometry,
      })),
    };
  }, [customFeatures]);

  return {
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
  };
}

export function useMapContext(): MapContextData {
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const savesQuery = useMapSavesQuery();
  const settingsQuery = useMapSettingsQuery();
  const layersQuery = useMapLayersQuery();
  const featuresQuery = useMapFeaturesQuery();
  const offlinePacksQuery = useMapOfflinePacksQuery();

  const nodes = nodesQuery.data ?? [];
  const sensors = sensorsQuery.data ?? [];
  const saves = savesQuery.data ?? [];
  const settings = settingsQuery.data;
  const layers = layersQuery.data ?? [];
  const features = featuresQuery.data ?? [];
  const offlinePacks = offlinePacksQuery.data ?? [];

  const derived = useMapDerivedData({
    nodes,
    sensors,
    layers,
    features,
    offlinePacks,
  });

  const mapReady =
    nodesQuery.isSuccess &&
    sensorsQuery.isSuccess &&
    savesQuery.isSuccess &&
    settingsQuery.isSuccess &&
    layersQuery.isSuccess &&
    featuresQuery.isSuccess &&
    offlinePacksQuery.isSuccess;

  const mapLoading =
    nodesQuery.isLoading ||
    sensorsQuery.isLoading ||
    savesQuery.isLoading ||
    settingsQuery.isLoading ||
    layersQuery.isLoading ||
    featuresQuery.isLoading ||
    offlinePacksQuery.isLoading;

  const mapError =
    nodesQuery.error ||
    sensorsQuery.error ||
    savesQuery.error ||
    settingsQuery.error ||
    layersQuery.error ||
    featuresQuery.error ||
    offlinePacksQuery.error;

  return {
    nodes,
    sensors,
    saves,
    settings,
    layers,
    features,
    offlinePacks,
    mapReady,
    mapLoading,
    mapError,
    ...derived,
  };
}
