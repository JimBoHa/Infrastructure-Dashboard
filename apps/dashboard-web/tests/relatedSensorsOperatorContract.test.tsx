import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import type { ReactElement } from "react";
import RelationshipFinderPanel from "@/features/trends/components/RelationshipFinderPanel";
import ResultsList from "@/features/trends/components/relationshipFinder/ResultsList";
import PreviewPane from "@/features/trends/components/relationshipFinder/PreviewPane";
import { normalizeUnifiedCandidates } from "@/features/trends/utils/candidateNormalizers";
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

describe("Related Sensors operator contract (ticket 69)", () => {
  it("renders subtitle, buttons, and micro-disclaimer copy", () => {
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

    expect(
      screen.getByText(
        "Sensors whose change events align with the focus sensor in this time range (optionally with lag). Not causality.",
      ),
    ).toBeInTheDocument();

    expect(
      screen.getByText(
        "Rankings are relative to the sensors evaluated in this run. Scores are not probabilities and can change when scope/filters change.",
      ),
    ).toBeInTheDocument();

    expect(
      screen.getByRole("button", { name: "Find related sensors" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));

    expect(
      screen.getByRole("button", { name: "Advanced (configure scoring)" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("combobox", { name: /candidate source/i }),
    ).toBeInTheDocument();
    expect(screen.getByText("Include weak evidence")).toBeInTheDocument();
  });

  it("defaults to all-sensors backend query mode with evaluate-all enabled (ticket 74)", async () => {
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
      return {
        result: {
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
          counts: { candidate_pool: 1, eligible_count: 1, evaluated_count: 1, ranked: 0 },
          versions: {},
        },
      };
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

    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    await screen.findByText("completed");
    {
      const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
      expect(request.params?.candidate_source).toBe("all_sensors_in_scope");
      expect(request.params?.candidate_sensor_ids).toEqual([]);
      expect(request.params?.evaluate_all_eligible).toBe(true);
    }

    createAnalysisJob.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));
    fireEvent.click(screen.getByLabelText(/Evaluate all eligible/i));
    fireEvent.change(screen.getByRole("combobox", { name: /candidate source/i }), {
      target: { value: "visible_in_trends" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Advanced (configure scoring)" }));
    await waitFor(() => expect(createAnalysisJob).toHaveBeenCalledTimes(1));
    {
      const request = createAnalysisJob.mock.calls[0]![0] as { params?: Record<string, unknown> };
      expect(request.params?.candidate_source).toBe("visible_in_trends");
      expect(request.params?.evaluate_all_eligible).toBeUndefined();
    }
  });

  it("shows evaluated/eligible + effective interval disclosure and exact no-results copy after a run", async () => {
    const api = await import("@/lib/api");
    const createAnalysisJob = api.createAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJob = api.fetchAnalysisJob as unknown as ReturnType<typeof vi.fn>;
    const fetchAnalysisJobResult = api.fetchAnalysisJobResult as unknown as ReturnType<typeof vi.fn>;

    const jobId = "job-related-empty";

    createAnalysisJob.mockResolvedValue({
      job: { id: jobId, status: "completed" },
    });
    fetchAnalysisJob.mockResolvedValue({
      job: { id: jobId, status: "completed" },
    });

    const result: RelatedSensorsUnifiedResultV2 = {
      job_type: "related_sensors_unified_v2",
      focus_sensor_id: "sensor-focus",
      computed_through_ts: "2026-02-10T00:00:00Z",
      interval_seconds: 120,
      bucket_count: 123,
      params: {
        focus_sensor_id: "sensor-focus",
        start: "2026-02-09T00:00:00Z",
        end: "2026-02-10T00:00:00Z",
        interval_seconds: 60,
        candidate_source: "all_sensors_in_scope",
        candidate_sensor_ids: [],
        evaluate_all_eligible: true,
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
      prefiltered_candidate_sensor_ids: [],
      truncated_candidate_sensor_ids: [],
      truncated_result_sensor_ids: [],
      timings_ms: {},
      counts: { candidate_pool: 1, eligible_count: 1, evaluated_count: 1, ranked: 0 },
      versions: {},
    };

    fetchAnalysisJobResult.mockResolvedValue({ result });

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

    await screen.findByText(/No candidates exceeded the evidence threshold in this time range\./i);

    await screen.findByText("Evaluated: 1 of 1 eligible sensors (limit: 80).");
    await screen.findByText(/Candidate source: All sensors in scope \(backend query\)/i);
    await screen.findByText("Effective interval: 120 (requested: 60).");

    expect(
      screen.getByText(
        "No candidates exceeded the evidence threshold in this time range. Evaluated 1 of 1 eligible sensors. Expand the time range, lower the event threshold, or include weak evidence in Advanced.",
      ),
    ).toBeInTheDocument();
  });

  it("renders Rank score + Evidence + pills and prunes legacy Blend/Confidence/Co-occur labels", () => {
    const nodesById = new Map(demoNodes.map((n) => [n.id, n]));
    const sensorsById = new Map(demoSensors.map((s) => [s.sensor_id, s]));

    const unifiedResult: RelatedSensorsUnifiedResultV2 = {
      job_type: "related_sensors_unified_v2",
      focus_sensor_id: "sensor-focus",
      computed_through_ts: null,
      interval_seconds: 60,
      bucket_count: 42,
      params: {
        focus_sensor_id: "sensor-focus",
        start: "2026-02-09T00:00:00Z",
        end: "2026-02-10T00:00:00Z",
        interval_seconds: 60,
        candidate_sensor_ids: ["sensor-candidate"],
        candidate_limit: 80,
        max_results: 60,
        filters: { exclude_sensor_ids: ["sensor-focus"] },
      },
      limits_used: {
        candidate_limit_used: 80,
        max_results_used: 60,
        max_sensors_used: 81,
      },
      candidates: [
        {
          sensor_id: "sensor-candidate",
          rank: 1,
          blended_score: 0.83,
          confidence_tier: "high",
          evidence: {
            events_score: 0.67,
            events_overlap: 5,
            n_focus: 8,
            n_candidate: 11,
            cooccurrence_count: 3,
            cooccurrence_strength: 0.9,
            cooccurrence_score: 123.4,
            best_lag_sec: -480,
          },
        },
      ],
      skipped_candidates: [],
      prefiltered_candidate_sensor_ids: [],
      truncated_candidate_sensor_ids: [],
      truncated_result_sensor_ids: [],
      timings_ms: {},
      counts: {},
      versions: {},
    };

    const candidates = normalizeUnifiedCandidates(unifiedResult, {
      sensorsById,
      nodesById,
      labelMap,
    });

    render(
      <ResultsList
        candidates={candidates}
        selectedCandidateId={candidates[0]!.sensor_id}
        onSelectCandidate={() => {}}
        sensorsById={sensorsById}
        nodesById={nodesById}
        badgeById={new Map()}
        selectedSensorIds={["sensor-focus"]}
        maxSeries={20}
      />,
    );

    const rankScoreLabel = screen.getByText("Rank score");
    expect(rankScoreLabel).toHaveAttribute(
      "title",
      "0–1 rank score relative to the evaluated candidates in this run. Not a probability. Not comparable across different runs or scopes.",
    );

    expect(screen.getByText("Evidence: strong")).toBeInTheDocument();
    expect(screen.getByText("Event match (F1): 0.67 • matched: 5")).toBeInTheDocument();
    expect(screen.getByText("Shared buckets: 3")).toBeInTheDocument();
    expect(screen.getByText("Co-occ strength: 0.90")).toBeInTheDocument();
    expect(screen.getByText("Lag: -8m (candidate earlier)")).toBeInTheDocument();

    expect(screen.queryByText(/Blend:/i)).toBeNull();
    expect(screen.queryByText(/Confidence:/i)).toBeNull();
    expect(screen.queryByText(/Co-occur:/i)).toBeNull();
  });

  it("renders Evidence summary copy, weak-episode warning template, and focus-event guardrail banners", async () => {
    const candidate = normalizeUnifiedCandidates(
      {
        job_type: "related_sensors_unified_v2",
        focus_sensor_id: "sensor-focus",
        computed_through_ts: null,
        interval_seconds: 120,
        bucket_count: 42,
	        params: {
	          focus_sensor_id: "sensor-focus",
	          start: "2026-02-09T00:00:00Z",
	          end: "2026-02-10T00:00:00Z",
	          interval_seconds: 60,
	          candidate_sensor_ids: ["sensor-candidate"],
	          candidate_limit: 80,
	          max_results: 60,
	          filters: { exclude_sensor_ids: ["sensor-focus"] },
	        },
	        limits_used: {
	          candidate_limit_used: 80,
	          max_results_used: 60,
	          max_sensors_used: 81,
	        },
	        candidates: [
	          {
            sensor_id: "sensor-candidate",
            rank: 1,
            blended_score: 0.5,
            confidence_tier: "low",
            episodes: [
              {
                start_ts: "2026-02-09T10:00:00Z",
                end_ts: "2026-02-09T11:00:00Z",
                window_sec: 3600,
                lag_sec: 0,
                lag_iqr_sec: 0,
                score_mean: 0.1,
                score_peak: 2.5,
                coverage: 0.02,
                num_points: 4,
              },
            ],
            evidence: {
              events_score: 0.12,
              events_overlap: 1,
              n_focus: 2,
              n_candidate: 4,
              cooccurrence_count: 1,
              cooccurrence_strength: 0.4,
              cooccurrence_score: 55.1,
              best_lag_sec: 0,
              direction_label: "opposite",
              sign_agreement: 0.25,
              delta_corr: -0.4,
              direction_n: 6,
              summary: [],
            },
	          },
		        ],
		        skipped_candidates: [],
		        prefiltered_candidate_sensor_ids: [],
		        truncated_candidate_sensor_ids: [],
		        truncated_result_sensor_ids: [],
		        timings_ms: {},
		        counts: {},
		        versions: {},
	      } as RelatedSensorsUnifiedResultV2,
      {
        sensorsById: new Map(demoSensors.map((s) => [s.sensor_id, s])),
        nodesById: new Map(demoNodes.map((n) => [n.id, n])),
        labelMap,
      },
    )[0]!;

    renderWithQueryClient(
      <PreviewPane
        focusSensorId={null}
        focusLabel="Focus"
        candidate={candidate}
        sensorsById={new Map(demoSensors.map((s) => [s.sensor_id, s]))}
        labelMap={labelMap}
        selectedSensorIds={[]}
        maxSeries={20}
        strategy="unified"
        relationshipMode="advanced"
        intervalSeconds={60}
        effectiveIntervalSeconds={120}
      />,
    );

    expect(screen.getByText("Evidence summary")).toBeInTheDocument();
    expect(screen.getByText("Event match (F1)")).toBeInTheDocument();
    expect(screen.getByText("Matched events")).toBeInTheDocument();
    expect(screen.getByText("Shared selected buckets")).toBeInTheDocument();
    expect(screen.getByText("Co-occ strength")).toBeInTheDocument();
    expect(screen.getByText("Direction")).toBeInTheDocument();
    expect(screen.getByText("opposite")).toBeInTheDocument();

    expect(screen.getByText("Direction")).toHaveAttribute(
      "title",
      "Matched pairs: 6 · Sign agreement: 25% · Δ corr: -0.40",
    );

    expect(
      screen.getByText(
        "All evidence is computed on bucketed data at effective interval 120.",
      ),
    ).toBeInTheDocument();

    expect(
      screen.getByText(
        "Weak episode: only 4 matched events (2% of focus events). Treat as low evidence. Try a different episode, expand the time range, or lower the event threshold.",
      ),
    ).toBeInTheDocument();

    expect(
      screen.getByText(
        "Too few focus events for stable ranking. Expand the time range or lower the event threshold.",
      ),
    ).toBeInTheDocument();

    await waitFor(() => {
      const label = screen.getByText("Co-occ strength");
      expect(label).toHaveAttribute("title", "Raw co-occ score: 55.1");
    });
  });
});
