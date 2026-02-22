export type BatteryChemistry = "lifepo4" | "lead_acid";

export type CurrentSignMode = "auto" | "positive_is_charging" | "positive_is_discharging";

export type SocAnchorMode = "disabled" | "blend_to_renogy_when_resting";

export type CapacityEstimationConfig = {
  enabled: boolean;
  min_soc_span_percent: number;
  ema_alpha: number;
  clamp_min_ah: number;
  clamp_max_ah: number;
};

export type BatteryModelConfig = {
  enabled: boolean;
  chemistry: BatteryChemistry;
  current_sign: CurrentSignMode;
  sticker_capacity_ah: number | null;
  soc_cutoff_percent: number;
  rest_current_abs_a: number;
  rest_minutes_required: number;
  soc_anchor_mode: SocAnchorMode;
  soc_anchor_max_step_percent: number;
  capacity_estimation: CapacityEstimationConfig;
};

export type BatteryConfigResponse = {
  node_id: string;
  battery_model: BatteryModelConfig;
  resolved_sticker_capacity_ah?: number | null;
  resolved_sticker_capacity_source?: string | null;
};

export type PowerRunwayConfig = {
  enabled: boolean;
  load_sensor_ids: string[];
  history_days: number;
  pv_derate: number;
  projection_days: number;
};

export type PowerRunwayConfigResponse = {
  node_id: string;
  power_runway: PowerRunwayConfig;
  load_sensors_valid: boolean;
};

