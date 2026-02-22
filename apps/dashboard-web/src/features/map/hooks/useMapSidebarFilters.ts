"use client";

import { useCallback, useMemo, useState } from "react";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { MapFeature } from "@/types/map";

type UseMapSidebarFiltersOptions = {
  nodes: DemoNode[];
  sensors: DemoSensor[];
  featuresByNode: Map<string, MapFeature>;
  featuresBySensor: Map<string, MapFeature>;
  customFeatures: MapFeature[];
  sensorsByNode: Map<string, DemoSensor[]>;
};

export function useMapSidebarFilters({
  nodes,
  sensors,
  featuresByNode,
  featuresBySensor,
  customFeatures,
  sensorsByNode,
}: UseMapSidebarFiltersOptions) {
  const [featureSearch, setFeatureSearch] = useState("");
  const [deviceSearch, setDeviceSearch] = useState("");
  const [expandedNodeIds, setExpandedNodeIds] = useState<Set<string>>(() => new Set());
  const [showAllBaseLayers, setShowAllBaseLayers] = useState(false);

  const filteredCustomFeatures = useMemo(() => {
    const q = featureSearch.trim().toLowerCase();
    if (!q) return customFeatures;
    return customFeatures.filter((feature) => {
      const props = (feature.properties ?? {}) as Record<string, unknown>;
      const name = typeof props.name === "string" ? props.name : "";
      const kind = typeof props.kind === "string" ? props.kind : "";
      return name.toLowerCase().includes(q) || kind.toLowerCase().includes(q) || String(feature.id).includes(q);
    });
  }, [customFeatures, featureSearch]);

  const unplacedNodes = useMemo(
    () => nodes.filter((node) => !featuresByNode.has(node.id)),
    [nodes, featuresByNode],
  );

  const deviceQuery = useMemo(() => deviceSearch.trim().toLowerCase(), [deviceSearch]);

  const filterNode = useCallback(
    (node: DemoNode) => {
      if (!deviceQuery) return true;
      const q = deviceQuery;
      if (node.name.toLowerCase().includes(q) || node.id.toLowerCase().includes(q)) return true;
      return sensors.some((sensor) => {
        if (sensor.node_id !== node.id) return false;
        return sensor.name.toLowerCase().includes(q) || sensor.sensor_id.toLowerCase().includes(q);
      });
    },
    [deviceQuery, sensors],
  );

  const filterSensor = useCallback(
    (sensor: DemoSensor) => {
      if (!deviceQuery) return true;
      const q = deviceQuery;
      return (
        sensor.name.toLowerCase().includes(q) ||
        sensor.sensor_id.toLowerCase().includes(q) ||
        sensor.node_id.toLowerCase().includes(q)
      );
    },
    [deviceQuery],
  );

  const filteredNodes = useMemo(() => nodes.filter(filterNode), [nodes, filterNode]);

  const unassignedSensors = useMemo(
    () => sensors.filter((sensor) => !sensor.node_id || !sensor.node_id.trim().length),
    [sensors],
  );

  const filteredUnassignedSensors = useMemo(
    () => unassignedSensors.filter(filterSensor),
    [unassignedSensors, filterSensor],
  );

  const toggleNodeExpanded = useCallback((nodeId: string) => {
    setExpandedNodeIds((prev) => {
      const next = new Set(prev);
      if (next.has(nodeId)) next.delete(nodeId);
      else next.add(nodeId);
      return next;
    });
  }, []);

  const formatCoords = useCallback((geometry: unknown): string | null => {
    if (!geometry || typeof geometry !== "object") return null;
    const record = geometry as Record<string, unknown>;
    if (record.type !== "Point") return null;
    const coords = record.coordinates;
    if (!Array.isArray(coords) || coords.length < 2) return null;
    const lng = coords[0];
    const lat = coords[1];
    if (typeof lng !== "number" || typeof lat !== "number") return null;
    return `${lat.toFixed(6)}, ${lng.toFixed(6)}`;
  }, []);

  return {
    deviceSearch,
    setDeviceSearch,
    deviceQuery,
    featureSearch,
    setFeatureSearch,
    expandedNodeIds,
    toggleNodeExpanded,
    showAllBaseLayers,
    setShowAllBaseLayers,
    featuresByNode,
    featuresBySensor,
    customFeatures,
    filteredCustomFeatures,
    unplacedNodes,
    filteredNodes,
    sensorsByNode,
    filterSensor,
    filteredUnassignedSensors,
    formatCoords,
  };
}
