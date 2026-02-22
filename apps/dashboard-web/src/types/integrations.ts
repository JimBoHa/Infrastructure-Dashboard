export type Ws2902Protocol = "wunderground" | "ambient";

export type Ws2902CreatedSensor = {
  sensor_id: string;
  name: string;
  type: string;
  unit: string;
  interval_seconds: number;
};

export type Ws2902CreateRequest = {
  nickname: string;
  protocol: Ws2902Protocol;
  interval_seconds?: number;
};

export type Ws2902CreateResponse = {
  id: string;
  node_id: string;
  nickname: string;
  protocol: Ws2902Protocol;
  enabled: boolean;
  ingest_path: string;
  token: string;
  created_at: string;
  sensors: Ws2902CreatedSensor[];
};

export type Ws2902StatusResponse = {
  id: string;
  node_id: string;
  nickname: string;
  protocol: Ws2902Protocol;
  enabled: boolean;
  ingest_path_template: string;
  created_at: string;
  rotated_at?: string | null;
  last_seen?: string | null;
  last_missing_fields: string[];
  last_payload?: Record<string, unknown> | null;
};

export type Ws2902RotateTokenResponse = {
  id: string;
  ingest_path: string;
  token: string;
  rotated_at: string;
};

export type RenogyBt2Mode = "ble" | "external";

export type RenogyPresetSensor = {
  sensor_id: string;
  name: string;
  metric: string;
  type: string;
  unit: string;
  interval_seconds: number;
};

export type ApplyRenogyBt2PresetRequest = {
  bt2_address: string;
  poll_interval_seconds?: number;
  mode?: RenogyBt2Mode;
  adapter?: string;
  unit_id?: number;
  device_name?: string;
  request_timeout_seconds?: number;
  connect_timeout_seconds?: number;
  service_uuid?: string;
  write_uuid?: string;
  notify_uuid?: string;
};

export type ApplyRenogyBt2PresetResponse = {
  status: "applied" | "already_configured" | "stored";
  node_id: string;
  node_agent_url?: string | null;
  bt2_address: string;
  mode: RenogyBt2Mode;
  poll_interval_seconds: number;
  warning?: string | null;
  sensors: RenogyPresetSensor[];
  what_to_check: string[];
};

export type RenogyRegisterMapSchema = {
  schema: Record<string, unknown>;
};

export type RenogyDesiredSettingsResponse = {
  node_id: string;
  device_type: string;
  desired: Record<string, unknown>;
  pending: boolean;
  desired_updated_at: string;
  last_applied?: Record<string, unknown> | null;
  last_applied_at?: string | null;
  last_apply_status?: string | null;
  last_apply_result?: Record<string, unknown> | null;
  apply_requested?: boolean;
  apply_requested_at?: string | null;
  maintenance_mode?: boolean;
};

export type RenogyValidateResponse = {
  ok: boolean;
  errors: string[];
};

export type RenogyReadCurrentResponse = {
  current: Record<string, unknown>;
  provider_status: string;
};

export type RenogyApplyResponse = {
  status: string;
  result: Record<string, unknown>;
};

export type RenogyHistoryEntry = {
  id: number;
  event_type: string;
  created_at: string;
  desired?: Record<string, unknown> | null;
  current?: Record<string, unknown> | null;
  diff?: Record<string, unknown> | null;
  result?: Record<string, unknown> | null;
};

export type ExternalDevicePoint = {
  name: string;
  metric: string;
  sensor_type: string;
  unit: string;
  protocol: string;
  register?: number | null;
  data_type?: string | null;
  scale?: number | null;
  oid?: string | null;
  path?: string | null;
  json_pointer?: string | null;
  bacnet_object?: string | null;
};

export type ExternalDeviceModel = {
  id: string;
  name: string;
  since_year?: number | null;
  protocols: string[];
  points: ExternalDevicePoint[];
};

export type ExternalDeviceVendor = {
  id: string;
  name: string;
  models: ExternalDeviceModel[];
};

export type ExternalDeviceCatalog = {
  version: number;
  vendors: ExternalDeviceVendor[];
};

export type ExternalDeviceSummary = {
  node_id: string;
  name: string;
  external_provider?: string | null;
  external_id?: string | null;
  config: Record<string, unknown>;
};

export type ExternalDeviceCreateRequest = {
  name: string;
  vendor_id: string;
  model_id: string;
  protocol: string;
  host?: string | null;
  port?: number | null;
  unit_id?: number | null;
  poll_interval_seconds?: number | null;
  snmp_community?: string | null;
  http_base_url?: string | null;
  http_username?: string | null;
  http_password?: string | null;
  lip_username?: string | null;
  lip_password?: string | null;
  lip_integration_report?: string | null;
  leap_client_cert_pem?: string | null;
  leap_client_key_pem?: string | null;
  leap_ca_pem?: string | null;
  leap_verify_ca?: boolean | null;
  external_id?: string | null;
};
