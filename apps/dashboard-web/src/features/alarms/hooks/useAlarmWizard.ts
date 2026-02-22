import { useMemo, useState } from "react";
import type { AlarmRule, AlarmWizardState } from "@/features/alarms/types/alarmTypes";
import { buildAdvancedJson } from "@/features/alarms/utils/ruleBuilder";

const defaultState: AlarmWizardState = {
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
  offlineSeconds: "5",
  rollingWindowSeconds: "300",
  rollingAggregate: "avg",
  rollingOp: "lt",
  rollingValue: "",
  deviationWindowSeconds: "300",
  deviationBaseline: "mean",
  deviationMode: "percent",
  deviationValue: "",
  consecutivePeriod: "day",
  consecutiveCount: "2",
  debounceSeconds: "0",
  clearHysteresisSeconds: "0",
  evalIntervalSeconds: "0",
  messageTemplate: "",
  advancedJson: "",
  advancedMode: false,
};

const parseNumberString = (value: number | null | undefined): string => {
  if (value == null || !Number.isFinite(value)) return "0";
  return String(Math.round(value * 1_000_000) / 1_000_000);
};

function wizardStateFromRule(rule: AlarmRule, mode: "edit" | "create"): AlarmWizardState {
  const next: AlarmWizardState = {
    ...defaultState,
    mode,
    ...(mode === "edit" ? { ruleId: rule.id } : {}),
    name: mode === "create" ? `Copy of ${rule.name}` : rule.name,
    description: rule.description,
    severity: rule.severity,
    origin: rule.origin,
    messageTemplate: rule.message_template,
    debounceSeconds: parseNumberString(rule.timing?.debounce_seconds ?? 0),
    clearHysteresisSeconds: parseNumberString(rule.timing?.clear_hysteresis_seconds ?? 0),
    evalIntervalSeconds: parseNumberString(rule.timing?.eval_interval_seconds ?? 0),
  };

  let supported = true;

  // Target selector mapping (only when round-trippable)
  const selector = rule.target_selector;
  if (selector.kind === "sensor") {
    next.selectorMode = "sensor";
    next.sensorId = selector.sensor_id;
  } else if (selector.kind === "node_sensors") {
    const match = selector.match ?? "per_sensor";
    const types = selector.types ?? [];
    if (match !== "per_sensor" || types.length > 0) {
      supported = false;
    } else {
      next.selectorMode = "node";
      next.nodeId = selector.node_id;
    }
  } else if (selector.kind === "filter") {
    const match = selector.match ?? "per_sensor";
    if (match !== "per_sensor") {
      supported = false;
    } else {
      next.selectorMode = "filter";
      next.filterProvider = selector.provider ?? "";
      next.filterMetric = selector.metric ?? "";
      next.filterType = selector.sensor_type ?? "";
    }
  } else {
    supported = false;
  }

  // Condition mapping (only when round-trippable)
  const condition = rule.condition_ast;
  if (condition.type === "threshold") {
    next.template = "threshold";
    next.thresholdOp = condition.op;
    next.thresholdValue = parseNumberString(condition.value);
  } else if (condition.type === "range") {
    next.template = "range";
    next.rangeMode = condition.mode;
    next.rangeLow = parseNumberString(condition.low);
    next.rangeHigh = parseNumberString(condition.high);
  } else if (condition.type === "offline") {
    next.template = "offline";
    next.offlineSeconds = parseNumberString(condition.missing_for_seconds);
  } else if (condition.type === "rolling_window") {
    next.template = "rolling_window";
    next.rollingWindowSeconds = parseNumberString(condition.window_seconds);
    next.rollingAggregate = condition.aggregate;
    next.rollingOp = condition.op;
    next.rollingValue = parseNumberString(condition.value);
  } else if (condition.type === "deviation") {
    next.template = "deviation";
    next.deviationWindowSeconds = parseNumberString(condition.window_seconds);
    next.deviationBaseline = condition.baseline;
    next.deviationMode = condition.mode;
    next.deviationValue = parseNumberString(condition.value);
  } else if (condition.type === "consecutive_periods") {
    if (condition.child.type !== "threshold") {
      supported = false;
    } else {
      next.template = "consecutive";
      next.consecutivePeriod = condition.period;
      next.consecutiveCount = parseNumberString(condition.count);
      next.thresholdOp = condition.child.op;
      next.thresholdValue = parseNumberString(condition.child.value);
    }
  } else {
    supported = false;
  }

  if (supported) {
    next.advancedMode = false;
    next.advancedJson = buildAdvancedJson(next);
  } else {
    next.advancedMode = true;
    next.advancedJson = JSON.stringify(
      {
        target_selector: rule.target_selector,
        condition_ast: rule.condition_ast,
        timing: rule.timing,
      },
      null,
      2,
    );
  }

  return next;
}

export default function useAlarmWizard() {
  const [open, setOpen] = useState(false);
  const [step, setStep] = useState(1);
  const [state, setState] = useState<AlarmWizardState>(defaultState);

  const reset = () => {
    setStep(1);
    setState(defaultState);
  };

  const openCreate = () => {
    reset();
    setOpen(true);
  };

  const openEdit = (rule: AlarmRule) => {
    const next = wizardStateFromRule(rule, "edit");
    setStep(1);
    setState(next);
    setOpen(true);
  };

  const openDuplicate = (rule: AlarmRule) => {
    const next = wizardStateFromRule(rule, "create");
    setStep(1);
    setState(next);
    setOpen(true);
  };

  const close = () => setOpen(false);

  const patch = (partial: Partial<AlarmWizardState>) => {
    setState((prev) => {
      const next = { ...prev, ...partial };
      if (!next.advancedMode) {
        next.advancedJson = buildAdvancedJson(next);
      }
      return next;
    });
  };

  const canAdvance = useMemo(() => {
    if (step === 1) {
      return state.name.trim().length > 0;
    }
    if (step === 2) {
      if (state.advancedMode) return state.advancedJson.trim().length > 0;
      if (state.selectorMode === "sensor") return state.sensorId.trim().length > 0;
      if (state.selectorMode === "node") return state.nodeId.trim().length > 0;
      return (
        state.filterProvider.trim().length > 0 ||
        state.filterMetric.trim().length > 0 ||
        state.filterType.trim().length > 0
      );
    }
    return true;
  }, [state, step]);

  return {
    open,
    setOpen,
    step,
    setStep,
    state,
    patch,
    reset,
    openCreate,
    openEdit,
    openDuplicate,
    close,
    canAdvance,
  };
}
