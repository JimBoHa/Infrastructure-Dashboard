export type MapSettings = {
  active_save_id: number;
  active_save_name: string;
  active_base_layer_id: number | null;
  center_lat: number;
  center_lng: number;
  zoom: number;
  bearing: number;
  pitch: number;
  updated_at: string | null;
};

export type MapSave = {
  id: number;
  name: string;
  created_at: string;
  updated_at: string;
};

export type MapLayerSourceType = "xyz" | "wms" | "arcgis" | "geojson" | "terrain";
export type MapLayerKind = "base" | "overlay";

export type MapLayer = {
  id: number;
  system_key: string | null;
  name: string;
  kind: MapLayerKind;
  source_type: MapLayerSourceType;
  config: Record<string, unknown>;
  opacity: number;
  enabled: boolean;
  z_index: number;
  created_at: string;
  updated_at: string;
};

export type MapLayerUpsertPayload = {
  name: string;
  kind: MapLayerKind;
  source_type: MapLayerSourceType;
  config: Record<string, unknown>;
  opacity?: number;
  enabled?: boolean;
  z_index?: number;
};

export type GeoJsonGeometry = {
  type: string;
  coordinates?: unknown;
  geometries?: unknown;
};

export type MapFeature = {
  id: number;
  node_id: string | null;
  sensor_id: string | null;
  geometry: GeoJsonGeometry;
  properties: Record<string, unknown>;
  created_at: string;
  updated_at: string;
};

export type MapFeatureUpsertPayload = {
  node_id?: string;
  sensor_id?: string;
  geometry: GeoJsonGeometry;
  properties?: Record<string, unknown>;
};

export type OfflineMapPack = {
  id: string;
  name: string;
  bounds: Record<string, unknown>;
  min_zoom: number;
  max_zoom: number;
  status: string;
  progress: Record<string, unknown>;
  error: string | null;
  updated_at: string;
};
