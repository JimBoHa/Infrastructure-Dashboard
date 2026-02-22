import { z } from "zod";
import { fetchJson } from "@/lib/http";
import { parseApiResponse } from "@/lib/apiSchemas";
import type {
  AnalyticsBundle,
  AnalyticsIntegration,
  AnalyticsPower,
  AnalyticsRateSchedule,
  AnalyticsSoil,
  AnalyticsSoilField,
  AnalyticsStatus,
  AnalyticsWater,
  TimeSeriesPoint,
} from "@/types/dashboard";

type UnknownRecord = Record<string, unknown>;
type Path = readonly string[];

const nowTimestamp = () => new Date();

const toRecord = (value: unknown): UnknownRecord =>
  value && typeof value === "object" ? (value as UnknownRecord) : {};

const coerceString = (value: unknown): string | undefined =>
  typeof value === "string" && value.length ? value : undefined;

const coerceNumber = (value: unknown, fallback = 0): number => {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim().length) {
    const parsed = Number(value);
    if (!Number.isNaN(parsed)) {
      return parsed;
    }
  }
  return fallback;
};

function readValue(source: UnknownRecord, path: Path): unknown {
  let current: unknown = source;
  for (const segment of path) {
    if (!current || typeof current !== "object") {
      return undefined;
    }
    current = (current as UnknownRecord)[segment];
  }
  return current;
}

const readNumber = (
  source: UnknownRecord,
  paths: Path[],
  fallback = 0,
): number => {
  for (const path of paths) {
    const candidate = readValue(source, path);
    const value = coerceNumber(candidate, Number.NaN);
    if (!Number.isNaN(value)) {
      return value;
    }
  }
  return fallback;
};

const readArray = (source: UnknownRecord, paths: Path[]): unknown[] => {
  for (const path of paths) {
    const candidate = readValue(source, path);
    if (Array.isArray(candidate)) {
      return candidate;
    }
  }
  return [];
};

function coerceSeries(
  value: unknown,
  fallback: TimeSeriesPoint[],
): TimeSeriesPoint[] {
  if (!Array.isArray(value) || !value.length) {
    return fallback;
  }
  const series: TimeSeriesPoint[] = [];
  for (const entry of value) {
    const record = toRecord(entry);
    const timestampCandidate =
      record.timestamp ??
      record.time ??
      record.recorded_at ??
      record.ts ??
      nowTimestamp();
    const timestamp =
      timestampCandidate instanceof Date
        ? timestampCandidate
        : new Date(timestampCandidate as string | number);
    const numeric = coerceNumber(record.value, Number.NaN);
    if (!Number.isNaN(numeric)) {
      series.push({ timestamp, value: numeric });
    }
  }
  return series.length ? series : fallback;
}

const readSeries = (
  source: UnknownRecord,
  paths: Path[],
  fallback: TimeSeriesPoint[],
): TimeSeriesPoint[] => {
  for (const path of paths) {
    const candidate = readValue(source, path);
    const series = coerceSeries(candidate, []);
    if (series.length) {
      return series;
    }
  }
  return fallback;
};

const normalizeIntegrations = (
  entries: unknown[],
  fallback: AnalyticsIntegration[],
): AnalyticsIntegration[] => {
  if (!entries.length) {
    return fallback;
  }
  const integrations: AnalyticsIntegration[] = [];
  entries.forEach((entry) => {
    const record = toRecord(entry);
    const name = coerceString(record.name);
    if (!name) return;
    const status = coerceString(record.status) ?? "unknown";
    const lastSeen =
      coerceString(record.last_seen) ??
      coerceString(record.updated_at) ??
      coerceString(record.timestamp);
    const metaValue = record.meta;
    const details =
      coerceString(record.details) ??
      (typeof metaValue === "string"
        ? metaValue
        : metaValue
          ? JSON.stringify(metaValue)
          : undefined);
    integrations.push({
      name,
      status,
      last_seen: lastSeen,
      details,
    });
  });
  return integrations.length ? integrations : fallback;
};

const normalizeRateSchedule = (
  value: unknown,
  fallback: AnalyticsRateSchedule,
): AnalyticsRateSchedule => {
  const record = toRecord(value);
  return {
    provider: coerceString(record.provider) ?? fallback.provider,
    current_rate: coerceNumber(record.current_rate, fallback.current_rate),
    est_monthly_cost: coerceNumber(
      record.est_monthly_cost,
      fallback.est_monthly_cost,
    ),
    currency: coerceString(record.currency) ?? fallback.currency,
    period_label:
      coerceString(record.period_label) ??
      coerceString(record.period) ??
      fallback.period_label,
  };
};

