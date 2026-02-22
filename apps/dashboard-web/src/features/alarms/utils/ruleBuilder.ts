import type {
  AlarmRuleCreateRequest,
  AlarmRuleTiming,
  AlarmTemplateKind,
  AlarmWizardState,
  ConditionNode,
  TargetSelector,
} from "@/features/alarms/types/alarmTypes";

const asNumber = (value: string, fallback = 0): number => {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
};

const toSelector = (state: AlarmWizardState): TargetSelector => {
  if (state.selectorMode === "sensor") {
    return {
      kind: "sensor",
      sensor_id: state.sensorId,
    };
  }
  if (state.selectorMode === "node") {
    return {
      kind: "node_sensors",
      node_id: state.nodeId,
      match: "per_sensor",
      types: [],
    };
  }
  return {
    kind: "filter",
    provider: state.filterProvider || undefined,
    metric: state.filterMetric || undefined,
    sensor_type: state.filterType || undefined,
    match: "per_sensor",
  };
};

const buildPrimitiveCondition = (state: AlarmWizardState): ConditionNode => {
  const template: AlarmTemplateKind = state.template;
  if (template === "threshold") {
    return {
      type: "threshold",
      op: state.thresholdOp,
      value: asNumber(state.thresholdValue),
    };
  }
  if (template === "range") {
    return {
      type: "range",
      mode: state.rangeMode,
      low: asNumber(state.rangeLow),
      high: asNumber(state.rangeHigh),
    };
  }
  if (template === "offline") {
    return {
      type: "offline",
      missing_for_seconds: Math.max(1, Math.floor(asNumber(state.offlineSeconds, 5))),
    };
  }
  if (template === "rolling_window") {
    return {
      type: "rolling_window",
      window_seconds: Math.max(1, Math.floor(asNumber(state.rollingWindowSeconds, 300))),
      aggregate: state.rollingAggregate,
      op: state.rollingOp,
      value: asNumber(state.rollingValue),
    };
  }
  if (template === "deviation") {
    return {
      type: "deviation",
      window_seconds: Math.max(1, Math.floor(asNumber(state.deviationWindowSeconds, 300))),
      baseline: state.deviationBaseline,
      mode: state.deviationMode,
      value: Math.max(0, asNumber(state.deviationValue)),
    };
  }
  const child: ConditionNode = {
    type: "threshold",
    op: state.thresholdOp,
    value: asNumber(state.thresholdValue),
  };
  return {
    type: "consecutive_periods",
    period: state.consecutivePeriod,
    count: Math.max(1, Math.floor(asNumber(state.consecutiveCount, 2))),
    child,
  };
};

const buildTiming = (state: AlarmWizardState): AlarmRuleTiming => ({
  debounce_seconds: Math.max(0, Math.floor(asNumber(state.debounceSeconds, 0))),
  clear_hysteresis_seconds: Math.max(0, Math.floor(asNumber(state.clearHysteresisSeconds, 0))),
  eval_interval_seconds: Math.max(0, Math.floor(asNumber(state.evalIntervalSeconds, 0))),
});

export function buildRequestFromWizard(state: AlarmWizardState): AlarmRuleCreateRequest {
  if (state.advancedMode) {
    const parsed = JSON.parse(state.advancedJson) as {
      target_selector: TargetSelector;
      condition_ast: ConditionNode;
      timing?: AlarmRuleTiming;
    };
    return {
      name: state.name.trim(),
      description: state.description.trim(),
      enabled: true,
      severity: state.severity,
      origin: state.origin.trim() || "threshold",
      target_selector: parsed.target_selector,
      condition_ast: parsed.condition_ast,
      timing: parsed.timing ?? buildTiming(state),
      message_template: state.messageTemplate.trim(),
    };
  }

  return {
    name: state.name.trim(),
    description: state.description.trim(),
    enabled: true,
    severity: state.severity,
    origin: state.origin.trim() || "threshold",
    target_selector: toSelector(state),
    condition_ast: buildPrimitiveCondition(state),
    timing: buildTiming(state),
    message_template: state.messageTemplate.trim(),
  };
}

export function buildAdvancedJson(state: AlarmWizardState): string {
  const payload = {
    target_selector: toSelector(state),
    condition_ast: buildPrimitiveCondition(state),
    timing: buildTiming(state),
  };
  return JSON.stringify(payload, null, 2);
}
