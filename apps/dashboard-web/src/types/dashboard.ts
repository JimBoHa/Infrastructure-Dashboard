export type TimeSeriesPoint = {
  timestamp: Date;
  value: number;
};

export type TrendSeriesPoint = Omit<TimeSeriesPoint, "value"> & {
  value: number | null;
  samples?: number;
  sensor_name?: string;
};

export type TrendSeriesEntry = {
  sensor_id: string;
  label?: string;
  unit?: string;
  display_decimals?: number;
  points: TrendSeriesPoint[];
};

export type DemoNode = {
  id: string;
  name: string;
  status: string;
  uptime_seconds: number | null;
  cpu_percent: number | null;
  storage_used_bytes: number | null;
  memory_percent?: number | null;
  memory_used_bytes?: number | null;
  ping_ms?: number | null;
  ping_p50_30m_ms?: number | null;
  ping_jitter_ms?: number | null;
  mqtt_broker_rtt_ms?: number | null;
  mqtt_broker_rtt_jitter_ms?: number | null;
  network_latency_ms?: number | null;
  network_jitter_ms?: number | null;
  uptime_percent_24h?: number | null;
  mac_eth: string | null;
  mac_wifi: string | null;
  ip_last: string | Record<string, unknown> | null;
  last_seen: Date | string | null;
  created_at: Date;
  config: Record<string, unknown>;
};

export type DemoSensor = {
  sensor_id: string;
  node_id: string;
  name: string;
  type: string;
  unit: string;
  interval_seconds: number;
  rolling_avg_seconds: number;
  latest_value?: number;
  latest_ts?: Date | null;
  status?: string | null;
  location?: string | null;
  created_at: Date;
  config: Record<string, unknown>;
};

export type DemoOutput = {
  id: string;
  node_id: string;
  name: string;
  type: string;
  state: string;
  last_command: Date | string | null;
  supported_states: string[];
  command_topic?: string | null;
  schedule_ids: string[];
  history: unknown[];
  config: Record<string, unknown>;
};

export type DemoUser = {
  id: string;
  name: string;
  email: string;
  role: string;
  capabilities: string[];
  last_login: Date | string | null;
};

export type DemoSchedule = {
  id: string;
  name: string;
  rrule: string;
  blocks: Array<{
    day: string;
    start: string;
    end: string;
  }>;
  conditions: Array<Record<string, unknown>>;
  actions: Array<Record<string, unknown>>;
  next_run: Date | string | null;
};

export type DemoAlarm = {
  id: number;
  name: string;
  rule: Record<string, unknown>;
  status: string;
  sensor_id: string | null;
  node_id: string | null;
  origin: string;
  anomaly_score: number;
  last_fired: Date | null;
  type: string;
  severity: string;
  target_type: string;
  target_id?: string;
  condition: Record<string, unknown>;
  active: boolean;
  message?: string | null;
  rule_id?: number | null;
  target_key?: string | null;
  resolved_at?: Date | null;
};

export type DemoAlarmEvent = {
  id: string;
  alarm_id: string | null;
  rule_id?: string | null;
  sensor_id: string | null;
  node_id: string | null;
  origin?: string;
  anomaly_score?: number;
  message?: string;
  status: string;
  created_at: Date | string;
  raised_at: string;
  cleared_at?: string | null;
  acknowledged?: boolean;
  transition?: string | null;
};

export type AnalyticsIntegration = {
  name: string;
  status: string;
  details?: string;
  last_seen?: string;
};

export type AnalyticsRateSchedule = {
  provider: string;
  current_rate: number;
  est_monthly_cost: number;
  currency?: string;
  period_label?: string;
};

export type AnalyticsPower = {
  live_kw: number;
  live_solar_kw: number;
  live_grid_kw: number;
  live_battery_kw?: number;
  kwh_24h: number;
  kwh_168h: number;
  solar_kwh_24h?: number;
  solar_kwh_168h?: number;
  grid_kwh_24h?: number;
  grid_kwh_168h?: number;
  battery_kwh_24h?: number;
  battery_kwh_168h?: number;
  series_24h: TimeSeriesPoint[];
  series_168h: TimeSeriesPoint[];
  solar_series_24h?: TimeSeriesPoint[];
  solar_series_168h?: TimeSeriesPoint[];
  grid_series_24h?: TimeSeriesPoint[];
  grid_series_168h?: TimeSeriesPoint[];
  battery_series_24h?: TimeSeriesPoint[];
  battery_series_168h?: TimeSeriesPoint[];
  integrations: AnalyticsIntegration[];
  rate_schedule: AnalyticsRateSchedule;
};

