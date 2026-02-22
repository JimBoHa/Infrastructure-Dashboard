import { useMemo } from "react";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import { Card } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import ScheduleConditionFormFields from "@/features/schedules/components/ScheduleConditionFormFields";
import {
  asString,
  conditionFormToPayload,
  conditionPayloadToForm,
  emptyConditionForm,
  hasFieldErrors,
  isBlank,
  parseJsonObjectArray,
  ScheduleDraft,
  Toast,
  validateConditionForm,
} from "@/features/schedules/lib/scheduleUtils";

export default function ScheduleConditionsEditor({
  draft,
  onChange,
  notify,
  sensors,
  nodes,
  showValidation,
}: {
  draft: ScheduleDraft;
  onChange: (draft: ScheduleDraft | null) => void;
  notify: (toast: Toast) => void;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  showValidation: boolean;
}) {
  const editingIndex = draft.editingConditionIndex;

  const nodeNameById = useMemo(
    () => new Map(nodes.map((node) => [node.id, node.name])),
    [nodes],
  );
  const sensorById = useMemo(
    () => new Map(sensors.map((sensor) => [sensor.sensor_id, sensor])),
    [sensors],
  );

  const sync = (
    conditionsList: Array<Record<string, unknown>>,
    patch: Partial<ScheduleDraft> = {},
  ) => {
    onChange({
      ...draft,
      ...patch,
      conditionsList,
      conditionsJson: JSON.stringify(conditionsList, null, 2),
    });
  };

  const addCondition = () => {
    const payload = conditionFormToPayload(emptyConditionForm());
    const nextIndex = draft.conditionsList.length;
    sync([...draft.conditionsList, payload], { editingConditionIndex: nextIndex });
    notify({ type: "success", text: "Added condition." });
  };

  const updateCondition = (idx: number, nextCondition: Record<string, unknown>) => {
    sync(draft.conditionsList.map((item, index) => (index === idx ? nextCondition : item)));
  };

  const removeCondition = (idx: number) => {
    const next = draft.conditionsList.filter((_, index) => index !== idx);
    const current = draft.editingConditionIndex;
    let nextEditing: number | null = current;
    if (current === idx) nextEditing = null;
    if (current != null && current > idx) nextEditing = current - 1;
    sync(next, { editingConditionIndex: nextEditing });
  };

  const applyJson = () => {
    try {
      const parsed = parseJsonObjectArray(draft.conditionsJson, "Conditions");
      sync(parsed, { conditionsMode: "form", editingConditionIndex: null });
      notify({ type: "success", text: "Applied conditions JSON." });
    } catch (error) {
      const text = error instanceof Error ? error.message : "Invalid conditions JSON.";
      notify({ type: "error", text });
    }
  };

  const switchToVisual = () => {
    if (draft.conditionsMode === "form") return;
    try {
      const parsed = parseJsonObjectArray(draft.conditionsJson, "Conditions");
      sync(parsed, { conditionsMode: "form", editingConditionIndex: null });
    } catch (error) {
      const text = error instanceof Error ? error.message : "Invalid conditions JSON.";
      notify({ type: "error", text });
    }
  };

  return (
    <Card className="space-y-2 rounded-lg gap-0 bg-card-inset p-3">
      <div className="flex items-center justify-between">
 <span className="font-semibold text-foreground">Conditions</span>
        <div className="flex items-center gap-2 text-[11px]">
          <button
            className={`rounded-full px-3 py-1 font-semibold ${
              draft.conditionsMode === "form"
                ? "bg-indigo-600 text-white"
 : "border border-border bg-white text-foreground hover:bg-muted"
            }`}
            onClick={switchToVisual}
          >
            Visual
          </button>
          <button
            className={`rounded-full px-3 py-1 font-semibold ${
              draft.conditionsMode === "json"
                ? "bg-indigo-600 text-white"
 : "border border-border bg-white text-foreground hover:bg-muted"
            }`}
            onClick={() => {
              onChange({ ...draft, conditionsMode: "json", editingConditionIndex: null });
            }}
          >
            Advanced JSON
          </button>
        </div>
      </div>

      {draft.conditionsMode === "json" ? (
        <div className="space-y-2">
          <Textarea
            value={draft.conditionsJson}
            onChange={(e) => onChange({ ...draft, conditionsJson: e.target.value })}
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
          {draft.conditionsList.length === 0 && (
 <p className="text-xs text-muted-foreground">
              No conditions. Add one below.
            </p>
          )}
          {draft.conditionsList.map((condition, idx) => {
            const formValue = conditionPayloadToForm(condition);
            const validationErrors = formValue ? validateConditionForm(formValue) : null;
            const showErrors = Boolean(
              showValidation &&
                validationErrors &&
                hasFieldErrors(validationErrors as Record<string, string | undefined>),
            );

            const describeSensor = (sensorId: string | undefined) => {
              if (isBlank(sensorId)) return "Select sensor...";
              const sensor = sensorById.get(sensorId ?? "");
              if (!sensor) return `Unknown sensor (${sensorId})`;
              const nodeName = nodeNameById.get(sensor.node_id);
              return nodeName ? `${sensor.name} / ${nodeName}` : sensor.name;
            };

            const summary = (() => {
              if (!formValue) return "";
              switch (formValue.type) {
                case "sensor":
                  return `${describeSensor(formValue.sensor_id)} ${
                    formValue.operator ?? ""
                  } ${formValue.threshold ?? ""}`.trim();
                case "sensor_value_between": {
                  const min = formValue.min == null ? "-inf" : String(formValue.min);
                  const max = formValue.max == null ? "inf" : String(formValue.max);
                  return `${describeSensor(formValue.sensor_id)} between ${min} and ${max}`;
                }
                case "node_status": {
                  const nodeName = isBlank(formValue.node_id)
                    ? "Select node..."
                    : nodeNameById.get(formValue.node_id ?? "") ??
                      `Unknown node (${formValue.node_id})`;
                  return `${nodeName} is ${formValue.status ?? ""}`.trim();
                }
                case "forecast":
                  return `${formValue.field ?? ""} ${formValue.operator ?? ""} ${
                    formValue.threshold ?? ""
                  } (next ${formValue.horizon_hours ?? ""}h)`.trim();
                case "analytics": {
                  const window = formValue.window_minutes
                    ? ` over ${formValue.window_minutes}m`
                    : "";
                  return `${formValue.key ?? ""} ${formValue.operator ?? ""} ${
                    formValue.threshold ?? ""
                  }${window}`.trim();
                }
                default:
                  return asString(condition.type) ?? "Condition";
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
                        editingConditionIndex:
                          draft.editingConditionIndex === idx ? null : idx,
                      })
                    }
                  >
 <p className="text-xs font-semibold text-foreground">
                      Condition {idx + 1}
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
                        Unsupported condition shape ({asString(condition.type) ?? "unknown"}
                        ). Use Advanced JSON.
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
                          onChange({
                            ...draft,
                            conditionsMode: "json",
                            editingConditionIndex: null,
                          })
                        }
                        type="button"
                      >
                        Open JSON
                      </button>
                    )}
                    <button
 className="rounded-lg border border-border bg-white px-2.5 py-1.5 text-[11px] font-semibold text-foreground hover:bg-muted"
                      onClick={() => removeCondition(idx)}
                      type="button"
                    >
                      Remove
                    </button>
                  </div>
                </div>

                {formValue && editingIndex === idx && (
                  <div className="mt-3">
                    <ScheduleConditionFormFields
                      value={formValue}
                      onChange={(next) => updateCondition(idx, conditionFormToPayload(next))}
                      sensors={sensors}
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
              onClick={addCondition}
              type="button"
            >
              Add condition
            </button>
          </div>
        </div>
      )}
    </Card>
  );
}
