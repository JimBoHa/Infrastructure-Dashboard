import type {
  AnalyticsFeedStatus,
  BackupRetentionConfig,
  BackupRetentionPolicy,
  DashboardSnapshot,
  DemoAdoptionCandidate,
  DemoAlarm,
  DemoAlarmEvent,
  DemoBackup,
  DemoConnection,
  DemoNode,
  DemoOutput,
  DemoSchedule,
  DemoSensor,
  DemoUser,
  TrendSeriesEntry,
} from "@/types/dashboard";
import type { PredictiveStatus, PredictiveTraceEntry } from "@/types/alarms";
import type {
  ActionLog,
  Incident,
  IncidentNotesListResponse,
  IncidentsListResponse,
  IncidentNote,
} from "@/types/incidents";
import type {
  AlarmRule,
  AlarmRuleCreateRequest,
  AlarmRulePreviewResponse,
  AlarmRuleStatsRequest,
  AlarmRuleStatsResponse,
  AlarmRuleUpdateRequest,
} from "@/features/alarms/types/alarmTypes";
import type {
  EmporiaDeviceUpdate,
  EmporiaDevicesResult,
  EmporiaLoginResult,
  SetupCredential,
} from "@/types/setup";
import type {
  ForecastSeriesResponse,
  ForecastStatus,
  CurrentWeatherResponse,
  PvForecastConfig,
  WeatherForecastConfig,
} from "@/types/forecast";
import type {
  BatteryConfigResponse,
  BatteryModelConfig,
  PowerRunwayConfig,
  PowerRunwayConfigResponse,
} from "@/types/battery";
import type { DevActivityStatus } from "@/types/devActivity";
import type {
  MapFeature,
  MapFeatureUpsertPayload,
  MapLayer,
  MapLayerUpsertPayload,
  OfflineMapPack,
  MapSave,
  MapSettings,
} from "@/types/map";
import type {
  Ws2902CreateRequest,
  Ws2902CreateResponse,
  Ws2902RotateTokenResponse,
  Ws2902StatusResponse,
  ApplyRenogyBt2PresetRequest,
  ApplyRenogyBt2PresetResponse,
  ExternalDeviceCatalog,
  ExternalDeviceCreateRequest,
  ExternalDeviceSummary,
} from "@/types/integrations";
import type {
  AnalysisJobCancelResponse,
  AnalysisJobCreateRequest,
  AnalysisJobCreateResponse,
  AnalysisJobEventsResponse,
  AnalysisJobResultResponse,
  AnalysisJobStatusResponse,
  TssePreviewRequestV1,
  TssePreviewResponseV1,
} from "@/types/analysis";
import { normalizeAnalyticsBundle } from "@/lib/analytics";
import { fetchJson, fetchBinary, extractStatus, putJson, deleteJson, postJson } from "@/lib/http";
import { decodeBinaryMetrics, readBinaryPoint } from "@/lib/binaryMetrics";
import {
  AdoptionCandidatesResponseSchema,
  AlarmEventsResponseSchema,
  AlarmRuleSchema,
  AlarmRulesResponseSchema,
  AlarmRulePreviewResponseSchema,
  AlarmsResponseSchema,
  AlarmRuleStatsResponseSchema,
  ActionLogsResponseSchema,
  AnalyticsFeedStatusSchema,
  BackupsResponseSchema,
  BackupRunResponseSchema,
  BackupRetentionConfigSchema,
  ConnectionSchema,
  DashboardSnapshotSchema,
  type DashboardSnapshotRaw,
  NodesResponseSchema,
  OutputsResponseSchema,
  PredictiveStatusSchema,
  PredictiveTraceResponseSchema,
  RecentRestoresResponseSchema,
  ScheduleCalendarResponseSchema,
  type ScheduleCalendarEventRaw,
  SchedulesResponseSchema,
  SensorsResponseSchema,
  SetupCredentialSchema,
  SetupCredentialsResponseSchema,
  EmporiaLoginResponseSchema,
  EmporiaDevicesResponseSchema,
  ForecastPollResponseSchema,
  ForecastSeriesResponseSchema,
  CurrentWeatherResponseSchema,
  ForecastStatusSchema,
  PvForecastConfigSchema,
  PvForecastCheckResponseSchema,
  BatteryConfigResponseSchema,
  PowerRunwayConfigResponseSchema,
  MapSettingsSchema,
  MapSaveSchema,
  MapSavesResponseSchema,
  MapLayerSchema,
  MapLayersResponseSchema,
  MapFeatureSchema,
  MapFeaturesResponseSchema,
  OfflineMapPackSchema,
  OfflineMapPacksResponseSchema,
  DevActivityStatusResponseSchema,
  UsersResponseSchema,
  WeatherForecastConfigSchema,
  ApplyRenogyBt2PresetResponseSchema,
  RenogyRegisterMapSchema,
  RenogyDesiredSettingsResponseSchema,
  RenogyValidateResponseSchema,
  RenogyReadCurrentResponseSchema,
  RenogyApplyResponseSchema,
  RenogyHistoryResponseSchema,
  NodeDisplayProfileSchema,
  UpdateNodeDisplayProfileResponseSchema,
  NodeSensorsConfigResponseSchema,
  ApplyNodeSensorsConfigResponseSchema,
  Ws2902CreateResponseSchema,
  Ws2902RotateTokenResponseSchema,
  Ws2902StatusResponseSchema,
  IncidentsListResponseSchema,
  IncidentDetailResponseSchema,
  IncidentSchema,
  IncidentNoteSchema,
  IncidentNotesListResponseSchema,
  ExternalDeviceCatalogSchema,
  ExternalDeviceSummariesSchema,
  ExternalDeviceSummarySchema,
  parseApiResponse,
  type ApiSchema,
  type NodeDisplayProfile,
  type UpdateNodeDisplayProfileResponse,
  type NodeSensorDraft,
  type NodeAds1263SettingsDraft,
  type NodeSensorsConfigResponse,
  type ApplyNodeSensorsConfigResponse,
  type RenogyRegisterMapSchema as RenogyRegisterMapSchemaType,
  type RenogyDesiredSettingsResponse as RenogyDesiredSettingsResponseType,
  type RenogyValidateResponse as RenogyValidateResponseType,
  type RenogyReadCurrentResponse as RenogyReadCurrentResponseType,
  type RenogyApplyResponse as RenogyApplyResponseType,
  type RenogyHistoryEntry as RenogyHistoryEntryType,
} from "@/lib/apiSchemas";

export {
  API_BASE,
  apiUrl,
  fetchBinary,
  fetchJson,
  fetchResponse,
  fetcher,
  postJson,
  putJson,
  deleteJson,
} from "@/lib/http";

type ApiRecord = Record<string, unknown>;

async function fetchJsonValidated<T>(
  path: string,
  schema: ApiSchema<T>,
  init?: RequestInit,
): Promise<T> {
  const payload = await fetchJson<unknown>(path, init);
  return parseApiResponse(schema, payload, path);
}

async function fetchJsonOptional<T>(path: string, schema: ApiSchema<T>): Promise<T | null> {
  try {
    return await fetchJsonValidated(path, schema);
  } catch (error) {
    const status = extractStatus(error);
    if (status === 404) return null;
    throw error;
  }
}

export const createAnalysisJob = async (request: AnalysisJobCreateRequest): Promise<AnalysisJobCreateResponse> =>
  postJson<AnalysisJobCreateResponse>("/api/analysis/jobs", request);

export const fetchAnalysisJob = async (jobId: string): Promise<AnalysisJobStatusResponse> =>
  fetchJson<AnalysisJobStatusResponse>(`/api/analysis/jobs/${encodeURIComponent(jobId)}`);

export const fetchAnalysisJobEvents = async (
  jobId: string,
  options: { after?: number; limit?: number } = {},
): Promise<AnalysisJobEventsResponse> => {
  const query = new URLSearchParams();
  if (typeof options.after === "number") query.set("after", String(options.after));
  if (typeof options.limit === "number") query.set("limit", String(options.limit));
  const suffix = query.toString();
  return fetchJson<AnalysisJobEventsResponse>(
    `/api/analysis/jobs/${encodeURIComponent(jobId)}/events${suffix ? `?${suffix}` : ""}`,
  );
};

export const fetchAnalysisJobResult = async <T = unknown>(
  jobId: string,
): Promise<AnalysisJobResultResponse<T>> =>
  fetchJson<AnalysisJobResultResponse<T>>(`/api/analysis/jobs/${encodeURIComponent(jobId)}/result`);

export const cancelAnalysisJob = async (jobId: string): Promise<AnalysisJobCancelResponse> =>
  postJson<AnalysisJobCancelResponse>(`/api/analysis/jobs/${encodeURIComponent(jobId)}/cancel`, {});

export const fetchAnalysisPreview = async (
  request: TssePreviewRequestV1,
): Promise<TssePreviewResponseV1> => postJson<TssePreviewResponseV1>("/api/analysis/preview", request);

const asString = (value: unknown): string | undefined =>
  typeof value === "string" ? value : value != null ? String(value) : undefined;

const asDate = (value: unknown, fallback = new Date(0)): Date => {
  if (value instanceof Date) {
    return value;
  }
  if (typeof value === "string" || typeof value === "number") {
    const parsed = new Date(value);
    return Number.isNaN(parsed.valueOf()) ? fallback : parsed;
  }
  return fallback;
};

const asDateOrNull = (value: unknown): Date | null => {
  if (value == null) {
    return null;
  }
  return asDate(value);
};

