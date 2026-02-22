import { formatDistanceToNow } from "date-fns";
import type { DemoSchedule } from "@/types/dashboard";
import { Card } from "@/components/ui/card";
import NodeButton from "@/features/nodes/components/NodeButton";

export default function ScheduleCard({
  schedule,
  onEdit,
}: {
  schedule: DemoSchedule;
  onEdit: () => void;
}) {
  const blocks = schedule.blocks ?? [];
  const conditions = schedule.conditions ?? [];
  const actions = schedule.actions ?? [];

  return (
    <Card className="gap-0 p-4">
      <div className="flex items-center justify-between">
        <div>
 <p className="text-sm font-semibold text-foreground">
            {schedule.name}
          </p>
 <p className="text-xs text-muted-foreground">{schedule.rrule}</p>
        </div>
        <div className="flex items-center gap-2">
 <span className="text-xs text-muted-foreground">
            Next {schedule.next_run ? formatDistanceToNow(new Date(schedule.next_run), { addSuffix: true }) : "tbd"}
          </span>
          <NodeButton size="xs" onClick={onEdit}>
            Edit
          </NodeButton>
        </div>
      </div>
 <div className="mt-3 text-xs text-muted-foreground">
 <p className="font-semibold text-muted-foreground">Blocks</p>
        <ul className="mt-1 space-y-1">
 {blocks.length === 0 && <li className="text-muted-foreground">None</li>}
          {blocks.map((block, idx) => (
            <li key={idx}>
              {block.day} / {block.start} - {block.end}
            </li>
          ))}
        </ul>
      </div>
 <div className="mt-3 text-xs text-muted-foreground">
 <p className="font-semibold text-muted-foreground">Conditions</p>
        <ul className="mt-1 flex flex-wrap gap-1">
 {conditions.length === 0 && <li className="text-muted-foreground">None</li>}
          {conditions.map((condition, idx) => (
 <li key={idx} className="rounded-full bg-muted px-2 py-1">
              {typeof condition === "object" && condition !== null
                ? String((condition as Record<string, unknown>).type ?? "condition")
                : String(condition)}
            </li>
          ))}
        </ul>
      </div>
 <div className="mt-3 text-xs text-muted-foreground">
 <p className="font-semibold text-muted-foreground">Actions</p>
        <ul className="mt-1 flex flex-wrap gap-1">
 {actions.length === 0 && <li className="text-muted-foreground">None</li>}
          {actions.map((action, idx) => (
 <li key={idx} className="rounded-full bg-muted px-2 py-1">
              {typeof action === "object" && action !== null
                ? String((action as Record<string, unknown>).type ?? "action")
                : String(action)}
            </li>
          ))}
        </ul>
      </div>
    </Card>
  );
}
