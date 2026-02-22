import { z } from "zod";

export type ApiSchema<T> = z.ZodType<T>;

export class ApiSchemaError extends Error {
  issues: z.ZodIssue[];
  path: string;

  constructor(path: string, issues: z.ZodIssue[]) {
    super(`API schema validation failed for ${path}`);
    this.name = "ApiSchemaError";
    this.path = path;
    this.issues = issues;
  }
}

export function parseApiResponse<T>(schema: ApiSchema<T>, data: unknown, path: string): T {
  const result = schema.safeParse(data);
  if (!result.success) {
    throw new ApiSchemaError(path, result.error.issues);
  }
  return result.data;
}

const RecordSchema = z.record(z.string(), z.unknown());
const NullableString = z.string().nullable().optional();
const OptionalString = z.string().optional();
const OptionalNumber = z.number().optional();
const OptionalNullableNumber = z.number().nullable().optional();

export const NodeSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    status: z.string(),
    uptime_seconds: OptionalNullableNumber.default(null),
    cpu_percent: OptionalNullableNumber.default(null),
    storage_used_bytes: OptionalNullableNumber.default(null),
    memory_percent: OptionalNullableNumber.default(null),
    memory_used_bytes: OptionalNullableNumber.default(null),
    ping_ms: OptionalNullableNumber.default(null),
    ping_p50_30m_ms: OptionalNullableNumber.default(null),
    ping_jitter_ms: OptionalNullableNumber.default(null),
    mqtt_broker_rtt_ms: OptionalNullableNumber.default(null),
    mqtt_broker_rtt_jitter_ms: OptionalNullableNumber.default(null),
    network_latency_ms: OptionalNullableNumber.default(null),
    network_jitter_ms: OptionalNullableNumber.default(null),
    uptime_percent_24h: OptionalNullableNumber.default(null),
    mac_eth: NullableString,
    mac_wifi: NullableString,
    ip_last: NullableString,
    last_seen: NullableString,
    created_at: NullableString,
    config: RecordSchema.optional(),
  })
  .passthrough();

export const NodesResponseSchema = z.array(NodeSchema);

export const DisplayTileTypeSchema = z.enum([
  "core_status",
  "latency",
  "sensor",
  "sensors",
  "trends",
  "outputs",
]);

export const DisplayTileSchema = z
  .object({
    type: DisplayTileTypeSchema,
    sensor_id: OptionalString,
  })
  .passthrough();

export const DisplayTrendRangeSchema = z.enum(["1h", "6h", "24h"]);

export const DisplayTrendConfigSchema = z
  .object({
    sensor_id: z.string(),
    default_range: DisplayTrendRangeSchema.optional().default("6h"),
  })
  .passthrough();

export const NodeDisplayProfileSchema = z
  .object({
    schema_version: z.number().optional().default(1),
    enabled: z.boolean(),
    kiosk_autostart: z.boolean(),
    ui_refresh_seconds: z.number(),
    latency_sample_seconds: z.number(),
    latency_window_samples: z.number(),
    tiles: z.array(DisplayTileSchema).default([]),
    outputs_enabled: z.boolean(),
    local_pin_hash: NullableString,
    trend_ranges: z.array(DisplayTrendRangeSchema).default(["1h", "6h", "24h"]),
    trends: z.array(DisplayTrendConfigSchema).default([]),
    core_api_base_url: NullableString,
  })
  .passthrough();

const ExternalDevicePointSchema = z
  .object({
    name: z.string(),
    metric: z.string(),
    sensor_type: z.string(),
    unit: z.string(),
    protocol: z.string(),
    register: OptionalNullableNumber.default(null),
    data_type: NullableString,
    scale: OptionalNullableNumber.default(null),
    oid: NullableString,
    path: NullableString,
    json_pointer: NullableString,
    bacnet_object: NullableString,
  })
  .passthrough();

const ExternalDeviceModelSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    since_year: OptionalNullableNumber.default(null),
    protocols: z.array(z.string()),
    points: z.array(ExternalDevicePointSchema),
  })
  .passthrough();

const ExternalDeviceVendorSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    models: z.array(ExternalDeviceModelSchema),
  })
  .passthrough();

export const ExternalDeviceCatalogSchema = z
  .object({
    version: z.number(),
    vendors: z.array(ExternalDeviceVendorSchema),
  })
  .passthrough();

