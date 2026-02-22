"use client";

import { createContext, useContext, useMemo, type ReactNode } from "react";
import { useNodesQuery, useSensorsQuery } from "@/lib/queries";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

export type AnalyticsData = {
  nodes: DemoNode[];
  sensors: DemoSensor[];
  nodesById: Map<string, DemoNode>;
  nodeLabelsById: Map<string, string>;
  sensorsById: Map<string, DemoSensor>;
  sensorsByNodeId: Map<string, DemoSensor[]>;
  isLoading: boolean;
  error: unknown;
};

const AnalyticsDataContext = createContext<AnalyticsData | null>(null);

export function AnalyticsDataProvider({ children }: { children: ReactNode }) {
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();

  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);

  const nodesById = useMemo(() => new Map(nodes.map((node) => [node.id, node])), [nodes]);
  const nodeLabelsById = useMemo(() => new Map(nodes.map((node) => [node.id, node.name])), [nodes]);

  const sensorsById = useMemo(() => new Map(sensors.map((sensor) => [sensor.sensor_id, sensor])), [sensors]);

  const sensorsByNodeId = useMemo(() => {
    const map = new Map<string, DemoSensor[]>();
    sensors.forEach((sensor) => {
      const existing = map.get(sensor.node_id) ?? [];
      existing.push(sensor);
      map.set(sensor.node_id, existing);
    });
    return map;
  }, [sensors]);

  const value: AnalyticsData = useMemo(
    () => ({
      nodes,
      sensors,
      nodesById,
      nodeLabelsById,
      sensorsById,
      sensorsByNodeId,
      isLoading: nodesQuery.isLoading || sensorsQuery.isLoading,
      error: nodesQuery.error || sensorsQuery.error,
    }),
    [
      nodeLabelsById,
      nodes,
      nodesById,
      nodesQuery.error,
      nodesQuery.isLoading,
      sensors,
      sensorsById,
      sensorsByNodeId,
      sensorsQuery.error,
      sensorsQuery.isLoading,
    ],
  );

  return <AnalyticsDataContext.Provider value={value}>{children}</AnalyticsDataContext.Provider>;
}

export function useAnalyticsData() {
  const ctx = useContext(AnalyticsDataContext);
  if (!ctx) {
    throw new Error("useAnalyticsData must be used within <AnalyticsDataProvider />");
  }
  return ctx;
}

