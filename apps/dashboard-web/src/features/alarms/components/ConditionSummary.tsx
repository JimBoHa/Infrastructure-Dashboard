import type { AlarmRule, ConditionNode } from "@/features/alarms/types/alarmTypes";
import { describeCondition } from "@/features/alarms/utils/ruleSummary";

export default function ConditionSummary({
  condition,
  className,
}: {
  condition: ConditionNode | AlarmRule["condition_ast"];
  className?: string;
}) {
  return <p className={className ?? "text-xs text-muted-foreground"}>{describeCondition(condition)}</p>;
}
