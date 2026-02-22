import {
  DAY_CODES,
  DayCode,
  ScheduleDraft,
} from "@/features/schedules/lib/scheduleUtils";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";

export default function ScheduleBasicsFields({
  draft,
  patchDraft,
  showValidation,
  timingErrors,
}: {
  draft: ScheduleDraft;
  patchDraft: (patch: Partial<ScheduleDraft>) => void;
  showValidation: boolean;
  timingErrors: { start?: string; end?: string };
}) {
  return (
    <>
      <div className="grid gap-4 md:grid-cols-2">
 <label className="space-y-1 text-sm text-muted-foreground">
 <span className="font-semibold text-foreground">Name</span>
          <Input
            value={draft.name}
            onChange={(e) => patchDraft({ name: e.target.value })}
          />
        </label>
 <label className="space-y-1 text-sm text-muted-foreground">
 <span className="font-semibold text-foreground">
            Recurrence (RRULE)
          </span>
          <Input
            value={draft.rrule}
            onChange={(e) => patchDraft({ rrule: e.target.value })}
          />
        </label>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
 <label className="space-y-1 text-sm text-muted-foreground">
 <span className="font-semibold text-foreground">Day</span>
          <Select
            value={draft.day}
            onChange={(e) => patchDraft({ day: e.target.value as DayCode })}
          >
            {DAY_CODES.map((code) => (
              <option key={code} value={code}>
                {code}
              </option>
            ))}
          </Select>
        </label>
 <label className="space-y-1 text-sm text-muted-foreground">
 <span className="font-semibold text-foreground">Start</span>
          <Input
            type="time"
            value={draft.start}
            onChange={(e) => patchDraft({ start: e.target.value })}
            className={
              showValidation && timingErrors.start
 ? "border-rose-400 bg-rose-50 focus:border-rose-500"
                : ""
            }
          />
          {showValidation && timingErrors.start && (
            <p className="text-xs text-rose-600">{timingErrors.start}</p>
          )}
        </label>
 <label className="space-y-1 text-sm text-muted-foreground">
 <span className="font-semibold text-foreground">End</span>
          <Input
            type="time"
            value={draft.end}
            onChange={(e) => patchDraft({ end: e.target.value })}
            className={
              showValidation && timingErrors.end
 ? "border-rose-400 bg-rose-50 focus:border-rose-500"
                : ""
            }
          />
          {showValidation && timingErrors.end && (
            <p className="text-xs text-rose-600">{timingErrors.end}</p>
          )}
        </label>
      </div>
    </>
  );
}
