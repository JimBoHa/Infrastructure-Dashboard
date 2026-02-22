"use client";

import { useEffect, useId, useState } from "react";
import InlineBanner from "@/components/InlineBanner";
import { postJson } from "@/lib/api";
import NodeButton from "@/features/nodes/components/NodeButton";
import type { DemoAdoptionCandidate, DemoNode } from "@/types/dashboard";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export default function AdoptionModal({
  candidate,
  initialName,
  restoreOptions = [],
  onClose,
  onAdopted,
  onAdoptedNode,
  onError,
}: {
  candidate: DemoAdoptionCandidate | null;
  initialName?: string;
  restoreOptions?: { node_id: string; node_name: string; last_backup: string }[];
  onClose: () => void;
  onAdopted: (message: string) => void;
  onAdoptedNode?: (node: DemoNode) => void;
  onError: (message: string) => void;
}) {
  const [name, setName] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [restoreFrom, setRestoreFrom] = useState<string>("");
  const inputId = useId();
  const selectId = useId();

  useEffect(() => {
    setName(initialName ?? "");
    setError(null);
    setBusy(false);
    setRestoreFrom("");
  }, [candidate, initialName]);

  const defaultName = candidate
    ? (candidate.hostname ?? candidate.service_name.replace("._", " "))
    : "";

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!candidate) return;
    setBusy(true);
    setError(null);
    try {
      let token: string | undefined;
      try {
        const tokenResponse = await postJson<{ token: string }>("/api/adoption/tokens", {
          mac_eth: candidate.mac_eth,
          mac_wifi: candidate.mac_wifi,
          service_name: candidate.service_name,
        });
        token = tokenResponse.token?.trim();
      } catch (issueErr) {
        const message =
          issueErr instanceof Error ? issueErr.message : "Failed to issue adoption token";
        throw new Error(message);
      }
      if (!token) {
        throw new Error("Failed to issue adoption token");
      }
      const body: {
        name: string;
        mac_eth: string | null | undefined;
        mac_wifi: string | null | undefined;
        ip: string | null | undefined;
        port: number | null | undefined;
        status: string;
        token: string;
        restore_from_node_id?: string;
      } = {
        name: name.trim() || defaultName,
        mac_eth: candidate.mac_eth,
        mac_wifi: candidate.mac_wifi,
        ip: candidate.ip,
        port: candidate.port,
        status: "online",
        token,
      };
      if (restoreFrom) {
        body.restore_from_node_id = restoreFrom;
      }
      const response = await postJson<DemoNode>("/api/adopt", body);
      onAdoptedNode?.(response);
      const restoreLabel = restoreFrom
        ? restoreOptions.find((opt) => opt.node_id === restoreFrom)
        : null;
      const restoreSuffix = restoreLabel
        ? ` Restore queued from ${restoreLabel.node_name} (${restoreLabel.last_backup}).`
        : "";
      onAdopted(`Adopted ${response.name ?? body.name} successfully.${restoreSuffix}`);
      onClose();
    } catch (err) {
      const message = err instanceof Error ? err.message : "Adoption failed";
      setError(message);
      onError(message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={!!candidate} onOpenChange={(open) => { if (!open) onClose(); }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Adopt node</DialogTitle>
          {candidate && <DialogDescription>{candidate.service_name}</DialogDescription>}
        </DialogHeader>
        {candidate && (
          <form className="space-y-4" onSubmit={handleSubmit}>
            <div>
 <label className="mb-2 block text-sm font-medium text-foreground" htmlFor={inputId}>
                Display name
              </label>
              <input
                id={inputId}
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder={defaultName}
 className="block w-full rounded-lg border border-border px-3 py-2 text-sm focus:border-indigo-500 focus:ring-indigo-500"
              />
            </div>
 <div className="grid grid-cols-2 gap-3 text-xs text-muted-foreground">
              <p>MAC (eth): {candidate.mac_eth ?? "-"}</p>
              <p>MAC (wifi): {candidate.mac_wifi ?? "-"}</p>
              <p>IP: {candidate.ip ?? "-"}</p>
              <p>Port: {candidate.port ?? "-"}</p>
            </div>
            {restoreOptions.length > 0 && (
              <div>
 <label className="mb-2 block text-sm font-medium text-foreground" htmlFor={selectId}>
                  Restore from backup (optional)
                </label>
                <select
                  id={selectId}
                  value={restoreFrom}
                  onChange={(event) => setRestoreFrom(event.target.value)}
 className="block w-full rounded-lg border border-border px-3 py-2 text-sm focus:border-indigo-500 focus:ring-indigo-500"
                >
                  <option value="">Do not restore</option>
                  {restoreOptions.map((option) => (
                    <option key={option.node_id} value={option.node_id}>
                      {option.node_name} - latest {option.last_backup}
                    </option>
                  ))}
                </select>
 <p className="mt-2 text-xs text-muted-foreground">
                  Choose a previous node to push its latest configuration onto this device after adoption.
                </p>
              </div>
            )}
            {error && (
              <InlineBanner tone="danger" className="rounded-lg px-3 py-2">{error}</InlineBanner>
            )}
            <DialogFooter>
              <NodeButton type="button" onClick={onClose}>
                Cancel
              </NodeButton>
              <NodeButton
                type="submit"
                disabled={busy}
                variant="primary"
              >
                {busy ? "Adopting..." : "Adopt"}
              </NodeButton>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
