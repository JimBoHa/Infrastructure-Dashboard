"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import clsx from "clsx";
import { formatDistanceToNow } from "date-fns";
import { useMetricsQuery, useNodesQuery, useSensorsQuery } from "@/lib/queries";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import InlineBanner from "@/components/InlineBanner";
import { TrendChart, type PersistedBestFit } from "@/components/TrendChart";
import { seriesColor } from "@/lib/chartTokens";
import type { EphemeralMarker } from "@/types/chartMarkers";
import type { AnnotationsOptions } from "highcharts";
import {
  fetchChartAnnotations,
  createChartAnnotation,
  updateChartAnnotation,
  deleteChartAnnotation,
  type ChartAnnotationRow,
} from "@/lib/api";
import { computeDomain, exportCsv } from "@/features/trends/utils/trendsUtils";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodeTypeBadge from "@/features/nodes/components/NodeTypeBadge";
import { formatSensorValueWithUnit, getSensorDisplayDecimals } from "@/lib/sensorFormat";
import MatrixProfilePanel from "@/features/trends/components/MatrixProfilePanel";
import AnalysisKey from "@/features/trends/components/AnalysisKey";
import RelationshipFinderPanel from "@/features/trends/components/RelationshipFinderPanel";
import SelectedSensorsCorrelationMatrixCard from "@/features/trends/components/SelectedSensorsCorrelationMatrixCard";
import type { RelatedSensorsExternalFocus } from "@/features/trends/types/relatedSensorsFocus";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { isPublicProviderSensor } from "@/lib/sensorOrigin";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import AnalyticsHeaderCard from "@/features/analytics/components/AnalyticsHeaderCard";
import {
  formatDateTimeInputValue,
  parseDateTimeInputValueToIso,
  useControllerTimeZone,
} from "@/lib/siteTime";
import { validateSavitzkyGolayOptions, type SavitzkyGolayEdgeMode } from "@/lib/savitzkyGolay";

const MAX_SERIES = 20;
const RANGE_OPTIONS_HOURS = [10 / 60, 1, 6, 12, 24, 72, 168, 336, 720, 2160, 8760] as const;
const INTERVAL_OPTIONS_SECONDS = [
  1, // 1s
  30, // 30s
  60, // 1 min
  300, // 5 min
  600, // 10 min
  1800, // 30 min
  3600, // 1 hour
  21600, // 6 hours
  43200, // 12 hours
  86400, // 1 day
] as const;

const EMPTY_SERIES_COLOR = "#9ca3af";
const CHART_HEIGHT_STORAGE_KEY = "fd_trends_chart_height_px";
const HIDE_EXTERNAL_SENSORS_STORAGE_KEY = "fd_trends_hide_external_sensors";
const LEGACY_DEFAULT_CHART_HEIGHT_PX = 320;
const DEFAULT_CHART_HEIGHT_PX = 420;
const MIN_CHART_HEIGHT_PX = 240;
const MAX_CHART_HEIGHT_PX = 960;
const CHART_HEIGHT_STEP_PX = 20;
const DEFAULT_SAVGOL_WINDOW_LENGTH = 11;
const DEFAULT_SAVGOL_POLY_ORDER = 3;

function formatRangeLabel(hours: number) {
  if (!Number.isFinite(hours) || hours <= 0) return "Custom range";
  if (hours < 1) {
    const minutes = Math.max(1, Math.round(hours * 60));
    return minutes === 1 ? "Last 1 minute" : `Last ${minutes} minutes`;
  }
  if (hours === 1) return "Last hour";
  if (hours < 48) return `Last ${hours} hours`;
  const days = hours / 24;
  if (Number.isInteger(days)) {
    if (days % 7 === 0) {
      const weeks = days / 7;
      return weeks === 1 ? "Last 7 days" : `Last ${weeks} weeks`;
    }
    return days === 1 ? "Last 24 hours" : `Last ${days} days`;
  }
  return `Last ${hours} hours`;
}

function formatIntervalLabel(seconds: number) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "Custom interval";
  if (seconds < 60) return `${seconds}s`;
  if (seconds % 86400 === 0) {
    const days = seconds / 86400;
    return days === 1 ? "1 day" : `${days} days`;
  }
  if (seconds % 3600 === 0) {
    const hours = seconds / 3600;
    return hours === 1 ? "1 hour" : `${hours} hours`;
  }
  const minutes = Math.round(seconds / 60);
  return minutes === 1 ? "1 min" : `${minutes} min`;
}

function recommendedIntervalSeconds(rangeHours: number) {
  if (rangeHours <= 10 / 60) return 1;
  if (rangeHours <= 1) return 30;
  if (rangeHours <= 6) return 60;
  if (rangeHours <= 24) return 60;
  if (rangeHours <= 72) return 300;
  if (rangeHours <= 168) return 1800;
  if (rangeHours <= 336) return 3600;
  if (rangeHours <= 720) return 21600;
  if (rangeHours <= 2160) return 21600;
  if (rangeHours <= 4320) return 43200;
  return 86400;
}

function parseDurationToSeconds(input: string): number | null {
  const raw = input.trim().toLowerCase();
  if (!raw) return null;
  const match = raw.match(/^(\d+(?:\.\d+)?)\s*([smhd])?$/i);
  if (!match) return null;
  const value = Number.parseFloat(match[1] ?? "");
  if (!Number.isFinite(value) || value <= 0) return null;
  const unit = (match[2] ?? "s").toLowerCase();
  const seconds =
    unit === "s"
      ? value
      : unit === "m"
        ? value * 60
        : unit === "h"
          ? value * 3600
          : value * 86400;
  if (!Number.isFinite(seconds) || seconds <= 0) return null;
  return seconds;
}

