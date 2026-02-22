import type { Page, Route } from "@playwright/test";

export const PLAYWRIGHT_STUB_TOKEN = "playwright-mobile-stub-token";

export const PLAYWRIGHT_STUB_USER = {
  id: "playwright-user",
  email: "playwright@farmdashboard.local",
  role: "admin",
  source: "playwright",
  capabilities: [
    "config.write",
    "users.manage",
    "schedules.write",
    "outputs.command",
    "alerts.view",
    "alerts.ack",
    "analytics.view",
    "analysis.view",
    "analysis.run",
  ],
} as const;

const ISO_NOW = new Date("2026-01-09T00:00:00.000Z").toISOString();

const SAMPLE_NODE_ID = "playwright-node-1";
const SAMPLE_WEATHER_NODE_ID = "playwright-weather-station-1";
const SAMPLE_SENSOR_ID = "playwright-sensor-1";
const SAMPLE_VOLTAGE_SENSOR_ID = "playwright-sensor-voltage";
const SAMPLE_CURRENT_SENSOR_ID = "playwright-sensor-current";
const SAMPLE_POWER_SENSOR_ID = "playwright-sensor-power";

const WS_TEMP_SENSOR_ID = "playwright-ws-temperature";
const WS_HUMIDITY_SENSOR_ID = "playwright-ws-humidity";
const WS_WIND_SPEED_SENSOR_ID = "playwright-ws-wind-speed";
const WS_WIND_GUST_SENSOR_ID = "playwright-ws-wind-gust";
const WS_WIND_DIR_SENSOR_ID = "playwright-ws-wind-direction";
const WS_RAIN_SENSOR_ID = "playwright-ws-rain";
const WS_RAIN_RATE_SENSOR_ID = "playwright-ws-rain-rate";
const WS_UV_SENSOR_ID = "playwright-ws-uv";
const WS_SOLAR_SENSOR_ID = "playwright-ws-solar-radiation";
const WS_PRESSURE_SENSOR_ID = "playwright-ws-pressure";

type AnalysisJobStub = {
  job: {
    id: string;
    job_type: string;
    status: string;
    job_key?: string | null;
    created_by?: string | null;
    created_at: string;
    updated_at: string;
    started_at?: string | null;
    completed_at?: string | null;
    canceled_at?: string | null;
    progress: {
      phase: string;
      completed: number;
      total?: number | null;
      message?: string | null;
    };
    error?: { code: string; message: string; details?: unknown } | null;
  };
  params: Record<string, unknown>;
  result: Record<string, unknown>;
  pollCount: number;
};

const buildRelatedSensorsResult = (params: Record<string, unknown>) => {
  const focusId = String(params.focus_sensor_id || SAMPLE_VOLTAGE_SENSOR_ID);
  const startIso = String(params.start || ISO_NOW);
  const endIso = String(params.end || ISO_NOW);
  const start = new Date(startIso);
  const end = new Date(endIso);
  const horizonMs = Math.max(1, end.getTime() - start.getTime());
  const windowSec = Math.max(300, Math.min(3600, Math.floor(horizonMs / 6000)));
  const episodeStart1 = new Date(start.getTime() + horizonMs * 0.3);
  const episodeEnd1 = new Date(episodeStart1.getTime() + windowSec * 1000);
  const episodeStart2 = new Date(start.getTime() + horizonMs * 0.55);
  const episodeEnd2 = new Date(episodeStart2.getTime() + windowSec * 1000);

  return {
    job_type: "related_sensors_v1",
    focus_sensor_id: focusId,
    computed_through_ts: end.toISOString(),
    params,
    candidates: [
      {
        sensor_id: SAMPLE_POWER_SENSOR_ID,
        rank: 1,
        score: 0.86,
        ann: {
          widen_stage: 1,
          union_pool_size: 64,
          embedding_hits: [{ vector: "value", rank: 1, score: 0.92 }],
          filters_applied: params.filters ?? {},
        },
        episodes: [
          {
            start_ts: episodeStart1.toISOString(),
            end_ts: episodeEnd1.toISOString(),
            window_sec: windowSec,
            lag_sec: 120,
            lag_iqr_sec: 60,
            score_mean: 0.72,
            score_peak: 0.89,
            coverage: 0.78,
            num_points: 120,
          },
          {
            start_ts: episodeStart2.toISOString(),
            end_ts: episodeEnd2.toISOString(),
            window_sec: windowSec,
            lag_sec: 180,
            lag_iqr_sec: 90,
            score_mean: 0.66,
            score_peak: 0.82,
            coverage: 0.71,
            num_points: 110,
          },
        ],
        why_ranked: {
          episode_count: 2,
          best_window_sec: windowSec,
          best_lag_sec: 120,
          coverage_pct: 78.0,
          score_components: { score: 0.86, similarity: 0.91 },
          penalties: [],
          bonuses: ["high_coverage", "stable_lag"],
        },
      },
      {
        sensor_id: SAMPLE_CURRENT_SENSOR_ID,
        rank: 2,
        score: 0.72,
        ann: {
          widen_stage: 1,
          union_pool_size: 64,
          embedding_hits: [{ vector: "value", rank: 2, score: 0.81 }],
          filters_applied: params.filters ?? {},
        },
        episodes: [
          {
            start_ts: episodeStart2.toISOString(),
            end_ts: episodeEnd2.toISOString(),
            window_sec: windowSec,
            lag_sec: -60,
            lag_iqr_sec: 45,
            score_mean: 0.55,
            score_peak: 0.71,
            coverage: 0.64,
            num_points: 95,
          },
        ],
        why_ranked: {
          episode_count: 1,
          best_window_sec: windowSec,
          best_lag_sec: -60,
          coverage_pct: 64.0,
          score_components: { score: 0.72, similarity: 0.75 },
          penalties: ["lower_coverage"],
          bonuses: [],
        },
      },
    ],
    timings_ms: { total: 1200 },
    versions: { scoring: "v1" },
  };
};

