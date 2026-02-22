"use client";

import { useMemo, useState } from "react";
import type { BackupRetentionConfig, DemoNode } from "@/types/dashboard";
import NodeButton from "@/features/nodes/components/NodeButton";
import { Input } from "@/components/ui/input";

interface RetentionPolicyTableProps {
  config: BackupRetentionConfig;
  nodes: DemoNode[];
  savingNodeId: string | null;
  onSubmit: (nodeId: string, keepDays: number | null) => void;
  disabled?: boolean;
}

export function RetentionPolicyTable({
  config,
  nodes,
  savingNodeId,
  onSubmit,
  disabled = false,
}: RetentionPolicyTableProps) {
  const policies = useMemo(() => config.policies ?? [], [config.policies]);
  const overrides = useMemo(
    () => new Map(policies.map((policy) => [policy.node_id, policy])),
    [policies],
  );

  const deriveInitialValues = () => {
    const result: Record<string, string> = {};
    nodes.forEach((node) => {
      const override = overrides.get(node.id);
      const keepDays = override?.keep_days ?? config.default_keep_days;
      result[node.id] = String(keepDays);
    });
    return result;
  };

  const [values, setValues] = useState<Record<string, string>>(deriveInitialValues);
  const [errors, setErrors] = useState<Record<string, string>>({});

  const clearError = (nodeId: string) => {
    setErrors((prev) => {
      if (!prev[nodeId]) {
        return prev;
      }
      const next = { ...prev };
      delete next[nodeId];
      return next;
    });
  };

  const handleChange = (nodeId: string, value: string) => {
    setValues((prev) => ({ ...prev, [nodeId]: value }));
    clearError(nodeId);
  };

  const handleSave = (nodeId: string) => {
    if (disabled) {
      return;
    }
    const raw = values[nodeId];
    const parsed = Number.parseInt(raw, 10);
    if (!Number.isFinite(parsed) || parsed < 1) {
      setErrors((prev) => ({
        ...prev,
        [nodeId]: "Enter a retention period of at least 1 day.",
      }));
      return;
    }
    clearError(nodeId);
    onSubmit(nodeId, parsed);
  };

  const handleReset = (nodeId: string) => {
    if (disabled) {
      return;
    }
    clearError(nodeId);
    onSubmit(nodeId, null);
  };

  if (!nodes.length) {
    return (
 <p className="px-3 py-4 text-sm text-muted-foreground">
        No nodes available for retention configuration.
      </p>
    );
  }

  return (
    <div className="mt-4 overflow-x-auto md:overflow-x-visible">
      <table className="min-w-full divide-y divide-border text-sm">
        <thead className="bg-card-inset">
          <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Node
            </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Keep days
            </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Source
            </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Actions
            </th>
          </tr>
        </thead>
        <tbody className="divide-y divide-border">
          {nodes.map((node) => {
            const override = overrides.get(node.id);
            const saving = savingNodeId === node.id;
            const error = errors[node.id];
            const readOnly = disabled && !saving;
            return (
              <tr key={node.id} className={saving ? "opacity-60" : undefined}>
                <td className="px-3 py-2 text-card-foreground">{node.name}</td>
                <td className="px-3 py-2 text-card-foreground">
                  <div className="flex items-center gap-2">
                    <Input
                      aria-label={`Retention days for ${node.name}`}
                      className="w-24 py-1.5"
                      min={1}
                      name={`keep-days-${node.id}`}
                      onChange={(event) => handleChange(node.id, event.target.value)}
                      type="number"
                      value={values[node.id] ?? ""}
                      disabled={saving || disabled}
                    />
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      days
                    </span>
                    {readOnly ? (
 <span className="text-xs font-semibold uppercase tracking-wide text-amber-600">
                        Read-only
                      </span>
                    ) : null}
                  </div>
                  {error && <p className="mt-1 text-xs text-rose-600">{error}</p>}
                </td>
 <td className="px-3 py-2 text-muted-foreground">
                  {override ? (
                    <span title="Custom retention period for this node.">
                      Override ({override.keep_days} days)
                    </span>
                  ) : (
                    <span title="Follows the default retention period.">
                      Default ({config.default_keep_days} days)
                    </span>
                  )}
                </td>
                <td className="px-3 py-2">
                  <div className="flex gap-2">
                    <NodeButton
                      disabled={saving || disabled}
                      onClick={() => handleSave(node.id)}
                      type="button"
                      size="xs"
                    >
                      Save
                    </NodeButton>
                    <NodeButton
                      disabled={saving || disabled || !override}
                      onClick={() => handleReset(node.id)}
                      title="Revert to the default retention period."
                      type="button"
                      size="xs"
                    >
                      Use default
                    </NodeButton>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