export default function TrendsPage() {
  const timeZone = useControllerTimeZone();
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const [rangeHours, setRangeHours] = useState(24);
  const [interval, setInterval] = useState(() => recommendedIntervalSeconds(24));
  const [rangeSelect, setRangeSelect] = useState<string>("24");
  const [intervalSelect, setIntervalSelect] = useState<string>(() =>
    String(recommendedIntervalSeconds(24)),
  );
  const [intervalDraft, setIntervalDraft] = useState(String(recommendedIntervalSeconds(24)));
  const [intervalError, setIntervalError] = useState<string | null>(null);
  const [rangeStartLocal, setRangeStartLocal] = useState<string>(() => {
    const now = new Date();
    now.setSeconds(0, 0);
    const start = new Date(now.getTime() - 24 * 60 * 60 * 1000);
    return formatDateTimeInputValue(start, timeZone);
  });
  const [rangeEndLocal, setRangeEndLocal] = useState<string>(() => {
    const now = new Date();
    now.setSeconds(0, 0);
    return formatDateTimeInputValue(now, timeZone);
  });
  const [rangeError, setRangeError] = useState<string | null>(null);
  const [independentAxes, setIndependentAxes] = useState(false);
  const [stacked, setStacked] = useState(false);
  const [savgolEnabled, setSavgolEnabled] = useState(false);
  const [savgolWindowLength, setSavgolWindowLength] = useState(DEFAULT_SAVGOL_WINDOW_LENGTH);
  const [savgolPolyOrder, setSavgolPolyOrder] = useState(DEFAULT_SAVGOL_POLY_ORDER);
  const [savgolDerivOrder, setSavgolDerivOrder] = useState(0);
  const [savgolEdgeMode, setSavgolEdgeMode] = useState<SavitzkyGolayEdgeMode>("interp");
  const [savgolDerivativeUnit, setSavgolDerivativeUnit] = useState<"s" | "min" | "h">("s");
  const [selected, setSelected] = useState<string[]>([]);
  const [relatedSensorsExternalFocus, setRelatedSensorsExternalFocus] =
    useState<RelatedSensorsExternalFocus | null>(null);
  const [ephemeralMarkers, setEphemeralMarkers] = useState<EphemeralMarker[]>([]);
  const [persistentAnnotations, setPersistentAnnotations] = useState<ChartAnnotationRow[]>([]);
  const [activePopoverMarkerId, setActivePopoverMarkerId] = useState<string | null>(null);
  const [promoteNote, setPromoteNote] = useState("");
  const [yMin, setYMin] = useState<string>("");
  const [yMax, setYMax] = useState<string>("");
  const [nodeFilter, setNodeFilter] = useState<string>("all");
  const [search, setSearch] = useState<string>("");
  const [hideExternalSensors, setHideExternalSensors] = useState<boolean>(() => {
    if (typeof window === "undefined") return false;
    try {
      return window.localStorage.getItem(HIDE_EXTERNAL_SENSORS_STORAGE_KEY) === "true";
    } catch {
      return false;
    }
  });
  const [chartHeightPx, setChartHeightPx] = useState<number>(() => {
    if (typeof window === "undefined") return DEFAULT_CHART_HEIGHT_PX;
    try {
      const raw = window.localStorage.getItem(CHART_HEIGHT_STORAGE_KEY);
      if (!raw) return DEFAULT_CHART_HEIGHT_PX;
      const parsed = Number(raw);
      if (!Number.isFinite(parsed)) return DEFAULT_CHART_HEIGHT_PX;
      const clamped = Math.min(MAX_CHART_HEIGHT_PX, Math.max(MIN_CHART_HEIGHT_PX, Math.round(parsed)));
      // Migrate legacy default values to the new, taller default (the old value was auto-persisted on first load).
      return clamped === LEGACY_DEFAULT_CHART_HEIGHT_PX ? DEFAULT_CHART_HEIGHT_PX : clamped;
    } catch {
      return DEFAULT_CHART_HEIGHT_PX;
    }
  });

  useEffect(() => {
    try {
      window.localStorage.setItem(CHART_HEIGHT_STORAGE_KEY, String(chartHeightPx));
    } catch {
      // ignore storage failures (private mode / disabled storage)
    }
  }, [chartHeightPx]);

  const prevTimeZoneRef = useRef(timeZone);
  useEffect(() => {
    const prev = prevTimeZoneRef.current;
    if (prev === timeZone) return;

    const startIso = parseDateTimeInputValueToIso(rangeStartLocal, prev);
    const endIso = parseDateTimeInputValueToIso(rangeEndLocal, prev);
    if (startIso) {
      const start = new Date(startIso);
      if (Number.isFinite(start.getTime())) setRangeStartLocal(formatDateTimeInputValue(start, timeZone));
    }
    if (endIso) {
      const end = new Date(endIso);
      if (Number.isFinite(end.getTime())) setRangeEndLocal(formatDateTimeInputValue(end, timeZone));
    }

    prevTimeZoneRef.current = timeZone;
  }, [rangeEndLocal, rangeStartLocal, timeZone]);

  useEffect(() => {
    try {
      window.localStorage.setItem(
        HIDE_EXTERNAL_SENSORS_STORAGE_KEY,
        hideExternalSensors ? "true" : "false",
      );
    } catch {
      // ignore storage failures (private mode / disabled storage)
    }
  }, [hideExternalSensors]);

  // Load persistent annotations on mount
  useEffect(() => {
    let cancelled = false;
    fetchChartAnnotations()
      .then((rows) => {
        if (!cancelled) setPersistentAnnotations(rows ?? []);
      })
      .catch(() => {
        // Non-critical: annotations may not be available yet (migration pending)
      });
    return () => { cancelled = true; };
  }, []);

  // Ephemeral marker callbacks
  const addEphemeralMarkers = useCallback((markers: EphemeralMarker[]) => {
    setEphemeralMarkers((prev) => {
      const ids = new Set(prev.map((m) => m.id));
      const fresh = markers.filter((m) => !ids.has(m.id));
      return fresh.length > 0 ? [...prev, ...fresh] : prev;
    });
  }, []);

  const dismissMarker = useCallback((id: string) => {
    setEphemeralMarkers((prev) => prev.filter((m) => m.id !== id));
    setActivePopoverMarkerId(null);
  }, []);

  const promoteMarker = useCallback(
    async (id: string, note?: string) => {
      const marker = ephemeralMarkers.find((m) => m.id === id);
      if (!marker) return;
      try {
        const row = await createChartAnnotation({
          chart_state: {
            type: "promoted_marker",
            source: marker.source,
            detail: marker.detail,
          },
          sensor_ids: marker.sensorIds,
          time_start: marker.timestamp.toISOString(),
          time_end: marker.timeEnd?.toISOString(),
          label: note || marker.label,
        });
        setPersistentAnnotations((prev) => [...prev, row]);
        setEphemeralMarkers((prev) => prev.filter((m) => m.id !== id));
        setActivePopoverMarkerId(null);
        setPromoteNote("");
      } catch {
        // Annotation save failed — marker stays ephemeral
      }
    },
    [ephemeralMarkers],
  );

  const jumpToTimestamp = useCallback(
    (timestampMs: number) => {
      if (typeof timestampMs !== "number" || !Number.isFinite(timestampMs)) return;
      const center = new Date(timestampMs);
      if (!Number.isFinite(center.getTime())) return;

      const halfWindowMs = 60 * 60 * 1000;
      const start = new Date(center.getTime() - halfWindowMs);
      const end = new Date(center.getTime() + halfWindowMs);

      setRangeSelect("custom");
      setRangeStartLocal(formatDateTimeInputValue(start, timeZone));
      setRangeEndLocal(formatDateTimeInputValue(end, timeZone));
      setRangeError(null);

      const hours = (end.getTime() - start.getTime()) / (60 * 60 * 1000);
      if (Number.isFinite(hours) && hours > 0) {
        setRangeHours(hours);
        if (intervalSelect !== "custom") {
          const rec = recommendedIntervalSeconds(hours);
          setInterval(rec);
          setIntervalSelect(String(rec));
          setIntervalDraft(String(rec));
        }
      }
    },
    [intervalSelect, timeZone],
  );

  const handleCreatePersistentAnnotation = useCallback(
    async (payload: Parameters<typeof createChartAnnotation>[0]) => {
      const row = await createChartAnnotation(payload);
      setPersistentAnnotations((prev) => [...prev, row]);
      return row;
    },
    [],
  );

  const handleUpdatePersistentAnnotation = useCallback(
    async (
      annotationId: string,
      payload: Partial<Parameters<typeof createChartAnnotation>[0]>,
    ) => {
      const row = await updateChartAnnotation(annotationId, payload);
      setPersistentAnnotations((prev) => {
        const existingIndex = prev.findIndex((entry) => entry.id === annotationId);
        if (existingIndex < 0) return [...prev, row];
        const next = [...prev];
        next[existingIndex] = row;
        return next;
      });
      return row;
    },
    [],
  );

  const handleDeletePersistentAnnotation = useCallback(async (annotationId: string) => {
    try {
      await deleteChartAnnotation(annotationId);
      setPersistentAnnotations((prev) => prev.filter((a) => a.id !== annotationId));
    } catch {
      // Non-critical: deletion failed
    }
  }, []);

  const handleMarkerClick = useCallback((markerId: string) => {
    setActivePopoverMarkerId((prev) => (prev === markerId ? null : markerId));
    setPromoteNote("");
  }, []);

  // Convert persistent annotations to Highcharts annotations format
  const persistentHcAnnotations: AnnotationsOptions[] = useMemo(() => {
    if (!persistentAnnotations) return [];

    const out: AnnotationsOptions[] = [];
    for (const row of persistentAnnotations) {
      const state = row.chart_state as Record<string, unknown> | undefined;
      const type = typeof state?.type === "string" ? state.type : "";

      if (type === "highcharts_annotation") {
        const options = state?.options as Record<string, unknown> | undefined;
        if (!options || typeof options !== "object") continue;
        out.push({ ...(options as AnnotationsOptions), id: row.id });
        continue;
      }

      if (type === "promoted_marker") {
        const tMs = new Date(row.time_start ?? row.created_at).getTime();
        out.push({
          id: row.id,
          draggable: "",
          zIndex: 5,
          labels: row.label
            ? [
                {
                  point: { x: tMs, y: 0, xAxis: 0, yAxis: 0 },
                  text: row.label,
                  backgroundColor: "#e11d48",
                  borderColor: "#e11d48",
                  style: { color: "#ffffff", fontSize: "10px", fontWeight: "600" },
                  padding: 4,
                  borderRadius: 4,
                  y: -10,
                },
              ]
            : [],
          shapes: [
            {
              type: "path",
              strokeWidth: 2,
              stroke: "#e11d48",
              dashStyle: "Solid",
              points: [
                { x: tMs, y: 0, xAxis: 0, yAxis: 0 },
                { x: tMs, y: 1, xAxis: 0, yAxis: 0 },
              ],
              point: { x: tMs, xAxis: 0 },
            },
          ],
        } as AnnotationsOptions);
        continue;
      }
    }

    return out;
  }, [persistentAnnotations]);

  const persistedBestFits: PersistedBestFit[] = useMemo(() => {
    if (!persistentAnnotations) return [];
    const out: PersistedBestFit[] = [];
    for (const row of persistentAnnotations) {
      const state = row.chart_state as Record<string, unknown> | undefined;
      if (!state || state.type !== "best_fit_v1") continue;

      const sensorId = typeof state.sensor_id === "string" ? state.sensor_id.trim() : "";
      if (!sensorId) continue;

      const startMsRaw = typeof state.start_ms === "number" ? state.start_ms : NaN;
      const endMsRaw = typeof state.end_ms === "number" ? state.end_ms : NaN;
      const startMsFromTime = row.time_start ? new Date(row.time_start).getTime() : NaN;
      const endMsFromTime = row.time_end ? new Date(row.time_end).getTime() : NaN;
      const startMs = Number.isFinite(startMsRaw) ? startMsRaw : startMsFromTime;
      const endMs = Number.isFinite(endMsRaw) ? endMsRaw : endMsFromTime;
      if (!Number.isFinite(startMs) || !Number.isFinite(endMs)) continue;

      out.push({
        annotationId: row.id,
        sensorId,
        startMs: Math.min(startMs, endMs),
        endMs: Math.max(startMs, endMs),
      });
    }
    return out;
  }, [persistentAnnotations]);

  const customStartIso = useMemo(
    () => parseDateTimeInputValueToIso(rangeStartLocal, timeZone),
    [rangeStartLocal, timeZone],
  );
  const customEndIso = useMemo(
    () => parseDateTimeInputValueToIso(rangeEndLocal, timeZone),
    [rangeEndLocal, timeZone],
  );
  const customRangeValid =
    rangeSelect !== "custom" ||
    (Boolean(customStartIso) && Boolean(customEndIso) && !rangeError);

  const savgolDeltaUnitSeconds = useMemo(() => {
    return savgolDerivativeUnit === "min" ? 60 : savgolDerivativeUnit === "h" ? 3600 : 1;
  }, [savgolDerivativeUnit]);

  const savgolDelta = useMemo(() => {
    if (!Number.isFinite(interval) || interval <= 0) return 1;
    return interval / savgolDeltaUnitSeconds;
  }, [interval, savgolDeltaUnitSeconds]);

  const savgolValidation = useMemo(() => {
    return validateSavitzkyGolayOptions({
      windowLength: savgolWindowLength,
      polyOrder: savgolPolyOrder,
      derivOrder: savgolDerivOrder,
      edgeMode: savgolEdgeMode,
      delta: savgolDelta,
    });
  }, [savgolDerivOrder, savgolDelta, savgolEdgeMode, savgolPolyOrder, savgolWindowLength]);

  const savgolError =
    savgolEnabled && !savgolValidation.ok ? savgolValidation.error ?? "Invalid Savitzky–Golay settings." : null;

  const savgolConfig = useMemo(() => {
    return {
      enabled: savgolEnabled,
      windowLength: savgolWindowLength,
      polyOrder: savgolPolyOrder,
      derivOrder: savgolDerivOrder,
      edgeMode: savgolEdgeMode,
      delta: savgolDelta,
      xUnitLabel: savgolDerivativeUnit,
    };
  }, [
    savgolDelta,
    savgolDerivOrder,
    savgolDerivativeUnit,
    savgolEdgeMode,
    savgolEnabled,
    savgolPolyOrder,
    savgolWindowLength,
  ]);

  useEffect(() => {
    if (rangeSelect !== "custom") {
      const id = setTimeout(() => setRangeSelect(String(rangeHours)), 0);
      return () => clearTimeout(id);
    }
    return;
  }, [rangeHours, rangeSelect]);

  useEffect(() => {
    if (intervalSelect !== "custom") {
      const id = setTimeout(() => setIntervalSelect(String(interval)), 0);
      return () => clearTimeout(id);
    }
    return;
  }, [interval, intervalSelect]);

  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);
  const nodesById = useMemo(() => {
    return new Map(nodes.map((node) => [node.id, node]));
  }, [nodes]);
  const sensorsById = useMemo(() => {
    return new Map(sensors.map((sensor) => [sensor.sensor_id, sensor]));
  }, [sensors]);

  const effectiveSelected = useMemo(
    () => selected.filter((sensorId) => sensorsById.has(sensorId)),
    [selected, sensorsById],
  );
  const effectiveSelectedSet = useMemo(
    () => new Set(effectiveSelected),
    [effectiveSelected],
  );

  const filteredSensors = useMemo(() => {
    const query = search.trim().toLowerCase();
    return sensors.filter((sensor) => {
      if (nodeFilter !== "all" && sensor.node_id !== nodeFilter) return false;
      if (
        hideExternalSensors &&
        isPublicProviderSensor(sensor)
      ) {
        return false;
      }
      if (!query) return true;
      const nodeName = nodesById.get(sensor.node_id)?.name ?? "";
      const haystack = `${sensor.name} ${sensor.sensor_id} ${sensor.type} ${sensor.unit} ${nodeName}`.toLowerCase();
      return haystack.includes(query);
    });
  }, [hideExternalSensors, sensors, nodesById, nodeFilter, search]);

  const sensorsByNode = useMemo(() => {
    const map = new Map<string, typeof sensors>();
    filteredSensors.forEach((sensor) => {
      const list = map.get(sensor.node_id) ?? [];
      list.push(sensor);
      map.set(sensor.node_id, list);
    });
    return map;
  }, [filteredSensors]);

  const visibleNodes = useMemo(() => {
    const entries = Array.from(sensorsByNode.keys()).map((nodeId) => ({
      id: nodeId,
      name: nodesById.get(nodeId)?.name ?? "Unknown node",
    }));
    return entries;
  }, [nodesById, sensorsByNode]);

  const labelMap = useMemo(() => {
    return new Map(
      sensors.map((sensor) => {
        const nodeName = nodesById.get(sensor.node_id)?.name ?? "Unknown node";
        const unit = sensor.unit ? ` (${sensor.unit})` : "";
        return [sensor.sensor_id, `${nodeName} — ${sensor.name}${unit}`];
      }),
    );
  }, [nodesById, sensors]);
  const sensorMeta = useMemo(() => {
    return new Map(
      sensors.map((sensor) => [
        sensor.sensor_id,
        {
          unit: sensor.unit,
          displayDecimals: getSensorDisplayDecimals(sensor),
        },
      ]),
    );
  }, [sensors]);
  const {
    data: fetchedSeries,
    isLoading: metricsLoading,
    error: metricsError,
  } = useMetricsQuery({
    sensorIds: effectiveSelected,
    rangeHours,
    interval,
    start: rangeSelect === "custom" ? customStartIso ?? undefined : undefined,
    end: rangeSelect === "custom" ? customEndIso ?? undefined : undefined,
    enabled: effectiveSelected.length > 0 && customRangeValid,
    refetchInterval: 10_000,
  });
  const labeledSeries = useMemo(() => {
    if (!fetchedSeries) return [];
    return fetchedSeries.map((series) => ({
      ...series,
      label: labelMap.get(series.sensor_id) ?? series.sensor_id,
      unit: sensorMeta.get(series.sensor_id)?.unit,
      display_decimals: sensorMeta.get(series.sensor_id)?.displayDecimals ?? undefined,
    }));
  }, [fetchedSeries, labelMap, sensorMeta]);
  const chartSeries = labeledSeries.filter((series) => series.points.length > 0);

  const seriesBySensorId = useMemo(() => {
    const map = new Map<string, (typeof labeledSeries)[number]>();
    labeledSeries.forEach((entry) => map.set(entry.sensor_id, entry));
    return map;
  }, [labeledSeries]);

  const relationshipSeries = useMemo(() => {
    return effectiveSelected.map((sensorId) => {
      const existing = seriesBySensorId.get(sensorId);
      if (existing) return existing;
      const meta = sensorMeta.get(sensorId);
      return {
        sensor_id: sensorId,
        label: labelMap.get(sensorId) ?? sensorId,
        unit: meta?.unit,
        display_decimals: meta?.displayDecimals ?? undefined,
        points: [],
      };
    });
  }, [effectiveSelected, labelMap, sensorMeta, seriesBySensorId]);

  const chartSeriesIndexBySensorId = useMemo(() => {
    const map = new Map<string, number>();
    chartSeries.forEach((series, index) => map.set(series.sensor_id, index));
    return map;
  }, [chartSeries]);

  const selectedBadges = useMemo(() => {
    return effectiveSelected.map((sensorId) => {
      const chartIndex = chartSeriesIndexBySensorId.get(sensorId);
      const chartColor =
        chartIndex != null
          ? seriesColor(chartIndex)
          : EMPTY_SERIES_COLOR;
      const axisSide =
        independentAxes && chartIndex != null ? (chartIndex % 2 === 0 ? "L" : "R") : null;
      const sensor = sensorsById.get(sensorId);
      const latestTs = sensor?.latest_ts ?? null;
      const latestValue = sensor?.latest_value ?? null;
      const latestLabel =
        latestValue != null ? formatSensorValueWithUnit(sensor ?? { config: {}, unit: "" }, latestValue) : null;
      const updatedLabel =
        latestTs instanceof Date && Number.isFinite(latestTs.getTime())
          ? formatDistanceToNow(latestTs, { addSuffix: true })
          : null;
      return {
        sensorId,
        label: labelMap.get(sensorId) ?? sensorId,
        color: chartColor,
        axisSide,
        hasData: chartIndex != null,
        updatedLabel,
        latestLabel,
      };
    });
  }, [chartSeriesIndexBySensorId, effectiveSelected, independentAxes, labelMap, sensorsById]);

  const toggleSensorSelection = useCallback(
    (sensorId: string) => {
      setSelected((prev) => {
        const cleaned = prev.filter((id) => sensorsById.has(id));
        if (cleaned.includes(sensorId)) {
          return cleaned.filter((id) => id !== sensorId);
        }
        if (cleaned.length >= MAX_SERIES) return cleaned;
        return [...cleaned, sensorId];
      });
    },
    [sensorsById],
  );

  if (nodesQuery.isLoading || sensorsQuery.isLoading) {
    return <LoadingState label="Loading sensors..." />;
  }
  if (nodesQuery.error || sensorsQuery.error) {
    return (
      <ErrorState
        message={
          (nodesQuery.error instanceof Error && nodesQuery.error.message) ||
          (sensorsQuery.error instanceof Error && sensorsQuery.error.message) ||
          "Failed to load sensors."
        }
      />
    );
  }

  const disabledNotice =
    effectiveSelected.length >= MAX_SERIES
      ? `Maximum of ${MAX_SERIES} series selected`
      : null;

  return (
    <div className="space-y-5">
      <AnalyticsHeaderCard
        tab="trends"
        actions={
          <>
            <AnalysisKey
              summary="Key"
              overview={
                <>
                  Pick up to <span className="font-semibold">{MAX_SERIES}</span> sensors, set{" "}
                  <span className="font-semibold">Range</span> +{" "}
                  <span className="font-semibold">Interval</span>, then use the analysis panels to compare patterns and export CSV.
                </>
              }
            >
              <div className="space-y-3">
                <div>
 <p className="text-xs font-semibold text-foreground">Quick workflow</p>
 <ol className="mt-1 list-decimal space-y-1 ps-5 text-muted-foreground">
                    <li>Select sensors in the Sensor picker (up to {MAX_SERIES}).</li>
                    <li>
                      Choose a time window (<span className="font-semibold">Range</span>) and resolution (
                      <span className="font-semibold">Interval</span>).
                    </li>
                    <li>Inspect the Trend chart (hover for exact values; gaps usually mean missing data).</li>
                    <li>
                      Use <span className="font-semibold">Related sensors</span> to discover candidates, then{" "}
                      <span className="font-semibold">Add</span> to overlay.
                    </li>
                    <li>
                      Use <span className="font-semibold">Relationships</span> to confirm (scatter + lag/lead + rolling).
                    </li>
                    <li>
                      Use <span className="font-semibold">Matrix Profile</span> to find repeating patterns/anomalies inside a
                      single sensor.
                    </li>
                  </ol>
                </div>

                <div>
 <p className="text-xs font-semibold text-foreground">Glossary (plain English)</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                    <li>
                      <span className="font-semibold">Interval / bucket</span>: the time “bin” size. One bucket becomes one
                      plotted point (typically the average value inside that time bin).
                    </li>
                    <li>
                      <span className="font-semibold">Correlation (r)</span>: a score from -1 to 1. Near 1 means the two
                      signals usually move together; near -1 means they move opposite; near 0 means no consistent
                      relationship.
                    </li>
                    <li>
                      <span className="font-semibold">Overlap buckets (n)</span>: how many Interval buckets had data for both
                      sensors (after the best lag). Small <code>n</code> = less reliable.
                    </li>
                    <li>
                      <span className="font-semibold">Best lag</span>: a time shift that lines sensors up.{" "}
                      <span className="font-semibold">+</span> means the candidate tends to happen later than the focus.
                    </li>
                    <li>
                      <span className="font-semibold">Event score</span>: 0..1 score for how often spikes line up (Events
                      mode).
                    </li>
                    <li>
                      <span className="font-semibold">Window (points)</span>: how long a “snippet” is for Matrix Profile
                      (measured in Interval buckets).
                    </li>
                    <li>
                      <span className="font-semibold">Distance</span>: “shape difference” between windows (lower = more
                      similar; higher = more unusual).
                    </li>
                  </ul>
                </div>

                <div>
 <p className="text-xs font-semibold text-foreground">Two important notes</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                    <li>Correlation is a hint, not proof. Use Relationships to sanity-check before you act on it.</li>
                    <li>
                      Times/axes are rendered in controller/site time (falls back to browser time if unavailable). We
                      plan to standardize time across the rest of the product later.
                    </li>
                  </ul>
                </div>
              </div>
            </AnalysisKey>
            <NodeButton
              type="button"
              onClick={() => exportCsv(effectiveSelected, chartSeries)}
              disabled={chartSeries.length === 0}
              size="sm"
            >
              Export CSV
            </NodeButton>
            <NodeButton
              type="button"
              onClick={() => setSelected([])}
              disabled={effectiveSelected.length === 0}
              size="sm"
              variant="secondary"
            >
              Clear
            </NodeButton>
          </>
        }
      />

      <div className="grid gap-6 xl:grid-cols-[280px_1fr]">
        <CollapsibleCard
          title="Sensor picker"
          description={`${effectiveSelected.length}/${MAX_SERIES} selected`}
          defaultOpen
          density="sm"
          bodyClassName="space-y-4"
        >

          {selectedBadges.length ? (
            <div className="mt-3">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Selected
              </p>
              <div className="mt-2 flex flex-wrap gap-2">
                {selectedBadges.map((badge) => {
                  const sensor = sensorsById.get(badge.sensorId) ?? null;
                  return (
                    <button
                      key={badge.sensorId}
                      type="button"
                      onClick={() =>
                        setSelected((prev) => prev.filter((id) => id !== badge.sensorId))
                      }
 className="inline-flex max-w-full items-center gap-2 rounded-full border border-border bg-white px-3 py-1 text-xs font-semibold text-foreground shadow-xs hover:bg-muted"
                      title="Remove from chart"
                    >
                      <span
                        className="size-2 rounded-full"
                        style={{ backgroundColor: badge.color }}
                        aria-hidden
                      />
                      {sensor ? <SensorOriginBadge sensor={sensor} size="xs" /> : null}
                      <span className="min-w-0 truncate">
                        {badge.label}
                        {badge.axisSide ? ` (${badge.axisSide})` : ""}
                        {!badge.hasData
                          ? ` · no data${badge.latestLabel ? ` · last ${badge.latestLabel}` : ""}${
                              badge.updatedLabel ? ` (${badge.updatedLabel})` : ""
                            }`
                          : ""}
                      </span>
 <span className="text-muted-foreground" aria-hidden>
                        ×
                      </span>
                    </button>
                  );
                })}
              </div>
              {independentAxes ? (
 <p className="mt-2 text-xs text-muted-foreground">
                  Independent axes: hover legend entries in the chart to reveal matching y-axis labels.
                </p>
              ) : null}
            </div>
          ) : null}

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <Select
              value={nodeFilter}
              onChange={(event) => setNodeFilter(event.target.value)}
              className="max-w-full min-w-0 sm:w-auto"
            >
              <option value="all">All nodes</option>
              {nodes.map((node) => (
                <option key={node.id} value={node.id}>
                  {node.name}
                </option>
              ))}
            </Select>
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search sensors…"
              className="flex-1"
            />
          </div>
 <label className="mt-3 flex cursor-pointer flex-wrap items-center gap-2 text-sm text-foreground">
            <input
              type="checkbox"
              checked={hideExternalSensors}
              onChange={(event) => setHideExternalSensors(event.target.checked)}
 className="mt-0.5 h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
            />
            <span className="font-semibold">Hide public provider data</span>
 <span className="text-xs text-muted-foreground">
              Open-Meteo, Forecast.Solar
            </span>
          </label>
          <div className="mt-4 max-h-[560px] space-y-2 overflow-y-auto pr-1">
            {visibleNodes.length === 0 ? (
 <Card className="rounded-lg gap-0 border-dashed px-3 py-4 text-sm text-muted-foreground">
                No sensors match the current filters.
              </Card>
            ) : (
              visibleNodes.map((node) => {
                const list = sensorsByNode.get(node.id) ?? [];
                const selectedCount = list.filter((sensor) =>
                  effectiveSelectedSet.has(sensor.sensor_id),
                ).length;
                const fullNode = nodesById.get(node.id) ?? null;
                return (
                  <CollapsibleCard
                    key={node.id}
                    title={<span className="flex min-w-0 items-center gap-2"><span className="truncate">{node.name}</span>{fullNode ? <NodeTypeBadge node={fullNode} size="sm" className="shrink-0" /> : null}</span>}
                    actions={<span className="shrink-0 text-xs font-semibold text-muted-foreground">{selectedCount}/{list.length}</span>}
                    defaultOpen={nodeFilter !== "all" || search.trim().length > 0}
                    density="sm"
                  >
                    <div className="space-y-2">
                      {list.map((sensor) => {
                        const checked = effectiveSelectedSet.has(sensor.sensor_id);
                        const disabled = !checked && effectiveSelected.length >= MAX_SERIES;
                        return (
                          <Card
                            key={sensor.sensor_id}
                            className={clsx(
                              "flex-row items-center gap-3 px-3 py-2 transition",
                              checked
                                ? "border-info-surface-border bg-info-surface"
                                : "hover:bg-muted",
                              disabled
                                ? "cursor-not-allowed opacity-60"
                                : "cursor-pointer",
                            )}
                            onClick={() => {
                              if (disabled) return;
                              toggleSensorSelection(sensor.sensor_id);
                            }}
                          >
                            <input
                              id={`sensor-pick-${sensor.sensor_id}`}
                              type="checkbox"
                              className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
                              checked={checked}
                              disabled={disabled}
                              onClick={(e) => e.stopPropagation()}
                              onChange={(e) => {
                                e.stopPropagation();
                                toggleSensorSelection(sensor.sensor_id);
                              }}
                            />
                            <label
                              htmlFor={`sensor-pick-${sensor.sensor_id}`}
                              className="min-w-0 cursor-[inherit]"
                              onClick={(e) => e.stopPropagation()}
                            >
                              <div className="flex min-w-0 items-center gap-2">
                                <p className="min-w-0 truncate text-sm font-semibold text-foreground">
                                  {sensor.name}
                                </p>
                                <SensorOriginBadge sensor={sensor} size="xs" />
                              </div>
                              <p className="truncate text-xs text-muted-foreground">
                                {sensor.type} · {sensor.unit || "—"} · {sensor.sensor_id}
                              </p>
                            </label>
                          </Card>
                        );
                      })}
                    </div>
                  </CollapsibleCard>
                );
              })
            )}
          </div>

          <AnalysisKey
            summary="Key"
            overview={
              <>
                Select up to <span className="font-semibold">{MAX_SERIES}</span> sensors to overlay. Use the node filter
                and search to find sensors; badges show where the sensor comes from.
              </>
            }
          >
            <div className="space-y-3">
              <div>
 <p className="text-xs font-semibold text-foreground">What this is</p>
 <p className="mt-1 text-xs text-muted-foreground">
                  Pick the sensors you want to chart. Sensors are grouped by node, and you can select up to{" "}
                  <span className="font-semibold">{MAX_SERIES}</span> at once.
                </p>
              </div>

              <div>
 <p className="text-xs font-semibold text-foreground">How to use</p>
 <ol className="mt-1 list-decimal space-y-1 ps-5 text-muted-foreground">
                  <li>
                    Use <span className="font-semibold">All nodes</span> or pick a single node.
                  </li>
                  <li>
                    Use <span className="font-semibold">Search</span> to filter by name/type/id.
                  </li>
                  <li>Check sensors to add them to the chart (and uncheck to remove).</li>
                  <li>
                    The <span className="font-semibold">Selected</span> chips at the top are your active chart series.
                  </li>
                </ol>
              </div>

              <div>
 <p className="text-xs font-semibold text-foreground">Badges</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                  <li>
                    <span className="font-semibold">Node</span> badges (e.g., Pi 5 / Core / WS) show where the sensor
                    lives.
                  </li>
                  <li>
                    <span className="font-semibold">Derived</span> means the sensor is computed from other sensors (a
                    formula), not read from hardware.
                  </li>
                  <li>
                    <span className="font-semibold">Public provider data</span> (badge:{" "}
                    <span className="font-semibold">PUBLIC</span>) means the sensor comes from an outside/public provider
                    (e.g., Open-Meteo, Forecast.Solar), not a physical device.
                  </li>
                </ul>
              </div>

              <div>
 <p className="text-xs font-semibold text-foreground">Filters</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                  <li>
                    <span className="font-semibold">Hide public provider data</span> removes Public provider data sensors
                    (<span className="font-semibold">PUBLIC</span> badge) from this list. It does not delete anything.
                  </li>
                </ul>
              </div>
            </div>
          </AnalysisKey>
        </CollapsibleCard>
        <section className="min-w-0 space-y-4">
          <CollapsibleCard
            title="Chart settings"
            description="Range, interval, axes, and y-domain controls for the chart below."
            defaultOpen
            bodyClassName="space-y-4"
          >

            <div className="mt-4 flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
              <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Range
                  <Select
                    className="mt-1"
                    value={rangeSelect}
                    onChange={(event) => {
                      const next = event.target.value;
                      setRangeError(null);
                      if (next === "custom") {
                        setRangeSelect("custom");
                        const now = new Date();
                        now.setSeconds(0, 0);
                        setRangeEndLocal(formatDateTimeInputValue(now, timeZone));
                        setRangeStartLocal(
                          formatDateTimeInputValue(new Date(now.getTime() - 24 * 60 * 60 * 1000), timeZone),
                        );
                        return;
                      }

                      const nextRange = Number(next);
                      setRangeSelect(next);
                      setRangeHours(nextRange);
                      if (intervalSelect !== "custom") {
                        const rec = recommendedIntervalSeconds(nextRange);
                        setInterval(rec);
                        setIntervalSelect(String(rec));
                        setIntervalDraft(String(rec));
                      }
                    }}
                  >
                    {RANGE_OPTIONS_HOURS.map((hrs) => (
                      <option key={hrs} value={hrs}>
                        {formatRangeLabel(hrs)}
                      </option>
                    ))}
                    <option value="custom">Custom…</option>
                  </Select>
                </label>

 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Interval
                  <Select
                    className="mt-1"
                    value={intervalSelect}
                    onChange={(event) => {
                      const next = event.target.value;
                      setIntervalError(null);
                      if (next === "custom") {
                        setIntervalSelect("custom");
                        setIntervalDraft(intervalDraft.trim() ? intervalDraft : `${interval}`);
                        return;
                      }

                      const nextInterval = Number(next);
                      setIntervalSelect(next);
                      setIntervalDraft(next);
                      setInterval(nextInterval);
                    }}
                  >
                    {INTERVAL_OPTIONS_SECONDS.map((sec) => (
                      <option key={sec} value={sec}>
                        {formatIntervalLabel(sec)}
                      </option>
                    ))}
                    <option value="custom">Custom…</option>
                  </Select>
                </label>

 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Y min
                  <Input
                    value={yMin}
                    onChange={(event) => setYMin(event.target.value)}
                    className="mt-1"
                    placeholder="auto"
                  />
                </label>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Y max
                  <Input
                    value={yMax}
                    onChange={(event) => setYMax(event.target.value)}
                    className="mt-1"
                    placeholder="auto"
                  />
                </label>
              </div>

 <div className="flex flex-wrap items-center gap-4 text-sm text-muted-foreground">
                <label className="inline-flex items-center gap-2">
                  <input
                    type="checkbox"
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
                    checked={stacked}
                    disabled={independentAxes}
                    onChange={(event) => setStacked(event.target.checked)}
                  />
                  Stack
                </label>
                <label className="inline-flex items-center gap-2">
                  <input
                    type="checkbox"
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
                    checked={independentAxes}
                    onChange={(event) => {
                      setIndependentAxes(event.target.checked);
                      if (event.target.checked) setStacked(false);
                    }}
                  />
                  Independent axes
                </label>
                <label className="inline-flex items-center gap-2">
                  <input
                    type="checkbox"
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
                    checked={savgolEnabled}
                    onChange={(event) => setSavgolEnabled(event.target.checked)}
                  />
 <span className={clsx(savgolError ? "text-rose-600" : undefined)}>
                    Savitzky–Golay
                  </span>
                </label>
                <div className="flex items-center gap-2">
                  <label className="inline-flex items-center gap-2">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Chart height
                    </span>
                    <input
                      type="range"
                      min={MIN_CHART_HEIGHT_PX}
                      max={MAX_CHART_HEIGHT_PX}
                      step={CHART_HEIGHT_STEP_PX}
                      value={chartHeightPx}
                      onChange={(event) => setChartHeightPx(Number(event.target.value))}
                      className="h-2 w-28 cursor-pointer accent-indigo-600"
                      aria-label="Chart height"
                    />
 <span className="text-xs tabular-nums text-muted-foreground">
                      {chartHeightPx}px
                    </span>
                  </label>
                  <NodeButton
                    type="button"
                    size="sm"
                    variant="secondary"
                    onClick={() => setChartHeightPx(DEFAULT_CHART_HEIGHT_PX)}
                    disabled={chartHeightPx === DEFAULT_CHART_HEIGHT_PX}
                  >
                    Reset
                  </NodeButton>
                </div>
              </div>
            </div>

            <div className="mt-3 grid gap-3 sm:grid-cols-2">
              {rangeSelect === "custom" ? (
                <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Start / end (site time)
                  </label>
                  <div className="mt-1 grid gap-2 sm:grid-cols-2">
                    <Input
                      type="datetime-local"
                      value={rangeStartLocal}
                      onChange={(event) => {
                        const next = event.target.value;
                        setRangeStartLocal(next);
                        const startIso = parseDateTimeInputValueToIso(next, timeZone);
                        const endIso = parseDateTimeInputValueToIso(rangeEndLocal, timeZone);
                        if (!startIso || !endIso) {
                          setRangeError("Enter a valid start and end date/time.");
                          return;
                        }
                        const start = new Date(startIso);
                        const end = new Date(endIso);
                        if (!(start < end)) {
                          setRangeError("Start must be before end.");
                          return;
                        }
                        const hours = (end.getTime() - start.getTime()) / (60 * 60 * 1000);
                        if (hours > 8760) {
                          setRangeError("Max range is 365d.");
                          return;
                        }
                        setRangeError(null);
                        setRangeHours(hours);
                        if (intervalSelect !== "custom") {
                          const rec = recommendedIntervalSeconds(hours);
                          setInterval(rec);
                          setIntervalSelect(String(rec));
                          setIntervalDraft(String(rec));
                        }
                      }}
                      aria-label="Start"
                    />
                    <Input
                      type="datetime-local"
                      value={rangeEndLocal}
                      onChange={(event) => {
                        const next = event.target.value;
                        setRangeEndLocal(next);
                        const startIso = parseDateTimeInputValueToIso(rangeStartLocal, timeZone);
                        const endIso = parseDateTimeInputValueToIso(next, timeZone);
                        if (!startIso || !endIso) {
                          setRangeError("Enter a valid start and end date/time.");
                          return;
                        }
                        const start = new Date(startIso);
                        const end = new Date(endIso);
                        if (!(start < end)) {
                          setRangeError("Start must be before end.");
                          return;
                        }
                        const hours = (end.getTime() - start.getTime()) / (60 * 60 * 1000);
                        if (hours > 8760) {
                          setRangeError("Max range is 365d.");
                          return;
                        }
                        setRangeError(null);
                        setRangeHours(hours);
                        if (intervalSelect !== "custom") {
                          const rec = recommendedIntervalSeconds(hours);
                          setInterval(rec);
                          setIntervalSelect(String(rec));
                          setIntervalDraft(String(rec));
                        }
                      }}
                      aria-label="End"
                    />
                  </div>
                  {rangeError ? (
 <p className="mt-1 text-xs text-rose-600">{rangeError}</p>
                  ) : (
 <p className="mt-1 text-xs text-muted-foreground">
                      Used for historic analysis; interpreted in controller/site time.
                    </p>
                  )}
                </div>
              ) : null}

              {intervalSelect === "custom" ? (
                <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Custom interval (seconds/minutes/hours/days)
                  </label>
                  <Input
                    value={intervalDraft}
                    onChange={(event) => {
                      const nextDraft = event.target.value;
                      setIntervalDraft(nextDraft);
                      const parsed = parseDurationToSeconds(nextDraft);
                      if (parsed == null) {
                        setIntervalError("Enter a duration like 60s, 10m, 1h.");
                        return;
                      }
                      const seconds = Math.round(parsed);
                      if (seconds < 1) {
                        setIntervalError("Min interval is 1s.");
                        return;
                      }
                      if (seconds > 86400 * 30) {
                        setIntervalError("Max interval is 30d.");
                        return;
                      }
                      setIntervalError(null);
                      setInterval(seconds);
                    }}
                    className="mt-1"
                    placeholder="e.g. 15m"
                  />
                  {intervalError ? (
 <p className="mt-1 text-xs text-rose-600">{intervalError}</p>
                  ) : (
 <p className="mt-1 text-xs text-muted-foreground">
                      Supports suffixes: <code className="px-1">s</code>, <code className="px-1">m</code>,{" "}
                      <code className="px-1">h</code>, <code className="px-1">d</code>.
                    </p>
                  )}
                </div>
              ) : null}
            </div>

 <p className="mt-2 text-xs text-muted-foreground">
              Interval buckets average all samples (not downsampled).
            </p>

            <CollapsibleCard title="Advanced: Savitzky–Golay" className="bg-card-inset shadow-xs" defaultOpen={false}>
              <div className="space-y-3">
 <p className="text-sm text-muted-foreground">
                  Savitzky–Golay (SG) fits a polynomial to each moving window. It smooths noise while preserving
                  peaks/edges better than a simple moving average. Derivatives show rate-of-change.
                </p>

                <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Window length (odd)
                    <Input
                      type="number"
                      min={3}
                      step={2}
                      value={savgolWindowLength}
                      onChange={(event) => {
                        const next = Number(event.target.value);
                        if (!Number.isFinite(next)) return;
                        let safe = Math.max(3, Math.min(999, Math.floor(next)));
                        if (safe % 2 === 0) safe = Math.min(999, safe + 1);
                        setSavgolWindowLength(safe);
                        if (savgolPolyOrder >= safe) setSavgolPolyOrder(Math.max(0, safe - 1));
                      }}
                      className="mt-1"
                    />
                  </label>

 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Polynomial degree
                    <Input
                      type="number"
                      min={0}
                      max={Math.max(0, savgolWindowLength - 1)}
                      step={1}
                      value={savgolPolyOrder}
                      onChange={(event) => {
                        const next = Number(event.target.value);
                        if (!Number.isFinite(next)) return;
                        const safe = Math.max(0, Math.min(savgolWindowLength - 1, Math.floor(next)));
                        setSavgolPolyOrder(safe);
                        if (savgolDerivOrder > safe) setSavgolDerivOrder(safe);
                      }}
                      className="mt-1"
                    />
                  </label>

 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Derivative order
                    <Input
                      type="number"
                      min={0}
                      max={savgolPolyOrder}
                      step={1}
                      value={savgolDerivOrder}
                      onChange={(event) => {
                        const next = Number(event.target.value);
                        if (!Number.isFinite(next)) return;
                        setSavgolDerivOrder(Math.max(0, Math.min(savgolPolyOrder, Math.floor(next))));
                      }}
                      className="mt-1"
                    />
                  </label>

 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Edge mode
                    <Select
                      className="mt-1"
                      value={savgolEdgeMode}
                      onChange={(event) => setSavgolEdgeMode(event.target.value as SavitzkyGolayEdgeMode)}
                    >
                      <option value="interp">Interp (recommended)</option>
                      <option value="nearest">Nearest</option>
                      <option value="mirror">Mirror</option>
                      <option value="clip">Clip (leave edges blank)</option>
                    </Select>
                  </label>

 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Derivative units
                    <Select
                      className="mt-1"
                      value={savgolDerivativeUnit}
                      onChange={(event) => setSavgolDerivativeUnit(event.target.value as "s" | "min" | "h")}
                    >
                      <option value="s">per second</option>
                      <option value="min">per minute</option>
                      <option value="h">per hour</option>
                    </Select>
                  </label>
                </div>

                {savgolError ? (
                  <InlineBanner tone="error">{savgolError}</InlineBanner>
                ) : (
 <p className="text-xs text-muted-foreground">
                    Δt is derived from the selected Interval: <span className="font-semibold">{formatIntervalLabel(interval)}</span>{" "}
                    (Δt = <span className="font-semibold">{savgolDelta.toFixed(3)}</span> {savgolDerivativeUnit}).
                  </p>
                )}

 <p className="text-xs text-muted-foreground">
                  Notes: SG is applied to the Trend chart only (visualization). CSV export uses the raw bucketed series.
                </p>
              </div>
            </CollapsibleCard>

            <AnalysisKey
              summary="Key"
              overview={
                <>
                  <span className="font-semibold">Range</span> chooses the time window;{" "}
                  <span className="font-semibold">Interval</span> chooses the bucket size. Use{" "}
                  <span className="font-semibold">Independent axes</span> for mixed units.
                </>
              }
            >
              <div className="space-y-3">
                <div>
 <p className="text-xs font-semibold text-foreground">Range + interval</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                    <li>
                      <code>Range</code> selects the time window. <code>Custom</code> uses controller/site time inputs.
                    </li>
                    <li>
                      <code>Interval</code> is the bucket size; each point is the average of all samples in that bucket.
                      Smaller buckets show spikes; larger buckets smooth them out.
                    </li>
                    <li>
                      When you change Range, Interval auto-adjusts to a recommended value (unless you set Interval to{" "}
                      <code>Custom</code>).
                    </li>
                  </ul>
                </div>

                <div>
 <p className="text-xs font-semibold text-foreground">Axes + scaling</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                    <li>
                      <code>Stacked</code> is best for same-unit series (sums values). Don’t stack mixed units.
                    </li>
                    <li>
                      <code>Independent axes</code> is best for mixed units. Hover the legend to show a series’ y-axis
                      labels.
                    </li>
                    <li>
                      <code>Y min</code>/<code>Y max</code> clamp the chart; leave blank for auto.
                    </li>
                  </ul>
                </div>

                <div>
 <p className="text-xs font-semibold text-foreground">
                    Savitzky–Golay (SG) smoothing
                  </p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                    <li>
                      <code>Savitzky–Golay</code> is an opt-in filter that smooths noise while preserving peaks/edges.
                    </li>
                    <li>
                      Use <code>Derivative order</code> 1 to see rate-of-change (units become <code>unit/{savgolDerivativeUnit}</code>).
                    </li>
                    <li>
                      SG never fills in missing data; gaps remain gaps.
                    </li>
                  </ul>
                </div>

                <div>
 <p className="text-xs font-semibold text-foreground">Chart height</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                    <li>
                      <code>Chart height</code> changes readability (taller = easier to inspect). This is saved locally
                      in your browser.
                    </li>
                  </ul>
                </div>

                <div>
 <p className="text-xs font-semibold text-foreground">Chart interactions</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                    <li>Scroll / trackpad / pinch to zoom the x-axis; drag to pan.</li>
                    <li>Double-click the chart to reset zoom.</li>
                  </ul>
                </div>
              </div>
            </AnalysisKey>
          </CollapsibleCard>

          {disabledNotice ? (
            <InlineBanner tone="info" className="mb-2">
              {disabledNotice}
            </InlineBanner>
          ) : null}
          {metricsLoading && effectiveSelected.length > 0 && chartSeries.length === 0 ? (
            <LoadingState label="Loading metrics..." />
          ) : metricsError ? (
            <ErrorState
              message={
                metricsError instanceof Error
                  ? metricsError.message
                  : "Unable to load metrics."
              }
            />
          ) : (
            <>
            <TrendChart
              title="Trend chart"
              description={
                <>
                  Bucketed values over time for your selected sensors. Each point is the average within an{" "}
                  <span className="font-semibold">Interval</span> bucket.
                </>
              }
              footer={
                <AnalysisKey
                  summary="Key"
                  overview={
                    <>
                      Each point is an <span className="font-semibold">Interval</span>-bucket average; gaps usually mean
                      missing data. Hover for exact values; zoom/pan is enabled.
                    </>
                  }
                >
                  <div className="space-y-3">
                    <div>
 <p className="text-xs font-semibold text-foreground">How to interpret</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                        <li>
                          Smaller <span className="font-semibold">Interval</span> shows sharper spikes; larger intervals
                          smooth noise.
                        </li>
                        {savgolEnabled ? (
                          <li>
                            <span className="font-semibold">Savitzky–Golay</span> is enabled, so the plotted values are
                            filtered (smoothed or derivative). Toggle it off to view raw bucket averages. CSV export
                            remains raw.
                          </li>
                        ) : null}
                        <li>Gaps usually mean missing data (the system does not &ldquo;fill in&rdquo; values).</li>
                        <li>Hover for exact values; tooltips include units and respect configured decimals.</li>
                        <li>
                          Each point represents one time <span className="font-semibold">bucket</span>; the x-axis labels
                          line up to the bucket timestamps (not &ldquo;between&rdquo; points).
                        </li>
                      </ul>
                    </div>

                    <div>
 <p className="text-xs font-semibold text-foreground">Axes</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                        <li>
                          Use <code>Independent axes</code> when mixing units (e.g., °C and W). Hover legend items to
                          reveal the matching y-axis.
                        </li>
                        <li>
                          Use <code>Stack</code> only for same-unit series (it sums values).
                        </li>
                      </ul>
                    </div>

                    <div>
 <p className="text-xs font-semibold text-foreground">Chart controls</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
                        <li>Scroll / trackpad / pinch to zoom the x-axis; drag to pan.</li>
                        <li>Double-click to reset zoom.</li>
                        <li>
                          Use the Chart analysis toolbar for drawing, measure tools, labels, and navigation modes.
                        </li>
                      </ul>
                    </div>
                  </div>
                </AnalysisKey>
              }
              data={chartSeries}
              ephemeralMarkers={effectiveSelected.length > 0 && !metricsError ? ephemeralMarkers : []}
              persistentAnnotations={persistentHcAnnotations}
              persistedBestFits={persistedBestFits}
              onMarkerClick={handleMarkerClick}
              onCreatePersistentAnnotation={handleCreatePersistentAnnotation}
              onUpdatePersistentAnnotation={handleUpdatePersistentAnnotation}
              onDeletePersistentAnnotation={handleDeletePersistentAnnotation}
              timeZone={timeZone}
              stacked={stacked}
              independentAxes={independentAxes}
              yDomain={computeDomain(yMin, yMax)}
              heightPx={chartHeightPx}
              savitzkyGolay={savgolConfig}
              analysisTools
            />
            {/* Ephemeral marker promote/dismiss popover */}
            {activePopoverMarkerId ? (() => {
              const marker = ephemeralMarkers.find((m) => m.id === activePopoverMarkerId);
              if (!marker) return null;
              return (
                <Card className="mt-2 gap-0 p-4 shadow-md">
                  <div className="flex items-start justify-between gap-4">
                    <div className="min-w-0">
                      <p className="text-sm font-semibold text-card-foreground">
                        {marker.label}
                      </p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        Source: {marker.source.replace(/_/g, " ")}
                        {marker.detail ? ` — ${marker.detail}` : ""}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => setActivePopoverMarkerId(null)}
                      className="text-muted-foreground hover:text-foreground"
                    >
                      ×
                    </button>
                  </div>
                  <div className="mt-3 flex items-end gap-2">
                    <Input
                      type="text"
                      value={promoteNote}
                      onChange={(e) => setPromoteNote(e.target.value)}
                      placeholder="Add a note (optional)…"
                      className="flex-1"
                    />
                    <NodeButton
                      variant="primary"
                      onClick={() => promoteMarker(marker.id, promoteNote || undefined)}
                    >
                      Save as annotation
                    </NodeButton>
                    <NodeButton
                      onClick={() => dismissMarker(marker.id)}
                    >
                      Dismiss
                    </NodeButton>
                  </div>
                </Card>
              );
            })() : null}
            </>
          )}

          {/* Relationship Finder - unified analysis panel */}
          {effectiveSelected.length > 0 && !metricsError ? (
            <RelationshipFinderPanel
              nodesById={nodesById}
              sensors={sensors}
              series={relationshipSeries}
              selectedBadges={selectedBadges.map(({ sensorId, label, color, hasData }) => ({
                sensorId,
                label,
                color,
                hasData,
              }))}
              selectedSensorIds={effectiveSelected}
              labelMap={labelMap}
              intervalSeconds={interval}
              rangeHours={rangeHours}
              rangeSelect={rangeSelect}
              customStartIso={customStartIso}
              customEndIso={customEndIso}
              customRangeValid={customRangeValid}
              timeZone={timeZone}
              maxSeries={MAX_SERIES}
              externalFocus={relatedSensorsExternalFocus}
              onClearExternalFocus={() => setRelatedSensorsExternalFocus(null)}
	              onAddToChart={(sensorId) => {
	                setSelected((prev) => {
	                  if (prev.includes(sensorId)) return prev;
	                  if (prev.length >= MAX_SERIES) return prev;
	                  return [...prev, sensorId];
	                });
	              }}
	              onAddEphemeralMarkers={addEphemeralMarkers}
	              onJumpToTimestamp={jumpToTimestamp}
	            />
	          ) : null}

          {effectiveSelected.length > 0 && !metricsError ? (
            <SelectedSensorsCorrelationMatrixCard
              selectedSensorIds={effectiveSelected}
              labelMap={labelMap}
              intervalSeconds={interval}
              rangeHours={rangeHours}
              rangeSelect={rangeSelect}
              customStartIso={customStartIso}
              customEndIso={customEndIso}
              customRangeValid={customRangeValid}
            />
          ) : null}

          {effectiveSelected.length > 0 && chartSeries.length > 0 && !metricsError ? (
            <MatrixProfilePanel
              series={chartSeries}
              selectedBadges={selectedBadges.map(({ axisSide: _axisSide, ...rest }) => rest)}
              intervalSeconds={interval}
              rangeHours={rangeHours}
              rangeSelect={rangeSelect}
              customStartIso={customStartIso}
              customEndIso={customEndIso}
              customRangeValid={customRangeValid}
              timeZone={timeZone}
              onAddEphemeralMarkers={addEphemeralMarkers}
              onSendFocusEvents={setRelatedSensorsExternalFocus}
              activeExternalFocus={relatedSensorsExternalFocus}
            />
          ) : null}

        </section>
      </div>
    </div>
  );
}
