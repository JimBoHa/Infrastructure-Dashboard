"use client";

import type {
  AnnotationsOptions,
  Options,
  SeriesLineOptions,
  XAxisPlotBandsOptions,
  XAxisPlotLinesOptions,
} from "highcharts";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Highcharts } from "./HighchartsProvider";
import { formatNumber } from "@/lib/format";
import type { TrendSeriesEntry } from "@/types/dashboard";
import type { EphemeralMarker } from "@/types/chartMarkers";
import type { ChartAnnotationPayload, ChartAnnotationRow } from "@/lib/api";
import { browserTimeZone, formatChartTooltipTime } from "@/lib/siteTime";
import type { SavitzkyGolayOptions } from "@/lib/savitzkyGolay";
import { savitzkyGolayFilter, validateSavitzkyGolayOptions } from "@/lib/savitzkyGolay";
import { CHART_PALETTE, seriesColor } from "@/lib/chartTokens";
import { createStockChartOptions } from "@/lib/chartFactories";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import InlineBanner from "@/components/InlineBanner";
import { HighchartsPanel, type HighchartsChartRef } from "@/components/charts/HighchartsPanel";

type TrendChartProps = {
  title?: string;
  description?: ReactNode;
  actions?: ReactNode;
  footer?: ReactNode;
  data: TrendSeriesEntry[];
  timeZone?: string;
  ephemeralMarkers?: EphemeralMarker[];
  persistentAnnotations?: AnnotationsOptions[];
  persistedBestFits?: PersistedBestFit[];
  onMarkerClick?: (markerId: string) => void;
  onCreatePersistentAnnotation?: (payload: ChartAnnotationPayload) => Promise<ChartAnnotationRow>;
  onUpdatePersistentAnnotation?: (
    annotationId: string,
    payload: Partial<ChartAnnotationPayload>,
  ) => Promise<ChartAnnotationRow>;
  onDeletePersistentAnnotation?: (annotationId: string) => Promise<void>;
  stacked?: boolean;
  independentAxes?: boolean;
  yDomain?: { min?: number; max?: number };
  navigator?: boolean;
  xPlotBands?: XAxisPlotBandsOptions[];
  xPlotLines?: XAxisPlotLinesOptions[];
  heightClassName?: string;
  heightPx?: number;
  savitzkyGolay?: SavitzkyGolayOptions & { enabled: boolean; xUnitLabel?: string };
  analysisTools?: boolean;
};

export type PersistedBestFit = {
  annotationId: string;
  sensorId: string;
  startMs: number;
  endMs: number;
};

type BindingToolKey =
  | "segment"
  | "horizontalLine"
  | "fibonacci"
  | "measureXY"
  | "measureX"
  | "labelAnnotation"
  | "arrowSegment"
  | "rectangleAnnotation";

type AnalysisTool = "none" | BindingToolKey | "best_fit" | "pan" | "eraser";

type HighchartsNavigationBindings = {
  selectedButtonElement?: HTMLElement | null;
  selectedButton?: unknown;
  currentUserDetails?: unknown;
  bindingsButtonClick: (button: HTMLElement, events: unknown, clickEvent: unknown) => void;
  deselectAnnotation?: () => void;
};

type HighchartsAnnotationInstance = {
  coll: "annotations";
  options: Record<string, unknown> & { id?: unknown };
  userOptions?: Record<string, unknown>;
};

type BestFitDefinition = {
  id: string;
  sensorId: string;
  startMs: number;
  endMs: number;
  annotationId?: string;
  dirty?: boolean;
  saving?: boolean;
  saveError?: string | null;
};

type LinearRegressionStats = {
  slopePerMs: number;
  xMean: number;
  yMean: number;
  r2: number | null;
  n: number;
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
const MIN_BEST_FIT_WINDOW_MS = 1_000;

function getHighchartsDefaultNavigationBindings(): Record<string, unknown> | undefined {
  const getOptions = (Highcharts as unknown as { getOptions?: () => unknown }).getOptions;
  if (typeof getOptions !== "function") return undefined;
  const options = getOptions() as { navigation?: { bindings?: unknown } } | undefined;
  const bindings = options?.navigation?.bindings;
  if (!bindings || typeof bindings !== "object") return undefined;
  return bindings as Record<string, unknown>;
}

function generateBestFitId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `best_fit_${crypto.randomUUID()}`;
  }
  return `best_fit_${Date.now()}_${Math.random().toString(16).slice(2)}`;
}

function computeLinearRegression(points: Array<{ x: number; y: number }>): LinearRegressionStats | null {
  const n = points.length;
  if (n < 2) return null;

  let sumX = 0;
  let sumY = 0;
  for (const pt of points) {
    sumX += pt.x;
    sumY += pt.y;
  }
  const xMean = sumX / n;
  const yMean = sumY / n;

  let sxx = 0;
  let sxy = 0;
  let syy = 0;
  for (const pt of points) {
    const dx = pt.x - xMean;
    const dy = pt.y - yMean;
    sxx += dx * dx;
    sxy += dx * dy;
    syy += dy * dy;
  }

  if (!Number.isFinite(sxx) || sxx <= 0) return null;
  const slopePerMs = sxy / sxx;
  if (!Number.isFinite(slopePerMs)) return null;

  const r2 = syy > 0 ? clamp((sxy * sxy) / (sxx * syy), 0, 1) : null;
  return { slopePerMs, xMean, yMean, r2, n };
}

function formatSigned(value: number, options: Intl.NumberFormatOptions): string {
  if (!Number.isFinite(value)) return "\u2014";
  const sign = value > 0 ? "+" : value < 0 ? "\u2212" : "";
  return `${sign}${formatNumber(Math.abs(value), options)}`;
}

function annotationErrorMessage(error: unknown): string {
  const status = (error as { status?: unknown } | null)?.status;
  if (status === 401 || status === 403) {
    return "You do not have permission to save chart annotations.";
  }
  if (error instanceof Error && error.message) return error.message;
  return "Failed to save annotation.";
}

function isBindingTool(tool: AnalysisTool): tool is BindingToolKey {
  return tool !== "none" && tool !== "best_fit" && tool !== "pan" && tool !== "eraser";
}

function isHighchartsAnnotationInstance(value: unknown): value is HighchartsAnnotationInstance {
  if (!value || typeof value !== "object") return false;
  const maybe = value as { coll?: unknown; options?: unknown };
  if (maybe.coll !== "annotations") return false;
  if (!maybe.options || typeof maybe.options !== "object") return false;
  return true;
}

function safeJsonCloneRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") return null;

  const seen = new WeakSet<object>();
  try {
    const json = JSON.stringify(value, (_key, next: unknown) => {
      if (typeof next === "function") return undefined;
      if (next && typeof next === "object") {
        const obj = next as object;
        if (seen.has(obj)) return undefined;
        seen.add(obj);
      }
      return next;
    });
    if (!json) return null;
    const parsed = JSON.parse(json) as unknown;
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : null;
  } catch {
    return null;
  }
}

function extractPersistableAnnotationOptions(annotation: HighchartsAnnotationInstance): Record<string, unknown> | null {
  const candidate = annotation.userOptions && typeof annotation.userOptions === "object" ? annotation.userOptions : annotation.options;
  return safeJsonCloneRecord(candidate);
}

