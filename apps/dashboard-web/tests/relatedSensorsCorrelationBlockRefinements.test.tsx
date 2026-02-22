import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import type { ReactElement } from "react";
import RelationshipFinderPanel from "@/features/trends/components/RelationshipFinderPanel";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type {
  CorrelationMatrixJobParamsV1,
  CorrelationMatrixResultV1,
  RelatedSensorsUnifiedResultV2,
} from "@/types/analysis";

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
    type: "temperature",
    unit: "C",
    created_at: "2026-01-01T00:00:00Z",
    config: {},
  },
  {
    sensor_id: "sensor-candidate",
    node_id: "node-1",
    name: "Candidate Sensor",
    type: "temperature",
    unit: "C",
    created_at: "2026-01-01T00:00:00Z",
    config: {},
  },
];

const labelMap = new Map<string, string>([
  ["sensor-focus", "North Field — Focus Sensor (C)"],
  ["sensor-candidate", "North Field — Candidate Sensor (C)"],
]);

describe("Related Sensors correlation block refinements (ticket 61)", () => {
  it("defaults to focus-vs-candidate list and keeps the full matrix as an explicit opt-in", async () => {
    const api = await import("@/lib/api");
    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

    const jobTypeById = new Map<string, string>();
    const paramsById = new Map<string, unknown>();

    let counter = 0;
    createAnalysisJob.mockImplementation(async (request: { job_type?: string; params?: unknown }) => {
      const id = `job-${++counter}`;
      jobTypeById.set(id, request.job_type ?? "unknown");
      if (request.params) paramsById.set(id, request.params);
      return { job: { id, status: "completed" } };
    });
    fetchAnalysisJob.mockImplementation(async (jobId: string) => ({ job: { id: jobId, status: "completed" } }));
    fetchAnalysisJobResult.mockImplementation(async (jobId: string) => {
      const kind = jobTypeById.get(jobId);
      const params = paramsById.get(jobId);
      if (kind === "related_sensors_unified_v2") {
        const unifiedParams = params as RelatedSensorsUnifiedResultV2["params"];
        const result: RelatedSensorsUnifiedResultV2 = {
          job_type: "related_sensors_unified_v2",
          focus_sensor_id: unifiedParams.focus_sensor_id,
          computed_through_ts: "2026-02-10T00:00:00Z",
          interval_seconds: 60,
          bucket_count: 10,
          params: unifiedParams,
          limits_used: { candidate_limit_used: 80, max_results_used: 20, max_sensors_used: 81 },
          candidates: [
            {
              sensor_id: "sensor-candidate",
              derived_from_focus: false,
              derived_dependency_path: null,
              rank: 1,
              blended_score: 0.9,
              confidence_tier: "high",
              episodes: null,
              top_bucket_timestamps: [],
              why_ranked: null,
              evidence: {
                events_score: 0.2,
                events_overlap: 4,
                n_focus: 10,
                n_candidate: 10,
                cooccurrence_count: 1,
                cooccurrence_strength: 0.3,
                summary: [],
              },
            },
          ],
          skipped_candidates: [],
          system_wide_buckets: [],
          prefiltered_candidate_sensor_ids: [],
          truncated_candidate_sensor_ids: [],
          truncated_result_sensor_ids: [],
          gap_skipped_deltas: {},
          timings_ms: {},
          counts: { eligible_count: 1, evaluated_count: 1, ranked: 1 },
          versions: {},
        };
        return { result };
      }

      if (kind === "correlation_matrix_v1") {
        const matrixParams = params as CorrelationMatrixJobParamsV1;
        const result: CorrelationMatrixResultV1 = {
          job_type: "correlation_matrix_v1",
          params: matrixParams,
          sensor_ids: matrixParams.sensor_ids,
          sensors: [],
          matrix: [
            [
              { r: 1, n: 10, status: "ok" },
              { r: 0.4, n: 10, n_eff: 10, q_value: 0.01, status: "ok", lag_sec: 0 },
            ],
            [
              { r: 0.4, n: 10, n_eff: 10, q_value: 0.01, status: "ok", lag_sec: 0 },
              { r: 1, n: 10, status: "ok" },
            ],
          ],
          computed_through_ts: "2026-02-10T00:00:00Z",
          interval_seconds: 60,
          bucket_count: 10,
          truncated_sensor_ids: [],
          timings_ms: {},
          versions: {},
        };
        return { result };
      }

      throw new Error(`unexpected job type ${kind ?? "none"}`);
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

    const correlationToggle = await screen.findByRole("button", {
      name: /Correlation.*not used for ranking/i,
    });
    fireEvent.click(correlationToggle);

    const correlationBlock = await screen.findByTestId("relationship-finder-correlation-block");
    const scoped = within(correlationBlock);

    await scoped.findByRole("button", { name: /Show full matrix/i });
    expect(scoped.getByRole("button", { name: "Candidate Sensor" })).toBeInTheDocument();
    expect(scoped.queryByTestId("mock-highcharts-react")).not.toBeInTheDocument();

    fireEvent.click(scoped.getByRole("button", { name: /Show full matrix/i }));
    await waitFor(() => expect(scoped.getByTestId("mock-highcharts-react")).toBeInTheDocument());
  });
});
