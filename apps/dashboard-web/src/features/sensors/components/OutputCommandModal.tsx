"use client";

import { useState } from "react";
import { postJson } from "@/lib/api";
import type { DemoOutput } from "@/types/dashboard";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import InlineBanner from "@/components/InlineBanner";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useAuth } from "@/components/AuthProvider";

export default function OutputCommandModal({
  output,
  onClose,
  onComplete,
  onError,
}: {
  output: DemoOutput | null | undefined;
  onClose: () => void;
  onComplete: (message: string) => void;
  onError: (message: string) => void;
}) {
  const { me } = useAuth();
  const canCommand = Boolean(me?.capabilities?.includes("outputs.command"));
  const [state, setState] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!output) return null;

  const supportedStates = output.supported_states?.length
    ? output.supported_states
    : ["on", "off"];

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!canCommand) {
      const message = "Insufficient permissions: outputs.command is required to send output commands.";
      setError(message);
      onError(message);
      return;
    }
    if (!state) {
      setError("Select a state to send");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const body = { state, reason: "manual" };
      const response = await postJson<DemoOutput>(`/api/outputs/${output.id}/command`, body);
      onComplete(`Command sent. Output is now ${response.state}.`);
      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to send command";
      setError(message);
      onError(message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={!!output} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="gap-0">
        <DialogTitle>Command output</DialogTitle>
        <DialogDescription className="mt-1">{output.name}</DialogDescription>
        <form className="mt-4 space-y-4" onSubmit={handleSubmit}>
          {!canCommand ? (
            <InlineBanner tone="warning" className="px-3 py-2 text-xs">
              Read-only: you need <code className="px-1">outputs.command</code> to send commands.
            </InlineBanner>
          ) : null}
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              State
            </label>
            <Select
              value={state}
              onChange={(event) => setState(event.target.value)}
              disabled={!canCommand || busy}
              className="mt-1"
            >
              <option value="">Select state...</option>
              {supportedStates.map((value) => (
                <option key={value} value={value}>
                  {value}
                </option>
              ))}
            </Select>
          </div>
          {error && (
            <InlineBanner tone="danger" className="rounded px-3 py-2 text-xs">{error}</InlineBanner>
          )}
          <div className="flex items-center justify-end gap-3">
            <NodeButton type="button" onClick={onClose}>
              Cancel
            </NodeButton>
            <NodeButton type="submit" disabled={busy || !canCommand} variant="primary">
              {busy ? "Sending..." : "Send"}
            </NodeButton>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