function inferAnnotationTimeRangeMs(annotationOptions: unknown): { startMs?: number; endMs?: number } {
  if (!annotationOptions || typeof annotationOptions !== "object") return {};
  const options = annotationOptions as Record<string, unknown>;
  const xValues: number[] = [];

  const maybeAdd = (value: unknown) => {
    if (typeof value === "number" && Number.isFinite(value)) xValues.push(value);
  };

  const typeOptions = options.typeOptions as Record<string, unknown> | undefined;
  const typePoints = typeOptions?.points as Array<Record<string, unknown>> | undefined;
  if (Array.isArray(typePoints)) {
    for (const pt of typePoints) {
      maybeAdd(pt?.x);
    }
  }

  const shapes = options.shapes as Array<Record<string, unknown>> | undefined;
  if (Array.isArray(shapes)) {
    for (const shape of shapes) {
      const pts = shape.points as Array<Record<string, unknown>> | undefined;
      if (Array.isArray(pts)) {
        for (const pt of pts) {
          maybeAdd(pt?.x);
        }
      }
      const point = shape.point as Record<string, unknown> | undefined;
      if (point) maybeAdd(point.x);
    }
  }

  const labels = options.labels as Array<Record<string, unknown>> | undefined;
  if (Array.isArray(labels)) {
    for (const label of labels) {
      const point = label.point as Record<string, unknown> | undefined;
      if (point) maybeAdd(point.x);
    }
  }

  if (xValues.length === 0) return {};
  const startMs = Math.min(...xValues);
  const endMs = Math.max(...xValues);
  if (!Number.isFinite(startMs) || !Number.isFinite(endMs)) return {};
  return { startMs, endMs };
}

const BINDING_TOOL_LABELS: Record<BindingToolKey, string> = {
  segment: "Trendline",
  horizontalLine: "Horizontal line",
  fibonacci: "Fibonacci retracement",
  measureXY: "Measure XY",
  measureX: "Distance",
  labelAnnotation: "Label",
  arrowSegment: "Arrow",
  rectangleAnnotation: "Rectangle highlight",
};

