"use client";

import { useCallback, useMemo, useState } from "react";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import InlineBanner from "@/components/InlineBanner";
import ReorderableList, { type ReorderableListItem } from "@/components/ReorderableList";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

export type OverviewLocalSensorsPrefsV1 = {
  version: 1;
  order: string[];
  hidden: string[];
};

export type OverviewLocalSensorsPrefsV2 = {
  version: 2;
  node_order: string[];
  sensor_order_by_node: Record<string, string[]>;
  hidden: string[];
};

const moveInArray = <T,>(items: T[], from: number, to: number): T[] => {
  const next = items.slice();
  const [removed] = next.splice(from, 1);
  next.splice(to, 0, removed);
  return next;
};

function reorderVisibleSubset({
  fullOrder,
  visibleOrder,
  fromIndex,
  toIndex,
}: {
  fullOrder: string[];
  visibleOrder: string[];
  fromIndex: number;
  toIndex: number;
}): string[] {
  if (fromIndex === toIndex) return fullOrder;
  if (fromIndex < 0 || toIndex < 0) return fullOrder;
  if (fromIndex >= visibleOrder.length || toIndex >= visibleOrder.length) return fullOrder;
  const movedVisible = moveInArray(visibleOrder, fromIndex, toIndex);
  const visibleSet = new Set(visibleOrder);
  let idx = 0;
  return fullOrder.map((id) => {
    if (!visibleSet.has(id)) return id;
    const nextId = movedVisible[idx];
    idx += 1;
    return nextId ?? id;
  });
}

function normalizeSearch(value: string): string {
  return value.trim().toLowerCase();
}

function sensorMatchesNeedle(sensor: DemoSensor, nodeName: string, needle: string): boolean {
  if (!needle) return true;
  const haystack = `${sensor.name} ${sensor.type} ${sensor.unit} ${sensor.sensor_id} ${nodeName}`.toLowerCase();
  return haystack.includes(needle);
}

function clampSensorLimit(sensorLimit: number): number {
  return Math.max(4, Math.min(24, Math.floor(sensorLimit)));
}

function deriveDefaultOrders({
  nodes,
  sensors,
}: {
  nodes: DemoNode[];
  sensors: DemoSensor[];
}): {
  nodeOrder: string[];
  sensorOrderByNode: Record<string, string[]>;
} {
  const nodeIdsWithSensors = new Set<string>();
  sensors.forEach((sensor) => {
    if (sensor.node_id) nodeIdsWithSensors.add(sensor.node_id);
  });

  const orderedFromNodes = nodes.map((node) => node.id).filter((id) => nodeIdsWithSensors.has(id));
  const unknownNodes = Array.from(nodeIdsWithSensors).filter((id) => !orderedFromNodes.includes(id));
  unknownNodes.sort();
  const nodeOrder = [...orderedFromNodes, ...unknownNodes];

  const sensorOrderByNode: Record<string, string[]> = {};
  nodeOrder.forEach((nodeId) => {
    sensorOrderByNode[nodeId] = [];
  });
  sensors.forEach((sensor) => {
    const nodeId = sensor.node_id;
    if (!nodeId) return;
    if (!sensorOrderByNode[nodeId]) sensorOrderByNode[nodeId] = [];
    sensorOrderByNode[nodeId].push(sensor.sensor_id);
  });

  return { nodeOrder, sensorOrderByNode };
}

