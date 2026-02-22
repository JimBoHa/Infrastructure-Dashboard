export type SetupDaemonConfig = {
  install_root: string;
  data_root: string;
  logs_root: string;
  core_binary: string;
  sidecar_binary: string;
  core_port: number;
  mqtt_host: string;
  mqtt_port: number;
  mqtt_username: string | null;
  mqtt_password_configured: boolean;
  redis_port: number;
  database_url: string;
  backup_root: string;
  backup_retention_days: number;
  service_user: string;
  service_group: string;
  bundle_path: string | null;
  farmctl_path: string;
  profile: string;
  launchd_label_prefix: string;
  setup_port: number;
  enable_analytics_feeds: boolean;
  enable_forecast_ingestion: boolean;
  analytics_feed_poll_interval_seconds: number;
  forecast_poll_interval_seconds: number;
  schedule_poll_interval_seconds: number;
  offline_threshold_seconds: number;
  sidecar_mqtt_topic_prefix: string;
  sidecar_mqtt_keepalive_secs: number;
  sidecar_enable_mqtt_listener: boolean;
  sidecar_batch_size: number;
  sidecar_flush_interval_ms: number;
  sidecar_max_queue: number;
  sidecar_status_poll_interval_ms: number;
};

export type SetupDaemonConfigDraft = {
  core_port: string;
  mqtt_host: string;
  mqtt_port: string;
  mqtt_username: string;
  mqtt_password: string;
  redis_port: string;
  backup_root: string;
  backup_retention_days: string;
  bundle_path: string;
  database_url: string;
  install_root: string;
  data_root: string;
  logs_root: string;
  core_binary: string;
  sidecar_binary: string;
  service_user: string;
  service_group: string;
  farmctl_path: string;
  launchd_label_prefix: string;
  setup_port: string;
  enable_analytics_feeds: boolean;
  enable_forecast_ingestion: boolean;
  analytics_feed_poll_interval_seconds: string;
  forecast_poll_interval_seconds: string;
  schedule_poll_interval_seconds: string;
  offline_threshold_seconds: string;
  sidecar_mqtt_topic_prefix: string;
  sidecar_mqtt_keepalive_secs: string;
  sidecar_enable_mqtt_listener: boolean;
  sidecar_batch_size: string;
  sidecar_flush_interval_ms: string;
  sidecar_max_queue: string;
  sidecar_status_poll_interval_ms: string;
};

export type SetupDaemonPreflightCheck = {
  id: string;
  status: string;
  message: string;
};

export type SetupDaemonLocalIp = {
  recommended: string | null;
  candidates: string[];
};

export type ControllerRuntimeConfig = {
  mqtt_username: string | null;
  mqtt_password_configured: boolean;
  enable_analytics_feeds: boolean;
  enable_forecast_ingestion: boolean;
  analytics_feed_poll_interval_seconds: number;
  forecast_poll_interval_seconds: number;
  schedule_poll_interval_seconds: number;
  offline_threshold_seconds: number;
  sidecar_mqtt_topic_prefix: string;
  sidecar_mqtt_keepalive_secs: number;
  sidecar_enable_mqtt_listener: boolean;
  sidecar_batch_size: number;
  sidecar_flush_interval_ms: number;
  sidecar_max_queue: number;
  sidecar_status_poll_interval_ms: number;
};

export const asStringValue = (value: unknown, fallback = ""): string =>
  typeof value === "string" ? value : value == null ? fallback : String(value);

export const asNullableString = (value: unknown): string | null =>
  typeof value === "string" ? value : value == null ? null : String(value);

export const asIntValue = (value: unknown, fallback: number): number => {
  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.floor(value);
  }
  if (typeof value === "string" && value.trim().length) {
    const parsed = Number.parseInt(value, 10);
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }
  return fallback;
};

export const asBoolValue = (value: unknown, fallback: boolean): boolean => {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    const lowered = value.trim().toLowerCase();
    if (["1", "true", "yes", "on"].includes(lowered)) return true;
    if (["0", "false", "no", "off"].includes(lowered)) return false;
  }
  return fallback;
};

