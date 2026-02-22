import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import type { ReactElement } from "react";
import RelationshipFinderPanel from "@/features/trends/components/RelationshipFinderPanel";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { RelatedSensorsUnifiedResultV2 } from "@/types/analysis";
import { PROVIDER_NO_HISTORY_LABEL } from "@/features/trends/utils/relatedSensorsUnifiedDiagnostics";

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
      queries: { retry: false },
    },
  });

  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

const demoNodes: DemoNode[] = [{ id: "node-1", name: "North Field" }];

const focus: DemoSensor = {
  sensor_id: "sensor-focus",
  node_id: "node-1",
  name: "Focus Sensor",
  type: "moisture",
  unit: "%",
  created_at: "2026-01-01T00:00:00Z",
  config: {},
};

const regular: DemoSensor = {
  sensor_id: "sensor-regular",
  node_id: "node-1",
  name: "Regular Sensor",
  type: "moisture",
  unit: "%",
  created_at: "2026-01-01T00:00:00Z",
  config: {},
};

const provider: DemoSensor = {
  sensor_id: "sensor-provider",
  node_id: "node-1",
  name: "Forecast Provider",
  type: "temperature",
  unit: "°C",
  created_at: "2026-01-01T00:00:00Z",
  config: { source: "forecast_points" },
};

const labelMap = new Map<string, string>([
  [focus.sensor_id, "North Field — Focus Sensor (%)"],
  [regular.sensor_id, "North Field — Regular Sensor (%)"],
  [provider.sensor_id, "North Field — Forecast Provider (°C)"],
]);

function emptyUnifiedResult(params: RelatedSensorsUnifiedResultV2["params"]): RelatedSensorsUnifiedResultV2 {
  const eligibleCount = params.candidate_sensor_ids?.length ?? 0;
  return {
    job_type: "related_sensors_unified_v2",
    focus_sensor_id: params.focus_sensor_id,
    computed_through_ts: "2026-02-10T00:00:00Z",
    interval_seconds: 60,
    bucket_count: 10,
    params,
    limits_used: { candidate_limit_used: 80, max_results_used: 20, max_sensors_used: 20 },
    candidates: [],
    skipped_candidates: [],
    prefiltered_candidate_sensor_ids: [],
    truncated_candidate_sensor_ids: [],
    truncated_result_sensor_ids: [],
    timings_ms: {},
    counts: {
      candidate_pool: eligibleCount,
      eligible_count: eligibleCount,
      evaluated_count: eligibleCount,
      ranked: 0,
    },
    versions: {},
  };
}