const asDateStrict = (value: unknown): Date | null => {
  if (value instanceof Date) {
    return Number.isNaN(value.valueOf()) ? null : value;
  }
  if (typeof value === "string" || typeof value === "number") {
    const parsed = new Date(value);
    return Number.isNaN(parsed.valueOf()) ? null : parsed;
  }
  return null;
};

const asFiniteNumberOrNull = (value: unknown): number | null => {
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : null;
  }
  if (typeof value === "string" && value.trim().length) {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
};

const asNumber = (value: unknown): number =>
  typeof value === "number" && !Number.isNaN(value) ? value : 0;

const asPositiveInt = (value: unknown, fallback: number): number => {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.floor(value);
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number.parseInt(value, 10);
    if (Number.isFinite(parsed) && parsed > 0) {
      return parsed;
    }
  }
  return fallback;
};

const asArray = <T>(value: unknown): T[] =>
  Array.isArray(value) ? (value as T[]) : [];

const asIpLast = (value: unknown): DemoNode["ip_last"] => {
  if (typeof value === "string") {
    const trimmed = value.trim();
    return trimmed ? trimmed : null;
  }

  if (value == null) {
    return null;
  }

  if (typeof value === "object") {
    if ("value" in value) {
      const inner = (value as { value?: unknown }).value;
      if (typeof inner === "string") {
        const trimmed = inner.trim();
        return trimmed ? trimmed : null;
      }
      if (typeof inner === "number" && Number.isFinite(inner)) {
        return String(inner);
      }
    }

    try {
      return JSON.stringify(value);
    } catch {
      return null;
    }
  }

  if (typeof value === "number" && Number.isFinite(value)) {
    return String(value);
  }

  return String(value);
};

const normalizeNodes = (raw: Array<ApiRecord | DemoNode>): DemoNode[] =>
  raw.map((entry) => {
    const node = entry as ApiRecord;
    const createdAt = asDate(node.created_at);
    return {
      id: asString(node.id) ?? "",
      name: asString(node.name) ?? "Node",
      status: asString(node.status) ?? "unknown",
      uptime_seconds: asFiniteNumberOrNull(node.uptime_seconds),
      cpu_percent: asFiniteNumberOrNull(node.cpu_percent),
      storage_used_bytes: asFiniteNumberOrNull(node.storage_used_bytes),
      memory_percent: asFiniteNumberOrNull(node.memory_percent),
      memory_used_bytes: asFiniteNumberOrNull(node.memory_used_bytes),
      ping_ms: asFiniteNumberOrNull(node.ping_ms),
      ping_p50_30m_ms: asFiniteNumberOrNull(node.ping_p50_30m_ms),
      ping_jitter_ms: asFiniteNumberOrNull(node.ping_jitter_ms),
      mqtt_broker_rtt_ms: asFiniteNumberOrNull(node.mqtt_broker_rtt_ms),
      mqtt_broker_rtt_jitter_ms: asFiniteNumberOrNull(node.mqtt_broker_rtt_jitter_ms),
      network_latency_ms: asFiniteNumberOrNull(node.network_latency_ms),
      network_jitter_ms: asFiniteNumberOrNull(node.network_jitter_ms),
      uptime_percent_24h: asFiniteNumberOrNull(node.uptime_percent_24h),
      mac_eth: (asString(node.mac_eth) ?? null),
      mac_wifi: (asString(node.mac_wifi) ?? null),
      ip_last: asIpLast(node.ip_last),
      last_seen: asDateOrNull(node.last_seen),
      created_at: createdAt,
      config: (node.config as ApiRecord) ?? {},
    };
  });

const normalizeSensors = (raw: ApiRecord[]): DemoSensor[] =>
  raw.map((sensor) => {
    const config = (sensor.config as ApiRecord) ?? {};
    const createdAt = asDate(sensor.created_at);
    const latest =
      typeof sensor.latest_value === "number"
        ? sensor.latest_value
        : typeof config.latest_value === "number"
          ? (config.latest_value as number)
          : undefined;
    const latestTsRaw = sensor.latest_ts ?? config.latest_ts;
    const latest_ts = latestTsRaw != null ? asDateOrNull(latestTsRaw) : null;
    const status = asString(sensor.status) ?? asString(config.status);
    const location = asString(sensor.location) ?? asString(config.location);
    return {
      sensor_id: asString(sensor.sensor_id) ?? "",
      node_id: asString(sensor.node_id) ?? "",
      name: asString(sensor.name) ?? "Sensor",
      type: asString(sensor.type) ?? "unknown",
      unit: asString(sensor.unit) ?? "",
      interval_seconds: asNumber(sensor.interval_seconds),
      rolling_avg_seconds: asNumber(sensor.rolling_avg_seconds),
      latest_value: latest,
      latest_ts,
      status,
      location,
      created_at: createdAt,
      config,
    };
  });

const normalizeOutputs = (raw: ApiRecord[]): DemoOutput[] =>
  raw.map((output) => {
    const config = (output.config as ApiRecord) ?? {};
    const scheduleIds = asArray<string>(output.schedule_ids).length
      ? asArray<string>(output.schedule_ids)
      : asArray<string>(config.schedule_ids);
    const supportedStates = asArray<string>(output.supported_states);
    return {
      id: asString(output.id) ?? "",
      node_id: asString(output.node_id) ?? "",
      name: asString(output.name) ?? "Output",
      type: asString(output.type) ?? "unknown",
      state: asString(output.state) ?? "unknown",
      last_command: asDateOrNull(output.last_command),
      supported_states: supportedStates,
      command_topic: asString(output.command_topic) ?? asString(config.command_topic),
      schedule_ids: scheduleIds,
      history: asArray(output.history),
      config,
    };
  });

const normalizeSetupCredentials = (raw: ApiRecord[]): SetupCredential[] =>
  raw.map((entry) => ({
    name: asString(entry.name) ?? "",
    has_value: Boolean(entry.has_value ?? entry.value),
    metadata: (entry.metadata as Record<string, unknown>) ?? {},
    created_at: asString(entry.created_at) ?? null,
    updated_at: asString(entry.updated_at) ?? null,
  }));

const normalizeSchedules = (raw: ApiRecord[]): DemoSchedule[] =>
  raw.map((schedule) => ({
    id: asString(schedule.id) ?? "",
    name: asString(schedule.name) ?? "Schedule",
    rrule: asString(schedule.rrule) ?? "",
    blocks: asArray<ApiRecord>(schedule.blocks).map((block) => ({
      day: (asString(block.day) ?? "").toUpperCase(),
      start: asString(block.start) ?? "",
      end: asString(block.end) ?? "",
    })),
    conditions: asArray<Record<string, unknown>>(schedule.conditions).filter(
      (entry): entry is Record<string, unknown> =>
        Boolean(entry) && typeof entry === "object" && !Array.isArray(entry),
    ),
    actions: asArray<Record<string, unknown>>(schedule.actions).filter(
      (entry): entry is Record<string, unknown> =>
        Boolean(entry) && typeof entry === "object" && !Array.isArray(entry),
    ),
    next_run: asDateOrNull(schedule.next_run),
  }));

const normalizeAlarms = (raw: ApiRecord[]): DemoAlarm[] =>
  raw.map((alarm) => {
    const rule = (alarm.rule as ApiRecord) ?? {};
    const targetSensor = asString(alarm.sensor_id);
    const targetNode = asString(alarm.node_id);
    const origin = asString(alarm.origin) ?? asString(rule.origin) ?? "threshold";
    const anomalyScore =
      alarm.anomaly_score != null ? asNumber(alarm.anomaly_score) : asNumber(rule.anomaly_score);
    const lastFiredValue = alarm.last_fired ?? alarm.last_raised;
    const lastFired = lastFiredValue != null ? asDate(lastFiredValue) : null;
    const status = asString(alarm.status) ?? "active";
    return {
      id: asNumber(alarm.id),
      name: asString(alarm.name) ?? "Alarm",
      rule,
      status,
      sensor_id: targetSensor ?? null,
      node_id: targetNode ?? null,
      origin,
      anomaly_score: anomalyScore,
      last_fired: lastFired,
      type: asString(rule.type) ?? "threshold",
      severity: asString(rule.severity) ?? "info",
      target_type: targetSensor ? "sensor" : "node",
      target_id: targetSensor ?? targetNode ?? undefined,
      condition: rule,
      active: status === "active",
      message: asString(alarm.message),
      rule_id:
        alarm.rule_id != null ? asNumber(alarm.rule_id) : asNumber((rule as ApiRecord).rule_id),
      target_key: asString(alarm.target_key) ?? null,
      resolved_at: asDateOrNull(alarm.resolved_at),
    };
  });

const normalizeAlarmEvents = (raw: ApiRecord[]): DemoAlarmEvent[] =>
  raw.map((event) => {
    const createdAt = asDate(event.created_at);
    const status = asString(event.status) ?? "active";
    return {
      id: asString(event.id) ?? asString(event.alarm_id) ?? "",
      alarm_id: asString(event.alarm_id) ?? null,
      rule_id: asString(event.rule_id) ?? null,
      sensor_id: asString(event.sensor_id) ?? null,
      node_id: asString(event.node_id) ?? null,
      origin: asString(event.origin) ?? undefined,
      anomaly_score: event.anomaly_score != null ? asNumber(event.anomaly_score) : undefined,
      message: asString(event.message),
      status,
      created_at: createdAt,
      raised_at: createdAt.toISOString(),
      cleared_at: event.cleared_at != null ? asDate(event.cleared_at).toISOString() : null,
      acknowledged: status === "acknowledged",
      transition: asString(event.transition) ?? null,
    };
  });