export type AnalyticsWater = {
  domestic_gal_24h: number;
  domestic_gal_168h: number;
  ag_gal_24h: number;
  ag_gal_168h: number;
  reservoir_depth: TimeSeriesPoint[];
  domestic_series: TimeSeriesPoint[];
  ag_series: TimeSeriesPoint[];
  domestic_series_24h?: TimeSeriesPoint[];
  domestic_series_168h?: TimeSeriesPoint[];
  ag_series_24h?: TimeSeriesPoint[];
  ag_series_168h?: TimeSeriesPoint[];
};

export type AnalyticsSoilField = {
  name: string;
  min: number;
  max: number;
  avg: number;
};

export type AnalyticsSoil = {
  fields: AnalyticsSoilField[];
  series: TimeSeriesPoint[];
  series_avg?: TimeSeriesPoint[];
  series_min?: TimeSeriesPoint[];
  series_max?: TimeSeriesPoint[];
};

export type AnalyticsStatus = {
  alarms_last_168h: number;
  nodes_online: number;
  nodes_offline: number;
  remote_nodes_online?: number;
  remote_nodes_offline?: number;
  battery_soc: number;
  battery_runtime_hours: number;
  solar_kw: number;
  current_load_kw?: number;
  estimated_runtime_hours?: number;
  storage_capacity_kwh?: number;
  last_updated?: string | null;
};

export type AnalyticsBundle = {
  power: AnalyticsPower;
  water: AnalyticsWater;
  soil: AnalyticsSoil;
  status: AnalyticsStatus;
};

export type AnalyticsFeedEntry = {
  status: string | null;
  details: string | null;
  last_seen: Date | string | null;
};

export type AnalyticsFeedHistoryEntry = {
  name: string;
  category: string;
  status: string;
  recorded_at: Date;
  meta?: Record<string, unknown>;
};

export type AnalyticsFeedStatus = {
  enabled: boolean;
  feeds: Record<string, AnalyticsFeedEntry>;
  history: AnalyticsFeedHistoryEntry[];
};

export type DemoBackup = {
  id: string;
  node_id: string;
  captured_at: Date;
  size_bytes: number;
  path: string;
};

export type BackupRetentionPolicy = {
  node_id: string;
  node_name: string;
  keep_days: number;
};

export type BackupRetentionConfig = {
  default_keep_days: number;
  policies: BackupRetentionPolicy[];
  last_cleanup_at: Date | null;
};

export type DemoAdoptionCandidate = {
  service_name: string;
  hostname?: string;
  ip?: string;
  port?: number;
  mac_eth?: string | null;
  mac_wifi?: string | null;
  adoption_token?: string;
  properties?: Record<string, string>;
};

export type DemoConnection = {
  mode: "local" | "cloud" | string;
  local_address: string;
  cloud_address: string;
  status: string;
  last_switch: Date | string | null;
  timezone: string | null;
};

export type DashboardSnapshot = Record<string, unknown> & {
  timestamp: Date;
  nodes: DemoNode[];
  sensors: DemoSensor[];
  outputs: DemoOutput[];
  schedules: DemoSchedule[];
  alarms: DemoAlarm[];
  alarm_events: DemoAlarmEvent[];
  users: DemoUser[];
  analytics: AnalyticsBundle;
  backups: DemoBackup[];
  backup_retention?: BackupRetentionConfig;
  adoption: DemoAdoptionCandidate[];
  connection: DemoConnection;
  trend_series: TrendSeriesEntry[];
};

export type DemoAnalyticsPower = AnalyticsPower;
export type DemoAnalyticsWater = AnalyticsWater;
export type DemoAnalyticsSoil = AnalyticsSoil;
export type DemoAnalyticsStatus = AnalyticsStatus;
export type DemoAnalytics = AnalyticsBundle;
