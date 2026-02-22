"use client";

import clsx from "clsx";
import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { XAxisPlotBandsOptions, XAxisPlotLinesOptions } from "highcharts";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import { TrendChart } from "@/components/TrendChart";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import { formatNumber, formatDuration } from "@/lib/format";
import { browserTimeZone, formatDateTimeForTimeZone } from "@/lib/siteTime";
import { fetchAnalysisPreview } from "@/lib/api";
import { zScoreNormalizeSeries } from "@/features/trends/utils/relatedSensors";
import { pickRepresentativeEpisodeIndex } from "@/features/trends/utils/episodeSelection";
import type { DemoSensor, TrendSeriesEntry } from "@/types/dashboard";
import type { TsseEpisodeV1, TssePreviewResponseV1 } from "@/types/analysis";
import type { CorrelationMatrixCellV1 } from "@/types/analysis";
import type { CorrelationMethod } from "../../utils/correlation";
import type {
  NormalizedCandidate,
  Strategy,
} from "../../types/relationshipFinder";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import SegmentedControl from "@/components/SegmentedControl";
import AddToChartButton from "./AddToChartButton";
import CorrelationPreview from "./CorrelationPreview";
import InlineBanner from "@/components/InlineBanner";
import { emitRelatedSensorsUxEvent } from "@/features/trends/utils/relatedSensorsUxEvents";

type PreviewPaneProps = {
  focusSensorId: string | null;
  focusLabel: string;
  candidate: NormalizedCandidate | null;
  sensorsById: Map<string, DemoSensor>;
  labelMap: Map<string, string>;
  selectedSensorIds: string[];
  maxSeries: number;
  onAddToChart?: (sensorId: string) => void;
  timeZone?: string;
  computedThroughTs?: string | null;
  relationshipMode?: "simple" | "advanced";
  // Correlation-specific props
  strategy?: Strategy;
  series?: TrendSeriesEntry[];
  intervalSeconds?: number;
  effectiveIntervalSeconds?: number | null;
  correlationMethod?: CorrelationMethod;
  analysisBucketCount?: number | null;
  onJumpToTimestamp?: (timestampMs: number) => void;
};

function shortLabel(label: string): string {
  const trimmed = label.trim();
  if (!trimmed) return trimmed;
  const parts = trimmed.split(" — ");
  const tail = parts.length > 1 ? parts.slice(1).join(" — ") : trimmed;
  return tail.replace(/\s*\([^)]*\)\s*$/, "").trim() || trimmed;
}

function formatLagSeconds(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "0";
  if (seconds === 0) return "0";
  const sign = seconds < 0 ? "-" : "+";
  const absSeconds = Math.abs(seconds);
  if (absSeconds >= 3600) {
    const hours = absSeconds / 3600;
    return `${sign}${formatNumber(hours, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}h`;
  }
  if (absSeconds >= 60) {
    const minutes = absSeconds / 60;
    return `${sign}${formatNumber(minutes, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}m`;
  }
  return `${sign}${absSeconds}s`;
}

function formatCoverage(value?: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `${formatNumber(value, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}%`;
}

function formatPValue(value?: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  if (value < 0.001) return "<0.001";
  return formatNumber(value, { maximumFractionDigits: 3 });
}

