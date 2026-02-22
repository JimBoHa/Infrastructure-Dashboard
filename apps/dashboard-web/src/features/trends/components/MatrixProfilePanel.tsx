"use client";

import clsx from "clsx";
import { useEffect, useMemo, useRef, useState } from "react";
import type { Options, SeriesLineOptions, SeriesScatterOptions, SeriesAreaOptions } from "highcharts";
import { HighchartsPanel, type HighchartsChartRef } from "@/components/charts/HighchartsPanel";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import InlineBanner from "@/components/InlineBanner";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import { formatNumber } from "@/lib/format";
import { browserTimeZone, formatChartTickTime, formatChartTooltipTime } from "@/lib/siteTime";
import type { TrendSeriesEntry } from "@/types/dashboard";
import CollapsibleCard from "@/components/CollapsibleCard";
import {
  cancelAnalysisJob,
  createAnalysisJob,
  fetchAnalysisJob,
  fetchAnalysisJobEvents,
  fetchAnalysisJobResult,
} from "@/lib/api";
import {
  type MatrixProfileJobParamsV1,
  type MatrixProfileResultV1,
  type MatrixProfileWindowV1,
} from "@/types/analysis";
import AnalysisKey from "@/features/trends/components/AnalysisKey";
import type { EphemeralMarker } from "@/types/chartMarkers";
import { createHeatmapOptions } from "@/lib/chartFactories";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import type { RelatedSensorsExternalFocus } from "@/features/trends/types/relatedSensorsFocus";

type SelectedBadge = {
  sensorId: string;
  label: string;
  color: string;
  hasData: boolean;
};

type TabKey = "anomalies" | "motifs" | "similarity";

type HeatmapState = {
  indices: number[];
  distances: Float32Array;
  min: number;
  max: number;
  window: number;
};

type WindowRange = { startIso: string; endIso: string };
type WindowSummary = {
  idx: number;
  dist: number;
  match: number | null;
  startIso: string | null;
  endIso: string | null;
  matchStartIso: string | null;
  matchEndIso: string | null;
};

function toDate(value: unknown): Date | null {
  if (value instanceof Date) return Number.isFinite(value.getTime()) ? value : null;
  if (typeof value === "string" || typeof value === "number") {
    const date = new Date(value);
    return Number.isFinite(date.getTime()) ? date : null;
  }
  return null;
}

function clampInt(value: number, min: number, max: number): number {
  const v = Number.isFinite(value) ? Math.floor(value) : min;
  return Math.max(min, Math.min(max, v));
}

function median(values: number[]): number | null {
  const sorted = values.filter(Number.isFinite).slice().sort((a, b) => a - b);
  if (!sorted.length) return null;
  const mid = Math.floor(sorted.length / 2);
  if (sorted.length % 2 === 1) return sorted[mid]!;
  return (sorted[mid - 1]! + sorted[mid]!) / 2;
}

function medianAbsoluteDeviation(values: number[], med: number): number | null {
  const deviations = values
    .filter(Number.isFinite)
    .map((v) => Math.abs(v - med))
    .filter(Number.isFinite);
  return median(deviations);
}

function zNormalize(values: number[]): number[] {
  const n = values.length;
  if (!n) return [];
  let sum = 0;
  let sumSq = 0;
  for (const v of values) {
    sum += v;
    sumSq += v * v;
  }
  const mean = sum / n;
  const variance = Math.max(0, sumSq / n - mean * mean);
  const std = Math.sqrt(variance);
  if (!Number.isFinite(std) || std <= 1e-12) {
    return Array.from({ length: n }, () => 0);
  }
  const inv = 1 / std;
  return values.map((v) => (v - mean) * inv);
}

function distanceZNorm(a: number[], b: number[]): number {
  if (a.length !== b.length || a.length === 0) return Number.POSITIVE_INFINITY;
  let sumSq = 0;
  for (let i = 0; i < a.length; i += 1) {
    const d = a[i]! - b[i]!;
    sumSq += d * d;
  }
  return Math.sqrt(sumSq);
}

function formatApproxDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "—";
  if (seconds >= 86400) {
    return `${formatNumber(seconds / 86400, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}d`;
  }
  if (seconds >= 3600) {
    return `${formatNumber(seconds / 3600, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}h`;
  }
  if (seconds >= 60) {
    return `${formatNumber(seconds / 60, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}m`;
  }
  return `${formatNumber(seconds, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}s`;
}

function formatWindowRange(
  startIso: string | null,
  endIso: string | null,
  timeZone: string,
): string | null {
  const start = startIso ? toDate(startIso) : null;
  if (!start) return null;
  const end = endIso ? toDate(endIso) : null;
  const startLabel = formatChartTooltipTime(start, timeZone);
  if (!end) return startLabel || null;
  const endLabel = formatChartTooltipTime(end, timeZone);
  if (!endLabel) return startLabel || null;
  return `${startLabel} → ${endLabel}`;
}

function normalizeWindowSummary(window: MatrixProfileWindowV1): WindowSummary | null {
  const idx = Number(window.window_index);
  const dist = Number(window.distance);
  if (!Number.isFinite(idx) || !Number.isFinite(dist)) return null;
  const matchRaw = window.match_index;
  const match =
    typeof matchRaw === "number" && Number.isFinite(matchRaw) && matchRaw >= 0 ? matchRaw : null;
  const startIso = typeof window.start_ts === "string" && window.start_ts.trim() ? window.start_ts : null;
  const endIso = typeof window.end_ts === "string" && window.end_ts.trim() ? window.end_ts : null;
  const matchStartIso =
    typeof window.match_start_ts === "string" && window.match_start_ts.trim() ? window.match_start_ts : null;
  const matchEndIso =
    typeof window.match_end_ts === "string" && window.match_end_ts.trim() ? window.match_end_ts : null;
  return {
    idx: Math.floor(idx),
    dist,
    match,
    startIso,
    endIso,
    matchStartIso,
    matchEndIso,
  };
}