export const ExternalDeviceSummarySchema = z
  .object({
    node_id: z.string(),
    name: z.string(),
    external_provider: NullableString,
    external_id: NullableString,
    config: RecordSchema.optional().default({}),
  })
  .passthrough();

export const ExternalDeviceSummariesSchema = z.array(ExternalDeviceSummarySchema);

export const UpdateNodeDisplayProfileResponseSchema = z
  .object({
    status: z.string(),
    node_id: z.string(),
    node_agent_url: NullableString,
    display: NodeDisplayProfileSchema,
    warning: NullableString,
  })
  .passthrough();

export type NodeDisplayProfile = z.infer<typeof NodeDisplayProfileSchema>;
export type UpdateNodeDisplayProfileResponse = z.infer<
  typeof UpdateNodeDisplayProfileResponseSchema
>;

export const NodeAds1263SettingsDraftSchema = z
  .object({
    enabled: z.boolean().optional().default(false),
    spi_bus: OptionalNullableNumber.default(null),
    spi_device: OptionalNullableNumber.default(null),
    spi_mode: OptionalNullableNumber.default(null),
    spi_speed_hz: OptionalNullableNumber.default(null),
    rst_bcm: OptionalNullableNumber.default(null),
    cs_bcm: OptionalNullableNumber.default(null),
    drdy_bcm: OptionalNullableNumber.default(null),
    vref_volts: OptionalNullableNumber.default(null),
    gain: OptionalNullableNumber.default(null),
    data_rate: NullableString,
    scan_interval_seconds: OptionalNullableNumber.default(null),
  })
  .passthrough();

export const NodeSensorDraftSchema = z
  .object({
    preset: z.string().optional().default("custom"),
    sensor_id: z.string().optional().default(""),
    name: z.string().optional().default(""),
    type: z.string().optional().default("analog"),
    channel: z.number().optional().default(0),
    unit: z.string().optional().default(""),
    location: NullableString,
    interval_seconds: z.number().optional().default(30),
    rolling_average_seconds: z.number().optional().default(0),
    input_min: OptionalNullableNumber.default(null),
    input_max: OptionalNullableNumber.default(null),
    output_min: OptionalNullableNumber.default(null),
    output_max: OptionalNullableNumber.default(null),
    offset: z.number().optional().default(0),
    scale: z.number().optional().default(1),
    pulses_per_unit: OptionalNullableNumber.default(null),
    current_loop_shunt_ohms: OptionalNullableNumber.default(null),
    current_loop_range_m: OptionalNullableNumber.default(null),
  })
  .passthrough();

export const NodeAnalogHealthSchema = z
  .object({
    ok: z.boolean().default(false),
    chip_id: NullableString,
    last_error: NullableString,
    last_ok_at: NullableString,
  })
  .passthrough();

export const NodeSensorsConfigResponseSchema = z
  .object({
    node_id: z.string(),
    sensors: z.array(NodeSensorDraftSchema).default([]),
    ads1263: NodeAds1263SettingsDraftSchema.optional().nullable().default(null),
    analog_backend: NullableString.optional().nullable().default(null),
    analog_health: NodeAnalogHealthSchema.optional().nullable().default(null),
  })
  .passthrough();

export const ApplyNodeSensorsConfigResponseSchema = z
  .object({
    status: z.string(),
    node_id: z.string(),
    node_agent_url: NullableString,
    sensors: z.array(NodeSensorDraftSchema).default([]),
    deleted_sensor_ids: z.array(z.string()).optional().default([]),
    warning: NullableString,
  })
  .passthrough();

export type NodeSensorDraft = z.infer<typeof NodeSensorDraftSchema>;
export type NodeAds1263SettingsDraft = z.infer<typeof NodeAds1263SettingsDraftSchema>;
export type NodeAnalogHealth = z.infer<typeof NodeAnalogHealthSchema>;
export type NodeSensorsConfigResponse = z.infer<typeof NodeSensorsConfigResponseSchema>;
export type ApplyNodeSensorsConfigResponse = z.infer<
  typeof ApplyNodeSensorsConfigResponseSchema
>;