describe("Related Sensors provider/forecast availability (ticket 72)", () => {
  it("excludes provider sensors by default in Simple mode", async () => {
    const api = await import("@/lib/api");
    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

    createAnalysisJob.mockResolvedValue({ job: { id: "job-1", status: "completed" } });
    fetchAnalysisJob.mockImplementation(async (jobId: string) => ({ job: { id: jobId, status: "completed" } }));
    fetchAnalysisJobResult.mockImplementation(async (_jobId: string) => ({
      result: emptyUnifiedResult({
        focus_sensor_id: focus.sensor_id,
        start: "2026-02-09T00:00:00Z",
        end: "2026-02-10T00:00:00Z",
        interval_seconds: 60,
        candidate_source: "all_sensors_in_scope",
        candidate_sensor_ids: [],
        candidate_limit: 80,
        filters: {
          exclude_sensor_ids: [focus.sensor_id],
          is_public_provider: false,
        },
      }),
    }));

    renderWithQueryClient(
      <RelationshipFinderPanel
        nodesById={new Map(demoNodes.map((n) => [n.id, n]))}
        sensors={[focus, regular, provider]}
        series={[]}
        selectedBadges={[]}
        selectedSensorIds={[focus.sensor_id]}
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
    const request = createAnalysisJob.mock.calls[0]![0];
    expect(request.params.filters.is_public_provider).toBe(false);
    expect(request.params.candidate_source).toBe("all_sensors_in_scope");
    expect(request.params.candidate_sensor_ids).toEqual([]);
  });

  it("when providers are included in Advanced mode, skipped provider sensors surface as not-available", async () => {
	    const api = await import("@/lib/api");
	    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
	    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
	    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

	    let simpleJobCounter = 0;
	    createAnalysisJob.mockImplementation(async (request: { params?: unknown }) => {
	      const params = request.params as { filters?: { is_public_provider?: boolean } } | undefined;
	      const includesProvider = params?.filters?.is_public_provider === undefined;

	      return {
	        job: {
	          id: includesProvider ? "job-advanced" : `job-simple-${++simpleJobCounter}`,
	          status: "completed",
	        },
	      };
	    });
	    fetchAnalysisJob.mockImplementation(async (jobId: string) => ({ job: { id: jobId, status: "completed" } }));

    fetchAnalysisJobResult.mockImplementation(async (jobId: string) => {
      if (jobId === "job-advanced") {
        const params: RelatedSensorsUnifiedResultV2["params"] = {
          focus_sensor_id: focus.sensor_id,
          start: "2026-02-09T00:00:00Z",
          end: "2026-02-10T00:00:00Z",
          interval_seconds: 60,
          candidate_source: "all_sensors_in_scope",
          candidate_sensor_ids: [],
          candidate_limit: 80,
          filters: { exclude_sensor_ids: [focus.sensor_id] },
        };
        return {
          result: {
            ...emptyUnifiedResult(params),
            counts: { candidate_pool: 1, eligible_count: 2, evaluated_count: 1, ranked: 0 },
            skipped_candidates: [{ sensor_id: provider.sensor_id, reason: "no_lake_history" }],
          },
        };
      }

      return {
        result: emptyUnifiedResult({
          focus_sensor_id: focus.sensor_id,
          start: "2026-02-09T00:00:00Z",
          end: "2026-02-10T00:00:00Z",
          interval_seconds: 60,
          candidate_source: "all_sensors_in_scope",
          candidate_sensor_ids: [],
          candidate_limit: 80,
          filters: {
            exclude_sensor_ids: [focus.sensor_id],
            is_public_provider: false,
          },
        }),
      };
    });

    renderWithQueryClient(
      <RelationshipFinderPanel
        nodesById={new Map(demoNodes.map((n) => [n.id, n]))}
        sensors={[focus, regular, provider]}
        series={[]}
        selectedBadges={[]}
        selectedSensorIds={[focus.sensor_id]}
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

	    fireEvent.click(await screen.findByRole("button", { name: "Advanced" }));
	    fireEvent.click(screen.getByLabelText(/Include provider\/forecast sensors/i));
	    fireEvent.click(screen.getByRole("button", { name: "Advanced (configure scoring)" }));

	    await waitFor(() => {
	      const calls = createAnalysisJob.mock.calls.map((call) => call[0]);
	      expect(
	        calls.some((call) => {
	          const params = call.params as { filters?: { is_public_provider?: boolean } } | undefined;
	          return params?.filters?.is_public_provider === undefined;
	        }),
	      ).toBe(true);
	    });

	    const request = createAnalysisJob.mock.calls
	      .map((call) => call[0])
	      .find((call) => {
	        const params = call.params as { filters?: { is_public_provider?: boolean } } | undefined;
	        return params?.filters?.is_public_provider === undefined;
	      })!;
	    expect(request.params.filters.is_public_provider).toBeUndefined();
	    expect(request.params.candidate_source).toBe("all_sensors_in_scope");
	    expect(request.params.candidate_sensor_ids).toEqual([]);

    await screen.findByText(`Skipped: 1 provider/forecast sensors — ${PROVIDER_NO_HISTORY_LABEL}`);

    const input = await screen.findByTestId("relationship-finder-diagnostic-sensor-id");
    fireEvent.change(input, { target: { value: provider.sensor_id } });
    await waitFor(() => {
      expect(screen.getByTestId("relationship-finder-diagnostic-result")).toHaveTextContent(
        PROVIDER_NO_HISTORY_LABEL,
      );
    });
  });
});
