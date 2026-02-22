export type PredictiveTraceEntry = {
  timestamp: string;
  model?: string | null;
  code: string;
  output: string;
};

export type AlarmOrigin = string;

export interface AlarmRecord {
  id: string | number;
  name: string;
  status?: string;
  sensor_id?: string | null;
  node_id?: string | null;
  origin?: AlarmOrigin | null;
  anomaly_score?: number | null;
  severity?: string | null;
  rule?: Record<string, unknown>;
  message?: string | null;
  last_fired?: string | null;
}

export interface PredictiveStatus {
  enabled: boolean;
  running: boolean;
  token_present: boolean;
  api_base_url: string;
  model?: string | null;
  fallback_models: string[];
  bootstrap_on_start: boolean;
  bootstrap_max_sensors: number;
  bootstrap_lookback_hours: number;
}
