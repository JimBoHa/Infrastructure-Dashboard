export type AlarmSeverity = "info" | "warning" | "critical";

export type TargetMatchMode = "per_sensor" | "any" | "all";

export type TargetSelector =
  | {
      kind: "sensor";
      sensor_id: string;
    }
  | {
      kind: "sensor_set";
      sensor_ids: string[];
      match?: TargetMatchMode;
    }
  | {
      kind: "node_sensors";
      node_id: string;
      types?: string[];
      match?: TargetMatchMode;
    }
  | {
      kind: "filter";
      provider?: string;
      metric?: string;
      sensor_type?: string;
      match?: TargetMatchMode;
    };

export type ConditionNode =
  | {
      type: "threshold";
      op: "lt" | "lte" | "gt" | "gte" | "eq" | "neq";
      value: number;
    }
  | {
      type: "range";
      mode: "inside" | "outside";
      low: number;
      high: number;
    }
  | {
      type: "offline";
      missing_for_seconds: number;
    }
  | {
      type: "rolling_window";
      window_seconds: number;
      aggregate: "avg" | "min" | "max" | "stddev";
      op: "lt" | "lte" | "gt" | "gte" | "eq" | "neq";
      value: number;
    }
  | {
      type: "deviation";
      window_seconds: number;
      baseline: "mean" | "median";
      mode: "percent" | "absolute";
      value: number;
    }
  | {
      type: "consecutive_periods";
      period: "eval" | "hour" | "day";
      count: number;
      child: ConditionNode;
    }
  | {
      type: "all";
      children: ConditionNode[];
    }
  | {
      type: "any";
      children: ConditionNode[];
    }
  | {
      type: "not";
      child: ConditionNode;
    };

export type AlarmRuleTiming = {
  debounce_seconds?: number;
  clear_hysteresis_seconds?: number;
  eval_interval_seconds?: number;
};

export type AlarmRule = {
  id: number;
  name: string;
  description: string;
  enabled: boolean;
  severity: AlarmSeverity;
  origin: string;
  target_selector: TargetSelector;
  condition_ast: ConditionNode;
  timing: AlarmRuleTiming;
  message_template: string;
  created_by?: string | null;
  created_at: string;
  updated_at: string;
  deleted_at?: string | null;
  active_count: number;
  last_eval_at?: string | null;
  last_error?: string | null;
};

export type AlarmRuleCreateRequest = {
  name: string;
  description?: string;
  enabled?: boolean;
  severity?: AlarmSeverity;
  origin?: string;
  target_selector: TargetSelector;
  condition_ast: ConditionNode;
  timing?: AlarmRuleTiming;
  message_template?: string;
};

export type AlarmRuleUpdateRequest = Partial<AlarmRuleCreateRequest>;

export type AlarmRulePreviewResult = {
  target_key: string;
  sensor_ids: string[];
  passed: boolean;
  observed_value?: number | null;
};

export type AlarmRulePreviewResponse = {
  targets_evaluated: number;
  results: AlarmRulePreviewResult[];
};

export type AlarmRuleStatsBucketAggregationMode =
  | "auto"
  | "avg"
  | "last"
  | "sum"
  | "min"
  | "max";

export type AlarmRuleStatsRequest = {
  target_selector: TargetSelector;
  start?: string;
  end?: string;
  interval_seconds?: number;
  bucket_aggregation_mode?: AlarmRuleStatsBucketAggregationMode;
};

export type AlarmRuleStatsBandSet = {
  lower_1?: number | null;
  upper_1?: number | null;
  lower_2?: number | null;
  upper_2?: number | null;
  lower_3?: number | null;
  upper_3?: number | null;
};

export type AlarmRuleStatsBands = {
  classic: AlarmRuleStatsBandSet;
  robust: AlarmRuleStatsBandSet;
};

export type AlarmRuleStatsSensor = {
  sensor_id: string;
  unit: string;
  interval_seconds: number;
  n: number;
  min?: number | null;
  max?: number | null;
  mean?: number | null;
  median?: number | null;
  stddev?: number | null;
  p01?: number | null;
  p05?: number | null;
  p25?: number | null;
  p75?: number | null;
  p95?: number | null;
  p99?: number | null;
  mad?: number | null;
  iqr?: number | null;
  coverage_pct?: number | null;
  missing_pct?: number | null;
  bands: AlarmRuleStatsBands;
};

export type AlarmRuleStatsResponse = {
  start: string;
  end: string;
  interval_seconds: number;
  bucket_aggregation_mode: string;
  sensors: AlarmRuleStatsSensor[];
};

export type AlarmTemplateKind =
  | "threshold"
  | "range"
  | "offline"
  | "rolling_window"
  | "deviation"
  | "consecutive";

export type AlarmWizardState = {
  mode: "create" | "edit";
  ruleId?: number;
  name: string;
  description: string;
  severity: AlarmSeverity;
  origin: string;
  template: AlarmTemplateKind;
  selectorMode: "sensor" | "node" | "filter";
  sensorId: string;
  nodeId: string;
  filterProvider: string;
  filterMetric: string;
  filterType: string;
  thresholdOp: "lt" | "lte" | "gt" | "gte" | "eq" | "neq";
  thresholdValue: string;
  rangeMode: "inside" | "outside";
  rangeLow: string;
  rangeHigh: string;
  offlineSeconds: string;
  rollingWindowSeconds: string;
  rollingAggregate: "avg" | "min" | "max" | "stddev";
  rollingOp: "lt" | "lte" | "gt" | "gte" | "eq" | "neq";
  rollingValue: string;
  deviationWindowSeconds: string;
  deviationBaseline: "mean" | "median";
  deviationMode: "percent" | "absolute";
  deviationValue: string;
  consecutivePeriod: "eval" | "hour" | "day";
  consecutiveCount: string;
  debounceSeconds: string;
  clearHysteresisSeconds: string;
  evalIntervalSeconds: string;
  messageTemplate: string;
  advancedJson: string;
  advancedMode: boolean;
};
