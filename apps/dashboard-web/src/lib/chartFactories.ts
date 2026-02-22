import type {
  AnnotationsOptions,
  Options,
  SeriesColumnOptions,
  SeriesHeatmapOptions,
  SeriesLineOptions,
  SeriesOptionsType,
  YAxisOptions,
} from "highcharts";
import { CHART_PALETTE, CHART_HEIGHTS, formatTooltipHtml } from "./chartTokens";
import { formatChartTickTime, formatChartTooltipTime } from "./siteTime";

// ---------------------------------------------------------------------------
// Base
// ---------------------------------------------------------------------------

/** Base options shared by ALL charts — tokens, credits, theme */
export function createBaseOptions(overrides?: Partial<Options>): Options {
  return {
    credits: { enabled: false },
    accessibility: { enabled: false },
    // Highcharts Stock shows the toolbar by default when the stock-tools
    // module is loaded.  Disable globally — only createStockChartOptions
    // re-enables it when stockTools: true is passed.
    stockTools: { gui: { enabled: false } },
    chart: {
      style: { fontFamily: "system-ui, -apple-system, sans-serif" },
      ...overrides?.chart,
    },
    ...overrides,
  } as Options;
}

// ---------------------------------------------------------------------------
// Time series
// ---------------------------------------------------------------------------

export type TimeSeriesFactoryOpts = {
  series: SeriesOptionsType[];
  timeZone: string;
  height?: number;
  navigator?: boolean;
  zoom?: boolean;
  yAxis?: YAxisOptions | YAxisOptions[];
  annotations?: AnnotationsOptions[];
};

/** Time series line chart — datetime x-axis, zoom, pan, optional navigator */
export function createTimeSeriesOptions(opts: TimeSeriesFactoryOpts): Options {
  const tz = opts.timeZone;
  const chart: Options["chart"] = {
    type: "line",
    height: opts.height,
  };
  if (opts.zoom !== false) {
    chart.zooming = { type: "x", mouseWheel: { enabled: true } };
    chart.panning = { enabled: true, type: "x" };
    chart.panKey = "shift";
  }
  return createBaseOptions({
    chart,
    xAxis: {
      type: "datetime",
      labels: {
        formatter: function () {
          return formatChartTickTime(this.value, tz);
        },
      },
    },
    yAxis: opts.yAxis ?? { title: { text: undefined } },
    tooltip: {
      shared: true,
      split: false,
      formatter: function () {
        const x = this.x as number;
        const header = formatChartTooltipTime(x, tz);
        let html = `<b>${header}</b><br/>`;
        this.points?.forEach((point) => {
          const seriesOpts = point.series.options as SeriesLineOptions & {
            custom?: { unit?: string; decimals?: number };
          };
          const unit = seriesOpts.custom?.unit ?? "";
          const decimals = seriesOpts.custom?.decimals;
          html += formatTooltipHtml(
            [{ x, y: point.y, color: String(point.color), seriesName: point.series.name, unit, decimals }],
            tz,
          ).replace(/^<b>.*?<\/b><br\/>/, ""); // strip duplicate header
        });
        return html;
      },
    },
    legend: { enabled: true, align: "center", verticalAlign: "bottom", layout: "horizontal" },
    navigator: { enabled: opts.navigator ?? false, height: 40, margin: 10 },
    rangeSelector: { enabled: false },
    scrollbar: { enabled: false },
    plotOptions: {
      series: { connectNulls: false, turboThreshold: 0 },
    },
    boost: { useGPUTranslations: true },
    series: opts.series,
    annotations: opts.annotations,
  });
}

// ---------------------------------------------------------------------------
// Stock chart (time series + stock tools)
// ---------------------------------------------------------------------------

export type StockChartFactoryOpts = TimeSeriesFactoryOpts & {
  stockTools?: boolean;
};