const normalizeAlarmRules = (raw: ApiRecord[]): AlarmRule[] =>
  raw.map((rule) => ({
    id: asNumber(rule.id),
    name: asString(rule.name) ?? "Alarm rule",
    description: asString(rule.description) ?? "",
    enabled: Boolean(rule.enabled ?? true),
    severity: (asString(rule.severity) ?? "warning") as AlarmRule["severity"],
    origin: asString(rule.origin) ?? "threshold",
    target_selector: ((rule.target_selector as Record<string, unknown>) ?? {}) as AlarmRule["target_selector"],
    condition_ast: ((rule.condition_ast as Record<string, unknown>) ?? {}) as AlarmRule["condition_ast"],
    timing: ((rule.timing as Record<string, unknown>) ?? {}) as AlarmRule["timing"],
    message_template: asString(rule.message_template) ?? "",
    created_by: asString(rule.created_by) ?? null,
    created_at: asString(rule.created_at) ?? new Date(0).toISOString(),
    updated_at: asString(rule.updated_at) ?? new Date(0).toISOString(),
    deleted_at: asString(rule.deleted_at) ?? null,
    active_count: asNumber(rule.active_count),
    last_eval_at: asString(rule.last_eval_at) ?? null,
    last_error: asString(rule.last_error) ?? null,
  }));

const normalizeIncidents = (raw: ApiRecord[]): Incident[] =>
  raw.map((incident) => ({
    id: asString(incident.id) ?? "",
    rule_id: asString(incident.rule_id) ?? null,
    target_key: asString(incident.target_key) ?? null,
    severity: asString(incident.severity) ?? "info",
    status: asString(incident.status) ?? "open",
    title: asString(incident.title) ?? "Incident",
    assigned_to: asString(incident.assigned_to) ?? null,
    snoozed_until: asDateOrNull(incident.snoozed_until),
    first_event_at: asDate(incident.first_event_at),
    last_event_at: asDate(incident.last_event_at),
    closed_at: asDateOrNull(incident.closed_at),
    created_at: asDate(incident.created_at),
    updated_at: asDate(incident.updated_at),
    total_event_count: asNumber(incident.total_event_count),
    active_event_count: asNumber(incident.active_event_count),
    note_count: asNumber(incident.note_count),
    last_message: asString(incident.last_message) ?? null,
    last_origin: asString(incident.last_origin) ?? null,
    last_sensor_id: asString(incident.last_sensor_id) ?? null,
    last_node_id: asString(incident.last_node_id) ?? null,
  }));

const normalizeIncidentNotes = (raw: ApiRecord[]): IncidentNote[] =>
  raw.map((note) => ({
    id: asString(note.id) ?? "",
    incident_id: asString(note.incident_id) ?? "",
    created_by: asString(note.created_by) ?? null,
    body: asString(note.body) ?? "",
    created_at: asDate(note.created_at),
  }));

const normalizeActionLogs = (raw: ApiRecord[]): ActionLog[] =>
  raw.map((log) => ({
    id: asString(log.id) ?? "",
    schedule_id: asString(log.schedule_id) ?? "",
    action: log.action ?? null,
    status: asString(log.status) ?? "unknown",
    message: asString(log.message) ?? null,
    created_at: asDate(log.created_at),
    output_id: asString(log.output_id) ?? null,
    node_id: asString(log.node_id) ?? null,
  }));

const flattenBackups = (raw: ApiRecord[]): DemoBackup[] => {
  const backups: DemoBackup[] = [];
  raw.forEach((summary) => {
    const record = summary as ApiRecord;
    const backupEntries = asArray<ApiRecord>(record.backups);
    if (backupEntries.length) {
      const nodeId = asString(record.node_id) ?? "";
      backupEntries.forEach((file) => {
        const capturedAt = asDate(
          asString(file.created_at) ?? asString(file.date) ?? new Date(),
          new Date(),
        );
        backups.push({
          id: `${nodeId}-${asString(file.date) ?? capturedAt.toISOString()}`,
          node_id: nodeId,
          captured_at: capturedAt,
          size_bytes: asNumber(file.size_bytes),
          path: asString(file.path) ?? "",
        });
      });
      return;
    }

    const nodeId = asString(record.node_id) ?? "";
    const path = asString(record.path);
    if (!nodeId || !path) {
      return;
    }
    const capturedAt =
      asString(record.captured_at) ??
      asString(record.created_at) ??
      asString(record.date) ??
      new Date();
    const capturedAtDate = asDate(capturedAt, new Date());
    backups.push({
      id: asString(record.id) ?? `${nodeId}-${capturedAtDate.toISOString()}`,
      node_id: nodeId,
      captured_at: capturedAtDate,
      size_bytes: asNumber(record.size_bytes),
      path,
    });
  });
  return backups.sort((a, b) => {
    const aTime = a.captured_at instanceof Date ? a.captured_at.getTime() : 0;
    const bTime = b.captured_at instanceof Date ? b.captured_at.getTime() : 0;
    return bTime - aTime;
  });
};

const normalizeRetentionPolicy = (
  entry: ApiRecord | BackupRetentionPolicy,
  fallback: number,
): BackupRetentionPolicy => {
  const policy = entry as ApiRecord;
  const nodeId = asString(policy.node_id) ?? "";
  const nodeNameRaw = asString(policy.node_name ?? policy.name);
  const nodeLabel = nodeNameRaw && nodeNameRaw.trim().length ? nodeNameRaw : nodeId;
  return {
    node_id: nodeId,
    node_name: nodeLabel || "Node",
    keep_days: asPositiveInt(policy.keep_days, fallback),
  };
};

const normalizeRetentionConfig = (
  raw: ApiRecord | BackupRetentionConfig,
): BackupRetentionConfig => {
  const config = raw as ApiRecord;
  const defaultKeepDays = asPositiveInt(config.default_keep_days, 30);
  const lastCleanupValue = config.last_cleanup_at ?? config.last_cleanup ?? config.last_cleanup_time;
  const lastCleanup = lastCleanupValue != null ? asDate(lastCleanupValue) : null;
  const policies = asArray<ApiRecord>(config.policies).map((policy) =>
    normalizeRetentionPolicy(policy, defaultKeepDays),
  );
  return {
    default_keep_days: defaultKeepDays,
    policies,
    last_cleanup_at: lastCleanup,
  };
};

const normalizeTrendSeries = (raw: Array<ApiRecord | TrendSeriesEntry>): TrendSeriesEntry[] =>
  raw
    .map((entry) => {
      const series = entry as ApiRecord;
      const sensorId = asString(series.sensor_id) ?? "";
      if (!sensorId) {
        return null;
      }
      const label = asString(series.label ?? series.sensor_name) ?? undefined;
      const unit = asString(series.unit) ?? undefined;
      const displayDecimalsRaw =
        asFiniteNumberOrNull(series.display_decimals ?? series.displayDecimals) ?? null;
      const display_decimals =
        displayDecimalsRaw != null ? Math.max(0, Math.min(6, Math.floor(displayDecimalsRaw))) : undefined;

      const points = asArray<ApiRecord>(series.points)
        .map((point) => {
          const timestamp = asDateStrict(point.timestamp ?? point.ts);
          if (!timestamp) {
            return null;
          }
          const samplesRaw = asFiniteNumberOrNull(point.samples);
          const samples =
            samplesRaw != null && Number.isFinite(samplesRaw) ? Math.max(0, Math.floor(samplesRaw)) : undefined;
          return {
            timestamp,
            value: asFiniteNumberOrNull(point.value),
            samples,
            sensor_name: asString(point.sensor_name) ?? undefined,
          };
        })
        .filter(Boolean) as TrendSeriesEntry["points"];
      return {
        sensor_id: sensorId,
        label,
        unit,
        display_decimals,
        points,
      } satisfies TrendSeriesEntry;
    })
    .filter(Boolean) as TrendSeriesEntry[];

const normalizeUsers = (raw: ApiRecord[]): DemoUser[] =>
  raw.map((user) => ({
    id: asString(user.id) ?? "",
    name: asString(user.name) ?? "User",
    email: asString(user.email) ?? "",
    role: asString(user.role) ?? "viewer",
    capabilities: asArray<string>(user.capabilities),
    last_login: asDateOrNull(user.last_login),
  }));

const normalizeConnection = (
  raw: ApiRecord | DemoConnection,
): DemoConnection => {
  const record = raw as ApiRecord;
  return {
    mode: asString(record.mode) ?? "local",
    local_address: asString(record.local_address) ?? "http://127.0.0.1:8000",
    cloud_address: asString(record.cloud_address) ?? "https://farm.example.com",
    status: asString(record.status) ?? "unknown",
    last_switch: asDateOrNull(record.last_switch),
    timezone: asString(record.timezone) ?? null,
  };
};