export const SensorSchema = z
  .object({
    sensor_id: z.string(),
    node_id: z.string(),
    name: z.string(),
    type: z.string(),
    unit: z.string(),
    interval_seconds: z.number(),
    rolling_avg_seconds: z.number(),
    latest_value: OptionalNullableNumber,
    latest_ts: NullableString.optional(),
    status: NullableString,
    location: NullableString,
    created_at: NullableString,
    config: RecordSchema.optional(),
  })
  .passthrough();

export const SensorsResponseSchema = z.array(SensorSchema);

export const OutputSchema = z
  .object({
    id: z.string(),
    node_id: z.string(),
    name: z.string(),
    type: z.string(),
    state: z.string(),
    last_command: NullableString,
    supported_states: z.array(z.string()).optional(),
    command_topic: NullableString,
    schedule_ids: z.array(z.string()).optional(),
    config: RecordSchema.optional(),
  })
  .passthrough();

export const OutputsResponseSchema = z.array(OutputSchema);

export const ScheduleBlockSchema = z.object({
  day: z.string(),
  start: z.string(),
  end: z.string(),
});

export const ScheduleSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    rrule: z.string(),
    blocks: z.array(ScheduleBlockSchema).default([]),
    conditions: z.array(RecordSchema).default([]),
    actions: z.array(RecordSchema).default([]),
    next_run: NullableString,
  })
  .passthrough();

export const SchedulesResponseSchema = z.array(ScheduleSchema);

export const ScheduleCalendarEventSchema = z
  .object({
    schedule_id: OptionalString,
    scheduleId: OptionalString,
    title: OptionalString,
    name: OptionalString,
    start: z.union([z.string(), z.number()]),
    end: z.union([z.string(), z.number()]),
  })
  .passthrough();

export const ScheduleCalendarResponseSchema = z.array(ScheduleCalendarEventSchema);
export type ScheduleCalendarEventRaw = z.infer<typeof ScheduleCalendarEventSchema>;

export const AlarmSchema = z
  .object({
    id: z.union([z.string(), z.number()]),
    name: z.string(),
    rule: RecordSchema.nullable().optional(),
    sensor_id: NullableString,
    node_id: NullableString,
    status: NullableString,
    origin: NullableString,
    anomaly_score: OptionalNullableNumber,
    last_fired: NullableString,
    last_raised: NullableString,
    rule_id: z.union([z.string(), z.number()]).nullable().optional(),
    target_key: NullableString,
    resolved_at: NullableString,
    message: NullableString,
  })
  .passthrough();

export const AlarmsResponseSchema = z.array(AlarmSchema);

export const AlarmEventSchema = z
  .object({
    alarm_id: z.string(),
    rule_id: NullableString,
    id: OptionalString,
    sensor_id: NullableString.optional(),
    node_id: NullableString.optional(),
    created_at: NullableString,
    status: OptionalString,
    message: OptionalString,
    origin: NullableString,
    anomaly_score: OptionalNullableNumber,
    transition: NullableString,
  })
  .passthrough();

export const AlarmEventsResponseSchema = z.array(AlarmEventSchema);

export const AlarmRuleSchema = z
  .object({
    id: z.number(),
    name: z.string(),
    description: z.string().default(""),
    enabled: z.boolean(),
    severity: z.enum(["info", "warning", "critical"]).default("warning"),
    origin: z.string().default("threshold"),
    target_selector: RecordSchema,
    condition_ast: RecordSchema,
    timing: RecordSchema.default({}),
    message_template: z.string().default(""),
    created_by: NullableString,
    created_at: z.string(),
    updated_at: z.string(),
    deleted_at: NullableString,
    active_count: z.number().default(0),
    last_eval_at: NullableString,
    last_error: NullableString,
  })
  .passthrough();

export const AlarmRulesResponseSchema = z.array(AlarmRuleSchema);

export const AlarmRulePreviewResultSchema = z
  .object({
    target_key: z.string(),
    sensor_ids: z.array(z.string()).default([]),
    passed: z.boolean(),
    observed_value: OptionalNullableNumber,
  })
  .passthrough();

export const AlarmRulePreviewResponseSchema = z
  .object({
    targets_evaluated: z.number(),
    results: z.array(AlarmRulePreviewResultSchema).default([]),
  })
  .passthrough();

