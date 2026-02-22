import { describe, expect, it } from "vitest";
import React from "react";
import { render } from "@testing-library/react";

import { createScatterOptions, createTimeSeriesOptions } from "../src/lib/chartFactories";
import { formatBytes, formatDuration, formatPercent } from "../src/lib/format";
import { shouldOfferDevLogin } from "../src/lib/devLogin";
import { pickStableCandidateId } from "../src/features/trends/utils/relationshipFinderSelection";
import type { NormalizedCandidate } from "../src/features/trends/types/relationshipFinder";
import CollapsibleCard from "../src/components/CollapsibleCard";

describe("smoke: formatting helpers", () => {
  it("formats durations into compact units", () => {
    expect(formatDuration(65)).toBe("1m");
    expect(formatDuration(3661)).toBe("1h 1m");
  });

  it("formats bytes with units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
  });

  it("formats percentages with one decimal place", () => {
    expect(formatPercent(0)).toBe("0.0%");
    expect(formatPercent(12.34)).toBe("12.3%");
  });
});

describe("smoke: chart factories", () => {
  it("does not set chart.zooming to undefined when zoom is disabled (WebKit-safe)", () => {
    const time = createTimeSeriesOptions({ series: [], timeZone: "UTC", zoom: false });
    expect(time.chart && "zooming" in time.chart).toBe(false);

    const scatter = createScatterOptions({ series: [], zoom: false });
    expect(scatter.chart && "zooming" in scatter.chart).toBe(false);
  });
});

describe("smoke: dev login gate", () => {
  it("stays disabled by default (non-dev env)", () => {
    expect(shouldOfferDevLogin({ nodeEnv: "production", enableFlag: "1", hostname: "localhost" })).toBe(false);
    expect(shouldOfferDevLogin({ nodeEnv: "test", enableFlag: "1", hostname: "localhost" })).toBe(false);
  });

  it("requires explicit enable flag and localhost host", () => {
    expect(shouldOfferDevLogin({ nodeEnv: "development", enableFlag: "0", hostname: "localhost" })).toBe(false);
    expect(shouldOfferDevLogin({ nodeEnv: "development", enableFlag: "1", hostname: "example.com" })).toBe(false);
    expect(shouldOfferDevLogin({ nodeEnv: "development", enableFlag: "1", hostname: "localhost" })).toBe(true);
  });
});

describe("smoke: relationship finder selection", () => {
  const c = (sensor_id: string) => ({ sensor_id }) as unknown as NormalizedCandidate;

  it("preserves selection if it still exists after refresh", () => {
    expect(pickStableCandidateId({ previousId: "b", candidates: [c("a"), c("b")] })).toBe("b");
  });

  it("falls back to the first candidate if selection disappears", () => {
    expect(pickStableCandidateId({ previousId: "c", candidates: [c("a"), c("b")] })).toBe("a");
  });
});

describe("smoke: CollapsibleCard layout guards", () => {
  it("allows content to shrink inside constrained layouts", () => {
    render(
      React.createElement(
        CollapsibleCard,
        { title: "Sensor picker", defaultOpen: true },
        React.createElement("div", null, "Body"),
      ),
    );

    const content = document.querySelector('div[data-state="open"].grid') as HTMLElement | null;
    expect(content).not.toBeNull();
    expect(content?.className).toContain("grid-cols-1");

    const wrapper = content?.firstElementChild as HTMLElement | null;
    expect(wrapper).not.toBeNull();
    expect(wrapper?.className).toContain("min-w-0");
  });
});
