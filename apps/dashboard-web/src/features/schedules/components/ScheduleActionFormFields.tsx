import type { DemoNode, DemoOutput } from "@/types/dashboard";
import type {
  ActionFieldErrors,
  ActionFormState,
} from "@/features/schedules/lib/scheduleUtils";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";

export default function ScheduleActionFormFields({
  value,
  onChange,
  outputs,
  nodes,
  errors,
  showErrors,
}: {
  value: ActionFormState;
  onChange: (value: ActionFormState) => void;
  outputs: DemoOutput[];
  nodes: DemoNode[];
  errors?: ActionFieldErrors;
  showErrors: boolean;
}) {
  const selectedOutput =
    value.type === "output"
      ? outputs.find((output) => output.id === value.output_id)
      : undefined;
  const nodeName = selectedOutput
    ? nodes.find((node) => node.id === selectedOutput.node_id)?.name ?? ""
    : "";

  return (
 <div className="grid gap-3 text-xs text-muted-foreground">
      <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
          Type
        </label>
        <Select
          value={value.type}
          onChange={(e) =>
            onChange({ ...value, type: e.target.value as ActionFormState["type"] })
          }
          className="mt-1 px-2.5 py-1.5 text-xs"
        >
          <option value="output">Output state</option>
          <option value="alarm">Raise alarm</option>
          <option value="mqtt_publish">MQTT publish</option>
        </Select>
        {showErrors && errors?.type && (
          <p className="text-[10px] text-rose-600">{errors.type}</p>
        )}
      </div>

      {value.type === "output" && (
        <div className="grid gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Output
            </label>
            <Select
              value={value.output_id ?? ""}
              onChange={(e) => onChange({ ...value, output_id: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.output_id
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
            >
              <option value="">Select output...</option>
              {outputs.map((output) => (
                <option key={output.id} value={output.id}>
                  {output.name} ({output.type})
                </option>
              ))}
            </Select>
            {showErrors && errors?.output_id && (
              <p className="text-[10px] text-rose-600">{errors.output_id}</p>
            )}
            {selectedOutput && (
 <p className="mt-1 text-[10px] text-muted-foreground">
                {nodeName ? `${nodeName} / ` : ""}Supported states:{" "}
                {selectedOutput.supported_states?.join(", ") ?? "n/a"}
              </p>
            )}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                State
              </label>
              <Select
                value={value.state ?? ""}
                onChange={(e) => onChange({ ...value, state: e.target.value })}
                className={`mt-1 rounded px-2 py-1 text-xs ${
                  showErrors && errors?.state
 ? "border-rose-400 bg-rose-50"
                    : ""
                }`}
              >
                {selectedOutput?.supported_states?.length
                  ? selectedOutput.supported_states.map((state) => (
                      <option key={state} value={state}>
                        {state}
                      </option>
                    ))
                  : ["on", "off"].map((state) => (
                      <option key={state} value={state}>
                        {state}
                      </option>
                    ))}
              </Select>
              {showErrors && errors?.state && (
                <p className="text-[10px] text-rose-600">{errors.state}</p>
              )}
            </div>
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Duration (sec)
              </label>
              <NumericDraftInput
                value={value.duration_seconds}
                onValueChange={(next) =>
                  onChange({
                    ...value,
                    duration_seconds: typeof next === "number" ? next : undefined,
                  })
                }
                className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                  showErrors && errors?.duration_seconds
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
                }`}
                placeholder="optional"
                inputMode="numeric"
                integer
              />
              {showErrors && errors?.duration_seconds && (
                <p className="text-[10px] text-rose-600">
                  {errors.duration_seconds}
                </p>
              )}
            </div>
          </div>
        </div>
      )}

      {value.type === "alarm" && (
        <div className="grid gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Severity
            </label>
            <Select
              value={value.severity ?? ""}
              onChange={(e) => onChange({ ...value, severity: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.severity
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
            >
              {"warning critical".split(" ").map((severity) => (
                <option key={severity} value={severity}>
                  {severity}
                </option>
              ))}
            </Select>
            {showErrors && errors?.severity && (
              <p className="text-[10px] text-rose-600">{errors.severity}</p>
            )}
          </div>
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Message
            </label>
            <Input
              value={value.message ?? ""}
              onChange={(e) => onChange({ ...value, message: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.message
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
            />
            {showErrors && errors?.message && (
              <p className="text-[10px] text-rose-600">{errors.message}</p>
            )}
          </div>
        </div>
      )}

      {value.type === "mqtt_publish" && (
        <div className="grid gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Topic
            </label>
            <Input
              value={value.topic ?? ""}
              onChange={(e) => onChange({ ...value, topic: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.topic
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
            />
            {showErrors && errors?.topic && (
              <p className="text-[10px] text-rose-600">{errors.topic}</p>
            )}
          </div>
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Payload
            </label>
            <Textarea
              value={value.payload ?? ""}
              onChange={(e) => onChange({ ...value, payload: e.target.value })}
              rows={3}
              className="mt-1 px-2.5 py-1.5 text-xs"
            />
          </div>
        </div>
      )}
    </div>
  );
}