export function emptyPowerAnalytics(): AnalyticsPower {
  return {
    live_kw: 0,
    live_solar_kw: 0,
    live_grid_kw: 0,
    live_battery_kw: 0,
    kwh_24h: 0,
    kwh_168h: 0,
    solar_kwh_24h: 0,
    solar_kwh_168h: 0,
    grid_kwh_24h: 0,
    grid_kwh_168h: 0,
    battery_kwh_24h: 0,
    battery_kwh_168h: 0,
    series_24h: [],
    series_168h: [],
    solar_series_24h: [],
    solar_series_168h: [],
    grid_series_24h: [],
    grid_series_168h: [],
    battery_series_24h: [],
    battery_series_168h: [],
    integrations: [],
    rate_schedule: {
      provider: "",
      current_rate: 0,
      est_monthly_cost: 0,
    },
  };
}

export function normalizePowerAnalytics(raw: unknown): AnalyticsPower {
  const fallback = emptyPowerAnalytics();
  const record = toRecord(raw);
  const power: AnalyticsPower = {
    live_kw: readNumber(
      record,
      [
        ["live_kw"],
        ["live", "kw"],
        ["consumption", "live_kw"],
      ],
      fallback.live_kw,
    ),
    live_solar_kw: readNumber(
      record,
      [
        ["live_solar_kw"],
        ["solar", "live_kw"],
        ["solar", "kw"],
      ],
      fallback.live_solar_kw,
    ),
    live_grid_kw: readNumber(
      record,
      [
        ["live_grid_kw"],
        ["grid", "live_kw"],
        ["grid", "kw"],
      ],
      fallback.live_grid_kw,
    ),
    live_battery_kw: readNumber(
      record,
      [
        ["live_battery_kw"],
        ["battery", "live_kw"],
        ["storage", "live_kw"],
      ],
      fallback.live_battery_kw,
    ),
    kwh_24h: readNumber(
      record,
      [
        ["kwh_24h"],
        ["consumption", "kwh_24h"],
      ],
      fallback.kwh_24h,
    ),
    kwh_168h: readNumber(
      record,
      [
        ["kwh_168h"],
        ["consumption", "kwh_168h"],
      ],
      fallback.kwh_168h,
    ),
    solar_kwh_24h: readNumber(
      record,
      [
        ["solar_kwh_24h"],
        ["solar", "kwh_24h"],
      ],
      fallback.solar_kwh_24h,
    ),
    solar_kwh_168h: readNumber(
      record,
      [
        ["solar_kwh_168h"],
        ["solar", "kwh_168h"],
      ],
      fallback.solar_kwh_168h,
    ),
    grid_kwh_24h: readNumber(
      record,
      [
        ["grid_kwh_24h"],
        ["grid", "kwh_24h"],
      ],
      fallback.grid_kwh_24h,
    ),
    grid_kwh_168h: readNumber(
      record,
      [
        ["grid_kwh_168h"],
        ["grid", "kwh_168h"],
      ],
      fallback.grid_kwh_168h,
    ),
    battery_kwh_24h: readNumber(
      record,
      [
        ["battery_kwh_24h"],
        ["storage", "kwh_24h"],
        ["battery", "kwh_24h"],
      ],
      fallback.battery_kwh_24h,
    ),
    battery_kwh_168h: readNumber(
      record,
      [
        ["battery_kwh_168h"],
        ["storage", "kwh_168h"],
        ["battery", "kwh_168h"],
      ],
      fallback.battery_kwh_168h,
    ),
    series_24h: readSeries(
      record,
      [
        ["series_24h"],
        ["series", "total_24h"],
        ["consumption", "series_24h"],
      ],
      fallback.series_24h,
    ),
    series_168h: readSeries(
      record,
      [
        ["series_168h"],
        ["series", "total_168h"],
        ["consumption", "series_168h"],
      ],
      fallback.series_168h,
    ),
    solar_series_24h: readSeries(
      record,
      [
        ["solar_series_24h"],
        ["solar", "series_24h"],
      ],
      fallback.solar_series_24h ?? [],
    ),
    solar_series_168h: readSeries(
      record,
      [
        ["solar_series_168h"],
        ["solar", "series_168h"],
      ],
      fallback.solar_series_168h ?? [],
    ),
    grid_series_24h: readSeries(
      record,
      [
        ["grid_series_24h"],
        ["grid", "series_24h"],
      ],
      fallback.grid_series_24h ?? [],
    ),
    grid_series_168h: readSeries(
      record,
      [
        ["grid_series_168h"],
        ["grid", "series_168h"],
      ],
      fallback.grid_series_168h ?? [],
    ),
    battery_series_24h: readSeries(
      record,
      [
        ["battery_series_24h"],
        ["storage", "series_24h"],
        ["battery", "series_24h"],
      ],
      fallback.battery_series_24h ?? [],
    ),
    battery_series_168h: readSeries(
      record,
      [
        ["battery_series_168h"],
        ["storage", "series_168h"],
        ["battery", "series_168h"],
      ],
      fallback.battery_series_168h ?? [],
    ),
    integrations: normalizeIntegrations(
      readArray(record, [["integrations"]]),
      fallback.integrations ?? [],
    ),
    rate_schedule: normalizeRateSchedule(
      record.rate_schedule,
      fallback.rate_schedule,
    ),
  };
  return power;
}