export const IncidentSchema = z
  .object({
    id: z.string(),
    rule_id: NullableString,
    target_key: NullableString,
    severity: z.string(),
    status: z.string(),
    title: z.string(),
    assigned_to: NullableString,
    snoozed_until: NullableString,
    first_event_at: z.string(),
    last_event_at: z.string(),
    closed_at: NullableString,
    created_at: z.string(),
    updated_at: z.string(),
    total_event_count: z.number(),
    active_event_count: z.number(),
    note_count: z.number(),
    last_message: NullableString.optional(),
    last_origin: NullableString.optional(),
    last_sensor_id: NullableString.optional(),
    last_node_id: NullableString.optional(),
  })
  .passthrough();

export const IncidentsListResponseSchema = z
  .object({
    incidents: z.array(IncidentSchema).default([]),
    next_cursor: NullableString,
  })
  .passthrough();

export const IncidentDetailResponseSchema = z
  .object({
    incident: IncidentSchema,
    events: z.array(AlarmEventSchema).default([]),
  })
  .passthrough();

export const IncidentNoteSchema = z
  .object({
    id: z.string(),
    incident_id: z.string(),
    created_by: NullableString,
    body: z.string(),
    created_at: z.string(),
  })
  .passthrough();

export const IncidentNotesListResponseSchema = z
  .object({
    notes: z.array(IncidentNoteSchema).default([]),
    next_cursor: NullableString,
  })
  .passthrough();

export const ActionLogSchema = z
  .object({
    id: z.string(),
    schedule_id: z.string(),
    action: z.unknown(),
    status: z.string(),
    message: NullableString,
    created_at: z.string(),
    output_id: NullableString,
    node_id: NullableString,
  })
  .passthrough();

export const ActionLogsResponseSchema = z.array(ActionLogSchema);

export const AlarmRuleStatsBandSetSchema = z
  .object({
    lower_1: OptionalNullableNumber,
    upper_1: OptionalNullableNumber,
    lower_2: OptionalNullableNumber,
    upper_2: OptionalNullableNumber,
    lower_3: OptionalNullableNumber,
    upper_3: OptionalNullableNumber,
  })
  .passthrough();

export const AlarmRuleStatsBandsSchema = z
  .object({
    classic: AlarmRuleStatsBandSetSchema,
    robust: AlarmRuleStatsBandSetSchema,
  })
  .passthrough();

export const AlarmRuleStatsSensorSchema = z
  .object({
    sensor_id: z.string(),
    unit: z.string(),
    interval_seconds: z.number(),
    n: z.number(),
    min: OptionalNullableNumber,
    max: OptionalNullableNumber,
    mean: OptionalNullableNumber,
    median: OptionalNullableNumber,
    stddev: OptionalNullableNumber,
    p01: OptionalNullableNumber,
    p05: OptionalNullableNumber,
    p25: OptionalNullableNumber,
    p75: OptionalNullableNumber,
    p95: OptionalNullableNumber,
    p99: OptionalNullableNumber,
    mad: OptionalNullableNumber,
    iqr: OptionalNullableNumber,
    coverage_pct: OptionalNullableNumber,
    missing_pct: OptionalNullableNumber,
    bands: AlarmRuleStatsBandsSchema,
  })
  .passthrough();

export const AlarmRuleStatsResponseSchema = z
  .object({
    start: z.string(),
    end: z.string(),
    interval_seconds: z.number(),
    bucket_aggregation_mode: z.string(),
    sensors: z.array(AlarmRuleStatsSensorSchema).default([]),
  })
  .passthrough();

export const UserSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    email: z.string(),
    role: z.string(),
    capabilities: z.array(z.string()),
    last_login: NullableString,
  })
  .passthrough();

export const UsersResponseSchema = z.array(UserSchema);

export const AdoptionCandidateSchema = z
  .object({
    service_name: z.string(),
    hostname: OptionalString,
    ip: OptionalString,
    port: OptionalNumber,
    mac_eth: NullableString,
    mac_wifi: NullableString,
    adoption_token: OptionalString,
    properties: z.record(z.string(), z.string()),
  })
  .passthrough();

export const AdoptionCandidatesResponseSchema = z.array(AdoptionCandidateSchema);

export const ConnectionSchema = z
  .object({
    mode: z.enum(["local", "cloud"]),
    local_address: z.string(),
    cloud_address: z.string(),
    status: z.string(),
    last_switch: NullableString,
    timezone: NullableString,
  })
  .passthrough();