const buildUnifiedRelatedSensorsResult = (params: Record<string, unknown>) => {
  const focusId = String(params.focus_sensor_id || SAMPLE_VOLTAGE_SENSOR_ID);
  const startIso = String(params.start || ISO_NOW);
  const endIso = String(params.end || ISO_NOW);
  const start = new Date(startIso);
  const end = new Date(endIso);
  const horizonMs = Math.max(1, end.getTime() - start.getTime());
  const windowSec = Math.max(300, Math.min(3600, Math.floor(horizonMs / 6000)));
  const episodeStart1 = new Date(start.getTime() + horizonMs * 0.3);
  const episodeEnd1 = new Date(episodeStart1.getTime() + windowSec * 1000);
  const episodeStart2 = new Date(start.getTime() + horizonMs * 0.58);
  const episodeEnd2 = new Date(episodeStart2.getTime() + windowSec * 1000);
  const candidateLimitUsed = Number(params.candidate_limit ?? 80) || 80;
  const maxResultsUsed = Number(params.max_results ?? 20) || 20;
  const maxSensorsUsed = Math.max(2, Math.min(100, candidateLimitUsed + 1));
  const cooccurrenceTotalSensors = Math.max(2, Math.min(64, maxSensorsUsed));

  return {
    job_type: "related_sensors_unified_v2",
    focus_sensor_id: focusId,
    computed_through_ts: end.toISOString(),
    interval_seconds: Number(params.interval_seconds ?? 60) || 60,
    bucket_count: 48,
    params,
    limits_used: {
      candidate_limit_used: candidateLimitUsed,
      max_results_used: maxResultsUsed,
      max_sensors_used: maxSensorsUsed,
    },
    candidates: [
      {
        sensor_id: SAMPLE_POWER_SENSOR_ID,
        rank: 1,
        blended_score: 0.88,
        confidence_tier: "high",
        episodes: [
          {
            start_ts: episodeStart1.toISOString(),
            end_ts: episodeEnd1.toISOString(),
            window_sec: windowSec,
            lag_sec: 120,
            lag_iqr_sec: 60,
            score_mean: 0.73,
            score_peak: 0.89,
            coverage: 0.79,
            num_points: 118,
          },
          {
            start_ts: episodeStart2.toISOString(),
            end_ts: episodeEnd2.toISOString(),
            window_sec: windowSec,
            lag_sec: 60,
            lag_iqr_sec: 60,
            score_mean: 0.67,
            score_peak: 0.82,
            coverage: 0.72,
            num_points: 107,
          },
        ],
        top_bucket_timestamps: [episodeStart1.getTime(), episodeStart2.getTime()],
        why_ranked: {
          episode_count: 2,
          best_window_sec: windowSec,
          best_lag_sec: 120,
          coverage_pct: 79.0,
          score_components: { score: 0.88, event: 0.82, cooccurrence: 0.76 },
          penalties: [],
          bonuses: ["high_coverage", "strong_shared_events"],
        },
        evidence: {
          events_score: 0.82,
          cooccurrence_score: 14.6,
          events_overlap: 16,
          cooccurrence_count: 6,
          best_lag_sec: 120,
          summary: [
            "Event alignment 0.82 across 16 overlap buckets",
            "Co-occurrence score 14.6 across 6 shared anomaly buckets",
          ],
        },
      },
      {
        sensor_id: SAMPLE_CURRENT_SENSOR_ID,
        rank: 2,
        blended_score: 0.64,
        confidence_tier: "medium",
        episodes: [
          {
            start_ts: episodeStart2.toISOString(),
            end_ts: episodeEnd2.toISOString(),
            window_sec: windowSec,
            lag_sec: -60,
            lag_iqr_sec: 45,
            score_mean: 0.57,
            score_peak: 0.75,
            coverage: 0.66,
            num_points: 93,
          },
        ],
        top_bucket_timestamps: [episodeStart2.getTime()],
        why_ranked: {
          episode_count: 1,
          best_window_sec: windowSec,
          best_lag_sec: -60,
          coverage_pct: 66.0,
          score_components: { score: 0.64, event: 0.58, cooccurrence: 0.42 },
          penalties: ["lower_coverage"],
          bonuses: [],
        },
        evidence: {
          events_score: 0.58,
          cooccurrence_score: 7.2,
          events_overlap: 11,
          cooccurrence_count: 3,
          best_lag_sec: -60,
          summary: [
            "Event alignment 0.58 across 11 overlap buckets",
            "Co-occurrence score 7.2 across 3 shared anomaly buckets",
          ],
        },
      },
    ],
    skipped_candidates: [],
    system_wide_buckets: [
      {
        ts: episodeStart1.getTime(),
        group_size: Math.max(10, Math.floor(cooccurrenceTotalSensors * 0.75)),
        severity_sum: 42.7,
      },
      {
        ts: episodeStart2.getTime(),
        group_size: Math.max(10, Math.floor(cooccurrenceTotalSensors * 0.6)),
        severity_sum: 33.4,
      },
    ],
    prefiltered_candidate_sensor_ids: [],
    truncated_candidate_sensor_ids: [],
    truncated_result_sensor_ids: [],
    timings_ms: { job_total_ms: 1320, events_ms: 760, cooccurrence_ms: 410 },
    counts: {
      candidate_pool: 24,
      ranked: 2,
      event_candidates: 2,
      cooccurrence_sensors: 2,
      cooccurrence_total_sensors: cooccurrenceTotalSensors,
    },
    versions: { unified: "v2", event_match: "v1", cooccurrence: "v1" },
  };
};