function buildOrdersFromFlatSensorIds({
  sensorIds,
  sensorsById,
  defaultNodeOrder,
  defaultSensorOrderByNode,
}: {
  sensorIds: string[];
  sensorsById: Map<string, DemoSensor>;
  defaultNodeOrder: string[];
  defaultSensorOrderByNode: Record<string, string[]>;
}): {
  nodeOrder: string[];
  sensorOrderByNode: Record<string, string[]>;
} {
  const nodeOrder: string[] = [];
  const nodeSeen = new Set<string>();
  const sensorOrderByNode: Record<string, string[]> = {};

  const pushNode = (nodeId: string) => {
    if (!nodeId) return;
    if (nodeSeen.has(nodeId)) return;
    nodeSeen.add(nodeId);
    nodeOrder.push(nodeId);
  };

  sensorIds.forEach((sensorId) => {
    const sensor = sensorsById.get(sensorId);
    if (!sensor) return;
    const nodeId = sensor.node_id;
    if (!nodeId) return;
    pushNode(nodeId);
    if (!sensorOrderByNode[nodeId]) sensorOrderByNode[nodeId] = [];
    if (!sensorOrderByNode[nodeId].includes(sensorId)) sensorOrderByNode[nodeId].push(sensorId);
  });

  defaultNodeOrder.forEach(pushNode);

  nodeOrder.forEach((nodeId) => {
    const existing = sensorOrderByNode[nodeId] ?? [];
    const seen = new Set(existing);
    const defaults = defaultSensorOrderByNode[nodeId] ?? [];
    const merged = existing.slice();
    defaults.forEach((id) => {
      if (seen.has(id)) return;
      seen.add(id);
      merged.push(id);
    });
    sensorOrderByNode[nodeId] = merged;
  });

  return { nodeOrder, sensorOrderByNode };
}

