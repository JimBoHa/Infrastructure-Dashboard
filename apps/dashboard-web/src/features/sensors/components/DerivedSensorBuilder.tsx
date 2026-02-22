"use client";

import { useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import NodeButton from "@/features/nodes/components/NodeButton";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { postJson } from "@/lib/http";
import { queryKeys } from "@/lib/queries";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

type DerivedInputDraft = {
  key: string;
  sensorId: string;
  var: string;
};

type FunctionButton = {
  label: string;
  insert: string;
  title?: string;
};

const QUICK_FUNCTION_BUTTONS: FunctionButton[] = [
  { label: "avg()", insert: "avg()" },
  { label: "min()", insert: "min()" },
  { label: "max()", insert: "max()" },
  { label: "sum()", insert: "sum()" },
  { label: "clamp(x,lo,hi)", insert: "clamp(, , )" },
  { label: "abs()", insert: "abs()" },
  { label: "round(x,dec)", insert: "round(, )" },
  { label: "sqrt()", insert: "sqrt()" },
  { label: "pow(x,y)", insert: "pow(, )" },
  { label: "if(cond,a,b)", insert: "if(, , )" },
];

const MORE_FUNCTION_BUTTONS: FunctionButton[] = [
  { label: "floor()", insert: "floor()" },
  { label: "ceil()", insert: "ceil()" },
  { label: "ln()", insert: "ln()" },
  { label: "log10()", insert: "log10()" },
  { label: "log(x,base)", insert: "log(, )" },
  { label: "exp()", insert: "exp()" },
  { label: "sin()", insert: "sin()" },
  { label: "cos()", insert: "cos()" },
  { label: "tan()", insert: "tan()" },
  { label: "deg2rad()", insert: "deg2rad()" },
  { label: "rad2deg()", insert: "rad2deg()" },
  { label: "sign()", insert: "sign()" },
];

const isValidVarName = (value: string): boolean => {
  const trimmed = value.trim();
  if (!trimmed) return false;
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(trimmed)) return false;
  return true;
};

const nextDefaultVar = (taken: Set<string>): string => {
  const letters = "abcdefghijklmnopqrstuvwxyz".split("");
  for (const letter of letters) {
    if (!taken.has(letter)) return letter;
  }
  let i = 1;
  while (i < 1000) {
    const candidate = `x${i}`;
    if (!taken.has(candidate)) return candidate;
    i += 1;
  }
  return `v${Date.now()}`;
};

const insertAtCursor = (
  el: HTMLTextAreaElement | null,
  value: string,
  fallbackSet: (next: string) => void,
): void => {
  if (!el) {
    fallbackSet(value);
    return;
  }
  const start = el.selectionStart ?? el.value.length;
  const end = el.selectionEnd ?? el.value.length;
  const next = el.value.slice(0, start) + value + el.value.slice(end);
  fallbackSet(next);
  const cursor = start + value.length;
  requestAnimationFrame(() => {
    try {
      el.focus();
      el.setSelectionRange(cursor, cursor);
    } catch {
      // ignore
    }
  });
};

