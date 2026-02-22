"use client";

import { useMemo, useState } from "react";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
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
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import InlineBanner from "@/components/InlineBanner";
import NodeButton from "@/features/nodes/components/NodeButton";
import type { MapFeature } from "@/types/map";

export type FeatureDraft = {
  name: string;
  kind: string;
  color: string;
  notes: string;
  geometry: unknown;
  properties: Record<string, unknown>;
};

const asString = (value: unknown): string =>
  typeof value === "string" ? value : value == null ? "" : String(value);

const DEFAULT_COLORS = [
  { label: "Green (fields)", value: "#22c55e" },
  { label: "Blue (water)", value: "#3b82f6" },
  { label: "Orange (hardware)", value: "#f97316" },
  { label: "Purple (utilities)", value: "#a855f7" },
  { label: "Gray", value: "#6b7280" },
];

export function featureDraftFromMapFeature(feature: MapFeature): FeatureDraft {
  const props = feature.properties ?? {};
  const name = asString(props.name) || asString(props.label) || "Untitled";
  const kind = asString(props.kind) || "overlay";
  const color = asString(props.color) || "#3b82f6";
  const notes = asString(props.notes);
  return {
    name,
    kind,
    color,
    notes,
    geometry: feature.geometry,
    properties: { ...(props as Record<string, unknown>) },
  };
}

export function FeatureEditorModal({
  draft,
  onClose,
  onSave,
  onDelete,
}: {
  draft: FeatureDraft;
  onClose: () => void;
  onSave: (draft: FeatureDraft) => Promise<void>;
  onDelete?: () => Promise<void>;
}) {
  const [local, setLocal] = useState<FeatureDraft>(draft);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const geometryType = useMemo(() => {
    if (!local.geometry || typeof local.geometry !== "object") return "Unknown";
    const record = local.geometry as Record<string, unknown>;
    return asString(record.type) || "Unknown";
  }, [local.geometry]);

  const canSave = Boolean(local.name.trim()) && Boolean(local.color.trim());

  return (
    <Dialog open onOpenChange={(v) => { if (!v && !busy) onClose(); }}>
      <DialogContent className="max-w-lg gap-0">
        <div className="flex items-start justify-between gap-3">
          <div>
            <DialogTitle>Edit map feature</DialogTitle>
            <DialogDescription className="mt-1">
              {geometryType} geometry · visible on the Map tab
            </DialogDescription>
          </div>
          <NodeButton size="sm" onClick={onClose}>
            Close
          </NodeButton>
        </div>

        <div className="mt-5 space-y-4">
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Label
            </label>
            <Input
              value={local.name}
              onChange={(e) => setLocal((prev) => ({ ...prev, name: e.target.value }))}
              placeholder="e.g., Field A, Main valve box, Drainage ditch"
              className="mt-1"
            />
 <p className="mt-1 text-xs text-muted-foreground">
              Use a short, descriptive name—this is what operators see.
            </p>
          </div>

          <div className="grid gap-4 sm:grid-cols-2">
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Type
              </label>
              <Select
                value={local.kind}
                onChange={(e) => setLocal((prev) => ({ ...prev, kind: e.target.value }))}
                className="mt-1"
              >
                <option value="field">Field / polygon</option>
                <option value="ditch">Drainage ditch</option>
                <option value="utility">Utility line</option>
                <option value="hardware">Hardware marker</option>
                <option value="note">Note</option>
              </Select>
 <p className="mt-1 text-xs text-muted-foreground">
                Helps filter and style overlays later.
              </p>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Color
              </label>
              <div className="mt-1 flex items-center gap-2">
                <input
                  type="color"
                  value={local.color}
                  onChange={(e) => setLocal((prev) => ({ ...prev, color: e.target.value }))}
 className="h-10 w-12 cursor-pointer rounded border border-border bg-white p-1"
                />
                <Select
                  value={DEFAULT_COLORS.some((c) => c.value === local.color) ? local.color : ""}
                  onChange={(e) => {
                    const value = e.target.value;
                    if (!value) return;
                    setLocal((prev) => ({ ...prev, color: value }));
                  }}
                >
                  <option value="">Choose a preset…</option>
                  {DEFAULT_COLORS.map((c) => (
                    <option key={c.value} value={c.value}>
                      {c.label}
                    </option>
                  ))}
                </Select>
              </div>
            </div>
          </div>

          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Notes (optional)
            </label>
            <Textarea
              value={local.notes}
              onChange={(e) => setLocal((prev) => ({ ...prev, notes: e.target.value }))}
              placeholder="Extra context for your team (ditch depth, valve size, etc.)"
              rows={4}
              className="mt-1"
            />
          </div>

          {error ? (
            <InlineBanner tone="danger" className="rounded-lg px-3 py-2">{error}</InlineBanner>
          ) : null}
        </div>

        <div className="mt-6 flex items-center justify-between gap-3">
          {onDelete ? (
            <>
              <NodeButton
                onClick={() => setConfirmDelete(true)}
                disabled={busy}
              >
                Delete
              </NodeButton>
              <AlertDialog open={confirmDelete} onOpenChange={setConfirmDelete}>
                <AlertDialogContent>
                  <AlertDialogHeader>
                    <AlertDialogTitle>Delete feature</AlertDialogTitle>
                    <AlertDialogDescription>
                      Delete this feature? This action cannot be undone.
                    </AlertDialogDescription>
                  </AlertDialogHeader>
                  <AlertDialogFooter>
                    <AlertDialogCancel>Cancel</AlertDialogCancel>
                    <AlertDialogAction
                      onClick={async () => {
                        setBusy(true);
                        setError(null);
                        try {
                          await onDelete();
                        } catch (err) {
                          setError(err instanceof Error ? err.message : "Delete failed.");
                          setBusy(false);
                          return;
                        }
                        setBusy(false);
                      }}
                    >
                      Delete
                    </AlertDialogAction>
                  </AlertDialogFooter>
                </AlertDialogContent>
              </AlertDialog>
            </>
          ) : (
            <div />
          )}

          <div className="flex items-center gap-3">
            <NodeButton onClick={onClose} disabled={busy}>
              Cancel
            </NodeButton>
            <NodeButton
              variant="primary"
              disabled={!canSave || busy}
              onClick={async () => {
                setBusy(true);
                setError(null);
                try {
                  const properties = {
                    ...(local.properties ?? {}),
                    name: local.name.trim(),
                    kind: local.kind,
                    color: local.color,
                    notes: local.notes,
                  } satisfies Record<string, unknown>;
                  await onSave({ ...local, properties });
                } catch (err) {
                  setError(err instanceof Error ? err.message : "Failed to save feature.");
                  setBusy(false);
                  return;
                }
                setBusy(false);
              }}
            >
              {busy ? "Saving\u2026" : "Save"}
            </NodeButton>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
