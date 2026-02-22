import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { vi } from "vitest";
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

describe("Related Sensors workflow improvements (ticket 62)", () => {
  it("defaults to All nodes scope in Simple mode", () => {
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
        rangeSelect="custom"
        customStartIso={null}
        customEndIso={null}
        customRangeValid={false}
        maxSeries={20}
      />,
    );

    const scopeSelect = screen.getByRole("combobox", { name: /scope/i });
    expect(scopeSelect).toHaveValue("all_nodes");
  });

  it("shows System-wide events and wires Jump to ±1h action", async () => {
    const api = await import("@/lib/api");
    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

    const jobId = "job-system-wide";
    createAnalysisJob.mockResolvedValue({ job: { id: jobId, status: "completed" } });
    fetchAnalysisJob.mockResolvedValue({ job: { id: jobId, status: "completed" } });

    const systemWideTs = 1_760_140_800_000; // 2026-02-10T00:00:00Z (ms)
    const result: RelatedSensorsUnifiedResultV2 = {
      job_type: "related_sensors_unified_v2",
      focus_sensor_id: "sensor-focus",
      computed_through_ts: "2026-02-10T00:00:00Z",
      interval_seconds: 60,
      bucket_count: 42,
      params: {
        focus_sensor_id: "sensor-focus",
        start: "2026-02-09T00:00:00Z",
        end: "2026-02-10T00:00:00Z",
        interval_seconds: 60,
        mode: "simple",
        quick_suggest: true,
        candidate_sensor_ids: ["sensor-candidate"],
        candidate_limit: 80,
        max_results: 20,
        filters: { exclude_sensor_ids: ["sensor-focus"] },
      },
      limits_used: {
        candidate_limit_used: 80,
        max_results_used: 20,
        max_sensors_used: 81,
      },
      candidates: [],
      skipped_candidates: [],
      system_wide_buckets: [
        { ts: systemWideTs, group_size: 18, severity_sum: 33.4 },
      ],
      prefiltered_candidate_sensor_ids: [],
      truncated_candidate_sensor_ids: [],
      truncated_result_sensor_ids: [],
      timings_ms: {},
      counts: {
        candidate_pool: 1,
        eligible_count: 1,
        evaluated_count: 1,
        ranked: 0,
        cooccurrence_total_sensors: 30,
      },
      versions: {},
    };

    fetchAnalysisJobResult.mockResolvedValue({ result });

    const onJumpToTimestamp = vi.fn();

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
        rangeSelect="24"
        customStartIso={null}
        customEndIso={null}
        customRangeValid={true}
        maxSeries={20}
        onJumpToTimestamp={onJumpToTimestamp}
      />,
    );

    await screen.findByText("System-wide events");

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Jump to ±1h" }),
      ).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: "Jump to ±1h" }));
    expect(onJumpToTimestamp).toHaveBeenCalledWith(systemWideTs);
  });

  it("passes exclude-system-wide-buckets as a run parameter", async () => {
    const api = await import("@/lib/api");
    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

    const jobId = "job-exclude-system-wide";
    createAnalysisJob.mockResolvedValue({ job: { id: jobId, status: "completed" } });
    fetchAnalysisJob.mockResolvedValue({ job: { id: jobId, status: "completed" } });
    fetchAnalysisJobResult.mockResolvedValue({ result: null });

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
        rangeSelect="24"
        customStartIso={null}
        customEndIso={null}
        customRangeValid={true}
        maxSeries={20}
      />,
    );

    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalled());
    await screen.findByText("completed");
    createAnalysisJob.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));
    fireEvent.click(screen.getByLabelText("Exclude system-wide buckets"));
    fireEvent.click(
      screen.getByRole("button", { name: "Advanced (configure scoring)" }),
    );

    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
    expect(request.params?.exclude_system_wide_buckets).toBe(true);
  });

  it("excludes derived-from-focus candidates by default in Simple mode and allows including them in Advanced", async () => {
    const api = await import("@/lib/api");
    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

    const jobId = "job-exclude-derived";
    createAnalysisJob.mockResolvedValue({ job: { id: jobId, status: "completed" } });
    fetchAnalysisJob.mockResolvedValue({ job: { id: jobId, status: "completed" } });
    fetchAnalysisJobResult.mockResolvedValue({ result: null });
    createAnalysisJob.mockClear();

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
        sensor_id: "sensor-derived",
        node_id: "node-1",
        name: "Derived from focus",
        type: "derived",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {
          source: "derived",
          derived: {
            expression: "a * 2",
            inputs: [{ sensor_id: "sensor-focus", var: "a", lag_seconds: 0 }],
          },
        },
      },
      {
        sensor_id: "sensor-normal",
        node_id: "node-1",
        name: "Normal candidate",
        type: "temperature",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {},
      },
    ];

    const labelMap = new Map<string, string>([
      ["sensor-focus", "North Field — Focus Sensor (C)"],
      ["sensor-derived", "North Field — Derived from focus (C)"],
      ["sensor-normal", "North Field — Normal candidate (C)"],
    ]);

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
        rangeSelect="24"
        customStartIso={null}
        customEndIso={null}
        customRangeValid={true}
        maxSeries={20}
      />,
    );

    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalled());
    await screen.findByText("completed");
    {
      const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
      const candidateIds = request.params?.candidate_sensor_ids as string[] | undefined;
      const excludeIds = request.params?.filters as { exclude_sensor_ids?: string[] } | undefined;
      expect(request.params?.candidate_source).toBe("all_sensors_in_scope");
      expect(candidateIds).toEqual([]);
      expect(excludeIds?.exclude_sensor_ids).toEqual(["sensor-derived", "sensor-focus"]);
    }
    createAnalysisJob.mockClear();

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));
    fireEvent.click(screen.getByLabelText("Include derived-from-focus candidates"));
    fireEvent.click(
      screen.getByRole("button", { name: "Advanced (configure scoring)" }),
    );

    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    {
      const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
      const candidateIds = request.params?.candidate_sensor_ids as string[] | undefined;
      const excludeIds = request.params?.filters as { exclude_sensor_ids?: string[] } | undefined;
      expect(request.params?.candidate_source).toBe("all_sensors_in_scope");
      expect(candidateIds).toEqual([]);
      expect(excludeIds?.exclude_sensor_ids).toEqual(["sensor-focus"]);
    }
  });
});