const normalizeAnalyticsFeedStatus = (raw: ApiRecord): AnalyticsFeedStatus => {
  const enabled = Boolean(raw.enabled);
  const feedsRaw = raw.feeds && typeof raw.feeds === "object" ? (raw.feeds as ApiRecord) : {};
  const feeds = Object.fromEntries(
    Object.entries(feedsRaw).map(([key, value]) => {
      const record = value as ApiRecord;
      return [
        key,
        {
          status: asString(record.status) ?? null,
          details: asString(record.details) ?? null,
          last_seen: asDateOrNull(record.last_seen),
        },
      ];
    }),
  );
  const history = asArray<ApiRecord>(raw.history).map((entry) => ({
    name: asString(entry.name) ?? "",
    category: asString(entry.category) ?? "",
    status: asString(entry.status) ?? "",
    recorded_at: asDate(entry.recorded_at ?? new Date(), new Date()),
    meta: (entry.meta as ApiRecord) ?? undefined,
  }));
  return {
    enabled,
    feeds,
    history,
  };
};

export async function fetchNodes(): Promise<DemoNode[]> {
  const raw = await fetchJsonValidated("/api/nodes", NodesResponseSchema);
  return normalizeNodes(raw);
}

export async function updateNodeOrder(nodeIds: string[]): Promise<void> {
  if (!nodeIds.length) {
    return;
  }
  await putJson<void>("/api/nodes/order", { node_ids: nodeIds });
}

export async function updateNodeSensorsOrder(nodeId: string, sensorIds: string[]): Promise<void> {
  if (!sensorIds.length) {
    return;
  }
  await putJson<void>(`/api/nodes/${encodeURIComponent(nodeId)}/sensors/order`, {
    sensor_ids: sensorIds,
  });
}

export async function getNodeDisplayProfile(nodeId: string): Promise<NodeDisplayProfile> {
  return fetchJsonValidated(`/api/nodes/${nodeId}/display`, NodeDisplayProfileSchema);
}

export async function updateNodeDisplayProfile(
  nodeId: string,
  profile: NodeDisplayProfile,
): Promise<UpdateNodeDisplayProfileResponse> {
  const path = `/api/nodes/${nodeId}/display`;
  const raw = await putJson<unknown>(path, profile);
  return parseApiResponse(UpdateNodeDisplayProfileResponseSchema, raw, path);
}

export async function getNodeSensorsConfig(nodeId: string): Promise<NodeSensorsConfigResponse> {
  return fetchJsonValidated(`/api/nodes/${nodeId}/sensors/config`, NodeSensorsConfigResponseSchema);
}

export async function updateNodeSensorsConfig(
  nodeId: string,
  sensors: NodeSensorDraft[],
  ads1263?: NodeAds1263SettingsDraft | null,
): Promise<ApplyNodeSensorsConfigResponse> {
  const path = `/api/nodes/${nodeId}/sensors/config`;
  const raw = await putJson<unknown>(path, { sensors, ads1263: ads1263 ?? undefined });
  return parseApiResponse(ApplyNodeSensorsConfigResponseSchema, raw, path);
}

export async function fetchSensors(): Promise<DemoSensor[]> {
  const raw = await fetchJsonValidated("/api/sensors", SensorsResponseSchema);
  return normalizeSensors(raw);
}

export async function fetchOutputs(): Promise<DemoOutput[]> {
  const raw = await fetchJsonValidated("/api/outputs", OutputsResponseSchema);
  return normalizeOutputs(raw);
}

export async function fetchSchedules(): Promise<DemoSchedule[]> {
  const raw = await fetchJsonValidated("/api/schedules", SchedulesResponseSchema);
  return normalizeSchedules(raw);
}

export async function fetchScheduleCalendar(
  start: string,
  end: string,
): Promise<ScheduleCalendarEventRaw[]> {
  return fetchJsonValidated(
    `/api/schedules/calendar?start=${start}&end=${end}`,
    ScheduleCalendarResponseSchema,
  );
}

export async function fetchAlarms(): Promise<DemoAlarm[]> {
  const raw = await fetchJsonValidated("/api/alarms", AlarmsResponseSchema);
  return normalizeAlarms(raw);
}

export async function fetchAlarmRules(): Promise<AlarmRule[]> {
  const raw = await fetchJsonValidated("/api/alarm-rules", AlarmRulesResponseSchema);
  return normalizeAlarmRules(raw as unknown as ApiRecord[]);
}

export async function createAlarmRule(payload: AlarmRuleCreateRequest): Promise<AlarmRule> {
  const raw = await postJson<unknown>("/api/alarm-rules", payload);
  const parsed = parseApiResponse(AlarmRuleSchema, raw, "/api/alarm-rules");
  return normalizeAlarmRules([parsed as unknown as ApiRecord])[0];
}

export async function updateAlarmRule(
  id: number,
  payload: AlarmRuleUpdateRequest,
): Promise<AlarmRule> {
  const path = `/api/alarm-rules/${id}`;
  const raw = await putJson<unknown>(path, payload);
  const parsed = parseApiResponse(AlarmRuleSchema, raw, path);
  return normalizeAlarmRules([parsed as unknown as ApiRecord])[0];
}

export async function deleteAlarmRule(id: number): Promise<void> {
  await deleteJson(`/api/alarm-rules/${id}`);
}

export async function enableAlarmRule(id: number): Promise<AlarmRule> {
  const path = `/api/alarm-rules/${id}/enable`;
  const raw = await postJson<unknown>(path);
  const parsed = parseApiResponse(AlarmRuleSchema, raw, path);
  return normalizeAlarmRules([parsed as unknown as ApiRecord])[0];
}

export async function disableAlarmRule(id: number): Promise<AlarmRule> {
  const path = `/api/alarm-rules/${id}/disable`;
  const raw = await postJson<unknown>(path);
  const parsed = parseApiResponse(AlarmRuleSchema, raw, path);
  return normalizeAlarmRules([parsed as unknown as ApiRecord])[0];
}

export async function previewAlarmRule(
  payload: AlarmRuleCreateRequest,
): Promise<AlarmRulePreviewResponse> {
  const raw = await postJson<unknown>("/api/alarm-rules/preview", {
    target_selector: payload.target_selector,
    condition_ast: payload.condition_ast,
    timing: payload.timing ?? {},
  });
  return parseApiResponse(
    AlarmRulePreviewResponseSchema,
    raw,
    "/api/alarm-rules/preview",
  ) as unknown as AlarmRulePreviewResponse;
}

export async function fetchAlarmEvents(limit = 100): Promise<DemoAlarmEvent[]> {
  const raw = await fetchJsonValidated(
    `/api/alarms/history?limit=${limit}`,
    AlarmEventsResponseSchema,
  );
  return normalizeAlarmEvents(raw);
}

export type FetchIncidentsParams = {
  status?: string;
  severity?: string;
  assigned_to?: string;
  unassigned?: boolean;
  from?: string;
  to?: string;
  search?: string;
  limit?: number;
  cursor?: string;
};

export async function fetchIncidents(params: FetchIncidentsParams = {}): Promise<IncidentsListResponse> {
  const query = new URLSearchParams();
  if (params.status) query.set("status", params.status);
  if (params.severity) query.set("severity", params.severity);
  if (params.assigned_to) query.set("assigned_to", params.assigned_to);
  if (params.unassigned) query.set("unassigned", "true");
  if (params.from) query.set("from", params.from);
  if (params.to) query.set("to", params.to);
  if (params.search) query.set("search", params.search);
  if (typeof params.limit === "number") query.set("limit", String(params.limit));
  if (params.cursor) query.set("cursor", params.cursor);
  const suffix = query.toString();

  const raw = await fetchJsonValidated(
    `/api/incidents${suffix ? `?${suffix}` : ""}`,
    IncidentsListResponseSchema,
  );

  const incidents = normalizeIncidents(raw.incidents as unknown as ApiRecord[]);
  const next_cursor = asString(raw.next_cursor) ?? null;
  return { incidents, next_cursor };
}

export async function fetchIncidentDetail(incidentId: string): Promise<{ incident: Incident; events: DemoAlarmEvent[] }> {
  const raw = await fetchJsonValidated(
    `/api/incidents/${encodeURIComponent(incidentId)}`,
    IncidentDetailResponseSchema,
  );
  const incident = normalizeIncidents([raw.incident as unknown as ApiRecord])[0]!;
  const events = normalizeAlarmEvents(raw.events as unknown as ApiRecord[]);
  return { incident, events };
}

export async function assignIncident(incidentId: string, userId: string | null): Promise<Incident> {
  const raw = await postJson<unknown>(`/api/incidents/${encodeURIComponent(incidentId)}/assign`, { user_id: userId });
  const parsed = parseApiResponse(IncidentSchema, raw, `/api/incidents/${incidentId}/assign`);
  return normalizeIncidents([parsed as unknown as ApiRecord])[0]!;
}

export async function snoozeIncident(incidentId: string, until: string | null): Promise<Incident> {
  const raw = await postJson<unknown>(`/api/incidents/${encodeURIComponent(incidentId)}/snooze`, { until });
  const parsed = parseApiResponse(IncidentSchema, raw, `/api/incidents/${incidentId}/snooze`);
  return normalizeIncidents([parsed as unknown as ApiRecord])[0]!;
}

export async function closeIncident(incidentId: string, closed: boolean): Promise<Incident> {
  const raw = await postJson<unknown>(`/api/incidents/${encodeURIComponent(incidentId)}/close`, { closed });
  const parsed = parseApiResponse(IncidentSchema, raw, `/api/incidents/${incidentId}/close`);
  return normalizeIncidents([parsed as unknown as ApiRecord])[0]!;
}