export default function MatrixProfilePanel({
  series,
  selectedBadges,
  intervalSeconds,
  rangeHours,
  rangeSelect,
  customStartIso,
  customEndIso,
  customRangeValid,
  timeZone,
  onAddEphemeralMarkers,
  onSendFocusEvents,
  activeExternalFocus,
}: {
  series: TrendSeriesEntry[];
  selectedBadges: SelectedBadge[];
  intervalSeconds: number;
  rangeHours: number;
  rangeSelect: string;
  customStartIso: string | null;
  customEndIso: string | null;
  customRangeValid: boolean;
  timeZone?: string;
  onAddEphemeralMarkers?: (markers: EphemeralMarker[]) => void;
  onSendFocusEvents?: (focus: RelatedSensorsExternalFocus) => void;
  activeExternalFocus?: RelatedSensorsExternalFocus | null;
}) {
  const effectiveTimeZone = timeZone ?? browserTimeZone();
  const queryClient = useQueryClient();
  const badgeById = useMemo(() => new Map(selectedBadges.map((b) => [b.sensorId, b])), [selectedBadges]);
  const seriesWithData = useMemo(() => series.filter((entry) => (entry.points?.length ?? 0) > 0), [series]);
  const selectableIds = useMemo(() => seriesWithData.map((s) => s.sensor_id), [seriesWithData]);

  const [tab, setTab] = useState<TabKey>("anomalies");
  const [sensorSelection, setSensorSelection] = useState<string | null>(null);
  const [maxPoints, setMaxPoints] = useState<number>(512);
  const [windowPoints, setWindowPoints] = useState<number>(() => {
    const approx = intervalSeconds > 0 ? Math.round(1800 / intervalSeconds) : 24;
    return clampInt(approx, 8, 96);
  });
  const [focusStart, setFocusStart] = useState<number | null>(null);
  const [pairOverride, setPairOverride] = useState<{ a: number; b: number } | null>(null);
  const [heatmapTarget, setHeatmapTarget] = useState<number>(84);
  const [analysisWindow, setAnalysisWindow] = useState<WindowRange | null>(null);
  const [jobId, setJobId] = useState<string | null>(null);
  const [jobRequestedAt, setJobRequestedAt] = useState<Date | null>(null);
  const [runError, setRunError] = useState<string | null>(null);
  const [focusSendNotice, setFocusSendNotice] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState<boolean>(false);

  const activeSensorId = useMemo(() => {
    if (sensorSelection && selectableIds.includes(sensorSelection)) return sensorSelection;
    return selectableIds[0] ?? null;
  }, [sensorSelection, selectableIds]);

  const activeSeries = useMemo(() => {
    if (!activeSensorId) return null;
    return seriesWithData.find((s) => s.sensor_id === activeSensorId) ?? null;
  }, [activeSensorId, seriesWithData]);

  const canComputeWindow = useMemo(() => {
    if (rangeSelect === "custom") return Boolean(customStartIso && customEndIso && customRangeValid);
    return Number.isFinite(rangeHours) && rangeHours > 0;
  }, [customEndIso, customRangeValid, customStartIso, rangeHours, rangeSelect]);

  const computeFallbackWindow = (): WindowRange | null => {
    if (!canComputeWindow) return null;
    if (rangeSelect === "custom" && customStartIso && customEndIso) {
      return { startIso: customStartIso, endIso: customEndIso };
    }
    const end = new Date();
    end.setMilliseconds(0);
    const start = new Date(end.getTime() - rangeHours * 60 * 60 * 1000);
    return { startIso: start.toISOString(), endIso: end.toISOString() };
  };

  const computeWindowFromSeries = (): WindowRange | null => {
    if (!activeSeries || !activeSeries.points?.length) return null;
    const startPoint = activeSeries.points[0]?.timestamp;
    const endPoint = activeSeries.points[activeSeries.points.length - 1]?.timestamp;
    const start = toDate(startPoint);
    const end = toDate(endPoint);
    if (!start || !end) return null;
    if (!Number.isFinite(start.getTime()) || !Number.isFinite(end.getTime())) return null;
    if (!(start < end)) return null;
    return { startIso: start.toISOString(), endIso: end.toISOString() };
  };

  const jobStatusQuery = useQuery({
    queryKey: ["analysis", "jobs", jobId ?? "none"],
    queryFn: () => fetchAnalysisJob(jobId as string),
    enabled: Boolean(jobId),
    refetchInterval: (query) => {
      const status = query.state.data?.job.status;
      if (!status) return 2000;
      return status === "completed" || status === "failed" || status === "canceled" ? false : 2000;
    },
  });

  const job = jobStatusQuery.data?.job ?? null;

  const jobEventsQuery = useQuery({
    queryKey: ["analysis", "jobs", jobId ?? "none", "events"],
    queryFn: () => fetchAnalysisJobEvents(jobId as string, { limit: 20 }),
    enabled: Boolean(jobId),
    refetchInterval: job?.status === "running" || job?.status === "pending" ? 2000 : false,
    staleTime: 10_000,
  });

  const latestEvent = useMemo(() => {
    const events = jobEventsQuery.data?.events ?? [];
    if (!events.length) return null;
    return events.reduce((latest, event) => (event.id > latest.id ? event : latest), events[0]!);
  }, [jobEventsQuery.data?.events]);

  const latestEventLabel = useMemo(() => {
    if (!latestEvent) return null;
    const payload = latestEvent.payload;
    if (payload && typeof payload === "object" && "message" in payload) {
      const message = (payload as { message?: unknown }).message;
      if (typeof message === "string" && message.trim()) return message;
    }
    return latestEvent.kind?.replace(/_/g, " ") ?? null;
  }, [latestEvent]);

  const jobResultQuery = useQuery({
    queryKey: ["analysis", "jobs", jobId ?? "none", "result"],
    queryFn: () => fetchAnalysisJobResult<MatrixProfileResultV1>(jobId as string),
    enabled: job?.status === "completed",
    staleTime: 60_000,
  });

  const result = jobResultQuery.data?.result ?? null;

  const computedThroughLabel = useMemo(() => {
    if (!result?.computed_through_ts) return null;
    const ts = new Date(result.computed_through_ts);
    if (!Number.isFinite(ts.getTime())) return result.computed_through_ts;
    return formatChartTooltipTime(ts, effectiveTimeZone);
  }, [effectiveTimeZone, result?.computed_through_ts]);

  const analysis = useMemo(() => {
    if (!activeSeries) {
      return {
        ok: false,
        reason: "Select at least one sensor with data to run Matrix Profile analysis.",
      } as const;
    }

    if (!result || result.sensor_id !== activeSeries.sensor_id) {
      return {
        ok: false,
        reason: "Run the analysis job to generate Matrix Profile results for this sensor.",
      } as const;
    }

    const timestampsRaw = Array.isArray(result.timestamps) ? result.timestamps : [];
    const valuesRaw = Array.isArray(result.values) ? result.values : [];
    const length = Math.min(timestampsRaw.length, valuesRaw.length);
    if (length < 16) {
      return { ok: false, reason: "Not enough points returned for analysis." } as const;
    }

    const timestamps = timestampsRaw.slice(0, length).map((ts) => toDate(ts) ?? new Date(ts));
    const values = valuesRaw.slice(0, length);
    const profile = result.profile ?? [];
    const profileIndex = result.profile_index ?? [];
    const window = Number.isFinite(result.window) ? result.window : null;
    if (!window || window < 4) {
      return { ok: false, reason: "Analysis returned an invalid window size." } as const;
    }

    if (values.length - window + 1 < 4) {
      return {
        ok: false,
        reason: "Not enough points for the chosen window size; increase range or lower the window.",
      } as const;
    }

    const windowStartRaw = Array.isArray(result.window_start_ts) ? result.window_start_ts : [];
    const windowStarts = (windowStartRaw.length ? windowStartRaw : timestamps)
      .slice(0, profile.length)
      .map((ts) => toDate(ts) ?? new Date(ts));

    const profileFinite = profile.filter((v) => Number.isFinite(v));
    const distMin = profileFinite.length ? Math.min(...profileFinite) : null;
    const distMax = profileFinite.length ? Math.max(...profileFinite) : null;
    const distMedian = profileFinite.length ? median(profileFinite) : null;
    const mad = distMedian != null ? medianAbsoluteDeviation(profileFinite, distMedian) : null;
    const robustScale = mad != null && mad > 1e-9 ? mad * 1.4826 : null;

    const withinProfile = (idx: number) => idx >= 0 && idx < profile.length;
    const motifsRaw = Array.isArray(result.motifs) ? result.motifs : [];
    const anomaliesRaw = Array.isArray(result.anomalies) ? result.anomalies : [];
    const motifs = motifsRaw
      .map(normalizeWindowSummary)
      .filter((entry): entry is WindowSummary => Boolean(entry && withinProfile(entry.idx)));
    const anomalies = anomaliesRaw
      .map(normalizeWindowSummary)
      .filter((entry): entry is WindowSummary => Boolean(entry && withinProfile(entry.idx)));

    const motifPairs = motifs
      .map((entry) => {
        if (entry.match == null || !withinProfile(entry.match)) return null;
        return {
          a: entry.idx,
          b: entry.match,
          dist: entry.dist,
          startIso: entry.startIso,
          endIso: entry.endIso,
          matchStartIso: entry.matchStartIso,
          matchEndIso: entry.matchEndIso,
        };
      })
      .filter(
        (
          entry,
        ): entry is {
          a: number;
          b: number;
          dist: number;
          startIso: string | null;
          endIso: string | null;
          matchStartIso: string | null;
          matchEndIso: string | null;
        } => Boolean(entry),
      );

    const focusDefault = anomalies[0]?.idx ?? motifPairs[0]?.a ?? 0;
    const focusIdx = focusStart != null && focusStart >= 0 && focusStart < profile.length ? focusStart : focusDefault;
    const matchIdx = profileIndex[focusIdx] ?? -1;

    const effectivePair = (() => {
      if (pairOverride) {
        const { a, b } = pairOverride;
        if (a >= 0 && b >= 0 && a < profile.length && b < profile.length) return pairOverride;
      }
      if (matchIdx >= 0 && matchIdx < profile.length) return { a: focusIdx, b: matchIdx };
      return null;
    })();

    const computeRobustZ = (dist: number) => {
      if (distMedian == null || robustScale == null) return null;
      return (dist - distMedian) / robustScale;
    };

    const focusDist = profile[focusIdx] ?? null;
    const focusRobustZ = focusDist != null && Number.isFinite(focusDist) ? computeRobustZ(focusDist) : null;

    return {
      ok: true,
      sensorId: result.sensor_id,
      sensorLabel: result.sensor_label ?? activeSeries.label ?? activeSeries.sensor_id,
      unit: result.unit ?? activeSeries.unit ?? "",
      step: result.step ?? null,
      effectiveIntervalSeconds: result.effective_interval_seconds ?? intervalSeconds,
      timestamps,
      windowStarts,
      values,
      window,
      profile,
      profileIndex,
      exclusionZone: Number.isFinite(result.exclusion_zone) ? result.exclusion_zone : Math.floor(window / 2),
      distMin,
      distMax,
      distMedian,
      robustScale,
      anomalies,
      motifPairs,
      focusIdx,
      focusDist,
      focusRobustZ,
      focusPair: effectivePair,
    } as const;
  }, [activeSeries, focusStart, intervalSeconds, pairOverride, result]);

  // Emit ephemeral markers to the main TrendChart when analysis completes
  useEffect(() => {
    if (!analysis?.ok || !onAddEphemeralMarkers) return;
    const markers: EphemeralMarker[] = [];

    // Emit top anomaly timestamps
    for (const anomaly of analysis.anomalies.slice(0, 5)) {
      const iso = anomaly.startIso;
      if (!iso) continue;
      const ts = new Date(iso);
      if (!Number.isFinite(ts.getTime())) continue;
      markers.push({
        id: `mp-anomaly-${analysis.sensorId}-${anomaly.idx}`,
        timestamp: ts,
        label: `Anomaly (MP)`,
        source: "matrix_profile",
        detail: `dist=${anomaly.dist.toFixed(2)}, idx=${anomaly.idx}`,
        sensorIds: [analysis.sensorId],
      });
    }

    // Emit motif occurrence timestamps
    for (const pair of analysis.motifPairs.slice(0, 3)) {
      const iso = pair.startIso;
      if (!iso) continue;
      const ts = new Date(iso);
      if (!Number.isFinite(ts.getTime())) continue;
      markers.push({
        id: `mp-motif-${analysis.sensorId}-${pair.a}`,
        timestamp: ts,
        label: `Motif (MP)`,
        source: "matrix_profile",
        detail: `dist=${pair.dist.toFixed(2)}, pair=[${pair.a},${pair.b}]`,
        sensorIds: [analysis.sensorId],
      });
    }

    if (markers.length > 0) onAddEphemeralMarkers(markers);
  }, [analysis, onAddEphemeralMarkers]);

  const summaryWindow = useMemo<WindowRange | null>(() => {
    if (analysisWindow) return analysisWindow;
    if (result) return { startIso: result.params.start, endIso: result.params.end };
    return null;
  }, [analysisWindow, result]);

  const activeFocusWindowStartSet = useMemo(() => {
    if (!analysis.ok) return new Set<string>();
    if (!activeExternalFocus) return new Set<string>();
    if (activeExternalFocus.source !== "matrix_profile") return new Set<string>();
    if (activeExternalFocus.focusSensorId !== analysis.sensorId) return new Set<string>();
    return new Set(activeExternalFocus.windows.map((window) => window.startIso));
  }, [activeExternalFocus, analysis]);

  const handleSendFocusEvents = () => {
    if (!analysis.ok) {
      setFocusSendNotice("Run Pattern Detector first to identify anomalies or motifs.");
      return;
    }
    if (!onSendFocusEvents) {
      setFocusSendNotice("Related Sensors panel not available in this view.");
      return;
    }

    const windows: RelatedSensorsExternalFocus["windows"] = [];
    const distMedian = analysis.distMedian;
    const robustScale = analysis.robustScale;
    const robustZ =
      distMedian != null && robustScale != null && robustScale > 1e-12
        ? (dist: number) => (dist - distMedian) / robustScale
        : null;

    if (tab === "motifs") {
      for (const pair of analysis.motifPairs.slice(0, 5)) {
        if (pair.startIso) {
          windows.push({
            kind: "motif",
            startIso: pair.startIso,
            endIso: pair.endIso ?? null,
            severity: 1,
          });
        }
        if (pair.matchStartIso) {
          windows.push({
            kind: "motif",
            startIso: pair.matchStartIso,
            endIso: pair.matchEndIso ?? null,
            severity: 1,
          });
        }
      }
    } else {
      for (const anomaly of analysis.anomalies.slice(0, 12)) {
        if (!anomaly.startIso) continue;
        const z = robustZ ? robustZ(anomaly.dist) : null;
        const severity =
          z != null && Number.isFinite(z) && z > 0
            ? Math.max(1, Math.min(25, z))
            : 1;
        windows.push({
          kind: "anomaly",
          startIso: anomaly.startIso,
          endIso: anomaly.endIso ?? null,
          severity,
        });
      }
    }

    const deduped = new Map<string, RelatedSensorsExternalFocus["windows"][number]>();
    for (const window of windows) {
      deduped.set(window.startIso, window);
    }
    const stableWindows = Array.from(deduped.values()).sort((a, b) =>
      a.startIso.localeCompare(b.startIso),
    );
    if (stableWindows.length === 0) {
      setFocusSendNotice("No focus windows available to send.");
      return;
    }

    onSendFocusEvents({
      source: "matrix_profile",
      requestedAtMs: Date.now(),
      focusSensorId: analysis.sensorId,
      windows: stableWindows,
    });

    setFocusSendNotice(
      `Sent ${stableWindows.length} focus window${stableWindows.length === 1 ? "" : "s"} to Related Sensors.`,
    );
    globalThis.setTimeout(() => setFocusSendNotice(null), 4000);
  };

  const windowLabel = useMemo(() => {
    if (!summaryWindow) return null;
    const start = new Date(summaryWindow.startIso);
    const end = new Date(summaryWindow.endIso);
    if (!Number.isFinite(start.getTime()) || !Number.isFinite(end.getTime())) {
      return `${summaryWindow.startIso} → ${summaryWindow.endIso}`;
    }
    return `${formatChartTooltipTime(start, effectiveTimeZone)} → ${formatChartTooltipTime(end, effectiveTimeZone)}`;
  }, [effectiveTimeZone, summaryWindow]);

  const runAnalysis = async () => {
    if (!activeSeries) {
      setRunError("Select a sensor with data before running analysis.");
      return;
    }
    if (!customRangeValid && rangeSelect === "custom") {
      setRunError("Fix the custom range inputs before running analysis.");
      return;
    }
    const window = computeWindowFromSeries() ?? computeFallbackWindow();
    if (!window) {
      setRunError("Select a valid range or load more data before running analysis.");
      return;
    }
    const exclusionZone = clampInt(Math.floor(windowPoints / 2), 0, windowPoints);
    const topK = clampInt(8, 1, 20);
    const maxWindows = clampInt(maxPoints, 64, 4096);

    const params: MatrixProfileJobParamsV1 = {
      sensor_id: activeSeries.sensor_id,
      start: window.startIso,
      end: window.endIso,
      interval_seconds: intervalSeconds,
      max_points: maxPoints,
      window_points: windowPoints,
      exclusion_zone: exclusionZone,
      max_windows: maxWindows,
      top_k: topK,
    };

    const jobKey = JSON.stringify({
      v: 1,
      sensor: activeSeries.sensor_id,
      start: window.startIso,
      end: window.endIso,
      interval: intervalSeconds,
      maxPoints,
      windowPoints,
      exclusionZone,
      maxWindows,
      topK,
    });

    setRunError(null);
    setSubmitting(true);
    try {
      const response = await createAnalysisJob({
        job_type: "matrix_profile_v1",
        params,
        job_key: jobKey,
        dedupe: true,
      });
      setJobId(response.job.id);
      setAnalysisWindow(window);
      setJobRequestedAt(new Date());
      queryClient.setQueryData(["analysis", "jobs", response.job.id], response);
      queryClient.removeQueries({ queryKey: ["analysis", "jobs", response.job.id, "result"] });
    } catch (error) {
      setRunError(error instanceof Error ? error.message : "Failed to start matrix profile analysis.");
    } finally {
      setSubmitting(false);
    }
  };

  const handleCancel = async () => {
    if (!jobId) return;
    try {
      const response = await cancelAnalysisJob(jobId);
      queryClient.setQueryData(["analysis", "jobs", jobId], response);
    } catch (error) {
      setRunError(error instanceof Error ? error.message : "Failed to cancel analysis job.");
    }
  };

  const jobStatusLabel = job?.status ? job.status.replace(/_/g, " ") : null;
  const shouldShowProgress = job?.status === "running" || job?.status === "pending";
  const progressTotal = job?.progress.total ?? null;
  const progressCompleted = job?.progress.completed ?? 0;
  const runDisabled = submitting || shouldShowProgress || !activeSeries || !canComputeWindow;
  const profileWarnings = result?.warnings ?? [];
  const sourcePoints = result?.source_points ?? null;
  const sampledPoints = result?.sampled_points ?? null;
  const downsampled =
    sourcePoints != null && sampledPoints != null && sampledPoints > 0 && sampledPoints < sourcePoints;
  const fallbackPoints = analysis.ok ? analysis.values.length : 0;

  const profileChartRef = useRef<HighchartsChartRef | null>(null);
  const seriesChartRef = useRef<HighchartsChartRef | null>(null);
  // heatmap rendered via Highcharts (see heatmapChartOptions memo)

  // Profile chart options (Highcharts)
  const profileChartOptions: Options | null = useMemo(() => {
    if (!analysis.ok) return null;
    const starts = analysis.windowStarts.length
      ? analysis.windowStarts.slice(0, Math.max(0, analysis.profile.length))
      : analysis.timestamps.slice(0, Math.max(0, analysis.profile.length));

    const profileData = analysis.profile.map((dist, idx) => [
      (starts[idx] ?? analysis.timestamps[analysis.timestamps.length - 1]!).getTime(),
      Number.isFinite(dist) ? dist : null,
    ]) as [number, number | null][];

    const anomalyData = analysis.anomalies.map((a) => [
      (starts[a.idx] ?? analysis.timestamps[analysis.timestamps.length - 1]!).getTime(),
      a.dist,
    ]) as [number, number][];

    const focus = analysis.focusIdx;
    const focusData = [
      [
        (starts[focus] ?? analysis.timestamps[analysis.timestamps.length - 1]!).getTime(),
        Number.isFinite(analysis.profile[focus]) ? analysis.profile[focus] : null,
      ],
    ] as [number, number | null][];

    const baseColor = badgeById.get(analysis.sensorId)?.color ?? "#4f46e5";

    return {
      chart: {
        type: "line",
        zooming: { type: "x", mouseWheel: { enabled: true } },
        panning: { enabled: true, type: "x" },
        panKey: "shift",
      },
      xAxis: {
        type: "datetime",
        labels: {
          formatter: function () {
            return formatChartTickTime(this.value, effectiveTimeZone);
          },
        },
      },
      yAxis: {
        title: { text: "Distance" },
        min: analysis.distMin ?? undefined,
        max: analysis.distMax ?? undefined,
      },
      tooltip: {
        shared: true,
        formatter: function () {
          const x = this.x as number;
          const header = formatChartTooltipTime(x, effectiveTimeZone);
          let html = `<b>${header}</b><br/>`;
          this.points?.forEach((point) => {
            const y = point.y;
            const value = typeof y === "number" && Number.isFinite(y) ? formatNumber(y, { maximumFractionDigits: 3 }) : "—";
            html += `<span style="color:${point.color}">\u25CF</span> ${point.series.name}: <b>${value}</b><br/>`;
          });
          return html;
        },
      },
      legend: { align: "center", verticalAlign: "bottom" },
      navigator: { enabled: false },
      rangeSelector: { enabled: false },
      scrollbar: { enabled: false },
      plotOptions: {
        series: {
          cursor: "pointer",
          point: {
            events: {
              click: function () {
                const idx = this.index;
                if (typeof idx === "number") {
                  setFocusStart(idx);
                  setPairOverride(null);
                }
              },
            },
          },
        },
        area: {
          fillColor: {
            linearGradient: { x1: 0, y1: 0, x2: 0, y2: 1 },
            stops: [
              [0, baseColor + "66"],
              [1, baseColor + "00"],
            ],
          },
        },
      },
      series: [
        {
          type: "area",
          name: "Matrix profile (distance)",
          data: profileData,
          color: baseColor,
          lineWidth: 2,
          marker: { enabled: false, radius: 0 },
        } as SeriesAreaOptions,
        {
          type: "scatter",
          name: "Top anomalies",
          data: anomalyData,
          color: "#e11d48",
          marker: { enabled: true, radius: 4 },
        } as SeriesScatterOptions,
        {
          type: "scatter",
          name: "Selected",
          data: focusData,
          color: "#111827",
          marker: { enabled: true, radius: 5 },
        } as SeriesScatterOptions,
      ],
    };
  }, [analysis, badgeById, effectiveTimeZone]);

  // Highlight series chart options (Highcharts)
  const highlightSeriesChartOptions: Options | null = useMemo(() => {
    if (!analysis.ok) return null;
    const baseColor = badgeById.get(analysis.sensorId)?.color ?? "#4f46e5";
    const pair = analysis.focusPair;
    const a = pair?.a ?? null;
    const b = pair?.b ?? null;
    const window = analysis.window;

    const baseData = analysis.timestamps.map((ts, idx) => [
      ts.getTime(),
      analysis.values[idx] ?? null,
    ]) as [number, number | null][];

    const highlight = (start: number | null) => {
      if (start == null || start < 0) return analysis.timestamps.map((ts) => [ts.getTime(), null]) as [number, null][];
      return analysis.timestamps.map((ts, idx) => {
        if (idx < start || idx >= start + window) return [ts.getTime(), null];
        return [ts.getTime(), analysis.values[idx] ?? null];
      }) as [number, number | null][];
    };

    const windowA = highlight(a);
    const windowB = highlight(b);

    return {
      chart: {
        type: "line",
        zooming: { type: "x", mouseWheel: { enabled: true } },
        panning: { enabled: true, type: "x" },
        panKey: "shift",
      },
      xAxis: {
        type: "datetime",
        labels: {
          formatter: function () {
            return formatChartTickTime(this.value, effectiveTimeZone);
          },
        },
      },
      yAxis: {
        title: { text: analysis.unit || undefined },
      },
      tooltip: {
        shared: true,
        formatter: function () {
          const x = this.x as number;
          return formatChartTooltipTime(x, effectiveTimeZone);
        },
      },
      legend: { align: "center", verticalAlign: "bottom" },
      navigator: { enabled: false },
      rangeSelector: { enabled: false },
      scrollbar: { enabled: false },
      series: [
        {
          type: "line",
          name: "Series",
          data: baseData,
          color: "#94a3b8",
          lineWidth: 1.5,
          marker: { enabled: false },
        } as SeriesLineOptions,
        {
          type: "line",
          name: "Window A",
          data: windowA,
          color: baseColor,
          lineWidth: 3,
          marker: { enabled: false },
        } as SeriesLineOptions,
        {
          type: "line",
          name: "Window B",
          data: windowB,
          color: "#16a34a",
          lineWidth: 3,
          marker: { enabled: false },
        } as SeriesLineOptions,
      ],
    };
  }, [analysis, badgeById, effectiveTimeZone]);

  // Window overlay chart options (Highcharts)
  const windowOverlay = useMemo(() => {
    if (!analysis.ok || !analysis.focusPair) return null;
    const { a, b } = analysis.focusPair;
    const startA = Math.min(a, b);
    const startB = Math.max(a, b);
    const window = analysis.window;

    const sliceA = analysis.values.slice(startA, startA + window);
    const sliceB = analysis.values.slice(startB, startB + window);
    if (sliceA.length !== window || sliceB.length !== window) return null;

    const normA = zNormalize(sliceA);
    const normB = zNormalize(sliceB);
    const dist = distanceZNorm(normA, normB);

    const overlayOptions: Options = {
      chart: { type: "line" },
      xAxis: {
        title: { text: "Offset (points)" },
      },
      yAxis: {
        title: { text: "Z-score" },
      },
      tooltip: {
        shared: true,
        formatter: function () {
          let html = "";
          this.points?.forEach((point) => {
            const x = point.x;
            const y = point.y;
            const xs = typeof x === "number" ? formatNumber(x, { maximumFractionDigits: 0 }) : "—";
            const ys = typeof y === "number" ? formatNumber(y, { maximumFractionDigits: 3 }) : "—";
            html += `<span style="color:${point.color}">\u25CF</span> ${point.series.name} @ ${xs}: <b>${ys}</b><br/>`;
          });
          return html;
        },
      },
      legend: { align: "center", verticalAlign: "bottom" },
      navigator: { enabled: false },
      rangeSelector: { enabled: false },
      scrollbar: { enabled: false },
      series: [
        {
          type: "line",
          name: "Window A (z-norm)",
          data: normA.map((y, x) => [x, y]),
          color: badgeById.get(analysis.sensorId)?.color ?? "#4f46e5",
          lineWidth: 2,
          marker: { enabled: false },
        } as SeriesLineOptions,
        {
          type: "line",
          name: "Window B (z-norm)",
          data: normB.map((y, x) => [x, y]),
          color: "#16a34a",
          lineWidth: 2,
          marker: { enabled: false },
        } as SeriesLineOptions,
      ],
    };

    return { options: overlayOptions, dist, startA, startB };
  }, [analysis, badgeById]);

  const heatmap = useMemo((): HeatmapState | null => {
    if (!analysis.ok) return null;
    if (tab !== "similarity") return null;
    const k = analysis.profile.length;
    if (k < 8) return null;

    const target = Math.min(Math.max(24, Math.floor(heatmapTarget)), k);
    const stride = Math.max(1, Math.floor(k / target));
    const indices: number[] = [];
    for (let i = 0; i < k; i += stride) indices.push(i);
    if (indices[indices.length - 1] !== k - 1) indices.push(k - 1);

    const count = indices.length;
    const window = analysis.window;
    const normalizedWindows = new Float32Array(count * window);
    const constant = new Uint8Array(count);

    for (let row = 0; row < count; row += 1) {
      const start = indices[row]!;
      const slice = analysis.values.slice(start, start + window);
      const norm = zNormalize(slice);
      const base = row * window;
      for (let t = 0; t < window; t += 1) normalizedWindows[base + t] = norm[t] ?? 0;
      if (norm.every((v) => Math.abs(v) <= 1e-12)) constant[row] = 1;
    }

    const distances = new Float32Array(count * count);
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;

    const dot = (a: number, b: number) => {
      const baseA = a * window;
      const baseB = b * window;
      let sum = 0;
      for (let t = 0; t < window; t += 1) sum += normalizedWindows[baseA + t]! * normalizedWindows[baseB + t]!;
      return sum;
    };

    for (let r = 0; r < count; r += 1) {
      for (let c = r; c < count; c += 1) {
        let dist: number;
        if (r === c) {
          dist = 0;
        } else if (constant[r] === 1 && constant[c] === 1) {
          dist = 0;
        } else if (constant[r] === 1 || constant[c] === 1) {
          dist = Math.sqrt(window);
        } else {
          const corr = dot(r, c) / window;
          dist = Math.sqrt(Math.max(0, 2 * window * (1 - corr)));
        }

        distances[r * count + c] = dist;
        distances[c * count + r] = dist;
        if (Number.isFinite(dist)) {
          min = Math.min(min, dist);
          max = Math.max(max, dist);
        }
      }
    }

    if (!Number.isFinite(min) || !Number.isFinite(max)) return null;
    return { indices, distances, min, max, window };
  }, [analysis, heatmapTarget, tab]);

  // Highcharts heatmap options (replaces canvas rendering)
  const heatmapChartOptions = useMemo(() => {
    if (!analysis.ok || !heatmap) return null;
    const count = heatmap.indices.length;
    const data: Array<[number, number, number]> = [];
    for (let r = 0; r < count; r += 1) {
      for (let c = 0; c < count; c += 1) {
        data.push([c, r, heatmap.distances[r * count + c]!]);
      }
    }

    return createHeatmapOptions({
      series: [
        {
          type: "heatmap",
          name: "Distance",
          data,
          borderWidth: 0,
        },
      ],
      colorAxisMin: heatmap.min,
      colorAxisMax: heatmap.max,
      height: 360,
      tooltip: {
        formatter: function () {
          // Heatmap tooltip context — access point via points array or direct cast
          const ctx = this as unknown as { point?: { x: number; y: number; value: number } };
          const dist = ctx.point?.value;
          return `<b>Distance:</b> ${dist != null && Number.isFinite(dist) ? formatNumber(dist, { maximumFractionDigits: 3 }) : "—"}`;
        },
      },
    });
  }, [analysis, heatmap]);

  if (!seriesWithData.length) {
    return (
      <CollapsibleCard
        title="Pattern & Anomaly Detector"
        description="Select a sensor with data to run motif/anomaly discovery on its time series."
        defaultOpen={false}
        data-testid="trends-matrix-profile"
      >
 <p className="text-sm text-muted-foreground">
          Not enough contiguous data is available yet. Select a sensor and increase the Range/Interval above if needed.
        </p>
      </CollapsibleCard>
    );
  }

  const tabButtonClass = (active: boolean) =>
    clsx(
      "inline-flex items-center justify-center rounded-lg border px-3 py-2 text-sm font-semibold shadow-xs transition",
      active
 ? "border-indigo-300 bg-indigo-50 text-indigo-900"
 : "border-border bg-white text-foreground hover:bg-muted",
    );

  return (
    <CollapsibleCard
      title="Pattern & Anomaly Detector"
      description={
        <>
          Cutting-edge time-series mining to surface <span className="font-semibold">motifs</span> (repeating patterns)
          and <span className="font-semibold">anomalies</span> (unusual windows), plus a self-similarity heatmap.
        </>
      }
      defaultOpen
      bodyClassName="space-y-4"
      data-testid="trends-matrix-profile"
    >
      <div className="flex flex-wrap items-end gap-2">
 <label className="text-xs font-semibold text-muted-foreground">
          Sensor
          <Select
            className="ms-2 h-9 max-w-[340px] text-foreground"
            value={activeSensorId ?? ""}
            onChange={(e) => {
              setSensorSelection(e.target.value);
              setFocusStart(null);
              setPairOverride(null);
            }}
          >
            {seriesWithData.map((s) => (
              <option key={s.sensor_id} value={s.sensor_id}>
                {s.label ?? s.sensor_id}
              </option>
            ))}
          </Select>
        </label>

 <label className="text-xs font-semibold text-muted-foreground">
          Window (points)
          <NumericDraftInput
 className="ms-2 h-9 w-24 rounded-lg border border-border bg-white px-3 text-sm text-foreground shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
            value={windowPoints}
            onValueChange={(next) => {
              if (typeof next !== "number" || !Number.isFinite(next)) return;
              setWindowPoints(clampInt(next, 4, 512));
              setPairOverride(null);
            }}
            integer
            min={4}
            max={512}
            clampOnBlur
          />
        </label>

 <label className="text-xs font-semibold text-muted-foreground">
          Analysis points
          <Select
            className="ms-2 h-9 w-28 text-foreground"
            value={String(maxPoints)}
            onChange={(e) => setMaxPoints(Number(e.target.value))}
          >
            <option value="256">256</option>
            <option value="512">512</option>
            <option value="768">768</option>
            <option value="1024">1024</option>
          </Select>
        </label>
        <NodeButton
          size="sm"
          variant="primary"
          type="button"
          onClick={runAnalysis}
          disabled={runDisabled}
          loading={submitting}
          title={
            !activeSeries
              ? "Select a sensor with data"
              : !canComputeWindow
                ? "Select a valid Range first"
                : shouldShowProgress
                  ? "Job already running"
                  : "Run matrix profile job"
          }
        >
          Run analysis
        </NodeButton>
        {shouldShowProgress ? (
          <NodeButton size="sm" variant="secondary" type="button" onClick={handleCancel}>
            Cancel
          </NodeButton>
        ) : null}
      </div>

 <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
        {windowLabel ? (
          <span>
 Window: <span className="font-semibold text-foreground">{windowLabel}</span>
          </span>
        ) : null}
        {jobRequestedAt ? <span>Last run {formatDistanceToNow(jobRequestedAt, { addSuffix: true })}</span> : null}
        {jobStatusLabel ? (
          <NodePill
            tone={job?.status === "completed" ? "success" : job?.status === "failed" ? "danger" : "info"}
            size="sm"
            caps
          >
            {jobStatusLabel}
          </NodePill>
        ) : null}
      </div>
      {latestEventLabel ? (
 <p className="text-xs text-muted-foreground">Latest event: {latestEventLabel}</p>
      ) : null}
      {computedThroughLabel ? (
 <p className="text-xs text-muted-foreground">
 Computed through: <span className="font-semibold text-foreground">{computedThroughLabel}</span>
        </p>
      ) : null}
      {sourcePoints != null || sampledPoints != null ? (
 <p className="text-xs text-muted-foreground">
          Source points:{" "}
 <span className="font-semibold text-foreground">
            {formatNumber(sourcePoints ?? fallbackPoints)}
          </span>{" "}
          · Sampled points:{" "}
 <span className="font-semibold text-foreground">
            {formatNumber(sampledPoints ?? fallbackPoints)}
          </span>
        </p>
      ) : null}

      <div className="flex flex-wrap items-center gap-2">
        <button type="button" className={tabButtonClass(tab === "anomalies")} onClick={() => setTab("anomalies")}>
          Anomalies
        </button>
        <button type="button" className={tabButtonClass(tab === "motifs")} onClick={() => setTab("motifs")}>
          Motifs
        </button>
        <button type="button" className={tabButtonClass(tab === "similarity")} onClick={() => setTab("similarity")}>
          Self-similarity
        </button>
      </div>

      {runError ? <InlineBanner tone="danger">{runError}</InlineBanner> : null}
      {focusSendNotice ? <InlineBanner tone="info">{focusSendNotice}</InlineBanner> : null}
      {downsampled ? (
        <InlineBanner tone="info">
          Input downsampled from {formatNumber(sourcePoints ?? 0)} to {formatNumber(sampledPoints ?? 0)} points for a safe run.
        </InlineBanner>
      ) : null}
      {profileWarnings.length ? (
        <InlineBanner tone="info">
          <span className="font-semibold">Job notes:</span>{" "}
          {profileWarnings.join(" ")}
        </InlineBanner>
      ) : null}
      {shouldShowProgress ? (
        <LoadingState
          label={
            job?.progress.message ??
            (progressTotal ? `Running analysis (${progressCompleted}/${progressTotal})…` : "Running analysis…")
          }
        />
      ) : job?.status === "failed" ? (
        <ErrorState message={job.error?.message || "Analysis job failed. Try again with a shorter window."} />
      ) : job?.status === "canceled" ? (
        <InlineBanner tone="info">Analysis job canceled. Adjust settings and run again.</InlineBanner>
      ) : job?.status === "completed" && jobResultQuery.isLoading ? (
        <LoadingState label="Loading results…" />
      ) : jobResultQuery.error ? (
        <ErrorState
          message={
            jobResultQuery.error instanceof Error
              ? jobResultQuery.error.message
              : "Failed to load analysis results."
          }
        />
      ) : null}

      {!analysis.ok ? (
        <Card className="rounded-lg gap-0 bg-card-inset px-4 py-3 text-sm text-card-foreground">
          {analysis.reason}
        </Card>
      ) : (
        <>
 <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
            <span
 className="rounded-full bg-muted px-3 py-1 font-semibold text-foreground"
              title="Points (n) = number of Interval-bucket points used for analysis after filtering gaps/downsampling."
            >
              Points (n): {analysis.values.length}
            </span>
            <span
 className="rounded-full bg-muted px-3 py-1 font-semibold text-foreground"
              title="Window length. Each point is one interval bucket."
            >
              Window: {analysis.window} pts (~
              {formatApproxDuration((analysis.effectiveIntervalSeconds ?? intervalSeconds) * analysis.window)})
            </span>
            <span
 className="rounded-full bg-muted px-3 py-1 font-semibold text-foreground"
              title="Exclusion zone (points) used to avoid trivial matches with nearby windows."
            >
              Exclusion: {analysis.exclusionZone} pts
            </span>
            {analysis.focusRobustZ != null ? (
              <span
 className="rounded-full bg-indigo-50 px-3 py-1 font-semibold text-indigo-800"
                title="Robust z-score of the selected window's distance vs typical. Higher ≈ more unusual."
              >
                Selected z≈{formatNumber(analysis.focusRobustZ, { maximumFractionDigits: 2 })}
              </span>
            ) : null}
          </div>

          <div className="mt-5 grid gap-6 lg:grid-cols-2">
            <Card className="min-w-0 gap-3 p-4 shadow-xs">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
 <p className="text-sm font-semibold text-foreground">Matrix profile</p>
 <p className="mt-1 text-xs text-muted-foreground">
                    Click the curve to inspect a window. Higher = more unusual.
                  </p>
                </div>
                <div className="flex flex-wrap items-start gap-2">
                  <NodeButton
                    size="xs"
                    type="button"
                    onClick={() => profileChartRef.current?.chart?.zoomOut()}
                  >
                    Reset zoom
                  </NodeButton>
                </div>
              </div>

              <div
                className="relative h-64 min-w-0"
                data-testid="matrix-profile-primary-chart"
              >
                {profileChartOptions ? (
                  <HighchartsPanel
                    chartRef={profileChartRef}
                    options={profileChartOptions}
                    wrapperClassName="h-full w-full"
                    resetZoomOnDoubleClick
                  />
                ) : null}
              </div>
            </Card>

            <Card className="min-w-0 gap-3 p-4 shadow-xs">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
 <p className="text-sm font-semibold text-foreground">Window highlights</p>
 <p className="mt-1 text-xs text-muted-foreground">
                    A and B show the nearest matching windows (motif candidates).
                  </p>
                </div>
                <div className="flex flex-wrap items-start gap-2">
                  <NodeButton
                    size="xs"
                    type="button"
                    onClick={() => seriesChartRef.current?.chart?.zoomOut()}
                  >
                    Reset zoom
                  </NodeButton>
                </div>
              </div>

              <div
                className="relative h-64 min-w-0"
                data-testid="matrix-profile-highlights-chart"
              >
                {highlightSeriesChartOptions ? (
                  <HighchartsPanel
                    chartRef={seriesChartRef}
                    options={highlightSeriesChartOptions}
                    wrapperClassName="h-full w-full"
                    resetZoomOnDoubleClick
                  />
                ) : null}
              </div>
            </Card>
          </div>

          <div className="mt-6 grid gap-6 lg:grid-cols-[1fr_320px]">
            <Card className="min-w-0 gap-3 p-4 shadow-xs">
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
 <p className="text-sm font-semibold text-foreground">Shape comparison</p>
 <p className="mt-1 text-xs text-muted-foreground">
                    Windows are z-normalized (mean 0, std 1) so you compare shapes, not absolute levels.
                  </p>
                </div>
              </div>

              <div className="relative h-56 min-w-0" data-testid="matrix-profile-shape-chart">
                {windowOverlay ? (
                  <HighchartsPanel
                    options={windowOverlay.options}
                    wrapperClassName="h-full w-full"
                  />
                ) : (
                  <Card className="rounded-lg gap-0 bg-card-inset px-3 py-2 text-sm text-card-foreground">
                    Select a point in the matrix profile to compare windows.
                  </Card>
                )}
              </div>
              {windowOverlay ? (
 <p className="mt-2 text-xs text-muted-foreground">
                  Z-norm distance:{" "}
 <span className="font-semibold text-foreground">
                    {formatNumber(windowOverlay.dist, { maximumFractionDigits: 3 })}
                  </span>
                </p>
              ) : null}

              {tab === "similarity" && heatmap ? (
                <Card className="mt-3 min-w-0 gap-3 p-4 shadow-xs">
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0">
 <p className="text-sm font-semibold text-foreground">Self-similarity heatmap</p>
 <p className="mt-1 text-xs text-muted-foreground">
                        Click a cell to pin a motif pair (A/B). Darker = more similar.
                      </p>
                    </div>
                    <div className="flex flex-wrap items-start gap-2">
 <label className="text-xs font-semibold text-muted-foreground">
                        Resolution
                        <Select
                          className="ms-2 h-8 px-2 text-xs text-foreground"
                          value={String(heatmapTarget)}
                          onChange={(e) => setHeatmapTarget(Number(e.target.value))}
                          title="Higher values compute a denser heatmap and may take longer."
                        >
                          <option value="48">48</option>
                          <option value="84">84</option>
                          <option value="128">128</option>
                          <option value="192">192</option>
                          <option value="256">256</option>
                        </Select>
                      </label>
                    </div>
                  </div>

                  <div className="relative min-w-0" data-testid="matrix-profile-heatmap-chart">
                    {heatmapChartOptions ? (
                      <HighchartsPanel
                        options={heatmapChartOptions}
                        centeredMaxWidthPx={420}
                      />
                    ) : null}
                  </div>

 <p className="mt-3 text-xs text-muted-foreground">
                    Heatmap uses a sampled set of subsequences for responsiveness; increasing the resolution above may take longer to compute.
                  </p>
                </Card>
              ) : null}
            </Card>

            <Card className="min-w-0 gap-3 p-4 shadow-xs">
              <div className="flex items-center justify-between gap-3">
 <p className="text-sm font-semibold text-foreground">
                  {tab === "motifs" ? "Top motifs" : "Top anomalies"}
                </p>
                <div className="flex items-center gap-2">
                  {onSendFocusEvents ? (
                    <NodeButton
                      size="xs"
                      type="button"
                      onClick={handleSendFocusEvents}
                      disabled={
                        !analysis.ok ||
                        (tab === "motifs" ? analysis.motifPairs.length === 0 : analysis.anomalies.length === 0)
                      }
                      title="Send focus events to Related Sensors"
                    >
                      Send to Related Sensors
                    </NodeButton>
                  ) : null}
                  <NodeButton size="xs" type="button" onClick={() => setPairOverride(null)} disabled={!analysis.focusPair}>
                    Clear pin
                  </NodeButton>
                </div>
              </div>

              {tab === "motifs" ? (
                <div className="mt-3 space-y-2">
                  {analysis.motifPairs.length ? (
                    analysis.motifPairs.map((pair, idx) => {
                      const motifRange = formatWindowRange(pair.startIso, pair.endIso, effectiveTimeZone);
                      const matchRange = formatWindowRange(pair.matchStartIso, pair.matchEndIso, effectiveTimeZone);
                      const used =
                        (pair.startIso && activeFocusWindowStartSet.has(pair.startIso)) ||
                        (pair.matchStartIso && activeFocusWindowStartSet.has(pair.matchStartIso));
                      return (
                        <button
                          key={`${pair.a}:${pair.b}`}
                          type="button"
 className="w-full rounded-lg border border-border bg-white px-3 py-2 text-left text-sm shadow-xs hover:bg-muted"
                          onClick={() => {
                            setPairOverride({ a: pair.a, b: pair.b });
                            setFocusStart(pair.a);
                          }}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <div className="flex items-center gap-2">
                              <span className="font-semibold">Motif #{idx + 1}</span>
                              {used ? (
                                <span
                                  className="rounded-full bg-indigo-50 px-2 py-1 text-[10px] font-semibold text-indigo-800"
                                  title="This motif window is currently used as the Related Sensors focus-event set."
                                >
                                  In Related Sensors
                                </span>
                              ) : null}
                            </div>
                            <span className="text-xs text-muted-foreground">
                              distance {formatNumber(pair.dist, { maximumFractionDigits: 3 })}
                            </span>
                          </div>
 <div className="mt-1 text-xs text-muted-foreground">
                            A @{pair.a} · B @{pair.b}
                          </div>
                          {motifRange || matchRange ? (
 <div className="mt-1 text-[11px] text-muted-foreground">
                              {motifRange ? (
                                <>
                                  A:{" "}
 <span className="font-semibold text-foreground">{motifRange}</span>
                                </>
                              ) : null}
                              {matchRange ? (
                                <>
                                  {motifRange ? " · " : ""}
                                  B:{" "}
 <span className="font-semibold text-foreground">{matchRange}</span>
                                </>
                              ) : null}
                            </div>
                          ) : null}
                        </button>
                      );
                    })
                  ) : (
                    <Card className="rounded-lg gap-0 bg-card-inset px-3 py-2 text-sm text-card-foreground">
                      Not enough data to identify motifs.
                    </Card>
                  )}
                </div>
              ) : (
                <div className="mt-3 space-y-2">
                  {analysis.anomalies.length ? (
                    analysis.anomalies.map((a, idx) => {
                      const windowRange = formatWindowRange(a.startIso, a.endIso, effectiveTimeZone);
                      const matchRange = formatWindowRange(a.matchStartIso, a.matchEndIso, effectiveTimeZone);
                      const used = a.startIso ? activeFocusWindowStartSet.has(a.startIso) : false;
                      return (
                        <button
                          key={`anom:${a.idx}`}
                          type="button"
 className="w-full rounded-lg border border-border bg-white px-3 py-2 text-left text-sm shadow-xs hover:bg-muted"
                          onClick={() => {
                            setFocusStart(a.idx);
                            setPairOverride(null);
                          }}
                        >
                          <div className="flex items-center justify-between gap-2">
                            <div className="flex items-center gap-2">
                              <span className="font-semibold">Anomaly #{idx + 1}</span>
                              {used ? (
                                <span
                                  className="rounded-full bg-indigo-50 px-2 py-1 text-[10px] font-semibold text-indigo-800"
                                  title="This anomaly window is currently used as the Related Sensors focus-event set."
                                >
                                  In Related Sensors
                                </span>
                              ) : null}
                            </div>
                            <span className="text-xs text-muted-foreground">
                              distance {formatNumber(a.dist, { maximumFractionDigits: 3 })}
                            </span>
                          </div>
 <div className="mt-1 text-xs text-muted-foreground">
                            idx @{a.idx} · nearest {a.match == null ? "—" : `@${a.match}`}
                          </div>
                          {windowRange || matchRange ? (
 <div className="mt-1 text-[11px] text-muted-foreground">
                              {windowRange ? (
                                <>
                                  Window:{" "}
 <span className="font-semibold text-foreground">
                                    {windowRange}
                                  </span>
                                </>
                              ) : null}
                              {matchRange ? (
                                <>
                                  {windowRange ? " · " : ""}
                                  Nearest:{" "}
 <span className="font-semibold text-foreground">
                                    {matchRange}
                                  </span>
                                </>
                              ) : null}
                            </div>
                          ) : null}
                        </button>
                      );
                    })
                  ) : (
                    <Card className="rounded-lg gap-0 bg-card-inset px-3 py-2 text-sm text-card-foreground">
                      Not enough data to identify anomalies.
                    </Card>
                  )}
                </div>
              )}

 <Card className="mt-4 rounded-lg gap-0 bg-card-inset px-3 py-2 text-xs text-foreground">
                {"Tip: if this feels noisy, increase \"Interval\" or reduce the time range to tighten the analysis window."}
              </Card>
            </Card>
          </div>
        </>
      )}

      <AnalysisKey
        summary="Key"
        overview={
          <>
            Find repeating patterns (<span className="font-semibold">motifs</span>) and unusual windows (
            <span className="font-semibold">anomalies</span>) inside a single sensor. Pick a sensor, choose a{" "}
            <span className="font-semibold">window</span>, then click the curve or list to inspect.
          </>
        }
      >
        <div className="space-y-3">
          <div>
 <p className="text-xs font-semibold text-foreground">What this does</p>
 <p className="mt-1 text-xs text-muted-foreground">
              Finds repeating patterns (<span className="font-semibold">motifs</span>) and unusual segments (
              <span className="font-semibold">anomalies</span>) inside a single sensor&apos;s time series. This is great for
              spotting cycles (pump runs, duty cycles) and &quot;weird moments&quot; without needing a second sensor.
            </p>
          </div>

          <div>
 <p className="text-xs font-semibold text-foreground">How to use</p>
 <ol className="mt-1 list-decimal space-y-1 ps-5 text-muted-foreground">
              <li>
                Pick a sensor and set <span className="font-semibold">Window</span>. Window is measured in points
                (bucketed by your Interval).
              </li>
              <li>
                Use <span className="font-semibold">Anomalies</span> to jump to unusual windows; use{" "}
                <span className="font-semibold">Motifs</span> to compare repeating windows (A vs B).
              </li>
              <li>
                Use <span className="font-semibold">Self-similarity</span> for a zoomed-out &quot;structure&quot; view. Higher
                resolution can take longer to compute.
              </li>
            </ol>
          </div>

          <div>
 <p className="text-xs font-semibold text-foreground">Key terms</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
              <li>
                <code>window</code>: segment length (points). Short windows find spikes; long windows find longer
                patterns.
              </li>
              <li>
                <code>Points (n)</code>: how many data points are used for analysis (each point is one Interval bucket
                after filtering gaps/downsampling).
              </li>
              <li>
                <code>Analysis points</code>: target compute size after downsampling (higher = slower, more detail).
              </li>
              <li>
                <code>distance</code>: shape distance between two windows (lower = more similar, higher = more unusual).
              </li>
              <li>
                <code>exclusion</code>: exclusion zone (points) to avoid trivial &quot;matches&quot; with nearby windows.
              </li>
              <li>
                <code>z</code>: robust &quot;how unusual&quot; score for the selected window (higher ≈ more unusual).
              </li>
              <li>
                <code>Resolution</code>: heatmap density for Self-similarity (higher = slower).
              </li>
            </ul>
          </div>

          <div>
 <p className="text-xs font-semibold text-foreground">Matrix profile</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
              <li>
                Each point represents one <span className="font-semibold">window</span> of your time series.
              </li>
              <li>
                The y-value is the <span className="font-semibold">distance</span> from that window to its most-similar
                other window in the same series.
              </li>
              <li>
                <span className="font-semibold">Low</span> distance means &quot;this pattern repeats somewhere else&quot;
                (motif-ish).
              </li>
              <li>
                <span className="font-semibold">High</span> distance means &quot;this window doesn&apos;t look like anything else&quot;
                (anomaly-ish).
              </li>
              <li>Click a peak to jump to an unusual moment; click a valley to jump to a repeating pattern.</li>
            </ul>
          </div>

          <div>
 <p className="text-xs font-semibold text-foreground">Window highlights</p>
 <p className="mt-1 text-xs text-muted-foreground">
              Shows where the currently-selected window (<span className="font-semibold">A</span>) occurs in the series,
              and the best matching window (<span className="font-semibold">B</span>). This is the &quot;where in time?&quot; view
              that complements the shape overlay.
            </p>
          </div>

          <div>
 <p className="text-xs font-semibold text-foreground">Shape comparison</p>
 <p className="mt-1 text-xs text-muted-foreground">
              Overlays the selected window (A) and its matched window (B) after{" "}
              <span className="font-semibold">z-normalizing</span>. This makes it easier to compare shapes even if the
              absolute level differs. The printed &quot;Z-norm distance&quot; is the shape difference (lower = more similar).
            </p>
          </div>

          <div>
 <p className="text-xs font-semibold text-foreground">Self-similarity heatmap</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
              <li>
                A zoomed-out map of &quot;how similar is this time window to that time window?&quot; Each cell compares two windows
                from the same series.
              </li>
              <li>
                <span className="font-semibold">Darker</span> cells mean more similar shapes (lower distance).
              </li>
              <li>Diagonal structure often means periodic behavior (daily cycles, repeating runs).</li>
              <li>Click a dark cell to pin a pair (A/B) and jump to Motifs.</li>
              <li>
                <code>Resolution</code> controls how dense the heatmap sampling is (higher = slower).
              </li>
            </ul>
          </div>

          <div>
 <p className="text-xs font-semibold text-foreground">Top motifs / anomalies list</p>
 <ul className="mt-1 list-disc space-y-1 ps-5 text-muted-foreground">
              <li>
                Motifs are the strongest repeating pattern pairs; anomalies are the most unusual windows (highest
                distance).
              </li>
              <li>Click an item to inspect it on the charts.</li>
              <li>
                <code>idx</code> is the window start index in the analyzed point series (each point is one Interval
                bucket).
              </li>
              <li>
                <code>nearest</code> is the index of the closest matching window.
              </li>
              <li>
                <code>distance</code> is the shape distance between the two windows (lower = more similar).
              </li>
            </ul>
          </div>
        </div>
      </AnalysisKey>
    </CollapsibleCard>
  );
}
