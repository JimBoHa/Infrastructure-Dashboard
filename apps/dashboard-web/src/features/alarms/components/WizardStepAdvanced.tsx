import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import type { AlarmWizardState } from "@/features/alarms/types/alarmTypes";

export default function WizardStepAdvanced({
  state,
  onPatch,
}: {
  state: AlarmWizardState;
  onPatch: (partial: Partial<AlarmWizardState>) => void;
}) {
  return (
    <div className="space-y-4">
      <label className="flex items-center gap-2 text-sm text-card-foreground">
        <input
          type="checkbox"
          checked={state.advancedMode}
          onChange={(event) => onPatch({ advancedMode: event.target.checked })}
        />
        Edit raw JSON rule payload
      </label>

      {state.advancedMode ? (
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Rule JSON</label>
          <Textarea
            value={state.advancedJson}
            onChange={(event) => onPatch({ advancedJson: event.target.value })}
            rows={16}
            className="font-mono text-xs"
          />
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Debounce (sec)</label>
            <Input
              value={state.debounceSeconds}
              onChange={(event) => onPatch({ debounceSeconds: event.target.value })}
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Clear hysteresis (sec)</label>
            <Input
              value={state.clearHysteresisSeconds}
              onChange={(event) => onPatch({ clearHysteresisSeconds: event.target.value })}
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Eval interval (sec)</label>
            <Input
              value={state.evalIntervalSeconds}
              onChange={(event) => onPatch({ evalIntervalSeconds: event.target.value })}
            />
          </div>
        </div>
      )}
    </div>
  );
}
