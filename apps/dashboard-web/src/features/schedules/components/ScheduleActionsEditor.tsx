import { useMemo } from "react";
import type { DemoNode, DemoOutput } from "@/types/dashboard";
import { Card } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import ScheduleActionFormFields from "@/features/schedules/components/ScheduleActionFormFields";
import {
  actionFormToPayload,
  actionPayloadToForm,
  asString,
  emptyActionForm,
  hasFieldErrors,
  isBlank,
  parseJsonObjectArray,
  ScheduleDraft,
  Toast,
  validateActionForm,
} from "@/features/schedules/lib/scheduleUtils";

export default function ScheduleActionsEditor({
  draft,
  onChange,
  notify,
  outputs,
  nodes,
  showValidation,
}: {
  draft: ScheduleDraft;
  onChange: (draft: ScheduleDraft | null) => void;
  notify: (toast: Toast) => void;
  outputs: DemoOutput[];
  nodes: DemoNode[];
  showValidation: boolean;
}) {
  const editingIndex = draft.editingActionIndex;

  const nodeNameById = useMemo(
    () => new Map(nodes.map((node) => [node.id, node.name])),
    [nodes],
  );
  const outputById = useMemo(
    () => new Map(outputs.map((output) => [output.id, output])),
    [outputs],
  );

  const sync = (
    actionsList: Array<Record<string, unknown>>,
    patch: Partial<ScheduleDraft> = {},
  ) => {
    onChange({
      ...draft,
      ...patch,
      actionsList,
      actionsJson: JSON.stringify(actionsList, null, 2),
    });
  };

  const addAction = () => {
    const payload = actionFormToPayload(emptyActionForm());
    const nextIndex = draft.actionsList.length;
    sync([...draft.actionsList, payload], { editingActionIndex: nextIndex });
    notify({ type: "success", text: "Added action." });
  };

  const updateAction = (idx: number, nextAction: Record<string, unknown>) => {
    sync(draft.actionsList.map((item, index) => (index === idx ? nextAction : item)));
  };

  const removeAction = (idx: number) => {
    const next = draft.actionsList.filter((_, index) => index !== idx);
    const current = draft.editingActionIndex;
    let nextEditing: number | null = current;
    if (current === idx) nextEditing = null;
    if (current != null && current > idx) nextEditing = current - 1;
    sync(next, { editingActionIndex: nextEditing });
  };

  const applyJson = () => {
    try {
      const parsed = parseJsonObjectArray(draft.actionsJson, "Actions");
      sync(parsed, { actionsMode: "form", editingActionIndex: null });
      notify({ type: "success", text: "Applied actions JSON." });
    } catch (error) {
      const text = error instanceof Error ? error.message : "Invalid actions JSON.";
      notify({ type: "error", text });
    }
  };

  const switchToVisual = () => {
    if (draft.actionsMode === "form") return;
    try {
      const parsed = parseJsonObjectArray(draft.actionsJson, "Actions");
      sync(parsed, { actionsMode: "form", editingActionIndex: null });
    } catch (error) {
      const text = error instanceof Error ? error.message : "Invalid actions JSON.";
      notify({ type: "error", text });
    }
  };

  return (
    <Card className="space-y-2 rounded-lg gap-0 bg-card-inset p-3">
      <div className="flex items-center justify-between">
 <span className="font-semibold text-foreground">Actions</span>
        <div className="flex items-center gap-2 text-[11px]">
          <button
            className={`rounded-full px-3 py-1 font-semibold ${
              draft.actionsMode === "form"
                ? "bg-indigo-600 text-white"
 : "border border-border bg-white text-foreground hover:bg-muted"
            }`}
            onClick={switchToVisual}
          >
            Visual
          </button>
          <button
            className={`rounded-full px-3 py-1 font-semibold ${
              draft.actionsMode === "json"
                ? "bg-indigo-600 text-white"
 : "border border-border bg-white text-foreground hover:bg-muted"
            }`}
            onClick={() => {
              onChange({ ...draft, actionsMode: "json", editingActionIndex: null });
            }}
          >
            Advanced JSON
          </button>
        </div>
      </div>

      {draft.actionsMode === "json" ? (
        <div className="space-y-2">
          <Textarea
            value={draft.actionsJson}
            onChange={(e) => onChange({ ...draft, actionsJson: e.target.value })}
            rows={5}
          />
          <div className="flex justify-end">
            <button
              className="rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-indigo-700 focus:outline-hidden focus:bg-indigo-700"
              onClick={applyJson}
              type="button"
            >
              Apply JSON
            </button>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          {draft.actionsList.length === 0 && (
 <p className="text-xs text-muted-foreground">No actions. Add one below.</p>
          )}
          {draft.actionsList.map((action, idx) => {
            const formValue = actionPayloadToForm(action);
            const validationErrors = formValue ? validateActionForm(formValue) : null;
            const showErrors = Boolean(
              showValidation &&
                validationErrors &&
                hasFieldErrors(validationErrors as Record<string, string | undefined>),
            );

            const describeOutput = (outputId: string | undefined) => {
              if (isBlank(outputId)) return "Select output...";
              const output = outputById.get(outputId ?? "");
              if (!output) return `Unknown output (${outputId})`;
              const nodeName = nodeNameById.get(output.node_id);
              return nodeName ? `${output.name} / ${nodeName}` : output.name;
            };

            const summary = (() => {
              if (!formValue) return "";
              switch (formValue.type) {
                case "output":
                  return `${describeOutput(formValue.output_id)} -> ${
                    formValue.state ?? ""
                  }`.trim();
                case "alarm":
                  return `${formValue.severity ?? ""}: ${formValue.message ?? ""}`.trim();
                case "mqtt_publish":
                  return `${formValue.topic ?? ""}`.trim();
                default:
                  return asString(action.type) ?? "Action";
              }
            })();

            return (
              <Card
                key={idx}
                className={`gap-0 rounded-lg p-3 ${
                  showErrors
 ? "border-rose-200 bg-rose-50/40"
                    : ""
                }`}
              >
                <div className="flex items-start justify-between gap-3">
                  <button
                    className="flex-1 text-left"
                    type="button"
                    onClick={() =>
                      onChange({
                        ...draft,
                        editingActionIndex: draft.editingActionIndex === idx ? null : idx,
                      })
                    }
                  >
 <p className="text-xs font-semibold text-foreground">
                      Action {idx + 1}
                    </p>
                    {formValue ? (
                      <p
                        className={`mt-1 text-xs ${
                          showErrors
 ? "text-rose-700"
 : "text-muted-foreground"
                        }`}
                      >
                        {summary || "Click to edit..."}
                      </p>
                    ) : (
 <p className="mt-1 text-xs text-muted-foreground">
                        Unsupported action shape ({asString(action.type) ?? "unknown"}).
                        Use Advanced JSON.
                      </p>
                    )}
                    {showErrors && (
                      <p className="mt-1 text-[10px] font-semibold text-rose-600">
                        Missing required fields
                      </p>
                    )}
                  </button>

                  <div className="flex shrink-0 items-center gap-2">
                    {formValue == null && (
                      <button
 className="rounded-lg border border-border bg-white px-2.5 py-1.5 text-[11px] font-semibold text-foreground hover:bg-muted"
                        onClick={() =>
                          onChange({ ...draft, actionsMode: "json", editingActionIndex: null })
                        }
                        type="button"
                      >
                        Open JSON
                      </button>
                    )}
                    <button
 className="rounded-lg border border-border bg-white px-2.5 py-1.5 text-[11px] font-semibold text-foreground hover:bg-muted"
                      onClick={() => removeAction(idx)}
                      type="button"
                    >
                      Remove
                    </button>
                  </div>
                </div>

                {formValue && editingIndex === idx && (
                  <div className="mt-3">
                    <ScheduleActionFormFields
                      value={formValue}
                      onChange={(next) => updateAction(idx, actionFormToPayload(next))}
                      outputs={outputs}
                      nodes={nodes}
                      errors={validationErrors ?? undefined}
                      showErrors={showValidation}
                    />
                  </div>
                )}
              </Card>
            );
          })}

          <div className="flex justify-end">
            <button
              className="rounded-lg bg-indigo-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-indigo-700 focus:outline-hidden focus:bg-indigo-700"
              onClick={addAction}
              type="button"
            >
              Add action
            </button>
          </div>
        </div>
      )}
    </Card>
  );
}
