"use client";

import InlineBanner from "@/components/InlineBanner";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";

type MapSaveModalProps = {
  isOpen: boolean;
  nameDraft: string;
  busy: boolean;
  error: string | null;
  onNameChange: (value: string) => void;
  onCancel: () => void;
  onConfirm: () => void;
};

export default function MapSaveModal({
  isOpen,
  nameDraft,
  busy,
  error,
  onNameChange,
  onCancel,
  onConfirm,
}: MapSaveModalProps) {
  return (
    <Dialog open={isOpen} onOpenChange={(v) => { if (!v && !busy) onCancel(); }}>
      <DialogContent className="gap-0 p-5">
        <DialogTitle>Save as new map</DialogTitle>
        <DialogDescription className="mt-1">
          Creates a named copy of the current map (placements + markup + view) so you can switch back later.
        </DialogDescription>

        <div className="mt-4 space-y-2">
 <label className="text-xs font-semibold text-foreground">Save name</label>
          <Input
            value={nameDraft}
            onChange={(event) => onNameChange(event.target.value)}
            placeholder="e.g. North field (winter layout)"
            disabled={busy}
            autoFocus
          />
          {error ? (
            <InlineBanner tone="danger" className="rounded-lg px-3 py-2 text-xs">{error}</InlineBanner>
          ) : null}
        </div>

        <div className="mt-5 flex items-center justify-end gap-2">
          <NodeButton size="sm" onClick={onCancel} disabled={busy}>
            Cancel
          </NodeButton>
          <NodeButton size="sm" variant="primary" onClick={onConfirm} disabled={busy}>
            {busy ? "Saving\u2026" : "Save"}
          </NodeButton>
        </div>
      </DialogContent>
    </Dialog>
  );
}