export default function DerivedSensorBuilder({
  ownerNodeId,
  nodes,
  sensors,
  canEdit,
  onCreated,
}: {
  ownerNodeId: string;
  nodes: DemoNode[];
  sensors: DemoSensor[];
  canEdit: boolean;
  onCreated?: (sensorId: string) => void;
}) {
  const queryClient = useQueryClient();
  const expressionRef = useRef<HTMLTextAreaElement | null>(null);

  const [name, setName] = useState("Derived sensor");
  const [type, setType] = useState("derived");
  const [unit, setUnit] = useState("");
  const [intervalSeconds, setIntervalSeconds] = useState<number | null>(30);
  const [rollingAvgSeconds, setRollingAvgSeconds] = useState<number | null>(0);
  const [expression, setExpression] = useState("");
  const [inputs, setInputs] = useState<DerivedInputDraft[]>([]);
  const [sensorSearch, setSensorSearch] = useState("");
  const [selectedSensorId, setSelectedSensorId] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

  const nodeNameById = useMemo(() => new Map(nodes.map((node) => [node.id, node.name])), [nodes]);
  const sensorById = useMemo(() => new Map(sensors.map((sensor) => [sensor.sensor_id, sensor])), [sensors]);

  const selectableSensors = useMemo(() => {
    const needle = sensorSearch.trim().toLowerCase();
    return sensors
      .filter((sensor) => {
        if (inputs.some((entry) => entry.sensorId === sensor.sensor_id)) return false;
        if (!needle) return true;
        const nodeName = nodeNameById.get(sensor.node_id) ?? sensor.node_id;
        return (
          sensor.name.toLowerCase().includes(needle) ||
          sensor.type.toLowerCase().includes(needle) ||
          nodeName.toLowerCase().includes(needle) ||
          sensor.sensor_id.toLowerCase().includes(needle)
        );
      })
      .slice()
      .sort((a, b) => {
        const nodeA = nodeNameById.get(a.node_id) ?? a.node_id;
        const nodeB = nodeNameById.get(b.node_id) ?? b.node_id;
        if (nodeA !== nodeB) return nodeA.localeCompare(nodeB);
        return a.name.localeCompare(b.name);
      });
  }, [inputs, nodeNameById, sensorSearch, sensors]);

  const invalidInputs = useMemo(() => {
    const invalid = new Set<string>();
    const seen = new Set<string>();
    for (const entry of inputs) {
      const trimmed = entry.var.trim();
      if (!isValidVarName(trimmed)) {
        invalid.add(entry.key);
        continue;
      }
      if (seen.has(trimmed)) {
        invalid.add(entry.key);
        continue;
      }
      seen.add(trimmed);
    }
    return invalid;
  }, [inputs]);

  const intervalValue = intervalSeconds ?? NaN;
  const rollingValue = rollingAvgSeconds ?? NaN;
  const invalidInterval = !Number.isFinite(intervalValue) || intervalValue < 0;
  const invalidRolling = !Number.isFinite(rollingValue) || rollingValue < 0;

  const canSubmit =
    canEdit &&
    !busy &&
    name.trim().length > 0 &&
    type.trim().length > 0 &&
    unit.trim().length > 0 &&
    inputs.length > 0 &&
    invalidInputs.size === 0 &&
    expression.trim().length > 0 &&
    !invalidInterval &&
    !invalidRolling;

  const addInput = () => {
    const sensorId = selectedSensorId.trim();
    const sensor = sensorById.get(sensorId);
    if (!sensor) return;
    const taken = new Set(inputs.map((entry) => entry.var.trim()).filter(Boolean));
    const nextVar = nextDefaultVar(taken);
    setInputs((current) => [
      ...current,
      { key: `input-${Date.now()}-${sensorId}`, sensorId, var: nextVar },
    ]);
    setSelectedSensorId("");
  };

  const create = async () => {
    if (!canSubmit) return;
    setBusy(true);
    setMessage(null);
    try {
      const payload = {
        node_id: ownerNodeId,
        name: name.trim(),
        type: type.trim(),
        unit: unit.trim(),
        interval_seconds: intervalValue,
        rolling_avg_seconds: rollingValue,
        config: {
          source: "derived",
          derived: {
            expression: expression.trim(),
            inputs: inputs.map((entry) => ({
              sensor_id: entry.sensorId,
              var: entry.var.trim(),
            })),
          },
        },
      };
      const created = (await postJson<unknown>("/api/sensors", payload)) as {
        sensor_id?: string;
      };
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
      ]);
      const sensorId = typeof created?.sensor_id === "string" ? created.sensor_id : null;
      setMessage({ type: "success", text: "Created derived sensor." });
      if (sensorId && onCreated) onCreated(sensorId);
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Failed to create derived sensor.",
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-6">
      {message ? (
        <InlineBanner tone={message.type === "success" ? "success" : "danger"}>{message.text}</InlineBanner>
      ) : null}

      <CollapsibleCard
        density="sm"
        title="Basics"
        description="Define how this derived sensor will be labeled and how its series is sampled."
        defaultOpen
      >
        <div className="grid gap-4 md:grid-cols-2">
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Display name
            </label>
            <Input
              className="mt-1"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g. Delta pressure"
            />
          </div>

          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Sensor type
            </label>
            <Input
              className="mt-1"
              value={type}
              onChange={(e) => setType(e.target.value)}
              placeholder="e.g. pressure"
            />
 <p className="mt-1 text-xs text-muted-foreground">
              Used for grouping/filters. Keep it stable (e.g. <code className="px-1">temperature</code>,{" "}
              <code className="px-1">pressure</code>, <code className="px-1">flow</code>).
            </p>
          </div>

          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Unit
            </label>
            <Input
              className="mt-1"
              value={unit}
              onChange={(e) => setUnit(e.target.value)}
              placeholder="e.g. kPa"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Interval (s)
              </label>
              <NumericDraftInput
                value={intervalSeconds}
                onValueChange={(next) => setIntervalSeconds(typeof next === "number" ? next : null)}
                integer
                min={0}
                enforceRange
                className={[
 "mt-1 w-full rounded-lg border bg-white px-3 py-2 text-sm text-foreground shadow-xs focus:outline-hidden focus:ring-2",
                  invalidInterval
 ? "border-rose-300 focus:ring-rose-300/40"
 : "border-border focus:ring-indigo-500",
                ].join(" ")}
                inputMode="numeric"
                autoComplete="off"
              />
 <p className="mt-1 text-xs text-muted-foreground">
                How often this series emits a value. Use <code className="px-1">0</code> for change-of-value (COV).
              </p>
            </div>

            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Rolling avg (s)
              </label>
              <NumericDraftInput
                value={rollingAvgSeconds}
                onValueChange={(next) => setRollingAvgSeconds(typeof next === "number" ? next : null)}
                integer
                min={0}
                enforceRange
                className={[
 "mt-1 w-full rounded-lg border bg-white px-3 py-2 text-sm text-foreground shadow-xs focus:outline-hidden focus:ring-2",
                  invalidRolling
 ? "border-rose-300 focus:ring-rose-300/40"
 : "border-border focus:ring-indigo-500",
                ].join(" ")}
                inputMode="numeric"
                autoComplete="off"
              />
 <p className="mt-1 text-xs text-muted-foreground">
                Smoothing window applied before each interval value is stored.
              </p>
            </div>
          </div>
        </div>
      </CollapsibleCard>

      <CollapsibleCard
        density="sm"
        title="Inputs"
        description={
          <span>
            Select sensors and assign each one a variable name (letters/numbers/underscore). Derived sensors can use
            other derived sensors as inputs. Cycles are rejected. Max chain depth: 10.
          </span>
        }
        defaultOpen
      >
        <div className="grid gap-3">
          <div className="grid gap-3 md:grid-cols-[1fr,220px,auto] md:items-end">
 <label className="flex flex-col gap-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Search sensors
              <Input
                value={sensorSearch}
                onChange={(e) => setSensorSearch(e.target.value)}
                className="mt-1"
                placeholder="Filter by node, name, type, or id…"
              />
            </label>

 <label className="flex flex-col gap-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Pick a sensor
              <Select
                value={selectedSensorId}
                onChange={(e) => setSelectedSensorId(e.target.value)}
                className="mt-1"
              >
                <option value="">Select…</option>
                {selectableSensors.map((sensor) => {
                  const nodeName = nodeNameById.get(sensor.node_id) ?? sensor.node_id;
                  const unitLabel = sensor.unit ? ` ${sensor.unit}` : "";
                  return (
                    <option key={sensor.sensor_id} value={sensor.sensor_id}>
                      {nodeName} — {sensor.name} ({sensor.type}{unitLabel})
                    </option>
                  );
                })}
              </Select>
            </label>

            <NodeButton
              size="sm"
              onClick={addInput}
              disabled={!canEdit || busy || !selectedSensorId}
            >
              Add input
            </NodeButton>
          </div>

          {inputs.length ? (
            <div className="space-y-2">
              {inputs.map((entry) => {
                const sensor = sensorById.get(entry.sensorId);
                const nodeName = sensor ? nodeNameById.get(sensor.node_id) ?? sensor.node_id : null;
                const invalid = invalidInputs.has(entry.key);
                const latestValue =
                  sensor?.latest_value != null && Number.isFinite(sensor.latest_value)
                    ? `${sensor.latest_value}${sensor.unit ? ` ${sensor.unit}` : ""}`
                    : "—";
                return (
                  <Card
                    key={entry.key}
                    className="flex flex-col gap-3 rounded-lg bg-card-inset p-3 md:flex-row md:items-center md:justify-between"
                  >
                    <div className="min-w-0 space-y-1">
                      <div className="flex flex-wrap items-center gap-2">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          {nodeName ?? "Unknown node"}
                        </span>
                        {sensor ? <SensorOriginBadge sensor={sensor} size="xs" /> : null}
                      </div>
 <p className="truncate text-sm font-semibold text-foreground">
                        {sensor?.name ?? entry.sensorId}
                      </p>
 <p className="text-xs text-muted-foreground">
                        {sensor ? `${sensor.type}${sensor.unit ? ` · ${sensor.unit}` : ""}` : "Sensor not found"} ·
                        Latest: {latestValue}
                      </p>
                    </div>

                    <div className="flex flex-col gap-2 md:flex-row md:items-end">
 <label className="flex flex-col gap-1 text-xs font-semibold text-foreground">
                        Variable
                        <Input
                          className={[
                            "w-40 px-2 py-1.5 font-normal",
                            invalid
 ? "border-rose-300 focus:border-rose-400 focus:ring-rose-200"
                              : "",
                          ].join(" ")}
                          value={entry.var}
                          onChange={(e) => {
                            const next = e.target.value;
                            setInputs((current) =>
                              current.map((row) => (row.key === entry.key ? { ...row, var: next } : row)),
                            );
                          }}
                          placeholder="e.g. a"
                        />
                      </label>

                      <div className="flex items-center gap-2">
                        <NodeButton
                          size="xs"
                          onClick={() => insertAtCursor(expressionRef.current, entry.var.trim() || entry.var, setExpression)}
                          disabled={!entry.var.trim()}
                        >
                          Insert
                        </NodeButton>
                        <NodeButton
                          size="xs"
                          variant="danger"
                          onClick={() => {
                            setInputs((current) => current.filter((row) => row.key !== entry.key));
                          }}
                          disabled={busy}
                        >
                          Remove
                        </NodeButton>
                      </div>
                    </div>
                  </Card>
                );
              })}
            </div>
          ) : (
 <p className="text-sm text-muted-foreground">No inputs yet.</p>
          )}
        </div>
      </CollapsibleCard>

      <CollapsibleCard
        density="sm"
        title="Expression"
        description={
          <span>
            Supported operators: <code className="px-1">+</code>, <code className="px-1">-</code>,{" "}
            <code className="px-1">*</code>, <code className="px-1">/</code>, parentheses. Common functions are below;
            expand “More functions” for the full library.
          </span>
        }
        defaultOpen
      >
        <div>
          <Textarea
            ref={expressionRef}
            value={expression}
            onChange={(e) => setExpression(e.target.value)}
            rows={4}
            className="font-mono"
            placeholder="e.g. clamp(avg(a, b), 0, 100)"
          />

          <div className="mt-3 flex flex-wrap gap-2">
            {QUICK_FUNCTION_BUTTONS.map((item) => (
              <NodeButton
                key={item.label}
                size="xs"
                title={item.title}
                onClick={() => insertAtCursor(expressionRef.current, item.insert, setExpression)}
              >
                {item.label}
              </NodeButton>
            ))}
          </div>

          <CollapsibleCard title="More functions" className="mt-3 bg-card-inset shadow-xs" density="sm" defaultOpen={false}>
            <div className="flex flex-wrap gap-2">
              {MORE_FUNCTION_BUTTONS.map((item) => (
                <NodeButton
                  key={item.label}
                  size="xs"
                  title={item.title}
                  onClick={() => insertAtCursor(expressionRef.current, item.insert, setExpression)}
                >
                  {item.label}
                </NodeButton>
              ))}
            </div>
 <div className="mt-2 space-y-1 text-xs text-muted-foreground">
              <p>
                Trig functions use radians. Use <code className="px-1">deg2rad()</code> /{" "}
                <code className="px-1">rad2deg()</code> to convert.
              </p>
              <p>
                Full library:{" "}
                <code className="px-1">min</code>, <code className="px-1">max</code>, <code className="px-1">sum</code>,{" "}
                <code className="px-1">avg</code>, <code className="px-1">clamp</code>, <code className="px-1">abs</code>,{" "}
                <code className="px-1">round</code>, <code className="px-1">floor</code>, <code className="px-1">ceil</code>,{" "}
                <code className="px-1">sqrt</code>, <code className="px-1">pow</code>, <code className="px-1">ln</code>,{" "}
                <code className="px-1">log10</code>, <code className="px-1">log</code>, <code className="px-1">exp</code>,{" "}
                <code className="px-1">sin</code>, <code className="px-1">cos</code>, <code className="px-1">tan</code>,{" "}
                <code className="px-1">deg2rad</code>, <code className="px-1">rad2deg</code>, <code className="px-1">sign</code>,{" "}
                <code className="px-1">if</code>.
              </p>
            </div>
          </CollapsibleCard>
        </div>
      </CollapsibleCard>

      <CollapsibleCard
        density="sm"
        title="Create"
        description={
          <span>
            Derived sensors are computed by the controller. They show as <code className="px-1">DERIVED</code> in the UI.
          </span>
        }
        defaultOpen
        actions={
          <NodeButton variant="primary" onClick={create} disabled={!canSubmit} loading={busy} size="sm">
            Create derived sensor
          </NodeButton>
        }
      >
        {!canEdit ? (
 <p className="text-xs text-muted-foreground">
            Read-only: requires <code className="px-1">config.write</code>.
          </p>
        ) : null}
      </CollapsibleCard>
    </div>
  );
}
