"use client";

import Highcharts from "highcharts/highstock";
import HighchartsAnnotationsModule from "highcharts/modules/annotations";
import HighchartsAnnotationsAdvancedModule from "highcharts/modules/annotations-advanced";
import HighchartsMoreModule from "highcharts/highcharts-more";
import HighchartsBoostModule from "highcharts/modules/boost";
import HighchartsStockToolsModule from "highcharts/modules/stock-tools";
import HighchartsDragPanesModule from "highcharts/modules/drag-panes";
import HighchartsPriceIndicatorModule from "highcharts/modules/price-indicator";
import HighchartsFullScreenModule from "highcharts/modules/full-screen";
import HighchartsHeatmapModule from "highcharts/modules/heatmap";

// Helper to handle ESM/CommonJS interop for Highcharts modules
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const initModule = (mod: any, hc: typeof Highcharts) => {
  const fn = typeof mod === "function" ? mod : mod?.default;
  if (typeof fn === "function") {
    fn(hc);
  }
};

// Initialize modules (client-side only for Next.js)
if (typeof window !== "undefined") {
  initModule(HighchartsMoreModule, Highcharts);
  initModule(HighchartsAnnotationsModule, Highcharts);
  initModule(HighchartsAnnotationsAdvancedModule, Highcharts);
  initModule(HighchartsBoostModule, Highcharts);
  initModule(HighchartsDragPanesModule, Highcharts);
  initModule(HighchartsPriceIndicatorModule, Highcharts);
  initModule(HighchartsFullScreenModule, Highcharts);
  initModule(HighchartsHeatmapModule, Highcharts);
  initModule(HighchartsStockToolsModule, Highcharts);
}

// Default dark/light theme support
Highcharts.setOptions({
  chart: {
    backgroundColor: "transparent",
    style: {
      fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
    },
  },
  credits: { enabled: false },
  title: { text: undefined },
  accessibility: { enabled: false },
  time: { timezoneOffset: 0 },
  rangeSelector: {
    buttonTheme: {
      fill: "none",
      stroke: "none",
      "stroke-width": 0,
      r: 8,
      style: { color: "#6b7280", fontWeight: "500" },
      states: {
        hover: { fill: "#e5e7eb", style: { color: "#111827" } },
        select: { fill: "#4f46e5", style: { color: "#ffffff" } },
      },
    },
    inputStyle: {
      color: "#374151",
      fontWeight: "400",
    },
    labelStyle: {
      color: "#6b7280",
      fontWeight: "400",
    },
  },
  navigator: {
    maskFill: "rgba(79, 70, 229, 0.1)",
    outlineColor: "#e5e7eb",
    handles: {
      backgroundColor: "#f3f4f6",
      borderColor: "#9ca3af",
    },
    series: {
      color: "#6366f1",
      lineWidth: 1,
    },
  },
  scrollbar: { enabled: false },
  tooltip: {
    backgroundColor: "#ffffff",
    borderColor: "#e5e7eb",
    borderRadius: 8,
    shadow: true,
    style: { color: "#111827", fontSize: "12px" },
  },
  legend: {
    itemStyle: { color: "#374151", fontWeight: "500" },
    itemHoverStyle: { color: "#111827" },
  },
  xAxis: {
    lineColor: "#e5e7eb",
    tickColor: "#e5e7eb",
    labels: { style: { color: "#6b7280", fontSize: "11px" } },
    gridLineColor: "#f3f4f6",
  },
  yAxis: {
    lineColor: "#e5e7eb",
    tickColor: "#e5e7eb",
    labels: { style: { color: "#6b7280", fontSize: "11px" } },
    gridLineColor: "#f3f4f6",
    title: { style: { color: "#6b7280", fontSize: "11px" } },
  },
  plotOptions: {
    series: {
      animation: { duration: 300 },
    },
    line: {
      lineWidth: 1.5,
      marker: { enabled: false, radius: 3 },
    },
    area: {
      lineWidth: 1.5,
      marker: { enabled: false, radius: 3 },
    },
    scatter: {
      marker: { radius: 4 },
    },
    column: {
      borderWidth: 0,
      borderRadius: 2,
    },
  },
});

export { Highcharts };
