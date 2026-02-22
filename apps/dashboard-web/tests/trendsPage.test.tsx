import { render, screen, fireEvent } from "@testing-library/react";
import { vi } from "vitest";
import TrendsPage from "@/app/(dashboard)/analytics/trends/page";
import type { DemoNode, DemoSensor, TrendSeriesEntry } from "@/types/dashboard";

const mockUseNodesQuery = vi.fn();
const mockUseSensorsQuery = vi.fn();
const mockUseMetricsQuery = vi.fn();

vi.mock("@/lib/queries", () => ({
  useConnectionQuery: () => ({ data: { timezone: "UTC" }, error: null, isLoading: false }),
  useNodesQuery: () => mockUseNodesQuery(),
  useSensorsQuery: () => mockUseSensorsQuery(),
  useMetricsQuery: () => mockUseMetricsQuery(),
}));

vi.mock("@/features/trends/components/MatrixProfilePanel", () => ({
  default: () => <div data-testid="matrix-profile-panel" />,
}));

vi.mock("@/features/trends/components/RelationshipFinderPanel", () => ({
  default: () => <div data-testid="relationship-finder-panel" />,
}));

vi.mock("@/features/trends/components/SelectedSensorsCorrelationMatrixCard", () => ({
  default: () => <div data-testid="selected-correlation-matrix" />,
}));

const trendChartMock = vi.fn();
vi.mock("@/components/TrendChart", () => ({
  TrendChart: (props: unknown) => {
    trendChartMock(props);
    return (
      <div data-testid="trend-chart">
        stacked:{(props as { stacked: boolean }).stacked ? "on" : "off"}; independent:
        {(props as { independentAxes: boolean }).independentAxes ? "on" : "off"}
      </div>
    );
  },
}));

const demoSensors: DemoSensor[] = [
  {
    sensor_id: "sensor-1",
    node_id: "node-1",
    name: "Soil Moisture",
    type: "moisture",
    unit: "%",
    created_at: "2024-01-01T00:00:00Z",
    config: {},
  },
];

const demoNodes: DemoNode[] = [
  {
    id: "node-1",
    name: "North Field",
  },
];

const baselineSeries: TrendSeriesEntry[] = [
  {
    sensor_id: "sensor-1",
    label: "Soil Moisture",
    points: [{ timestamp: "2024-01-01T00:00:00Z", value: 42 }],
  },
];

describe("TrendsPage axis toggles", () => {
  beforeEach(() => {
    trendChartMock.mockClear();
    mockUseNodesQuery.mockReturnValue({
      data: demoNodes,
      error: null,
      isLoading: false,
    });
    mockUseSensorsQuery.mockReturnValue({
      data: demoSensors,
      error: null,
      isLoading: false,
    });
    mockUseMetricsQuery.mockReturnValue({
      data: baselineSeries,
      error: null,
      isLoading: false,
    });
  });

  it("toggles stacked and independent axes", () => {
    render(<TrendsPage />);
    const stackToggle = screen.getByLabelText(/^Stack$/i) as HTMLInputElement;
    const independentToggle = screen.getByLabelText(/Independent axes/i) as HTMLInputElement;

    expect(stackToggle.checked).toBe(false);
    expect(independentToggle.checked).toBe(false);
    expect(screen.getByTestId("trend-chart").textContent).toContain("stacked:off");

    fireEvent.click(stackToggle);

    expect(stackToggle.checked).toBe(true);
    expect(independentToggle.checked).toBe(false);
    expect(trendChartMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ stacked: true, independentAxes: false }),
    );

    fireEvent.click(independentToggle);

    expect(stackToggle.checked).toBe(false);
    expect(stackToggle.disabled).toBe(true);
    expect(independentToggle.checked).toBe(true);
    expect(trendChartMock).toHaveBeenLastCalledWith(
      expect.objectContaining({ stacked: false, independentAxes: true }),
    );
  });

  it("lets users toggle sensor picker selection via row click and checkbox click", () => {
    render(<TrendsPage />);

    const sensorCheckbox = screen.getByRole("checkbox", {
      name: /soil moisture/i,
    }) as HTMLInputElement;
    const sensorCard = sensorCheckbox.closest('[data-slot="card"]');
    if (!sensorCard) throw new Error("Sensor picker card not found.");

    expect(sensorCheckbox.checked).toBe(false);
    expect(screen.getByText("0/20 selected")).toBeTruthy();

    fireEvent.click(sensorCard);
    expect(sensorCheckbox.checked).toBe(true);
    expect(screen.getByText("1/20 selected")).toBeTruthy();

    fireEvent.click(sensorCheckbox);
    expect(sensorCheckbox.checked).toBe(false);
    expect(screen.getByText("0/20 selected")).toBeTruthy();

    fireEvent.click(sensorCheckbox);
    expect(sensorCheckbox.checked).toBe(true);

    fireEvent.click(sensorCard);
    expect(sensorCheckbox.checked).toBe(false);
  });
});
