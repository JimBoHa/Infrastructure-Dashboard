import { Card } from "@/components/ui/card";
import type { AlarmRule } from "@/features/alarms/types/alarmTypes";

export default function RuleHealthPanel({ rules }: { rules: AlarmRule[] }) {
  const errorRules = rules.filter((rule) => Boolean(rule.last_error));
  const activeRules = rules.filter((rule) => rule.active_count > 0);
  const disabledRules = rules.filter((rule) => !rule.enabled);

  return (
    <Card className="rounded-xl border border-border p-4">
      <h3 className="text-sm font-semibold text-card-foreground">Rule health</h3>
      <div className="mt-3 grid gap-3 md:grid-cols-3">
        <div>
          <p className="text-xs text-muted-foreground">Rules with errors</p>
          <p className="text-xl font-semibold text-red-700">{errorRules.length}</p>
        </div>
        <div>
          <p className="text-xs text-muted-foreground">Rules currently firing</p>
          <p className="text-xl font-semibold text-amber-700">{activeRules.length}</p>
        </div>
        <div>
          <p className="text-xs text-muted-foreground">Disabled rules</p>
          <p className="text-xl font-semibold text-muted-foreground">{disabledRules.length}</p>
        </div>
      </div>
    </Card>
  );
}