/** Stock chart — time series + navigator + Stock Tools toolbar */
export function createStockChartOptions(opts: StockChartFactoryOpts): Options {
  const base = createTimeSeriesOptions({
    ...opts,
    navigator: opts.navigator ?? true,
  });

  if (opts.stockTools) {
    (base as Options & { stockTools?: unknown }).stockTools = {
      gui: {
        enabled: true,
        buttons: [
          "indicators",
          "separator",
          "simpleShapes",
          "lines",
          "crookedLines",
          "measure",
          "advanced",
          "separator",
          "toggleAnnotations",
          "separator",
          "verticalLabels",
          "flags",
          "separator",
          "zoomChange",
          "fullScreen",
          "separator",
          "currentPriceIndicator",
          "saveChart",
        ],
      },
    };
    base.navigation = {
      ...base.navigation,
      bindings: {
        ...((base.navigation as Record<string, unknown>)?.bindings as Record<string, unknown>),
      },
    };
  } else {
    // Explicitly disable — Highcharts Stock shows the toolbar by default
    (base as Options & { stockTools?: unknown }).stockTools = {
      gui: { enabled: false },
    };
  }

  return base;
}

// ---------------------------------------------------------------------------
// Scatter
// ---------------------------------------------------------------------------

export type ScatterFactoryOpts = {
  series: SeriesOptionsType[];
  xAxisTitle?: string;
  yAxisTitle?: string;
  height?: number;
  zoom?: boolean;
};

/** Scatter plot — x/y axes, optional regression line */
export function createScatterOptions(opts: ScatterFactoryOpts): Options {
  const chart: Options["chart"] = {
    type: "scatter",
    height: opts.height ?? CHART_HEIGHTS.compact,
  };
  if (opts.zoom !== false) {
    chart.zooming = { type: "xy" };
  }
  return createBaseOptions({
    chart,
    title: { text: undefined },
    xAxis: {
      title: opts.xAxisTitle ? { text: opts.xAxisTitle, style: { color: "#6b7280", fontSize: "11px" } } : undefined,
      gridLineWidth: 1,
      gridLineColor: "#f3f4f6",
    },
    yAxis: {
      title: opts.yAxisTitle ? { text: opts.yAxisTitle, style: { color: "#6b7280", fontSize: "11px" } } : undefined,
      gridLineColor: "#f3f4f6",
    },
    legend: { enabled: false },
    navigator: { enabled: false },
    rangeSelector: { enabled: false },
    scrollbar: { enabled: false },
    series: opts.series,
  });
}

// ---------------------------------------------------------------------------
// Histogram / column
// ---------------------------------------------------------------------------

export type HistogramFactoryOpts = {
  series: (SeriesColumnOptions | SeriesOptionsType)[];
  timeZone?: string;
  xAxisTitle?: string;
  yAxisTitle?: string;
  height?: number;
  xType?: "datetime" | "category" | "linear";
};

/** Histogram / column chart */
export function createHistogramOptions(opts: HistogramFactoryOpts): Options {
  const tz = opts.timeZone;
  return createBaseOptions({
    chart: {
      type: "column",
      height: opts.height ?? CHART_HEIGHTS.compact,
    },
    title: { text: undefined },
    xAxis: {
      type: opts.xType ?? "linear",
      title: opts.xAxisTitle ? { text: opts.xAxisTitle, style: { color: "#6b7280", fontSize: "11px" } } : undefined,
      labels: tz
        ? {
            formatter: function () {
              return formatChartTickTime(this.value, tz);
            },
          }
        : undefined,
    },
    yAxis: {
      title: opts.yAxisTitle ? { text: opts.yAxisTitle, style: { color: "#6b7280", fontSize: "11px" } } : undefined,
    },
    legend: { enabled: false },
    navigator: { enabled: false },
    rangeSelector: { enabled: false },
    scrollbar: { enabled: false },
    series: opts.series,
  });
}

// ---------------------------------------------------------------------------
// Heatmap
// ---------------------------------------------------------------------------

