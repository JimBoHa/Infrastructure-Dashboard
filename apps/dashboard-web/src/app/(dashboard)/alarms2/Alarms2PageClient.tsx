"use client";

import { useMemo, useState, useCallback, type ReactNode } from "react";
import PageHeaderCard from "@/components/PageHeaderCard";
import InlineBanner from "@/components/InlineBanner";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import { Card } from "@/components/ui/card";
import { Badge, type BadgeTone } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { useAuth } from "@/components/AuthProvider";
import {
  useAlarmRulesQuery,
  useAlarmsQuery,
  useAlarmEventsQuery,
  useNodesQuery,
  useSensorsQuery,
} from "@/lib/queries";
import type {
  AlarmRule,
  AlarmRulePreviewResponse,
  AlarmSeverity,
  AlarmTemplateKind,
  AlarmWizardState,
} from "@/features/alarms/types/alarmTypes";
import type { DemoAlarm, DemoAlarmEvent, DemoNode, DemoSensor } from "@/types/dashboard";
import useAlarmMutations from "@/features/alarms/hooks/useAlarmMutations";
import { buildRequestFromWizard, buildAdvancedJson } from "@/features/alarms/utils/ruleBuilder";
import { describeCondition } from "@/features/alarms/utils/ruleSummary";
import { cn } from "@/lib/utils";

/* ═══════════════════════════════════════════════
   Constants
   ═══════════════════════════════════════════════ */

const RECIPES: {
  id: AlarmTemplateKind;
  label: string;
  desc: string;
  hint: string;
  accent: string;
  iconBg: string;
  icon: ReactNode;
}[] = [
  {
    id: "threshold",
    label: "Threshold",
    desc: "Value crosses a limit",
    hint: "e.g. pump pressure drops below 15 PSI",
    accent: "border-l-4 border-l-amber-400",
    iconBg: "bg-amber-100 text-amber-700",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-5">
        <path d="M3 12h18" strokeLinecap="round" />
        <path d="M16 6L8 18" strokeWidth="2.5" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    id: "range",
    label: "Range Band",
    desc: "Value leaves a safe zone",
    hint: "e.g. voltage outside 105–130 V",
    accent: "border-l-4 border-l-sky-400",
    iconBg: "bg-sky-100 text-sky-700",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-5">
        <path d="M3 7h18M3 17h18" strokeLinecap="round" />
        <rect x="3" y="7" width="18" height="10" fill="currentColor" opacity="0.08" stroke="none" />
      </svg>
    ),
  },
  {
    id: "offline",
    label: "Sensor Offline",
    desc: "No data for a duration",
    hint: "e.g. no readings for 5 minutes",
    accent: "border-l-4 border-l-gray-400",
    iconBg: "bg-gray-100 text-gray-600",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-5">
        <circle cx="12" cy="12" r="9" />
        <path d="M4 4l16 16" strokeLinecap="round" />
      </svg>
    ),
  },
  {
    id: "rolling_window",
    label: "Rolling Average",
    desc: "Aggregate over a time window",
    hint: "e.g. avg power > 5 kW over 10 min",
    accent: "border-l-4 border-l-violet-400",
    iconBg: "bg-violet-100 text-violet-700",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-5">
        <path d="M3 17l4-4 3 2 4-6 4 3 3-4" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    id: "deviation",
    label: "Deviation",
    desc: "Drift from baseline",
    hint: "e.g. > 10% deviation from mean",
    accent: "border-l-4 border-l-teal-400",
    iconBg: "bg-teal-100 text-teal-700",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-5">
        <path d="M12 4v16M7 8l-4 4 4 4M17 8l4 4-4 4" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    ),
  },
  {
    id: "consecutive",
    label: "Sustained",
    desc: "Condition persists for N periods",
    hint: "e.g. battery < 12 V for 3 consecutive days",
    accent: "border-l-4 border-l-rose-400",
    iconBg: "bg-rose-100 text-rose-700",
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="size-5">
        <rect x="4" y="8" width="4" height="8" rx="1" />
        <rect x="10" y="8" width="4" height="8" rx="1" />
        <rect x="16" y="8" width="4" height="8" rx="1" />
      </svg>
    ),
  },
];

const OP_LABELS: Record<string, string> = {
  lt: "drops below",
  lte: "drops to or below",
  gt: "rises above",
  gte: "rises to or above",
  eq: "equals",
  neq: "does not equal",
};

const OP_OPTIONS = [
  { value: "lt", label: "< less than" },
  { value: "lte", label: "\u2264 less than or equal" },
  { value: "gt", label: "> greater than" },
  { value: "gte", label: "\u2265 greater than or equal" },
  { value: "eq", label: "= equals" },
  { value: "neq", label: "\u2260 not equal" },
];

const AGG_OPTIONS = [
  { value: "avg", label: "Average" },
  { value: "min", label: "Minimum" },
  { value: "max", label: "Maximum" },
  { value: "stddev", label: "Std. Deviation" },
];

const SEVERITY_CFG: Record<AlarmSeverity, { tone: BadgeTone; label: string; hint: string }> = {
  info: { tone: "info", label: "Info", hint: "Low priority" },
  warning: { tone: "warning", label: "Warning", hint: "Needs attention" },
  critical: { tone: "danger", label: "Critical", hint: "Immediate action" },
};

const STEP_LABELS = ["Recipe", "Target", "Condition", "Details", "Review"];

const DEFAULT_FORM: AlarmWizardState = {
  mode: "create",
  name: "",
  description: "",
  severity: "warning",
  origin: "threshold",
  template: "threshold",
  selectorMode: "sensor",
  sensorId: "",
  nodeId: "",
  filterProvider: "",
  filterMetric: "",
  filterType: "",
  thresholdOp: "lt",
  thresholdValue: "",
  rangeMode: "outside",
  rangeLow: "",
  rangeHigh: "",
  offlineSeconds: "300",
  rollingWindowSeconds: "300",
  rollingAggregate: "avg",
  rollingOp: "gt",
  rollingValue: "",
  deviationWindowSeconds: "300",
  deviationBaseline: "mean",
  deviationMode: "percent",
  deviationValue: "",
  consecutivePeriod: "day",
  consecutiveCount: "3",
  debounceSeconds: "0",
  clearHysteresisSeconds: "0",
  evalIntervalSeconds: "0",
  messageTemplate: "",
  advancedJson: "",
  advancedMode: false,
};

