import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type {
  ConditionFieldErrors,
  ConditionFormState,
} from "@/features/schedules/lib/scheduleUtils";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";

export default function ScheduleConditionFormFields({
  value,
  onChange,
  sensors,
  nodes,
  errors,
  showErrors,
}: {
  value: ConditionFormState;
  onChange: (value: ConditionFormState) => void;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  errors?: ConditionFieldErrors;
  showErrors: boolean;
}) {
  return (
 <div className="grid gap-3 text-xs text-muted-foreground">
      <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
          Type
        </label>
        <Select
          value={value.type}
          onChange={(e) =>
            onChange({ ...value, type: e.target.value as ConditionFormState["type"] })
          }
          className="mt-1 px-2.5 py-1.5 text-xs"
        >
          <option value="sensor">Sensor threshold</option>
          <option value="sensor_value_between">Sensor between</option>
          <option value="node_status">Node status</option>
          <option value="forecast">Forecast</option>
          <option value="analytics">Analytics</option>
        </Select>
        {showErrors && errors?.type && (
          <p className="text-[10px] text-rose-600">{errors.type}</p>
        )}
      </div>

      {(value.type === "sensor" || value.type === "sensor_value_between") && (
        <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            Sensor
          </label>
          <Select
            value={value.sensor_id ?? ""}
            onChange={(e) => onChange({ ...value, sensor_id: e.target.value })}
            className={`mt-1 rounded px-2 py-1 text-xs ${
              showErrors && errors?.sensor_id
 ? "border-rose-400 bg-rose-50"
                : ""
            }`}
          >
            <option value="">Select sensor...</option>
            {sensors.map((sensor) => (
              <option key={sensor.sensor_id} value={sensor.sensor_id}>
                {sensor.name} ({sensor.type})
              </option>
            ))}
          </Select>
          {showErrors && errors?.sensor_id && (
            <p className="text-[10px] text-rose-600">{errors.sensor_id}</p>
          )}
        </div>
      )}

      {value.type === "sensor" && (
        <div className="grid grid-cols-2 gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Operator
            </label>
            <Select
              value={value.operator ?? ""}
              onChange={(e) => onChange({ ...value, operator: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.operator
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
            >
              {"< <= > >= ==".split(" ").map((op) => (
                <option key={op} value={op}>
                  {op}
                </option>
              ))}
            </Select>
            {showErrors && errors?.operator && (
              <p className="text-[10px] text-rose-600">{errors.operator}</p>
            )}
          </div>
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Threshold
            </label>
            <NumericDraftInput
              value={value.threshold}
              onValueChange={(next) =>
                onChange({ ...value, threshold: typeof next === "number" ? next : undefined })
              }
              className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                showErrors && errors?.threshold
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
              }`}
              inputMode="decimal"
            />
            {showErrors && errors?.threshold && (
              <p className="text-[10px] text-rose-600">{errors.threshold}</p>
            )}
          </div>
        </div>
      )}

      {value.type === "sensor_value_between" && (
        <div className="grid grid-cols-2 gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Min
            </label>
            <NumericDraftInput
              value={value.min}
              onValueChange={(next) =>
                onChange({ ...value, min: typeof next === "number" ? next : undefined })
              }
              className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                showErrors && errors?.min
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
              }`}
              inputMode="decimal"
            />
            {showErrors && errors?.min && <p className="text-[10px] text-rose-600">{errors.min}</p>}
          </div>
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Max
            </label>
            <NumericDraftInput
              value={value.max}
              onValueChange={(next) =>
                onChange({ ...value, max: typeof next === "number" ? next : undefined })
              }
              className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                showErrors && errors?.max
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
              }`}
              inputMode="decimal"
            />
            {showErrors && errors?.max && <p className="text-[10px] text-rose-600">{errors.max}</p>}
          </div>
        </div>
      )}

      {value.type === "node_status" && (
        <div className="grid grid-cols-2 gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Node
            </label>
            <Select
              value={value.node_id ?? ""}
              onChange={(e) => onChange({ ...value, node_id: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.node_id
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
            >
              <option value="">Select node...</option>
              {nodes.map((node) => (
                <option key={node.id} value={node.id}>
                  {node.name}
                </option>
              ))}
            </Select>
            {showErrors && errors?.node_id && (
              <p className="text-[10px] text-rose-600">{errors.node_id}</p>
            )}
          </div>
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Status
            </label>
            <Select
              value={value.status ?? ""}
              onChange={(e) => onChange({ ...value, status: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.status
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
            >
              {"online offline maintenance".split(" ").map((status) => (
                <option key={status} value={status}>
                  {status}
                </option>
              ))}
            </Select>
            {showErrors && errors?.status && (
              <p className="text-[10px] text-rose-600">{errors.status}</p>
            )}
          </div>
        </div>
      )}

      {value.type === "forecast" && (
        <div className="grid gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Forecast field
            </label>
            <Input
              value={value.field ?? ""}
              onChange={(e) => onChange({ ...value, field: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.field
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
              placeholder="rain_mm"
            />
            {showErrors && errors?.field && (
              <p className="text-[10px] text-rose-600">{errors.field}</p>
            )}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Operator
              </label>
              <Select
                value={value.operator ?? ""}
                onChange={(e) => onChange({ ...value, operator: e.target.value })}
                className={`mt-1 rounded px-2 py-1 text-xs ${
                  showErrors && errors?.operator
 ? "border-rose-400 bg-rose-50"
                    : ""
                }`}
              >
                {"< <= > >=".split(" ").map((op) => (
                  <option key={op} value={op}>
                    {op}
                  </option>
                ))}
              </Select>
              {showErrors && errors?.operator && (
                <p className="text-[10px] text-rose-600">{errors.operator}</p>
              )}
            </div>
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Threshold
              </label>
              <NumericDraftInput
                value={value.threshold}
                onValueChange={(next) =>
                  onChange({ ...value, threshold: typeof next === "number" ? next : undefined })
                }
                className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                  showErrors && errors?.threshold
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
                }`}
                inputMode="decimal"
              />
              {showErrors && errors?.threshold && (
                <p className="text-[10px] text-rose-600">{errors.threshold}</p>
              )}
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Horizon (hours)
              </label>
              <NumericDraftInput
                value={value.horizon_hours}
                onValueChange={(next) =>
                  onChange({ ...value, horizon_hours: typeof next === "number" ? next : undefined })
                }
                className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                  showErrors && errors?.horizon_hours
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
                }`}
                inputMode="decimal"
              />
              {showErrors && errors?.horizon_hours && (
                <p className="text-[10px] text-rose-600">{errors.horizon_hours}</p>
              )}
            </div>
            <div className="flex items-end gap-2">
 <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
                  checked={value.fail_open ?? false}
                  onChange={(e) => onChange({ ...value, fail_open: e.target.checked })}
                />
                Fail open
              </label>
            </div>
          </div>
        </div>
      )}

      {value.type === "analytics" && (
        <div className="grid gap-3">
          <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
              Metric key
            </label>
            <Input
              value={value.key ?? ""}
              onChange={(e) => onChange({ ...value, key: e.target.value })}
              className={`mt-1 rounded px-2 py-1 text-xs ${
                showErrors && errors?.key
 ? "border-rose-400 bg-rose-50"
                  : ""
              }`}
              placeholder="power_kw"
            />
            {showErrors && errors?.key && (
              <p className="text-[10px] text-rose-600">{errors.key}</p>
            )}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Operator
              </label>
              <Select
                value={value.operator ?? ""}
                onChange={(e) => onChange({ ...value, operator: e.target.value })}
                className={`mt-1 rounded px-2 py-1 text-xs ${
                  showErrors && errors?.operator
 ? "border-rose-400 bg-rose-50"
                    : ""
                }`}
              >
                {"< <= > >=".split(" ").map((op) => (
                  <option key={op} value={op}>
                    {op}
                  </option>
                ))}
              </Select>
              {showErrors && errors?.operator && (
                <p className="text-[10px] text-rose-600">{errors.operator}</p>
              )}
            </div>
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Threshold
              </label>
              <NumericDraftInput
                value={value.threshold}
                onValueChange={(next) =>
                  onChange({ ...value, threshold: typeof next === "number" ? next : undefined })
                }
                className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                  showErrors && errors?.threshold
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
                }`}
                inputMode="decimal"
              />
              {showErrors && errors?.threshold && (
                <p className="text-[10px] text-rose-600">{errors.threshold}</p>
              )}
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
 <label className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Window (minutes)
              </label>
              <NumericDraftInput
                value={value.window_minutes}
                onValueChange={(next) =>
                  onChange({ ...value, window_minutes: typeof next === "number" ? next : undefined })
                }
                className={`mt-1 w-full rounded border px-2 py-1 text-xs ${
                  showErrors && errors?.window_minutes
 ? "border-rose-400 bg-rose-50"
 : "border-border bg-white"
                }`}
                placeholder="optional"
                inputMode="decimal"
              />
              {showErrors && errors?.window_minutes && (
                <p className="text-[10px] text-rose-600">{errors.window_minutes}</p>
              )}
            </div>
            <div className="flex items-end gap-2">
 <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
                  checked={value.fail_open ?? false}
                  onChange={(e) => onChange({ ...value, fail_open: e.target.checked })}
                />
                Fail open
              </label>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
