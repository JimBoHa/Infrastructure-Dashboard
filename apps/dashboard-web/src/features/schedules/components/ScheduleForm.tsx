"use client";

import type { DemoNode, DemoOutput, DemoSensor } from "@/types/dashboard";
import {
  ScheduleDraft,
  Toast,
} from "@/features/schedules/lib/scheduleUtils";
import { useScheduleForm } from "@/features/schedules/hooks/useScheduleForm";
import ScheduleBasicsFields from "@/features/schedules/components/ScheduleBasicsFields";
import ScheduleConditionsEditor from "@/features/schedules/components/ScheduleConditionsEditor";
import ScheduleActionsEditor from "@/features/schedules/components/ScheduleActionsEditor";
import NodeButton from "@/features/nodes/components/NodeButton";

export default function ScheduleForm({
  draft,
  nodes,
  sensors,
  outputs,
  onChange,
  onCancel,
  onSave,
  saving,
  notify,
}: {
  draft: ScheduleDraft;
  nodes: DemoNode[];
  sensors: DemoSensor[];
  outputs: DemoOutput[];
  onChange: (draft: ScheduleDraft | null) => void;
  onCancel: () => void;
  onSave: () => void;
  saving: boolean;
  notify: (toast: Toast) => void;
}) {
  const { patchDraft, showValidation, timingErrors, handleSave } = useScheduleForm({
    draft,
    onChange,
    onSave,
    notify,
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
 <h3 className="text-lg font-semibold text-foreground">
            {draft.mode === "create" ? "Create schedule" : "Edit schedule"}
          </h3>
 <p className="text-sm text-muted-foreground">
            Set the day, start/end, conditions, and actions. Use Advanced JSON for power-user edits. Saving writes directly to /api/schedules.
          </p>
        </div>
        <NodeButton size="xs" onClick={onCancel}>
          Cancel
        </NodeButton>
      </div>

      <ScheduleBasicsFields
        draft={draft}
        patchDraft={patchDraft}
        showValidation={showValidation}
        timingErrors={timingErrors}
      />

      <div className="grid gap-4 md:grid-cols-2">
        <ScheduleConditionsEditor
          draft={draft}
          onChange={onChange}
          notify={notify}
          sensors={sensors}
          nodes={nodes}
          showValidation={showValidation}
        />
        <ScheduleActionsEditor
          draft={draft}
          onChange={onChange}
          notify={notify}
          outputs={outputs}
          nodes={nodes}
          showValidation={showValidation}
        />
      </div>

      <div className="flex justify-end gap-2">
        <NodeButton onClick={onCancel} type="button">
          Discard
        </NodeButton>
        <NodeButton variant="primary" onClick={handleSave} disabled={saving} type="button">
          {saving ? "Saving..." : "Save"}
        </NodeButton>
      </div>
    </div>
  );
}
