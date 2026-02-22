import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import type { AlarmWizardState } from "@/features/alarms/types/alarmTypes";

export default function WizardStepBasics({
  state,
  onPatch,
}: {
  state: AlarmWizardState;
  onPatch: (partial: Partial<AlarmWizardState>) => void;
}) {
  return (
    <div className="space-y-4">
      <div>
        <label className="text-xs font-semibold text-muted-foreground">Alarm name</label>
        <Input
          value={state.name}
          onChange={(event) => onPatch({ name: event.target.value })}
          placeholder="Well pump pressure low"
        />
      </div>
      <div>
        <label className="text-xs font-semibold text-muted-foreground">Description</label>
        <Textarea
          value={state.description}
          onChange={(event) => onPatch({ description: event.target.value })}
          placeholder="Optional operator notes"
          rows={3}
        />
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Severity</label>
          <Select
            value={state.severity}
            onChange={(event) =>
              onPatch({ severity: event.target.value as AlarmWizardState["severity"] })
            }
          >
            <option value="info">Info</option>
            <option value="warning">Warning</option>
            <option value="critical">Critical</option>
          </Select>
        </div>
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Origin</label>
          <Input
            value={state.origin}
            onChange={(event) => onPatch({ origin: event.target.value })}
            placeholder="threshold"
          />
        </div>
      </div>
      <div>
        <label className="text-xs font-semibold text-muted-foreground">Event message</label>
        <Input
          value={state.messageTemplate}
          onChange={(event) => onPatch({ messageTemplate: event.target.value })}
          placeholder="Leave blank to auto-generate"
        />
      </div>
    </div>
  );
}