/* ═══════════════════════════════════════════════
   Helpers
   ═══════════════════════════════════════════════ */

const sevTone = (s: string): BadgeTone =>
  s === "critical" ? "danger" : s === "warning" ? "warning" : "info";

const targetLabel = (f: AlarmWizardState, sensors: DemoSensor[], nodes: DemoNode[]): string => {
  if (f.selectorMode === "sensor" && f.sensorId) {
    const s = sensors.find((x) => x.sensor_id === f.sensorId);
    return s ? s.name : f.sensorId;
  }
  if (f.selectorMode === "node" && f.nodeId) {
    const n = nodes.find((x) => x.id === f.nodeId);
    return n ? `all sensors on ${n.name}` : `node ${f.nodeId}`;
  }
  if (f.selectorMode === "filter") {
    const parts = [f.filterProvider, f.filterMetric, f.filterType].filter(Boolean);
    return parts.length ? `sensors matching ${parts.join(", ")}` : "filtered sensors";
  }
  return "\u2026";
};

const condLabel = (f: AlarmWizardState): string => {
  if (f.template === "threshold")
    return `value ${OP_LABELS[f.thresholdOp] ?? f.thresholdOp} ${f.thresholdValue || "\u2026"}`;
  if (f.template === "range")
    return `value ${f.rangeMode} [${f.rangeLow || "\u2026"}, ${f.rangeHigh || "\u2026"}]`;
  if (f.template === "offline") return `no data for ${f.offlineSeconds || "\u2026"}s`;
  if (f.template === "rolling_window")
    return `${f.rollingAggregate} over ${f.rollingWindowSeconds || "\u2026"}s ${OP_LABELS[f.rollingOp] ?? f.rollingOp} ${f.rollingValue || "\u2026"}`;
  if (f.template === "deviation")
    return `${f.deviationMode} deviation from ${f.deviationBaseline} > ${f.deviationValue || "\u2026"}`;
  if (f.template === "consecutive")
    return `value ${OP_LABELS[f.thresholdOp] ?? f.thresholdOp} ${f.thresholdValue || "\u2026"} for ${f.consecutiveCount || "\u2026"} consecutive ${f.consecutivePeriod}(s)`;
  return "\u2026";
};

const suggestName = (f: AlarmWizardState, sensors: DemoSensor[], nodes: DemoNode[]): string => {
  const tgt = targetLabel(f, sensors, nodes);
  const recipe = RECIPES.find((r) => r.id === f.template);
  return `${recipe?.label ?? "Alarm"}: ${tgt}`;
};

/** Reverse-parse an existing AlarmRule into wizard form state for editing. */
const parseRuleToForm = (rule: AlarmRule): AlarmWizardState => {
  const base: Partial<AlarmWizardState> = {
    mode: "edit",
    ruleId: rule.id,
    name: rule.name,
    description: rule.description,
    severity: rule.severity,
    origin: rule.origin,
    messageTemplate: rule.message_template,
    debounceSeconds: String(rule.timing?.debounce_seconds ?? 0),
    clearHysteresisSeconds: String(rule.timing?.clear_hysteresis_seconds ?? 0),
    evalIntervalSeconds: String(rule.timing?.eval_interval_seconds ?? 0),
  };

  const sel = rule.target_selector;
  if (sel.kind === "sensor") {
    base.selectorMode = "sensor";
    base.sensorId = sel.sensor_id;
  } else if (sel.kind === "node_sensors") {
    base.selectorMode = "node";
    base.nodeId = sel.node_id;
  } else if (sel.kind === "filter") {
    base.selectorMode = "filter";
    base.filterProvider = sel.provider ?? "";
    base.filterMetric = sel.metric ?? "";
    base.filterType = sel.sensor_type ?? "";
  } else {
    return {
      ...DEFAULT_FORM,
      ...base,
      advancedMode: true,
      advancedJson: JSON.stringify(
        { target_selector: rule.target_selector, condition_ast: rule.condition_ast, timing: rule.timing },
        null,
        2,
      ),
    };
  }

  const c = rule.condition_ast;
  if (c.type === "threshold")
    return { ...DEFAULT_FORM, ...base, template: "threshold", thresholdOp: c.op, thresholdValue: String(c.value) };
  if (c.type === "range")
    return { ...DEFAULT_FORM, ...base, template: "range", rangeMode: c.mode, rangeLow: String(c.low), rangeHigh: String(c.high) };
  if (c.type === "offline")
    return { ...DEFAULT_FORM, ...base, template: "offline", offlineSeconds: String(c.missing_for_seconds) };
  if (c.type === "rolling_window")
    return {
      ...DEFAULT_FORM, ...base, template: "rolling_window",
      rollingWindowSeconds: String(c.window_seconds), rollingAggregate: c.aggregate,
      rollingOp: c.op, rollingValue: String(c.value),
    };
  if (c.type === "deviation")
    return {
      ...DEFAULT_FORM, ...base, template: "deviation",
      deviationWindowSeconds: String(c.window_seconds), deviationBaseline: c.baseline,
      deviationMode: c.mode, deviationValue: String(c.value),
    };
  if (c.type === "consecutive_periods" && c.child.type === "threshold")
    return {
      ...DEFAULT_FORM, ...base, template: "consecutive",
      consecutivePeriod: c.period, consecutiveCount: String(c.count),
      thresholdOp: c.child.op, thresholdValue: String(c.child.value),
    };

  return {
    ...DEFAULT_FORM, ...base, advancedMode: true,
    advancedJson: JSON.stringify(
      { target_selector: rule.target_selector, condition_ast: rule.condition_ast, timing: rule.timing },
      null, 2,
    ),
  };
};

/* ═══════════════════════════════════════════════
   Sub-components
   ═══════════════════════════════════════════════ */

