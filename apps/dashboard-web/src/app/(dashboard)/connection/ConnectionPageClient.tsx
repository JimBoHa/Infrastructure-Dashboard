"use client";

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys, useConnectionQuery } from "@/lib/queries";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import InlineBanner from "@/components/InlineBanner";
import PageHeaderCard from "@/components/PageHeaderCard";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { DemoConnection } from "@/types/dashboard";
import { putJson } from "@/lib/api";
import NodeButton from "@/features/nodes/components/NodeButton";

export default function ConnectionPageClient() {
  const queryClient = useQueryClient();
  const { data: connection, error, isLoading } = useConnectionQuery();
  const [message, setMessage] = useState<string | null>(null);
  if (isLoading) return <LoadingState label="Loading connection…" />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load connection."} />;
  }
  if (!connection) return <ErrorState message="No connection loaded." />;

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Connection"
        description="Configure the controller to connect via local IP or cloud endpoint. Demo mode echoes settings locally."
      />

      {message && (
        <InlineBanner tone="info">{message}</InlineBanner>
      )}

      <ConnectionForm
        key={`${connection.mode}-${connection.local_address}-${connection.cloud_address}`}
        connection={connection}
        onSaved={(text) => {
          setMessage(text);
          void queryClient.invalidateQueries({ queryKey: queryKeys.connection });
        }}
      />

      <CollapsibleCard
        title="Need to adopt a new node?"
        description="Discovery scanning and adoption are managed from the Nodes tab."
        defaultOpen={false}
        actions={
          <NodeButton onClick={() => window.location.assign("/nodes")} size="sm">
            Open Nodes
          </NodeButton>
        }
      >
 <p className="text-sm text-muted-foreground">
          Use Nodes for scanning and adoption so the controller’s node inventory stays consistent.
        </p>
      </CollapsibleCard>
    </div>
  );
}

function ConnectionForm({
  connection,
  onSaved,
}: {
  connection: DemoConnection;
  onSaved: (message: string) => void;
}) {
  const [mode, setMode] = useState<"local" | "cloud">(
    connection.mode === "cloud" ? "cloud" : "local",
  );
  const [localAddress, setLocalAddress] = useState(connection.local_address);
  const [cloudAddress, setCloudAddress] = useState(connection.cloud_address);
  const [status, setStatus] = useState(connection.status);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSave = async () => {
    setBusy(true);
    setError(null);
    try {
      await putJson("/api/connection", {
        mode,
        local_address: localAddress,
        cloud_address: cloudAddress,
        status,
      });
      onSaved("Connection settings updated.");
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to update connection";
      setError(text);
    } finally {
      setBusy(false);
    }
  };

  return (
    <CollapsibleCard
      title="Controller connection"
      description="Choose whether to connect via local controller address or a cloud endpoint."
      defaultOpen
    >
      <form className="space-y-4">
        <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Mode
          </label>
          <div className="mt-2 flex gap-3">
            <button
              type="button"
              onClick={() => setMode("local")}
              className={`rounded-lg px-4 py-2 text-sm font-semibold transition-colors ${
                mode === "local"
                  ? "bg-indigo-600 text-white hover:bg-indigo-700"
 : "border border-border bg-white text-foreground hover:bg-muted"
              }`}
            >
              Local controller
            </button>
            <button
              type="button"
              onClick={() => setMode("cloud")}
              className={`rounded-lg px-4 py-2 text-sm font-semibold transition-colors ${
                mode === "cloud"
                  ? "bg-indigo-600 text-white hover:bg-indigo-700"
 : "border border-border bg-white text-foreground hover:bg-muted"
              }`}
            >
              Cloud
            </button>
          </div>
        </div>

        <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Local IP / Host
          </label>
          <Input
            value={localAddress}
            onChange={(event) => setLocalAddress(event.target.value)}
            className="mt-1"
            placeholder="http://192.168.1.40:8000"
          />
        </div>

        <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Cloud endpoint
          </label>
          <Input
            value={cloudAddress}
            onChange={(event) => setCloudAddress(event.target.value)}
            className="mt-1"
            placeholder="https://farm.example.com"
          />
        </div>

        <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Status
          </label>
          <Select
            value={status}
            onChange={(event) => setStatus(event.target.value)}
            className="mt-1"
          >
            <option value="connected">Connected</option>
            <option value="degraded">Degraded</option>
            <option value="offline">Offline</option>
          </Select>
        </div>

        {error && (
          <InlineBanner tone="danger" className="px-3 py-2 text-xs">
            {error}
          </InlineBanner>
        )}

        <NodeButton
          type="button"
          onClick={handleSave}
          disabled={busy}
          variant="primary"
        >
          {busy ? "Saving…" : "Save"}
        </NodeButton>
      </form>
    </CollapsibleCard>
  );
}