const buildEventMatchResult = (params: Record<string, unknown>) => {
  const focusId = String(params.focus_sensor_id || SAMPLE_VOLTAGE_SENSOR_ID);
  const startIso = String(params.start || ISO_NOW);
  const endIso = String(params.end || ISO_NOW);
  const start = new Date(startIso);
  const end = new Date(endIso);
  const horizonMs = Math.max(1, end.getTime() - start.getTime());
  const intervalSeconds = Number(params.interval_seconds ?? 60) || 60;
  const bucketCount = Math.max(1, Math.round(horizonMs / Math.max(1, intervalSeconds * 1000)));
  const windowSec = Math.max(180, Math.min(1800, Math.floor(horizonMs / 8000)));
  const episodeStart1 = new Date(start.getTime() + horizonMs * 0.35);
  const episodeEnd1 = new Date(episodeStart1.getTime() + windowSec * 1000);
  const episodeStart2 = new Date(start.getTime() + horizonMs * 0.62);
  const episodeEnd2 = new Date(episodeStart2.getTime() + windowSec * 1000);

  return {
    job_type: "event_match_v1",
    focus_sensor_id: focusId,
    computed_through_ts: end.toISOString(),
    interval_seconds: intervalSeconds,
    bucket_count: bucketCount,
    params,
    candidates: [
      {
        sensor_id: SAMPLE_POWER_SENSOR_ID,
        rank: 1,
        score: 0.78,
        overlap: 14,
        n_focus: 20,
        n_candidate: 18,
        zero_lag: { lag_sec: 0, score: 0.62, overlap: 10, n_candidate: 18 },
        best_lag: { lag_sec: 120, score: 0.78, overlap: 14, n_candidate: 18 },
        episodes: [
          {
            start_ts: episodeStart1.toISOString(),
            end_ts: episodeEnd1.toISOString(),
            window_sec: windowSec,
            lag_sec: 120,
            lag_iqr_sec: 60,
            score_mean: 0.68,
            score_peak: 0.86,
            coverage: 0.72,
            num_points: 90,
          },
          {
            start_ts: episodeStart2.toISOString(),
            end_ts: episodeEnd2.toISOString(),
            window_sec: windowSec,
            lag_sec: 60,
            lag_iqr_sec: 45,
            score_mean: 0.61,
            score_peak: 0.79,
            coverage: 0.64,
            num_points: 80,
          },
        ],
      },
      {
        sensor_id: SAMPLE_CURRENT_SENSOR_ID,
        rank: 2,
        score: 0.61,
        overlap: 11,
        n_focus: 20,
        n_candidate: 16,
        zero_lag: { lag_sec: 0, score: 0.52, overlap: 9, n_candidate: 16 },
        best_lag: { lag_sec: -60, score: 0.61, overlap: 11, n_candidate: 16 },
        episodes: [
          {
            start_ts: episodeStart2.toISOString(),
            end_ts: episodeEnd2.toISOString(),
            window_sec: windowSec,
            lag_sec: -60,
            lag_iqr_sec: 50,
            score_mean: 0.55,
            score_peak: 0.7,
            coverage: 0.58,
            num_points: 76,
          },
        ],
      },
    ],
    truncated_sensor_ids: [WS_UV_SENSOR_ID, WS_SOLAR_SENSOR_ID],
    timings_ms: { total: 980 },
    versions: { event_match: "stub-v1" },
  };
};

const asStringArray = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];
  return value.map((entry) => String(entry)).filter(Boolean);
};

const buildCorrelationMatrixResult = (params: Record<string, unknown>) => {
  const requestedIds = asStringArray(params.sensor_ids).filter(Boolean);
  const baseSensorIds =
    requestedIds.length > 0
      ? requestedIds
      : [SAMPLE_VOLTAGE_SENSOR_ID, SAMPLE_CURRENT_SENSOR_ID, SAMPLE_POWER_SENSOR_ID];
  const sensorPool = baseSensorIds.length >= 2 ? baseSensorIds : [SAMPLE_VOLTAGE_SENSOR_ID, SAMPLE_CURRENT_SENSOR_ID];
  const maxSensors = 2;
  const truncatedSensorIds = sensorPool.length > maxSensors ? sensorPool.slice(maxSensors) : [];
  const sensorIds = sensorPool.slice(0, Math.max(2, Math.min(sensorPool.length, maxSensors)));
  const size = sensorIds.length;
  const requestedIntervalSeconds = Number(params.interval_seconds ?? 60) || 60;
  const intervalSeconds =
    requestedIntervalSeconds > 0 && requestedIntervalSeconds < 120
      ? requestedIntervalSeconds * 2
      : requestedIntervalSeconds;
  const matrix = Array.from({ length: size }, (_, row) =>
    Array.from({ length: size }, (_, col) => {
      if (row === col) {
        return { r: 1, n: 48 };
      }
      const distance = Math.abs(row - col);
      const scale = Math.max(1, size - 1);
      const base = 0.25 + 0.6 * (1 - distance / scale);
      const sign = (row + col) % 2 === 0 ? 1 : -1;
      const r = Number((sign * base).toFixed(2));
      const n = Math.max(6, 48 - distance * 6);
      return { r, n };
    }),
  );

  const bucketCount = Math.max(1, Math.round(48_000 / Math.max(1, intervalSeconds)));

  return {
    job_type: "correlation_matrix_v1",
    params,
    sensor_ids: sensorIds,
    sensors: sensorIds.map((sensorId) => ({ sensor_id: sensorId, name: sensorId, unit: "V" })),
    matrix,
    computed_through_ts: String(params.end ?? ISO_NOW),
    interval_seconds: intervalSeconds,
    bucket_count: bucketCount,
    truncated_sensor_ids: truncatedSensorIds,
    timings_ms: { total: 520 },
    versions: { matrix: "stub-v1" },
  };
};

