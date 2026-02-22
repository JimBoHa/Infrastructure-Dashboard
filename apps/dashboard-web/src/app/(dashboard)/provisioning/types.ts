export type NodeFieldKey =
  | "nodeName"
  | "nodeId"
  | "adoptionToken"
  | "wifiSsid"
  | "wifiPassword"
  | "heartbeatIntervalSeconds"
  | "telemetryIntervalSeconds";

export type SensorDriverType = "analog" | "pulse" | "gpio_pulse";

export type SensorPresetKey =
  | "custom"
  | "temperature"
  | "soil_temperature"
  | "moisture"
  | "soil_moisture"
  | "humidity"
  | "pressure"
  | "pressure_4_20ma_300psi"
  | "wind_speed"
  | "wind_direction"
  | "solar_irradiance"
  | "lux"
  | "rain_gauge"
  | "rain_gauge_inches"
  | "flow_meter"
  | "flow_meter_gallons"
  | "water_level"
  | "water_level_4_20ma_240in"
  | "fertilizer_level"
  | "current"
  | "voltage"
  | "power_kw";

export type SensorFieldKey =
  | "sensor_id"
  | "name"
  | "type"
  | "channel"
  | "unit"
  | "interval_seconds"
  | "rolling_average_seconds"
  | "input_min"
  | "input_max"
  | "output_min"
  | "output_max"
  | "offset"
  | "scale"
  | "pulses_per_unit"
  | "current_loop_shunt_ohms"
  | "current_loop_range_m";

export type SensorDraft = {
  key: string;
  preset: SensorPresetKey;
  sensor_id: string;
  name: string;
  type: SensorDriverType;
  channel: number;
  unit: string;
  location: string;
  interval_seconds: number;
  rolling_average_seconds: number;
  input_min: number | null;
  input_max: number | null;
  output_min: number | null;
  output_max: number | null;
  offset: number;
  scale: number;
  pulses_per_unit: number | null;
  current_loop_shunt_ohms: number | null;
  current_loop_range_m: number | null;
};

export type ProvisioningDraft = {
  nodeName: string;
  nodeId: string;
  adoptionToken: string;
  wifiSsid: string;
  wifiPassword: string;
  heartbeatIntervalSeconds: string;
  telemetryIntervalSeconds: string;
  sensors: SensorDraft[];
};

export type DraftValidation = {
  ok: boolean;
  errors: string[];
  invalidNodeFields: Set<NodeFieldKey>;
  invalidSensorFields: Map<string, Set<SensorFieldKey>>;
};

export type SensorPreset = {
  label: string;
  hint: string;
  driver: SensorDriverType;
  unit: string;
  interval_seconds: number;
  rolling_average_seconds: number;
  defaultName: string;
  input_min?: number | null;
  input_max?: number | null;
  output_min?: number | null;
  output_max?: number | null;
  offset?: number;
  scale?: number;
  pulses_per_unit?: number | null;
  current_loop_shunt_ohms?: number | null;
  current_loop_range_m?: number | null;
};

export type PreviewMode = "node-config" | "firstboot";