export const BackupFileSchema = z
  .object({
    created_at: OptionalString,
    date: OptionalString,
    size_bytes: OptionalNumber,
    path: OptionalString,
  })
  .passthrough();

export const BackupSummarySchema = z
  .object({
    node_id: z.string(),
    backups: z.array(BackupFileSchema).default([]),
  })
  .passthrough();

export const BackupsResponseSchema = z.array(BackupSummarySchema);

export const BackupRunResponseSchema = z
  .object({
    status: z.string(),
    reason: OptionalString,
  })
  .passthrough();

export const BackupEntrySchema = z
  .object({
    id: OptionalString,
    node_id: z.string(),
    node_name: OptionalString,
    captured_at: NullableString,
    size_bytes: OptionalNumber,
    path: z.string(),
  })
  .passthrough();

export const DashboardBackupsSchema = z.array(
  z.union([BackupSummarySchema, BackupEntrySchema]),
);

export const BackupRetentionPolicySchema = z
  .object({
    node_id: z.string(),
    node_name: OptionalString,
    name: OptionalString,
    keep_days: OptionalNullableNumber,
  })
  .passthrough();

export const BackupRetentionConfigSchema = z
  .object({
    default_keep_days: z.number(),
    policies: z.array(BackupRetentionPolicySchema).default([]),
    last_cleanup_at: NullableString,
    last_cleanup: NullableString,
    last_cleanup_time: NullableString,
  })
  .passthrough();

export const TrendPointSchema = z
  .object({
    timestamp: z.string(),
    value: z.number(),
    samples: OptionalNullableNumber,
    sensor_name: NullableString,
  })
  .passthrough();

export const TrendSeriesSchema = z
  .object({
    sensor_id: z.string(),
    label: NullableString,
    sensor_name: NullableString,
    points: z.array(TrendPointSchema),
  })
  .passthrough();

export const PredictiveTraceEntrySchema = z
  .object({
    timestamp: z.string(),
    model: NullableString,
    code: z.string(),
    output: z.string(),
  })
  .passthrough();

export const PredictiveTraceResponseSchema = z.array(PredictiveTraceEntrySchema);

export const PredictiveStatusSchema = z
  .object({
    enabled: z.boolean(),
    running: z.boolean(),
    token_present: z.boolean(),
    api_base_url: z.string(),
    model: NullableString,
    fallback_models: z.array(z.string()).default([]),
    bootstrap_on_start: z.boolean().default(false),
    bootstrap_max_sensors: z.number().default(25),
    bootstrap_lookback_hours: z.number().default(24),
  })
  .passthrough();

export const RecentRestoreSchema = z
  .object({
    backup_node_id: z.string(),
    date: z.string(),
    recorded_at: z.string(),
    status: z.string(),
  })
  .passthrough();

export const RecentRestoresResponseSchema = z.array(RecentRestoreSchema);

export const AnalyticsFeedEntrySchema = z
  .object({
    status: OptionalString,
    details: OptionalString,
    last_seen: NullableString,
  })
  .passthrough();

export const AnalyticsFeedHistorySchema = z
  .object({
    category: z.string(),
    name: z.string(),
    status: z.string(),
    recorded_at: z.string(),
    meta: RecordSchema.optional(),
  })
  .passthrough();

export const AnalyticsFeedStatusSchema = z
  .object({
    enabled: z.boolean(),
    feeds: z.record(z.string(), AnalyticsFeedEntrySchema),
    history: z.array(AnalyticsFeedHistorySchema),
  })
  .passthrough();

export const SetupCredentialSchema = z
  .object({
    name: z.string(),
    has_value: z.boolean().optional(),
    metadata: RecordSchema.optional(),
    created_at: NullableString,
    updated_at: NullableString,
  })
  .passthrough();

export const SetupCredentialsResponseSchema = z
  .object({
    credentials: z.array(SetupCredentialSchema),
  })
  .passthrough();

export const EmporiaDeviceSchema = z
  .object({
    device_gid: z.string(),
    name: NullableString.optional(),
    model: NullableString.optional(),
    firmware: NullableString.optional(),
    address: NullableString.optional(),
  })
  .passthrough();

export const EmporiaLoginResponseSchema = z
  .object({
    token_present: z.boolean(),
    site_ids: z.array(z.string()),
    devices: z.array(EmporiaDeviceSchema),
  })
  .passthrough();

