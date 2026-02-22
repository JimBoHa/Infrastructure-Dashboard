"use client";

/* eslint-disable @typescript-eslint/no-explicit-any */

import maplibregl, { type LngLatLike, type Map as MapLibreMap, type StyleSpecification } from "maplibre-gl";
import MapboxDraw from "@mapbox/mapbox-gl-draw";
import React, { forwardRef, useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import InlineBanner from "@/components/InlineBanner";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import type { MapFeature, MapLayer, MapSettings } from "@/types/map";
import type { FeatureDraft } from "@/features/map/FeatureEditorModal";
import { featureDraftFromMapFeature } from "@/features/map/FeatureEditorModal";
import type { MapFeatureCollection } from "@/features/map/hooks/useMapDerivedData";
import { pointFromGeometry } from "@/features/map/utils/geometry";

export type MapClickLngLat = { lng: number; lat: number };

export type MapCanvasHandle = {
  zoomToFeet: (feet: number) => void;
  getView: () =>
    | {
        center_lat: number;
        center_lng: number;
        zoom: number;
        bearing: number;
        pitch: number;
      }
    | null;
  focusGeometry: (geometry: unknown) => void;
  attachBackendId: (drawId: string, backendId: number) => void;
  discardDrawFeature: (drawId: string) => void;
  startDraw: (mode: "point" | "line" | "polygon") => void;
  stopDraw: () => void;
};

export type OpenFeatureDraftPayload = {
  mode: "create" | "edit";
  draft: FeatureDraft;
  drawId?: string;
  featureId?: number;
};

const MAPLIBRE_CSP_WORKER_URL = "/vendor/maplibre-gl-csp-worker.js";

if (typeof window !== "undefined") {
  const anyMaplibre = maplibregl as any;
  if (typeof anyMaplibre.workerUrl !== "string" || anyMaplibre.workerUrl.length === 0) {
    anyMaplibre.workerUrl = MAPLIBRE_CSP_WORKER_URL;
  }

  // Hardening: In rare cases during route transitions/unmount, MapLibre can be asked for layers/sources
  // after `map.remove()` (at which point `map.style` may be unset). In Safari/WebKit this can surface as
  // `TypeError: undefined is not an object (evaluating 'this.style.getLayer')` and crash the Next.js app.
  // Make these accessors safe by returning `undefined` / no-op when `style` is missing.
  const mapProto = (anyMaplibre.Map as any)?.prototype;
  if (mapProto && !mapProto.__fd_safeStyleAccessors) {
    mapProto.__fd_safeStyleAccessors = true;

    const wrapGet = (fnName: string) => {
      const original = mapProto[fnName];
      if (typeof original !== "function") return;
      mapProto[fnName] = function (...args: any[]) {
        if (!(this as any)?.style) return undefined;
        try {
          return original.apply(this, args);
        } catch {
          return undefined;
        }
      };
    };

    const wrapVoid = (fnName: string) => {
      const original = mapProto[fnName];
      if (typeof original !== "function") return;
      mapProto[fnName] = function (...args: any[]) {
        if (!(this as any)?.style) return;
        try {
          return original.apply(this, args);
        } catch {
          return;
        }
      };
    };

    wrapGet("getLayer");
    wrapGet("getSource");
    wrapVoid("removeLayer");
    wrapVoid("removeSource");
  }
}

type MapCanvasProps = {
  canEdit: boolean;
  settings: MapSettings;
  baseLayer: MapLayer | null;
  overlayLayers: MapLayer[];
  nodeFeatureCollection: MapFeatureCollection;
  sensorFeatureCollection: MapFeatureCollection;
  customFeatureCollection: MapFeatureCollection;
  customFeaturesById: Map<number, MapFeature>;
  placementActive: boolean;
  onMapClick: (pos: MapClickLngLat) => void;
  onUpsertEntityLocation: (payload: { nodeId?: string; sensorId?: string; lng: number; lat: number }) => Promise<void>;
  onUpdateCustomFeature: (featureId: number, geometry: unknown, properties: Record<string, unknown>) => Promise<void>;
  onDeleteCustomFeature: (featureId: number) => Promise<void>;
  onOpenFeatureDraft: (payload: OpenFeatureDraftPayload) => void;
};

const DEFAULT_STYLE: StyleSpecification = {
  version: 8,
  glyphs: "/api/map/glyphs/{fontstack}/{range}",
  sources: {},
  layers: [
    {
      id: "background",
      type: "background",
      paint: { "background-color": "#0b1020" },
    },
    {
      id: "fd-basemap-anchor",
      type: "background",
      paint: { "background-opacity": 0 },
    },
  ],
};

const NODE_SOURCE_ID = "fd-nodes";
const NODE_LAYER_ID = "fd-nodes-circle";
const NODE_LABEL_LAYER_ID = "fd-nodes-label";
const SENSOR_SOURCE_ID = "fd-sensors";
const SENSOR_LAYER_ID = "fd-sensors-circle";
const SENSOR_LABEL_LAYER_ID = "fd-sensors-label";

const mercatorAltitudeFeet = (map: MapLibreMap): number | null => {
  const anyMap = map as any;
  if (typeof anyMap.getFreeCameraOptions === "function") {
    const camera = anyMap.getFreeCameraOptions();
    const position = camera?.position;
    if (
      position &&
      typeof position.z === "number" &&
      typeof position.meterInMercatorCoordinateUnits === "function"
    ) {
      const metersPerUnit = position.meterInMercatorCoordinateUnits();
      if (typeof metersPerUnit === "number" && metersPerUnit > 0) {
        const meters = position.z / metersPerUnit;
        return meters * 3.28084;
      }
    }
  }

  const center = map.getCenter();
  const zoom = map.getZoom();
  const metersPerPixel =
    (156543.03392 * Math.cos((center.lat * Math.PI) / 180)) / Math.pow(2, zoom);
  const height = map.getContainer().clientHeight || 0;
  if (!height) return null;
  const approxMeters = metersPerPixel * height;
  return approxMeters * 3.28084;
};

const haversineMeters = (a: { lng: number; lat: number }, b: { lng: number; lat: number }): number => {
  const toRad = (deg: number) => (deg * Math.PI) / 180;
  const R = 6371000;
  const dLat = toRad(b.lat - a.lat);
  const dLng = toRad(b.lng - a.lng);
  const lat1 = toRad(a.lat);
  const lat2 = toRad(b.lat);
  const sinLat = Math.sin(dLat / 2);
  const sinLng = Math.sin(dLng / 2);
  const h = sinLat * sinLat + Math.cos(lat1) * Math.cos(lat2) * sinLng * sinLng;
  return 2 * R * Math.asin(Math.min(1, Math.sqrt(h)));
};

const viewportHeightFeet = (map: MapLibreMap): number | null => {
  const container = map.getContainer();
  const width = container.clientWidth || 0;
  const height = container.clientHeight || 0;
  if (!width || !height) return null;
  const x = Math.round(width / 2);

  const top = map.unproject([x, 0]);
  const bottom = map.unproject([x, height]);
  const meters = haversineMeters({ lng: top.lng, lat: top.lat }, { lng: bottom.lng, lat: bottom.lat });
  if (!Number.isFinite(meters) || meters <= 0) return null;
  return meters * 3.28084;
};

const boundsFromGeometry = (geometry: unknown):
  | { minLng: number; minLat: number; maxLng: number; maxLat: number }
  | null => {
  if (!geometry || typeof geometry !== "object") return null;
  const record = geometry as Record<string, unknown>;
  const coords = record.coordinates;
  if (!coords) return null;

  let minLng = Infinity;
  let minLat = Infinity;
  let maxLng = -Infinity;
  let maxLat = -Infinity;

  const stack: unknown[] = [coords];
  while (stack.length) {
    const value = stack.pop();
    if (!Array.isArray(value)) continue;
    if (value.length >= 2 && typeof value[0] === "number" && typeof value[1] === "number") {
      const lng = value[0] as number;
      const lat = value[1] as number;
      if (lng < minLng) minLng = lng;
      if (lat < minLat) minLat = lat;
      if (lng > maxLng) maxLng = lng;
      if (lat > maxLat) maxLat = lat;
      continue;
    }
    for (const item of value) stack.push(item);
  }

  if (!Number.isFinite(minLng) || !Number.isFinite(minLat) || !Number.isFinite(maxLng) || !Number.isFinite(maxLat)) {
    return null;
  }
  return { minLng, minLat, maxLng, maxLat };
};

const buildAttribution = (config: Record<string, unknown>): string | undefined => {
  const value = typeof config.attribution === "string" ? config.attribution.trim() : "";
  return value ? value : undefined;
};

const clampRasterZoom = (value: unknown): number | null => {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  const zoom = Math.floor(value);
  if (zoom < 0 || zoom > 24) return null;
  return zoom;
};

const defaultMaxZoomForLayer = (layer: MapLayer, hintUrl: string): number => {
  const systemKey = (layer.system_key ?? "").trim().toLowerCase();
  if (systemKey === "topo") return 16;
  if (systemKey === "streets") return 19;
  if (systemKey === "satellite") return 23;

  const lowerUrl = hintUrl.toLowerCase();
  if (lowerUrl.includes("basemap.nationalmap.gov") || lowerUrl.includes("usgstopo")) return 16;
  if (lowerUrl.includes("openstreetmap.org")) return 19;
  if (lowerUrl.includes("arcgisonline.com") || lowerUrl.includes("world_imagery")) return 23;

  return 19;
};

type RasterSourceTemplate = {
  tiles: string[];
  tileSize: number;
  attribution?: string;
  maxZoom?: number;
};

const buildTileUrlTemplate = (
  layer: MapLayer,
): RasterSourceTemplate | null => {
  const config = (layer.config ?? {}) as Record<string, unknown>;
  const attribution = buildAttribution(config);
  const maxZoomConfig = clampRasterZoom(config.max_zoom);

  if (layer.source_type === "xyz") {
    const url = typeof config.url_template === "string" ? config.url_template.trim() : "";
    if (!url) return null;
    const tileSize = typeof config.tile_size === "number" ? Math.floor(config.tile_size) : 256;
    return {
      tiles: [url],
      tileSize,
      attribution,
      maxZoom: maxZoomConfig ?? defaultMaxZoomForLayer(layer, url),
    };
  }

  if (layer.source_type === "arcgis") {
    const baseUrl = typeof config.base_url === "string" ? config.base_url.trim() : "";
    if (!baseUrl) return null;
    const mode = typeof config.mode === "string" ? config.mode.trim().toLowerCase() : "tile";
    if (mode === "export") {
      const format = typeof config.format === "string" ? config.format.trim() : "png32";
      const transparent = config.transparent === false ? "false" : "true";
      const glue = baseUrl.endsWith("/") ? "" : "/";
      const url =
        `${baseUrl}${glue}export` +
        `?bbox={bbox-epsg-3857}&bboxSR=3857&imageSR=3857&size=256,256&format=${encodeURIComponent(format)}` +
        `&transparent=${transparent}&f=image`;
      return { tiles: [url], tileSize: 256, attribution };
    }
    const glue = baseUrl.endsWith("/") ? "" : "/";
    const url = `${baseUrl}${glue}tile/{z}/{y}/{x}`;
    return {
      tiles: [url],
      tileSize: 256,
      attribution,
      maxZoom: maxZoomConfig ?? defaultMaxZoomForLayer(layer, baseUrl),
    };
  }

  if (layer.source_type === "wms") {
    const baseUrl = typeof config.base_url === "string" ? config.base_url.trim() : "";
    const layers = typeof config.layers === "string" ? config.layers.trim() : "";
    if (!baseUrl || !layers) return null;

    const version = typeof config.version === "string" ? config.version.trim() : "1.3.0";
    const styles = typeof config.styles === "string" ? config.styles.trim() : "";
    const format = typeof config.format === "string" ? config.format.trim() : "image/png";
    const transparent = config.transparent === false ? "false" : "true";
    const crsKey = version.startsWith("1.3") ? "crs" : "srs";

    const join = baseUrl.includes("?") ? "&" : "?";
    const url =
      `${baseUrl}${join}service=WMS&request=GetMap` +
      `&version=${encodeURIComponent(version)}` +
      `&layers=${encodeURIComponent(layers)}` +
      `&styles=${encodeURIComponent(styles)}` +
      `&format=${encodeURIComponent(format)}` +
      `&transparent=${transparent}` +
      `&width=256&height=256` +
      `&${crsKey}=EPSG:3857` +
      `&bbox={bbox-epsg-3857}`;

    return { tiles: [url], tileSize: 256, attribution, maxZoom: maxZoomConfig ?? undefined };
  }

  return null;
};

const DRAW_STYLES = [
  // polygon fill
  {
    id: "fd-draw-polygon-fill-inactive",
    type: "fill",
    filter: ["all", ["==", "active", "false"], ["==", "$type", "Polygon"]],
    paint: {
      "fill-color": ["coalesce", ["get", "color"], "#22c55e"],
      "fill-opacity": 0.2,
    },
  },
  {
    id: "fd-draw-polygon-fill-active",
    type: "fill",
    filter: ["all", ["==", "active", "true"], ["==", "$type", "Polygon"]],
    paint: {
      "fill-color": ["coalesce", ["get", "color"], "#22c55e"],
      "fill-opacity": 0.35,
    },
  },
  // polygon outline
  {
    id: "fd-draw-polygon-stroke-inactive",
    type: "line",
    filter: ["all", ["==", "active", "false"], ["==", "$type", "Polygon"]],
    layout: { "line-cap": "round", "line-join": "round" },
    paint: {
      "line-color": ["coalesce", ["get", "color"], "#22c55e"],
      "line-width": 2,
    },
  },
  {
    id: "fd-draw-polygon-stroke-active",
    type: "line",
    filter: ["all", ["==", "active", "true"], ["==", "$type", "Polygon"]],
    layout: { "line-cap": "round", "line-join": "round" },
    paint: {
      "line-color": ["coalesce", ["get", "color"], "#22c55e"],
      "line-width": 3,
    },
  },
  // lines
  {
    id: "fd-draw-line-inactive",
    type: "line",
    filter: ["all", ["==", "active", "false"], ["==", "$type", "LineString"]],
    layout: { "line-cap": "round", "line-join": "round" },
    paint: {
      "line-color": ["coalesce", ["get", "color"], "#3b82f6"],
      "line-width": 3,
    },
  },
  {
    id: "fd-draw-line-active",
    type: "line",
    filter: ["all", ["==", "active", "true"], ["==", "$type", "LineString"]],
    layout: { "line-cap": "round", "line-join": "round" },
    paint: {
      "line-color": ["coalesce", ["get", "color"], "#3b82f6"],
      "line-width": 4,
    },
  },
  // points
  {
    id: "fd-draw-point-inactive",
    type: "circle",
    filter: ["all", ["==", "active", "false"], ["==", "$type", "Point"]],
    paint: {
      "circle-radius": 6,
      "circle-color": ["coalesce", ["get", "color"], "#f97316"],
      "circle-stroke-color": "#111827",
      "circle-stroke-width": 1,
    },
  },
  {
    id: "fd-draw-point-active",
    type: "circle",
    filter: ["all", ["==", "active", "true"], ["==", "$type", "Point"]],
    paint: {
      "circle-radius": 7,
      "circle-color": ["coalesce", ["get", "color"], "#f97316"],
      "circle-stroke-color": "#111827",
      "circle-stroke-width": 2,
    },
  },
  // vertices
  {
    id: "fd-draw-vertex-halo",
    type: "circle",
    filter: ["all", ["==", "meta", "vertex"], ["==", "$type", "Point"]],
    paint: {
      "circle-radius": 6,
      "circle-color": "#ffffff",
    },
  },
  {
    id: "fd-draw-vertex",
    type: "circle",
    filter: ["all", ["==", "meta", "vertex"], ["==", "$type", "Point"]],
    paint: {
      "circle-radius": 4,
      "circle-color": "#111827",
    },
  },
  // labels
  {
    id: "fd-draw-label-point",
    type: "symbol",
    minzoom: 15,
    filter: ["all", ["==", "meta", "feature"], ["==", "$type", "Point"]],
    layout: {
      "text-field": ["coalesce", ["get", "name"], ["get", "label"], ""],
      "text-font": ["Noto Sans Regular"],
      "text-size": 12,
      "text-anchor": "top",
      "text-offset": [0, 1.15],
      "text-optional": true,
    },
    paint: {
      "text-color": ["coalesce", ["get", "color"], "#111827"],
      "text-halo-color": "rgba(255,255,255,0.92)",
      "text-halo-width": 1.6,
    },
  },
  {
    id: "fd-draw-label-line",
    type: "symbol",
    minzoom: 14,
    filter: ["all", ["==", "meta", "feature"], ["==", "$type", "LineString"]],
    layout: {
      "symbol-placement": "line",
      "text-field": ["coalesce", ["get", "name"], ["get", "label"], ""],
      "text-font": ["Noto Sans Regular"],
      "text-size": 12,
      "text-optional": true,
      "text-keep-upright": true,
    },
    paint: {
      "text-color": ["coalesce", ["get", "color"], "#111827"],
      "text-halo-color": "rgba(255,255,255,0.92)",
      "text-halo-width": 1.6,
    },
  },
  {
    id: "fd-draw-label-polygon",
    type: "symbol",
    minzoom: 13,
    filter: ["all", ["==", "meta", "feature"], ["==", "$type", "Polygon"]],
    layout: {
      "symbol-placement": "point",
      "text-field": ["coalesce", ["get", "name"], ["get", "label"], ""],
      "text-font": ["Noto Sans Regular"],
      "text-size": 12,
      "text-optional": true,
    },
    paint: {
      "text-color": ["coalesce", ["get", "color"], "#111827"],
      "text-halo-color": "rgba(255,255,255,0.92)",
      "text-halo-width": 1.6,
    },
  },
] as any[];

export const MapCanvas = forwardRef<MapCanvasHandle, MapCanvasProps>(function MapCanvas(
  {
    canEdit,
    settings,
    baseLayer,
    overlayLayers,
    nodeFeatureCollection,
    sensorFeatureCollection,
    customFeatureCollection,
    customFeaturesById,
    placementActive,
    onMapClick,
    onUpsertEntityLocation,
    onUpdateCustomFeature,
    onDeleteCustomFeature,
    onOpenFeatureDraft,
  },
  ref,
) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const mapRef = useRef<MapLibreMap | null>(null);
  const drawRef = useRef<any>(null);
  const mapMountedRef = useRef(false);
  const drawHandlersRef = useRef<
    | {
        onCreate: (event: any) => void;
        onUpdate: (event: any) => void;
        onDelete: (event: any) => void;
        onModeChange: (event: any) => void;
        onClick: (event: any) => void;
      }
    | null
  >(null);
  const syncingDrawRef = useRef(false);
  const managedLayers = useRef<{ layerIds: string[]; sourceIds: string[] }>({ layerIds: [], sourceIds: [] });
  const nodeGeojsonRef = useRef<any>(null);
  const sensorGeojsonRef = useRef<any>(null);
  const mapInteractionsReadyRef = useRef(false);
  const dragRef = useRef<{ kind: "node" | "sensor"; id: string } | null>(null);
  const zoomAdjustTimerRef = useRef<number | null>(null);
  const canEditRef = useRef(canEdit);
  const onUpsertEntityLocationRef = useRef(onUpsertEntityLocation);
  const onMapClickRef = useRef(onMapClick);
  const placementActiveRef = useRef(placementActive);

  useEffect(() => {
    canEditRef.current = canEdit;
  }, [canEdit]);

  useEffect(() => {
    onMapClickRef.current = onMapClick;
  }, [onMapClick]);

  useEffect(() => {
    onUpsertEntityLocationRef.current = onUpsertEntityLocation;
  }, [onUpsertEntityLocation]);

  useEffect(() => {
    placementActiveRef.current = placementActive;
  }, [placementActive]);

  const [loaded, setLoaded] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [altitudeFt, setAltitudeFt] = useState<number | null>(null);
  const [viewportFt, setViewportFt] = useState<number | null>(null);
  const [zoomLevel, setZoomLevel] = useState<number>(settings.zoom);
  const [drawHealthy, setDrawHealthy] = useState(false);

  const pendingUpdateRef = useRef<
    Map<
      number,
      {
        geometry: unknown;
        properties: Record<string, unknown>;
      }
    >
  >(new Map());
  const pendingUpdateTimerRef = useRef<number | null>(null);

  const flushPendingUpdates = useCallback(async () => {
    if (pendingUpdateTimerRef.current != null) {
      window.clearTimeout(pendingUpdateTimerRef.current);
      pendingUpdateTimerRef.current = null;
    }

    const pending = Array.from(pendingUpdateRef.current.entries());
    pendingUpdateRef.current.clear();
    for (const [backendId, payload] of pending) {
      try {
        await onUpdateCustomFeature(backendId, payload.geometry, payload.properties);
      } catch (err) {
        console.error("Failed to persist map feature update", backendId, err);
      }
    }
  }, [onUpdateCustomFeature]);

  const schedulePersistUpdates = useCallback(() => {
    if (pendingUpdateTimerRef.current != null) return;
    pendingUpdateTimerRef.current = window.setTimeout(() => {
      void flushPendingUpdates();
    }, 750);
  }, [flushPendingUpdates]);

  useEffect(() => {
    const timerRef = pendingUpdateTimerRef;
    const updateRef = pendingUpdateRef;
    return () => {
      if (timerRef.current != null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      updateRef.current.clear();
    };
  }, []);

  const updateReadouts = useCallback(() => {
    const map = mapRef.current;
    const anyMap = map as any;
    if (!map || anyMap?._removed || !mapMountedRef.current) return;
    setZoomLevel(map.getZoom());
    setAltitudeFt(mercatorAltitudeFeet(map));
    setViewportFt(viewportHeightFeet(map));
  }, []);

  useEffect(() => {
    if (!containerRef.current) return;
    if (mapRef.current) return;

    mapMountedRef.current = true;
    const map = new maplibregl.Map({
      container: containerRef.current,
      style: DEFAULT_STYLE,
      center: [settings.center_lng, settings.center_lat] as LngLatLike,
      zoom: settings.zoom,
      bearing: settings.bearing,
      pitch: settings.pitch,
      maxZoom: 24,
      attributionControl: false,
    });

    // Hardening: MapLibre + MapboxDraw can sometimes schedule work that outlives component teardown.
    // In those cases, calling `map.getLayer()` after `map.remove()` can throw because `map.style` is unset.
    // Wrap the most common style-accessors to return undefined/no-op once unmounted.
    {
      const anyMap = map as any;
      if (!anyMap._fdStyleAccessorsPatched) {
        anyMap._fdStyleAccessorsPatched = true;
        const originalGetLayer = map.getLayer.bind(map);
        const originalGetSource = map.getSource.bind(map);
        const originalRemoveLayer = map.removeLayer.bind(map);
        const originalRemoveSource = map.removeSource.bind(map);

        (map as any).getLayer = (id: string) => {
          const currentAnyMap = map as any;
          if (!mapMountedRef.current || currentAnyMap?._removed || !currentAnyMap?.style) return undefined;
          try {
            return originalGetLayer(id);
          } catch {
            return undefined;
          }
        };

        (map as any).getSource = (id: string) => {
          const currentAnyMap = map as any;
          if (!mapMountedRef.current || currentAnyMap?._removed || !currentAnyMap?.style) return undefined;
          try {
            return originalGetSource(id);
          } catch {
            return undefined;
          }
        };

        (map as any).removeLayer = (id: string) => {
          const currentAnyMap = map as any;
          if (!mapMountedRef.current || currentAnyMap?._removed || !currentAnyMap?.style) return;
          try {
            originalRemoveLayer(id);
          } catch {
            // ignore
          }
        };

        (map as any).removeSource = (id: string) => {
          const currentAnyMap = map as any;
          if (!mapMountedRef.current || currentAnyMap?._removed || !currentAnyMap?.style) return;
          try {
            originalRemoveSource(id);
          } catch {
            // ignore
          }
        };
      }
    }
    mapRef.current = map;

    map.addControl(new maplibregl.NavigationControl({ visualizePitch: true }), "top-right");
    map.addControl(new maplibregl.ScaleControl({ maxWidth: 140, unit: "imperial" }), "bottom-left");
    map.addControl(new maplibregl.AttributionControl({ compact: true }), "bottom-right");

    const onLoad = () => {
      if (!mapMountedRef.current) return;
      setLoaded(true);
      updateReadouts();
    };

    const onError = (event: any) => {
      if (!mapMountedRef.current) return;
      const error = (event as any)?.error;
      const message =
        error instanceof Error
          ? error.message
          : typeof error?.message === "string"
            ? error.message
            : null;
      if (!message) return;
      const lower = message.toLowerCase();
      if (
        !lower.includes("worker") &&
        !lower.includes("csp") &&
        !lower.includes("maplibre") &&
        !lower.includes("web worker")
      ) {
        return;
      }
      setLoadError((prev) => prev ?? message);
    };

    const onClick = (event: any) => {
      if (!mapMountedRef.current) return;
      if (!event?.lngLat) return;
      const draw = drawRef.current;
      const mode = draw?.getMode?.();
      if (placementActiveRef.current) {
        if (draw && typeof draw.changeMode === "function" && typeof mode === "string" && mode !== "simple_select") {
          try {
            draw.changeMode("simple_select");
          } catch {
            // ignore
          }
        }
        onMapClickRef.current({ lng: event.lngLat.lng, lat: event.lngLat.lat });
        return;
      }
      if (typeof mode === "string" && mode !== "simple_select" && mode !== "direct_select") return;
      onMapClickRef.current({ lng: event.lngLat.lng, lat: event.lngLat.lat });
    };

    map.on("load", onLoad);
    map.on("error", onError);
    map.on("move", updateReadouts);
    map.on("resize", updateReadouts);
    map.on("click", onClick);

    return () => {
      mapMountedRef.current = false;
      if (zoomAdjustTimerRef.current != null) {
        window.clearTimeout(zoomAdjustTimerRef.current);
        zoomAdjustTimerRef.current = null;
      }

      try {
        map.off("load", onLoad);
        map.off("error", onError);
        map.off("move", updateReadouts);
        map.off("resize", updateReadouts);
        map.off("click", onClick);

        const drawHandlers = drawHandlersRef.current;
        if (drawHandlers) {
          map.off("draw.create", drawHandlers.onCreate);
          map.off("draw.update", drawHandlers.onUpdate);
          map.off("draw.delete", drawHandlers.onDelete);
          map.off("draw.modechange", drawHandlers.onModeChange);
          map.off("click", drawHandlers.onClick);
          drawHandlersRef.current = null;
        }

        const draw = drawRef.current;
        if (draw) {
          try {
            map.removeControl(draw);
          } catch {
            // ignore
          }
          drawRef.current = null;
        }
      } catch {
        // ignore teardown errors to avoid breaking navigation
      }

      try {
        map.remove();
      } catch {
        // ignore
      }
      mapRef.current = null;
      nodeGeojsonRef.current = null;
      sensorGeojsonRef.current = null;
      mapInteractionsReadyRef.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (loaded || loadError) return;
    const controller = new AbortController();
    const timer = window.setTimeout(async () => {
      try {
        const res = await fetch(MAPLIBRE_CSP_WORKER_URL, { method: "HEAD", signal: controller.signal });
        if (!res.ok) {
          if (!mapMountedRef.current) return;
          setLoadError(
            `Map failed to load (${res.status}). Missing MapLibre worker at ${MAPLIBRE_CSP_WORKER_URL}.`,
          );
          return;
        }
      } catch {
        // Ignore fetch errors; MapLibre may still load.
      }
      if (!mapMountedRef.current) return;
      setLoadError(
        "Map is taking longer than expected to load. Try refreshing; if it persists, check browser console for MapLibre worker/style errors.",
      );
    }, 10_000);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [loaded, loadError]);

  useEffect(() => {
    const map = mapRef.current;
    const anyMap = map as any;
    if (!map || !loaded || anyMap?._removed || !mapMountedRef.current) return;
    try {
      map.jumpTo({
        center: [settings.center_lng, settings.center_lat] as LngLatLike,
        zoom: settings.zoom,
        bearing: settings.bearing,
        pitch: settings.pitch,
      });
    } catch {
      // ignore
    }
  }, [
    loaded,
    settings.active_save_id,
    settings.center_lat,
    settings.center_lng,
    settings.zoom,
    settings.bearing,
    settings.pitch,
  ]);

  const syncBaseAndOverlays = useCallback(() => {
    const map = mapRef.current;
    const anyMap = map as any;
    if (!map || !loaded || anyMap?._removed || !mapMountedRef.current) return;

    const removeLayerSafe = (id: string) => {
      try {
        if (map.getLayer(id)) map.removeLayer(id);
      } catch {
        // ignore
      }
    };
    const removeSourceSafe = (id: string) => {
      try {
        if (map.getSource(id)) map.removeSource(id);
      } catch {
        // ignore
      }
    };

    // Clear previous basemap/overlay layers (only the layers we manage).
    for (const id of managedLayers.current.layerIds) removeLayerSafe(id);
    for (const id of managedLayers.current.sourceIds) removeSourceSafe(id);
    managedLayers.current = { layerIds: [], sourceIds: [] };

    let beforeId: string | undefined;
    try {
      beforeId = map.getLayer("fd-basemap-anchor") ? "fd-basemap-anchor" : undefined;
    } catch {
      beforeId = undefined;
    }

    const activeBase = baseLayer && baseLayer.enabled !== false ? baseLayer : null;
    if (activeBase) {
      const raster = buildTileUrlTemplate(activeBase);
      if (raster) {
        const source: any = {
          type: "raster",
          tiles: raster.tiles,
          tileSize: raster.tileSize,
          attribution: raster.attribution,
        };
        if (typeof raster.maxZoom === "number") source.maxzoom = raster.maxZoom;
        map.addSource("fd-basemap", source);
        managedLayers.current.sourceIds.push("fd-basemap");
        map.addLayer(
          {
            id: "fd-basemap",
            type: "raster",
            source: "fd-basemap",
            paint: { "raster-opacity": 1.0 },
          } as any,
          beforeId,
        );
        managedLayers.current.layerIds.push("fd-basemap");
      }
    }

    for (const layer of overlayLayers
      .filter((l) => l.enabled)
      .slice()
      .sort((a, b) => a.z_index - b.z_index || a.id - b.id)) {
      const prefix = `fd-overlay-${layer.id}`;
      const opacity = typeof layer.opacity === "number" ? layer.opacity : 1.0;

      if (layer.source_type === "geojson") {
        const config = (layer.config ?? {}) as Record<string, unknown>;
        const data = config.data as any;
        if (!data || typeof data !== "object") continue;
        map.addSource(prefix, { type: "geojson", data } as any);
        managedLayers.current.sourceIds.push(prefix);
        map.addLayer(
          {
            id: `${prefix}-fill`,
            type: "fill",
            source: prefix,
            filter: ["==", "$type", "Polygon"],
            paint: { "fill-color": "#60a5fa", "fill-opacity": 0.15 * opacity },
          } as any,
          beforeId,
        );
        managedLayers.current.layerIds.push(`${prefix}-fill`);
        map.addLayer(
          {
            id: `${prefix}-line`,
            type: "line",
            source: prefix,
            filter: ["==", "$type", "LineString"],
            paint: { "line-color": "#60a5fa", "line-opacity": opacity, "line-width": 2 },
          } as any,
          beforeId,
        );
        managedLayers.current.layerIds.push(`${prefix}-line`);
        map.addLayer(
          {
            id: `${prefix}-circle`,
            type: "circle",
            source: prefix,
            filter: ["==", "$type", "Point"],
            paint: {
              "circle-color": "#60a5fa",
              "circle-opacity": opacity,
              "circle-radius": 4,
              "circle-stroke-color": "#111827",
              "circle-stroke-width": 1,
            },
          } as any,
          beforeId,
        );
        managedLayers.current.layerIds.push(`${prefix}-circle`);
        continue;
      }

      if (layer.source_type === "terrain") {
        const config = (layer.config ?? {}) as Record<string, unknown>;
        const url = typeof config.url_template === "string" ? config.url_template.trim() : "";
        if (!url) continue;

        const tileSize = typeof config.tile_size === "number" ? Math.floor(config.tile_size) : 256;
        const maxZoomConfig = clampRasterZoom(config.max_zoom);
        const attribution = buildAttribution(config);
        const encodingRaw = typeof config.encoding === "string" ? config.encoding.trim().toLowerCase() : "terrarium";
        const encoding = encodingRaw === "mapbox" ? "mapbox" : "terrarium";

        map.addSource(prefix, {
          type: "raster-dem",
          tiles: [url],
          tileSize,
          attribution,
          maxzoom: maxZoomConfig ?? 13,
          encoding,
        } as any);
        managedLayers.current.sourceIds.push(prefix);
        map.addLayer(
          {
            id: `${prefix}-hillshade`,
            type: "hillshade",
            source: prefix,
            paint: {
              "hillshade-exaggeration": 0.35 * opacity,
              "hillshade-highlight-color": "rgba(255,255,255,0.55)",
              "hillshade-shadow-color": "rgba(17,24,39,0.55)",
              "hillshade-accent-color": "rgba(37,99,235,0.35)",
            },
          } as any,
          beforeId,
        );
        managedLayers.current.layerIds.push(`${prefix}-hillshade`);
        continue;
      }

      const raster = buildTileUrlTemplate(layer);
      if (!raster) continue;
      const source: any = {
        type: "raster",
        tiles: raster.tiles,
        tileSize: raster.tileSize,
        attribution: raster.attribution,
      };
      if (typeof raster.maxZoom === "number") source.maxzoom = raster.maxZoom;
      map.addSource(prefix, source);
      managedLayers.current.sourceIds.push(prefix);
      map.addLayer(
        {
          id: `${prefix}-raster`,
          type: "raster",
          source: prefix,
          paint: { "raster-opacity": opacity },
        } as any,
        beforeId,
      );
      managedLayers.current.layerIds.push(`${prefix}-raster`);
    }
  }, [baseLayer, loaded, overlayLayers]);

  useEffect(() => {
    syncBaseAndOverlays();
  }, [syncBaseAndOverlays]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !loaded) return;

    const sourceId = "fd-custom-static";
    const fillId = "fd-custom-static-fill";
    const lineId = "fd-custom-static-line";
    const circleId = "fd-custom-static-circle";
    const labelPointId = "fd-custom-static-label-point";
    const labelLineId = "fd-custom-static-label-line";
    const labelPolygonId = "fd-custom-static-label-polygon";

    const remove = () => {
      if (map.getLayer(labelPointId)) map.removeLayer(labelPointId);
      if (map.getLayer(labelLineId)) map.removeLayer(labelLineId);
      if (map.getLayer(labelPolygonId)) map.removeLayer(labelPolygonId);
      if (map.getLayer(fillId)) map.removeLayer(fillId);
      if (map.getLayer(lineId)) map.removeLayer(lineId);
      if (map.getLayer(circleId)) map.removeLayer(circleId);
      if (map.getSource(sourceId)) map.removeSource(sourceId);
    };

    if (canEdit && drawHealthy) {
      remove();
      return;
    }

    if (!map.getSource(sourceId)) {
      map.addSource(sourceId, { type: "geojson", data: customFeatureCollection } as any);
      map.addLayer({
        id: fillId,
        type: "fill",
        source: sourceId,
        filter: ["==", "$type", "Polygon"],
        paint: {
          "fill-color": ["coalesce", ["get", "color"], "#22c55e"],
          "fill-opacity": 0.2,
        },
      } as any);
      map.addLayer({
        id: lineId,
        type: "line",
        source: sourceId,
        filter: ["any", ["==", "$type", "LineString"], ["==", "$type", "Polygon"]],
        paint: {
          "line-color": ["coalesce", ["get", "color"], "#22c55e"],
          "line-width": 2,
          "line-opacity": 0.9,
        },
      } as any);
      map.addLayer({
        id: circleId,
        type: "circle",
        source: sourceId,
        filter: ["==", "$type", "Point"],
        paint: {
          "circle-radius": 6,
          "circle-color": ["coalesce", ["get", "color"], "#f97316"],
          "circle-stroke-color": "#111827",
          "circle-stroke-width": 1,
          "circle-opacity": 0.9,
        },
      } as any);
      map.addLayer({
        id: labelPointId,
        type: "symbol",
        source: sourceId,
        minzoom: 15,
        filter: ["==", "$type", "Point"],
        layout: {
          "text-field": ["coalesce", ["get", "name"], ["get", "label"], ""],
          "text-size": 12,
          "text-anchor": "top",
          "text-offset": [0, 1.15],
          "text-optional": true,
        },
        paint: {
          "text-color": ["coalesce", ["get", "color"], "#111827"],
          "text-halo-color": "rgba(255,255,255,0.92)",
          "text-halo-width": 1.6,
        },
      } as any);
      map.addLayer({
        id: labelLineId,
        type: "symbol",
        source: sourceId,
        minzoom: 14,
        filter: ["==", "$type", "LineString"],
        layout: {
          "symbol-placement": "line",
          "text-field": ["coalesce", ["get", "name"], ["get", "label"], ""],
          "text-size": 12,
          "text-optional": true,
          "text-keep-upright": true,
        },
        paint: {
          "text-color": ["coalesce", ["get", "color"], "#111827"],
          "text-halo-color": "rgba(255,255,255,0.92)",
          "text-halo-width": 1.6,
        },
      } as any);
      map.addLayer({
        id: labelPolygonId,
        type: "symbol",
        source: sourceId,
        minzoom: 13,
        filter: ["==", "$type", "Polygon"],
        layout: {
          "symbol-placement": "point",
          "text-field": ["coalesce", ["get", "name"], ["get", "label"], ""],
          "text-size": 12,
          "text-optional": true,
        },
        paint: {
          "text-color": ["coalesce", ["get", "color"], "#111827"],
          "text-halo-color": "rgba(255,255,255,0.92)",
          "text-halo-width": 1.6,
        },
      } as any);
      return;
    }

    const src = map.getSource(sourceId) as any;
    if (src && typeof src.setData === "function") {
      src.setData(customFeatureCollection);
    }
  }, [canEdit, customFeatureCollection, drawHealthy, loaded]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !loaded) return;

    // (Re)create the draw controller if needed.
    if (canEdit && !drawRef.current) {
      setDrawHealthy(false);
      const draw = new MapboxDraw({
        displayControlsDefault: false,
        defaultMode: "simple_select",
        styles: DRAW_STYLES,
      });
      drawRef.current = draw;
      map.addControl(draw as any, "top-left");
      setDrawHealthy(true);

      // If entity layers already exist (e.g., permission changed), keep them above the draw overlay so they remain visible.
      for (const layerId of [NODE_LAYER_ID, NODE_LABEL_LAYER_ID, SENSOR_LAYER_ID, SENSOR_LABEL_LAYER_ID]) {
        if (map.getLayer(layerId)) {
          try {
            map.moveLayer(layerId);
          } catch {
            // ignore
          }
        }
      }

      const onCreate = (event: any) => {
        if (syncingDrawRef.current) return;
        const feature = event?.features?.[0];
        if (!feature) return;
        const drawId = feature.id as string | undefined;
        if (!drawId) return;

        const geo = feature.geometry;
        const geometryType = typeof geo?.type === "string" ? geo.type : "Unknown";
        const defaults = (() => {
          if (geometryType === "Point") return { kind: "hardware", color: "#f97316" };
          if (geometryType === "LineString") return { kind: "utility", color: "#3b82f6" };
          return { kind: "field", color: "#22c55e" };
        })();

        const draft: FeatureDraft = {
          name: "Untitled",
          kind: defaults.kind,
          color: defaults.color,
          notes: "",
          geometry: geo,
          properties: { kind: defaults.kind, color: defaults.color, name: "Untitled" },
        };

        onOpenFeatureDraft({ mode: "create", draft, drawId });
        try {
          draw.changeMode("simple_select");
        } catch {
          // ignore
        }
      };

      const onUpdate = (event: any) => {
        if (syncingDrawRef.current) return;
        const draw = drawRef.current;
        const mode = draw?.getMode?.();
        if (typeof mode === "string" && mode !== "simple_select" && mode !== "direct_select") return;

        const feature = event?.features?.[0];
        if (!feature) return;
        const props = (feature.properties ?? {}) as Record<string, unknown>;
        const backendIdRaw = props.backend_id;
        const backendId = typeof backendIdRaw === "number" ? backendIdRaw : Number(backendIdRaw ?? NaN);
        if (!Number.isFinite(backendId)) return;

        pendingUpdateRef.current.set(backendId, { geometry: feature.geometry, properties: props });
        if (mode === "simple_select") schedulePersistUpdates();
      };

      const onDelete = (event: any) => {
        if (syncingDrawRef.current) return;
        const feature = event?.features?.[0];
        if (!feature) return;
        const props = (feature.properties ?? {}) as Record<string, unknown>;
        const backendIdRaw = props.backend_id;
        const backendId = typeof backendIdRaw === "number" ? backendIdRaw : Number(backendIdRaw ?? NaN);
        if (!Number.isFinite(backendId)) return;
        void onDeleteCustomFeature(backendId).catch((err) => {
          console.error("Failed to delete map feature", backendId, err);
        });
      };

      const onModeChange = (event: any) => {
        if (syncingDrawRef.current) return;
        const mode = typeof event?.mode === "string" ? event.mode : null;
        if (mode !== "simple_select") return;
        if (!pendingUpdateRef.current.size) return;
        void flushPendingUpdates();
      };

      const onDrawClick = (e: any) => {
        if (!drawRef.current) return;
        const features = map.queryRenderedFeatures(e.point);
        const drawFeature = features.find((f: any) => typeof f?.properties?.backend_id !== "undefined");
        if (!drawFeature) return;
        const props = drawFeature.properties as Record<string, unknown>;
        const backendIdRaw = props.backend_id;
        const backendId = typeof backendIdRaw === "number" ? backendIdRaw : Number(backendIdRaw ?? NaN);
        if (!Number.isFinite(backendId)) return;

        const stored = customFeaturesById.get(backendId);
        if (!stored) return;

        onOpenFeatureDraft({
          mode: "edit",
          draft: featureDraftFromMapFeature(stored),
          featureId: stored.id,
        });
      };

      map.on("draw.create", onCreate);
      map.on("draw.update", onUpdate);
      map.on("draw.delete", onDelete);
      map.on("draw.modechange", onModeChange);
      map.on("click", onDrawClick);
      drawHandlersRef.current = { onCreate, onUpdate, onDelete, onModeChange, onClick: onDrawClick };
    }

    if (!canEdit && drawRef.current) {
      try {
        const drawHandlers = drawHandlersRef.current;
        if (drawHandlers) {
          map.off("draw.create", drawHandlers.onCreate);
          map.off("draw.update", drawHandlers.onUpdate);
          map.off("draw.delete", drawHandlers.onDelete);
          map.off("draw.modechange", drawHandlers.onModeChange);
          map.off("click", drawHandlers.onClick);
          drawHandlersRef.current = null;
        }
        map.removeControl(drawRef.current);
      } catch {
        // ignore
      }
      drawRef.current = null;
      setDrawHealthy(false);
    }
  }, [
    canEdit,
    customFeaturesById,
    loaded,
    onDeleteCustomFeature,
    onOpenFeatureDraft,
    flushPendingUpdates,
    schedulePersistUpdates,
  ]);

  useEffect(() => {
    if (!canEdit) return;
    const draw = drawRef.current;
    if (!draw) return;
    syncingDrawRef.current = true;
    try {
      draw.set(customFeatureCollection);
      setDrawHealthy(true);
    } catch (err) {
      console.error("Map drawing overlay failed; falling back to static rendering", err);
      setDrawHealthy(false);
    } finally {
      syncingDrawRef.current = false;
    }
  }, [canEdit, customFeatureCollection]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !loaded) return;

    if (mapInteractionsReadyRef.current) return;

    map.addSource(NODE_SOURCE_ID, { type: "geojson", data: { type: "FeatureCollection", features: [] } } as any);
    map.addLayer({
      id: NODE_LAYER_ID,
      type: "circle",
      source: NODE_SOURCE_ID,
      paint: {
        "circle-radius": ["case", ["boolean", ["feature-state", "hover"], false], 9, 7],
        "circle-color": ["match", ["get", "status"], "online", "#10b981", "offline", "#ef4444", "#64748b"],
        "circle-stroke-color": "#111827",
        "circle-stroke-width": ["case", ["boolean", ["feature-state", "hover"], false], 2, 1],
        "circle-opacity": 0.92,
      },
    } as any);
    map.addLayer({
      id: NODE_LABEL_LAYER_ID,
      type: "symbol",
      source: NODE_SOURCE_ID,
      minzoom: 12,
      layout: {
        "text-field": ["coalesce", ["get", "name"], "Node"],
        "text-font": ["Noto Sans Regular"],
        "text-size": 13,
        "text-anchor": "top",
        "text-offset": [0, 1.2],
        "text-optional": true,
      },
      paint: {
        "text-color": "#111827",
        "text-halo-color": "rgba(255,255,255,0.92)",
        "text-halo-width": 1.6,
      },
    } as any);

    map.addSource(SENSOR_SOURCE_ID, { type: "geojson", data: { type: "FeatureCollection", features: [] } } as any);
    map.addLayer({
      id: SENSOR_LAYER_ID,
      type: "circle",
      source: SENSOR_SOURCE_ID,
      paint: {
        "circle-radius": ["case", ["boolean", ["feature-state", "hover"], false], 7, 5],
        "circle-color": "#f97316",
        "circle-stroke-color": "#111827",
        "circle-stroke-width": ["case", ["boolean", ["feature-state", "hover"], false], 2, 1],
        "circle-opacity": 0.9,
      },
    } as any);
    map.addLayer({
      id: SENSOR_LABEL_LAYER_ID,
      type: "symbol",
      source: SENSOR_SOURCE_ID,
      minzoom: 14,
      layout: {
        "text-field": ["coalesce", ["get", "name"], "Sensor"],
        "text-font": ["Noto Sans Regular"],
        "text-size": 12,
        "text-anchor": "top",
        "text-offset": [0, 1.15],
        "text-optional": true,
      },
      paint: {
        "text-color": "#111827",
        "text-halo-color": "rgba(255,255,255,0.92)",
        "text-halo-width": 1.6,
      },
    } as any);

    const setHover = (sourceId: string, featureId: string | number | null, hover: boolean) => {
      if (featureId == null) return;
      try {
        map.setFeatureState({ source: sourceId, id: featureId }, { hover });
      } catch {
        // ignore invalid ids
      }
    };

    const nodeEnter = (event: any) => {
      const id = event?.features?.[0]?.id;
      setHover(NODE_SOURCE_ID, id, true);
      map.getCanvas().style.cursor = canEditRef.current ? "grab" : "pointer";
    };
    const nodeLeave = (event: any) => {
      const id = event?.features?.[0]?.id;
      setHover(NODE_SOURCE_ID, id, false);
      if (!dragRef.current) map.getCanvas().style.cursor = "";
    };
    const sensorEnter = (event: any) => {
      const id = event?.features?.[0]?.id;
      setHover(SENSOR_SOURCE_ID, id, true);
      map.getCanvas().style.cursor = canEditRef.current ? "grab" : "pointer";
    };
    const sensorLeave = (event: any) => {
      const id = event?.features?.[0]?.id;
      setHover(SENSOR_SOURCE_ID, id, false);
      if (!dragRef.current) map.getCanvas().style.cursor = "";
    };

    const openPopupForFeature = (
      kind: "node" | "sensor",
      feature: any,
      lngLat: { lng: number; lat: number },
    ) => {
      const name = typeof feature?.properties?.name === "string" ? feature.properties.name : kind;
      let subtitle = "";
      if (kind === "node") {
        const status = typeof feature?.properties?.status === "string" ? feature.properties.status : "unknown";
        const lastSeen = feature?.properties?.last_seen ?? null;
        subtitle = formatNodeStatusLabel(status, lastSeen);
      } else {
        const nodeId = typeof feature?.properties?.node_id === "string" ? feature.properties.node_id : null;
        subtitle = nodeId ? `Overrides ${nodeId}` : "Location override";
      }

      new maplibregl.Popup({ offset: 16 })
        .setLngLat(lngLat)
        .setHTML(
          `<div style="font-weight:700;margin-bottom:4px;">${String(name)}</div>` +
            `<div style="font-size:12px;color:#6b7280;">${String(subtitle)} · ${lngLat.lat.toFixed(6)}, ${lngLat.lng.toFixed(6)}</div>`,
        )
        .addTo(map);
    };

    const handleNodeClick = (event: any) => {
      const feature = event?.features?.[0];
      if (!feature || !event?.lngLat) return;
      openPopupForFeature("node", feature, event.lngLat);
    };
    const handleSensorClick = (event: any) => {
      const feature = event?.features?.[0];
      if (!feature || !event?.lngLat) return;
      openPopupForFeature("sensor", feature, event.lngLat);
    };

    map.on("click", NODE_LAYER_ID, handleNodeClick);
    map.on("click", SENSOR_LAYER_ID, handleSensorClick);

    const beginDrag = (kind: "node" | "sensor", id: string) => {
      dragRef.current = { kind, id };
      map.dragPan.disable();
      map.getCanvas().style.cursor = "grabbing";
    };

    const handleNodeMouseDown = (event: any) => {
      if (!canEditRef.current) return;
      if (placementActiveRef.current) return;
      const draw = drawRef.current;
      const mode = draw?.getMode?.();
      if (typeof mode === "string" && mode !== "simple_select" && mode !== "direct_select") return;
      const feature = event?.features?.[0];
      const nodeId = feature?.properties?.node_id ?? feature?.id;
      if (!nodeId) return;
      event.preventDefault();
      beginDrag("node", String(nodeId));
    };

    const handleSensorMouseDown = (event: any) => {
      if (!canEditRef.current) return;
      if (placementActiveRef.current) return;
      const draw = drawRef.current;
      const mode = draw?.getMode?.();
      if (typeof mode === "string" && mode !== "simple_select" && mode !== "direct_select") return;
      const feature = event?.features?.[0];
      const sensorId = feature?.properties?.sensor_id ?? feature?.id;
      if (!sensorId) return;
      event.preventDefault();
      beginDrag("sensor", String(sensorId));
    };

    map.on("mousedown", NODE_LAYER_ID, handleNodeMouseDown);
    map.on("mousedown", SENSOR_LAYER_ID, handleSensorMouseDown);

    const updateDraggedFeature = (kind: "node" | "sensor", id: string, lng: number, lat: number) => {
      const collection = kind === "node" ? nodeGeojsonRef.current : sensorGeojsonRef.current;
      if (!collection?.features) return;
      const idx = collection.features.findIndex((f: any) => String(f?.id) === id);
      if (idx < 0) return;
      const geometry = collection.features[idx]?.geometry;
      if (!geometry || geometry.type !== "Point") return;
      geometry.coordinates = [lng, lat];
      const srcId = kind === "node" ? NODE_SOURCE_ID : SENSOR_SOURCE_ID;
      const src = map.getSource(srcId) as any;
      if (src && typeof src.setData === "function") {
        src.setData(collection);
      }
    };

    const endDrag = (event: any) => {
      const drag = dragRef.current;
      if (!drag) return;
      dragRef.current = null;
      map.dragPan.enable();
      map.getCanvas().style.cursor = "";
      if (!event?.lngLat) return;
      const lng = event.lngLat.lng;
      const lat = event.lngLat.lat;
      if (drag.kind === "node") void onUpsertEntityLocationRef.current({ nodeId: drag.id, lng, lat });
      if (drag.kind === "sensor") void onUpsertEntityLocationRef.current({ sensorId: drag.id, lng, lat });
    };

    const handleMouseMove = (event: any) => {
      const drag = dragRef.current;
      if (!drag || !event?.lngLat) return;
      updateDraggedFeature(drag.kind, drag.id, event.lngLat.lng, event.lngLat.lat);
    };

    map.on("mousemove", handleMouseMove);

    map.on("mouseup", endDrag);
    map.on("touchend", endDrag);

    map.on("mouseenter", NODE_LAYER_ID, nodeEnter);
    map.on("mouseleave", NODE_LAYER_ID, nodeLeave);
    map.on("mouseenter", SENSOR_LAYER_ID, sensorEnter);
    map.on("mouseleave", SENSOR_LAYER_ID, sensorLeave);

    mapInteractionsReadyRef.current = true;

    return () => {
      dragRef.current = null;
      mapInteractionsReadyRef.current = false;

      const anyMap = map as any;
      if (anyMap?._removed) return;

      try {
        map.off("mouseenter", NODE_LAYER_ID, nodeEnter);
        map.off("mouseleave", NODE_LAYER_ID, nodeLeave);
        map.off("mouseenter", SENSOR_LAYER_ID, sensorEnter);
        map.off("mouseleave", SENSOR_LAYER_ID, sensorLeave);
        map.off("click", NODE_LAYER_ID, handleNodeClick);
        map.off("click", SENSOR_LAYER_ID, handleSensorClick);
        map.off("mousedown", NODE_LAYER_ID, handleNodeMouseDown);
        map.off("mousedown", SENSOR_LAYER_ID, handleSensorMouseDown);
        map.off("mousemove", handleMouseMove);
        map.off("mouseup", endDrag);
        map.off("touchend", endDrag);
      } catch {
        // ignore teardown errors to avoid breaking navigation
      }

      try {
        if (map.getLayer(NODE_LABEL_LAYER_ID)) map.removeLayer(NODE_LABEL_LAYER_ID);
        if (map.getLayer(NODE_LAYER_ID)) map.removeLayer(NODE_LAYER_ID);
        if (map.getSource(NODE_SOURCE_ID)) map.removeSource(NODE_SOURCE_ID);
        if (map.getLayer(SENSOR_LABEL_LAYER_ID)) map.removeLayer(SENSOR_LABEL_LAYER_ID);
        if (map.getLayer(SENSOR_LAYER_ID)) map.removeLayer(SENSOR_LAYER_ID);
        if (map.getSource(SENSOR_SOURCE_ID)) map.removeSource(SENSOR_SOURCE_ID);
      } catch {
        // ignore teardown errors to avoid breaking navigation
      }
    };
  }, [loaded]);

  useEffect(() => {
    const map = mapRef.current;
    if (!map || !loaded || !mapInteractionsReadyRef.current) return;

    const nodeSource = map.getSource(NODE_SOURCE_ID) as any;
    if (nodeSource && typeof nodeSource.setData === "function") {
      nodeSource.setData(nodeFeatureCollection);
      nodeGeojsonRef.current = nodeFeatureCollection;
    }

    const sensorSource = map.getSource(SENSOR_SOURCE_ID) as any;
    if (sensorSource && typeof sensorSource.setData === "function") {
      sensorSource.setData(sensorFeatureCollection);
      sensorGeojsonRef.current = sensorFeatureCollection;
    }
  }, [loaded, nodeFeatureCollection, sensorFeatureCollection]);

  useImperativeHandle(
    ref,
    () => ({
      zoomToFeet: (feet: number) => {
        const map = mapRef.current;
        if (!map) return;
        const container = map.getContainer();
        const heightPx = container.clientHeight || 0;
        if (!heightPx || feet <= 0) {
          map.easeTo({ zoom: Math.min(24, Math.max(0, 20)), duration: 800 });
          return;
        }

        const center = map.getCenter();
        const targetMeters = feet / 3.28084;
        const metersPerPixel = targetMeters / heightPx;
        const base = 156543.03392 * Math.cos((center.lat * Math.PI) / 180);
        if (!Number.isFinite(base) || base <= 0 || !Number.isFinite(metersPerPixel) || metersPerPixel <= 0) {
          map.easeTo({ zoom: Math.min(24, Math.max(0, 20)), duration: 800 });
          return;
        }

        const desiredZoom = Math.log2(base / metersPerPixel);
        const clamped = Math.min(24, Math.max(0, desiredZoom));
        map.easeTo({ zoom: clamped, duration: 650 });

        if (zoomAdjustTimerRef.current != null) {
          window.clearTimeout(zoomAdjustTimerRef.current);
          zoomAdjustTimerRef.current = null;
        }

        zoomAdjustTimerRef.current = window.setTimeout(() => {
          const activeMap = mapRef.current;
          const anyActiveMap = activeMap as any;
          if (!activeMap || anyActiveMap?._removed) return;
          const currentViewport = viewportHeightFeet(activeMap);
          if (!currentViewport || currentViewport <= 0) return;
          const delta = Math.log2(currentViewport / feet);
          const next = Math.min(24, Math.max(0, activeMap.getZoom() + delta));
          activeMap.easeTo({ zoom: next, duration: 450 });
        }, 700);
      },
      getView: () => {
        const map = mapRef.current;
        if (!map) return null;
        const center = map.getCenter();
        return {
          center_lat: center.lat,
          center_lng: center.lng,
          zoom: map.getZoom(),
          bearing: map.getBearing(),
          pitch: map.getPitch(),
        };
      },
      focusGeometry: (geometry: unknown) => {
        const map = mapRef.current;
        if (!map) return;
        const point = pointFromGeometry(geometry);
        if (point) {
          map.easeTo({
            center: [point.lng, point.lat],
            zoom: Math.max(map.getZoom(), 19),
            duration: 700,
          });
          return;
        }
        const bounds = boundsFromGeometry(geometry);
        if (!bounds) return;
        map.fitBounds(
          [
            [bounds.minLng, bounds.minLat],
            [bounds.maxLng, bounds.maxLat],
          ],
          { padding: 80, duration: 800 },
        );
      },
      attachBackendId: (drawId: string, backendId: number) => {
        const draw = drawRef.current;
        if (!draw) return;
        draw.setFeatureProperty(drawId, "backend_id", backendId);
      },
      discardDrawFeature: (drawId: string) => {
        const draw = drawRef.current;
        if (!draw) return;
        draw.delete(drawId);
      },
      startDraw: (mode: "point" | "line" | "polygon") => {
        const draw = drawRef.current;
        if (!draw) return;
        if (mode === "point") draw.changeMode("draw_point");
        if (mode === "line") draw.changeMode("draw_line_string");
        if (mode === "polygon") draw.changeMode("draw_polygon");
      },
      stopDraw: () => {
        const draw = drawRef.current;
        if (!draw) return;
        try {
          draw.changeMode("simple_select");
        } catch {
          // ignore
        }
      },
    }),
    [],
  );

  return (
    <div className="absolute inset-0">
      <div ref={containerRef} className="h-full w-full" />

      {!loaded && !loadError ? (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-black/0">
 <div className="pointer-events-auto rounded-xl border border-border bg-white/95 px-4 py-3 text-xs text-foreground shadow-xs backdrop-blur">
            Loading map…
          </div>
        </div>
      ) : null}

      {loadError ? (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-black/0">
          <InlineBanner tone="danger" className="pointer-events-auto max-w-md text-xs">
            <div className="font-semibold">Map failed to load</div>
            <div className="mt-1 break-words opacity-90">{loadError}</div>
          </InlineBanner>
        </div>
      ) : null}

      <div className="pointer-events-none absolute left-3 top-3 flex flex-col gap-2">
 <div className="pointer-events-auto rounded-xl border border-border bg-white/95 px-3 py-2 text-xs text-foreground shadow-xs backdrop-blur">
          <div className="flex items-center justify-between gap-3">
            <div className="font-semibold">Zoom</div>
            <div>{zoomLevel.toFixed(2)}</div>
          </div>
          <div className="mt-1 flex items-center justify-between gap-3">
            <div className="font-semibold">Eye altitude</div>
            <div>{altitudeFt ? `${Math.round(altitudeFt).toLocaleString()}′` : "—"}</div>
          </div>
          <div className="mt-1 flex items-center justify-between gap-3">
            <div className="font-semibold">Viewport height</div>
            <div>{viewportFt ? `${Math.round(viewportFt).toLocaleString()}′` : "—"}</div>
          </div>
        </div>
      </div>
    </div>
  );
});
