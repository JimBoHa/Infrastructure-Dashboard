import { render, screen } from "@testing-library/react";
import { vi } from "vitest";
import AnalyticsPage from "@/app/(dashboard)/analytics/page";
import type { AnalyticsFeedStatus, AnalyticsBundle, DemoNode, DemoSensor, TrendSeriesEntry } from "@/types/dashboard";

let mockAnalytics: AnalyticsBundle | null = null;
let mockFeedStatus: AnalyticsFeedStatus | null = null;
const mockForecastStatus = {
  providers: {
    "Open-Meteo": { status: "ok" },
    "Forecast.Solar": { status: "ok" },
  },
};
const mockNodes: DemoNode[] = [
  {
    id: "node-1",
    name: "North Field",
    status: "online",
    uptime_seconds: 0,
    cpu_percent: 0,
    storage_used_bytes: 0,
    mac_eth: null,
    mac_wifi: null,
    ip_last: null,
    last_seen: null,
    created_at: new Date(),
    config: {},
  },
];
const mockSensors: DemoSensor[] = [
  {
    sensor_id: "sensor-batt-1",
    node_id: "node-1",
    name: "Battery Voltage",
    type: "renogy_bt2",
    unit: "V",
    interval_seconds: 30,
    rolling_avg_seconds: 0,
    config: { metric: "battery_voltage_v", category: "battery" },
    created_at: new Date(),
  },
];
const mockVoltageSeries: TrendSeriesEntry[] = [
  {
    sensor_id: "sensor-batt-1",
    label: "North Field",
    points: [{ timestamp: new Date("2025-01-01T00:00:00Z"), value: 12.8 }],
  },
];

function getFeedStatus() {
  return mockFeedStatus;
}

vi.mock("@/lib/queries", () => ({
  useConnectionQuery: () => ({
    data: { timezone: "UTC" },
    error: null,
    isLoading: false,
  }),
  useAnalyticsQuery: () => ({
    data: mockAnalytics,
    error: null,
    isLoading: false,
  }),
  useAnalyticsFeedStatusQuery: () => ({ data: getFeedStatus() }),
  useForecastStatusQuery: () => ({ data: mockForecastStatus, error: null, isLoading: false }),
  useWeatherForecastConfigQuery: () => ({ data: null, error: null, isLoading: false }),
  useWeatherForecastHourlyQuery: () => ({ data: null, error: null, isLoading: false }),
  useWeatherForecastDailyQuery: () => ({ data: null, error: null, isLoading: false }),
  usePvForecastConfigQuery: () => ({ data: null, error: null, isLoading: false }),
  usePvForecastHourlyQuery: () => ({ data: null, error: null, isLoading: false, isFetching: false }),
  useSensorsQuery: () => ({
    data: mockSensors,
    isLoading: false,
    error: null,
  }),
  useNodesQuery: () => ({
    data: mockNodes,
    isLoading: false,
    error: null,
  }),
  useMetricsQuery: () => ({
    data: mockVoltageSeries,
    isLoading: false,
    isFetching: false,
    error: null,
  }),
}));

const analytics = {
  power: {
    live_kw: 10,
    live_solar_kw: 4,
    live_grid_kw: 6,
    kwh_24h: 120,
    kwh_168h: 840,
    solar_kwh_24h: 50,
    solar_kwh_168h: 300,
    series_24h: [],
    solar_series_24h: [],
    grid_series_24h: [],
    battery_series_24h: [],
    series_168h: [],
    solar_series_168h: [],
    grid_series_168h: [],
    battery_series_168h: [],
    integrations: [],
    rate_schedule: { provider: "Utility", current_rate: 0.2, est_monthly_cost: 100 },
  },
  water: {
    domestic_gal_24h: 1200,
    domestic_gal_168h: 8000,
    ag_gal_168h: 16000,
    ag_gal_24h: 600,
    reservoir_depth: [],
    domestic_series_24h: [],
    ag_series_24h: [],
    ag_series_168h: [],
  },
  soil: {
    fields: [{ name: "Field 1", min: 20, max: 40, avg: 30 }],
    series: [],
    series_avg: [],
    series_min: [],
    series_max: [],
  },
  status: {
    alarms_last_168h: 1,
    nodes_online: 3,
    nodes_offline: 1,
    remote_nodes_online: 2,
    remote_nodes_offline: 0,
    battery_soc: 75,
    solar_kw: 4,
    current_load_kw: 6,
    estimated_runtime_hours: 5,
    storage_capacity_kwh: 13,
  },
};

describe("AnalyticsPage", () => {
  beforeEach(() => {
    mockFeedStatus = {
      enabled: true,
      feeds: { Emporia: { status: "ok" }, Enphase: { status: "error" } },
      history: [],
    };
    mockAnalytics = analytics;
  });

  it("shows feed health chips", () => {
    render(<AnalyticsPage />);
    expect(screen.getByRole("heading", { name: /Feed health/i })).toBeInTheDocument();
    expect(screen.getByText("Emporia")).toBeInTheDocument();
    expect(screen.getByText("Enphase")).toBeInTheDocument();
  });

  it("renders the battery voltage chart card", () => {
    render(<AnalyticsPage />);
    expect(
      screen.getByRole("heading", { name: /Battery voltage/i }),
    ).toBeInTheDocument();
  });
});
