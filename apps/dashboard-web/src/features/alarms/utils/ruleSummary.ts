import type { AlarmRule, AlarmWizardState, ConditionNode, TargetSelector } from "@/features/alarms/types/alarmTypes";

const describeSelector = (selector: TargetSelector): string => {
  if (selector.kind === "sensor") {
    return `sensor ${selector.sensor_id}`;
  }
  if (selector.kind === "sensor_set") {
    return `sensor set (${selector.sensor_ids.length})`;
  }
  if (selector.kind === "node_sensors") {
    return `node ${selector.node_id} sensors`;
  }
  return "filtered sensors";
};

export const describeCondition = (node: ConditionNode): string => {
  switch (node.type) {
    case "threshold":
      return `value ${node.op} ${node.value}`;
    case "range":
      return node.mode === "inside"
        ? `value inside [${node.low}, ${node.high}]`
        : `value outside [${node.low}, ${node.high}]`;
    case "offline":
      return `no data for ${node.missing_for_seconds}s`;
    case "rolling_window":
      return `${node.aggregate} over ${node.window_seconds}s ${node.op} ${node.value}`;
    case "deviation":
      return `deviation (${node.mode}) from ${node.baseline} over ${node.window_seconds}s >= ${node.value}`;
    case "consecutive_periods":
      return `${describeCondition(node.child)} for ${node.count} consecutive ${node.period}(s)`;
    case "all":
      return node.children.map(describeCondition).join(" and ");
    case "any":
      return node.children.map(describeCondition).join(" or ");
    case "not":
      return `not (${describeCondition(node.child)})`;
    default:
      return "condition";
  }
};

export const ruleSummary = (rule: AlarmRule): string => {
  return `Trigger when ${describeSelector(rule.target_selector)} has ${describeCondition(rule.condition_ast)}.`;
};

export const wizardSummary = (state: AlarmWizardState): string => {
  if (state.template === "threshold") {
    return `Trigger when value ${state.thresholdOp} ${state.thresholdValue || "…"}.`;
  }
  if (state.template === "range") {
    return `Trigger when value is ${state.rangeMode} [${state.rangeLow || "…"}, ${state.rangeHigh || "…"}].`;
  }
  if (state.template === "offline") {
    return `Trigger when no data for ${state.offlineSeconds || "…"} seconds.`;
  }
  if (state.template === "rolling_window") {
    return `Trigger when ${state.rollingAggregate} over ${state.rollingWindowSeconds || "…"}s ${state.rollingOp} ${state.rollingValue || "…"}.`;
  }
  if (state.template === "deviation") {
    return `Trigger when ${state.deviationMode} deviation from ${state.deviationBaseline} over ${state.deviationWindowSeconds || "…"}s exceeds ${state.deviationValue || "…"}.`;
  }
  return `Trigger when threshold holds for ${state.consecutiveCount || "…"} consecutive ${state.consecutivePeriod}(s).`;
};
