"use client";

import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import NodeButton from "@/features/nodes/components/NodeButton";
import { cn } from "@/lib/utils";

import { SENSOR_PRESETS } from "../presets";
import type {
  SensorDraft,
  SensorDriverType,
  SensorFieldKey,
  SensorPresetKey,
} from "../types";

type Props = {
  sensor: SensorDraft;
  invalidFields: Set<SensorFieldKey>;
  isExpanded: boolean;
  onToggleExpanded: () => void;
  onUpdateSensor: (patch: Partial<SensorDraft>) => void;
  onApplyPreset: (preset: SensorPresetKey) => void;
  onGenerateSensorId: () => void;
  onRemove: () => void;
  onDone: () => void;
};

/** Base classes for raw <input> rendered by NumericDraftInput (mirrors Input component). */
const INPUT_BASE =
 "mt-1 block w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground shadow-xs placeholder:text-muted-foreground focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30 disabled:opacity-60";

const INVALID_CLASS =
  "border-red-300 bg-red-50 text-red-900 placeholder:text-red-400 focus:border-red-500 focus:ring-red-500";

export function SensorCard({
  sensor,
  invalidFields,
  isExpanded,
  onToggleExpanded,
  onUpdateSensor,
  onApplyPreset,
  onGenerateSensorId,
  onRemove,
  onDone,
}: Props) {
  return (
    <Card className="gap-0 p-0">
      <button
        type="button"
        onClick={onToggleExpanded}
 className="flex w-full items-start justify-between gap-3 px-4 py-3 text-left hover:bg-muted"
      >
        <div>
          <div className="flex items-center gap-2">
            <p className="text-sm font-semibold text-card-foreground">
              {sensor.name || "Untitled sensor"}
            </p>
            {invalidFields.size > 0 && (
              <span className="rounded-full bg-danger-surface px-2 py-0.5 text-xs font-semibold text-danger-surface-foreground">
                Needs attention
              </span>
            )}
          </div>
 <p className="mt-0.5 text-xs text-muted-foreground">
            {sensor.sensor_id || "missing-id"} · {sensor.type} · ch {sensor.channel} ·{" "}
            {sensor.interval_seconds === 0 ? "COV" : `${sensor.interval_seconds}s`}
          </p>
        </div>
 <span className="text-xs font-semibold text-muted-foreground">
          {isExpanded ? "Hide" : "Edit"}
        </span>
      </button>

      {isExpanded && (
        <div className="border-t border-border px-4 py-4">
          <div className="grid gap-4 md:grid-cols-2">
            <div className="md:col-span-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Preset</label>
              <Select
                value={sensor.preset}
                onChange={(event) => onApplyPreset(event.target.value as SensorPresetKey)}
                className="mt-1"
              >
                {Object.entries(SENSOR_PRESETS).map(([key, preset]) => (
                  <option key={key} value={key}>
                    {preset.label}
                  </option>
                ))}
              </Select>
 <p className="mt-1 text-xs text-muted-foreground">
                {SENSOR_PRESETS[sensor.preset].hint}
              </p>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Sensor ID</label>
              <div className="mt-1 flex gap-2">
                <Input
                  value={sensor.sensor_id}
                  onChange={(event) => onUpdateSensor({ sensor_id: event.target.value })}
                  className={cn(invalidFields.has("sensor_id") && INVALID_CLASS)}
                  placeholder="soil-moisture-north"
                  autoComplete="off"
                />
                <NodeButton
                  type="button"
                  size="xs"
                  onClick={onGenerateSensorId}
                >
                  Gen
                </NodeButton>
              </div>
 <p className="mt-1 text-xs text-muted-foreground">
                Used in MQTT topics; keep it stable.
              </p>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Display name</label>
              <Input
                value={sensor.name}
                onChange={(event) => onUpdateSensor({ name: event.target.value })}
                className={cn("mt-1", invalidFields.has("name") && INVALID_CLASS)}
                placeholder="Soil Moisture - North"
                autoComplete="off"
              />
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Driver</label>
              <Select
                value={sensor.type}
                onChange={(event) =>
                  onUpdateSensor({ type: event.target.value as SensorDriverType })
                }
                className={cn("mt-1", invalidFields.has("type") && INVALID_CLASS)}
              >
                <option value="analog">Analog (ADC voltage)</option>
                <option value="pulse">Pulse counter</option>
                <option value="gpio_pulse">GPIO pulse</option>
              </Select>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Channel</label>
              <Input
                value={String(sensor.channel)}
                onChange={(event) =>
                  onUpdateSensor({
                    channel: Number.parseInt(event.target.value || "0", 10),
                  })
                }
                className={cn("mt-1", invalidFields.has("channel") && INVALID_CLASS)}
                inputMode="numeric"
                autoComplete="off"
              />
 <p className="mt-1 text-xs text-muted-foreground">
                Analog channels: ADS1263 HAT uses 0–9.
              </p>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Unit</label>
              <Input
                value={sensor.unit}
                onChange={(event) => onUpdateSensor({ unit: event.target.value })}
                className={cn("mt-1", invalidFields.has("unit") && INVALID_CLASS)}
                placeholder="%"
                autoComplete="off"
              />
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Location (optional)</label>
              <Input
                value={sensor.location}
                onChange={(event) => onUpdateSensor({ location: event.target.value })}
                className="mt-1"
                placeholder="North field row 5"
                autoComplete="off"
              />
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Interval (seconds)</label>
              <NumericDraftInput
                value={sensor.interval_seconds}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    onUpdateSensor({ interval_seconds: next });
                  }
                }}
                emptyBehavior="keep"
                min={0}
                enforceRange
                className={cn(INPUT_BASE, invalidFields.has("interval_seconds") && INVALID_CLASS)}
                inputMode="decimal"
                autoComplete="off"
              />
 <p className="mt-1 text-xs text-muted-foreground">
                Use 0 to publish only on change-of-value (COV).
              </p>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Rolling avg (seconds)</label>
              <NumericDraftInput
                value={sensor.rolling_average_seconds}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    onUpdateSensor({ rolling_average_seconds: next });
                  }
                }}
                emptyBehavior="keep"
                min={0}
                enforceRange
                className={cn(INPUT_BASE, invalidFields.has("rolling_average_seconds") && INVALID_CLASS)}
                inputMode="decimal"
                autoComplete="off"
              />
 <p className="mt-1 text-xs text-muted-foreground">0 disables rolling average.</p>
            </div>

            <div className="md:col-span-2 grid gap-4 md:grid-cols-2">
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Input range (min / max)
                </label>
                <div className="mt-1 grid grid-cols-2 gap-2">
                  <NumericDraftInput
                    value={sensor.input_min}
                    onValueChange={(next) =>
                      onUpdateSensor({ input_min: typeof next === "number" ? next : null })
                    }
                    className={INPUT_BASE}
                    placeholder="e.g. 0 (V or mA)"
                    inputMode="decimal"
                    autoComplete="off"
                  />
                  <NumericDraftInput
                    value={sensor.input_max}
                    onValueChange={(next) =>
                      onUpdateSensor({ input_max: typeof next === "number" ? next : null })
                    }
                    className={INPUT_BASE}
                    placeholder="e.g. 10 (V) or 20 (mA)"
                    inputMode="decimal"
                    autoComplete="off"
                  />
                </div>
 <p className="mt-1 text-xs text-muted-foreground">
                  Maps raw voltage/current to engineering units (linear scaling).
                </p>
              </div>

              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Output range (min / max)
                </label>
                <div className="mt-1 grid grid-cols-2 gap-2">
                  <NumericDraftInput
                    value={sensor.output_min}
                    onValueChange={(next) =>
                      onUpdateSensor({ output_min: typeof next === "number" ? next : null })
                    }
                    className={INPUT_BASE}
                    placeholder="e.g. 0 (psi)"
                    inputMode="decimal"
                    autoComplete="off"
                  />
                  <NumericDraftInput
                    value={sensor.output_max}
                    onValueChange={(next) =>
                      onUpdateSensor({ output_max: typeof next === "number" ? next : null })
                    }
                    className={INPUT_BASE}
                    placeholder="e.g. 300 (psi)"
                    inputMode="decimal"
                    autoComplete="off"
                  />
                </div>
 <p className="mt-1 text-xs text-muted-foreground">
                  Linear scaling applied before offset/scale.
                </p>
              </div>
            </div>

            {sensor.preset.startsWith("water_level") ? (
              <div className="md:col-span-2 grid gap-4 md:grid-cols-2">
                <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Shunt resistor (Ω)
                  </label>
                  <NumericDraftInput
                    value={sensor.current_loop_shunt_ohms}
                    onValueChange={(next) =>
                      onUpdateSensor({ current_loop_shunt_ohms: typeof next === "number" ? next : null })
                    }
                    className={cn(INPUT_BASE, invalidFields.has("current_loop_shunt_ohms") && INVALID_CLASS)}
                    placeholder="e.g. 163"
                    inputMode="decimal"
                    autoComplete="off"
                  />
 <p className="mt-1 text-xs text-muted-foreground">
                    4–20mA loop: voltage (V) ÷ ohms = current. Use the resistor in series with the transducer.
                  </p>
                </div>

                <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Transducer range (m)
                  </label>
                  <NumericDraftInput
                    value={sensor.current_loop_range_m}
                    onValueChange={(next) =>
                      onUpdateSensor({ current_loop_range_m: typeof next === "number" ? next : null })
                    }
                    className={cn(INPUT_BASE, invalidFields.has("current_loop_range_m") && INVALID_CLASS)}
                    placeholder="e.g. 5.0"
                    inputMode="decimal"
                    autoComplete="off"
                  />
 <p className="mt-1 text-xs text-muted-foreground">
                    Used to compute depth and emit fault-quality markers when current is out of range.
                  </p>
                </div>

 <p className="md:col-span-2 text-xs text-muted-foreground">
                  When both fields are set, current-loop conversion overrides the linear input/output mapping above.
                </p>
              </div>
            ) : null}

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Offset</label>
              <NumericDraftInput
                value={sensor.offset}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    onUpdateSensor({ offset: next });
                  }
                }}
                emptyBehavior="keep"
                className={INPUT_BASE}
                placeholder="0"
                inputMode="decimal"
                autoComplete="off"
              />
 <p className="mt-1 text-xs text-muted-foreground">Additive correction after scaling.</p>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Scale</label>
              <NumericDraftInput
                value={sensor.scale}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    onUpdateSensor({ scale: next });
                  }
                }}
                emptyBehavior="keep"
                className={INPUT_BASE}
                placeholder="1.0"
                inputMode="decimal"
                autoComplete="off"
              />
 <p className="mt-1 text-xs text-muted-foreground">
                Multiplicative correction after offset.
              </p>
            </div>

            {(sensor.type === "pulse" || sensor.type === "gpio_pulse") && (
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Pulses per unit</label>
                <NumericDraftInput
                  value={sensor.pulses_per_unit}
                  onValueChange={(next) =>
                    onUpdateSensor({ pulses_per_unit: typeof next === "number" ? next : null })
                  }
                  className={INPUT_BASE}
                  placeholder="e.g. 450 pulses per gallon"
                  inputMode="decimal"
                  autoComplete="off"
                />
 <p className="mt-1 text-xs text-muted-foreground">
                  Converts pulse counts to engineering units before publishing.
                </p>
              </div>
            )}
          </div>

          <div className="mt-4 flex flex-wrap items-center justify-between gap-3">
            <NodeButton
              type="button"
              variant="danger"
              onClick={onRemove}
            >
              Remove sensor
            </NodeButton>
            <NodeButton
              type="button"
              onClick={onDone}
            >
              Done
            </NodeButton>
          </div>
        </div>
      )}
    </Card>
  );
}