export const TrendChart = ({
  title,
  description,
  actions,
  footer,
  data,
  timeZone,
  ephemeralMarkers,
  persistentAnnotations,
  persistedBestFits,
  onMarkerClick,
  onCreatePersistentAnnotation,
  onUpdatePersistentAnnotation,
  onDeletePersistentAnnotation,
  stacked = false,
  independentAxes = false,
  yDomain,
  navigator = true,
  xPlotBands,
  xPlotLines,
  heightClassName = "h-80",
  heightPx,
  savitzkyGolay,
  analysisTools = false,
}: TrendChartProps) => {
  const effectiveTimeZone = timeZone ?? browserTimeZone();
  const chartRef = useRef<HighchartsChartRef | null>(null);
  // Store user zoom state to preserve across re-renders
  const zoomStateRef = useRef<{ min: number; max: number } | null>(null);
  const hasHeader = Boolean(title || description || actions);
  const hasFooter = Boolean(footer);
  const savgolEnabled = Boolean(savitzkyGolay?.enabled);
  const defaultNavigationBindings = useMemo(() => getHighchartsDefaultNavigationBindings(), []);

  const [bestFitSensorId, setBestFitSensorId] = useState<string>("");
  const [bestFits, setBestFits] = useState<BestFitDefinition[]>([]);
  const [bestFitEditId, setBestFitEditId] = useState<string | null>(null);
  const bestFitsRef = useRef<BestFitDefinition[]>([]);
  const [autoSaveBestFits, setAutoSaveBestFits] = useState(false);
  const [bestFitError, setBestFitError] = useState<string | null>(null);
  const [activeTool, setActiveTool] = useState<AnalysisTool>("none");

  useEffect(() => {
    bestFitsRef.current = bestFits;
  }, [bestFits]);

  const activeToolRef = useRef<AnalysisTool>(activeTool);
  useEffect(() => {
    activeToolRef.current = activeTool;
  }, [activeTool]);

  const onCreatePersistentAnnotationRef = useRef(onCreatePersistentAnnotation);
  useEffect(() => {
    onCreatePersistentAnnotationRef.current = onCreatePersistentAnnotation;
  }, [onCreatePersistentAnnotation]);

  const onUpdatePersistentAnnotationRef = useRef(onUpdatePersistentAnnotation);
  useEffect(() => {
    onUpdatePersistentAnnotationRef.current = onUpdatePersistentAnnotation;
  }, [onUpdatePersistentAnnotation]);

  const onDeletePersistentAnnotationRef = useRef(onDeletePersistentAnnotation);
  useEffect(() => {
    onDeletePersistentAnnotationRef.current = onDeletePersistentAnnotation;
  }, [onDeletePersistentAnnotation]);

  const persistedAnnotationIdsRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    const next = new Set<string>();
    for (const entry of persistentAnnotations ?? []) {
      const id = (entry as { id?: unknown } | undefined)?.id;
      if (typeof id === "string" && id.trim().length > 0) next.add(id);
    }
    persistedAnnotationIdsRef.current = next;
  }, [persistentAnnotations]);

  useEffect(() => {
    if (!analysisTools) return;
    const incoming = persistedBestFits ?? [];
    const incomingIds = new Set(incoming.map((entry) => entry.annotationId));
    setBestFits((prev) => {
      const prevByAnnotationId = new Map(
        prev
          .filter((entry) => Boolean(entry.annotationId))
          .map((entry) => [entry.annotationId as string, entry]),
      );
      const next: BestFitDefinition[] = [];

      for (const entry of prev) {
        if (!entry.annotationId) next.push(entry);
      }

      for (const saved of incoming) {
        const existing = prevByAnnotationId.get(saved.annotationId);
        if (existing?.saving || existing?.dirty) {
          next.push(existing);
          continue;
        }
        next.push({
          id: existing?.id ?? `best_fit_saved_${saved.annotationId}`,
          sensorId: saved.sensorId,
          startMs: saved.startMs,
          endMs: saved.endMs,
          annotationId: saved.annotationId,
          dirty: false,
          saving: false,
          saveError: null,
        });
      }

      for (const entry of prev) {
        if (!entry.annotationId) continue;
        if (incomingIds.has(entry.annotationId)) continue;
        if (entry.saving || entry.dirty) {
          next.push(entry);
        }
      }

      return next;
    });
  }, [analysisTools, persistedBestFits]);

  // Convert ephemeral markers to Highcharts annotations (outlined/muted style)
  const ephemeralAnnotations: AnnotationsOptions[] = useMemo(() => {
    if (!ephemeralMarkers?.length) return [];

    return ephemeralMarkers.map((marker) => ({
      draggable: "",
      zIndex: 4,
      labels: [
        {
          point: { x: marker.timestamp.getTime(), y: 0, xAxis: 0, yAxis: 0 },
          text: marker.label,
          backgroundColor: "transparent",
          borderColor: CHART_PALETTE.reference,
          borderWidth: 1,
          style: {
            color: CHART_PALETTE.reference,
            fontSize: "10px",
            fontWeight: "500",
            fontStyle: "italic",
          },
          padding: 4,
          borderRadius: 4,
          y: -10,
        },
      ],
      shapes: [
        {
          type: "path",
          strokeWidth: 1,
          stroke: CHART_PALETTE.reference,
          dashStyle: "Dash",
          points: [
            { x: marker.timestamp.getTime(), y: 0, xAxis: 0, yAxis: 0 },
            { x: marker.timestamp.getTime(), y: 1, xAxis: 0, yAxis: 0 },
          ],
          point: { x: marker.timestamp.getTime(), y: 0, xAxis: 0, yAxis: 0 },
        },
      ],
      events: onMarkerClick
        ? {
            click: function () {
              onMarkerClick(marker.id);
            },
          }
        : undefined,
    } as AnnotationsOptions));
  }, [ephemeralMarkers, onMarkerClick]);

  // Merge annotations: ephemeral + persistent
  const allAnnotations: AnnotationsOptions[] = useMemo(() => {
    return [...ephemeralAnnotations, ...(persistentAnnotations ?? [])];
  }, [ephemeralAnnotations, persistentAnnotations]);

  const savgolValidation = useMemo(() => {
    if (!savgolEnabled || !savitzkyGolay) return { ok: true } as const;
    const { windowLength, polyOrder, derivOrder, delta, edgeMode } = savitzkyGolay;
    return validateSavitzkyGolayOptions({
      windowLength,
      polyOrder,
      derivOrder,
      delta,
      edgeMode,
    });
  }, [savgolEnabled, savitzkyGolay]);

  const filteredValues = useMemo(() => {
    if (!savgolEnabled || !savitzkyGolay || !savgolValidation.ok) return null;
    const { windowLength, polyOrder, derivOrder, delta, edgeMode } = savitzkyGolay;
    const options: SavitzkyGolayOptions = {
      windowLength,
      polyOrder,
      derivOrder,
      delta,
      edgeMode,
    };
    return data.map((series) => savitzkyGolayFilter(series.points.map((pt) => pt.value), options));
  }, [data, savgolEnabled, savgolValidation.ok, savitzkyGolay]);

  const derivedUnitLabel = useCallback(
    (unit: string | undefined) => {
      if (!savgolEnabled || !savitzkyGolay || !savgolValidation.ok) return unit;
      const derivOrder = Math.floor(savitzkyGolay.derivOrder ?? 0);
      if (!unit || derivOrder <= 0) return unit;
      const xUnit = savitzkyGolay.xUnitLabel ?? "s";
      return derivOrder === 1 ? `${unit}/${xUnit}` : `${unit}/${xUnit}^${derivOrder}`;
    },
    [savgolEnabled, savgolValidation.ok, savitzkyGolay],
  );

  const formatDatasetLabel = useCallback(
    (baseLabel: string, unit: string | undefined) => {
      const nextUnit = derivedUnitLabel(unit);
      if (!nextUnit || nextUnit === unit) return baseLabel;
      const suffix = unit ? ` (${unit})` : "";
      const replacement = ` (${nextUnit})`;
      if (suffix && baseLabel.endsWith(suffix)) {
        return `${baseLabel.slice(0, -suffix.length)}${replacement}`;
      }
      return `${baseLabel}${replacement}`;
    },
    [derivedUnitLabel],
  );

  // Build series from data
  const series: SeriesLineOptions[] = useMemo(() => {
    return data.map((entry, index) => {
      const baseLabel = entry.label ?? entry.sensor_id;
      const unit = derivedUnitLabel(entry.unit);
      const label = `${formatDatasetLabel(baseLabel, entry.unit)}${
        independentAxes ? ` (${index % 2 === 0 ? "L" : "R"})` : ""
      }`;
      const seriesValues = filteredValues?.[index] ?? entry.points.map((pt) => pt.value);
      const color = seriesColor(index);

      // Count non-null points to determine if sparse
      let nonNullPoints = 0;
      for (const pt of entry.points) {
        if (typeof pt.value === "number" && Number.isFinite(pt.value)) {
          nonNullPoints += 1;
          if (nonNullPoints > 1) break;
        }
      }
      const sparse = nonNullPoints <= 1;

      return {
        type: "line" as const,
        name: label,
        data: entry.points.map((pt, ptIndex) => [
          pt.timestamp.getTime(),
          seriesValues[ptIndex] ?? null,
        ]),
        color,
        lineWidth: 1.5,
        marker: {
          enabled: sparse,
          radius: sparse ? 3 : 0,
          symbol: "circle",
        },
        states: {
          hover: {
            lineWidth: 2,
          },
        },
        yAxis: independentAxes ? index : 0,
        stacking: stacked ? "normal" : undefined,
        tooltip: {
          valueSuffix: unit ? ` ${unit}` : undefined,
          valueDecimals: entry.display_decimals ?? 2,
        },
        custom: {
          unit,
          decimals: entry.display_decimals,
        },
      } as SeriesLineOptions;
    });
  }, [data, derivedUnitLabel, filteredValues, formatDatasetLabel, independentAxes, stacked]);

  const bestFitSeriesOptions = useMemo(() => {
    if (!analysisTools || bestFits.length === 0 || series.length === 0) {
      return { overlays: [] as SeriesLineOptions[], rows: [] as Array<{ id: string; message: string }> };
    }

    const sensorIndexById = new Map(data.map((entry, index) => [entry.sensor_id, index]));
    const overlays: SeriesLineOptions[] = [];
    const rows: Array<{ id: string; message: string }> = [];

    for (const fit of bestFits) {
      const seriesIndex = sensorIndexById.get(fit.sensorId);
      if (seriesIndex == null) continue;
      const seriesEntry = data[seriesIndex];
      const seriesOpts = series[seriesIndex];
      const hcData = (seriesOpts.data as Array<[number, number | null]> | undefined) ?? [];

      const startMs = Math.min(fit.startMs, fit.endMs);
      const endMs = Math.max(fit.startMs, fit.endMs);
      const points: Array<{ x: number; y: number }> = [];
      for (const [x, y] of hcData) {
        if (x < startMs || x > endMs) continue;
        if (typeof y !== "number" || !Number.isFinite(y)) continue;
        points.push({ x, y });
      }

      const stats = computeLinearRegression(points);
      if (!stats) {
        const label = seriesEntry?.label ?? seriesEntry?.sensor_id ?? fit.sensorId;
        rows.push({
          id: fit.id,
          message: `Best fit: "${label}" needs at least 2 points inside the selected window.`,
        });
        continue;
      }

      const yStart = stats.yMean + stats.slopePerMs * (startMs - stats.xMean);
      const yEnd = stats.yMean + stats.slopePerMs * (endMs - stats.xMean);
      if (!Number.isFinite(yStart) || !Number.isFinite(yEnd)) {
        continue;
      }

      const axisIndex = independentAxes ? seriesIndex : 0;
      const baseLabel = seriesEntry?.label ?? seriesEntry?.sensor_id ?? fit.sensorId;
      const overlayLabel = `Best fit — ${baseLabel}`;
      const color = seriesColor(seriesIndex);
      overlays.push({
        type: "line" as const,
        name: overlayLabel,
        data: [
          [startMs, yStart],
          [endMs, yEnd],
        ],
        color,
        dashStyle: "Dash",
        lineWidth: 2.5,
        marker: { enabled: false },
        enableMouseTracking: false,
        yAxis: axisIndex,
        stacking: undefined,
        zIndex: 6,
        states: { hover: { enabled: false } },
      } as SeriesLineOptions);

      const unit = (seriesOpts.custom as { unit?: string } | undefined)?.unit;
      const decimals = (seriesOpts.custom as { decimals?: number } | undefined)?.decimals;
      const decimalsSafe = typeof decimals === "number" && Number.isFinite(decimals) ? decimals : 2;

      const durationHours = (endMs - startMs) / 3_600_000;
      const delta = yEnd - yStart;
      const perHour = durationHours > 0 ? delta / durationHours : Number.NaN;
      const r2Label = stats.r2 != null ? formatNumber(stats.r2, { minimumFractionDigits: 2, maximumFractionDigits: 2 }) : "\u2014";

      const summary = [
        `${formatChartTooltipTime(startMs, effectiveTimeZone)} \u2192 ${formatChartTooltipTime(endMs, effectiveTimeZone)}`,
        `n=${stats.n}`,
        `R\u00b2=${r2Label}`,
        `\u0394=${formatSigned(delta, { minimumFractionDigits: 0, maximumFractionDigits: decimalsSafe })}${unit ? ` ${unit}` : ""}`,
        `rate=${formatSigned(perHour, { minimumFractionDigits: 0, maximumFractionDigits: decimalsSafe })}${
          unit ? ` ${unit}` : ""
        }/h`,
      ].join(" \u00b7 ");
      rows.push({ id: fit.id, message: summary });
    }

    return { overlays, rows };
  }, [analysisTools, bestFits, data, effectiveTimeZone, independentAxes, series]);

  const chartSeries = useMemo(() => {
    if (stacked) return series;
    return bestFitSeriesOptions.overlays.length > 0 ? [...series, ...bestFitSeriesOptions.overlays] : series;
  }, [bestFitSeriesOptions.overlays, series, stacked]);

  useEffect(() => {
    if (!analysisTools) return;

    if (bestFitSensorId && data.some((entry) => entry.sensor_id === bestFitSensorId)) return;
    const first = data[0]?.sensor_id ?? "";
    setBestFitSensorId(first);
  }, [analysisTools, bestFitSensorId, data]);

  useEffect(() => {
    if (!analysisTools) return;

    const allowed = new Set(data.map((entry) => entry.sensor_id));
    setBestFits((prev) => prev.filter((fit) => allowed.has(fit.sensorId)));
  }, [analysisTools, data]);

  const cancelHighchartsBindingTool = useCallback(() => {
    const chart = chartRef.current?.chart as unknown as { navigationBindings?: HighchartsNavigationBindings } | undefined;
    const navigation = chart?.navigationBindings;
    if (!navigation?.selectedButtonElement || !navigation?.selectedButton) return;

    try {
      navigation.bindingsButtonClick(
        navigation.selectedButtonElement,
        navigation.selectedButton,
        new MouseEvent("click"),
      );
    } catch {
      // Non-critical: best-effort cancellation.
    }
  }, []);

  const setPanModeEnabled = useCallback((enabled: boolean) => {
    const chart = chartRef.current?.chart;
    if (!chart) return;

    // In Highcharts, panning generally requires holding panKey when zooming is enabled.
    // We treat "Pan mode" as a dedicated navigation state: drag to pan, not drag-to-zoom.
    chart.update(
      {
        chart: enabled
          ? {
              zooming: {},
              panning: { enabled: true, type: "x" },
              panKey: undefined,
            }
          : {
              zooming: { type: "x", mouseWheel: { enabled: true } },
              panning: { enabled: true, type: "x" },
              panKey: "shift",
            },
      },
      true,
      false,
      false,
    );

    try {
      chart.container.style.cursor = enabled ? "grab" : "";
    } catch {
      // ignore
    }
  }, []);

  const beginBestFit = useCallback(
    (mode: "new" | "edit", editId?: string) => {
      if (!analysisTools || data.length === 0) return;
      if (stacked) {
        setBestFitError("Best fit is disabled while Stack is enabled. Turn off Stack to compute a meaningful fit.");
        return;
      }

      cancelHighchartsBindingTool();
      if (activeToolRef.current === "pan") setPanModeEnabled(false);
      setActiveTool("best_fit");

      setBestFitError(null);
      setBestFitEditId(mode === "edit" ? editId ?? null : null);
    },
    [analysisTools, cancelHighchartsBindingTool, data.length, setActiveTool, setPanModeEnabled, stacked],
  );

  const cancelBestFit = useCallback(() => {
    setBestFitEditId(null);
    setBestFitError(null);
    if (activeToolRef.current === "best_fit") setActiveTool("none");
  }, [setActiveTool]);

  const persistBestFit = useCallback(
    async (fitId: string) => {
      const fit = bestFitsRef.current.find((entry) => entry.id === fitId);
      const create = onCreatePersistentAnnotationRef.current;
      const update = onUpdatePersistentAnnotationRef.current;
      if (!fit || !create) return;

      const seriesEntry = data.find((entry) => entry.sensor_id === fit.sensorId);
      const label = seriesEntry?.label ?? fit.sensorId;
      const payload: ChartAnnotationPayload = {
        chart_state: {
          type: "best_fit_v1",
          v: 1,
          sensor_id: fit.sensorId,
          start_ms: fit.startMs,
          end_ms: fit.endMs,
        },
        sensor_ids: [fit.sensorId],
        time_start: new Date(fit.startMs).toISOString(),
        time_end: new Date(fit.endMs).toISOString(),
        label: `Best fit — ${label}`,
      };

      setBestFits((prev) =>
        prev.map((entry) =>
          entry.id === fitId
            ? {
                ...entry,
                saving: true,
                saveError: null,
              }
            : entry,
        ),
      );
      setBestFitError(null);

      try {
        const row =
          fit.annotationId && update
            ? await update(fit.annotationId, payload)
            : await create(payload);
        setBestFits((prev) =>
          prev.map((entry) =>
            entry.id === fitId
              ? {
                  ...entry,
                  annotationId: row.id,
                  dirty: false,
                  saving: false,
                  saveError: null,
                }
              : entry,
          ),
        );
      } catch (error) {
        const message = annotationErrorMessage(error);
        setBestFits((prev) =>
          prev.map((entry) =>
            entry.id === fitId
              ? {
                  ...entry,
                  saving: false,
                  saveError: message,
                }
              : entry,
          ),
        );
        setBestFitError(message);
      }
    },
    [data],
  );

  const removeBestFit = useCallback(async (fitId: string) => {
    const fit = bestFitsRef.current.find((entry) => entry.id === fitId);
    if (!fit) return;
    if (fit.annotationId) {
      const del = onDeletePersistentAnnotationRef.current;
      if (del) {
        try {
          await del(fit.annotationId);
        } catch (error) {
          const message = annotationErrorMessage(error);
          setBestFits((prev) =>
            prev.map((entry) =>
              entry.id === fitId
                ? {
                    ...entry,
                    saveError: message,
                  }
                : entry,
            ),
          );
          setBestFitError(message);
          return;
        }
      }
    }
    setBestFits((prev) => prev.filter((entry) => entry.id !== fitId));
  }, []);

  const clearBestFits = useCallback(async () => {
    const snapshot = bestFitsRef.current;
    let failures = 0;
    const del = onDeletePersistentAnnotationRef.current;
    if (del) {
      for (const fit of snapshot) {
        if (!fit.annotationId) continue;
        try {
          await del(fit.annotationId);
        } catch {
          failures += 1;
        }
      }
    }
    setBestFits([]);
    if (failures > 0) {
      setBestFitError(`Failed to delete ${failures} saved best-fit annotation(s).`);
    }
  }, []);

  useEffect(() => {
    if (!analysisTools) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;

      const tool = activeToolRef.current;
      if (tool === "best_fit") {
        cancelBestFit();
        return;
      }
      if (tool === "pan") {
        setPanModeEnabled(false);
        setActiveTool("none");
        return;
      }
      if (tool === "eraser") {
        setActiveTool("none");
        return;
      }
      if (tool !== "none") {
        cancelHighchartsBindingTool();
        setActiveTool("none");
        return;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [analysisTools, cancelBestFit, cancelHighchartsBindingTool, setActiveTool, setPanModeEnabled]);

  useEffect(() => {
    if (!analysisTools) return;
    if (!stacked) return;
    if (activeTool !== "best_fit") return;
    cancelBestFit();
  }, [activeTool, analysisTools, cancelBestFit, stacked]);

  // Build y-axes configuration
  const yAxis = useMemo(() => {
    if (!independentAxes) {
      return [
        {
          title: { text: undefined },
          min: yDomain?.min,
          max: yDomain?.max,
          opposite: false,
        },
      ];
    }

    return data.map((_, index) => ({
      title: { text: undefined },
      min: yDomain?.min,
      max: yDomain?.max,
      opposite: index % 2 !== 0,
      gridLineWidth: index === 0 ? 1 : 0,
      labels: {
        style: { color: seriesColor(index) },
      },
    }));
  }, [data, independentAxes, yDomain]);

  // Resolve the chart pixel height for Highcharts (must match the CSS container).
  // h-80 = 320px via Tailwind; heightPx overrides when set.
  const resolvedHeightPx =
    typeof heightPx === "number" && Number.isFinite(heightPx)
      ? Math.max(160, Math.round(heightPx))
      : 320;

  const chartOptions: Options = useMemo(() => {
    const base = createStockChartOptions({
      series: chartSeries,
      timeZone: effectiveTimeZone,
      height: resolvedHeightPx,
      navigator,
      yAxis,
      annotations: allAnnotations.length > 0 ? allAnnotations : undefined,
    });

    // Merge TrendChart-specific overrides on top of factory base
    const mergedPlotLines = xPlotLines && xPlotLines.length > 0 ? [...xPlotLines] : undefined;

    const opts: Options = {
      ...base,
      ...(analysisTools
        ? {
            navigation: {
              ...((base.navigation as Record<string, unknown> | undefined) ?? {}),
              // Highcharts only instantiates `chart.navigationBindings` when
              // `navigation.bindings` exists on the chart options. We intentionally
              // disable the stock-tools GUI, but still need the default bindings
              // so our custom toolbar can arm tools.
              bindings: {
                ...(defaultNavigationBindings ?? {}),
              },
              events: {
                deselectButton: () => {
                  const tool = activeToolRef.current;
                  if (isBindingTool(tool)) {
                    setActiveTool("none");
                  }

                  const create = onCreatePersistentAnnotationRef.current;
                  if (!create || !isBindingTool(tool)) return;

                  const chartImmediate = chartRef.current?.chart as unknown as {
                    navigationBindings?: HighchartsNavigationBindings;
                  } | undefined;
                  const inProgress = chartImmediate?.navigationBindings?.currentUserDetails;

                  // Highcharts fires `deselectButton` before the binding's `end()` handler
                  // runs and before it cleans up in-progress annotations. Defer one tick so
                  // we can inspect the finalized `chart.annotations` list.
                  setTimeout(() => {
                    const chartNow = chartRef.current?.chart as unknown as {
                      annotations?: unknown[];
                      removeAnnotation?: (idOrAnnotation: unknown) => void;
                      addAnnotation?: (options: unknown) => unknown;
                    } | undefined;
                    const liveAnnotations = chartNow?.annotations ?? [];
                    const hasInProgress = isHighchartsAnnotationInstance(inProgress);
                    if (!chartNow || (liveAnnotations.length === 0 && !hasInProgress)) return;

                    const persistedIds = persistedAnnotationIdsRef.current;
                    const combined = hasInProgress ? [...liveAnnotations, inProgress] : liveAnnotations;
                    const deduped = combined.filter((entry, index) => combined.indexOf(entry) === index);
                    const candidates = deduped
                      .filter((entry): entry is HighchartsAnnotationInstance =>
                        isHighchartsAnnotationInstance(entry),
                      )
                      .filter((annotation) => {
                        const id = annotation.options?.id;
                        if (typeof id === "string") {
                          if (id.startsWith("ephemeral_marker_")) return false;
                          if (persistedIds.has(id)) return false;
                        }
                        return true;
                      });

                    void (async () => {
                      for (const annotation of candidates) {
                        try {
                          const safeOptions = extractPersistableAnnotationOptions(annotation);
                          if (!safeOptions) continue;
                          const { startMs, endMs } = inferAnnotationTimeRangeMs(safeOptions);

                          const payload: ChartAnnotationPayload = {
                            chart_state: {
                              type: "highcharts_annotation",
                              v: 1,
                              options: safeOptions,
                            },
                            sensor_ids: data.map((entry) => entry.sensor_id),
                            time_start:
                              typeof startMs === "number" ? new Date(startMs).toISOString() : undefined,
                            time_end: typeof endMs === "number" ? new Date(endMs).toISOString() : undefined,
                          };

                          const row = await create(payload);
                          persistedAnnotationIdsRef.current = new Set([
                            ...persistedAnnotationIdsRef.current,
                            row.id,
                          ]);

                          try {
                            chartNow.removeAnnotation?.(annotation);
                          } catch {
                            // ignore removal failures
                          }

                          chartNow.addAnnotation?.({
                            ...(safeOptions as Record<string, unknown>),
                            id: row.id,
                          });
                        } catch {
                          // Non-critical: annotation save is best-effort.
                        }
                      }
                    })();
                  }, 0);
                },
                showPopup: function (event: unknown) {
                  if (activeToolRef.current !== "eraser") return;

                  const annotation = (event as { annotation?: unknown } | undefined)?.annotation;
                  if (!isHighchartsAnnotationInstance(annotation)) return;

                  const id = annotation.options?.id;
                  const del = onDeletePersistentAnnotationRef.current;
                  if (typeof id === "string" && id && del) {
                    void del(id);
                  }

                  try {
                    const chart = chartRef.current?.chart as unknown as {
                      removeAnnotation?: (idOrAnnotation: unknown) => void;
                    } | undefined;
                    chart?.removeAnnotation?.(typeof id === "string" && id ? id : annotation);
                  } catch {
                    // ignore removal failures
                  }

                  try {
                    (this as unknown as { deselectAnnotation?: () => void }).deselectAnnotation?.();
                  } catch {
                    // ignore
                  }
                },
              },
            },
          }
        : {}),
      chart: {
        ...(base.chart as Record<string, unknown>),
        events: {
          ...(((base.chart as Record<string, unknown>)?.events as Record<string, unknown>) ?? {}),
          selection: function (event) {
            if (!analysisTools || activeToolRef.current !== "best_fit") return true;
            const selection = (event as { xAxis?: Array<{ min?: number; max?: number }> }).xAxis?.[0];
            const min = selection?.min;
            const max = selection?.max;
            if (typeof min !== "number" || typeof max !== "number") {
              setBestFitError("Drag across the chart to choose a best-fit window.");
              return false;
            }

            const startMs = Math.min(min, max);
            const endMs = Math.max(min, max);
            if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs - startMs < MIN_BEST_FIT_WINDOW_MS) {
              setBestFitError("Select a wider time range (at least 1 second) for best fit.");
              return false;
            }

            const id = bestFitEditId ?? generateBestFitId();
            let shouldPersist = false;
            setBestFits((prev) => {
              const existing = prev.find((entry) => entry.id === id);
              shouldPersist = autoSaveBestFits || Boolean(existing?.annotationId);
              const nextDef: BestFitDefinition = {
                id,
                sensorId: bestFitSensorId,
                startMs,
                endMs,
                annotationId: existing?.annotationId,
                dirty: Boolean(existing?.annotationId),
                saving: false,
                saveError: null,
              };
              const existingIdx = prev.findIndex((entry) => entry.id === id);
              if (existingIdx >= 0) {
                const copy = [...prev];
                copy[existingIdx] = nextDef;
                return copy;
              }
              return [...prev, nextDef];
            });

            setBestFitEditId(null);
            setBestFitError(null);
            setActiveTool("none");
            if (shouldPersist) {
              void persistBestFit(id);
            }
            return false;
          },
        },
      },
      xAxis: {
        ...(base.xAxis as Record<string, unknown>),
        crosshair:
          analysisTools && activeTool === "best_fit"
            ? { color: CHART_PALETTE.reference, width: 1, dashStyle: "Dash" }
            : undefined,
        plotLines: mergedPlotLines,
        plotBands: xPlotBands,
        events: {
          // Capture user zoom to preserve across re-renders
          afterSetExtremes: function (e) {
            if (e.trigger === "zoom" || e.trigger === "navigator") {
              zoomStateRef.current = { min: e.min, max: e.max };
            } else if (e.trigger === "rangeSelectorButton" || e.trigger === undefined) {
              if (e.min === e.dataMin && e.max === e.dataMax) {
                zoomStateRef.current = null;
              }
            }
          },
        },
      },
      tooltip: {
        shared: true,
        split: false,
        xDateFormat: "%Y-%m-%d %H:%M:%S",
        formatter: function () {
          const x = this.x as number;
          const header = formatChartTooltipTime(x, effectiveTimeZone);
          let html = `<b>${header}</b><br/>`;

          this.points?.forEach((point) => {
            const seriesOpts = point.series.options as SeriesLineOptions & {
              custom?: { unit?: string; decimals?: number };
            };
            const unit = seriesOpts.custom?.unit ?? "";
            const decimals = seriesOpts.custom?.decimals;
            const rawY = point.y;
            const value =
              typeof rawY === "number" && Number.isFinite(rawY)
                ? decimals != null
                  ? formatNumber(rawY, {
                      minimumFractionDigits: decimals,
                      maximumFractionDigits: decimals,
                    })
                  : formatNumber(rawY, { minimumFractionDigits: 0, maximumFractionDigits: 2 })
                : "\u2014";
            const suffix = unit && !point.series.name.includes(unit) ? ` ${unit}` : "";
            html += `<span style="color:${point.color}">\u25CF</span> ${point.series.name}: <b>${value}${suffix}</b><br/>`;
          });

          return html;
        },
      },
    };

    return opts;
  }, [
    activeTool,
    allAnnotations,
    analysisTools,
    autoSaveBestFits,
    bestFitEditId,
    bestFitSensorId,
    chartSeries,
    data,
    defaultNavigationBindings,
    effectiveTimeZone,
    navigator,
    persistBestFit,
    resolvedHeightPx,
    xPlotBands,
    xPlotLines,
    yAxis,
  ]);

  const resetZoom = useCallback(() => {
    zoomStateRef.current = null;
    chartRef.current?.chart?.zoomOut();
  }, []);

  const zoomByFactor = useCallback((factor: number) => {
    const chart = chartRef.current?.chart;
    const xAxis = chart?.xAxis?.[0];
    if (!xAxis) return;

    const extremes = xAxis.getExtremes();
    const dataMin = extremes.dataMin;
    const dataMax = extremes.dataMax;
    const min = extremes.min ?? dataMin;
    const max = extremes.max ?? dataMax;
    if (typeof min !== "number" || typeof max !== "number") return;
    if (!Number.isFinite(min) || !Number.isFinite(max)) return;
    const range = max - min;
    if (!(range > 0)) return;

    const span = typeof dataMin === "number" && typeof dataMax === "number" ? dataMax - dataMin : null;
    const nextRange = range * factor;
    if (span != null && Number.isFinite(span) && nextRange >= span) {
      resetZoom();
      return;
    }

    const center = min + range / 2;
    let nextMin = center - nextRange / 2;
    let nextMax = center + nextRange / 2;

    if (typeof dataMin === "number" && Number.isFinite(dataMin) && nextMin < dataMin) {
      nextMin = dataMin;
      nextMax = nextMin + nextRange;
    }
    if (typeof dataMax === "number" && Number.isFinite(dataMax) && nextMax > dataMax) {
      nextMax = dataMax;
      nextMin = nextMax - nextRange;
    }

    if (typeof dataMin === "number" && Number.isFinite(dataMin)) nextMin = Math.max(dataMin, nextMin);
    if (typeof dataMax === "number" && Number.isFinite(dataMax)) nextMax = Math.min(dataMax, nextMax);
    if (!Number.isFinite(nextMin) || !Number.isFinite(nextMax) || nextMax <= nextMin) return;

    xAxis.setExtremes(nextMin, nextMax, true, false, { trigger: "zoom" });
  }, [resetZoom]);

  const handleBindingToolClick = useCallback(
    (bindingKey: BindingToolKey, event: React.MouseEvent<HTMLButtonElement>) => {
      const chart = chartRef.current?.chart as unknown as {
        navigationBindings?: HighchartsNavigationBindings;
        options?: { navigation?: { bindings?: Record<string, unknown> } };
      } | undefined;
      const navigation = chart?.navigationBindings;
      const binding = chart?.options?.navigation?.bindings?.[bindingKey] ?? defaultNavigationBindings?.[bindingKey];
      if (!analysisTools || !navigation || !binding) return;

      if (activeToolRef.current === "best_fit") cancelBestFit();
      if (activeToolRef.current === "pan") setPanModeEnabled(false);
      if (activeToolRef.current === "eraser") setActiveTool("none");

      navigation.bindingsButtonClick(event.currentTarget, binding, event.nativeEvent);
      setActiveTool(navigation.selectedButtonElement ? bindingKey : "none");
    },
    [analysisTools, cancelBestFit, defaultNavigationBindings, setActiveTool, setPanModeEnabled],
  );

  const toggleEraser = useCallback(() => {
    if (!analysisTools) return;

    if (activeToolRef.current === "eraser") {
      setActiveTool("none");
      return;
    }

    cancelBestFit();
    cancelHighchartsBindingTool();
    setPanModeEnabled(false);
    setActiveTool("eraser");
  }, [analysisTools, cancelBestFit, cancelHighchartsBindingTool, setActiveTool, setPanModeEnabled]);

  const togglePan = useCallback(() => {
    if (!analysisTools) return;

    if (activeToolRef.current === "pan") {
      setPanModeEnabled(false);
      setActiveTool("none");
      return;
    }

    cancelBestFit();
    cancelHighchartsBindingTool();
    setActiveTool("pan");
    setPanModeEnabled(true);
  }, [analysisTools, cancelBestFit, cancelHighchartsBindingTool, setActiveTool, setPanModeEnabled]);

  const clearAllAnnotations = useCallback(() => {
    if (!analysisTools) return;

    cancelBestFit();
    cancelHighchartsBindingTool();
    setPanModeEnabled(false);
    setActiveTool("none");

    const fitAnnotationIds = bestFitsRef.current
      .map((fit) => fit.annotationId)
      .filter((id): id is string => typeof id === "string" && id.trim().length > 0);
    const del = onDeletePersistentAnnotationRef.current;
    for (const id of fitAnnotationIds) {
      if (del) void del(id);
    }
    setBestFits([]);

    const chart = chartRef.current?.chart as unknown as {
      annotations?: Array<{ options?: Record<string, unknown> }>;
      removeAnnotation?: (idOrAnnotation: unknown) => void;
    } | undefined;
    const ids =
      chart?.annotations
        ?.map((annotation) => annotation?.options?.id)
        .filter((id): id is string => typeof id === "string" && id.trim().length > 0) ?? [];

    for (const id of ids) {
      try {
        chart?.removeAnnotation?.(id);
      } catch {
        // ignore removal failures
      }
      if (del) void del(id);
    }
  }, [analysisTools, cancelBestFit, cancelHighchartsBindingTool, setActiveTool, setPanModeEnabled]);

  const handleDoubleClick = useCallback(() => {
    if (analysisTools && activeTool !== "none") return;
    resetZoom();
  }, [activeTool, analysisTools, resetZoom]);

  // Restore zoom state after chart updates (e.g., when data changes)
  useEffect(() => {
    const chart = chartRef.current?.chart;
    if (!chart || !zoomStateRef.current) return;

    const { min, max } = zoomStateRef.current;
    const xAxis = chart.xAxis?.[0];
    if (!xAxis) return;

    // Only restore if the stored range is within the new data bounds
    // Cast to access dataMin/dataMax which exist at runtime but aren't in base types
    const axisWithData = xAxis as typeof xAxis & { dataMin?: number; dataMax?: number };
    const dataMin = axisWithData.dataMin ?? 0;
    const dataMax = axisWithData.dataMax ?? 0;
    if (min >= dataMin && max <= dataMax) {
      // Use setTimeout to ensure Highcharts has finished its update cycle
      const timeoutId = setTimeout(() => {
        xAxis.setExtremes(min, max, true, false);
      }, 0);
      return () => clearTimeout(timeoutId);
    }
  }, [chartOptions]);

  const header = hasHeader ? (
    <div className="mb-3 flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
      <div className="min-w-0">
        {title ? (
          <p className="text-sm font-semibold text-card-foreground">{title}</p>
        ) : null}
        {description ? (
 <p className="mt-1 text-xs text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {actions ? <div className="flex flex-wrap items-center gap-2">{actions}</div> : null}
    </div>
  ) : null;

  if (!data.length) {
    if (hasHeader || hasFooter) {
      return (
        <Card className="gap-3 p-4">
          {header}
          <Card className="gap-0 border-dashed py-6 text-center text-sm text-muted-foreground">
            Select sensors to visualise their trends.
          </Card>
          {footer ? <div className="mt-3">{footer}</div> : null}
        </Card>
      );
    }
    return (
      <Card className="gap-0 border-dashed py-6 text-center text-sm text-muted-foreground">
        Select sensors to visualise their trends.
      </Card>
    );
  }

  const heightStyle =
    typeof heightPx === "number" && Number.isFinite(heightPx)
      ? { height: `${Math.max(160, Math.round(heightPx))}px` }
      : undefined;
  const effectiveHeightClassName = heightStyle ? "" : heightClassName;

  return (
    <Card className="min-w-0 gap-3 p-4">
      {header}
      {analysisTools ? (
        <Card className="gap-0 bg-card-inset p-3 shadow-sm">
          <div className="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
            <div className="min-w-0">
              <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Chart analysis
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                Run best-fit windows, draw annotations, and measure directly on the chart. Only one tool is active at a
                time; press <span className="font-semibold">Esc</span> to cancel.
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <span className="rounded-full border border-border bg-white px-3 py-1 text-xs font-semibold text-muted-foreground">
                Active tool: {activeTool === "none" ? "None" : activeTool === "best_fit" ? "Best fit" : BINDING_TOOL_LABELS[activeTool as BindingToolKey] ?? activeTool}
              </span>
              <Button type="button" size="xs" variant="danger" onClick={clearAllAnnotations}>
                Clear all annotations
              </Button>
            </div>
          </div>

          <div className="mt-3 space-y-3">
            <div className="rounded-xl border border-border bg-white p-2 shadow-xs">
              <p className="px-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Primary analysis
              </p>
              <div className="flex flex-wrap items-center gap-1">
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "best_fit" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={() => (activeTool === "best_fit" ? cancelBestFit() : beginBestFit("new"))}
                  disabled={data.length === 0 || stacked || !bestFitSensorId}
                >
                  Best fit
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "segment" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("segment", event)}
                >
                  Trendline
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "horizontalLine" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("horizontalLine", event)}
                >
                  H-line
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "measureXY" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("measureXY", event)}
                >
                  Measure XY
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "measureX" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("measureX", event)}
                >
                  Distance
                </Button>
              </div>
            </div>

            <div className="hidden rounded-xl border border-border bg-white p-2 shadow-xs lg:block">
              <p className="px-2 pb-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Secondary tools
              </p>
              <div className="flex flex-wrap items-center gap-1">
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "labelAnnotation" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("labelAnnotation", event)}
                >
                  Label
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "arrowSegment" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("arrowSegment", event)}
                >
                  Arrow
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "rectangleAnnotation" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("rectangleAnnotation", event)}
                >
                  Rectangle
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "fibonacci" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("fibonacci", event)}
                >
                  Fibonacci
                </Button>
                <Button type="button" size="xs" variant="secondary" onClick={() => zoomByFactor(0.75)}>
                  Zoom +
                </Button>
                <Button type="button" size="xs" variant="secondary" onClick={() => zoomByFactor(1 / 0.75)}>
                  Zoom −
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "pan" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={togglePan}
                >
                  Pan
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "eraser" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={toggleEraser}
                >
                  Eraser
                </Button>
              </div>
            </div>

            <details className="rounded-xl border border-border bg-white p-2 shadow-xs lg:hidden">
              <summary className="cursor-pointer text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">
                Secondary tools
              </summary>
              <div className="mt-2 flex flex-wrap items-center gap-1">
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "labelAnnotation" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("labelAnnotation", event)}
                >
                  Label
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "arrowSegment" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("arrowSegment", event)}
                >
                  Arrow
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "rectangleAnnotation" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("rectangleAnnotation", event)}
                >
                  Rectangle
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "fibonacci" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={(event) => handleBindingToolClick("fibonacci", event)}
                >
                  Fibonacci
                </Button>
                <Button type="button" size="xs" variant="secondary" onClick={() => zoomByFactor(0.75)}>
                  Zoom +
                </Button>
                <Button type="button" size="xs" variant="secondary" onClick={() => zoomByFactor(1 / 0.75)}>
                  Zoom −
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "pan" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={togglePan}
                >
                  Pan
                </Button>
                <Button
                  type="button"
                  size="xs"
                  variant="secondary"
                  className={activeTool === "eraser" ? "border-indigo-200 bg-indigo-50 text-indigo-700" : undefined}
                  onClick={toggleEraser}
                >
                  Eraser
                </Button>
              </div>
            </details>
          </div>

          <div className="mt-3 flex flex-wrap items-end gap-3">
            <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Best-fit series
              <Select
                className="mt-1 w-64 max-w-full"
                value={bestFitSensorId}
                onChange={(event) => setBestFitSensorId(event.target.value)}
                disabled={activeTool === "best_fit"}
              >
                {data.map((entry) => (
                  <option key={entry.sensor_id} value={entry.sensor_id}>
                    {entry.label ?? entry.sensor_id}
                  </option>
                ))}
              </Select>
            </label>
            <label className="flex items-center gap-2 rounded-lg border border-border bg-white px-3 py-2 text-xs font-semibold text-muted-foreground">
              <input
                type="checkbox"
                className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
                checked={autoSaveBestFits}
                onChange={(event) => setAutoSaveBestFits(event.target.checked)}
              />
              Auto-save best fits
            </label>
            {activeTool === "best_fit" ? (
              <Button type="button" size="sm" variant="ghost" onClick={cancelBestFit}>
                Cancel best fit
              </Button>
            ) : null}
            {bestFits.length > 0 ? (
              <Button type="button" size="sm" variant="secondary" onClick={() => void clearBestFits()}>
                Clear fits
              </Button>
            ) : null}
          </div>

          {stacked ? (
            <InlineBanner tone="info" className="mt-3">
              Best fit is disabled while <span className="font-semibold">Stack</span> is enabled. Turn Stack off to fit
              a single series.
            </InlineBanner>
          ) : bestFitError ? (
            <InlineBanner tone="error" className="mt-3">
              {bestFitError}
            </InlineBanner>
          ) : activeTool === "best_fit" ? (
            <InlineBanner tone="info" className="mt-3">
              Best fit is armed. Drag across the chart to select the time window, then release to create the line.
            </InlineBanner>
          ) : activeTool === "pan" ? (
            <InlineBanner tone="info" className="mt-3">
              Pan mode is active. Drag to pan the x-axis; use trackpad/scroll to zoom. Press Esc to exit Pan mode.
            </InlineBanner>
          ) : activeTool === "eraser" ? (
            <InlineBanner tone="info" className="mt-3">
              Eraser is active. Click an annotation to delete it. Press Esc to exit Eraser mode.
            </InlineBanner>
          ) : isBindingTool(activeTool) ? (
            <InlineBanner tone="info" className="mt-3">
              {BINDING_TOOL_LABELS[activeTool]} is armed. Click the chart to place it. Press Esc to cancel.
            </InlineBanner>
          ) : null}

          {bestFitSeriesOptions.rows.length > 0 ? (
            <div className="mt-3 space-y-2">
              <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Best-fit lines
              </p>
              <div className="grid gap-2 md:grid-cols-2">
                {bestFitSeriesOptions.rows.map((row) => {
                  const fit = bestFits.find((entry) => entry.id === row.id);
                  const sensorId = fit?.sensorId;
                  const seriesIndex = sensorId ? data.findIndex((entry) => entry.sensor_id === sensorId) : -1;
                  const color = seriesIndex >= 0 ? seriesColor(seriesIndex) : CHART_PALETTE.reference;
                  const seriesLabel =
                    seriesIndex >= 0 ? (data[seriesIndex]?.label ?? data[seriesIndex]?.sensor_id) : sensorId ?? "Series";
                  const statusLabel = fit?.saving
                    ? "Saving..."
                    : fit?.annotationId
                      ? fit.dirty
                        ? "Unsaved changes"
                        : "Saved"
                      : "Draft";
                  const showSaveButton = Boolean(fit && (!fit.annotationId || fit.dirty));

                  return (
                    <div key={row.id} className="rounded-xl border border-border bg-white p-3 shadow-xs">
                      <div className="flex items-start justify-between gap-3">
                        <div className="min-w-0">
                          <p className="flex items-center gap-2 text-sm font-semibold text-foreground">
                            <span
                              className="h-2.5 w-2.5 rounded-full"
                              style={{ backgroundColor: color }}
                              aria-hidden
                            />
                            <span className="min-w-0 truncate">Best fit — {seriesLabel}</span>
                          </p>
                          <p className="mt-1 text-xs text-muted-foreground">{row.message}</p>
                          <p className="mt-1 text-xs font-semibold text-muted-foreground">{statusLabel}</p>
                          {fit?.saveError ? (
                            <p className="mt-1 text-xs text-rose-600">{fit.saveError}</p>
                          ) : null}
                        </div>
                        <div className="flex shrink-0 items-center gap-2">
                          {showSaveButton ? (
                            <Button
                              type="button"
                              size="sm"
                              variant="secondary"
                              onClick={() => void persistBestFit(row.id)}
                              disabled={!fit || Boolean(fit.saving)}
                            >
                              {fit?.annotationId ? "Save changes" : "Save"}
                            </Button>
                          ) : null}
                          <Button
                            type="button"
                            size="sm"
                            variant="secondary"
                            onClick={() => {
                              if (!fit) return;
                              setBestFitSensorId(fit.sensorId);
                              beginBestFit("edit", fit.id);
                            }}
                            disabled={activeTool === "best_fit" || !fit || Boolean(fit.saving)}
                          >
                            Edit
                          </Button>
                          <Button
                            type="button"
                            size="sm"
                            variant="danger"
                            onClick={() => void removeBestFit(row.id)}
                            disabled={activeTool === "best_fit" || Boolean(fit?.saving)}
                          >
                            Remove
                          </Button>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          ) : null}
        </Card>
      ) : null}
      <div
        data-testid="trend-chart-container"
        className={`relative ${effectiveHeightClassName}`}
        style={heightStyle}
        onDoubleClick={handleDoubleClick}
      >
        <HighchartsPanel
          chartRef={chartRef}
          constructorType="stockChart"
          options={chartOptions}
          wrapperClassName="h-full w-full"
        />
      </div>
      {independentAxes ? (
 <p className="mt-2 text-xs text-muted-foreground">
          Independent axes: series alternate between left and right y-axes.
        </p>
      ) : null}
      {footer ? <div className="mt-3">{footer}</div> : null}
    </Card>
  );
};