export default function LocalSensorsConfigModal({
  sensorLimit,
  nodes,
  sensors,
  initialOrder,
  initialHidden,
  onClose,
  onSave,
}: {
  sensorLimit: number;
  nodes: DemoNode[];
  sensors: DemoSensor[];
  initialOrder: string[];
  initialHidden: string[];
  onClose: () => void;
  onSave: (prefs: OverviewLocalSensorsPrefsV2) => void;
}) {
  const limitLabel = clampSensorLimit(sensorLimit);

  const nodesById = useMemo(() => new Map(nodes.map((node) => [node.id, node])), [nodes]);
  const sensorsById = useMemo(() => new Map(sensors.map((sensor) => [sensor.sensor_id, sensor])), [sensors]);

  const defaults = useMemo(() => deriveDefaultOrders({ nodes, sensors }), [nodes, sensors]);
  const defaultNodeOrder = defaults.nodeOrder;
  const defaultSensorOrderByNode = defaults.sensorOrderByNode;

  const initialState = useMemo(() => {
    const hiddenSet = new Set(initialHidden.filter((id) => sensorsById.has(id)));
    const built = buildOrdersFromFlatSensorIds({
      sensorIds: initialOrder,
      sensorsById,
      defaultNodeOrder,
      defaultSensorOrderByNode,
    });

    return {
      nodeOrder: built.nodeOrder,
      sensorOrderByNode: built.sensorOrderByNode,
      hidden: hiddenSet,
      selectedNodeId: built.nodeOrder[0] ?? null,
    };
  }, [defaultNodeOrder, defaultSensorOrderByNode, initialHidden, initialOrder, sensorsById]);

  const [search, setSearch] = useState("");
  const [nodeOrder, setNodeOrder] = useState<string[]>(() => initialState.nodeOrder);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(() => initialState.selectedNodeId);
  const [sensorOrderByNode, setSensorOrderByNode] = useState<Record<string, string[]>>(
    () => initialState.sensorOrderByNode,
  );
  const [hidden, setHidden] = useState<Set<string>>(() => initialState.hidden);

  const searchNeedle = useMemo(() => normalizeSearch(search), [search]);

  const matchingSensorIdsByNode = useMemo(() => {
    const map = new Map<string, string[]>();
    nodeOrder.forEach((nodeId) => {
      const nodeName = nodesById.get(nodeId)?.name ?? nodeId;
      const ids = (sensorOrderByNode[nodeId] ?? []).filter((id) => {
        const sensor = sensorsById.get(id);
        return sensor ? sensorMatchesNeedle(sensor, nodeName, searchNeedle) : false;
      });
      map.set(nodeId, ids);
    });
    return map;
  }, [nodeOrder, nodesById, searchNeedle, sensorOrderByNode, sensorsById]);

  const visibleNodeOrder = useMemo(() => {
    if (!searchNeedle) return nodeOrder;
    return nodeOrder.filter((nodeId) => (matchingSensorIdsByNode.get(nodeId)?.length ?? 0) > 0);
  }, [matchingSensorIdsByNode, nodeOrder, searchNeedle]);

  const activeNodeId = useMemo(() => {
    if (selectedNodeId && visibleNodeOrder.includes(selectedNodeId)) return selectedNodeId;
    return visibleNodeOrder[0] ?? null;
  }, [selectedNodeId, visibleNodeOrder]);

  const shownOrderedIds = useMemo(() => {
    const ids: string[] = [];
    nodeOrder.forEach((nodeId) => {
      (sensorOrderByNode[nodeId] ?? []).forEach((id) => {
        if (hidden.has(id)) return;
        ids.push(id);
      });
    });
    return ids;
  }, [hidden, nodeOrder, sensorOrderByNode]);

  const inUseSet = useMemo(() => new Set(shownOrderedIds.slice(0, limitLabel)), [limitLabel, shownOrderedIds]);

  const moveNode = useCallback((nodeId: string, delta: number) => {
    setNodeOrder((prev) => {
      const from = prev.indexOf(nodeId);
      if (from < 0) return prev;
      const to = from + delta;
      if (to < 0 || to >= prev.length) return prev;
      return moveInArray(prev, from, to);
    });
  }, []);

  const nodeItems: ReorderableListItem[] = useMemo(() => {
    return visibleNodeOrder.map((nodeId) => {
      const allIds = sensorOrderByNode[nodeId] ?? [];
      const shownCount = allIds.filter((id) => !hidden.has(id)).length;
      const matchCount = matchingSensorIdsByNode.get(nodeId)?.length ?? 0;
      const nodeName = nodesById.get(nodeId)?.name ?? nodeId;
      const nodeIndex = nodeOrder.indexOf(nodeId);
      const subtitle = searchNeedle
        ? `${matchCount} match${matchCount === 1 ? "" : "es"} · ${shownCount}/${allIds.length} shown`
        : `${shownCount}/${allIds.length} shown`;
      return {
        id: nodeId,
        title: nodeName,
        subtitle,
        right: (
          <div
            className="flex items-center gap-1 sm:hidden"
            onClick={(event) => event.stopPropagation()}
            onPointerDown={(event) => event.stopPropagation()}
          >
            <NodeButton
              size="xs"
              type="button"
              onClick={() => moveNode(nodeId, -1)}
              disabled={nodeIndex <= 0}
            >
              Up
            </NodeButton>
            <NodeButton
              size="xs"
              type="button"
              onClick={() => moveNode(nodeId, 1)}
              disabled={nodeIndex < 0 || nodeIndex >= nodeOrder.length - 1}
            >
              Down
            </NodeButton>
          </div>
        ),
      };
    });
  }, [
    hidden,
    matchingSensorIdsByNode,
    moveNode,
    nodeOrder,
    nodesById,
    searchNeedle,
    sensorOrderByNode,
    visibleNodeOrder,
  ]);

  const sensorIdsForSelectedNode = useMemo(() => {
    if (!activeNodeId) return [];
    const ids = sensorOrderByNode[activeNodeId] ?? [];
    if (!searchNeedle) return ids;
    const matches = new Set(matchingSensorIdsByNode.get(activeNodeId) ?? []);
    return ids.filter((id) => matches.has(id));
  }, [activeNodeId, matchingSensorIdsByNode, searchNeedle, sensorOrderByNode]);

  const sensorItems: ReorderableListItem[] = useMemo(() => {
    if (!activeNodeId) return [];
    return sensorIdsForSelectedNode
      .map((id, idx) => {
        const sensor = sensorsById.get(id);
        if (!sensor) return null;
        const isHidden = hidden.has(id);
        const inUse = inUseSet.has(id);
        const subtitleBase = `${sensor.type}${sensor.unit ? ` · ${sensor.unit}` : ""}`;
        const subtitle = isHidden
          ? `${subtitleBase} · Hidden`
          : inUse
            ? `${subtitleBase} · In use`
            : `${subtitleBase} · Below cap`;

        const moveSensor = (delta: number) => {
          setSensorOrderByNode((prev) => {
            const current = prev[activeNodeId] ?? [];
            const from = current.indexOf(id);
            if (from < 0) return prev;
            const to = from + delta;
            if (to < 0 || to >= current.length) return prev;
            return { ...prev, [activeNodeId]: moveInArray(current, from, to) };
          });
        };

        const toggleHidden = () => {
          setHidden((prev) => {
            const next = new Set(prev);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return next;
          });
        };

        return {
          id,
          title: sensor.name,
          subtitle,
          right: (
            <div
              className="flex items-center gap-2"
              onClick={(event) => event.stopPropagation()}
              onPointerDown={(event) => event.stopPropagation()}
            >
              <div className="flex items-center gap-1 sm:hidden">
                <NodeButton size="xs" type="button" onClick={() => moveSensor(-1)} disabled={idx === 0}>
                  Up
                </NodeButton>
                <NodeButton
                  size="xs"
                  type="button"
                  onClick={() => moveSensor(1)}
                  disabled={idx >= sensorIdsForSelectedNode.length - 1}
                >
                  Down
                </NodeButton>
              </div>
              <NodeButton size="xs" type="button" onClick={toggleHidden}>
                {isHidden ? "Show" : "Hide"}
              </NodeButton>
            </div>
          ),
        };
      })
      .filter((item): item is NonNullable<typeof item> => item != null);
  }, [activeNodeId, hidden, inUseSet, sensorIdsForSelectedNode, sensorsById]);

  const resetToDefault = () => {
    setNodeOrder(defaultNodeOrder);
    setSensorOrderByNode(defaultSensorOrderByNode);
    setHidden(new Set());
    setSelectedNodeId(defaultNodeOrder[0] ?? null);
    setSearch("");
  };

  const hideAllForSelectedNode = () => {
    if (!activeNodeId) return;
    setHidden((prev) => {
      const next = new Set(prev);
      (sensorOrderByNode[activeNodeId] ?? []).forEach((id) => next.add(id));
      return next;
    });
  };

  const showAllForSelectedNode = () => {
    if (!activeNodeId) return;
    setHidden((prev) => {
      const next = new Set(prev);
      (sensorOrderByNode[activeNodeId] ?? []).forEach((id) => next.delete(id));
      return next;
    });
  };

  const save = () => {
    const out: OverviewLocalSensorsPrefsV2 = {
      version: 2,
      node_order: nodeOrder,
      sensor_order_by_node: sensorOrderByNode,
      hidden: Array.from(hidden.values()),
    };
    onSave(out);
    onClose();
  };

  return (
    <Dialog open onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="max-w-5xl max-h-[calc(100vh-2rem)] overflow-y-auto gap-0 p-4 sm:p-6">
        <header className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
          <div className="space-y-1">
            <DialogTitle>Configure local sensors</DialogTitle>
            <DialogDescription>
              Choose which sensors appear in the Overview visualizations and set the priority order.
            </DialogDescription>
 <p className="text-xs text-muted-foreground">
              Only the first <span className="font-semibold">{limitLabel}</span> shown sensors are queried/rendered.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <NodeButton onClick={onClose} type="button">
              Cancel
            </NodeButton>
            <NodeButton variant="primary" onClick={save} type="button">
              Save
            </NodeButton>
          </div>
        </header>

        <div className="mt-5 flex flex-wrap items-end justify-between gap-3">
 <label className="flex min-w-[240px] flex-1 flex-col gap-1 text-xs font-semibold text-muted-foreground">
            Search
            <Input
              className="h-9 text-foreground"
              placeholder="Filter by sensor or node…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </label>

          <div className="flex flex-wrap gap-2">
            <NodeButton size="sm" onClick={resetToDefault} type="button">
              Reset to default
            </NodeButton>
          </div>
        </div>

        <div className="mt-5 grid gap-6 lg:grid-cols-[340px_1fr]">
          <section className="space-y-2">
            <div className="flex items-center justify-between">
 <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Nodes
              </h4>
 <span className="text-xs text-muted-foreground">
                {visibleNodeOrder.length} shown
              </span>
            </div>
            {!visibleNodeOrder.length ? (
              <Card className="rounded-lg gap-0 border-dashed p-4 text-sm text-muted-foreground">
                No matching nodes.
              </Card>
            ) : (
              <ReorderableList
                items={nodeItems}
                activeId={activeNodeId}
                onSelect={(id) => setSelectedNodeId(id)}
                onMove={(fromIndex, toIndex) => {
                  setNodeOrder((prev) => {
                    const visible = searchNeedle
                      ? prev.filter((nodeId) => {
                          const nodeName = nodesById.get(nodeId)?.name ?? nodeId;
                          const ids = sensorOrderByNode[nodeId] ?? [];
                          return ids.some((id) => {
                            const sensor = sensorsById.get(id);
                            return sensor ? sensorMatchesNeedle(sensor, nodeName, searchNeedle) : false;
                          });
                        })
                      : prev;
                    return reorderVisibleSubset({ fullOrder: prev, visibleOrder: visible, fromIndex, toIndex });
                  });
                }}
              />
            )}
          </section>

          <section className="space-y-2">
            <div className="flex flex-wrap items-start justify-between gap-2">
              <div>
 <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Sensors
                </h4>
 <p className="text-xs text-muted-foreground">
                  {activeNodeId ? nodesById.get(activeNodeId)?.name ?? activeNodeId : "Select a node to reorder sensors."}
                </p>
              </div>
              {activeNodeId ? (
                <div className="flex items-center gap-2">
                  <NodeButton size="xs" type="button" onClick={showAllForSelectedNode}>
                    Show all
                  </NodeButton>
                  <NodeButton size="xs" type="button" onClick={hideAllForSelectedNode}>
                    Hide all
                  </NodeButton>
                </div>
              ) : null}
            </div>

            {activeNodeId ? (
              sensorItems.length ? (
                <>
                  <ReorderableList
                    items={sensorItems}
                    onMove={(fromIndex, toIndex) => {
                      setSensorOrderByNode((prev) => {
                        const current = prev[activeNodeId] ?? [];
                        const nodeName = nodesById.get(activeNodeId)?.name ?? activeNodeId;
                        const visible = searchNeedle
                          ? current.filter((id) => {
                              const sensor = sensorsById.get(id);
                              return sensor ? sensorMatchesNeedle(sensor, nodeName, searchNeedle) : false;
                            })
                          : current;
                        const next = reorderVisibleSubset({
                          fullOrder: current,
                          visibleOrder: visible,
                          fromIndex,
                          toIndex,
                        });
                        return { ...prev, [activeNodeId]: next };
                      });
                    }}
                  />
                  <InlineBanner tone="info" className="mt-3">
                    Drag the handle (desktop) or use Up/Down (mobile) to reorder. Hide removes sensors from Overview only.
                  </InlineBanner>
                </>
              ) : (
                <Card className="rounded-lg gap-0 border-dashed p-4 text-sm text-muted-foreground">
                  No matching sensors for this node.
                </Card>
              )
            ) : (
              <Card className="rounded-lg gap-0 border-dashed p-4 text-sm text-muted-foreground">
                Select a node to reorder its sensors.
              </Card>
            )}
          </section>
        </div>
      </DialogContent>
    </Dialog>
  );
}
