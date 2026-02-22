import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import IncidentDetailSheet from "@/features/incidents/components/IncidentDetailSheet";
import type { DemoNode, DemoSensor, DemoUser, TrendSeriesEntry } from "@/types/dashboard";
import type { Incident, IncidentNote } from "@/types/incidents";

vi.mock("@/components/TrendChart", () => ({
  TrendChart: () => <div data-testid="mock-trend-chart" />,
}));

vi.mock("@/features/trends/components/relationshipFinder/ResultsList", () => ({
  default: () => <div data-testid="mock-related-results" />,
}));

vi.mock("@/features/trends/components/relationshipFinder/PreviewPane", () => ({
  default: () => <div data-testid="mock-related-preview" />,
}));

vi.mock("@/features/trends/hooks/useAnalysisJob", () => ({
  useAnalysisJob: () => ({
    result: null,
    error: null,
    progressMessage: null,
    isSubmitting: false,
    isRunning: true,
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
    fetchIncidentDetail: vi.fn(),
    fetchIncidentNotes: vi.fn(),
    createIncidentNote: vi.fn(),
    fetchActionLogs: vi.fn(),
    assignIncident: vi.fn(),
    snoozeIncident: vi.fn(),
    closeIncident: vi.fn(),
    postJson: vi.fn(),
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
    useAlarmsQuery: () => ({ data: [], error: null, isLoading: false }),
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

const users: DemoUser[] = [
  {
    id: "user-1",
    name: "Operator",
    email: "operator@example.com",
    role: "admin",
    capabilities: ["config.write"],
    last_login: null,
  },
];

describe("IncidentDetailSheet operator workflows", () => {
  it("lets operators add an incident note", async () => {
    const api = await import("@/lib/api");
    const fetchIncidentDetail = api.fetchIncidentDetail as unknown as ReturnType<typeof vi.fn>;
    const fetchIncidentNotes = api.fetchIncidentNotes as unknown as ReturnType<typeof vi.fn>;
    const createIncidentNote = api.createIncidentNote as unknown as ReturnType<typeof vi.fn>;
    const fetchActionLogs = api.fetchActionLogs as unknown as ReturnType<typeof vi.fn>;

    const incident: Incident = {
      id: "incident-1",
      rule_id: "1",
      target_key: "sensor:sensor-1",
      severity: "warning",
      status: "open",
      title: "Overheat on Temp",
      assigned_to: null,
      snoozed_until: null,
      first_event_at: new Date("2026-02-11T00:00:00Z"),
      last_event_at: new Date("2026-02-11T00:00:00Z"),
      closed_at: null,
      created_at: new Date("2026-02-11T00:00:00Z"),
      updated_at: new Date("2026-02-11T00:00:00Z"),
      total_event_count: 1,
      active_event_count: 1,
      note_count: 0,
      last_message: "Too hot",
      last_origin: "threshold",
      last_sensor_id: "sensor-1",
      last_node_id: "node-1",
    };

    fetchIncidentDetail.mockResolvedValue({
      incident,
      events: [
        {
          id: "evt-1",
          alarm_id: "alarm-1",
          rule_id: "1",
          sensor_id: "sensor-1",
          node_id: "node-1",
          origin: "threshold",
          message: "Too hot",
          status: "triggered",
          created_at: "2026-02-11T00:00:00Z",
          raised_at: "2026-02-11T00:00:00Z",
          transition: "fired",
        },
      ],
    });

    fetchIncidentNotes.mockResolvedValue({ notes: [], next_cursor: null });
    fetchActionLogs.mockResolvedValue([]);

    const created: IncidentNote = {
      id: "note-1",
      incident_id: "incident-1",
      created_by: "user-1",
      body: "Investigated and adjusted threshold.",
      created_at: new Date("2026-02-11T00:05:00Z"),
    };
    createIncidentNote.mockResolvedValue(created);

    renderWithQueryClient(
      <IncidentDetailSheet
        open
        onOpenChange={() => undefined}
        incidentId="incident-1"
        canEdit
        canAck={false}
        meUserId="user-1"
        sensors={sensors}
        nodes={nodes}
        users={users}
      />,
    );

    await waitFor(() => expect(screen.getByText("Overheat on Temp")).toBeTruthy());

    const textarea = await screen.findByPlaceholderText(/what did you observe/i);
    fireEvent.change(textarea, { target: { value: "Investigated and adjusted threshold." } });

    const button = screen.getByRole("button", { name: /^add note$/i });
    fireEvent.click(button);

    await waitFor(() =>
      expect(createIncidentNote).toHaveBeenCalledWith("incident-1", "Investigated and adjusted threshold."),
    );
  });
});