export function emptyWaterAnalytics(): AnalyticsWater {
  return {
    domestic_gal_24h: 0,
    domestic_gal_168h: 0,
    ag_gal_24h: 0,
    ag_gal_168h: 0,
    reservoir_depth: [],
    domestic_series: [],
    ag_series: [],
    domestic_series_24h: [],
    domestic_series_168h: [],
    ag_series_24h: [],
    ag_series_168h: [],
  };
}

export function normalizeWaterAnalytics(raw: unknown): AnalyticsWater {
  const fallback = emptyWaterAnalytics();
  const record = toRecord(raw);

  const domesticSeries = readSeries(
    record,
    [
      ["domestic_series"],
      ["domestic", "series"],
    ],
    fallback.domestic_series,
  );

  const agSeries = readSeries(
    record,
    [
      ["ag_series"],
      ["ag", "series"],
    ],
    fallback.ag_series,
  );

  return {
    domestic_gal_24h: readNumber(
      record,
      [["domestic_gal_24h"], ["domestic", "gal_24h"]],
      fallback.domestic_gal_24h,
    ),
    domestic_gal_168h: readNumber(
      record,
      [["domestic_gal_168h"], ["domestic", "gal_168h"]],
      fallback.domestic_gal_168h,
    ),
    ag_gal_24h: readNumber(
      record,
      [["ag_gal_24h"], ["ag", "gal_24h"]],
      fallback.ag_gal_24h,
    ),
    ag_gal_168h: readNumber(
      record,
      [["ag_gal_168h"], ["ag", "gal_168h"]],
      fallback.ag_gal_168h,
    ),
    reservoir_depth: readSeries(
      record,
      [
        ["reservoir_depth"],
        ["reservoir", "series"],
      ],
      fallback.reservoir_depth,
    ),
    domestic_series: domesticSeries,
    ag_series: agSeries,
    domestic_series_24h: readSeries(
      record,
      [
        ["domestic_series_24h"],
        ["domestic", "series_24h"],
        ["domestic_series"],
        ["domestic", "series"],
      ],
      domesticSeries,
    ),
    domestic_series_168h: readSeries(
      record,
      [
        ["domestic_series_168h"],
        ["domestic", "series_168h"],
        ["domestic_series"],
        ["domestic", "series"],
      ],
      domesticSeries,
    ),
    ag_series_24h: readSeries(
      record,
      [
        ["ag_series_24h"],
        ["ag", "series_24h"],
        ["ag_series"],
        ["ag", "series"],
      ],
      agSeries,
    ),
    ag_series_168h: readSeries(
      record,
      [
        ["ag_series_168h"],
        ["ag", "series_168h"],
        ["ag_series"],
        ["ag", "series"],
      ],
      agSeries,
    ),
  };
}

export function emptySoilAnalytics(): AnalyticsSoil {
  return {
    fields: [],
    series: [],
    series_avg: [],
    series_min: [],
    series_max: [],
  };
}

const normalizeSoilField = (entry: unknown): AnalyticsSoilField | null => {
  const record = toRecord(entry);
  const name = coerceString(record.name);
  if (!name) return null;
  return {
    name,
    min: coerceNumber(record.min, 0),
    max: coerceNumber(record.max, 0),
    avg: coerceNumber(record.avg, 0),
  };
};

export function normalizeSoilAnalytics(raw: unknown): AnalyticsSoil {
  const fallback = emptySoilAnalytics();
  const record = toRecord(raw);
  const fieldsRaw = readArray(record, [["fields"]]);
  const fields = fieldsRaw
    .map((field) => normalizeSoilField(field))
    .filter((field): field is AnalyticsSoilField => field !== null);
  const seriesAvg = readSeries(
    record,
    [
      ["series_avg"],
      ["series"],
      ["avg_series"],
    ],
    fallback.series_avg ?? fallback.series,
  );
  const seriesMin = readSeries(
    record,
    [
      ["series_min"],
      ["min_series"],
    ],
    fallback.series_min ?? [],
  );
  const seriesMax = readSeries(
    record,
    [
      ["series_max"],
      ["max_series"],
    ],
    fallback.series_max ?? [],
  );
  return {
    fields,
    series: seriesAvg,
    series_avg: seriesAvg,
    series_min: seriesMin,
    series_max: seriesMax,
  };
}