const buildCooccurrenceResult = (params: Record<string, unknown>) => {
  const selection = asStringArray(params.sensor_ids);
  const candidates = asStringArray(params.candidate_sensor_ids);
  const focusId = String(params.focus_sensor_id || selection[0] || candidates[0] || SAMPLE_VOLTAGE_SENSOR_ID);
  const pool = selection.length ? selection : candidates.length ? candidates : [SAMPLE_VOLTAGE_SENSOR_ID, SAMPLE_CURRENT_SENSOR_ID, SAMPLE_POWER_SENSOR_ID];
  const unique = Array.from(new Set([focusId, ...pool]));
  const sensors = unique.slice(0, 5);

  const start = new Date(String(params.start ?? ISO_NOW));
  const end = new Date(String(params.end ?? ISO_NOW));
  const horizonMs = Math.max(60_000, end.getTime() - start.getTime());
  const intervalSeconds = Number(params.interval_seconds ?? 60) || 60;
  const bucketCount = Math.max(1, Math.round(horizonMs / Math.max(1, intervalSeconds * 1000)));
  const buckets = [0.25, 0.55, 0.78].map((ratio, idx) => {
    const ts = start.getTime() + horizonMs * ratio;
    const sensorList = sensors.slice(0, Math.max(3, Math.min(sensors.length, 3 + idx))).map((sensorId, sIdx) => ({
      sensor_id: sensorId,
      z: 3.2 + sIdx * 0.6 + idx * 0.4,
      direction: sIdx % 2 === 0 ? "up" : "down",
      delta: (sIdx % 2 === 0 ? 1 : -1) * (1.5 + sIdx * 0.4),
    }));
    const groupSize = sensorList.length;
    const severitySum = sensorList.reduce((sum, evt) => sum + Math.abs(evt.z), 0);
    const pairWeight = (groupSize * (groupSize - 1)) / 2;
    const score = pairWeight * severitySum;
    return {
      ts,
      sensors: sensorList,
      group_size: groupSize,
      severity_sum: Number(severitySum.toFixed(2)),
      pair_weight: pairWeight,
      score: Number(score.toFixed(2)),
    };
  });

  return {
    job_type: "cooccurrence_v1",
    params,
    focus_sensor_id: focusId,
    buckets,
    computed_through_ts: String(params.end ?? ISO_NOW),
    truncated_sensor_ids: [WS_UV_SENSOR_ID, WS_SOLAR_SENSOR_ID],
    interval_seconds: intervalSeconds,
    bucket_count: bucketCount,
    timings_ms: { total: 640 },
    versions: { cooccurrence: "stub-v1" },
  };
};

const buildMatrixProfileResult = (params: Record<string, unknown>) => {
  const sensorId = String(params.sensor_id ?? SAMPLE_VOLTAGE_SENSOR_ID);
  const start = new Date(String(params.start ?? ISO_NOW));
  const end = new Date(String(params.end ?? ISO_NOW));
  const intervalSeconds = Number(params.interval_seconds ?? 60) || 60;
  const targetPoints = Math.max(64, Math.min(256, Number(params.max_points ?? 256) || 256));
  const window = Math.max(8, Math.min(32, Number(params.window_points ?? 16) || 16));
  const totalMs = Math.max(1, end.getTime() - start.getTime());
  const stepMs = Math.max(1, Math.floor(totalMs / targetPoints));

  const timestamps = Array.from({ length: targetPoints }, (_, idx) =>
    new Date(start.getTime() + idx * stepMs).toISOString(),
  );
  const values = Array.from({ length: targetPoints }, (_, idx) => {
    const base = 10 + Math.sin(idx / 6) * 2 + Math.sin(idx / 18) * 0.8;
    const spike = idx % 37 === 0 ? 2.4 : 0;
    return Number((base + spike).toFixed(3));
  });

  const k = Math.max(0, targetPoints - window + 1);
  const profile = Array.from({ length: k }, (_, idx) => Number((0.4 + idx * 0.05).toFixed(3)));
  const profileIndex =
    k > 0 ? Array.from({ length: k }, (_, idx) => (idx + Math.floor(window / 2) + 7) % k) : [];
  const windowStartTs = timestamps.slice(0, k);
  const exclusionZoneRaw = Number(params.exclusion_zone ?? Math.floor(window / 2));
  const exclusionZone = Number.isFinite(exclusionZoneRaw)
    ? Math.max(0, Math.min(Math.floor(exclusionZoneRaw), window))
    : Math.floor(window / 2);
  const topKRaw = Number(params.top_k ?? 5);
  const topK = Number.isFinite(topKRaw) ? Math.max(1, Math.min(20, Math.floor(topKRaw))) : 5;

  const windowSummary = (idx: number) => {
    const startTs = windowStartTs[idx] ?? timestamps[0] ?? ISO_NOW;
    const endIdx = Math.min(idx + window - 1, timestamps.length - 1);
    const endTs = timestamps[endIdx] ?? startTs;
    const matchIndex = profileIndex[idx] ?? -1;
    const matchStartTs = matchIndex >= 0 ? windowStartTs[matchIndex] ?? timestamps[matchIndex] : null;
    const matchEndIdx = matchIndex >= 0 ? Math.min(matchIndex + window - 1, timestamps.length - 1) : -1;
    const matchEndTs =
      matchIndex >= 0 ? (timestamps[matchEndIdx] ?? matchStartTs ?? startTs) : null;
    return {
      window_index: idx,
      start_ts: startTs,
      end_ts: endTs,
      distance: profile[idx] ?? 0,
      match_index: matchIndex >= 0 ? matchIndex : null,
      match_start_ts: matchStartTs,
      match_end_ts: matchEndTs,
    };
  };

  const uniqueIndices = (indices: number[]) =>
    Array.from(new Set(indices.filter((idx) => idx >= 0 && idx < k)));
  const motifIndices = uniqueIndices(
    Array.from({ length: topK }, (_, i) => Math.min(2 + i * 2, Math.max(0, k - 1))),
  );
  const anomalyIndices = uniqueIndices(
    Array.from({ length: topK }, (_, i) => Math.max(0, k - 3 - i * 2)),
  );
  const motifs = k > 0 ? motifIndices.map(windowSummary) : [];
  const anomalies = k > 0 ? anomalyIndices.map(windowSummary) : [];

  return {
    job_type: "matrix_profile_v1",
    params,
    sensor_id: sensorId,
    sensor_label: sensorId,
    unit: "V",
    timestamps,
    values,
    window_start_ts: windowStartTs,
    profile,
    profile_index: profileIndex,
    window,
    exclusion_zone: exclusionZone,
    step: 1,
    effective_interval_seconds: intervalSeconds,
    computed_through_ts: String(params.end ?? ISO_NOW),
    warnings: ["Downsampled input to keep the job within a safe compute budget."],
    motifs,
    anomalies,
    source_points: targetPoints * 4,
    sampled_points: targetPoints,
    timings_ms: { total: 740 },
    versions: { matrix_profile: "stub-v1" },
  };
};