export async function fetchIncidentNotes(
  incidentId: string,
  options: { limit?: number; cursor?: string } = {},
): Promise<IncidentNotesListResponse> {
  const query = new URLSearchParams();
  if (typeof options.limit === "number") query.set("limit", String(options.limit));
  if (options.cursor) query.set("cursor", options.cursor);
  const suffix = query.toString();

  const raw = await fetchJsonValidated(
    `/api/incidents/${encodeURIComponent(incidentId)}/notes${suffix ? `?${suffix}` : ""}`,
    IncidentNotesListResponseSchema,
  );
  return {
    notes: normalizeIncidentNotes(raw.notes as unknown as ApiRecord[]),
    next_cursor: asString(raw.next_cursor) ?? null,
  };
}

export async function createIncidentNote(incidentId: string, body: string): Promise<IncidentNote> {
  const raw = await postJson<unknown>(`/api/incidents/${encodeURIComponent(incidentId)}/notes`, { body });
  const parsed = parseApiResponse(IncidentNoteSchema, raw, `/api/incidents/${incidentId}/notes`);
  return normalizeIncidentNotes([parsed as unknown as ApiRecord])[0]!;
}

export async function fetchActionLogs(params: { from: string; to: string; node_id?: string; schedule_id?: string; limit?: number }): Promise<ActionLog[]> {
  const query = new URLSearchParams();
  query.set("from", params.from);
  query.set("to", params.to);
  if (params.node_id) query.set("node_id", params.node_id);
  if (params.schedule_id) query.set("schedule_id", params.schedule_id);
  if (typeof params.limit === "number") query.set("limit", String(params.limit));
  const raw = await fetchJsonValidated(`/api/action-logs?${query.toString()}`, ActionLogsResponseSchema);
  return normalizeActionLogs(raw as unknown as ApiRecord[]);
}

export async function fetchAlarmRuleStats(payload: AlarmRuleStatsRequest): Promise<AlarmRuleStatsResponse> {
  const raw = await postJson<unknown>("/api/alarm-rules/stats", payload);
  return parseApiResponse(
    AlarmRuleStatsResponseSchema,
    raw,
    "/api/alarm-rules/stats",
  ) as unknown as AlarmRuleStatsResponse;
}

export async function fetchUsers(): Promise<DemoUser[]> {
  const raw = await fetchJsonValidated("/api/users", UsersResponseSchema);
  return normalizeUsers(raw);
}

export async function fetchConnection(): Promise<DemoConnection> {
  const raw = await fetchJsonValidated("/api/connection", ConnectionSchema);
  return normalizeConnection(raw);
}

export async function fetchAdoptionCandidates(): Promise<DemoAdoptionCandidate[]> {
  return fetchJsonValidated("/api/scan", AdoptionCandidatesResponseSchema);
}

export async function fetchBackups(): Promise<DemoBackup[]> {
  const raw = await fetchJsonValidated("/api/backups", BackupsResponseSchema);
  return flattenBackups(raw);
}

export async function createWs2902Integration(
  request: Ws2902CreateRequest,
): Promise<Ws2902CreateResponse> {
  const raw = await postJson<unknown>("/api/weather-stations/ws-2902", request);
  return parseApiResponse(Ws2902CreateResponseSchema, raw, "/api/weather-stations/ws-2902");
}

export async function getWs2902IntegrationStatus(
  integrationId: string,
): Promise<Ws2902StatusResponse> {
  const raw = await fetchJson<unknown>(`/api/weather-stations/ws-2902/${integrationId}`);
  return parseApiResponse(
    Ws2902StatusResponseSchema,
    raw,
    `/api/weather-stations/ws-2902/${integrationId}`,
  );
}

export async function rotateWs2902IntegrationToken(
  integrationId: string,
): Promise<Ws2902RotateTokenResponse> {
  const raw = await postJson<unknown>(
    `/api/weather-stations/ws-2902/${integrationId}/rotate-token`,
  );
  return parseApiResponse(
    Ws2902RotateTokenResponseSchema,
    raw,
    `/api/weather-stations/ws-2902/${integrationId}/rotate-token`,
  );
}

export async function getWs2902IntegrationStatusByNode(
  nodeId: string,
): Promise<Ws2902StatusResponse> {
  const path = `/api/weather-stations/ws-2902/node/${encodeURIComponent(nodeId)}`;
  const raw = await fetchJson<unknown>(path);
  return parseApiResponse(Ws2902StatusResponseSchema, raw, path);
}

export async function rotateWs2902IntegrationTokenByNode(
  nodeId: string,
): Promise<Ws2902RotateTokenResponse> {
  const path = `/api/weather-stations/ws-2902/node/${encodeURIComponent(nodeId)}/rotate-token`;
  const raw = await postJson<unknown>(path);
  return parseApiResponse(Ws2902RotateTokenResponseSchema, raw, path);
}

export async function applyRenogyBt2Preset(
  nodeId: string,
  request: ApplyRenogyBt2PresetRequest,
): Promise<ApplyRenogyBt2PresetResponse> {
  const raw = await postJson<unknown>(`/api/nodes/${nodeId}/presets/renogy-bt2`, request);
  return parseApiResponse(
    ApplyRenogyBt2PresetResponseSchema,
    raw,
    `/api/nodes/${nodeId}/presets/renogy-bt2`,
  );
}

export async function fetchRenogySettingsSchema(
  nodeId: string,
): Promise<RenogyRegisterMapSchemaType> {
  return fetchJsonValidated(
    `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/schema`,
    RenogyRegisterMapSchema,
  );
}

export async function fetchRenogyDesiredSettings(
  nodeId: string,
): Promise<RenogyDesiredSettingsResponseType> {
  return fetchJsonValidated(
    `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/desired`,
    RenogyDesiredSettingsResponseSchema,
  );
}

export async function updateRenogyDesiredSettings(
  nodeId: string,
  desired: Record<string, unknown>,
): Promise<RenogyDesiredSettingsResponseType> {
  const path = `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/desired`;
  const raw = await putJson<unknown>(path, { desired });
  return parseApiResponse(RenogyDesiredSettingsResponseSchema, raw, path);
}

export async function validateRenogyDesiredSettings(
  nodeId: string,
  desired: Record<string, unknown>,
): Promise<RenogyValidateResponseType> {
  const path = `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/validate`;
  const raw = await postJson<unknown>(path, { desired });
  return parseApiResponse(RenogyValidateResponseSchema, raw, path);
}

export async function readRenogyCurrentSettings(
  nodeId: string,
): Promise<RenogyReadCurrentResponseType> {
  const path = `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/read`;
  const raw = await postJson<unknown>(path, {});
  return parseApiResponse(RenogyReadCurrentResponseSchema, raw, path);
}

export async function applyRenogySettings(
  nodeId: string,
): Promise<RenogyApplyResponseType> {
  const path = `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/apply`;
  const raw = await postJson<unknown>(path, {});
  return parseApiResponse(RenogyApplyResponseSchema, raw, path);
}

export async function fetchRenogySettingsHistory(
  nodeId: string,
): Promise<RenogyHistoryEntryType[]> {
  return fetchJsonValidated(
    `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/history`,
    RenogyHistoryResponseSchema,
  );
}

export async function rollbackRenogyDesiredSettings(
  nodeId: string,
  eventId: number,
): Promise<RenogyDesiredSettingsResponseType> {
  const path = `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/rollback`;
  const raw = await postJson<unknown>(path, { event_id: eventId });
  return parseApiResponse(RenogyDesiredSettingsResponseSchema, raw, path);
}

export async function setRenogyMaintenanceMode(
  nodeId: string,
  enabled: boolean,
): Promise<RenogyDesiredSettingsResponseType> {
  const path = `/api/nodes/${encodeURIComponent(nodeId)}/renogy-bt2/settings/maintenance`;
  const raw = await putJson<unknown>(path, { enabled });
  return parseApiResponse(RenogyDesiredSettingsResponseSchema, raw, path);
}

export type BackupRunResponse = {
  status: string;
  reason?: string;
};

export async function runBackupNow(): Promise<BackupRunResponse> {
  const raw = await postJson<unknown>("/api/backups/run", {});
  return parseApiResponse(BackupRunResponseSchema, raw, "/api/backups/run");
}

export async function fetchMetricsSeries(
  sensorIds: string[],
  start: string,
  end: string,
  interval: number,
  init?: RequestInit,
): Promise<TrendSeriesEntry[]> {
  if (!sensorIds.length) {
    return [];
  }

  const path = buildMetricsQuery(sensorIds, start, end, interval);
  const buffer = await fetchBinary(path, init);
  const decoded = decodeBinaryMetrics(buffer);

  return convertBinaryToTrendSeries(decoded, interval);
}

function convertBinaryToTrendSeries(
  decoded: ReturnType<typeof decodeBinaryMetrics>,
  intervalSeconds: number,
): TrendSeriesEntry[] {
  return decoded.map((series) => {
    // Read all points as [timestamp_ms, value] pairs
    const rawPoints: Array<[number, number]> = [];
    for (let i = 0; i < series.point_count; i++) {
      rawPoints.push(readBinaryPoint(series, i));
    }

    // Insert null gaps for time skips (optimized: data is pre-sorted, work with ms numbers)
    const pointsWithGaps = addNullGapsForTimeSkipsBinary(rawPoints, intervalSeconds);

    return {
      sensor_id: series.sensor_id,
      label: series.sensor_name ?? undefined,
      unit: undefined,
      display_decimals: undefined,
      points: pointsWithGaps,
    };
  });
}

/**
 * Optimized gap detection for pre-sorted binary data.
 * Works with timestamp-ms numbers directly, only creating Date objects in output.
 */
