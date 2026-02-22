import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import type { ReactElement } from "react";
import RelationshipFinderPanel from "@/features/trends/components/RelationshipFinderPanel";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { RelatedSensorsUnifiedResultV2 } from "@/types/analysis";

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

vi.mock("@/lib/api", () => ({
  cancelAnalysisJob: vi.fn(),
  createAnalysisJob: vi.fn(),
  fetchAnalysisJob: vi.fn(),
  fetchAnalysisJobEvents: vi.fn(),
  fetchAnalysisJobResult: vi.fn(),
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

const demoNodes: DemoNode[] = [{ id: "node-1", name: "North Field" }];

const demoSensors: DemoSensor[] = [
  {
    sensor_id: "sensor-focus",
    node_id: "node-1",
    name: "Focus Sensor",
    type: "moisture",
    unit: "%",
    created_at: "2026-01-01T00:00:00Z",
    config: {},
  },
  {
    sensor_id: "sensor-candidate",
    node_id: "node-1",
    name: "Candidate Sensor",
    type: "moisture",
    unit: "%",
    created_at: "2026-01-01T00:00:00Z",
    config: {},
  },
];

const labelMap = new Map<string, string>([
  ["sensor-focus", "North Field — Focus Sensor (%)"],
  ["sensor-candidate", "North Field — Candidate Sensor (%)"],
]);

describe("Related Sensors Advanced mitigation controls (tickets 73/75)", () => {
  it("surfaces deseasoning/periodic penalty + delta corr controls and submits params in Advanced mode", async () => {
    const api = await import("@/lib/api");
    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

    let counter = 0;
    const paramsByJobId = new Map<string, RelatedSensorsUnifiedResultV2["params"]>();

    createAnalysisJob.mockImplementation(async (request: { params?: RelatedSensorsUnifiedResultV2["params"] }) => {
      const id = `job-${++counter}`;
      if (request.params) paramsByJobId.set(id, request.params);
      return { job: { id, status: "completed" } };
    });
    fetchAnalysisJob.mockImplementation(async (jobId: string) => ({ job: { id: jobId, status: "completed" } }));
    fetchAnalysisJobResult.mockImplementation(async (jobId: string) => {
      const params = paramsByJobId.get(jobId);
      if (!params) throw new Error(`missing params for ${jobId}`);
      const result: RelatedSensorsUnifiedResultV2 = {
        job_type: "related_sensors_unified_v2",
        focus_sensor_id: params.focus_sensor_id,
        computed_through_ts: "2026-02-10T00:00:00Z",
        interval_seconds: 60,
        bucket_count: 10,
        params,
        limits_used: { candidate_limit_used: 80, max_results_used: 20, max_sensors_used: 81 },
        candidates: [],
        skipped_candidates: [],
        system_wide_buckets: [],
        prefiltered_candidate_sensor_ids: [],
        truncated_candidate_sensor_ids: [],
        truncated_result_sensor_ids: [],
        gap_skipped_deltas: {},
        timings_ms: {},
        counts: { eligible_count: 1, evaluated_count: 1, ranked: 0 },
        versions: {},
      };
      return { result };
    });

    renderWithQueryClient(
      <RelationshipFinderPanel
        nodesById={new Map(demoNodes.map((n) => [n.id, n]))}
        sensors={demoSensors}
        series={[]}
        selectedBadges={[]}
        selectedSensorIds={["sensor-focus"]}
        labelMap={labelMap}
        intervalSeconds={60}
        rangeHours={24}
        rangeSelect="24h"
        customStartIso={null}
        customEndIso={null}
        customRangeValid={true}
        maxSeries={20}
      />,
    );

    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalled());
    createAnalysisJob.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));

    expect(
      screen.getByText(
        "Mitigate diurnal/periodic artifacts (may reduce true positives for truly periodic mechanisms).",
      ),
    ).toBeInTheDocument();

    fireEvent.change(screen.getByRole("combobox", { name: /deseasoning/i }), {
      target: { value: "hour_of_day_mean" },
    });

    fireEvent.click(screen.getByRole("checkbox", { name: /Periodic penalty/i }));
    fireEvent.click(screen.getByRole("checkbox", { name: /Include.*corr/i }));

    fireEvent.click(screen.getByRole("button", { name: "Advanced (configure scoring)" }));

    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };

    expect(request.params?.deseason_mode).toBe("hour_of_day_mean");
    expect(request.params?.periodic_penalty_enabled).toBe(false);
    expect(request.params?.include_delta_corr_signal).toBe(true);
    expect((request.params?.weights as Record<string, unknown>)?.delta_corr).toBe(0.2);
  });
});

