import { Card } from "@/components/ui/card";
import NodeButton from "@/features/nodes/components/NodeButton";
import { Badge } from "@/components/ui/badge";
import type { AlarmRule } from "@/features/alarms/types/alarmTypes";
import ConditionSummary from "@/features/alarms/components/ConditionSummary";

const severityTone = (severity: string): "info" | "warning" | "danger" => {
  if (severity === "critical") return "danger";
  if (severity === "warning") return "warning";
  return "info";
};

export default function AlarmCard({
  rule,
  canEdit,
  onEdit,
  onDuplicate,
  onToggle,
  onDelete,
}: {
  rule: AlarmRule;
  canEdit: boolean;
  onEdit: (rule: AlarmRule) => void;
  onDuplicate?: (rule: AlarmRule) => void;
  onToggle: (rule: AlarmRule) => void;
  onDelete: (rule: AlarmRule) => void;
}) {
  return (
    <Card className="rounded-xl border border-border p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-sm font-semibold text-card-foreground">{rule.name}</p>
          {rule.description ? (
            <p className="mt-1 text-xs text-muted-foreground">{rule.description}</p>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          <Badge tone={severityTone(rule.severity)}>{rule.severity}</Badge>
          <Badge tone={rule.enabled ? "success" : "muted"}>{rule.enabled ? "enabled" : "disabled"}</Badge>
        </div>
      </div>
      <ConditionSummary condition={rule.condition_ast} className="mt-2 text-xs text-muted-foreground" />
      <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <span>Active: {rule.active_count}</span>
        <span>•</span>
        <span>Origin: {rule.origin}</span>
        {rule.last_eval_at ? (
          <>
            <span>•</span>
            <span>Last eval: {new Date(rule.last_eval_at).toLocaleString()}</span>
          </>
        ) : null}
      </div>
      {rule.last_error ? (
        <p className="mt-2 text-xs text-red-700">Last evaluator error: {rule.last_error}</p>
      ) : null}
      <div className="mt-4 flex flex-wrap items-center gap-2">
        <NodeButton size="sm" onClick={() => onEdit(rule)} disabled={!canEdit}>
          Edit
        </NodeButton>
        {onDuplicate ? (
          <NodeButton size="sm" onClick={() => onDuplicate(rule)} disabled={!canEdit}>
            Duplicate
          </NodeButton>
        ) : null}
        <NodeButton size="sm" onClick={() => onToggle(rule)} disabled={!canEdit}>
          {rule.enabled ? "Disable" : "Enable"}
        </NodeButton>
        <NodeButton size="sm" onClick={() => onDelete(rule)} disabled={!canEdit}>
          Delete
        </NodeButton>
      </div>
    </Card>
  );
}
