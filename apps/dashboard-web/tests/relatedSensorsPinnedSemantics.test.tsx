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
      queries: { retry: false },
    },
  });

  return render(<QueryClientProvider client={queryClient}>{ui}</QueryClientProvider>);
}

const demoNodes: DemoNode[] = [{ id: "node-1", name: "North Field" }];

function emptyUnifiedResult(params: RelatedSensorsUnifiedResultV2["params"]): RelatedSensorsUnifiedResultV2 {
  const pinnedRequested = Array.isArray(params.pinned_sensor_ids) ? params.pinned_sensor_ids.length : 0;
  const eligibleCount = new Set([
    ...(params.candidate_sensor_ids ?? []),
    ...(params.pinned_sensor_ids ?? []),
  ]).size;
  return {
    job_type: "related_sensors_unified_v2",
    focus_sensor_id: params.focus_sensor_id,
    computed_through_ts: "2026-02-10T00:00:00Z",
    interval_seconds: 60,
    bucket_count: 10,
    params,
    limits_used: { candidate_limit_used: 80, max_results_used: 20, max_sensors_used: 81 },
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
      pinned_requested: pinnedRequested,
      pinned_included: pinnedRequested,
      pinned_truncated: 0,
    },
    versions: {},
  };
}

describe("Related Sensors pinned semantics (ticket 56)", () => {
  it("includes pinned_sensor_ids in repeated Simple-mode Find requests", async () => {
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
      return { result: emptyUnifiedResult(params) };
    });

    const sensors: DemoSensor[] = [
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
        sensor_id: "sensor-a",
        node_id: "node-1",
        name: "Candidate A",
        type: "moisture",
        unit: "%",
        created_at: "2026-01-01T00:00:00Z",
        config: {},
      },
      {
        sensor_id: "sensor-b",
        node_id: "node-1",
        name: "Candidate B",
        type: "moisture",
        unit: "%",
        created_at: "2026-01-01T00:00:00Z",
        config: {},
      },
    ];

    const labelMap = new Map<string, string>([
      ["sensor-focus", "North Field — Focus Sensor (%)"],
      ["sensor-a", "North Field — Candidate A (%)"],
      ["sensor-b", "North Field — Candidate B (%)"],
    ]);

    renderWithQueryClient(
      <RelationshipFinderPanel
        nodesById={new Map(demoNodes.map((n) => [n.id, n]))}
        sensors={sensors}
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
    await screen.findByText("completed");
    createAnalysisJob.mockClear();

    fireEvent.change(screen.getByTestId("relationship-finder-pin-sensor-id"), {
      target: { value: "sensor-b" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Pin" }));

    fireEvent.click(screen.getByRole("button", { name: "Find related sensors" }));
    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    {
      const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
      expect(request.params?.pinned_sensor_ids).toEqual(["sensor-b"]);
    }
    await screen.findByText("completed");

    createAnalysisJob.mockClear();
    expect(
      screen.queryByRole("button", { name: "Refine (more candidates)" }),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Find related sensors" }));
    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    {
      const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
      expect(request.params?.pinned_sensor_ids).toEqual(["sensor-b"]);
    }
  });

  it("does not truncate pinned_sensor_ids to candidate_limit when pinned count exceeds the requested cap", async () => {
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
      return { result: emptyUnifiedResult(params) };
    });

    const focus: DemoSensor = {
      sensor_id: "sensor-focus",
      node_id: "node-1",
      name: "Focus Sensor",
      type: "moisture",
      unit: "%",
      created_at: "2026-01-01T00:00:00Z",
      config: {},
    };
    const pins: DemoSensor[] = Array.from({ length: 25 }, (_, idx) => {
      const n = String(idx + 1).padStart(2, "0");
      return {
        sensor_id: `sensor-${n}`,
        node_id: "node-1",
        name: `Candidate ${n}`,
        type: "moisture",
        unit: "%",
        created_at: "2026-01-01T00:00:00Z",
        config: {},
      };
    });

    const labelMap = new Map<string, string>([
      [focus.sensor_id, "North Field — Focus Sensor (%)"],
      ...pins.map((sensor) => [sensor.sensor_id, `North Field — ${sensor.name} (%)`] as const),
    ]);

    renderWithQueryClient(
      <RelationshipFinderPanel
        nodesById={new Map(demoNodes.map((n) => [n.id, n]))}
        sensors={[focus, ...pins]}
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
    await screen.findByText("completed");
    createAnalysisJob.mockClear();

    const pinInput = screen.getByTestId("relationship-finder-pin-sensor-id");
    for (const sensor of pins) {
      fireEvent.change(pinInput, { target: { value: sensor.sensor_id } });
      fireEvent.click(screen.getByRole("button", { name: "Pin" }));
    }

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));
    const candidateLimitInput = screen.getByLabelText("Candidate limit");
    fireEvent.focus(candidateLimitInput);
    fireEvent.change(candidateLimitInput, { target: { value: "20" } });
    fireEvent.blur(candidateLimitInput);

    createAnalysisJob.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Advanced (configure scoring)" }));
    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    {
      const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
      expect(request.params?.candidate_limit).toBe(20);
      expect(request.params?.pinned_sensor_ids).toEqual(pins.map((sensor) => sensor.sensor_id).sort());
    }
  });
});
