import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AlarmsPageClient from "@/app/(dashboard)/alarms/AlarmsPageClient";

const mockUseAlarmRulesQuery = vi.fn();
const mockUseNodesQuery = vi.fn();
const mockUseSensorsQuery = vi.fn();
const mockUseUsersQuery = vi.fn();

vi.mock("@/components/AuthProvider", () => ({
  useAuth: () => ({
    me: {
      id: "user-1",
      capabilities: ["config.write"],
    },
  }),
}));

vi.mock("@/lib/queries", () => ({
  useAlarmRulesQuery: () => mockUseAlarmRulesQuery(),
  useNodesQuery: () => mockUseNodesQuery(),
  useSensorsQuery: () => mockUseSensorsQuery(),
  useUsersQuery: () => mockUseUsersQuery(),
}));

vi.mock("@/features/incidents/components/IncidentsConsole", () => ({
  default: () => <div data-testid="incidents-console" />,
}));

vi.mock("@/features/alarms/components/AlarmWizard", () => ({
  default: () => <div data-testid="alarm-wizard" />,
}));

vi.mock("@/features/alarms/components/RuleHealthPanel", () => ({
  default: () => <div data-testid="rule-health-panel" />,
}));

vi.mock("@/features/alarms/components/AlarmHistoryPanel", () => ({
  default: () => <div data-testid="alarm-history-panel" />,
}));

vi.mock("@/features/alarms/hooks/useAlarmWizard", () => ({
  default: () => ({
    open: false,
    setOpen: vi.fn(),
    step: 1,
    setStep: vi.fn(),
    canAdvance: true,
    state: {
      mode: "create",
      name: "",
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
      thresholdValue: "10",
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
      messageTemplate: "",
      advancedJson: "{}",
      advancedMode: false,
    },
    patch: vi.fn(),
    openCreate: vi.fn(),
    openEdit: vi.fn(),
    openDuplicate: vi.fn(),
    reset: vi.fn(),
  }),
}));

vi.mock("@/features/alarms/hooks/useAlarmMutations", () => ({
  default: () => ({
    create: vi.fn(),
    update: vi.fn(),
    delete: vi.fn(),
    enable: vi.fn(),
    disable: vi.fn(),
    preview: vi.fn(),
  }),
}));

describe("AlarmsPageClient (operator-grade shell)", () => {
  beforeEach(() => {
    mockUseAlarmRulesQuery.mockReturnValue({ data: [], error: null, isLoading: false });
    mockUseNodesQuery.mockReturnValue({ data: [], error: null, isLoading: false });
    mockUseSensorsQuery.mockReturnValue({ data: [], error: null, isLoading: false });
    mockUseUsersQuery.mockReturnValue({ data: [], error: null, isLoading: false });
  });

  it("renders Incidents by default and allows switching to Rules", () => {
    render(<AlarmsPageClient />);

    expect(screen.getByText("Alarms")).toBeTruthy();
    expect(screen.getByText("Incidents")).toBeTruthy();
    expect(screen.getByText("Rules")).toBeTruthy();
    expect(screen.getByTestId("incidents-console")).toBeTruthy();

    fireEvent.click(screen.getByText("Rules"));

    expect(screen.queryByTestId("incidents-console")).toBeNull();
    expect(screen.getByTestId("rule-health-panel")).toBeTruthy();
    expect(screen.getByTestId("alarm-history-panel")).toBeTruthy();
  });
});

