"use client";

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import { updateBackupRetentionPolicies } from "@/lib/api";
import { queryKeys, useBackupRetentionConfigQuery } from "@/lib/queries";
import { RetentionPolicyTable } from "@/components/backups/RetentionPolicyTable";
import CollapsibleCard from "@/components/CollapsibleCard";
import type { DemoNode } from "@/types/dashboard";

export default function RetentionSection({
  nodes,
  onNotify,
  canEdit,
}: {
  nodes: DemoNode[];
  onNotify: (message: { type: "success" | "error"; text: string }) => void;
  canEdit: boolean;
}) {
  const [savingNodeId, setSavingNodeId] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const {
    data,
    error,
    isLoading,
  } = useBackupRetentionConfigQuery();

  const handleSubmit = async (nodeId: string, keepDays: number | null) => {
    setSaveError(null);
    setSavingNodeId(nodeId);
    if (!canEdit) {
      const text = "Insufficient permissions: you need config.write to update retention policies.";
      setSaveError(text);
      onNotify({ type: "error", text });
      setSavingNodeId(null);
      return;
    }
    try {
      const updated = await updateBackupRetentionPolicies([
        { node_id: nodeId, keep_days: keepDays },
      ]);
      queryClient.setQueryData(queryKeys.backupRetention, updated);
      const nodeName = nodes.find((node) => node.id === nodeId)?.name ?? nodeId;
      const text =
        keepDays === null
          ? `Retention for ${nodeName} reverted to the default policy.`
          : `Retention for ${nodeName} set to ${keepDays} day(s).`;
      onNotify({ type: "success", text });
    } catch (err) {
      const raw = err instanceof Error ? err.message : "Failed to update retention.";
      const text = raw.includes("(403)")
        ? "Insufficient permissions: you need config.write to update retention policies."
        : raw;
      setSaveError(text);
      onNotify({ type: "error", text });
    } finally {
      setSavingNodeId(null);
    }
  };

  const lastCleanupDisplay = (() => {
    if (!data?.last_cleanup_at) {
      return null;
    }
    try {
      const timestamp = new Date(data.last_cleanup_at);
      if (Number.isNaN(timestamp.getTime())) {
        return null;
      }
      return `${formatDistanceToNow(timestamp, { addSuffix: true })} (${timestamp.toLocaleString()})`;
    } catch {
      return null;
    }
  })();

  const retentionKey = data
    ? `${data.default_keep_days}|${[...(data.policies ?? [])]
        .map((policy) => `${policy.node_id}:${policy.keep_days}`)
        .sort()
        .join(",")}`
    : "retention-empty";

  return (
    <CollapsibleCard
      title="Retention policies"
      description={
        <>
          Define how long configuration backups are kept per node.
          <span
            aria-label="Retention policy help"
 className="ml-2 inline-flex h-5 w-5 items-center justify-center rounded-full bg-muted text-xs font-semibold text-muted-foreground"
            title="Set a custom retention period per node. Use default to fall back to the global policy."
          >
            ?
          </span>
        </>
      }
      defaultOpen={false}
      bodyClassName="space-y-4"
    >
      {data ? (
 <p className="text-xs text-muted-foreground">
          Default retention{" "}
 <span className="font-semibold text-foreground">
            {data.default_keep_days} day{data.default_keep_days === 1 ? "" : "s"}
          </span>
          .
          {lastCleanupDisplay ? (
 <span className="ml-2 text-muted-foreground">Last cleanup {lastCleanupDisplay}.</span>
          ) : (
 <span className="ml-2 text-muted-foreground">Cleanup runs nightly to enforce retention.</span>
          )}
        </p>
      ) : null}

      {error ? (
        <p className="text-sm text-rose-600">
          Failed to load retention policies: {error instanceof Error ? error.message : "Unknown error"}
        </p>
      ) : null}

      {saveError ? <p className="text-sm text-rose-600">Retention update failed: {saveError}</p> : null}

      {isLoading && !data ? (
 <p className="text-sm text-muted-foreground">Loading retention policies...</p>
      ) : data ? (
        <RetentionPolicyTable
          key={retentionKey}
          config={data}
          nodes={nodes}
          onSubmit={handleSubmit}
          savingNodeId={savingNodeId}
          disabled={!canEdit}
        />
      ) : (
 <p className="text-sm text-muted-foreground">
          Retention information is unavailable. Try again later.
        </p>
      )}
    </CollapsibleCard>
  );
}