export const EmporiaDeviceSettingsSchema = EmporiaDeviceSchema.extend({
  enabled: z.boolean(),
  hidden: z.boolean().optional(),
  include_in_power_summary: z.boolean(),
  group_label: NullableString.optional(),
  circuits: z
    .array(
      z
        .object({
          circuit_key: z.string(),
          name: z.string(),
          raw_channel_num: NullableString.optional(),
          nested_device_gid: NullableString.optional(),
          enabled: z.boolean(),
          hidden: z.boolean(),
          include_in_power_summary: z.boolean(),
          is_mains: z.boolean(),
        })
        .passthrough(),
    )
    .optional(),
}).passthrough();

export const EmporiaDevicesResponseSchema = z
  .object({
    token_present: z.boolean(),
    site_ids: z.array(z.string()),
    devices: z.array(EmporiaDeviceSettingsSchema),
  })
  .passthrough();

export const ForecastProviderStatusSchema = z
  .object({
    status: z.string(),
    last_seen: NullableString.optional(),
    details: NullableString.optional(),
    meta: RecordSchema.optional(),
  })
  .passthrough();

export const ForecastStatusSchema = z
  .object({
    enabled: z.boolean(),
    providers: z.record(z.string(), ForecastProviderStatusSchema).default({}),
  })
  .passthrough();

export const WeatherForecastConfigSchema = z
  .object({
    enabled: z.boolean(),
    provider: NullableString.optional(),
    latitude: z.number().nullable().optional(),
    longitude: z.number().nullable().optional(),
    updated_at: NullableString.optional(),
  })
  .passthrough();

export const ForecastSeriesPointSchema = z
  .object({
    timestamp: z.string(),
    value: z.number(),
  })
  .passthrough();

export const ForecastSeriesMetricSchema = z
  .object({
    unit: z.string(),
    points: z.array(ForecastSeriesPointSchema),
  })
  .passthrough();

export const ForecastSeriesResponseSchema = z
  .object({
    provider: z.string(),
    kind: z.string(),
    subject_kind: z.string(),
    subject: z.string(),
    issued_at: z.string(),
    metrics: z.record(z.string(), ForecastSeriesMetricSchema).default({}),
  })
  .passthrough();

export const CurrentWeatherMetricSchema = z
  .object({
    unit: z.string(),
    value: z.number(),
  })
  .passthrough();

export const CurrentWeatherResponseSchema = z
  .object({
    provider: z.string(),
    latitude: z.number(),
    longitude: z.number(),
    observed_at: z.string(),
    fetched_at: z.string(),
    metrics: z.record(z.string(), CurrentWeatherMetricSchema).default({}),
  })
  .passthrough();

export const PvForecastConfigSchema = z
  .object({
    enabled: z.boolean(),
    provider: z.string(),
    latitude: z.number(),
    longitude: z.number(),
    tilt_deg: z.number(),
    azimuth_deg: z.number(),
    kwp: z.number(),
    time_format: z.string(),
    updated_at: z.string(),
  })
  .passthrough();

export const BatteryChemistrySchema = z.enum(["lifepo4", "lead_acid"]);

export const CurrentSignModeSchema = z.enum([
  "auto",
  "positive_is_charging",
  "positive_is_discharging",
]);

export const SocAnchorModeSchema = z.enum(["disabled", "blend_to_renogy_when_resting"]);

export const CapacityEstimationConfigSchema = z
  .object({
    enabled: z.boolean(),
    min_soc_span_percent: z.number(),
    ema_alpha: z.number(),
    clamp_min_ah: z.number(),
    clamp_max_ah: z.number(),
  })
  .passthrough();

export const BatteryModelConfigSchema = z
  .object({
    enabled: z.boolean(),
    chemistry: BatteryChemistrySchema,
    current_sign: CurrentSignModeSchema,
    sticker_capacity_ah: z.number().nullable().optional().default(null),
    soc_cutoff_percent: z.number(),
    rest_current_abs_a: z.number(),
    rest_minutes_required: z.number(),
    soc_anchor_mode: SocAnchorModeSchema,
    soc_anchor_max_step_percent: z.number(),
    capacity_estimation: CapacityEstimationConfigSchema,
  })
  .passthrough();

