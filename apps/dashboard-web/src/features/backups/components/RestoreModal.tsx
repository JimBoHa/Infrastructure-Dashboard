"use client";

import { useState } from "react";
import type { DemoBackup, DemoNode } from "@/types/dashboard";
import NodeButton from "@/features/nodes/components/NodeButton";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";

export default function RestoreModal({
  backup,
  nodes,
  onClose,
  onConfirm,
}: {
  backup: DemoBackup | null;
  nodes: DemoNode[];
  onClose: () => void;
  onConfirm: (target: DemoNode | undefined) => void;
}) {
  const [targetId, setTargetId] = useState<string>("self");

  const targetNode = targetId === "self" ? undefined : nodes.find((node) => node.id === targetId);

  return (
    <Dialog open={!!backup} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Restore backup</DialogTitle>
          {backup && <DialogDescription>{backup.path}</DialogDescription>}
        </DialogHeader>
        {backup && (
 <div className="space-y-3 text-sm text-muted-foreground">
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Target node
              </label>
              <Select
                className="mt-1"
                value={targetId}
                onChange={(event) => setTargetId(event.target.value)}
              >
                <option value="self">Replace {backup.node_id}</option>
                {nodes
                  .filter((node) => node.id !== backup.node_id)
                  .map((node) => (
                    <option key={node.id} value={node.id}>
                      {node.name}
                    </option>
                  ))}
              </Select>
            </div>
 <p className="text-xs text-muted-foreground">
              The backup will be pushed to the target node after confirming. In demo mode this simulates the workflow.
            </p>
          </div>
        )}
        <DialogFooter>
          <NodeButton onClick={onClose}>
            Cancel
          </NodeButton>
          <NodeButton variant="primary" onClick={() => onConfirm(targetNode)}>
            Restore
          </NodeButton>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