const emptyArrayEndpoints = new Set([
  "/api/outputs",
  "/api/alarms",
  "/api/alarms/history",
  "/api/schedules",
  "/api/users",
  "/api/scan",
  "/api/backups",
  "/api/backups/recent-restores",
  "/api/api-tokens",
  "/api/forecast/history",
]);

const defaultJsonByPath: Record<string, unknown> = {
  "/api/auth/me": PLAYWRIGHT_STUB_USER,
  "/api/connection": {
    status: "online",
    mode: "local",
    local_address: "127.0.0.1:8000",
    cloud_address: "unknown",
    last_switch: null,
  },
  "/api/predictive/status": {
    enabled: false,
    running: false,
    token_present: false,
    api_base_url: "",
    model: null,
    fallback_models: [],
    bootstrap_on_start: false,
    bootstrap_max_sensors: 25,
    bootstrap_lookback_hours: 24,
  },
  "/api/predictive/trace": [],
  "/api/dev/activity": { active: false, message: null, updated_at: null, expires_at: null },
  "/api/nodes": [
    {
      id: SAMPLE_NODE_ID,
      name: "Playwright Node",
      status: "online",
      uptime_seconds: 1234,
      cpu_percent: 12.3,
      storage_used_bytes: 1024 * 1024 * 1024,
      mac_eth: null,
      mac_wifi: null,
      ip_last: "192.168.1.10",
      last_seen: ISO_NOW,
      created_at: ISO_NOW,
      config: {},
    },
    {
      id: SAMPLE_WEATHER_NODE_ID,
      name: "Playwright Weather Station",
      status: "online",
      uptime_seconds: 555,
      cpu_percent: 2.1,
      storage_used_bytes: 256 * 1024 * 1024,
      mac_eth: null,
      mac_wifi: null,
      ip_last: "192.168.1.55",
      last_seen: ISO_NOW,
      created_at: ISO_NOW,
      config: { kind: "ws-2902" },
    },
  ],
  "/api/sensors": [
    {
      sensor_id: SAMPLE_SENSOR_ID,
      node_id: SAMPLE_NODE_ID,
      name: "Playwright Sensor",
      type: "temperature",
      unit: "degC",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 21.5,
      status: "online",
      location: "Playwright Lab",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_TEMP_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Outdoor temperature",
      type: "temperature",
      unit: "°C",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 19.8,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_HUMIDITY_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Outdoor humidity",
      type: "humidity",
      unit: "%",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 52.1,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_WIND_SPEED_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Wind speed",
      type: "wind_speed",
      unit: "m/s",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 3.2,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_WIND_GUST_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Wind gust",
      type: "wind_gust",
      unit: "m/s",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 5.1,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_WIND_DIR_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Wind direction",
      type: "wind_direction",
      unit: "°",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 245,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_RAIN_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Daily rain",
      type: "rain",
      unit: "mm",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 1.4,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_RAIN_RATE_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Rain rate",
      type: "rain_rate",
      unit: "mm/h",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 0,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_UV_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "UV index",
      type: "uv",
      unit: "",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 0.8,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_SOLAR_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Solar radiation",
      type: "solar_radiation",
      unit: "W/m²",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 220,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: WS_PRESSURE_SENSOR_ID,
      node_id: SAMPLE_WEATHER_NODE_ID,
      name: "Pressure",
      type: "pressure",
      unit: "kPa",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 101.2,
      status: "online",
      location: "Playwright Yard",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: SAMPLE_VOLTAGE_SENSOR_ID,
      node_id: SAMPLE_NODE_ID,
      name: "Playwright Voltage",
      type: "voltage",
      unit: "V",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 13.2,
      status: "online",
      location: "Playwright Lab",
      created_at: ISO_NOW,
      config: { source: "emporia_cloud", metric: "mains_voltage_v" },
    },
    {
      sensor_id: SAMPLE_CURRENT_SENSOR_ID,
      node_id: SAMPLE_NODE_ID,
      name: "Playwright Current",
      type: "current",
      unit: "A",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 5.1,
      status: "online",
      location: "Playwright Lab",
      created_at: ISO_NOW,
      config: {},
    },
    {
      sensor_id: SAMPLE_POWER_SENSOR_ID,
      node_id: SAMPLE_NODE_ID,
      name: "Playwright Power",
      type: "power",
      unit: "W",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 250,
      status: "online",
      location: "Playwright Lab",
      created_at: ISO_NOW,
      config: {},
    },
  ],
  "/api/metrics/query": { series: [] },
  "/api/backups/retention": {
    default_keep_days: 30,
    policies: [],
    last_cleanup_at: null,
    last_cleanup: null,
    last_cleanup_time: null,
  },
  "/api/map/saves": [
    { id: 1, name: "Default", created_at: ISO_NOW, updated_at: ISO_NOW },
  ],
  "/api/map/settings": {
    active_save_id: 1,
    active_save_name: "Default",
    active_base_layer_id: null,
    center_lat: 36.9741,
    center_lng: -122.0308,
    zoom: 16,
    bearing: 0,
    pitch: 0,
    updated_at: ISO_NOW,
  },
  "/api/map/layers": [],
  "/api/map/features": [
    {
      id: 42,
      node_id: null,
      sensor_id: null,
      geometry: {
        type: "Polygon",
        coordinates: [
          [
            [-122.0308, 36.9741],
            [-122.0307, 36.9741],
            [-122.0307, 36.9740],
            [-122.0308, 36.9740],
            [-122.0308, 36.9741],
          ],
        ],
      },
      properties: { name: "Playwright Field", kind: "field", color: "#22c55e" },
      created_at: ISO_NOW,
      updated_at: ISO_NOW,
    },
  ],
  "/api/map/offline/packs": [],
  "/api/setup/credentials": { credentials: [] },
  "/api/setup/emporia/devices": { token_present: false, site_ids: [], devices: [] },
  "/api/setup/integrations/mapillary/token": { configured: false, access_token: null },
  "/api/forecast/weather/config": {
    enabled: false,
    provider: null,
    latitude: null,
    longitude: null,
    updated_at: ISO_NOW,
  },
  "/api/forecast/status": { enabled: false, providers: {} },
  "/api/analytics/feeds/status": { enabled: false, feeds: {}, history: [] },
  "/api/analytics/power": {},
  "/api/analytics/water": {},
  "/api/analytics/soil": {},
  "/api/analytics/status": {},
};