function addNullGapsForTimeSkipsBinary(
  rawPoints: Array<[number, number]>,
  intervalSeconds: number,
): TrendSeriesEntry["points"] {
  if (rawPoints.length < 2) {
    return rawPoints.map(([ts, value]) => ({
      timestamp: new Date(ts),
      value: Number.isFinite(value) ? value : null,
      samples: undefined,
    }));
  }

  const interval = Number.isFinite(intervalSeconds) ? Math.max(1, Math.floor(intervalSeconds)) : 1;

  // Compute median delta from a sample for gap threshold
  const sampleSize = Math.min(rawPoints.length - 1, 500);
  const step = Math.max(1, Math.floor((rawPoints.length - 1) / sampleSize));
  const deltas: number[] = [];
  for (let i = step; i < rawPoints.length; i += step) {
    const deltaMs = rawPoints[i][0] - rawPoints[i - step][0];
    const deltaSec = deltaMs / (1000 * step);
    if (Number.isFinite(deltaSec) && deltaSec > 0) deltas.push(deltaSec);
  }
  deltas.sort((a, b) => a - b);
  const typicalDelta = deltas.length >= 5 ? deltas[Math.floor(deltas.length / 2)] : null;
  const baseDelta = Math.max(interval, typicalDelta ?? interval);
  const gapThresholdMs = baseDelta * 2.5 * 1000;

  const output: TrendSeriesEntry["points"] = [];
  for (let i = 0; i < rawPoints.length; i++) {
    const [ts, value] = rawPoints[i];
    const finiteValue = Number.isFinite(value) ? value : null;
    output.push({ timestamp: new Date(ts), value: finiteValue, samples: undefined });

    if (i + 1 < rawPoints.length && finiteValue !== null) {
      const [nextTs, nextVal] = rawPoints[i + 1];
      if (Number.isFinite(nextVal)) {
        const deltaMs = nextTs - ts;
        if (deltaMs > gapThresholdMs) {
          const gapTs = ts + interval * 1000;
          if (gapTs < nextTs) {
            output.push({ timestamp: new Date(gapTs), value: null, samples: 0 });
          }
        }
      }
    }
  }

  return output;
}

const normalizeSnapshot = (snapshot: DashboardSnapshotRaw): DashboardSnapshot => ({
  ...snapshot,
  timestamp: asDate(snapshot.timestamp ?? new Date(), new Date()),
  nodes: normalizeNodes(snapshot.nodes as unknown as Array<ApiRecord | DemoNode>),
  sensors: normalizeSensors(snapshot.sensors as unknown as ApiRecord[]),
  outputs: normalizeOutputs(snapshot.outputs as unknown as ApiRecord[]),
  schedules: normalizeSchedules(snapshot.schedules as unknown as ApiRecord[]),
  alarms: normalizeAlarms(snapshot.alarms as unknown as ApiRecord[]),
  alarm_events: normalizeAlarmEvents(snapshot.alarm_events as unknown as ApiRecord[]),
  users: normalizeUsers(snapshot.users as unknown as ApiRecord[]),
  connection: normalizeConnection(snapshot.connection as unknown as ApiRecord),
  analytics: normalizeAnalyticsBundle(snapshot.analytics),
  backups: flattenBackups(snapshot.backups as unknown as ApiRecord[]),
  backup_retention: snapshot.backup_retention
    ? normalizeRetentionConfig(snapshot.backup_retention as unknown as ApiRecord)
    : undefined,
  trend_series:
    snapshot.trend_series && snapshot.trend_series.length
      ? normalizeTrendSeries(snapshot.trend_series as unknown as ApiRecord[])
      : [],
});

export async function fetchDashboardSnapshot(): Promise<DashboardSnapshot> {
  const snapshot = await fetchJsonValidated("/api/dashboard/state", DashboardSnapshotSchema);
  return normalizeSnapshot(snapshot);
}

export interface BackupRetentionUpdatePayload {
  node_id: string;
  keep_days: number | null;
}

export async function fetchPredictiveTrace(): Promise<PredictiveTraceEntry[]> {
  return fetchJsonValidated("/api/predictive/trace", PredictiveTraceResponseSchema);
}

export async function fetchPredictiveStatus(): Promise<PredictiveStatus> {
  const payload = await fetchJsonValidated("/api/predictive/status", PredictiveStatusSchema);
  return payload as PredictiveStatus;
}

export interface PredictiveConfigUpdatePayload {
  enabled?: boolean;
  api_base_url?: string;
  model?: string | null;
  api_token?: string;
}

export async function updatePredictiveConfig(
  payload: PredictiveConfigUpdatePayload,
): Promise<PredictiveStatus> {
  const raw = await putJson<unknown>("/api/predictive/config", payload);
  return parseApiResponse(PredictiveStatusSchema, raw, "/api/predictive/config") as PredictiveStatus;
}

export async function fetchBackupRetentionConfig(): Promise<BackupRetentionConfig> {
  const raw = await fetchJsonValidated("/api/backups/retention", BackupRetentionConfigSchema);
  return normalizeRetentionConfig(raw);
}

export async function updateBackupRetentionPolicies(
  updates: BackupRetentionUpdatePayload[],
): Promise<BackupRetentionConfig> {
  const payload = {
    policies: updates.map((entry) => ({
      node_id: entry.node_id,
      keep_days: entry.keep_days ?? null,
    })),
  };
  const raw = await putJson<unknown>("/api/backups/retention", payload);
  const parsed = parseApiResponse(BackupRetentionConfigSchema, raw, "/api/backups/retention");
  return normalizeRetentionConfig(parsed);
}

export async function fetchRecentRestores() {
  return fetchJsonValidated("/api/restores/recent", RecentRestoresResponseSchema);
}

export async function fetchAnalyticsFeedStatus(): Promise<AnalyticsFeedStatus> {
  const raw = await fetchJsonValidated("/api/analytics/feeds/status", AnalyticsFeedStatusSchema);
  return normalizeAnalyticsFeedStatus(raw as ApiRecord);
}

export async function fetchForecastStatus(): Promise<ForecastStatus> {
  return fetchJsonValidated("/api/forecast/status", ForecastStatusSchema);
}

export async function pollForecasts(): Promise<{ status: string; providers: Record<string, string> }> {
  const raw = await postJson<unknown>("/api/forecast/poll", {});
  const parsed = parseApiResponse(ForecastPollResponseSchema, raw, "/api/forecast/poll");
  return {
    status: parsed.status,
    providers: (parsed.providers as Record<string, string> | undefined) ?? {},
  };
}

export async function fetchWeatherForecastConfig(): Promise<WeatherForecastConfig> {
  const raw = await fetchJsonValidated("/api/forecast/weather/config", WeatherForecastConfigSchema);
  return {
    enabled: Boolean((raw as ApiRecord).enabled),
    provider: asString((raw as ApiRecord).provider) ?? null,
    latitude:
      typeof (raw as ApiRecord).latitude === "number"
        ? ((raw as ApiRecord).latitude as number)
        : null,
    longitude:
      typeof (raw as ApiRecord).longitude === "number"
        ? ((raw as ApiRecord).longitude as number)
        : null,
    updated_at: asString((raw as ApiRecord).updated_at) ?? null,
  };
}

export async function updateWeatherForecastConfig(payload: {
  enabled: boolean;
  latitude: number;
  longitude: number;
  provider?: string;
}): Promise<WeatherForecastConfig> {
  const raw = await putJson<unknown>("/api/forecast/weather/config", payload);
  const parsed = parseApiResponse(
    WeatherForecastConfigSchema,
    raw,
    "/api/forecast/weather/config",
  );
  return {
    enabled: Boolean((parsed as ApiRecord).enabled),
    provider: asString((parsed as ApiRecord).provider) ?? null,
    latitude:
      typeof (parsed as ApiRecord).latitude === "number"
        ? ((parsed as ApiRecord).latitude as number)
        : null,
    longitude:
      typeof (parsed as ApiRecord).longitude === "number"
        ? ((parsed as ApiRecord).longitude as number)
        : null,
    updated_at: asString((parsed as ApiRecord).updated_at) ?? null,
  };
}

export async function fetchWeatherForecastHourly(hours: number): Promise<ForecastSeriesResponse | null> {
  const params = new URLSearchParams();
  params.set("hours", String(hours));
  return fetchJsonOptional(
    `/api/forecast/weather/hourly?${params.toString()}`,
    ForecastSeriesResponseSchema,
  );
}

export async function fetchWeatherForecastDaily(days: number): Promise<ForecastSeriesResponse | null> {
  const params = new URLSearchParams();
  params.set("days", String(days));
  return fetchJsonOptional(
    `/api/forecast/weather/daily?${params.toString()}`,
    ForecastSeriesResponseSchema,
  );
}

export async function fetchCurrentWeather(nodeId: string): Promise<CurrentWeatherResponse | null> {
  const params = new URLSearchParams();
  params.set("node_id", nodeId);
  return fetchJsonOptional(
    `/api/forecast/weather/current?${params.toString()}`,
    CurrentWeatherResponseSchema,
  );
}

export async function fetchPvForecastConfig(nodeId: string): Promise<PvForecastConfig | null> {
  return fetchJsonOptional(`/api/forecast/pv/config/${encodeURIComponent(nodeId)}`, PvForecastConfigSchema);
}

