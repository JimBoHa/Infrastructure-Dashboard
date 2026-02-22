"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import ReorderableList, { type ReorderableListItem } from "@/components/ReorderableList";
import NodeButton from "@/features/nodes/components/NodeButton";
import { queryKeys } from "@/lib/queries";
import { updateNodeOrder, updateNodeSensorsOrder } from "@/lib/api";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

const moveInArray = <T,>(items: T[], from: number, to: number): T[] => {
  const next = items.slice();
  const [removed] = next.splice(from, 1);
  next.splice(to, 0, removed);
  return next;
};

const arraysEqual = (a: string[], b: string[]) => a.length === b.length && a.every((v, idx) => v === b[idx]);

export default function DisplayOrderModal({
  open,
  nodes,
  sensors,
  onClose,
}: {
  open: boolean;
  nodes: DemoNode[];
  sensors: DemoSensor[];
  onClose: () => void;
}) {
  const queryClient = useQueryClient();
  const [nodeOrder, setNodeOrder] = useState<string[]>([]);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [sensorOrderByNode, setSensorOrderByNode] = useState<Record<string, string[]>>({});
  const initialNodeOrder = useRef<string[]>([]);
  const initialSensorOrderByNode = useRef<Record<string, string[]>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const nodesById = useMemo(() => new Map(nodes.map((n) => [n.id, n])), [nodes]);
  const sensorsById = useMemo(() => new Map(sensors.map((s) => [s.sensor_id, s])), [sensors]);

  const sensorsForSelectedNode = useMemo(() => {
    if (!selectedNodeId) return [];
    return sensorOrderByNode[selectedNodeId] ?? [];
  }, [selectedNodeId, sensorOrderByNode]);

  useEffect(() => {
    if (!open) return;
    const orderedNodeIds = nodes.map((node) => node.id);
    const byNode: Record<string, string[]> = {};
    for (const node of nodes) {
      byNode[node.id] = [];
    }
    for (const sensor of sensors) {
      const nodeId = sensor.node_id;
      if (!nodeId) continue;
      if (!byNode[nodeId]) byNode[nodeId] = [];
      byNode[nodeId].push(sensor.sensor_id);
    }

    initialNodeOrder.current = orderedNodeIds;
    initialSensorOrderByNode.current = byNode;
    setNodeOrder(orderedNodeIds);
    setSensorOrderByNode(byNode);
    setSelectedNodeId((prev) => (prev && nodesById.has(prev) ? prev : orderedNodeIds[0] ?? null));
    setError(null);
    setBusy(false);
  }, [nodes, nodesById, open, sensors]);

  const nodeItems: ReorderableListItem[] = nodeOrder
    .map((id) => nodesById.get(id))
    .filter((node): node is DemoNode => Boolean(node))
    .map((node) => {
      const count = (sensorOrderByNode[node.id] ?? []).length;
      return {
        id: node.id,
        title: node.name,
        subtitle: `${count} sensor${count === 1 ? "" : "s"}`,
      };
    });

  const sensorItems: ReorderableListItem[] = sensorsForSelectedNode
    .map((id) => sensorsById.get(id))
    .filter((sensor): sensor is DemoSensor => Boolean(sensor))
    .map((sensor) => ({
      id: sensor.sensor_id,
      title: sensor.name,
      subtitle: `${sensor.type}${sensor.unit ? ` / ${sensor.unit}` : ""}`,
    }));

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const nodesChanged = !arraysEqual(nodeOrder, initialNodeOrder.current);
      if (nodesChanged) {
        await updateNodeOrder(nodeOrder);
      }

      const sensorNodesToUpdate = Object.entries(sensorOrderByNode)
        .filter(([nodeId, ids]) => {
          const before = initialSensorOrderByNode.current[nodeId] ?? [];
          return !arraysEqual(ids, before);
        })
        .map(([nodeId, ids]) => ({ nodeId, ids }));

      for (const entry of sensorNodesToUpdate) {
        if (!entry.ids.length) continue;
        await updateNodeSensorsOrder(entry.nodeId, entry.ids);
      }

      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
      ]);

      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save display order.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="max-w-5xl max-h-[calc(100vh-2rem)] overflow-y-auto gap-0 p-4 sm:p-6">
          <header className="sticky top-0 z-10 -mx-4 -mt-4 border-b border-border bg-card px-4 pt-4 pb-4 shadow-xs sm:-mx-6 sm:-mt-6 sm:px-6 sm:pt-6">
            <div className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
            <div className="space-y-1">
              <DialogTitle>
                Display order
              </DialogTitle>
              <DialogDescription>
                Drag-and-drop to reorder nodes and sensors. This order is stored on the controller and applies across tabs.
              </DialogDescription>
            </div>
            <div className="flex items-center gap-2">
              <NodeButton onClick={onClose} disabled={busy}>
                Cancel
              </NodeButton>
              <NodeButton variant="primary" onClick={() => void save()} disabled={busy}>
                {busy ? "Saving…" : "Save"}
              </NodeButton>
            </div>
            </div>
          </header>

          {error ? (
            <div className="mt-4">
              <InlineBanner tone="error">{error}</InlineBanner>
            </div>
          ) : null}

          <div className="mt-5 grid gap-6 lg:grid-cols-[340px_1fr]">
            <section className="space-y-2">
              <div className="flex items-center justify-between">
 <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Nodes
                </h4>
 <span className="text-xs text-muted-foreground">
                  {nodeItems.length} total
                </span>
              </div>
              <ReorderableList
                items={nodeItems}
                activeId={selectedNodeId}
                onSelect={(id) => setSelectedNodeId(id)}
                onMove={(fromIndex, toIndex) => {
                  setNodeOrder((prev) => moveInArray(prev, fromIndex, toIndex));
                }}
              />
            </section>

            <section className="space-y-2">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
 <h4 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Sensors
                  </h4>
 <p className="truncate text-xs text-muted-foreground">
                    {selectedNodeId
                      ? nodesById.get(selectedNodeId)?.name ?? selectedNodeId
                      : "Select a node to reorder sensors."}
                  </p>
                </div>
 <span className="shrink-0 text-xs text-muted-foreground">
                  {sensorItems.length} shown
                </span>
              </div>

              {selectedNodeId ? (
                <ReorderableList
                  items={sensorItems}
                  onMove={(fromIndex, toIndex) => {
                    setSensorOrderByNode((prev) => {
                      const current = prev[selectedNodeId] ?? [];
                      return { ...prev, [selectedNodeId]: moveInArray(current, fromIndex, toIndex) };
                    });
                  }}
                />
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
