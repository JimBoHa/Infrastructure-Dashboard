import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { HighchartsPanel, type HighchartsChartRef } from "@/components/charts/HighchartsPanel";

let lastHighchartsProps: Record<string, unknown> | null = null;
let lastHighchartsRef: HighchartsChartRef | null = null;

vi.mock("highcharts-react-official", async () => {
  const React = await import("react");
  const Mock = React.forwardRef(function MockHighcharts(
    props: Record<string, unknown>,
    ref: React.ForwardedRef<HighchartsChartRef>,
  ) {
    lastHighchartsProps = props;
    const refValue = {
      chart: {
        zoomOut: vi.fn(),
        reflow: vi.fn(),
      },
      container: { current: null },
    } as unknown as HighchartsChartRef;
    lastHighchartsRef = refValue;
    if (typeof ref === "function") {
      ref(refValue);
    } else if (ref) {
      ref.current = refValue;
    }
    return React.createElement("div", { "data-testid": "mock-highcharts-react" });
  });
  return { default: Mock };
});

vi.mock("@/components/HighchartsProvider", () => ({
  Highcharts: {},
}));

describe("HighchartsPanel", () => {
  beforeEach(() => {
    lastHighchartsProps = null;
    lastHighchartsRef = null;
  });

  it("applies default fill container classes and optional centered width", () => {
    render(
      <HighchartsPanel
        options={{}}
        containerClassName="custom-container"
        centeredMaxWidthPx={420}
      />,
    );

    const containerProps = (lastHighchartsProps?.containerProps ?? {}) as {
      className?: string;
      style?: Record<string, unknown>;
    };
    expect(containerProps.className).toContain("h-full");
    expect(containerProps.className).toContain("w-full");
    expect(containerProps.className).toContain("custom-container");
    expect(containerProps.style?.maxWidth).toBe("420px");
    expect(containerProps.style?.margin).toBe("0 auto");
  });

  it("resets chart zoom on double click when enabled", () => {
    render(
      <HighchartsPanel
        options={{}}
        testId="chart-wrapper"
        resetZoomOnDoubleClick
      />,
    );

    const zoomOutSpy = lastHighchartsRef?.chart.zoomOut as ReturnType<typeof vi.fn> | undefined;
    expect(zoomOutSpy).toBeDefined();
    fireEvent.doubleClick(screen.getByTestId("chart-wrapper"));
    expect(zoomOutSpy).toHaveBeenCalledTimes(1);
  });

  it("registers and disconnects ResizeObserver when auto-reflow is enabled", () => {
    const observeSpy = vi.spyOn(globalThis.ResizeObserver.prototype, "observe");
    const disconnectSpy = vi.spyOn(globalThis.ResizeObserver.prototype, "disconnect");

    const { unmount } = render(
      <HighchartsPanel
        options={{}}
        enableAutoReflow
      />,
    );

    expect(observeSpy).toHaveBeenCalledTimes(1);
    unmount();
    expect(disconnectSpy).toHaveBeenCalledTimes(1);
  });
});