function shouldStub(pathname: string) {
  return pathname.startsWith("/api/");
}

function jsonForPath(pathname: string): unknown | null {
  if (pathname in defaultJsonByPath) return defaultJsonByPath[pathname];
  if (emptyArrayEndpoints.has(pathname)) return [];
  return null;
}

async function fulfillJson(route: Route, body: unknown) {
  return route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify(body),
  });
}

/** Build and fulfill a response in the FDB1 binary metrics wire format. */
async function fulfillBinaryMetrics(
  route: Route,
  seriesData: Array<{
    sensor_id: string;
    sensor_name: string;
    base_timestamp_ms: number;
    points: Array<{ offset_seconds: number; value: number }>;
  }>,
) {
  const encoder = new TextEncoder();
  const parts: number[] = [];

  // Magic "FDB1" as raw bytes (not LE u32)
  parts.push(0x46, 0x44, 0x42, 0x31);

  // series_count (u16 LE)
  const sc = seriesData.length;
  parts.push(sc & 0xff, (sc >> 8) & 0xff);

  // total_point_count (u32 LE)
  const totalPoints = seriesData.reduce((sum, s) => sum + s.points.length, 0);
  parts.push(
    totalPoints & 0xff,
    (totalPoints >> 8) & 0xff,
    (totalPoints >> 16) & 0xff,
    (totalPoints >> 24) & 0xff,
  );

  // Series headers
  for (const s of seriesData) {
    const idBytes = encoder.encode(s.sensor_id);
    parts.push(idBytes.length & 0xff, (idBytes.length >> 8) & 0xff);
    parts.push(...idBytes);

    const nameBytes = encoder.encode(s.sensor_name);
    parts.push(nameBytes.length & 0xff, (nameBytes.length >> 8) & 0xff);
    if (nameBytes.length > 0) parts.push(...nameBytes);

    const pc = s.points.length;
    parts.push(pc & 0xff, (pc >> 8) & 0xff, (pc >> 16) & 0xff, (pc >> 24) & 0xff);

    const f64buf = new ArrayBuffer(8);
    new DataView(f64buf).setFloat64(0, s.base_timestamp_ms, true);
    parts.push(...new Uint8Array(f64buf));
  }

  // Point data (bulk)
  for (const s of seriesData) {
    for (const pt of s.points) {
      const os = pt.offset_seconds;
      parts.push(os & 0xff, (os >> 8) & 0xff, (os >> 16) & 0xff, (os >> 24) & 0xff);
      const f32buf = new ArrayBuffer(4);
      new DataView(f32buf).setFloat32(0, pt.value, true);
      parts.push(...new Uint8Array(f32buf));
    }
  }

  const buffer = Buffer.from(new Uint8Array(parts));
  return route.fulfill({
    status: 200,
    contentType: "application/octet-stream",
    body: buffer,
  });
}

