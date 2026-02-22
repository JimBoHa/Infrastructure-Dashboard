import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/http", () => {
  const fetchJson = vi.fn();
  return {
    fetchJson,
    extractStatus: (error: unknown) => {
      if (error instanceof Error) {
        const match = /Request failed \((\d{3})/u.exec(error.message);
        if (match) {
          return Number(match[1]);
        }
      }
      return null;
    },
  };
});

import { fetchJson } from "@/lib/http";
import {
  fetchAnalyticsBundle,
  normalizeAnalyticsBundle,
  normalizePowerAnalytics,
  normalizeSoilAnalytics,
  normalizeStatusAnalytics,
  normalizeWaterAnalytics,
} from "@/lib/analytics";

const mockedFetchJson = vi.mocked(fetchJson);

beforeEach(() => {
  mockedFetchJson.mockReset();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("normalizePowerAnalytics", () => {
  it("coerces legacy flat payloads", () => {
    const result = normalizePowerAnalytics({
      live_kw: 18.4,
      live_solar_kw: 6.1,
      live_grid_kw: 12.3,
      kwh_24h: 420.5,
      kwh_168h: 1960.2,
      series_24h: [{ timestamp: "2025-11-01T00:00:00Z", value: 12 }],
      rate_schedule: {
        provider: "Demo Utility",
        current_rate: 0.27,
        est_monthly_cost: 512.4,
      },
    });

    expect(result.live_kw).toBeCloseTo(18.4);
    expect(result.series_24h).toHaveLength(1);
    expect(result.series_168h).toHaveLength(result.series_168h.length);
    expect(result.rate_schedule.provider).toBe("Demo Utility");
    expect(result.solar_series_24h).toHaveLength(0);
    expect(result.grid_kwh_24h).toBe(0);
  });

  it("maps nested consumption data and converts strings", () => {
    const result = normalizePowerAnalytics({
      consumption: {
        live_kw: 42.5,
        kwh_24h: 180.2,
        kwh_168h: 960.4,
        series_24h: [{ timestamp: "2025-11-01T00:00:00Z", value: 20 }],
        series_168h: [{ timestamp: "2025-10-30T00:00:00Z", value: 15 }],
      },
      solar: {
        live_kw: 16.7,
        kwh_24h: 72.5,
        series_24h: [{ timestamp: "2025-11-01T00:00:00Z", value: 8 }],
        series_168h: [{ timestamp: "2025-10-30T00:00:00Z", value: 7 }],
      },
      grid: {
        live_kw: 25.8,
        kwh_24h: 90.4,
      },
      storage: {
        live_kw: 3.1,
        kwh_24h: 12.3,
        series_24h: [{ timestamp: "2025-11-01T00:00:00Z", value: 4 }],
        series_168h: [{ timestamp: "2025-10-30T00:00:00Z", value: 3 }],
      },
      integrations: [{ name: "Tesla Gateway", status: "connected", details: "Main house" }],
      rate_schedule: {
        provider: "Utility Pro",
        current_rate: "0.33",
        est_monthly_cost: "640.12",
        currency: "USD",
      },
    });

    expect(result.live_kw).toBeCloseTo(42.5);
    expect(result.live_solar_kw).toBeCloseTo(16.7);
    expect(result.live_grid_kw).toBeCloseTo(25.8);
    expect(result.live_battery_kw).toBeCloseTo(3.1);
    expect(result.solar_kwh_24h).toBeCloseTo(72.5);
    expect(result.grid_kwh_24h).toBeCloseTo(90.4);
    expect(result.battery_kwh_24h).toBeCloseTo(12.3);
    expect(result.rate_schedule.current_rate).toBeCloseTo(0.33);
    expect(result.rate_schedule.est_monthly_cost).toBeCloseTo(640.12);
    expect(result.integrations).toHaveLength(1);
    expect(result.battery_series_24h).toHaveLength(1);
  });
});

describe("normalizeWaterAnalytics", () => {
  it("supports legacy domestic/ag series", () => {
    const result = normalizeWaterAnalytics({
      domestic_gal_24h: 1200,
      ag_gal_168h: 5400,
      domestic_series: [{ timestamp: "2025-11-01T00:00:00Z", value: 40 }],
      ag_series: [{ timestamp: "2025-10-31T00:00:00Z", value: 80 }],
      reservoir_depth: [{ timestamp: "2025-10-31T00:00:00Z", value: 62 }],
    });

    expect(result.domestic_gal_24h).toBe(1200);
    expect(result.ag_gal_168h).toBe(5400);
    expect(result.domestic_series_24h).toHaveLength(1);
    expect(result.ag_series_168h.length).toBeGreaterThan(0);
    expect(result.reservoir_depth).toHaveLength(1);
  });
});

describe("normalizeSoilAnalytics", () => {
  it("builds field stats and averages", () => {
    const result = normalizeSoilAnalytics({
      fields: [{ name: "North Field", min: 32.1, max: 38.6, avg: 35.4 }],
      series_avg: [{ timestamp: "2025-11-01T00:00:00Z", value: 35 }],
    });

    expect(result.fields).toHaveLength(1);
    expect(result.series_avg).toHaveLength(1);
    expect(result.series_min).toHaveLength(0);
    expect(result.series_max).toHaveLength(0);
  });
});

describe("normalizeStatusAnalytics", () => {
  it("derives metrics from nested payloads", () => {
    const result = normalizeStatusAnalytics({
      nodes: { online: 6, offline: 1 },
      remote_nodes: { online: 3, offline: 2 },
      battery: { soc: 64, estimated_runtime_hours: 9, capacity_kwh: 84 },
      solar: { kw: 12.5 },
      load: { kw: 8.2 },
      alarms: { last_168h: 4 },
    });

    expect(result.nodes_online).toBe(6);
    expect(result.remote_nodes_offline).toBe(2);
    expect(result.battery_soc).toBe(64);
    expect(result.current_load_kw).toBeCloseTo(8.2);
    expect(result.estimated_runtime_hours).toBe(9);
    expect(result.storage_capacity_kwh).toBe(84);
  });
});

describe("normalizeAnalyticsBundle", () => {
  it("falls back to empty bundle when analytics missing", () => {
    const bundle = normalizeAnalyticsBundle(undefined);
    expect(bundle.power.live_kw).toBe(0);
    expect(bundle.water.domestic_series_24h).toEqual([]);
    expect(bundle.status.nodes_offline).toBe(0);
  });
});

describe("fetchAnalyticsBundle", () => {
  it("returns normalized data from four endpoints", async () => {
    mockedFetchJson
      .mockResolvedValueOnce({
        live_kw: 20,
        live_solar_kw: 8,
        live_grid_kw: 12,
        kwh_24h: 300,
        rate_schedule: { provider: "Utility", current_rate: 0.25, est_monthly_cost: 400 },
      })
      .mockResolvedValueOnce({
        domestic_gal_24h: 800,
        ag_gal_168h: 2000,
      })
      .mockResolvedValueOnce({
        fields: [{ name: "Pasture", min: 28, max: 34, avg: 31 }],
      })
      .mockResolvedValueOnce({
        alarms_last_168h: 2,
        nodes_online: 5,
        nodes_offline: 1,
      });

    const result = await fetchAnalyticsBundle();
    expect(mockedFetchJson.mock.calls).toEqual([
      ["/api/analytics/power"],
      ["/api/analytics/water"],
      ["/api/analytics/soil"],
      ["/api/analytics/status"],
    ]);
    expect(result.power.live_kw).toBe(20);
    expect(result.water.domestic_gal_24h).toBe(800);
    expect(result.soil.fields).toHaveLength(1);
    expect(result.status.nodes_offline).toBe(1);
  });

  it("surfaces request failures (no demo fallback)", async () => {
    mockedFetchJson.mockRejectedValue(new Error("Request failed (503): upstream error"));
    await expect(fetchAnalyticsBundle()).rejects.toThrow(/Request failed/);
  });
});