export const parseSetupDaemonConfig = (payload: unknown): SetupDaemonConfig => {
  if (!payload || typeof payload !== "object") {
    throw new Error("Invalid setup daemon config payload.");
  }
  const record = payload as Record<string, unknown>;
  const rawMqttPassword = record.mqtt_password;
  const sidecarBatchSize = asIntValue(record.sidecar_batch_size, 500);
  const sidecarMaxQueue = asIntValue(record.sidecar_max_queue, sidecarBatchSize * 10);
  return {
    install_root: asStringValue(record.install_root),
    data_root: asStringValue(record.data_root),
    logs_root: asStringValue(record.logs_root),
    core_binary: asStringValue(record.core_binary),
    sidecar_binary: asStringValue(record.sidecar_binary),
    core_port: asIntValue(record.core_port, 8000),
    mqtt_host: asStringValue(record.mqtt_host, "127.0.0.1"),
    mqtt_port: asIntValue(record.mqtt_port, 1883),
    mqtt_username: asNullableString(record.mqtt_username),
    mqtt_password_configured:
      typeof rawMqttPassword === "string" ? rawMqttPassword.trim().length > 0 : false,
    redis_port: asIntValue(record.redis_port, 6379),
    database_url: asStringValue(record.database_url),
    backup_root: asStringValue(record.backup_root),
    backup_retention_days: asIntValue(record.backup_retention_days, 30),
    service_user: asStringValue(record.service_user),
    service_group: asStringValue(record.service_group),
    bundle_path: asNullableString(record.bundle_path),
    farmctl_path: asStringValue(record.farmctl_path),
    profile: asStringValue(record.profile, "prod"),
    launchd_label_prefix: asStringValue(record.launchd_label_prefix),
    setup_port: asIntValue(record.setup_port, 8800),
    enable_analytics_feeds: asBoolValue(record.enable_analytics_feeds, true),
    enable_forecast_ingestion: asBoolValue(record.enable_forecast_ingestion, true),
    analytics_feed_poll_interval_seconds: asIntValue(record.analytics_feed_poll_interval_seconds, 300),
    forecast_poll_interval_seconds: asIntValue(record.forecast_poll_interval_seconds, 3600),
    schedule_poll_interval_seconds: asIntValue(record.schedule_poll_interval_seconds, 15),
    offline_threshold_seconds: asIntValue(record.offline_threshold_seconds, 5),
    sidecar_mqtt_topic_prefix: asStringValue(record.sidecar_mqtt_topic_prefix, "iot"),
    sidecar_mqtt_keepalive_secs: asIntValue(record.sidecar_mqtt_keepalive_secs, 30),
    sidecar_enable_mqtt_listener: asBoolValue(record.sidecar_enable_mqtt_listener, true),
    sidecar_batch_size: sidecarBatchSize,
    sidecar_flush_interval_ms: asIntValue(record.sidecar_flush_interval_ms, 750),
    sidecar_max_queue: sidecarMaxQueue,
    sidecar_status_poll_interval_ms: asIntValue(record.sidecar_status_poll_interval_ms, 1000),
  };
};

export const parseControllerRuntimeConfig = (payload: unknown): ControllerRuntimeConfig => {
  if (!payload || typeof payload !== "object") {
    throw new Error("Invalid controller runtime config payload.");
  }
  const record = payload as Record<string, unknown>;
  const sidecarBatchSize = asIntValue(record.sidecar_batch_size, 500);
  const sidecarMaxQueue = asIntValue(record.sidecar_max_queue, sidecarBatchSize * 10);
  return {
    mqtt_username: asNullableString(record.mqtt_username),
    mqtt_password_configured: asBoolValue(record.mqtt_password_configured, false),
    enable_analytics_feeds: asBoolValue(record.enable_analytics_feeds, true),
    enable_forecast_ingestion: asBoolValue(record.enable_forecast_ingestion, true),
    analytics_feed_poll_interval_seconds: asIntValue(record.analytics_feed_poll_interval_seconds, 300),
    forecast_poll_interval_seconds: asIntValue(record.forecast_poll_interval_seconds, 3600),
    schedule_poll_interval_seconds: asIntValue(record.schedule_poll_interval_seconds, 15),
    offline_threshold_seconds: asIntValue(record.offline_threshold_seconds, 5),
    sidecar_mqtt_topic_prefix: asStringValue(record.sidecar_mqtt_topic_prefix, "iot"),
    sidecar_mqtt_keepalive_secs: asIntValue(record.sidecar_mqtt_keepalive_secs, 30),
    sidecar_enable_mqtt_listener: asBoolValue(record.sidecar_enable_mqtt_listener, true),
    sidecar_batch_size: sidecarBatchSize,
    sidecar_flush_interval_ms: asIntValue(record.sidecar_flush_interval_ms, 750),
    sidecar_max_queue: sidecarMaxQueue,
    sidecar_status_poll_interval_ms: asIntValue(record.sidecar_status_poll_interval_ms, 1000),
  };
};

export const parseSetupDaemonPreflight = (payload: unknown): SetupDaemonPreflightCheck[] => {
  if (!payload || typeof payload !== "object") {
    throw new Error("Invalid setup daemon preflight payload.");
  }
  const record = payload as Record<string, unknown>;
  const checksRaw = Array.isArray(record.checks) ? record.checks : [];
  return checksRaw
    .map((entry) => {
      if (!entry || typeof entry !== "object") {
        return null;
      }
      const check = entry as Record<string, unknown>;
      const id = asStringValue(check.id);
      if (!id) return null;
      return {
        id,
        status: asStringValue(check.status, "unknown"),
        message: asStringValue(check.message),
      } satisfies SetupDaemonPreflightCheck;
    })
    .filter(Boolean) as SetupDaemonPreflightCheck[];
};

export const parseSetupDaemonLocalIp = (payload: unknown): SetupDaemonLocalIp => {
  if (!payload || typeof payload !== "object") {
    throw new Error("Invalid setup daemon local-ip payload.");
  }
  const record = payload as Record<string, unknown>;
  const candidates = Array.isArray(record.candidates)
    ? record.candidates.map((item) => asStringValue(item)).filter(Boolean)
    : [];
  return {
    recommended: record.recommended != null ? asNullableString(record.recommended) : null,
    candidates,
  };
};