function formatScore(value?: number | null): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return formatNumber(value, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

type PreviewContextPreset = "auto" | "episode" | "1h" | "3h" | "6h" | "24h" | "72h" | "custom";

function safeParseTimestampMs(value: string | null | undefined): number | null {
  if (!value) return null;
  const ms = Date.parse(value);
  return Number.isFinite(ms) ? ms : null;
}

function insertNullGaps(points: TrendSeriesEntry["points"], bucketSeconds: number): TrendSeriesEntry["points"] {
  const bucketMs = Math.max(1, Math.round(bucketSeconds * 1000));
  if (!Number.isFinite(bucketMs) || bucketMs <= 0) return points;
  if (points.length < 2) return points;

  const thresholdMs = bucketMs * 1.5;
  const out: TrendSeriesEntry["points"] = [];

  for (let i = 0; i < points.length; i += 1) {
    const current = points[i];
    if (!current) continue;
    out.push(current);

    const next = points[i + 1];
    if (!next) continue;

    const deltaMs = next.timestamp.getTime() - current.timestamp.getTime();
    if (!Number.isFinite(deltaMs) || deltaMs <= thresholdMs) continue;

    out.push({
      timestamp: new Date(current.timestamp.getTime() + bucketMs),
      value: null,
      samples: 0,
    });
  }

  return out;
}

function mapPreviewSeries(
  series: TssePreviewResponseV1["focus"],
  label: string,
  unit?: string,
  bucketSeconds?: number,
): TrendSeriesEntry {
  const basePoints = series.points.map((point) => ({
    timestamp: new Date(point.timestamp),
    value: point.value,
    samples: point.samples,
  }));

  return {
    sensor_id: series.sensor_id,
    label,
    unit: unit ?? series.unit ?? undefined,
    points:
      bucketSeconds != null && Number.isFinite(bucketSeconds)
        ? insertNullGaps(basePoints, bucketSeconds)
        : basePoints,
  };
}

export default function PreviewPane({
  focusSensorId,
  focusLabel,
  candidate,
  sensorsById,
  labelMap,
  selectedSensorIds,
  maxSeries,
  onAddToChart,
  timeZone,
  computedThroughTs,
  relationshipMode = "simple",
  strategy,
  series,
  intervalSeconds,
  effectiveIntervalSeconds,
  correlationMethod = "pearson",
  analysisBucketCount,
  onJumpToTimestamp,
}: PreviewPaneProps) {
  const effectiveTimeZone = timeZone ?? browserTimeZone();

  // Get series data for focus and candidate (for correlation preview)
  const seriesById = useMemo(() => {
    const map = new Map<string, TrendSeriesEntry>();
    series?.forEach((s) => map.set(s.sensor_id, s));
    return map;
  }, [series]);

  const focusSeries = focusSensorId ? seriesById.get(focusSensorId) ?? null : null;
  const candidateSeries = candidate ? seriesById.get(candidate.sensor_id) ?? null : null;

  // Extract correlation cell from raw candidate data
  const correlationCell = useMemo((): CorrelationMatrixCellV1 | null => {
    if (!candidate?.raw || candidate.raw.type !== "correlation") return null;
    const { data } = candidate.raw;
    return data as CorrelationMatrixCellV1;
  }, [candidate]);

  const [previewMode, setPreviewMode] = useState<"normalized" | "raw">("normalized");
  const [alignByLag, setAlignByLag] = useState(true);
  const [showMatchedEventsOnly, setShowMatchedEventsOnly] = useState(false);
  const [contextPreset, setContextPreset] = useState<PreviewContextPreset>("auto");
  const [customContextHours, setCustomContextHours] = useState<number>(1);

  // Get episodes from raw data
  const episodes = useMemo(() => {
    if (!candidate?.raw) return [];
    const { type, data } = candidate.raw;
    if (type === "unified" && "episodes" in data) {
      return (data.episodes as TsseEpisodeV1[]) ?? [];
    }
    if (type === "similarity" && "episodes" in data) {
      return (data.episodes as TsseEpisodeV1[]) ?? [];
    }
    if (type === "events" && "episodes" in data) {
      return (data.episodes as TsseEpisodeV1[]) ?? [];
    }
    return [];
  }, [candidate]);

  const defaultEpisodeIndex = useMemo(
    () => pickRepresentativeEpisodeIndex(episodes),
    [episodes],
  );

  // Store per-candidate selection to avoid effect-driven state resets.
  const [episodeSelectionByCandidateId, setEpisodeSelectionByCandidateId] =
    useState<Map<string, number>>(() => new Map());
  const [lagOverrideByCandidateId, setLagOverrideByCandidateId] =
    useState<Map<string, number>>(() => new Map());

  const selectedEpisodeIndex = useMemo(() => {
    const candidateId = candidate?.sensor_id;
    if (!candidateId) return 0;
    const idx =
      episodeSelectionByCandidateId.get(candidateId) ?? defaultEpisodeIndex;
    if (episodes.length === 0) return 0;
    return Math.min(Math.max(0, idx), episodes.length - 1);
  }, [
    candidate?.sensor_id,
    defaultEpisodeIndex,
    episodeSelectionByCandidateId,
    episodes.length,
  ]);

  const selectedEpisode = useMemo(() => {
    if (!episodes.length) return null;
    const idx = Math.min(Math.max(0, selectedEpisodeIndex), episodes.length - 1);
    return episodes[idx] ?? null;
  }, [episodes, selectedEpisodeIndex]);

  const effectiveLagSec = useMemo(() => {
    const baseLag = selectedEpisode?.lag_sec ?? 0;
    const candidateId = candidate?.sensor_id;
    if (!candidateId) return baseLag;
    return lagOverrideByCandidateId.get(candidateId) ?? baseLag;
  }, [candidate?.sensor_id, lagOverrideByCandidateId, selectedEpisode?.lag_sec]);

  const previewWindow = useMemo(() => {
    if (!selectedEpisode) return null;
    const episodeStartMs = safeParseTimestampMs(selectedEpisode.start_ts);
    const episodeEndMs = safeParseTimestampMs(selectedEpisode.end_ts);
    if (episodeStartMs == null || episodeEndMs == null || episodeEndMs <= episodeStartMs) {
      return { startTs: selectedEpisode.start_ts, endTs: selectedEpisode.end_ts, episodeStartMs: null, episodeEndMs: null };
    }

    const baseWindowSeconds = Math.max(1, Math.round((episodeEndMs - episodeStartMs) / 1000));

    let paddingSeconds = 0;
    if (contextPreset === "1h") paddingSeconds = 1 * 3600;
    if (contextPreset === "3h") paddingSeconds = 3 * 3600;
    if (contextPreset === "6h") paddingSeconds = 6 * 3600;
    if (contextPreset === "24h") paddingSeconds = 24 * 3600;
    if (contextPreset === "72h") paddingSeconds = 72 * 3600;
    if (contextPreset === "custom") {
      const boundedHours = Math.min(168, Math.max(0.1, customContextHours));
      paddingSeconds = Math.round(boundedHours * 3600);
    }
    if (contextPreset === "auto") {
      const targetWindowSeconds = Math.max(baseWindowSeconds * 4, 24 * 3600);
      paddingSeconds = Math.max(0, Math.round((targetWindowSeconds - baseWindowSeconds) / 2));
    }

    let startMs = episodeStartMs;
    let endMs = episodeEndMs;
    if (contextPreset !== "episode") {
      startMs = startMs - paddingSeconds * 1000;
      endMs = endMs + paddingSeconds * 1000;
      const computedThroughMs = safeParseTimestampMs(computedThroughTs);
      if (
        computedThroughMs != null &&
        computedThroughMs >= episodeEndMs &&
        endMs > computedThroughMs
      ) {
        endMs = computedThroughMs;
      }
      if (endMs <= startMs) {
        startMs = episodeStartMs;
        endMs = episodeEndMs;
      }
    }

    return {
      startTs: new Date(startMs).toISOString(),
      endTs: new Date(endMs).toISOString(),
      episodeStartMs,
      episodeEndMs,
    };
  }, [computedThroughTs, contextPreset, customContextHours, selectedEpisode]);

  const episodeXPlotBands = useMemo<XAxisPlotBandsOptions[] | undefined>(() => {
    if (previewWindow?.episodeStartMs == null || previewWindow?.episodeEndMs == null) return undefined;
    return [
      {
        id: "episode-window",
        from: previewWindow.episodeStartMs,
        to: previewWindow.episodeEndMs,
        color: "rgba(99, 102, 241, 0.10)",
        zIndex: 3,
      },
    ];
  }, [previewWindow]);

  const episodeXPlotLines = useMemo<XAxisPlotLinesOptions[] | undefined>(() => {
    if (previewWindow?.episodeStartMs == null || previewWindow?.episodeEndMs == null) return undefined;
    return [
      {
        id: "episode-start",
        value: previewWindow.episodeStartMs,
        width: 1,
        color: "rgba(99, 102, 241, 0.55)",
        dashStyle: "Dash",
        zIndex: 4,
      },
      {
        id: "episode-end",
        value: previewWindow.episodeEndMs,
        width: 1,
        color: "rgba(99, 102, 241, 0.55)",
        dashStyle: "Dash",
        zIndex: 4,
      },
    ];
  }, [previewWindow]);

  const similarityStats = useMemo(() => {
    if (!candidate?.raw || candidate.raw.type !== "similarity") return null;
    const scoreComponents = candidate.raw.data.why_ranked?.score_components ?? {};
    return {
      pRaw: scoreComponents["lag_p_raw"] ?? null,
      pLag: scoreComponents["lag_p_lag"] ?? null,
      qValue: scoreComponents["q_value"] ?? null,
      n: scoreComponents["aligned_points"] ?? null,
      nEff: scoreComponents["n_eff"] ?? null,
      mLag: scoreComponents["m_lag"] ?? null,
      bestLag: candidate.raw.data.why_ranked?.best_lag_sec ?? null,
    };
  }, [candidate]);

  const unifiedEvidence = useMemo(() => {
    if (!candidate?.raw || candidate.raw.type !== "unified") return null;
    return {
      summary: candidate.raw.data.evidence?.summary ?? [],
      timestamps: candidate.raw.data.top_bucket_timestamps ?? [],
      eventsScore: candidate.raw.data.evidence?.events_score ?? null,
      eventsOverlap: candidate.raw.data.evidence?.events_overlap ?? null,
      nFocus: candidate.raw.data.evidence?.n_focus ?? null,
      nCandidate: candidate.raw.data.evidence?.n_candidate ?? null,
      cooccurrenceCount: candidate.raw.data.evidence?.cooccurrence_count ?? null,
      focusBucketCoveragePct: candidate.raw.data.evidence?.focus_bucket_coverage_pct ?? null,
      candidateBucketCoveragePct: candidate.raw.data.evidence?.candidate_bucket_coverage_pct ?? null,
      cooccurrenceStrength: candidate.raw.data.evidence?.cooccurrence_strength ?? null,
      cooccurrenceScoreRaw: candidate.raw.data.evidence?.cooccurrence_score ?? null,
      topLags: candidate.raw.data.evidence?.top_lags ?? [],
      directionLabel: candidate.raw.data.evidence?.direction_label ?? null,
      directionN: candidate.raw.data.evidence?.direction_n ?? null,
      signAgreement: candidate.raw.data.evidence?.sign_agreement ?? null,
      deltaCorr: candidate.raw.data.evidence?.delta_corr ?? null,
      timeOfDayEntropyNorm: candidate.raw.data.evidence?.time_of_day_entropy_norm ?? null,
      timeOfDayEntropyWeight: candidate.raw.data.evidence?.time_of_day_entropy_weight ?? null,
    };
  }, [candidate]);

  const directionTooltip = useMemo(() => {
    if (!unifiedEvidence) return undefined;
    const parts: string[] = [];

    if (unifiedEvidence.directionN != null && Number.isFinite(unifiedEvidence.directionN)) {
      const n = unifiedEvidence.directionN;
      const sparseNote =
        unifiedEvidence.directionLabel === "unknown" && n < 3
          ? " (<3 → unknown)"
          : "";
      parts.push(
        `Matched pairs: ${formatNumber(n, { maximumFractionDigits: 0 })}${sparseNote}`,
      );
    }

    if (unifiedEvidence.signAgreement != null && Number.isFinite(unifiedEvidence.signAgreement)) {
      parts.push(
        `Sign agreement: ${formatCoverage(unifiedEvidence.signAgreement * 100)}`,
      );
    }

    if (unifiedEvidence.deltaCorr != null && Number.isFinite(unifiedEvidence.deltaCorr)) {
      parts.push(`Δ corr: ${formatScore(unifiedEvidence.deltaCorr)}`);
    }

    return parts.length > 0 ? parts.join(" · ") : undefined;
  }, [unifiedEvidence]);

  const unifiedCoverage = useMemo(() => {
    if (!unifiedEvidence) return null;

    const overlap =
      unifiedEvidence.eventsOverlap != null && Number.isFinite(unifiedEvidence.eventsOverlap)
        ? unifiedEvidence.eventsOverlap
        : null;
    const nFocus =
      unifiedEvidence.nFocus != null && Number.isFinite(unifiedEvidence.nFocus)
        ? unifiedEvidence.nFocus
        : null;
    const nCandidate =
      unifiedEvidence.nCandidate != null && Number.isFinite(unifiedEvidence.nCandidate)
        ? unifiedEvidence.nCandidate
        : null;
    const cooccCount =
      unifiedEvidence.cooccurrenceCount != null && Number.isFinite(unifiedEvidence.cooccurrenceCount)
        ? unifiedEvidence.cooccurrenceCount
        : null;
    const bucketsTotal =
      analysisBucketCount != null && Number.isFinite(analysisBucketCount) && analysisBucketCount > 0
        ? analysisBucketCount
        : null;

    const focusPct =
      overlap != null && nFocus != null && nFocus > 0 ? (overlap / nFocus) * 100 : null;
    const candidatePct =
      overlap != null && nCandidate != null && nCandidate > 0 ? (overlap / nCandidate) * 100 : null;
    const bucketsPct =
      cooccCount != null && bucketsTotal != null ? (cooccCount / bucketsTotal) * 100 : null;

    return {
      focusPct,
      candidatePct,
      bucketsPct,
    };
  }, [analysisBucketCount, unifiedEvidence]);

  // Preview data query
  const previewQuery = useQuery({
    queryKey: [
      "analysis",
      "preview",
      focusSensorId ?? "none",
      candidate?.sensor_id ?? "none",
      previewWindow?.startTs ?? "none",
      previewWindow?.endTs ?? "none",
      contextPreset,
      effectiveLagSec,
    ],
    queryFn: () =>
      fetchAnalysisPreview({
        focus_sensor_id: focusSensorId!,
        candidate_sensor_id: candidate!.sensor_id,
        episode_start_ts: previewWindow?.startTs,
        episode_end_ts: previewWindow?.endTs,
        lag_seconds: effectiveLagSec,
        max_points: 800,
      }),
    enabled: Boolean(focusSensorId && candidate && selectedEpisode),
    staleTime: 60_000,
  });

  const previewData = previewQuery.data ?? null;
  const lagAlignmentSuppressed = useMemo(() => {
    if (!alignByLag || !previewData?.candidate_aligned) return false;
    const alignedCount = previewData.candidate_aligned.points.length;
    const rawCount = previewData.candidate.points.length;
    return alignedCount <= 1 && rawCount > 1;
  }, [alignByLag, previewData]);

  const hasEventOverlays = useMemo(() => {
    const overlays = previewData?.event_overlays;
    if (!overlays) return false;
    const focusCount = Array.isArray(overlays.focus_event_ts_ms) ? overlays.focus_event_ts_ms.length : 0;
    const candCount = Array.isArray(overlays.candidate_event_ts_ms) ? overlays.candidate_event_ts_ms.length : 0;
    const matchedFocus = Array.isArray(overlays.matched_focus_event_ts_ms)
      ? overlays.matched_focus_event_ts_ms.length
      : 0;
    const matchedCand = Array.isArray(overlays.matched_candidate_event_ts_ms)
      ? overlays.matched_candidate_event_ts_ms.length
      : 0;
    return focusCount + candCount + matchedFocus + matchedCand > 0;
  }, [previewData?.event_overlays]);

  const eventXPlotLines = useMemo<XAxisPlotLinesOptions[] | undefined>(() => {
    const overlays = previewData?.event_overlays;
    if (!overlays || !hasEventOverlays) return undefined;

    const lagSec = effectiveLagSec;
    const lagMs = alignByLag ? lagSec * 1000 : 0;

    const focusEvents = showMatchedEventsOnly
      ? overlays.matched_focus_event_ts_ms ?? []
      : overlays.focus_event_ts_ms ?? [];
    const candidateEvents = showMatchedEventsOnly
      ? overlays.matched_candidate_event_ts_ms ?? []
      : overlays.candidate_event_ts_ms ?? [];

    const maxLinesPerSeries = 120;
    const focusColor = showMatchedEventsOnly ? "rgba(79, 70, 229, 0.55)" : "rgba(99, 102, 241, 0.35)";
    const candColor = showMatchedEventsOnly ? "rgba(217, 119, 6, 0.55)" : "rgba(245, 158, 11, 0.35)";
    const dashStyle = showMatchedEventsOnly ? "Solid" : "ShortDash";

    const lines: XAxisPlotLinesOptions[] = [];
    for (const ts of focusEvents.slice(0, maxLinesPerSeries)) {
      if (typeof ts !== "number" || !Number.isFinite(ts)) continue;
      lines.push({
        id: `focus-event-${ts}`,
        value: ts,
        width: showMatchedEventsOnly ? 2 : 1,
        color: focusColor,
        dashStyle,
        zIndex: 2,
      });
    }
    for (const rawTs of candidateEvents.slice(0, maxLinesPerSeries)) {
      if (typeof rawTs !== "number" || !Number.isFinite(rawTs)) continue;
      const ts = rawTs - lagMs;
      lines.push({
        id: `candidate-event-${rawTs}`,
        value: ts,
        width: showMatchedEventsOnly ? 2 : 1,
        color: candColor,
        dashStyle,
        zIndex: 2,
      });
    }
    return lines.length > 0 ? lines : undefined;
  }, [alignByLag, effectiveLagSec, hasEventOverlays, previewData?.event_overlays, showMatchedEventsOnly]);

  const mergedXPlotLines = useMemo<XAxisPlotLinesOptions[] | undefined>(() => {
    const base = episodeXPlotLines ?? [];
    const extras = eventXPlotLines ?? [];
    const merged = [...base, ...extras];
    return merged.length > 0 ? merged : undefined;
  }, [episodeXPlotLines, eventXPlotLines]);

  const previewSeries = useMemo(() => {
    if (!previewData || !focusSensorId || !candidate) return null;
    const candidateLabel = labelMap.get(candidate.sensor_id) ?? candidate.label;
    const focusUnit = sensorsById.get(focusSensorId)?.unit;
    const candidateUnit = sensorsById.get(candidate.sensor_id)?.unit;

    const focusSeriesData = mapPreviewSeries(
      previewData.focus,
      focusLabel,
      focusUnit,
      previewData.bucket_seconds,
    );
    const useAlignedCandidate =
      alignByLag &&
      previewData.candidate_aligned &&
      !lagAlignmentSuppressed;
    const candidateSource = useAlignedCandidate
      ? (previewData.candidate_aligned ?? previewData.candidate)
      : previewData.candidate;
    const candidateSeriesData = mapPreviewSeries(
      candidateSource,
      candidateLabel,
      candidateUnit,
      previewData.bucket_seconds,
    );

    if (previewMode === "normalized") {
      return [zScoreNormalizeSeries(focusSeriesData), zScoreNormalizeSeries(candidateSeriesData)];
    }
    return [focusSeriesData, candidateSeriesData];
  }, [
    alignByLag,
    candidate,
    focusLabel,
    focusSensorId,
    lagAlignmentSuppressed,
    labelMap,
    previewData,
    previewMode,
    sensorsById,
  ]);

  const isOnChart = candidate ? selectedSensorIds.includes(candidate.sensor_id) : false;
  const isAtLimit = selectedSensorIds.length >= maxSeries;

  const sparseEpisodeWarning = useMemo(() => {
    if (!selectedEpisode) return null;

    const coveragePct = selectedEpisode.coverage * 100.0;
    const points = selectedEpisode.num_points;
    const isSparse = coveragePct < 5 || points < 10;
    if (!isSparse) return null;

    return {
      coveragePct,
      points,
    };
  }, [selectedEpisode]);

  const focusEventGuardrail = useMemo(() => {
    const nFocus = unifiedEvidence?.nFocus;
    if (nFocus == null || !Number.isFinite(nFocus)) return null;
    if (nFocus < 3) {
      return "Too few focus events for stable ranking. Expand the time range or lower the event threshold.";
    }
    if (nFocus > 2000) {
      return "Focus sensor is very eventful; results may reflect noise. Increase the event threshold or raise the interval.";
    }
    return null;
  }, [unifiedEvidence?.nFocus]);

  // Empty state
  if (!candidate) {
    return (
      <Card className="gap-0 p-4 py-4">
 <h4 className="text-sm font-semibold text-foreground">
          Preview
        </h4>
        <Card
          className="mt-4 rounded-lg gap-0 border-dashed px-4 py-8 text-center text-sm text-muted-foreground"
          aria-live="polite"
        >
          Select a result to preview.
        </Card>
      </Card>
    );
  }

  const renderEpisodeButton = (episode: TsseEpisodeV1, idx: number) => {
    const start = new Date(episode.start_ts);
    const end = new Date(episode.end_ts);
    const label =
      Number.isFinite(start.getTime()) && Number.isFinite(end.getTime())
        ? `${formatDateTimeForTimeZone(start, effectiveTimeZone, {
            month: "numeric",
            day: "numeric",
            hour: "numeric",
            minute: "2-digit",
          })} → ${formatDateTimeForTimeZone(end, effectiveTimeZone, {
            hour: "numeric",
            minute: "2-digit",
          })}`
        : `${episode.start_ts} → ${episode.end_ts}`;
    const active = idx === selectedEpisodeIndex;

    return (
      <button
        key={`${episode.start_ts}-${episode.end_ts}-${idx}`}
        type="button"
        className={clsx(
          "w-full rounded-lg border px-3 py-2 text-left text-xs transition",
          active
 ? "border-indigo-300 bg-indigo-50 text-indigo-900"
 : "border-border bg-white text-foreground hover:bg-muted",
        )}
        onClick={() => {
          if (!candidate) return;
          emitRelatedSensorsUxEvent("episode_selected", {
            panel: "related_sensors",
            focus_sensor_id: focusSensorId,
            candidate_sensor_id: candidate.sensor_id,
            candidate_rank: candidate.rank,
            episode_index: idx,
            episode_count: episodes.length,
            episode_window_sec: episode.window_sec,
            lag_sec: episode.lag_sec,
            coverage: episode.coverage,
            score_peak: episode.score_peak,
          });
          setEpisodeSelectionByCandidateId((prev) => {
            const next = new Map(prev);
            next.set(candidate.sensor_id, idx);
            return next;
          });
        }}
      >
        <p className="font-semibold">{label}</p>
 <div className="mt-1 flex flex-wrap gap-2 text-[11px] text-muted-foreground">
          <span>Window {formatDuration(episode.window_sec)}</span>
          <span>Lag {formatLagSeconds(episode.lag_sec)}</span>
          <span>Coverage {formatCoverage(episode.coverage * 100)}</span>
          <span>
            <span title="Peak absolute robust z-score of focus deltas within matched events in this episode.">
              Peak |Δz|{" "}
            </span>
            {formatNumber(episode.score_peak, {
              minimumFractionDigits: 2,
              maximumFractionDigits: 2,
            })}
          </span>
        </div>
      </button>
    );
  };

  return (
    <Card
      className="gap-0 p-3 py-3"
      aria-live="polite"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
 <h4 className="text-sm font-semibold text-foreground">
            Preview: {shortLabel(candidate.label)}
          </h4>
 <p className="mt-1 text-xs text-muted-foreground">
            <span title="0–1 rank score relative to the evaluated candidates in this run. Not a probability. Not comparable across different runs or scopes.">
              Rank #{candidate.rank} · Rank score {candidate.score_label}
            </span>
          </p>
        </div>
        <AddToChartButton
          sensorId={candidate.sensor_id}
          isOnChart={isOnChart}
          isAtLimit={isAtLimit}
          onAddToChart={onAddToChart}
          size="md"
        />
      </div>

      {/* Summary badges */}
      <div className="mt-3 flex flex-wrap gap-1">
        {candidate.badges.map((badge, idx) => (
          <NodePill
            key={`${badge.type}-${idx}`}
            tone={badge.tone}
            size="sm"
            weight="normal"
            title={badge.tooltip}
          >
            {badge.label}: {badge.value}
          </NodePill>
        ))}
      </div>

      {/* Correlation strategy - use specialized preview */}
      {strategy === "correlation" ? (
        <div className="mt-4">
          <CorrelationPreview
            focusSeries={focusSeries}
            candidateSeries={candidateSeries}
            cell={correlationCell}
            method={correlationMethod}
            intervalSeconds={intervalSeconds ?? 3600}
            timeZone={effectiveTimeZone}
          />
        </div>
      ) : (
        <>
          {sparseEpisodeWarning ? (
            <div className="mt-4">
              <InlineBanner tone="warning">
                Weak episode: only{" "}
                {formatNumber(sparseEpisodeWarning.points, { maximumFractionDigits: 0 })} matched events{" "}
                ({formatCoverage(sparseEpisodeWarning.coveragePct)} of focus events). Treat as low evidence. Try a different episode, expand the time range, or lower the event threshold.
              </InlineBanner>
            </div>
          ) : null}
          {strategy === "unified" && focusEventGuardrail ? (
            <div className="mt-4">
              <InlineBanner tone="warning">{focusEventGuardrail}</InlineBanner>
            </div>
          ) : null}
          {strategy === "similarity" && similarityStats && (
            <Card className="mt-4 rounded-lg bg-card-inset p-3">
              <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Statistical inference
              </p>
              <div className="mt-2 grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">p(raw)</p>
                  <p className="font-semibold text-foreground">{formatPValue(similarityStats.pRaw)}</p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">p(lag)</p>
                  <p className="font-semibold text-foreground">{formatPValue(similarityStats.pLag)}</p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">q (FDR)</p>
                  <p className="font-semibold text-foreground">{formatPValue(similarityStats.qValue)}</p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">best lag</p>
                  <p className="font-semibold text-foreground">{formatLagSeconds(similarityStats.bestLag)}</p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">n</p>
                  <p className="font-semibold text-foreground">
                    {similarityStats.n != null
                      ? formatNumber(similarityStats.n, { maximumFractionDigits: 0 })
                      : "—"}
                  </p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">n_eff</p>
                  <p className="font-semibold text-foreground">
                    {similarityStats.nEff != null
                      ? formatNumber(similarityStats.nEff, { maximumFractionDigits: 0 })
                      : "—"}
                  </p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">m_lag</p>
                  <p className="font-semibold text-foreground">
                    {similarityStats.mLag != null
                      ? formatNumber(similarityStats.mLag, { maximumFractionDigits: 0 })
                      : "—"}
                  </p>
                </div>
              </div>
            </Card>
          )}

          {strategy === "unified" && unifiedEvidence && (
            <Card className="mt-4 rounded-lg bg-card-inset p-3">
              <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Evidence summary
              </p>
              <div className="mt-2 grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">Event match (F1)</p>
                  <p className="font-semibold text-foreground">{formatScore(unifiedEvidence.eventsScore)}</p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">Matched events</p>
                  <p className="font-semibold text-foreground">
                    {unifiedEvidence.eventsOverlap != null
                      ? formatNumber(unifiedEvidence.eventsOverlap, { maximumFractionDigits: 0 })
                      : "—"}
                  </p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p className="text-muted-foreground">Shared selected buckets</p>
                  <p className="font-semibold text-foreground">
                    {unifiedEvidence.cooccurrenceCount != null
                      ? formatNumber(unifiedEvidence.cooccurrenceCount, { maximumFractionDigits: 0 })
                      : "—"}
                  </p>
                </div>
                <div
                  className="rounded border border-border bg-white px-2 py-1"
                  title="Buckets present ÷ expected buckets in the analysis window (after quality filtering and min-samples gating)."
                >
                  <p className="text-muted-foreground">Bucket coverage</p>
                  <p className="font-semibold text-foreground">
                    F {formatCoverage(unifiedEvidence.focusBucketCoveragePct)} · C{" "}
                    {formatCoverage(unifiedEvidence.candidateBucketCoveragePct)}
                  </p>
                </div>
                <div className="rounded border border-border bg-white px-2 py-1">
                  <p
                    className="text-muted-foreground"
                    title={
                      relationshipMode === "advanced" && unifiedEvidence.cooccurrenceScoreRaw != null
                        ? `Raw co-occ score: ${formatNumber(unifiedEvidence.cooccurrenceScoreRaw, { maximumFractionDigits: 1 })}`
                        : undefined
                    }
                  >
                    Co-occ strength
                  </p>
                  <p className="font-semibold text-foreground">
                    {unifiedEvidence.cooccurrenceStrength != null
                      ? formatScore(unifiedEvidence.cooccurrenceStrength)
                      : "—"}
                  </p>
                </div>
	                <div className="rounded border border-border bg-white px-2 py-1">
	                  <p className="text-muted-foreground" title={directionTooltip}>
	                    Direction
	                  </p>
	                  <p className="font-semibold text-foreground">
	                    {unifiedEvidence.directionLabel ?? "—"}
	                  </p>
	                </div>
                  <div className="rounded border border-border bg-white px-2 py-1">
                    <p
                      className="text-muted-foreground"
                      title={
                        unifiedEvidence.deltaCorr == null
                          ? "Signed correlation on bucket deltas at best lag. Not statistical significance. Not used for ranking unless enabled. Requires ≥10 aligned delta pairs."
                          : "Signed correlation on bucket deltas at best lag. Not statistical significance. Not used for ranking unless enabled."
                      }
                    >
                      Δ corr
                    </p>
                    <p className="font-semibold text-foreground">
                      {unifiedEvidence.deltaCorr != null
                        ? formatScore(unifiedEvidence.deltaCorr)
                        : "—"}
                    </p>
                  </div>
                  <div className="rounded border border-border bg-white px-2 py-1">
                    <p
                      className="text-muted-foreground"
                      title="Mitigate diurnal/periodic artifacts by downweighting sensors whose events cluster at fixed times-of-day. Weight = clamp(H_norm, 0.25, 1.0) where H_norm is normalized entropy over 24 hour-of-day bins (UTC)."
                    >
                      Periodic weight
                    </p>
                    <p className="font-semibold text-foreground">
                      {unifiedEvidence.timeOfDayEntropyWeight != null
                        ? formatScore(unifiedEvidence.timeOfDayEntropyWeight)
                        : "—"}
                    </p>
                  </div>
	                <div className="rounded border border-border bg-white px-2 py-1">
	                  <p className="text-muted-foreground">% focus events matched</p>
	                  <p className="font-semibold text-foreground">
	                    {formatCoverage(unifiedCoverage?.focusPct)}
	                  </p>
	                </div>
	                <div className="rounded border border-border bg-white px-2 py-1">
	                  <p className="text-muted-foreground">% candidate events matched</p>
	                  <p className="font-semibold text-foreground">
	                    {formatCoverage(unifiedCoverage?.candidatePct)}
	                  </p>
	                </div>
	                <div
	                  className="rounded border border-border bg-white px-2 py-1"
	                  title="Shared selected anomaly buckets ÷ total buckets in the analysis window."
	                >
	                  <p className="text-muted-foreground">% time buckets shared</p>
	                  <p className="font-semibold text-foreground">
	                    {formatCoverage(unifiedCoverage?.bucketsPct)}
	                  </p>
	                </div>
	              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                All evidence is computed on bucketed data at effective interval{" "}
                {effectiveIntervalSeconds ?? intervalSeconds ?? "—"}.
              </p>
              {unifiedEvidence.nFocus != null && unifiedEvidence.nCandidate != null ? (
                <p className="mt-1 text-xs text-muted-foreground">
                  Focus events: {formatNumber(unifiedEvidence.nFocus, { maximumFractionDigits: 0 })} · Candidate events:{" "}
                  {formatNumber(unifiedEvidence.nCandidate, { maximumFractionDigits: 0 })}
                </p>
              ) : null}
              {unifiedEvidence.summary.length > 0 && (
                <ul className="mt-3 list-disc space-y-1 pl-4 text-xs text-muted-foreground">
                  {unifiedEvidence.summary.map((line, idx) => (
                    <li key={`${line}-${idx}`}>{line}</li>
                  ))}
                </ul>
              )}
              {relationshipMode === "advanced" && unifiedEvidence.topLags.length > 0 && (
                <div className="mt-3">
                  <p className="text-xs font-semibold text-foreground">Top lags</p>
                  <div className="mt-1 space-y-2">
                    {unifiedEvidence.topLags.slice(0, 3).map((lag) => {
                      const isActive = lag.lag_sec === effectiveLagSec;
                      const matchedLabel = formatNumber(lag.overlap, { maximumFractionDigits: 0 });
                      const lagLabel = `Lag ${formatLagSeconds(lag.lag_sec)} (F1 ${formatScore(lag.score)}, matched ${matchedLabel})`;
                      return (
                        <div
                          key={lag.lag_sec}
                          className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-border bg-white px-3 py-2 text-xs"
                        >
                          <p className="font-semibold text-foreground" title={lagLabel}>
                            {lagLabel}
                          </p>
                          <NodeButton
                            type="button"
                            size="xs"
                            variant={isActive ? "secondary" : "ghost"}
                            disabled={isActive}
                            aria-label={
                              isActive
                                ? `Lag ${formatLagSeconds(lag.lag_sec)} is active`
                                : `Use lag ${formatLagSeconds(lag.lag_sec)} for preview alignment`
                            }
                            onClick={() => {
                              if (!candidate) return;
                              setAlignByLag(true);
                              setLagOverrideByCandidateId((prev) => {
                                const next = new Map(prev);
                                next.set(candidate.sensor_id, lag.lag_sec);
                                return next;
                              });
                            }}
                          >
                            {isActive ? "In use" : "Use this lag for preview alignment"}
                          </NodeButton>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
              {unifiedEvidence.timestamps.length > 0 && (
                <div className="mt-3">
	                  <p className="text-xs font-semibold text-foreground">Top shared bucket timestamps</p>
	                  <div className="mt-1 flex flex-wrap gap-1">
	                    {unifiedEvidence.timestamps.slice(0, 5).map((ts) => {
	                      const pill = (
	                        <NodePill tone="muted" size="sm" weight="normal">
	                          {formatDateTimeForTimeZone(new Date(ts), effectiveTimeZone, {
	                            month: "numeric",
	                            day: "numeric",
	                            hour: "numeric",
	                            minute: "2-digit",
	                          })}
	                        </NodePill>
	                      );
	                      if (!onJumpToTimestamp) return <span key={ts}>{pill}</span>;
	                      return (
	                        <button
	                          key={ts}
	                          type="button"
	                          onClick={() => onJumpToTimestamp(ts)}
	                          title="Jump to ±1h on the main Trends chart"
	                        >
	                          {pill}
	                        </button>
	                      );
	                    })}
	                  </div>
	                </div>
	              )}
            </Card>
          )}

          {/* Episodes list (for similarity/events) */}
          {episodes.length > 0 && (
            <div className="mt-4 space-y-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Episodes ({episodes.length})
              </p>
              <div className="max-h-48 space-y-2 overflow-y-auto">
                {episodes.map((episode, idx) => renderEpisodeButton(episode, idx))}
              </div>
            </div>
          )}

	          {/* Chart controls */}
	          <div className="mt-4 flex flex-wrap items-end justify-between gap-3">
	            <SegmentedControl
	              value={previewMode}
	              onChange={(next) => setPreviewMode(next as "normalized" | "raw")}
	              options={[
	                { value: "normalized", label: "Normalized" },
	                { value: "raw", label: "Raw" },
	              ]}
	              size="xs"
	            />

	            {onJumpToTimestamp && selectedEpisode ? (
	              <NodeButton
	                type="button"
	                size="xs"
	                variant="secondary"
	                onClick={() => {
	                  const startMs = safeParseTimestampMs(selectedEpisode.start_ts);
	                  const endMs = safeParseTimestampMs(selectedEpisode.end_ts);
	                  const centerMs =
	                    startMs != null && endMs != null && endMs > startMs
	                      ? Math.round((startMs + endMs) / 2)
	                      : startMs ?? endMs;
	                  if (centerMs == null) return;
	                  onJumpToTimestamp(centerMs);
	                }}
	              >
	                Jump to ±1h
	              </NodeButton>
	            ) : null}

	            {episodes.length > 0 && (
	              <label className="text-sm">
	                <span className="text-xs font-semibold text-foreground">Context</span>
	                <Select
                  className="mt-1 min-w-[160px] shadow-sm"
                  value={contextPreset}
                  onChange={(e) => setContextPreset(e.target.value as PreviewContextPreset)}
                >
                  <option value="auto">Auto</option>
                  <option value="episode">Episode</option>
                  <option value="1h">±1h</option>
                  <option value="3h">±3h</option>
                  <option value="6h">±6h</option>
                  <option value="24h">±24h</option>
                  <option value="72h">±72h</option>
                  <option value="custom">Custom…</option>
                </Select>
              </label>
            )}

            {episodes.length > 0 && contextPreset === "custom" ? (
              <label className="text-sm">
                <span className="text-xs font-semibold text-foreground">Custom ±hours</span>
                <NumericDraftInput
                  className="mt-1 h-9 w-28 rounded-lg border border-border bg-white px-3 text-sm text-foreground shadow-sm focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                  value={customContextHours}
                  onValueChange={(next) => {
                    if (typeof next !== "number" || !Number.isFinite(next)) return;
                    setCustomContextHours(next);
                  }}
                  min={0.1}
                  max={168}
                  clampOnBlur
                  step="0.1"
                  data-testid="auto-compare-context-custom-hours"
                  aria-label="Custom context plus/minus hours"
                />
              </label>
            ) : null}

	            {episodes.length > 0 ? (
	              <div className="flex flex-wrap items-center gap-3">
	                <label className="flex items-center gap-2 text-xs font-semibold text-foreground">
	                  <input
	                    type="checkbox"
	                    checked={alignByLag}
	                    onChange={(e) => setAlignByLag(e.target.checked)}
	                    className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
	                  />
	                  Align by lag
	                </label>
	                {hasEventOverlays ? (
	                  <label className="flex items-center gap-2 text-xs font-semibold text-foreground">
	                    <input
	                      type="checkbox"
	                      checked={showMatchedEventsOnly}
	                      onChange={(e) => setShowMatchedEventsOnly(e.target.checked)}
	                      className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
	                    />
	                    Show matched events only
	                  </label>
	                ) : null}
	              </div>
	            ) : null}
	          </div>
          {lagAlignmentSuppressed && (
            <p className="mt-2 text-xs text-muted-foreground">
              Lag alignment produced too few points for this candidate. Showing raw timeline instead.
            </p>
          )}

          {/* Preview chart */}
          <div className="mt-4">
            {previewQuery.isLoading ? (
              <LoadingState label="Loading preview…" />
            ) : previewQuery.error ? (
              <ErrorState
                message={
                  previewQuery.error instanceof Error
                    ? previewQuery.error.message
                    : "Failed to load preview."
                }
              />
            ) : previewSeries ? (
              <>
	                <TrendChart
	                  data={previewSeries}
	                  independentAxes={previewMode === "raw"}
	                  navigator={false}
	                  xPlotBands={episodeXPlotBands}
	                  xPlotLines={mergedXPlotLines}
	                  timeZone={effectiveTimeZone}
	                  heightPx={240}
	                />
                {previewData?.bucket_seconds && (
                  <p className="mt-2 text-xs text-muted-foreground">
                    Preview bucket size: {formatDuration(previewData.bucket_seconds)}
                  </p>
                )}
                {previewData?.focus.bucket_coverage_pct != null ||
                previewData?.candidate.bucket_coverage_pct != null ? (
                  <p
                    className="mt-1 text-xs text-muted-foreground"
                    title="Buckets present ÷ expected buckets in this preview window. Gaps reflect quality filtering and min-samples gating."
                  >
                    Bucket coverage (preview window): Focus{" "}
                    {formatCoverage(previewData?.focus.bucket_coverage_pct)} · Candidate{" "}
                    {formatCoverage(previewData?.candidate.bucket_coverage_pct)}
                  </p>
                ) : null}
              </>
            ) : episodes.length > 0 ? (
              <Card className="rounded-lg gap-0 border-dashed px-3 py-4 text-sm text-muted-foreground">
                Select an episode to load the preview chart.
              </Card>
            ) : (
              <Card className="rounded-lg gap-0 border-dashed px-3 py-4 text-sm text-muted-foreground">
                No preview available for this strategy.
              </Card>
            )}
          </div>
        </>
      )}

      {/* Computed through timestamp */}
      {computedThroughTs && (
 <p className="mt-3 text-xs text-muted-foreground">
          Computed through:{" "}
          {formatDateTimeForTimeZone(new Date(computedThroughTs), effectiveTimeZone, {
            month: "short",
            day: "numeric",
            year: "numeric",
            hour: "numeric",
            minute: "2-digit",
          })}
        </p>
      )}
    </Card>
  );
}