export function emptyStatusAnalytics(): AnalyticsStatus {
  return {
    alarms_last_168h: 0,
    nodes_online: 0,
    nodes_offline: 0,
    remote_nodes_online: 0,
    remote_nodes_offline: 0,
    battery_soc: 0,
    battery_runtime_hours: 0,
    solar_kw: 0,
    current_load_kw: 0,
    estimated_runtime_hours: 0,
    storage_capacity_kwh: 0,
    last_updated: null,
  };
}

export function normalizeStatusAnalytics(raw: unknown): AnalyticsStatus {
  const fallback = emptyStatusAnalytics();
  const record = toRecord(raw);
  return {
    alarms_last_168h: readNumber(
      record,
      [["alarms_last_168h"], ["alarms", "last_168h"]],
      fallback.alarms_last_168h,
    ),
    nodes_online: readNumber(
      record,
      [["nodes_online"], ["nodes", "online"]],
      fallback.nodes_online,
    ),
    nodes_offline: readNumber(
      record,
      [["nodes_offline"], ["nodes", "offline"]],
      fallback.nodes_offline,
    ),
    remote_nodes_online: readNumber(
      record,
      [["remote_nodes_online"], ["remote_nodes", "online"]],
      fallback.remote_nodes_online,
    ),
    remote_nodes_offline: readNumber(
      record,
      [["remote_nodes_offline"], ["remote_nodes", "offline"]],
      fallback.remote_nodes_offline,
    ),
    battery_runtime_hours: readNumber(
      record,
      [
        ["battery_runtime_hours"],
        ["battery", "runtime_hours"],
        ["estimated_runtime_hours"],
        ["battery", "estimated_runtime_hours"],
      ],
      fallback.battery_runtime_hours,
    ),
    battery_soc: readNumber(
      record,
      [["battery_soc"], ["battery", "soc"]],
      fallback.battery_soc,
    ),
    solar_kw: readNumber(
      record,
      [["solar_kw"], ["solar", "kw"]],
      fallback.solar_kw,
    ),
    current_load_kw: readNumber(
      record,
      [["current_load_kw"], ["load", "kw"]],
      fallback.current_load_kw,
    ),
    estimated_runtime_hours: readNumber(
      record,
      [
        ["estimated_runtime_hours"],
        ["battery", "estimated_runtime_hours"],
      ],
      fallback.estimated_runtime_hours,
    ),
    storage_capacity_kwh: readNumber(
      record,
      [
        ["storage_capacity_kwh"],
        ["battery", "capacity_kwh"],
        ["storage", "capacity_kwh"],
      ],
      fallback.storage_capacity_kwh,
    ),
    last_updated:
      coerceString(
        readValue(record, ["last_updated"]) as string | undefined,
      ) ?? fallback.last_updated,
  };
}

export function emptyAnalyticsBundle(): AnalyticsBundle {
  return {
    power: emptyPowerAnalytics(),
    water: emptyWaterAnalytics(),
    soil: emptySoilAnalytics(),
    status: emptyStatusAnalytics(),
  };
}

export function normalizeAnalyticsBundle(raw: unknown): AnalyticsBundle {
  const record = toRecord(raw);
  return {
    power: normalizePowerAnalytics(record.power),
    water: normalizeWaterAnalytics(record.water),
    soil: normalizeSoilAnalytics(record.soil),
    status: normalizeStatusAnalytics(record.status),
  };
}

type Normalizer<T> = (raw: unknown) => T;

const AnalyticsPayloadSchema = z.record(z.string(), z.unknown());

async function fetchAnalyticsResource<T>(
  path: string,
  normalize: Normalizer<T>,
): Promise<T> {
  const payload = await fetchJson<unknown>(path);
  const parsed = parseApiResponse(AnalyticsPayloadSchema, payload, path);
  return normalize(parsed);
}

export const fetchPowerAnalytics = () =>
  fetchAnalyticsResource("/api/analytics/power", normalizePowerAnalytics);

export const fetchWaterAnalytics = () =>
  fetchAnalyticsResource("/api/analytics/water", normalizeWaterAnalytics);

export const fetchSoilAnalytics = () =>
  fetchAnalyticsResource("/api/analytics/soil", normalizeSoilAnalytics);

export const fetchStatusAnalytics = () =>
  fetchAnalyticsResource("/api/analytics/status", normalizeStatusAnalytics);

export async function fetchAnalyticsBundle(): Promise<AnalyticsBundle> {
  const [power, water, soil, status] = await Promise.all([
    fetchPowerAnalytics(),
    fetchWaterAnalytics(),
    fetchSoilAnalytics(),
    fetchStatusAnalytics(),
  ]);
  return {
    power,
    water,
    soil,
    status,
  };
}