export async function installStubApi(
  page: Page,
  options?: {
    jsonByPath?: Record<string, unknown>;
    jsonByPathPrefix?: Record<string, unknown>;
    emptyArrayEndpoints?: string[];
  },
) {
  const analysisJobsById = new Map<string, AnalysisJobStub>();
  const analysisJobsByKey = new Map<string, string>();
  let analysisJobCounter = 0;

  const progressLabelForJob = (jobType: string) => {
    switch (jobType) {
      case "related_sensors_unified_v2":
        return "Merging relationship evidence";
      case "correlation_matrix_v1":
        return "Computing matrix";
      case "cooccurrence_v1":
        return "Scanning anomalies";
      case "event_match_v1":
        return "Matching spikes";
      case "matrix_profile_v1":
        return "Computing profile";
      default:
        return "Scoring candidates";
    }
  };

  const tickAnalysisJob = (job: AnalysisJobStub) => {
    if (job.job.status !== "running") return;
    job.pollCount += 1;
    const total = job.job.progress.total ?? 3;
    const nextCompleted = Math.min(total, job.pollCount);
    job.job.progress.completed = nextCompleted;
    const label = progressLabelForJob(job.job.job_type);
    job.job.progress.message = `${label} (${nextCompleted}/${total})`;
    job.job.updated_at = new Date().toISOString();
    if (nextCompleted >= total) {
      job.job.status = "completed";
      job.job.progress.phase = "completed";
      job.job.completed_at = new Date().toISOString();
      job.job.progress.message = "Completed";
    }
  };
  await page.addInitScript((token) => {
    try {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    } catch {
      // ignore
    }
  }, PLAYWRIGHT_STUB_TOKEN);

  await page.route("**/*", async (route) => {
    const requestUrl = route.request().url();
    if (!requestUrl.startsWith("http")) return route.continue();
    const url = new URL(requestUrl);
    if (!shouldStub(url.pathname)) return route.continue();

    if (options?.jsonByPath && url.pathname in options.jsonByPath) {
      return fulfillJson(route, options.jsonByPath[url.pathname]);
    }

    if (options?.jsonByPathPrefix) {
      for (const [prefix, body] of Object.entries(options.jsonByPathPrefix)) {
        if (url.pathname.startsWith(prefix)) {
          return fulfillJson(route, body);
        }
      }
    }

    if (url.pathname === "/api/metrics/query") {
      const sensorIds = url.searchParams.getAll("sensor_ids[]");
      const intervalSeconds = Number(url.searchParams.get("interval") ?? "60") || 60;
      const startParam = url.searchParams.get("start");
      const start = startParam ? new Date(startParam) : new Date(ISO_NOW);
      const isBinary = url.searchParams.get("format") === "binary";

      const pointsPerSeries = 48;
      const seriesData = sensorIds.map((sensorId) => {
        const baseMs = start.getTime();
        const points = Array.from({ length: pointsPerSeries }, (_, idx) => {
          const phase = idx / 6;
          let value = 0;
          switch (sensorId) {
            case SAMPLE_SENSOR_ID: // temperature
              value = 18 + Math.sin(phase) * 4;
              break;
            case SAMPLE_VOLTAGE_SENSOR_ID:
              value = 12.7 + Math.sin(phase) * 0.6;
              break;
            case SAMPLE_CURRENT_SENSOR_ID:
              value = 5 + Math.cos(phase) * 3;
              break;
            case SAMPLE_POWER_SENSOR_ID:
              value = 250 + Math.sin(phase) * 180;
              break;
            case WS_TEMP_SENSOR_ID:
              value = 16 + Math.sin(phase) * 6;
              break;
            case WS_HUMIDITY_SENSOR_ID:
              value = 48 + Math.cos(phase) * 14;
              break;
            case WS_WIND_SPEED_SENSOR_ID:
              value = 2.5 + Math.max(0, Math.sin(phase * 1.3)) * 4;
              break;
            case WS_WIND_GUST_SENSOR_ID:
              value = 4 + Math.max(0, Math.sin(phase * 1.3)) * 6;
              break;
            case WS_WIND_DIR_SENSOR_ID:
              value = (220 + Math.sin(phase) * 60 + 360) % 360;
              break;
            case WS_RAIN_RATE_SENSOR_ID:
              value = Math.max(0, Math.sin(phase - 2.5)) * 5;
              break;
            case WS_RAIN_SENSOR_ID:
              value = Math.max(0, Math.sin(phase - 2.5)) * 2;
              break;
            case WS_UV_SENSOR_ID:
              value = Math.max(0, Math.sin(phase - 1.5)) * 8;
              break;
            case WS_SOLAR_SENSOR_ID:
              value = Math.max(0, Math.sin(phase - 1.5)) * 850;
              break;
            case WS_PRESSURE_SENSOR_ID:
              value = 101.3 + Math.sin(phase / 2) * 0.6;
              break;
            default:
              value = Math.sin(phase) * 10;
          }
          return { offset_seconds: idx * intervalSeconds, value };
        });

        return { sensor_id: sensorId, sensor_name: sensorId, base_timestamp_ms: baseMs, points };
      });

      if (isBinary) {
        return fulfillBinaryMetrics(route, seriesData);
      }

      // JSON fallback for non-binary callers
      const series = seriesData.map((s) => ({
        sensor_id: s.sensor_id,
        sensor_name: s.sensor_name,
        label: null,
        points: s.points.map((p) => ({
          timestamp: new Date(s.base_timestamp_ms + p.offset_seconds * 1000).toISOString(),
          value: p.value,
          samples: 1,
        })),
      }));
      return fulfillJson(route, { series });
    }

    if (url.pathname === "/api/analysis/jobs" && route.request().method() === "POST") {
      const body = (route.request().postDataJSON?.() ?? {}) as Record<string, unknown>;
      const jobType = String(body.job_type ?? "");
      const jobKey = body.job_key ? String(body.job_key) : null;
      const dedupe = Boolean(body.dedupe);
      const params = (body.params ?? {}) as Record<string, unknown>;

      if (dedupe && jobKey && analysisJobsByKey.has(jobKey)) {
        const existingId = analysisJobsByKey.get(jobKey) as string;
        const existingJob = analysisJobsById.get(existingId);
        if (existingJob) {
          return fulfillJson(route, { job: existingJob.job });
        }
      }

      analysisJobCounter += 1;
      const id = `analysis-job-${analysisJobCounter}`;
      const now = new Date().toISOString();
      const result =
        jobType === "related_sensors_unified_v2"
          ? buildUnifiedRelatedSensorsResult(params)
          : jobType === "correlation_matrix_v1"
          ? buildCorrelationMatrixResult(params)
          : jobType === "cooccurrence_v1"
            ? buildCooccurrenceResult(params)
            : jobType === "event_match_v1"
              ? buildEventMatchResult(params)
            : jobType === "matrix_profile_v1"
              ? buildMatrixProfileResult(params)
              : buildRelatedSensorsResult(params);
      const job: AnalysisJobStub = {
        job: {
          id,
          job_type: jobType || "related_sensors_unified_v2",
          status: "running",
          job_key: jobKey,
          created_by: PLAYWRIGHT_STUB_USER.id,
          created_at: now,
          updated_at: now,
          started_at: now,
          completed_at: null,
          canceled_at: null,
          progress: {
            phase: "running",
            completed: 0,
            total: 5,
            message: progressLabelForJob(jobType || "related_sensors_unified_v2"),
          },
          error: null,
        },
        params,
        result,
        pollCount: 0,
      };
      analysisJobsById.set(id, job);
      if (jobKey) analysisJobsByKey.set(jobKey, id);
      return fulfillJson(route, { job: job.job });
    }

    if (url.pathname.startsWith("/api/analysis/jobs/")) {
      const parts = url.pathname.split("/").filter(Boolean);
      const id = parts[3];
      const action = parts[4];
      const job = id ? analysisJobsById.get(id) : undefined;

      if (!job) {
        return route.fulfill({
          status: 404,
          contentType: "application/json",
          body: JSON.stringify({ error: "Not found" }),
        });
      }

      if (!action && route.request().method() === "GET") {
        tickAnalysisJob(job);
        return fulfillJson(route, { job: job.job });
      }

      if (action === "result" && route.request().method() === "GET") {
        return fulfillJson(route, { job_id: id, result: job.result });
      }

      if (action === "cancel" && route.request().method() === "POST") {
        if (job.job.status === "running" || job.job.status === "pending") {
          job.job.status = "canceled";
          job.job.canceled_at = new Date().toISOString();
          job.job.updated_at = job.job.canceled_at;
          job.job.progress.message = "Canceled";
        }
        return fulfillJson(route, { job: job.job });
      }

      if (action === "events" && route.request().method() === "GET") {
        return fulfillJson(route, { events: [], next_after: null });
      }
    }

    if (url.pathname === "/api/analysis/preview" && route.request().method() === "POST") {
      const body = (route.request().postDataJSON?.() ?? {}) as Record<string, unknown>;
      const focusId = String(body.focus_sensor_id ?? SAMPLE_VOLTAGE_SENSOR_ID);
      const candidateId = String(body.candidate_sensor_id ?? SAMPLE_POWER_SENSOR_ID);
      const startIso = String(body.episode_start_ts ?? ISO_NOW);
      const endIso = String(body.episode_end_ts ?? ISO_NOW);
      const lagSeconds = Number(body.lag_seconds ?? 0) || 0;

      const start = new Date(startIso);
      const end = new Date(endIso);
      const windowMs = Math.max(1, end.getTime() - start.getTime());
      const pointsPerSeries = 48;
      const intervalMs = Math.max(1000, Math.floor(windowMs / pointsPerSeries));

      const buildSeries = (sensorId: string) => {
        const points = Array.from({ length: pointsPerSeries }, (_, idx) => {
          const t = new Date(start.getTime() + idx * intervalMs);
          const phase = idx / 6;
          let value = 0;
          switch (sensorId) {
            case SAMPLE_VOLTAGE_SENSOR_ID:
              value = 12.7 + Math.sin(phase) * 0.6;
              break;
            case SAMPLE_CURRENT_SENSOR_ID:
              value = 5 + Math.cos(phase) * 3;
              break;
            case SAMPLE_POWER_SENSOR_ID:
              value = 250 + Math.sin(phase) * 180;
              break;
            default:
              value = Math.sin(phase) * 10;
          }
          return { timestamp: t.toISOString(), value, samples: 1 };
        });

        return { sensor_id: sensorId, sensor_name: sensorId, unit: null, points };
      };

      const focusSeries = buildSeries(focusId);
      const candidateSeries = buildSeries(candidateId);
      const candidateAligned = lagSeconds
        ? {
            ...candidateSeries,
            points: candidateSeries.points.map((point) => ({
              ...point,
              timestamp: new Date(new Date(point.timestamp).getTime() - lagSeconds * 1000).toISOString(),
            })),
          }
        : null;

      const focusEvents = [10, 16, 24, 31].map((idx) => start.getTime() + idx * intervalMs);
      const candidateEvents = focusEvents.map((ts) => ts + lagSeconds * 1000);
      const matchedFocus = focusEvents.slice(0, 3);
      const matchedCandidate = candidateEvents.slice(0, 3);

      return fulfillJson(route, {
        focus: focusSeries,
        candidate: candidateSeries,
        candidate_aligned: candidateAligned,
        selected_episode: null,
        bucket_seconds: Math.max(1, Math.floor(intervalMs / 1000)),
        event_overlays: {
          focus_event_ts_ms: focusEvents,
          candidate_event_ts_ms: candidateEvents,
          matched_focus_event_ts_ms: matchedFocus,
          matched_candidate_event_ts_ms: matchedCandidate,
          tolerance_seconds: Math.max(0, Math.floor(intervalMs / 1000)),
        },
      });
    }

    if (options?.emptyArrayEndpoints?.includes(url.pathname)) {
      return fulfillJson(route, []);
    }

    const jsonBody = jsonForPath(url.pathname);
    if (jsonBody !== null) {
      return fulfillJson(route, jsonBody);
    }

    return route.fulfill({
      status: 200,
      contentType: "application/json",
      body: "null",
    });
  });
}