export type HeatmapFactoryOpts = {
  series: SeriesHeatmapOptions[];
  xCategories?: string[];
  yCategories?: string[];
  colorAxisMin?: number;
  colorAxisMax?: number;
  height?: number;
  tooltip?: Options["tooltip"];
};

/** Heatmap — color axis, x/y categories or datetime */
export function createHeatmapOptions(opts: HeatmapFactoryOpts): Options {
  return createBaseOptions({
    chart: {
      type: "heatmap",
      height: opts.height ?? CHART_HEIGHTS.standard,
    },
    title: { text: undefined },
    xAxis: {
      categories: opts.xCategories,
      labels: { style: { fontSize: "10px" } },
    },
    yAxis: {
      categories: opts.yCategories,
      title: { text: undefined },
      labels: { style: { fontSize: "10px" } },
      reversed: true,
    },
    colorAxis: {
      min: opts.colorAxisMin ?? 0,
      max: opts.colorAxisMax,
      stops: [
        [0, "#eef2ff"],   // indigo-50
        [0.5, "#818cf8"], // indigo-400
        [1, "#312e81"],   // indigo-900
      ],
    },
    legend: {
      align: "right",
      layout: "vertical",
      margin: 0,
      verticalAlign: "top",
      y: 25,
      symbolHeight: 280,
    },
    tooltip: opts.tooltip ?? { enabled: true },
    navigator: { enabled: false },
    rangeSelector: { enabled: false },
    scrollbar: { enabled: false },
    series: opts.series as SeriesOptionsType[],
  });
}

// ---------------------------------------------------------------------------
// Sparkline
// ---------------------------------------------------------------------------

export type SparklineFactoryOpts = {
  data: Array<[number, number | null]>;
  color?: string;
  height?: number;
  fillOpacity?: number;
};

/** Sparkline — tiny inline chart, no axes/legend/tooltip chrome */
export function createSparklineOptions(opts: SparklineFactoryOpts): Options {
  const color = opts.color ?? CHART_PALETTE.series[0]!;
  return createBaseOptions({
    chart: {
      type: "area",
      height: opts.height ?? CHART_HEIGHTS.sparkline,
      margin: [0, 0, 0, 0],
      spacing: [0, 0, 0, 0],
      backgroundColor: "transparent",
    },
    title: { text: undefined },
    xAxis: {
      visible: false,
      type: "datetime",
    },
    yAxis: {
      visible: false,
    },
    legend: { enabled: false },
    tooltip: { enabled: false },
    navigator: { enabled: false },
    rangeSelector: { enabled: false },
    scrollbar: { enabled: false },
    plotOptions: {
      area: {
        lineWidth: 1.5,
        marker: { enabled: false },
        fillColor: {
          linearGradient: { x1: 0, y1: 0, x2: 0, y2: 1 },
          stops: [
            [0, color + "59"], // ~35% opacity
            [1, color + "00"],
          ],
        },
        states: { hover: { lineWidth: 1.5 } },
      },
    },
    series: [
      {
        type: "area",
        data: opts.data,
        color,
        connectNulls: false,
      },
    ],
  });
}

// ---------------------------------------------------------------------------
// Stock Tools mixin
// ---------------------------------------------------------------------------

export type StockToolsConfig = {
  buttons?: string[];
};

/** Add Stock Tools toolbar to any options object */
export function withStockTools(options: Options, config?: StockToolsConfig): Options {
  const buttons = config?.buttons ?? [
    "indicators",
    "separator",
    "simpleShapes",
    "lines",
    "crookedLines",
    "measure",
    "advanced",
    "separator",
    "toggleAnnotations",
    "separator",
    "verticalLabels",
    "flags",
    "separator",
    "zoomChange",
    "fullScreen",
    "separator",
    "currentPriceIndicator",
    "saveChart",
  ];
  return {
    ...options,
    stockTools: { gui: { enabled: true, buttons } },
  } as Options;
}