function StatusStrip({ rules, activeCount }: { rules: AlarmRule[]; activeCount: number }) {
  const enabled = rules.filter((r) => r.enabled).length;
  const disabled = rules.length - enabled;
  const errored = rules.filter((r) => r.last_error).length;

  const cells = [
    { label: "Active", value: activeCount, color: activeCount > 0 ? "text-rose-600" : "text-muted-foreground", bg: activeCount > 0 ? "bg-rose-50 border-rose-200" : "bg-card border-border", dot: activeCount > 0 ? "bg-rose-500" : "bg-gray-300", pulse: activeCount > 0 },
    { label: "Enabled", value: enabled, color: "text-emerald-600", bg: "bg-card border-border", dot: "bg-emerald-400", pulse: false },
    { label: "Disabled", value: disabled, color: "text-muted-foreground", bg: "bg-card border-border", dot: "bg-gray-300", pulse: false },
    { label: "Errors", value: errored, color: errored > 0 ? "text-amber-600" : "text-muted-foreground", bg: errored > 0 ? "bg-amber-50 border-amber-200" : "bg-card border-border", dot: errored > 0 ? "bg-amber-400" : "bg-gray-300", pulse: false },
  ];

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      {cells.map((c) => (
        <div key={c.label} className={cn("flex items-center gap-3 rounded-xl border px-4 py-3 shadow-sm", c.bg)}>
          {c.pulse ? (
            <span className="relative flex size-2.5">
              <span className="absolute inline-flex size-full animate-ping rounded-full bg-rose-400 opacity-75" />
              <span className="relative inline-flex size-2.5 rounded-full bg-rose-500" />
            </span>
          ) : (
            <span className={cn("size-2 rounded-full", c.dot)} />
          )}
          <div>
            <span className={cn("text-xl font-bold tabular-nums tracking-tight", c.color)}>{c.value}</span>
            <span className="ml-1.5 text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">{c.label}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

function StepBar({ current, onChange, maxReached }: { current: number; onChange: (s: number) => void; maxReached: number }) {
  return (
    <div className="flex items-center gap-0.5">
      {STEP_LABELS.map((label, i) => {
        const done = i < current;
        const active = i === current;
        const reachable = i <= maxReached;
        return (
          <div key={label} className="flex items-center gap-0.5">
            {i > 0 && <div className={cn("h-px w-4 sm:w-8", done ? "bg-indigo-400" : "bg-border")} />}
            <button
              type="button"
              disabled={!reachable}
              onClick={() => reachable && onChange(i)}
              className={cn(
                "rounded-full px-2 py-0.5 text-[11px] font-semibold transition-colors",
                active
                  ? "bg-indigo-600 text-white"
                  : done
                    ? "bg-indigo-100 text-indigo-700 hover:bg-indigo-200"
                    : reachable
                      ? "bg-muted text-muted-foreground hover:bg-muted/80"
                      : "bg-muted/40 text-muted-foreground/40 cursor-not-allowed",
              )}
            >
              <span className="hidden sm:inline">{label}</span>
              <span className="sm:hidden">{i + 1}</span>
            </button>
          </div>
        );
      })}
    </div>
  );
}

function Field({ label, hint, children, className }: { label: string; hint?: string; children: ReactNode; className?: string }) {
  return (
    <label className={cn("block", className)}>
      <span className="mb-1 block text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">{label}</span>
      {children}
      {hint && <span className="mt-1 block text-[11px] text-muted-foreground">{hint}</span>}
    </label>
  );
}

/* ═══════════════════════════════════════════════
   Builder Steps
   ═══════════════════════════════════════════════ */

function RecipeStep({ selected, onSelect }: { selected: AlarmTemplateKind; onSelect: (id: AlarmTemplateKind) => void }) {
  return (
    <div>
      <p className="mb-3 text-sm text-muted-foreground">What kind of alarm do you want to create?</p>
      <div className="grid grid-cols-1 gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
        {RECIPES.map((r) => (
          <button
            key={r.id}
            type="button"
            onClick={() => onSelect(r.id)}
            className={cn(
              "flex items-start gap-3 rounded-xl border bg-card p-3.5 text-left transition-all hover:shadow-md",
              r.accent,
              selected === r.id
                ? "ring-2 ring-indigo-500 ring-offset-1 shadow-md"
                : "shadow-sm hover:ring-1 hover:ring-indigo-200",
            )}
          >
            <div className={cn("flex size-9 shrink-0 items-center justify-center rounded-lg", r.iconBg)}>
              {r.icon}
            </div>
            <div className="min-w-0">
              <p className="text-sm font-semibold text-card-foreground">{r.label}</p>
              <p className="text-xs text-muted-foreground">{r.desc}</p>
              <p className="mt-1 text-[11px] italic text-muted-foreground/70">{r.hint}</p>
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}

function TargetStep({ form, onPatch, sensors, nodes }: { form: AlarmWizardState; onPatch: (p: Partial<AlarmWizardState>) => void; sensors: DemoSensor[]; nodes: DemoNode[] }) {
  const modes = [
    { value: "sensor" as const, label: "Single sensor" },
    { value: "node" as const, label: "All on a node" },
    { value: "filter" as const, label: "By type / provider" },
  ];

  return (
    <div>
      <p className="mb-3 text-sm text-muted-foreground">What should this alarm monitor?</p>
      <div className="mb-4 flex flex-wrap gap-2">
        {modes.map((m) => (
          <button
            key={m.value}
            type="button"
            onClick={() => onPatch({ selectorMode: m.value })}
            className={cn(
              "rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors",
              form.selectorMode === m.value
                ? "border-indigo-300 bg-indigo-50 text-indigo-700"
                : "border-border text-muted-foreground hover:bg-muted",
            )}
          >
            {m.label}
          </button>
        ))}
      </div>

      {form.selectorMode === "sensor" && (
        <Field label="Sensor">
          <Select value={form.sensorId} onChange={(e) => onPatch({ sensorId: e.target.value })}>
            <option value="">Select a sensor\u2026</option>
            {sensors.map((s) => (
              <option key={s.sensor_id} value={s.sensor_id}>
                {s.name} ({s.type}{s.unit ? `, ${s.unit}` : ""})
              </option>
            ))}
          </Select>
        </Field>
      )}
      {form.selectorMode === "node" && (
        <Field label="Node">
          <Select value={form.nodeId} onChange={(e) => onPatch({ nodeId: e.target.value })}>
            <option value="">Select a node\u2026</option>
            {nodes.map((n) => (
              <option key={n.id} value={n.id}>
                {n.name} ({n.status})
              </option>
            ))}
          </Select>
        </Field>
      )}
      {form.selectorMode === "filter" && (
        <div className="grid gap-3 sm:grid-cols-3">
          <Field label="Provider" hint="e.g. emporia, renogy">
            <Input value={form.filterProvider} onChange={(e) => onPatch({ filterProvider: e.target.value })} placeholder="any" />
          </Field>
          <Field label="Metric" hint="e.g. voltage_v, power_w">
            <Input value={form.filterMetric} onChange={(e) => onPatch({ filterMetric: e.target.value })} placeholder="any" />
          </Field>
          <Field label="Sensor type" hint="e.g. analog, digital">
            <Input value={form.filterType} onChange={(e) => onPatch({ filterType: e.target.value })} placeholder="any" />
          </Field>
        </div>
      )}
    </div>
  );
}

function ConditionStep({ form, onPatch }: { form: AlarmWizardState; onPatch: (p: Partial<AlarmWizardState>) => void }) {
  const t = form.template;

  return (
    <div>
      <p className="mb-3 text-sm text-muted-foreground">Define the trigger condition.</p>

      {t === "threshold" && (
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Operator">
            <Select value={form.thresholdOp} onChange={(e) => onPatch({ thresholdOp: e.target.value as typeof form.thresholdOp })}>
              {OP_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </Select>
          </Field>
          <Field label="Value">
            <Input type="number" step="any" value={form.thresholdValue} onChange={(e) => onPatch({ thresholdValue: e.target.value })} placeholder="15" />
          </Field>
        </div>
      )}

      {t === "range" && (
        <div className="space-y-3">
          <Field label="Alert when value is">
            <div className="flex gap-2">
              {(["outside", "inside"] as const).map((m) => (
                <button key={m} type="button" onClick={() => onPatch({ rangeMode: m })} className={cn(
                  "rounded-lg border px-3 py-1.5 text-xs font-medium transition-colors",
                  form.rangeMode === m ? "border-indigo-300 bg-indigo-50 text-indigo-700" : "border-border text-muted-foreground hover:bg-muted",
                )}>
                  {m === "outside" ? "Outside range" : "Inside range"}
                </button>
              ))}
            </div>
          </Field>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Low bound"><Input type="number" step="any" value={form.rangeLow} onChange={(e) => onPatch({ rangeLow: e.target.value })} placeholder="105" /></Field>
            <Field label="High bound"><Input type="number" step="any" value={form.rangeHigh} onChange={(e) => onPatch({ rangeHigh: e.target.value })} placeholder="130" /></Field>
          </div>
        </div>
      )}

      {t === "offline" && (
        <Field label="Alert after no data for (seconds)">
          <Input type="number" value={form.offlineSeconds} onChange={(e) => onPatch({ offlineSeconds: e.target.value })} placeholder="300" />
        </Field>
      )}

      {t === "rolling_window" && (
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Window (seconds)"><Input type="number" value={form.rollingWindowSeconds} onChange={(e) => onPatch({ rollingWindowSeconds: e.target.value })} placeholder="300" /></Field>
          <Field label="Aggregate">
            <Select value={form.rollingAggregate} onChange={(e) => onPatch({ rollingAggregate: e.target.value as typeof form.rollingAggregate })}>
              {AGG_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </Select>
          </Field>
          <Field label="Operator">
            <Select value={form.rollingOp} onChange={(e) => onPatch({ rollingOp: e.target.value as typeof form.rollingOp })}>
              {OP_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
            </Select>
          </Field>
          <Field label="Value"><Input type="number" step="any" value={form.rollingValue} onChange={(e) => onPatch({ rollingValue: e.target.value })} placeholder="5" /></Field>
        </div>
      )}

      {t === "deviation" && (
        <div className="grid gap-3 sm:grid-cols-2">
          <Field label="Window (seconds)"><Input type="number" value={form.deviationWindowSeconds} onChange={(e) => onPatch({ deviationWindowSeconds: e.target.value })} placeholder="300" /></Field>
          <Field label="Baseline">
            <Select value={form.deviationBaseline} onChange={(e) => onPatch({ deviationBaseline: e.target.value as typeof form.deviationBaseline })}>
              <option value="mean">Mean</option>
              <option value="median">Median</option>
            </Select>
          </Field>
          <Field label="Deviation mode">
            <Select value={form.deviationMode} onChange={(e) => onPatch({ deviationMode: e.target.value as typeof form.deviationMode })}>
              <option value="percent">Percent (%)</option>
              <option value="absolute">Absolute</option>
            </Select>
          </Field>
          <Field label="Deviation threshold"><Input type="number" step="any" value={form.deviationValue} onChange={(e) => onPatch({ deviationValue: e.target.value })} placeholder="10" /></Field>
        </div>
      )}

      {t === "consecutive" && (
        <div className="space-y-3">
          <p className="text-xs text-muted-foreground">Fires when a threshold holds for multiple consecutive periods.</p>
          <div className="grid gap-3 sm:grid-cols-2">
            <Field label="Operator">
              <Select value={form.thresholdOp} onChange={(e) => onPatch({ thresholdOp: e.target.value as typeof form.thresholdOp })}>
                {OP_OPTIONS.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
              </Select>
            </Field>
            <Field label="Value"><Input type="number" step="any" value={form.thresholdValue} onChange={(e) => onPatch({ thresholdValue: e.target.value })} placeholder="12" /></Field>
            <Field label="Period type">
              <Select value={form.consecutivePeriod} onChange={(e) => onPatch({ consecutivePeriod: e.target.value as typeof form.consecutivePeriod })}>
                <option value="eval">Evaluation cycle</option>
                <option value="hour">Hour</option>
                <option value="day">Day</option>
              </Select>
            </Field>
            <Field label="Consecutive count"><Input type="number" value={form.consecutiveCount} onChange={(e) => onPatch({ consecutiveCount: e.target.value })} placeholder="3" /></Field>
          </div>
        </div>
      )}
    </div>
  );
}

function DetailsStep({ form, onPatch, suggestedName }: { form: AlarmWizardState; onPatch: (p: Partial<AlarmWizardState>) => void; suggestedName: string }) {
  return (
    <div className="space-y-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Rule name">
          <Input value={form.name} onChange={(e) => onPatch({ name: e.target.value })} placeholder={suggestedName} />
          {!form.name && suggestedName && (
            <button type="button" onClick={() => onPatch({ name: suggestedName })} className="mt-1 text-xs text-indigo-600 hover:underline">
              Use suggestion: {suggestedName}
            </button>
          )}
        </Field>
        <Field label="Severity">
          <div className="flex gap-2">
            {(["info", "warning", "critical"] as const).map((sev) => {
              const cfg = SEVERITY_CFG[sev];
              return (
                <button
                  key={sev}
                  type="button"
                  onClick={() => onPatch({ severity: sev })}
                  className={cn(
                    "flex-1 rounded-lg border px-3 py-2.5 text-center transition-colors",
                    form.severity === sev
                      ? sev === "critical"
                        ? "border-rose-300 bg-rose-50 text-rose-700"
                        : sev === "warning"
                          ? "border-amber-300 bg-amber-50 text-amber-700"
                          : "border-sky-300 bg-sky-50 text-sky-700"
                      : "border-border text-muted-foreground hover:bg-muted",
                  )}
                >
                  <div className="text-xs font-bold">{cfg.label}</div>
                  <div className="mt-0.5 text-[10px] opacity-70">{cfg.hint}</div>
                </button>
              );
            })}
          </div>
        </Field>
      </div>

      <Field label="Description (optional)">
        <Input value={form.description} onChange={(e) => onPatch({ description: e.target.value })} placeholder="Why does this alarm matter?" />
      </Field>

      <Field label="Message template (optional)" hint="Shown when alarm fires. Use {{value}} for the current reading.">
        <Input value={form.messageTemplate} onChange={(e) => onPatch({ messageTemplate: e.target.value })} placeholder="{{sensor}} reading is {{value}}" />
      </Field>

      <details className="group">
        <summary className="cursor-pointer select-none text-[11px] font-semibold uppercase tracking-wider text-muted-foreground hover:text-foreground">
          Advanced timing &#x25B8;
        </summary>
        <div className="mt-3 grid gap-3 sm:grid-cols-3">
          <Field label="Debounce (s)" hint="Delay before firing">
            <Input type="number" value={form.debounceSeconds} onChange={(e) => onPatch({ debounceSeconds: e.target.value })} />
          </Field>
          <Field label="Clear hysteresis (s)" hint="Delay before clearing">
            <Input type="number" value={form.clearHysteresisSeconds} onChange={(e) => onPatch({ clearHysteresisSeconds: e.target.value })} />
          </Field>
          <Field label="Eval interval (s)" hint="0 = system default">
            <Input type="number" value={form.evalIntervalSeconds} onChange={(e) => onPatch({ evalIntervalSeconds: e.target.value })} />
          </Field>
        </div>
      </details>
    </div>
  );
}

function ReviewStep({
  form, sensors, nodes, preview, previewLoading, onPreview, onSave, saving,
}: {
  form: AlarmWizardState; sensors: DemoSensor[]; nodes: DemoNode[];
  preview: AlarmRulePreviewResponse | null; previewLoading: boolean;
  onPreview: () => void; onSave: () => void; saving: boolean;
}) {
  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-indigo-200 bg-indigo-50/50 p-4">
        <p className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-indigo-600">Alarm Summary</p>
        <p className="text-sm leading-relaxed text-card-foreground">
          When <span className="font-semibold text-indigo-700">{targetLabel(form, sensors, nodes)}</span>{" "}
          has <span className="font-semibold text-indigo-700">{condLabel(form)}</span>,{" "}
          raise a <Badge tone={sevTone(form.severity)} size="sm">{form.severity}</Badge> alarm
          {form.name ? <> named <span className="font-semibold">&ldquo;{form.name}&rdquo;</span></> : null}.
        </p>
      </div>

      <div className="flex items-center justify-between">
        <p className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">Live Preview</p>
        <Button size="xs" onClick={onPreview} loading={previewLoading}>Test now</Button>
      </div>

      {preview && (
        <div className="rounded-xl border bg-card p-3">
          <p className="mb-2 text-xs text-muted-foreground">{preview.targets_evaluated} target(s) evaluated</p>
          {preview.results.length > 0 ? (
            <div className="space-y-1">
              {preview.results.slice(0, 20).map((r, i) => (
                <div key={i} className={cn("flex items-center justify-between rounded-lg px-3 py-1.5 text-xs", r.passed ? "bg-rose-50 text-rose-700" : "bg-emerald-50 text-emerald-700")}>
                  <span className="font-mono text-[11px]">{r.target_key}</span>
                  <span>
                    {r.observed_value != null && <span className="mr-2 font-mono">{Number(r.observed_value).toFixed(2)}</span>}
                    {r.passed ? "WOULD FIRE" : "OK"}
                  </span>
                </div>
              ))}
              {preview.results.length > 20 && <p className="text-xs text-muted-foreground">\u2026and {preview.results.length - 20} more</p>}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground">No targets matched the selector.</p>
          )}
        </div>
      )}

      <div className="flex justify-end pt-2">
        <Button variant="primary" onClick={onSave} loading={saving}>
          {form.mode === "edit" ? "Update alarm rule" : "Create alarm rule"}
        </Button>
      </div>
    </div>
  );
}

/* ═══════════════════════════════════════════════
   Rules Table
   ═══════════════════════════════════════════════ */

function RulesTable({
  rules, expandedId, onExpand, canEdit, onEdit, onToggle, onDelete, search, onSearchChange, sevFilter, onSevFilterChange,
}: {
  rules: AlarmRule[]; expandedId: number | null; onExpand: (id: number | null) => void;
  canEdit: boolean; onEdit: (r: AlarmRule) => void; onToggle: (r: AlarmRule) => void; onDelete: (r: AlarmRule) => void;
  search: string; onSearchChange: (s: string) => void;
  sevFilter: "all" | AlarmSeverity; onSevFilterChange: (s: "all" | AlarmSeverity) => void;
}) {
  const filtered = useMemo(() => {
    let list = rules;
    if (search) {
      const q = search.toLowerCase();
      list = list.filter(
        (r) =>
          r.name.toLowerCase().includes(q) ||
          r.description?.toLowerCase().includes(q) ||
          describeCondition(r.condition_ast).toLowerCase().includes(q),
      );
    }
    if (sevFilter !== "all") list = list.filter((r) => r.severity === sevFilter);
    return list;
  }, [rules, search, sevFilter]);

  return (
    <div className="rounded-xl border bg-card shadow-sm">
      {/* Toolbar */}
      <div className="flex flex-wrap items-center gap-3 border-b border-border px-4 py-3">
        <div className="relative min-w-[180px] flex-1">
          <svg className="absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <circle cx="11" cy="11" r="8" /><path d="M21 21l-4.35-4.35" strokeLinecap="round" />
          </svg>
          <Input className="pl-9" value={search} onChange={(e) => onSearchChange(e.target.value)} placeholder="Search rules\u2026" />
        </div>
        <Select className="w-auto" value={sevFilter} onChange={(e) => onSevFilterChange(e.target.value as "all" | AlarmSeverity)}>
          <option value="all">All severities</option>
          <option value="critical">Critical</option>
          <option value="warning">Warning</option>
          <option value="info">Info</option>
        </Select>
        <span className="text-xs text-muted-foreground">{filtered.length} of {rules.length} rules</span>
      </div>

      {/* Rows */}
      {filtered.length === 0 ? (
        <div className="px-4 py-10 text-center">
          {rules.length === 0 ? (
            <div className="space-y-1">
              <p className="text-sm font-medium text-card-foreground">No alarm rules configured</p>
              <p className="text-xs text-muted-foreground">Create your first rule to start monitoring sensor conditions.</p>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">No rules match your search.</p>
          )}
        </div>
      ) : (
        <div className="divide-y divide-border">
          {filtered.map((rule) => {
            const expanded = expandedId === rule.id;
            return (
              <div key={rule.id}>
                {/* Row */}
                <div
                  className={cn("group flex cursor-pointer items-center gap-3 px-4 py-2.5 transition-colors hover:bg-muted/40", expanded && "bg-muted/20")}
                  onClick={() => onExpand(expanded ? null : rule.id)}
                >
                  <svg viewBox="0 0 20 20" fill="currentColor" className={cn("size-4 shrink-0 text-muted-foreground transition-transform", expanded && "rotate-90")}>
                    <path fillRule="evenodd" d="M7.21 14.77a.75.75 0 0 1 .02-1.06L11.19 10 7.23 6.29a.75.75 0 1 1 1.04-1.08l4.54 4.25a.75.75 0 0 1 0 1.08l-4.54 4.25a.75.75 0 0 1-1.06-.02Z" clipRule="evenodd" />
                  </svg>
                  <span className={cn("size-2 shrink-0 rounded-full", !rule.enabled ? "bg-gray-300" : rule.active_count > 0 ? "bg-rose-500" : rule.last_error ? "bg-amber-400" : "bg-emerald-400")} />
                  <span className="min-w-0 flex-1 truncate text-sm font-medium text-card-foreground">{rule.name}</span>
                  <Badge tone={sevTone(rule.severity)} size="sm">{rule.severity}</Badge>
                  <span className="hidden w-52 truncate text-[11px] font-mono text-muted-foreground lg:block">{describeCondition(rule.condition_ast)}</span>
                  {rule.active_count > 0 && <Badge tone="danger" size="sm">{rule.active_count} active</Badge>}
                  {canEdit && (
                    <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100">
                      <button type="button" onClick={(e) => { e.stopPropagation(); onToggle(rule); }} className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground" title={rule.enabled ? "Disable" : "Enable"}>
                        <svg viewBox="0 0 20 20" fill="currentColor" className="size-3.5">
                          {rule.enabled
                            ? <><circle cx="10" cy="10" r="8" fill="none" stroke="currentColor" strokeWidth="1.5" /><rect x="7" y="6" width="6" height="8" rx="0.5" /></>
                            : <><circle cx="10" cy="10" r="8" fill="none" stroke="currentColor" strokeWidth="1.5" /><path d="M8 7l6 3-6 3V7z" /></>}
                        </svg>
                      </button>
                      <button type="button" onClick={(e) => { e.stopPropagation(); onEdit(rule); }} className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground" title="Edit">
                        <svg viewBox="0 0 20 20" fill="currentColor" className="size-3.5"><path d="M2.695 14.763l-1.262 3.154a.5.5 0 00.65.65l3.155-1.262a4 4 0 001.343-.885L17.5 5.5a2.121 2.121 0 00-3-3L3.58 13.42a4 4 0 00-.885 1.343z" /></svg>
                      </button>
                      <button type="button" onClick={(e) => { e.stopPropagation(); if (confirm(`Delete "${rule.name}"?`)) onDelete(rule); }} className="rounded p-1 text-muted-foreground hover:bg-rose-50 hover:text-rose-600" title="Delete">
                        <svg viewBox="0 0 20 20" fill="currentColor" className="size-3.5"><path fillRule="evenodd" d="M8.75 1A2.75 2.75 0 006 3.75v.443c-.795.077-1.584.176-2.365.298a.75.75 0 10.23 1.482l.149-.022.841 10.518A2.75 2.75 0 007.596 19h4.807a2.75 2.75 0 002.742-2.53l.841-10.52.149.023a.75.75 0 00.23-1.482A41.03 41.03 0 0014 4.193V3.75A2.75 2.75 0 0011.25 1h-2.5zM10 4c.84 0 1.673.025 2.5.075V3.75c0-.69-.56-1.25-1.25-1.25h-2.5c-.69 0-1.25.56-1.25 1.25v.325C8.327 4.025 9.16 4 10 4zM8.58 7.72a.75.75 0 00-1.5.06l.3 7.5a.75.75 0 101.5-.06l-.3-7.5zm4.34.06a.75.75 0 10-1.5-.06l-.3 7.5a.75.75 0 101.5.06l.3-7.5z" clipRule="evenodd" /></svg>
                      </button>
                    </div>
                  )}
                </div>
                {/* Expanded detail */}
                {expanded && (
                  <div className="border-t border-dashed border-border bg-[hsl(var(--card-inset))] px-4 py-3">
                    <div className="grid gap-x-8 gap-y-2 text-xs sm:grid-cols-2">
                      <div><span className="font-semibold text-muted-foreground">Target: </span><span className="font-mono text-[11px]">{JSON.stringify(rule.target_selector)}</span></div>
                      <div><span className="font-semibold text-muted-foreground">Condition: </span>{describeCondition(rule.condition_ast)}</div>
                      <div><span className="font-semibold text-muted-foreground">Timing: </span>debounce {rule.timing?.debounce_seconds ?? 0}s &middot; hysteresis {rule.timing?.clear_hysteresis_seconds ?? 0}s &middot; eval {rule.timing?.eval_interval_seconds ?? 0}s</div>
                      <div><span className="font-semibold text-muted-foreground">Origin: </span>{rule.origin}</div>
                      {rule.description && <div className="sm:col-span-2"><span className="font-semibold text-muted-foreground">Description: </span>{rule.description}</div>}
                      {rule.last_eval_at && <div><span className="font-semibold text-muted-foreground">Last eval: </span>{new Date(rule.last_eval_at).toLocaleString()}</div>}
                      {rule.last_error && <div className="text-rose-600 sm:col-span-2"><span className="font-semibold">Error: </span>{rule.last_error}</div>}
                      {rule.message_template && <div className="sm:col-span-2"><span className="font-semibold text-muted-foreground">Message: </span>{rule.message_template}</div>}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/* ═══════════════════════════════════════════════
   Active Alarms Banner
   ═══════════════════════════════════════════════ */

function ActiveAlarmsBanner({ alarms }: { alarms: DemoAlarm[] }) {
  const active = useMemo(() => alarms.filter((a) => a.status === "active"), [alarms]);
  if (active.length === 0) return null;

  return (
    <div className="rounded-xl border border-rose-200 bg-rose-50/50 px-4 py-3 shadow-sm">
      <div className="mb-2 flex items-center gap-2">
        <span className="relative flex size-2.5">
          <span className="absolute inline-flex size-full animate-ping rounded-full bg-rose-400 opacity-75" />
          <span className="relative inline-flex size-2.5 rounded-full bg-rose-500" />
        </span>
        <span className="text-[11px] font-bold uppercase tracking-wider text-rose-700">
          {active.length} Active Alarm{active.length !== 1 ? "s" : ""}
        </span>
      </div>
      <div className="flex flex-wrap gap-2">
        {active.map((alarm) => (
          <div key={alarm.id} className="flex items-center gap-2 rounded-lg border border-rose-200 bg-white px-3 py-1.5 text-xs">
            <Badge tone={sevTone(alarm.severity)} size="sm">{alarm.severity}</Badge>
            <span className="font-medium text-card-foreground">{alarm.name}</span>
            <span className="text-muted-foreground">{alarm.origin ?? "threshold"}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ═══════════════════════════════════════════════
   Recent Events
   ═══════════════════════════════════════════════ */

function RecentEvents() {
  const { data: events = [] } = useAlarmEventsQuery(50);
  const typed = events as DemoAlarmEvent[];
  if (typed.length === 0) return null;

  return (
    <div className="rounded-xl border bg-card shadow-sm">
      <div className="border-b border-border px-4 py-3">
        <h3 className="text-sm font-semibold text-card-foreground">Recent Events</h3>
        <p className="text-xs text-muted-foreground">Last 50 alarm events</p>
      </div>
      <div className="max-h-64 divide-y divide-border overflow-y-auto">
        {typed.slice(0, 50).map((ev, i) => (
          <div key={ev.id ?? i} className="flex items-center gap-3 px-4 py-2 text-xs">
            <Badge tone={sevTone(ev.status === "active" ? "warning" : "info")} size="sm">{ev.status}</Badge>
            <span className="min-w-0 flex-1 truncate text-card-foreground">{ev.message ?? ev.origin ?? "\u2014"}</span>
            <span className="shrink-0 font-mono text-[11px] text-muted-foreground">{ev.transition ?? "\u2014"}</span>
            <span className="shrink-0 text-muted-foreground">{ev.raised_at ? new Date(ev.raised_at).toLocaleString() : "\u2014"}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ═══════════════════════════════════════════════
   Main Page
   ═══════════════════════════════════════════════ */

export default function Alarms2PageClient() {
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));

  const rulesQ = useAlarmRulesQuery();
  const alarmsQ = useAlarmsQuery();
  const nodesQ = useNodesQuery();
  const sensorsQ = useSensorsQuery();
  const mutations = useAlarmMutations();

  /* Builder state */
  const [builderOpen, setBuilderOpen] = useState(false);
  const [step, setStep] = useState(0);
  const [maxStep, setMaxStep] = useState(0);
  const [form, setForm] = useState<AlarmWizardState>(DEFAULT_FORM);
  const [preview, setPreview] = useState<AlarmRulePreviewResponse | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);

  /* Table state */
  const [expandedId, setExpandedId] = useState<number | null>(null);
  const [search, setSearch] = useState("");
  const [sevFilter, setSevFilter] = useState<"all" | AlarmSeverity>("all");

  /* Feedback */
  const [msg, setMsg] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [saving, setSaving] = useState(false);

  /* Derived data */
  const isLoading = rulesQ.isLoading || alarmsQ.isLoading || nodesQ.isLoading || sensorsQ.isLoading;
  const error = rulesQ.error || alarmsQ.error;

  const rules = useMemo(() => {
    const list = (rulesQ.data ?? []) as AlarmRule[];
    const ord: Record<string, number> = { critical: 0, warning: 1, info: 2 };
    return [...list].sort((a, b) => {
      const d = (ord[a.severity] ?? 99) - (ord[b.severity] ?? 99);
      return d !== 0 ? d : a.name.localeCompare(b.name);
    });
  }, [rulesQ.data]);

  const alarms = useMemo(() => (alarmsQ.data ?? []) as DemoAlarm[], [alarmsQ.data]);
  const sensors = (sensorsQ.data ?? []) as DemoSensor[];
  const nodes = (nodesQ.data ?? []) as DemoNode[];
  const activeCount = useMemo(() => alarms.filter((a) => a.status === "active").length, [alarms]);

  /* Builder actions */
  const patchForm = useCallback((p: Partial<AlarmWizardState>) => {
    setForm((prev) => {
      const next = { ...prev, ...p };
      if (!next.advancedMode) next.advancedJson = buildAdvancedJson(next);
      return next;
    });
  }, []);

  const goStep = useCallback((s: number) => {
    setStep(s);
    setMaxStep((prev) => Math.max(prev, s));
  }, []);

  const openCreate = () => {
    setForm(DEFAULT_FORM);
    setStep(0);
    setMaxStep(0);
    setPreview(null);
    setBuilderOpen(true);
  };

  const openEdit = (rule: AlarmRule) => {
    setForm(parseRuleToForm(rule));
    setStep(0);
    setMaxStep(4);
    setPreview(null);
    setBuilderOpen(true);
  };

  const closeBuilder = () => setBuilderOpen(false);

  const canNext = useMemo(() => {
    if (step === 0) return true;
    if (step === 1) {
      if (form.selectorMode === "sensor") return form.sensorId.trim().length > 0;
      if (form.selectorMode === "node") return form.nodeId.trim().length > 0;
      return form.filterProvider.trim().length > 0 || form.filterMetric.trim().length > 0 || form.filterType.trim().length > 0;
    }
    if (step === 2) {
      if (form.template === "threshold") return form.thresholdValue.trim().length > 0;
      if (form.template === "range") return form.rangeLow.trim().length > 0 && form.rangeHigh.trim().length > 0;
      if (form.template === "offline") return form.offlineSeconds.trim().length > 0;
      if (form.template === "rolling_window") return form.rollingValue.trim().length > 0;
      if (form.template === "deviation") return form.deviationValue.trim().length > 0;
      if (form.template === "consecutive") return form.thresholdValue.trim().length > 0 && form.consecutiveCount.trim().length > 0;
      return true;
    }
    if (step === 3) return form.name.trim().length > 0;
    return true;
  }, [step, form]);

  const handlePreview = async () => {
    setPreviewLoading(true);
    try {
      const result = await mutations.preview(buildRequestFromWizard(form));
      setPreview(result);
    } catch { setPreview(null); }
    finally { setPreviewLoading(false); }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const payload = buildRequestFromWizard(form);
      if (form.mode === "edit" && form.ruleId != null) {
        await mutations.update(form.ruleId, payload);
        setMsg({ type: "success", text: "Alarm rule updated." });
      } else {
        await mutations.create(payload);
        setMsg({ type: "success", text: "Alarm rule created." });
      }
      closeBuilder();
    } catch (err) {
      setMsg({ type: "error", text: err instanceof Error ? err.message : "Failed to save alarm rule." });
    } finally {
      setSaving(false);
    }
  };

  const handleToggle = async (rule: AlarmRule) => {
    try {
      if (rule.enabled) await mutations.disable(rule.id);
      else await mutations.enable(rule.id);
      setMsg({ type: "success", text: `Rule ${rule.enabled ? "disabled" : "enabled"}.` });
    } catch (err) {
      setMsg({ type: "error", text: err instanceof Error ? err.message : "Failed." });
    }
  };

  const handleDelete = async (rule: AlarmRule) => {
    try {
      await mutations.delete(rule.id);
      setMsg({ type: "success", text: "Rule deleted." });
    } catch (err) {
      setMsg({ type: "error", text: err instanceof Error ? err.message : "Failed." });
    }
  };

  if (isLoading) return <LoadingState label="Loading alarms\u2026" />;
  if (error) return <ErrorState message={error instanceof Error ? error.message : "Failed to load."} />;

  const suggested = suggestName(form, sensors, nodes);
  const liveSentence = step >= 1
    ? `When ${targetLabel(form, sensors, nodes)} ${step >= 2 ? condLabel(form) : "\u2026"} \u2192 ${form.severity}`
    : null;

  return (
    <div className="space-y-4">
      <PageHeaderCard
        title="Alarm Rules"
        description="Configure conditional and threshold alarms. Monitor active incidents and review event history."
        actions={
          canEdit && !builderOpen ? (
            <Button variant="primary" onClick={openCreate}>+ New alarm rule</Button>
          ) : undefined
        }
      />

      {msg && (
        <InlineBanner tone={msg.type === "success" ? "success" : "error"}>
          <div className="flex items-center justify-between">
            <span>{msg.text}</span>
            <button type="button" className="ml-4 text-xs underline" onClick={() => setMsg(null)}>dismiss</button>
          </div>
        </InlineBanner>
      )}

      <StatusStrip rules={rules} activeCount={activeCount} />
      <ActiveAlarmsBanner alarms={alarms} />

      {/* ── Inline Builder ─────────────────────── */}
      {builderOpen && (
        <Card className="border-indigo-200 shadow-md">
          {/* Header */}
          <div className="flex flex-wrap items-center justify-between gap-3 px-4">
            <div className="flex items-center gap-3">
              <h3 className="text-sm font-bold uppercase tracking-wider text-indigo-700">
                {form.mode === "edit" ? "Edit Rule" : "New Rule"}
              </h3>
              <StepBar current={step} onChange={goStep} maxReached={maxStep} />
            </div>
            <Button size="xs" variant="ghost" onClick={closeBuilder}>Cancel</Button>
          </div>

          {/* Body */}
          <div className="border-t border-border px-4 py-4">
            {step === 0 && <RecipeStep selected={form.template} onSelect={(id) => { patchForm({ template: id }); goStep(1); }} />}
            {step === 1 && <TargetStep form={form} onPatch={patchForm} sensors={sensors} nodes={nodes} />}
            {step === 2 && <ConditionStep form={form} onPatch={patchForm} />}
            {step === 3 && <DetailsStep form={form} onPatch={patchForm} suggestedName={suggested} />}
            {step === 4 && <ReviewStep form={form} sensors={sensors} nodes={nodes} preview={preview} previewLoading={previewLoading} onPreview={handlePreview} onSave={handleSave} saving={saving} />}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between border-t border-border px-4 py-3">
            <div className="min-w-0 flex-1">
              {liveSentence && <p className="truncate text-xs italic text-muted-foreground">{liveSentence}</p>}
            </div>
            <div className="flex shrink-0 items-center gap-2">
              {step > 0 && <Button size="xs" onClick={() => goStep(step - 1)}>&larr; Back</Button>}
              {step < 4 && <Button size="xs" variant="primary" onClick={() => canNext && goStep(step + 1)} disabled={!canNext}>Next &rarr;</Button>}
            </div>
          </div>
        </Card>
      )}

      {/* ── Rules Table ────────────────────────── */}
      <RulesTable
        rules={rules}
        expandedId={expandedId}
        onExpand={setExpandedId}
        canEdit={canEdit}
        onEdit={openEdit}
        onToggle={(r) => void handleToggle(r)}
        onDelete={(r) => void handleDelete(r)}
        search={search}
        onSearchChange={setSearch}
        sevFilter={sevFilter}
        onSevFilterChange={setSevFilter}
      />

      {/* ── Recent Events ──────────────────────── */}
      <RecentEvents />
    </div>
  );
}