export const BatteryConfigResponseSchema = z
  .object({
    node_id: z.string(),
    battery_model: BatteryModelConfigSchema,
    resolved_sticker_capacity_ah: z.number().nullable().optional(),
    resolved_sticker_capacity_source: NullableString.optional(),
  })
  .passthrough();

export const PowerRunwayConfigSchema = z
  .object({
    enabled: z.boolean(),
    load_sensor_ids: z.array(z.string()).default([]),
    history_days: z.number(),
    pv_derate: z.number(),
    projection_days: z.number(),
  })
  .passthrough();

export const PowerRunwayConfigResponseSchema = z
  .object({
    node_id: z.string(),
    power_runway: PowerRunwayConfigSchema,
    load_sensors_valid: z.boolean().default(false),
  })
  .passthrough();

export const PvForecastCheckResponseSchema = z
  .object({
    status: z.string(),
    place: z.string().nullish(),
    timezone: z.string().nullish(),
    checked_at: z.string(),
  })
  .passthrough();

export const ForecastPollResponseSchema = z
  .object({
    status: z.string(),
    providers: z.record(z.string(), z.string()).optional(),
  })
  .passthrough();

export const DashboardSnapshotSchema = z
  .object({
    timestamp: z.string(),
    nodes: z.array(NodeSchema),
    sensors: z.array(SensorSchema),
    outputs: z.array(OutputSchema),
    users: z.array(UserSchema),
    schedules: z.array(ScheduleSchema),
    alarms: z.array(AlarmSchema),
    alarm_events: z.array(AlarmEventSchema).default([]),
    analytics: RecordSchema.optional(),
    backups: DashboardBackupsSchema,
    backup_retention: BackupRetentionConfigSchema.optional(),
    adoption: z.array(AdoptionCandidateSchema).default([]),
    connection: ConnectionSchema,
    trend_series: z.array(TrendSeriesSchema).default([]),
  })
  .passthrough();

export type DashboardSnapshotRaw = z.infer<typeof DashboardSnapshotSchema>;

export const Ws2902ProtocolSchema = z.enum(["wunderground", "ambient"]);

export const Ws2902CreatedSensorSchema = z
  .object({
    sensor_id: z.string(),
    name: z.string(),
    type: z.string(),
    unit: z.string(),
    interval_seconds: z.number(),
  })
  .passthrough();

export const Ws2902CreateResponseSchema = z
  .object({
    id: z.string(),
    node_id: z.string(),
    nickname: z.string(),
    protocol: Ws2902ProtocolSchema,
    enabled: z.boolean(),
    ingest_path: z.string(),
    token: z.string(),
    created_at: z.string(),
    sensors: z.array(Ws2902CreatedSensorSchema),
  })
  .passthrough();

export const Ws2902StatusResponseSchema = z
  .object({
    id: z.string(),
    node_id: z.string(),
    nickname: z.string(),
    protocol: Ws2902ProtocolSchema,
    enabled: z.boolean(),
    ingest_path_template: z.string(),
    created_at: z.string(),
    rotated_at: NullableString,
    last_seen: NullableString,
    last_missing_fields: z.array(z.string()).default([]),
    last_payload: RecordSchema.nullable().optional(),
  })
  .passthrough();

export const Ws2902RotateTokenResponseSchema = z
  .object({
    id: z.string(),
    ingest_path: z.string(),
    token: z.string(),
    rotated_at: z.string(),
  })
  .passthrough();

export const RenogyBt2ModeSchema = z.enum(["ble", "external"]);

export const RenogyPresetSensorSchema = z
  .object({
    sensor_id: z.string(),
    name: z.string(),
    metric: z.string(),
    type: z.string(),
    unit: z.string(),
    interval_seconds: z.number(),
  })
  .passthrough();

export const ApplyRenogyBt2PresetResponseSchema = z
  .object({
    status: z.enum(["applied", "already_configured", "stored"]),
    node_id: z.string(),
    node_agent_url: z.string().nullable().optional(),
    bt2_address: z.string(),
    mode: RenogyBt2ModeSchema,
    poll_interval_seconds: z.number(),
    warning: z.string().nullable().optional(),
    sensors: z.array(RenogyPresetSensorSchema).default([]),
    what_to_check: z.array(z.string()).default([]),
  })
  .passthrough();

export const RenogyRegisterMapSchema = z
  .object({
    schema: RecordSchema,
  })
  .passthrough();

