import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import PreviewPane from "@/features/trends/components/relationshipFinder/PreviewPane";
import type { NormalizedCandidate } from "@/features/trends/types/relationshipFinder";
import type { RelatedSensorsUnifiedCandidateV2, TssePreviewResponseV1 } from "@/types/analysis";
import type { DemoSensor } from "@/types/dashboard";

vi.mock("@/components/HighchartsProvider", () => ({
  Highcharts: {},
}));

vi.mock("highcharts-react-official", async () => {
  const React = await import("react");
  const Mock = React.forwardRef(function MockHighcharts() {
    return React.createElement("div", { "data-testid": "mock-highcharts-react" });
  });
  return { default: Mock };
});

vi.mock("@/components/TrendChart", () => ({
  TrendChart: () => <div data-testid="mock-trend-chart" />,
}));

vi.mock("@/lib/api", () => ({
  fetchAnalysisPreview: vi.fn(),
}));

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

function makePreviewResponse(): TssePreviewResponseV1 {
  return {
    focus: {
      sensor_id: "sensor-focus",
      unit: "C",
      points: [
        { timestamp: "2026-02-10T00:00:00Z", value: 1, samples: 1 },
        { timestamp: "2026-02-10T00:01:00Z", value: 2, samples: 1 },
      ],
    },
    candidate: {
      sensor_id: "sensor-candidate",
      unit: "C",
      points: [
        { timestamp: "2026-02-10T00:00:00Z", value: 10, samples: 1 },
        { timestamp: "2026-02-10T00:01:00Z", value: 11, samples: 1 },
      ],
    },
    bucket_seconds: 60,
    event_overlays: {
      focus_event_ts_ms: [],
      candidate_event_ts_ms: [],
      matched_focus_event_ts_ms: [],
      matched_candidate_event_ts_ms: [],
      tolerance_seconds: 0,
    },
  };
}

describe("Related Sensors top lag hypotheses (ticket 76)", () => {
  it("renders Top lags in Advanced preview and lets operators pick a lag for preview alignment", async () => {
    const api = await import("@/lib/api");
    const fetchAnalysisPreview = api.fetchAnalysisPreview as unknown as ReturnType<typeof vi.fn>;
    fetchAnalysisPreview.mockResolvedValue(makePreviewResponse());

    const sensors: DemoSensor[] = [
      {
        sensor_id: "sensor-focus",
        node_id: "node-1",
        name: "Focus Sensor",
        type: "temperature",
        unit: "C",
        created_at: "2026-02-01T00:00:00Z",
        config: {},
      },
      {
        sensor_id: "sensor-candidate",
        node_id: "node-1",
        name: "Candidate Sensor",
        type: "temperature",
        unit: "C",
        created_at: "2026-02-01T00:00:00Z",
        config: {},
      },
    ];

    const sensorsById = new Map(sensors.map((s) => [s.sensor_id, s]));
    const labelMap = new Map<string, string>([
      ["sensor-focus", "Node — Focus Sensor (C)"],
      ["sensor-candidate", "Node — Candidate Sensor (C)"],
    ]);

    const unifiedData: RelatedSensorsUnifiedCandidateV2 = {
      sensor_id: "sensor-candidate",
      derived_from_focus: false,
      derived_dependency_path: null,
      rank: 1,
      blended_score: 0.9,
      confidence_tier: "high",
      episodes: [
        {
          start_ts: "2026-02-10T00:00:00Z",
          end_ts: "2026-02-10T01:00:00Z",
          window_sec: 3600,
          lag_sec: 0,
          lag_iqr_sec: 0,
          score_mean: 0.2,
          score_peak: 2.0,
          coverage: 0.2,
          num_points: 10,
        },
      ],
      top_bucket_timestamps: [],
      why_ranked: {
        episode_count: 1,
        best_lag_sec: 0,
        best_window_sec: 3600,
        best_lag_r_ci_low: null,
        best_lag_r_ci_high: null,
        coverage_pct: 20,
        score_components: {},
        penalties: [],
        bonuses: [],
      },
      evidence: {
        events_score: 0.2,
        events_overlap: 4,
        n_focus: 10,
        n_candidate: 10,
        cooccurrence_count: 0,
        best_lag_sec: 0,
        top_lags: [
          { lag_sec: 600, score: 0.21, overlap: 4, n_candidate: 10 },
          { lag_sec: 0, score: 0.2, overlap: 4, n_candidate: 10 },
          { lag_sec: -600, score: 0.19, overlap: 4, n_candidate: 10 },
        ],
        summary: [],
      },
    };

    const candidate: NormalizedCandidate = {
      sensor_id: "sensor-candidate",
      label: "Candidate Sensor",
      node_name: "Node",
      node_id: "node-1",
      sensor_type: "temperature",
      unit: "C",
      rank: 1,
      score: 0.9,
      score_label: "0.90",
      badges: [],
      strategy: "unified",
      status: "ok",
      raw: { type: "unified", data: unifiedData },
    };

    renderWithQueryClient(
      <PreviewPane
        focusSensorId="sensor-focus"
        focusLabel="Focus Sensor"
        candidate={candidate}
        sensorsById={sensorsById}
        labelMap={labelMap}
        selectedSensorIds={["sensor-focus"]}
        maxSeries={10}
        relationshipMode="advanced"
        strategy="unified"
        series={[]}
        intervalSeconds={60}
        effectiveIntervalSeconds={60}
        analysisBucketCount={60}
      />,
    );

    expect(await screen.findByText("Top lags")).toBeTruthy();
    expect(
      screen.getByText("Lag +10m (F1 0.21, matched 4)"),
    ).toBeTruthy();

    await waitFor(() => expect(fetchAnalysisPreview).toHaveBeenCalledTimes(1));
    expect(fetchAnalysisPreview.mock.calls[0]?.[0]).toEqual(
      expect.objectContaining({ lag_seconds: 0 }),
    );

    fireEvent.click(
      screen.getByRole("button", {
        name: "Use lag +10m for preview alignment",
      }),
    );

    await waitFor(() => expect(fetchAnalysisPreview).toHaveBeenCalledTimes(2));
    expect(fetchAnalysisPreview.mock.calls[1]?.[0]).toEqual(
      expect.objectContaining({ lag_seconds: 600 }),
    );
  });
});
