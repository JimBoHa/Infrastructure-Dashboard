import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { useState } from "react";
import AlarmWizard from "@/features/alarms/components/AlarmWizard";
import type { AlarmRuleCreateRequest, AlarmWizardState } from "@/features/alarms/types/alarmTypes";
import type { DemoNode, DemoSensor, TrendSeriesEntry } from "@/types/dashboard";

vi.mock("@/components/charts/HighchartsPanel", () => ({
  HighchartsPanel: () => <div data-testid="mock-highcharts-panel" />,
}));

vi.mock("@/components/TrendChart", () => ({
  TrendChart: () => <div data-testid="mock-trend-chart" />,
}));

vi.mock("@/features/trends/hooks/useAnalysisJob", () => ({
  useAnalysisJob: () => ({
    result: null,
    error: null,
    progressMessage: null,
    isSubmitting: false,
    isRunning: false,
    canCancel: false,
    run: vi.fn().mockResolvedValue(null),
    cancel: vi.fn().mockResolvedValue(undefined),
  }),
  generateJobKey: (params: Record<string, unknown>) => JSON.stringify(params),
}));

vi.mock("@/lib/api", async () => {
  const actual = await vi.importActual<typeof import("@/lib/api")>("@/lib/api");
  return {
    ...actual,
    fetchAlarmRuleStats: vi.fn(),
  };
});

const previewSeries: TrendSeriesEntry[] = [
  {
    sensor_id: "sensor-1",
    unit: "C",
    points: [
      { timestamp: "2026-02-10T00:00:00Z" as unknown as Date, value: 41, samples: 1 },
      { timestamp: "2026-02-10T00:01:00Z" as unknown as Date, value: 43, samples: 1 },
    ],
  },
];

vi.mock("@/lib/queries", async () => {
  const actual = await vi.importActual<typeof import("@/lib/queries")>("@/lib/queries");
  return {
    ...actual,
    useTrendPreviewQuery: () => ({ data: previewSeries, error: null, isLoading: false }),
  };
});

function renderWithQueryClient(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });
  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

const sensors: DemoSensor[] = [
  {
    sensor_id: "sensor-1",
    node_id: "node-1",
    name: "Temp",
    type: "temperature",
    unit: "C",
    interval_seconds: 60,
    rolling_avg_seconds: 0,
    created_at: new Date(),
    config: {},
  },
];

const nodes: DemoNode[] = [
  {
    id: "node-1",
    name: "Node",
    status: "online",
    uptime_seconds: 0,
    cpu_percent: null,
    storage_used_bytes: null,
    mac_eth: null,
    mac_wifi: null,
    ip_last: null,
    last_seen: null,
    created_at: new Date(),
    config: {},
  },
];

const initialState: AlarmWizardState = {
  mode: "create",
  name: "Overheat",
  description: "",
  severity: "warning",
  origin: "threshold",
  template: "threshold",
  selectorMode: "sensor",
  sensorId: "sensor-1",
  nodeId: "",
  filterProvider: "",
  filterMetric: "",
  filterType: "",
  thresholdOp: "gt",
  thresholdValue: "50",
  rangeMode: "inside",
  rangeLow: "0",
  rangeHigh: "1",
  offlineSeconds: "5",
  rollingWindowSeconds: "300",
  rollingAggregate: "avg",
  rollingOp: "gt",
  rollingValue: "1",
  deviationWindowSeconds: "300",
  deviationBaseline: "mean",
  deviationMode: "absolute",
  deviationValue: "1",
  consecutivePeriod: "eval",
  consecutiveCount: "2",
  debounceSeconds: "0",
  clearHysteresisSeconds: "0",
  evalIntervalSeconds: "0",
  messageTemplate: "Alarm fired",
  advancedJson: "{}",
  advancedMode: false,
};

describe("AlarmWizard operator flow", () => {
  it("reaches Guidance + Backtest steps with mocked stats/preview", async () => {
    const api = await import("@/lib/api");
    const fetchAlarmRuleStats = api.fetchAlarmRuleStats as unknown as ReturnType<typeof vi.fn>;

    fetchAlarmRuleStats.mockResolvedValue({
      start: "2026-02-10T00:00:00Z",
      end: "2026-02-11T00:00:00Z",
      interval_seconds: 60,
      bucket_aggregation_mode: "auto",
      sensors: [
        {
          sensor_id: "sensor-1",
          unit: "C",
          interval_seconds: 60,
          n: 100,
          min: 0,
          max: 100,
          mean: 50,
          median: 49,
          stddev: 10,
          p01: 5,
          p05: 10,
          p25: 40,
          p75: 60,
          p95: 90,
          p99: 95,
          mad: 8,
          iqr: 20,
          coverage_pct: 90,
          missing_pct: 10,
          bands: {
            classic: {
              lower_1: 40,
              upper_1: 60,
              lower_2: 30,
              upper_2: 70,
              lower_3: 20,
              upper_3: 80,
            },
            robust: {
              lower_1: 41,
              upper_1: 59,
              lower_2: 33,
              upper_2: 67,
              lower_3: 25,
              upper_3: 75,
            },
          },
        },
      ],
    });

    const onSave = vi.fn<Parameters<(payload: AlarmRuleCreateRequest) => Promise<void>>, Promise<void>>().mockResolvedValue();
    const onPreview = vi.fn().mockResolvedValue({ targets_evaluated: 0, results: [] });

    function Wrapper() {
      const [step, setStep] = useState(1);
      const [state, setState] = useState<AlarmWizardState>(initialState);
      return (
        <AlarmWizard
          open
          onOpenChange={() => undefined}
          step={step}
          onStepChange={setStep}
          state={state}
          onPatch={(partial) => setState((prev) => ({ ...prev, ...partial }))}
          sensors={sensors}
          nodes={nodes}
          canAdvance
          saving={false}
          onSave={onSave}
          onPreview={onPreview}
        />
      );
    }

    renderWithQueryClient(<Wrapper />);

    fireEvent.click(screen.getByRole("button", { name: /next/i }));
    fireEvent.click(screen.getByRole("button", { name: /next/i }));

    expect(screen.getByText(/Step 3\/5: Guidance/i)).toBeTruthy();

    await waitFor(() => expect(fetchAlarmRuleStats).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByText("Stats")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: /next/i }));

    expect(screen.getByText(/Step 4\/5: Backtest/i)).toBeTruthy();
    expect(screen.getByText("Backtest")).toBeTruthy();
    expect(screen.getByRole("button", { name: /run backtest/i })).toBeTruthy();
  });
});