export const RenogyDesiredSettingsResponseSchema = z
  .object({
    node_id: z.string(),
    device_type: z.string(),
    desired: RecordSchema,
    pending: z.boolean(),
    desired_updated_at: z.string(),
    last_applied: RecordSchema.nullable().optional(),
    last_applied_at: z.string().nullable().optional(),
    last_apply_status: z.string().nullable().optional(),
    last_apply_result: RecordSchema.nullable().optional(),
    apply_requested: z.boolean().optional(),
    apply_requested_at: z.string().nullable().optional(),
    maintenance_mode: z.boolean().optional(),
  })
  .passthrough();

export const RenogyDesiredSettingsUpdateRequestSchema = z
  .object({
    desired: RecordSchema,
  })
  .passthrough();

export const RenogyValidateResponseSchema = z
  .object({
    ok: z.boolean(),
    errors: z.array(z.string()).default([]),
  })
  .passthrough();

export const RenogyReadCurrentResponseSchema = z
  .object({
    current: RecordSchema,
    provider_status: z.string(),
  })
  .passthrough();

export const RenogyApplyResponseSchema = z
  .object({
    status: z.string(),
    result: RecordSchema,
  })
  .passthrough();

export const RenogyHistoryEntrySchema = z
  .object({
    id: z.number(),
    event_type: z.string(),
    created_at: z.string(),
    desired: RecordSchema.nullable().optional(),
    current: RecordSchema.nullable().optional(),
    diff: RecordSchema.nullable().optional(),
    result: RecordSchema.nullable().optional(),
  })
  .passthrough();

export const RenogyHistoryResponseSchema = z.array(RenogyHistoryEntrySchema);

export type RenogyRegisterMapSchema = z.infer<typeof RenogyRegisterMapSchema>;
export type RenogyDesiredSettingsResponse = z.infer<typeof RenogyDesiredSettingsResponseSchema>;
export type RenogyValidateResponse = z.infer<typeof RenogyValidateResponseSchema>;
export type RenogyReadCurrentResponse = z.infer<typeof RenogyReadCurrentResponseSchema>;
export type RenogyApplyResponse = z.infer<typeof RenogyApplyResponseSchema>;
export type RenogyHistoryEntry = z.infer<typeof RenogyHistoryEntrySchema>;

export const MapSettingsSchema = z
  .object({
    active_save_id: z.number(),
    active_save_name: z.string(),
    active_base_layer_id: z.number().nullable().optional(),
    center_lat: z.number(),
    center_lng: z.number(),
    zoom: z.number(),
    bearing: z.number(),
    pitch: z.number(),
    updated_at: z.string().nullable().optional(),
  })
  .passthrough();

export const MapSaveSchema = z
  .object({
    id: z.number(),
    name: z.string(),
    created_at: z.string(),
    updated_at: z.string(),
  })
  .passthrough();

export const MapSavesResponseSchema = z.array(MapSaveSchema);

export const MapLayerSchema = z
  .object({
    id: z.number(),
    system_key: NullableString,
    name: z.string(),
    kind: z.string(),
    source_type: z.string(),
    config: RecordSchema,
    opacity: z.number(),
    enabled: z.boolean(),
    z_index: z.number(),
    created_at: z.string(),
    updated_at: z.string(),
  })
  .passthrough();

export const MapLayersResponseSchema = z.array(MapLayerSchema);

export const MapFeatureSchema = z
  .object({
    id: z.number(),
    node_id: NullableString,
    sensor_id: NullableString,
    geometry: RecordSchema,
    properties: RecordSchema,
    created_at: z.string(),
    updated_at: z.string(),
  })
  .passthrough();

export const MapFeaturesResponseSchema = z.array(MapFeatureSchema);

export const OfflineMapPackSchema = z
  .object({
    id: z.string(),
    name: z.string(),
    bounds: RecordSchema,
    min_zoom: z.number(),
    max_zoom: z.number(),
    status: z.string(),
    progress: RecordSchema,
    error: NullableString,
    updated_at: z.string(),
  })
  .passthrough();

export const OfflineMapPacksResponseSchema = z.array(OfflineMapPackSchema);

export const DevActivityStatusResponseSchema = z
  .object({
    active: z.boolean(),
    message: NullableString,
    updated_at: NullableString,
    expires_at: NullableString,
  })
  .passthrough();
