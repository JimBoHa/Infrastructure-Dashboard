import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TrendChart } from "@/components/TrendChart";
import type { ChartAnnotationRow } from "@/lib/api";
import type { TrendSeriesEntry } from "@/types/dashboard";

let latestOptions: Record<string, unknown> | null = null;

vi.mock("@/components/HighchartsProvider", () => ({
  Highcharts: {
    getOptions: () => ({ navigation: { bindings: {} } }),
  },
}));

vi.mock("@/components/charts/HighchartsPanel", () => ({
  HighchartsPanel: ({ options }: { options: Record<string, unknown> }) => {
    latestOptions = options;
    return <div data-testid="mock-highcharts-panel" />;
  },
}));

const chartData: TrendSeriesEntry[] = [
  {
    sensor_id: "sensor-1",
    label: "Soil Moisture",
    unit: "%",
    points: [
      { timestamp: new Date("2026-02-01T00:00:00Z"), value: 10 },
      { timestamp: new Date("2026-02-01T01:00:00Z"), value: 20 },
      { timestamp: new Date("2026-02-01T02:00:00Z"), value: 30 },
    ],
  },
];

function selectionHandler() {
  const handler = (latestOptions as { chart?: { events?: { selection?: unknown } } } | null)?.chart?.events
    ?.selection;
  if (typeof handler !== "function") {
    throw new Error("selection handler is not available");
  }
  return handler as (event: unknown) => unknown;
}

describe("TrendChart best-fit toolbar", () => {
  it("uses drag selection and removes the duplicate start button flow", async () => {
    render(<TrendChart data={chartData} analysisTools timeZone="UTC" />);

    expect(screen.queryByRole("button", { name: /start best fit/i })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /^Best fit$/i }));
    await waitFor(() => {
      expect(screen.getByText(/Best fit is armed/i)).toBeTruthy();
    });

    act(() => {
      selectionHandler()({
        xAxis: [{ min: Date.parse("2026-02-01T00:00:00Z"), max: Date.parse("2026-02-01T02:00:00Z") }],
      });
    });

    await waitFor(() => {
      expect(screen.getByText(/Best-fit lines/i)).toBeTruthy();
      expect(screen.getByText(/^Draft$/i)).toBeTruthy();
    });
  });

  it("saves a draft best-fit line as a persistent annotation", async () => {
    const createAnnotation = vi.fn(async (): Promise<ChartAnnotationRow> => ({
      id: "annotation-1",
      chart_state: {},
      sensor_ids: ["sensor-1"],
      created_at: "2026-02-09T00:00:00Z",
      updated_at: "2026-02-09T00:00:00Z",
    }));

    render(
      <TrendChart
        data={chartData}
        analysisTools
        timeZone="UTC"
        onCreatePersistentAnnotation={createAnnotation}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /^Best fit$/i }));
    act(() => {
      selectionHandler()({
        xAxis: [{ min: Date.parse("2026-02-01T00:00:00Z"), max: Date.parse("2026-02-01T02:00:00Z") }],
      });
    });

    await waitFor(() => expect(screen.getByRole("button", { name: /^Save$/i })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /^Save$/i }));

    await waitFor(() => {
      expect(createAnnotation).toHaveBeenCalledTimes(1);
      expect(screen.getByText(/^Saved$/i)).toBeTruthy();
    });
  });
});