export async function updatePvForecastConfig(
  nodeId: string,
  payload: {
    enabled: boolean;
    latitude: number;
    longitude: number;
    tilt_deg: number;
    azimuth_deg: number;
    kwp: number;
    time_format?: string;
  },
): Promise<PvForecastConfig> {
  const raw = await putJson<unknown>(
    `/api/forecast/pv/config/${encodeURIComponent(nodeId)}`,
    payload,
  );
  return parseApiResponse(
    PvForecastConfigSchema,
    raw,
    `/api/forecast/pv/config/${encodeURIComponent(nodeId)}`,
  );
}

export async function fetchBatteryConfig(nodeId: string): Promise<BatteryConfigResponse> {
  return fetchJsonValidated(
    `/api/battery/config/${encodeURIComponent(nodeId)}`,
    BatteryConfigResponseSchema,
  );
}

export async function updateBatteryConfig(
  nodeId: string,
  payload: { battery_model: BatteryModelConfig },
): Promise<BatteryConfigResponse> {
  const path = `/api/battery/config/${encodeURIComponent(nodeId)}`;
  const raw = await putJson<unknown>(path, payload);
  return parseApiResponse(BatteryConfigResponseSchema, raw, path);
}

export async function fetchPowerRunwayConfig(nodeId: string): Promise<PowerRunwayConfigResponse> {
  return fetchJsonValidated(
    `/api/power/runway/config/${encodeURIComponent(nodeId)}`,
    PowerRunwayConfigResponseSchema,
  );
}

export async function updatePowerRunwayConfig(
  nodeId: string,
  payload: { power_runway: PowerRunwayConfig },
): Promise<PowerRunwayConfigResponse> {
  const path = `/api/power/runway/config/${encodeURIComponent(nodeId)}`;
  const raw = await putJson<unknown>(path, payload);
  return parseApiResponse(PowerRunwayConfigResponseSchema, raw, path);
}

export async function checkPvForecastPlane(payload: {
  latitude: number;
  longitude: number;
  tilt_deg: number;
  azimuth_deg: number;
  kwp: number;
}): Promise<{
  status: string;
  place: string | null;
  timezone: string | null;
  checked_at: string;
}> {
  const raw = await postJson<unknown>("/api/forecast/pv/check", payload);
  const parsed = parseApiResponse(
    PvForecastCheckResponseSchema,
    raw,
    "/api/forecast/pv/check",
  );
  return {
    status: String((parsed as ApiRecord).status ?? "unknown"),
    place:
      typeof (parsed as ApiRecord).place === "string"
        ? ((parsed as ApiRecord).place as string)
        : null,
    timezone:
      typeof (parsed as ApiRecord).timezone === "string"
        ? ((parsed as ApiRecord).timezone as string)
        : null,
    checked_at: String((parsed as ApiRecord).checked_at ?? ""),
  };
}

export async function fetchPvForecastHourly(
  nodeId: string,
  hours: number,
  historyHours = 0,
): Promise<ForecastSeriesResponse | null> {
  const params = new URLSearchParams();
  params.set("hours", String(hours));
  if (historyHours > 0) {
    params.set("history_hours", String(historyHours));
  }
  return fetchJsonOptional(
    `/api/forecast/pv/${encodeURIComponent(nodeId)}/hourly?${params.toString()}`,
    ForecastSeriesResponseSchema,
  );
}

export async function fetchPvForecastDaily(
  nodeId: string,
  days: number,
): Promise<ForecastSeriesResponse | null> {
  const params = new URLSearchParams();
  params.set("days", String(days));
  return fetchJsonOptional(
    `/api/forecast/pv/${encodeURIComponent(nodeId)}/daily?${params.toString()}`,
    ForecastSeriesResponseSchema,
  );
}

export async function fetchSetupCredentials(): Promise<SetupCredential[]> {
  const payload = await fetchJsonValidated("/api/setup/credentials", SetupCredentialsResponseSchema);
  return normalizeSetupCredentials((payload.credentials as ApiRecord[]) ?? []);
}

export async function fetchExternalDeviceCatalog(): Promise<ExternalDeviceCatalog> {
  return fetchJsonValidated("/api/integrations/devices/catalog", ExternalDeviceCatalogSchema);
}

export async function fetchExternalDevices(): Promise<ExternalDeviceSummary[]> {
  return fetchJsonValidated("/api/integrations/devices", ExternalDeviceSummariesSchema);
}

export async function createExternalDevice(
  request: ExternalDeviceCreateRequest,
): Promise<ExternalDeviceSummary> {
  const raw = await postJson<unknown>("/api/integrations/devices", request);
  return parseApiResponse(ExternalDeviceSummarySchema, raw, "/api/integrations/devices");
}

export async function syncExternalDevice(nodeId: string): Promise<void> {
  await postJson(`/api/integrations/devices/${encodeURIComponent(nodeId)}/sync`, {});
}

export async function deleteExternalDevice(nodeId: string): Promise<void> {
  await deleteJson(`/api/integrations/devices/${encodeURIComponent(nodeId)}`);
}

export async function loginEmporia(payload: {
  username: string;
  password: string;
  site_ids?: string[];
}): Promise<EmporiaLoginResult> {
  const raw = await postJson<unknown>("/api/setup/emporia/login", payload);
  const parsed = parseApiResponse(EmporiaLoginResponseSchema, raw, "/api/setup/emporia/login");
  const devices: EmporiaLoginResult["devices"] = Array.isArray(parsed.devices)
    ? (parsed.devices as ApiRecord[]).map((entry) => ({
        device_gid: asString(entry.device_gid) ?? "",
        name: entry.name != null ? asString(entry.name) ?? null : null,
        model: entry.model != null ? asString(entry.model) ?? null : null,
        firmware: entry.firmware != null ? asString(entry.firmware) ?? null : null,
        address: entry.address != null ? asString(entry.address) ?? null : null,
      }))
    : [];
  const siteIds = Array.isArray(parsed.site_ids)
    ? (parsed.site_ids as Array<string | number>)
        .map((item) => (typeof item === "number" ? String(item) : String(item ?? "")))
        .filter((item) => Boolean(item))
    : [];

  return {
    token_present: Boolean(parsed.token_present),
    site_ids: siteIds,
    devices,
  };
}

export async function fetchEmporiaDevices(): Promise<EmporiaDevicesResult> {
  const parsed = await fetchJsonValidated("/api/setup/emporia/devices", EmporiaDevicesResponseSchema);
  const record = parsed as ApiRecord;

  const devices: EmporiaDevicesResult["devices"] = Array.isArray(record.devices)
    ? (record.devices as ApiRecord[]).map((entry) => ({
        device_gid: asString(entry.device_gid) ?? "",
        name: entry.name != null ? asString(entry.name) ?? null : null,
        model: entry.model != null ? asString(entry.model) ?? null : null,
        firmware: entry.firmware != null ? asString(entry.firmware) ?? null : null,
        address: entry.address != null ? asString(entry.address) ?? null : null,
        enabled: Boolean(entry.enabled),
        hidden: Boolean(entry.hidden),
        include_in_power_summary: Boolean(entry.include_in_power_summary),
        group_label: entry.group_label != null ? asString(entry.group_label) ?? null : null,
        circuits: Array.isArray(entry.circuits)
          ? (entry.circuits as ApiRecord[]).map((circuit) => ({
              circuit_key: asString(circuit.circuit_key) ?? "",
              name: asString(circuit.name) ?? "",
              raw_channel_num: circuit.raw_channel_num != null ? asString(circuit.raw_channel_num) ?? null : null,
              nested_device_gid:
                circuit.nested_device_gid != null ? asString(circuit.nested_device_gid) ?? null : null,
              enabled: Boolean(circuit.enabled),
              hidden: Boolean(circuit.hidden),
              include_in_power_summary: Boolean(circuit.include_in_power_summary),
              is_mains: Boolean(circuit.is_mains),
            }))
          : [],
      }))
    : [];

  const siteIds = Array.isArray(record.site_ids)
    ? (record.site_ids as Array<string | number>)
        .map((item) => (typeof item === "number" ? String(item) : String(item ?? "")))
        .filter((item) => Boolean(item))
    : [];

  return {
    token_present: Boolean(record.token_present),
    site_ids: siteIds,
    devices,
  };
}

export async function updateEmporiaDevices(devices: EmporiaDeviceUpdate[]): Promise<void> {
  await putJson("/api/setup/emporia/devices", { devices });
}

export async function upsertSetupCredential(
  name: string,
  value: string,
  metadata: Record<string, unknown> = {},
): Promise<SetupCredential> {
  const raw = await putJson<unknown>(`/api/setup/credentials/${name}`, { value, metadata });
  const parsed = parseApiResponse(SetupCredentialSchema, raw, `/api/setup/credentials/${name}`);
  return normalizeSetupCredentials([parsed as ApiRecord])[0];
}

export async function deleteSetupCredential(name: string): Promise<void> {
  await deleteJson(`/api/setup/credentials/${name}`);
}

export function buildMetricsQuery(
  sensorIds: string[],
  start: string,
  end: string,
  interval: number,
) {
  const params = new URLSearchParams();
  sensorIds.forEach((id) => params.append("sensor_ids[]", id));
  params.set("start", start);
  params.set("end", end);
  params.set("interval", String(interval));
  params.set("format", "binary");
  return `/api/metrics/query?${params.toString()}`;
}

const normalizeMapSettings = (raw: unknown): MapSettings => {
  const record = raw as ApiRecord;
  return {
    active_save_id: typeof record.active_save_id === "number" ? record.active_save_id : Number(record.active_save_id ?? 0),
    active_save_name: asString(record.active_save_name) ?? "Map",
    active_base_layer_id: typeof record.active_base_layer_id === "number" ? record.active_base_layer_id : null,
    center_lat: typeof record.center_lat === "number" ? record.center_lat : 0,
    center_lng: typeof record.center_lng === "number" ? record.center_lng : 0,
    zoom: typeof record.zoom === "number" ? record.zoom : 0,
    bearing: typeof record.bearing === "number" ? record.bearing : 0,
    pitch: typeof record.pitch === "number" ? record.pitch : 0,
    updated_at: asString(record.updated_at) ?? null,
  };
};

const normalizeMapSave = (raw: unknown): MapSave => {
  const record = raw as ApiRecord;
  return {
    id: typeof record.id === "number" ? record.id : Number(record.id ?? 0),
    name: asString(record.name) ?? "Map",
    created_at: asString(record.created_at) ?? "",
    updated_at: asString(record.updated_at) ?? "",
  };
};

const normalizeMapLayer = (raw: unknown): MapLayer => {
  const record = raw as ApiRecord;
  const kind = asString(record.kind) ?? "overlay";
  const sourceType = asString(record.source_type) ?? "xyz";
  return {
    id: typeof record.id === "number" ? record.id : Number(record.id ?? 0),
    system_key: asString(record.system_key) ?? null,
    name: asString(record.name) ?? "Layer",
    kind: (kind === "base" ? "base" : "overlay") as MapLayer["kind"],
    source_type: (sourceType as MapLayer["source_type"]) ?? "xyz",
    config: (record.config as Record<string, unknown>) ?? {},
    opacity: typeof record.opacity === "number" ? record.opacity : 1,
    enabled: Boolean(record.enabled),
    z_index: typeof record.z_index === "number" ? record.z_index : 0,
    created_at: asString(record.created_at) ?? "",
    updated_at: asString(record.updated_at) ?? "",
  };
};

const normalizeMapFeature = (raw: unknown): MapFeature => {
  const record = raw as ApiRecord;
  const geometryRaw = record.geometry;
  const geometry =
    geometryRaw &&
    typeof geometryRaw === "object" &&
    "type" in geometryRaw &&
    typeof (geometryRaw as { type?: unknown }).type === "string"
      ? (geometryRaw as MapFeature["geometry"])
      : { type: "Point", coordinates: [0, 0] };
  return {
    id: typeof record.id === "number" ? record.id : Number(record.id ?? 0),
    node_id: asString(record.node_id) ?? null,
    sensor_id: asString(record.sensor_id) ?? null,
    geometry,
    properties: (record.properties as Record<string, unknown>) ?? {},
    created_at: asString(record.created_at) ?? "",
    updated_at: asString(record.updated_at) ?? "",
  };
};

const normalizeOfflineMapPack = (raw: unknown): OfflineMapPack => {
  const record = raw as ApiRecord;
  return {
    id: asString(record.id) ?? "",
    name: asString(record.name) ?? "",
    bounds: (record.bounds as Record<string, unknown>) ?? {},
    min_zoom: typeof record.min_zoom === "number" ? record.min_zoom : Number(record.min_zoom ?? 0),
    max_zoom: typeof record.max_zoom === "number" ? record.max_zoom : Number(record.max_zoom ?? 0),
    status: asString(record.status) ?? "unknown",
    progress: (record.progress as Record<string, unknown>) ?? {},
    error: asString(record.error) ?? null,
    updated_at: asString(record.updated_at) ?? "",
  };
};

export async function fetchMapSettings(): Promise<MapSettings> {
  const raw = await fetchJsonValidated("/api/map/settings", MapSettingsSchema);
  return normalizeMapSettings(raw);
}

export async function updateMapSettings(payload: {
  active_base_layer_id: number | null;
  center_lat: number;
  center_lng: number;
  zoom: number;
  bearing?: number;
  pitch?: number;
}): Promise<MapSettings> {
  const raw = await putJson<unknown>("/api/map/settings", payload);
  const parsed = parseApiResponse(MapSettingsSchema, raw, "/api/map/settings");
  return normalizeMapSettings(parsed);
}

export async function fetchMapSaves(): Promise<MapSave[]> {
  const raw = await fetchJsonValidated("/api/map/saves", MapSavesResponseSchema);
  return (raw as unknown[]).map((entry) => normalizeMapSave(entry));
}

export async function createMapSave(payload: {
  name: string;
  active_base_layer_id?: number | null;
  center_lat?: number;
  center_lng?: number;
  zoom?: number;
  bearing?: number;
  pitch?: number;
}): Promise<MapSave> {
  const raw = await postJson<unknown>("/api/map/saves", payload);
  const parsed = parseApiResponse(MapSaveSchema, raw, "/api/map/saves");
  return normalizeMapSave(parsed);
}

export async function applyMapSave(id: number): Promise<MapSettings> {
  const raw = await postJson<unknown>(`/api/map/saves/${id}/apply`, {});
  const parsed = parseApiResponse(MapSettingsSchema, raw, `/api/map/saves/${id}/apply`);
  return normalizeMapSettings(parsed);
}

export async function fetchMapLayers(): Promise<MapLayer[]> {
  const raw = await fetchJsonValidated("/api/map/layers", MapLayersResponseSchema);
  return (raw as unknown[]).map((entry) => normalizeMapLayer(entry));
}

export async function createMapLayer(payload: MapLayerUpsertPayload): Promise<MapLayer> {
  const raw = await postJson<unknown>("/api/map/layers", payload);
  const parsed = parseApiResponse(MapLayerSchema, raw, "/api/map/layers");
  return normalizeMapLayer(parsed);
}

export async function updateMapLayer(id: number, payload: MapLayerUpsertPayload): Promise<MapLayer> {
  const raw = await putJson<unknown>(`/api/map/layers/${id}`, payload);
  const parsed = parseApiResponse(MapLayerSchema, raw, `/api/map/layers/${id}`);
  return normalizeMapLayer(parsed);
}

export async function deleteMapLayer(id: number): Promise<void> {
  await deleteJson(`/api/map/layers/${id}`);
}

export async function fetchMapFeatures(): Promise<MapFeature[]> {
  const raw = await fetchJsonValidated("/api/map/features", MapFeaturesResponseSchema);
  return (raw as unknown[]).map((entry) => normalizeMapFeature(entry));
}

export async function createMapFeature(payload: MapFeatureUpsertPayload): Promise<MapFeature> {
  const raw = await postJson<unknown>("/api/map/features", payload);
  const parsed = parseApiResponse(MapFeatureSchema, raw, "/api/map/features");
  return normalizeMapFeature(parsed);
}

export async function updateMapFeature(
  id: number,
  payload: { geometry: unknown; properties?: Record<string, unknown> },
): Promise<MapFeature> {
  const raw = await putJson<unknown>(`/api/map/features/${id}`, payload);
  const parsed = parseApiResponse(MapFeatureSchema, raw, `/api/map/features/${id}`);
  return normalizeMapFeature(parsed);
}

export async function deleteMapFeature(id: number): Promise<void> {
  await deleteJson(`/api/map/features/${id}`);
}

export async function fetchOfflineMapPacks(): Promise<OfflineMapPack[]> {
  const raw = await fetchJsonValidated("/api/map/offline/packs", OfflineMapPacksResponseSchema);
  return (raw as unknown[]).map((entry) => normalizeOfflineMapPack(entry));
}

export async function installOfflineMapPack(packId: string): Promise<OfflineMapPack> {
  const id = packId.trim();
  const raw = await postJson<unknown>(`/api/map/offline/packs/${encodeURIComponent(id)}/install`, {});
  const parsed = parseApiResponse(OfflineMapPackSchema, raw, `/api/map/offline/packs/${id}/install`);
  return normalizeOfflineMapPack(parsed);
}

export async function fetchDevActivityStatus(): Promise<DevActivityStatus> {
  const raw = await fetchJsonValidated("/api/dev/activity", DevActivityStatusResponseSchema);
  const record = raw as ApiRecord;
  return {
    active: Boolean(record.active),
    message: asString(record.message) ?? null,
    updated_at: asString(record.updated_at) ?? null,
    expires_at: asString(record.expires_at) ?? null,
  };
}

// ---------------------------------------------------------------------------
// Chart Annotations
// ---------------------------------------------------------------------------

export type ChartAnnotationPayload = {
  chart_state: Record<string, unknown>;
  sensor_ids?: string[];
  time_start?: string;
  time_end?: string;
  label?: string;
};

export type ChartAnnotationRow = {
  id: string;
  chart_state: Record<string, unknown>;
  sensor_ids?: string[];
  time_start?: string;
  time_end?: string;
  label?: string;
  created_by?: string;
  created_at: string;
  updated_at: string;
};

export async function fetchChartAnnotations(): Promise<ChartAnnotationRow[]> {
  return fetchJson<ChartAnnotationRow[]>("/api/chart-annotations");
}

export async function createChartAnnotation(
  payload: ChartAnnotationPayload,
): Promise<ChartAnnotationRow> {
  return postJson<ChartAnnotationRow>("/api/chart-annotations", payload);
}

export async function updateChartAnnotation(
  id: string,
  payload: Partial<ChartAnnotationPayload>,
): Promise<ChartAnnotationRow> {
  return putJson<ChartAnnotationRow>(`/api/chart-annotations/${encodeURIComponent(id)}`, payload);
}

export async function deleteChartAnnotation(id: string): Promise<void> {
  await deleteJson(`/api/chart-annotations/${encodeURIComponent(id)}`);
}
