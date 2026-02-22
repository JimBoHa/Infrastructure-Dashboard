"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { formatDistanceToNow } from "date-fns";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import InlineBanner from "@/components/InlineBanner";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import { Select } from "@/components/ui/select";
import AnalysisKey from "@/features/trends/components/AnalysisKey";
import { formatNumber } from "@/lib/format";
import { browserTimeZone, formatDateTimeForTimeZone } from "@/lib/siteTime";
import type { DemoNode, DemoSensor, TrendSeriesEntry } from "@/types/dashboard";
import type { EphemeralMarker } from "@/types/chartMarkers";
import type {
  CorrelationLagModeV1,
  CorrelationMatrixJobParamsV1,
  CorrelationMatrixResultV1,
  CorrelationValueModeV1,
  ExplicitFocusEventV1,
  RelatedSensorsUnifiedJobParamsV2,
  RelatedSensorsUnifiedResultV2,
} from "@/types/analysis";
import {
  type NormalizedCandidate,
  type RelationshipFinderMode,
  type SelectedBadge,
  type SharedControlState,
  type UnifiedControlState,
  DEFAULT_SHARED_CONTROLS,
  DEFAULT_UNIFIED_CONTROLS,
} from "../types/relationshipFinder";
import { useAnalysisJob } from "../hooks/useAnalysisJob";
import {
  createLookupMaps,
  normalizeUnifiedCandidates,
} from "../utils/candidateNormalizers";
import { pickStableCandidateId } from "../utils/relationshipFinderSelection";
import { generateCorrelationJobKey } from "../strategies/correlation";
import { buildRelatedMatrixSensorIds } from "../utils/correlationMatrixSelection";
import { collectDerivedDependentsOfFocus } from "../utils/derivedDependencies";
import {
  diagnoseUnifiedCandidateAbsence,
  PROVIDER_NO_HISTORY_LABEL,
} from "../utils/relatedSensorsUnifiedDiagnostics";
import { emitRelatedSensorsUxEvent } from "../utils/relatedSensorsUxEvents";
import type { RelatedSensorsExternalFocus } from "../types/relatedSensorsFocus";
import { CorrelationMatrix, ResultsList, PreviewPane } from "./relationshipFinder";

type RelationshipFinderPanelProps = {
  nodesById: Map<string, DemoNode>;
  sensors: DemoSensor[];
  series: TrendSeriesEntry[];
  selectedBadges: SelectedBadge[];
  selectedSensorIds: string[];
  labelMap: Map<string, string>;
  intervalSeconds: number;
  rangeHours: number;
  rangeSelect: string;
  customStartIso: string | null;
  customEndIso: string | null;
  customRangeValid: boolean;
  timeZone?: string;
  maxSeries: number;
  externalFocus?: RelatedSensorsExternalFocus | null;
  onClearExternalFocus?: () => void;
  onAddToChart?: (sensorId: string) => void;
  onAddEphemeralMarkers?: (markers: EphemeralMarker[]) => void;
  onJumpToTimestamp?: (timestampMs: number) => void;
};

type WindowRange = {
  startIso: string;
  endIso: string;
};

type LastRunSnapshot = {
  eligibleSensorIds: string[];
};

type RelatedSensorsUxRunKind = "quick" | "refine" | "advanced";
type RelatedSensorsUxRunTrigger = "manual" | "auto" | "refine_button";

type RelatedSensorsUxRunMeta = {
  kind: RelatedSensorsUxRunKind;
  trigger: RelatedSensorsUxRunTrigger;
  startedAtMs: number;
  focusSensorId: string;
  mode: RelationshipFinderMode;
  candidateSource: RelatedSensorsUnifiedJobParamsV2["candidate_source"];
  scope: SharedControlState["scope"];
  candidateLimit: number;
  maxResults: number;
  eligibleCount: number;
  candidatePoolCount: number;
  pinnedCount: number;
};

function shortLabel(label: string): string {
  const trimmed = label.trim();
  if (!trimmed) return trimmed;
  const parts = trimmed.split(" — ");
  const tail = parts.length > 1 ? parts.slice(1).join(" — ") : trimmed;
  return tail.replace(/\s*\([^)]*\)\s*$/, "").trim() || trimmed;
}

function getSensorSource(sensor: DemoSensor): string | null {
  const source = sensor.config?.["source"];
  return typeof source === "string" ? source : null;
}

function fnv1a64CandidateHash(ids: string[]): string {
  let hash = BigInt("0xcbf29ce484222325");
  const prime = BigInt("0x100000001b3");
  const mask = BigInt("0xffffffffffffffff");
  for (const id of ids) {
    for (let idx = 0; idx < id.length; idx += 1) {
      hash ^= BigInt(id.charCodeAt(idx));
      hash = (hash * prime) & mask;
    }
    hash ^= BigInt(0xff);
    hash = (hash * prime) & mask;
  }
  return hash.toString(16).padStart(16, "0");
}

function overlapAtK(left: string[], right: string[], k: number): number | null {
  const kk = Math.max(1, Math.floor(k));
  const leftTop = left.slice(0, kk);
  const rightSet = new Set(right.slice(0, kk));
  if (leftTop.length === 0) return null;
  let hits = 0;
  for (const sensorId of leftTop) {
    if (rightSet.has(sensorId)) hits += 1;
  }
  return hits / kk;
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

type CandidateSnapshot = {
  count: number;
  hash: string;
};

function buildJobKey(
  params: RelatedSensorsUnifiedJobParamsV2,
  candidateSnapshot?: CandidateSnapshot,
): string {
  const candidateIds = [...(params.candidate_sensor_ids ?? [])].sort();
  const pinnedIds = [...(params.pinned_sensor_ids ?? [])].sort();
  const focusEventIds = [...(params.focus_events ?? [])]
    .map((evt) => `${evt.ts}:${evt.severity ?? ""}`)
    .sort();
  const candidatesKey: CandidateSnapshot = candidateSnapshot ?? {
    count: candidateIds.length,
    hash: fnv1a64CandidateHash(candidateIds),
  };
  const focusKey: CandidateSnapshot = {
    count: focusEventIds.length,
    hash: fnv1a64CandidateHash(focusEventIds),
  };
  return JSON.stringify({
    v: 4,
    focus: params.focus_sensor_id,
    start: params.start,
    end: params.end,
    interval: params.interval_seconds,
    mode: params.mode,
    candidateSource: params.candidate_source,
    evaluateAllEligible: params.evaluate_all_eligible,
    quick: params.quick_suggest,
    stabilityEnabled: params.stability_enabled,
    excludeSystemWideBuckets: params.exclude_system_wide_buckets,
    candidateLimit: params.candidate_limit,
    maxResults: params.max_results,
    includeLow: params.include_low_confidence,
    weights: params.weights,
    includeDeltaCorrSignal: params.include_delta_corr_signal,
    preprocess: {
      deseason: params.deseason_mode,
      periodicPenaltyEnabled: params.periodic_penalty_enabled,
    },
    event: {
      polarity: params.polarity,
      z: params.z_threshold,
      zCap: params.z_cap,
      minSep: params.min_separation_buckets,
      gapMaxBuckets: params.gap_max_buckets,
      maxLag: params.max_lag_buckets,
      maxEvents: params.max_events,
      maxEpisodes: params.max_episodes,
      episodeGap: params.episode_gap_buckets,
    },
    focusEvents: {
      count: focusKey.count,
      hash: focusKey.hash,
    },
    cooccurrence: {
      tolerance: params.tolerance_buckets,
      minSensors: params.min_sensors,
    },
    filters: params.filters,
    candidates: {
      count: candidatesKey.count,
      hash: candidatesKey.hash,
    },
    pinned: {
      count: pinnedIds.length,
      hash: fnv1a64CandidateHash(pinnedIds),
    },
  });
}

const RELATED_MATRIX_MAX_SENSORS = 25;

export default function RelationshipFinderPanel({
  nodesById,
  sensors,
  series,
  selectedBadges,
  selectedSensorIds,
  labelMap,
  intervalSeconds,
  rangeHours,
  rangeSelect,
  customStartIso,
  customEndIso,
  customRangeValid,
  timeZone,
  maxSeries,
  externalFocus,
  onClearExternalFocus,
  onAddToChart,
  onAddEphemeralMarkers,
  onJumpToTimestamp,
}: RelationshipFinderPanelProps) {
  const effectiveTimeZone = timeZone ?? browserTimeZone();

  const sensorsById = useMemo(() => new Map(sensors.map((s) => [s.sensor_id, s])), [sensors]);

  const badgeById = useMemo(() => {
    const map = new Map<string, SelectedBadge>();
    selectedBadges.forEach((badge) => map.set(badge.sensorId, badge));
    return map;
  }, [selectedBadges]);

  const lookups = useMemo(() => createLookupMaps(sensors, nodesById, labelMap), [sensors, nodesById, labelMap]);

  const [mode, setMode] = useState<RelationshipFinderMode>("simple");
  const [shared, setShared] = useState<SharedControlState>(() => ({
    ...DEFAULT_SHARED_CONTROLS,
    focusSensorId: selectedSensorIds[0] ?? null,
  }));
  const [controls, setControls] = useState<UnifiedControlState>(DEFAULT_UNIFIED_CONTROLS);

  const [candidates, setCandidates] = useState<NormalizedCandidate[]>([]);
  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(null);
  const [computedThroughTs, setComputedThroughTs] = useState<string | null>(null);
  const [visibleCount, setVisibleCount] = useState(25);
  const [lastRunSnapshot, setLastRunSnapshot] = useState<LastRunSnapshot | null>(null);
  const [diagnosticSensorId, setDiagnosticSensorId] = useState<string>("");
  const [pinnedSensorIds, setPinnedSensorIds] = useState<string[]>([]);
  const [pinDraftSensorId, setPinDraftSensorId] = useState<string>("");
  const [correlationValueMode, setCorrelationValueMode] =
    useState<CorrelationValueModeV1>("levels");
  const [correlationLagMode, setCorrelationLagMode] =
    useState<CorrelationLagModeV1>("aligned");
  const [correlationMaxLagBuckets, setCorrelationMaxLagBuckets] =
    useState<number>(controls.maxLagBuckets);
  const [showFullCorrelationMatrix, setShowFullCorrelationMatrix] =
    useState<boolean>(false);

  const lastResultIdRef = useRef<string | null>(null);
  const autoSuggestKeyRef = useRef<string | null>(null);
  const lastMatrixJobKeyRef = useRef<string | null>(null);
  const panelOpenedRef = useRef(false);
  const lastRunMetaRef = useRef<RelatedSensorsUxRunMeta | null>(null);
  const lastCompletedRunRef = useRef<{
    focusSensorId: string;
    kind: RelatedSensorsUxRunKind;
    top10SensorIds: string[];
  } | null>(null);

  useEffect(() => {
    emitRelatedSensorsUxEvent("panel_opened", {
      panel: "related_sensors",
      reason: "mount",
      pathname: globalThis.location?.pathname ?? null,
    });
    panelOpenedRef.current = true;
  }, []);

  const focusSensorId = useMemo(() => {
    if (shared.focusSensorId && sensorsById.has(shared.focusSensorId)) {
      return shared.focusSensorId;
    }
    return selectedSensorIds[0] ?? null;
  }, [selectedSensorIds, sensorsById, shared.focusSensorId]);

  const focusSensorOptions = useMemo(() => {
    const ids = [...selectedSensorIds];
    if (focusSensorId && !ids.includes(focusSensorId)) {
      ids.unshift(focusSensorId);
    }
    return ids;
  }, [focusSensorId, selectedSensorIds]);

  const explicitFocusEvents = useMemo(() => {
    if (!externalFocus) return [] as ExplicitFocusEventV1[];
    if (!focusSensorId || externalFocus.focusSensorId !== focusSensorId) {
      return [] as ExplicitFocusEventV1[];
    }

    const dedup = new Map<string, ExplicitFocusEventV1>();
    for (const window of externalFocus.windows) {
      const ts = window.startIso.trim();
      if (!ts) continue;
      const severity =
        typeof window.severity === "number" && Number.isFinite(window.severity)
          ? window.severity
          : null;
      const existing = dedup.get(ts);
      if (!existing) {
        dedup.set(ts, { ts, severity: severity ?? undefined });
        continue;
      }
      const prev = existing.severity;
      const prevValue =
        typeof prev === "number" && Number.isFinite(prev)
          ? prev
          : Number.NEGATIVE_INFINITY;
      const nextValue =
        typeof severity === "number" && Number.isFinite(severity)
          ? severity
          : Number.NEGATIVE_INFINITY;
      if (nextValue > prevValue) {
        dedup.set(ts, { ts, severity: severity ?? undefined });
      }
    }

    return Array.from(dedup.values()).sort((a, b) => a.ts.localeCompare(b.ts));
  }, [externalFocus, focusSensorId]);

  const lastExternalFocusAppliedRef = useRef<number | null>(null);
  const pendingExternalFocusRunRef = useRef<number | null>(null);

  useEffect(() => {
    if (!externalFocus) return;
    if (lastExternalFocusAppliedRef.current === externalFocus.requestedAtMs) return;
    lastExternalFocusAppliedRef.current = externalFocus.requestedAtMs;

    const nextFocusSensorId = externalFocus.focusSensorId;
    const requestedAtMs = externalFocus.requestedAtMs;
    const timer = globalThis.setTimeout(() => {
      setShared((prev) => ({ ...prev, focusSensorId: nextFocusSensorId }));
      pendingExternalFocusRunRef.current = requestedAtMs;
    }, 0);
    return () => globalThis.clearTimeout(timer);
  }, [externalFocus]);

  const effectivePinnedSensorIds = useMemo(() => {
    const focus = focusSensorId?.trim() ?? "";
    const normalized = pinnedSensorIds
      .map((id) => id.trim())
      .filter((id) => Boolean(id) && (!focus || id !== focus));
    normalized.sort();
    return normalized;
  }, [focusSensorId, pinnedSensorIds]);

  const pinnedSensorIdSet = useMemo(
    () => new Set(effectivePinnedSensorIds),
    [effectivePinnedSensorIds],
  );

  const togglePinnedSensorId = useCallback(
    (sensorId: string) => {
      const normalized = sensorId.trim();
      if (!normalized) return;
      if (focusSensorId && normalized === focusSensorId) return;

      setPinnedSensorIds((prev) => {
        const next = new Set(prev.map((id) => id.trim()).filter(Boolean));
        if (next.has(normalized)) {
          next.delete(normalized);
        } else {
          next.add(normalized);
        }
        return Array.from(next).sort();
      });
    },
    [focusSensorId],
  );

  const pinSensorId = useCallback(
    (sensorId: string) => {
      const normalized = sensorId.trim();
      if (!normalized) return;
      if (focusSensorId && normalized === focusSensorId) return;

      setPinnedSensorIds((prev) => {
        if (prev.includes(normalized)) return prev;
        return [...prev, normalized].map((id) => id.trim()).filter(Boolean).sort();
      });
    },
    [focusSensorId],
  );

  const unpinSensorId = useCallback((sensorId: string) => {
    const normalized = sensorId.trim();
    if (!normalized) return;
    setPinnedSensorIds((prev) => prev.filter((id) => id !== normalized));
  }, []);

  const clearPinnedSensorIds = useCallback(() => {
    setPinnedSensorIds([]);
  }, []);

  const pinOptionSensorIds = useMemo(() => {
    const ids: string[] = [];
    for (const sensor of sensors) {
      const id = sensor.sensor_id.trim();
      if (!id) continue;
      if (focusSensorId && id === focusSensorId) continue;
      if (pinnedSensorIdSet.has(id)) continue;
      ids.push(id);
    }
    ids.sort();
    return ids;
  }, [focusSensorId, pinnedSensorIdSet, sensors]);

  const analysisJob = useAnalysisJob<RelatedSensorsUnifiedResultV2>({
    onComplete: (payload) => {
      const result = payload as RelatedSensorsUnifiedResultV2;
      const runMeta = lastRunMetaRef.current;
      const top10SensorIds = result.candidates.slice(0, 10).map((candidate) => candidate.sensor_id);
      const tierCounts = { high: 0, medium: 0, low: 0 } as Record<string, number>;
      for (const candidate of result.candidates) {
        const tier = candidate.confidence_tier;
        if (typeof tier === "string") {
          tierCounts[tier] = (tierCounts[tier] ?? 0) + 1;
        }
      }
      const strongCount = (tierCounts.high ?? 0) + (tierCounts.medium ?? 0);
      const previousCompleted = lastCompletedRunRef.current;
      const quickRefineOverlapAt10 =
        runMeta?.kind === "refine" &&
        previousCompleted?.kind === "quick" &&
        previousCompleted.focusSensorId === result.focus_sensor_id
          ? overlapAtK(previousCompleted.top10SensorIds, top10SensorIds, 10)
          : null;

      const derivedKind: RelatedSensorsUxRunKind =
        result.params?.mode === "advanced"
          ? "advanced"
          : result.params?.quick_suggest
            ? "quick"
            : "refine";
      const effectiveKind = runMeta?.kind ?? derivedKind;

      emitRelatedSensorsUxEvent("run_completed", {
        panel: "related_sensors",
        status: "completed",
        kind: effectiveKind,
        trigger: runMeta?.trigger ?? "manual",
        focus_sensor_id: result.focus_sensor_id,
        candidate_source: result.params?.candidate_source ?? null,
        scope: runMeta?.scope ?? null,
        result_count: result.candidates.length,
        strong_count: strongCount,
        tier_counts: tierCounts,
        counts: result.counts ?? null,
        timings_ms: result.timings_ms ?? null,
        quick_refine_overlap_at_10: quickRefineOverlapAt10,
        elapsed_ms: runMeta ? Date.now() - runMeta.startedAtMs : null,
      });
      lastCompletedRunRef.current = {
        focusSensorId: result.focus_sensor_id,
        kind: effectiveKind,
        top10SensorIds,
      };

      const resultId = JSON.stringify({
        focus: result.focus_sensor_id,
        computed_through_ts: result.computed_through_ts,
        candidates: result.candidates.map((candidate) => candidate.sensor_id),
      });
      const isNewResult = lastResultIdRef.current !== resultId;
      lastResultIdRef.current = resultId;

      const normalized = normalizeUnifiedCandidates(result, lookups);
      setCandidates(normalized);
      setComputedThroughTs(result.computed_through_ts ?? null);
      if (isNewResult) {
        setVisibleCount(25);
      }
      setSelectedCandidateId((prev) =>
        pickStableCandidateId({ previousId: prev, candidates: normalized }),
      );

      if (isNewResult && onAddEphemeralMarkers) {
        const markers: EphemeralMarker[] = [];
        for (const candidate of normalized.slice(0, 6)) {
          if (candidate.raw.type !== "unified") continue;
          for (const ts of (candidate.raw.data.top_bucket_timestamps ?? []).slice(0, 3)) {
            const when = new Date(ts);
            if (!Number.isFinite(when.getTime())) continue;
            markers.push({
              id: `rf-unified-${candidate.sensor_id}-${ts}`,
              timestamp: when,
              label: "Related event",
              source: "event_match",
              detail: `${candidate.label} co-occurrence`,
              sensorIds: [focusSensorId ?? "", candidate.sensor_id].filter(Boolean),
            });
          }
        }
        if (markers.length > 0) {
          onAddEphemeralMarkers(markers);
        }
      }
    },
    onError: (message) => {
      const runMeta = lastRunMetaRef.current;
      emitRelatedSensorsUxEvent("run_completed", {
        panel: "related_sensors",
        status: "failed",
        kind: runMeta?.kind ?? null,
        trigger: runMeta?.trigger ?? null,
        focus_sensor_id: runMeta?.focusSensorId ?? focusSensorId ?? null,
        error: message,
        elapsed_ms: runMeta ? Date.now() - runMeta.startedAtMs : null,
      });
    },
    onCancel: () => {
      const runMeta = lastRunMetaRef.current;
      emitRelatedSensorsUxEvent("run_completed", {
        panel: "related_sensors",
        status: "canceled",
        kind: runMeta?.kind ?? null,
        trigger: runMeta?.trigger ?? null,
        focus_sensor_id: runMeta?.focusSensorId ?? focusSensorId ?? null,
        elapsed_ms: runMeta ? Date.now() - runMeta.startedAtMs : null,
      });
    },
  });
  const matrixJob = useAnalysisJob<CorrelationMatrixResultV1>();
  const runMatrixJob = matrixJob.run;
  const resetMatrixJob = matrixJob.reset;

  const canComputeWindow = useMemo(() => {
    if (rangeSelect === "custom") {
      return Boolean(customStartIso && customEndIso && customRangeValid);
    }
    return Number.isFinite(rangeHours) && rangeHours > 0;
  }, [customEndIso, customRangeValid, customStartIso, rangeHours, rangeSelect]);

  const computeWindow = useCallback((): WindowRange | null => {
    if (!canComputeWindow) return null;

    if (rangeSelect === "custom" && customStartIso && customEndIso) {
      return { startIso: customStartIso, endIso: customEndIso };
    }

    const end = new Date();
    end.setMilliseconds(0);
    const start = new Date(end.getTime() - rangeHours * 60 * 60 * 1000);
    return { startIso: start.toISOString(), endIso: end.toISOString() };
  }, [
    canComputeWindow,
    customEndIso,
    customStartIso,
    rangeHours,
    rangeSelect,
  ]);

  const includeProviderSensors = mode === "advanced" && shared.includeProviderSensors;
  const excludeDerivedFromFocus = mode === "simple" || !shared.includeDerivedFromFocus;
  const candidateSource =
    mode === "advanced" ? shared.candidateSource : "all_sensors_in_scope";

  const derivedDependentsOfFocus = useMemo(() => {
    if (!focusSensorId || !excludeDerivedFromFocus) return null;
    return collectDerivedDependentsOfFocus(focusSensorId, sensorsById, {
      maxDepth: 10,
      maxVisited: 5000,
    });
  }, [excludeDerivedFromFocus, focusSensorId, sensorsById]);

  const excludeSensorIds = useMemo(() => {
    const ids = new Set<string>();
    if (focusSensorId) ids.add(focusSensorId);
    if (excludeDerivedFromFocus && derivedDependentsOfFocus) {
      derivedDependentsOfFocus.forEach((id) => ids.add(id));
    }
    return Array.from(ids).sort();
  }, [derivedDependentsOfFocus, excludeDerivedFromFocus, focusSensorId]);

  const candidatePoolIds = useMemo(() => {
    if (!focusSensorId) return [];
    const focus = sensorsById.get(focusSensorId);
    if (!focus) return [];

    return sensors
      .filter((sensor) => sensor.sensor_id !== focusSensorId)
      .filter((sensor) => {
        if (shared.scope === "same_node" && sensor.node_id !== focus.node_id) {
          return false;
        }
        if (shared.sameUnitOnly && sensor.unit !== focus.unit) {
          return false;
        }
        if (shared.sameTypeOnly && sensor.type !== focus.type) {
          return false;
        }
        if (
          excludeDerivedFromFocus &&
          derivedDependentsOfFocus &&
          derivedDependentsOfFocus.has(sensor.sensor_id)
        ) {
          return false;
        }
        if (!includeProviderSensors && getSensorSource(sensor) === "forecast_points") {
          return false;
        }
        return true;
      })
      .map((sensor) => sensor.sensor_id)
      .sort();
  }, [
    focusSensorId,
    sensorsById,
    sensors,
    shared.scope,
    shared.sameUnitOnly,
    shared.sameTypeOnly,
    excludeDerivedFromFocus,
    derivedDependentsOfFocus,
    includeProviderSensors,
  ]);

  const runAnalysis = useCallback(
    async (opts?: { force?: boolean; trigger?: RelatedSensorsUxRunTrigger }) => {
      const window = computeWindow();
      if (!window || !focusSensorId) return;

      const trigger: RelatedSensorsUxRunTrigger = opts?.trigger ?? "manual";
      const runKind: RelatedSensorsUxRunKind =
        mode === "advanced" ? "advanced" : "refine";
      const candidateLimit = controls.candidateLimit;
      const maxResults = controls.maxResults;
      const candidateSensorIds =
        candidateSource === "all_sensors_in_scope" ? [] : candidatePoolIds;
      const pinnedIds = effectivePinnedSensorIds;
      const eligibleSensorIds = Array.from(
        new Set([...candidatePoolIds, ...pinnedIds]),
      ).sort();
      const candidateSnapshot: CandidateSnapshot = {
        count: candidatePoolIds.length,
        hash: fnv1a64CandidateHash(candidatePoolIds),
      };

      const effectiveZThreshold =
        mode === "simple" && shared.scope === "all_nodes"
          ? controls.zThreshold + 0.5
          : controls.zThreshold;
      const effectiveMinSensors =
        mode === "simple" && shared.scope === "all_nodes"
          ? Math.max(controls.minSensors, 3)
          : controls.minSensors;

      const params: RelatedSensorsUnifiedJobParamsV2 = {
        focus_sensor_id: focusSensorId,
        start: window.startIso,
        end: window.endIso,
        focus_events: explicitFocusEvents.length > 0 ? explicitFocusEvents : undefined,
        interval_seconds: intervalSeconds,
        mode,
        candidate_source: candidateSource,
        stability_enabled:
          mode === "advanced" && controls.stabilityEnabled ? true : undefined,
        exclude_system_wide_buckets: shared.excludeSystemWideBuckets,
        candidate_sensor_ids: candidateSensorIds,
        pinned_sensor_ids: pinnedIds,
        evaluate_all_eligible: shared.evaluateAllEligible ? true : undefined,
        candidate_limit: candidateLimit,
        max_results: maxResults,
        include_low_confidence: controls.includeLowConfidence,
        include_delta_corr_signal:
          mode === "advanced" && controls.includeDeltaCorrSignal ? true : undefined,
        weights: {
          events: controls.eventsWeight,
          cooccurrence: controls.cooccurrenceWeight,
          delta_corr:
            mode === "advanced" && controls.includeDeltaCorrSignal
              ? controls.deltaCorrWeight
              : undefined,
        },
        deseason_mode: mode === "advanced" ? controls.deseasonMode : undefined,
        periodic_penalty_enabled:
          mode === "advanced" ? controls.periodicPenaltyEnabled : undefined,
        cooccurrence_score_mode:
          mode === "advanced" ? controls.cooccurrenceScoreMode : undefined,
        cooccurrence_bucket_preference_mode:
          mode === "advanced" ? controls.cooccurrenceBucketPreferenceMode : undefined,
        polarity: controls.polarity,
        z_threshold: effectiveZThreshold,
        z_cap: controls.zCap,
        min_separation_buckets: controls.minSeparationBuckets,
        gap_max_buckets: controls.gapMaxBuckets,
        max_lag_buckets: controls.maxLagBuckets,
        max_events: controls.maxEvents,
        max_episodes: controls.maxEpisodes,
        episode_gap_buckets: controls.episodeGapBuckets,
        tolerance_buckets: controls.toleranceBuckets,
        min_sensors: effectiveMinSensors,
        filters: {
          same_node_only: shared.scope === "same_node",
          same_unit_only: shared.sameUnitOnly,
          same_type_only: shared.sameTypeOnly,
          is_public_provider: includeProviderSensors ? undefined : false,
          exclude_sensor_ids: excludeSensorIds,
        },
      };

      setLastRunSnapshot({ eligibleSensorIds });
      setDiagnosticSensorId("");

      emitRelatedSensorsUxEvent("run_started", {
        panel: "related_sensors",
        kind: runKind,
        trigger,
        focus_sensor_id: focusSensorId,
        mode,
        range_select: rangeSelect,
        window_start: window.startIso,
        window_end: window.endIso,
        interval_seconds: intervalSeconds,
        candidate_source: candidateSource,
        scope: shared.scope,
        same_unit_only: shared.sameUnitOnly,
        same_type_only: shared.sameTypeOnly,
        include_provider_sensors: includeProviderSensors,
        candidate_limit: candidateLimit,
        max_results: maxResults,
        candidate_pool_count: candidatePoolIds.length,
        pinned_count: pinnedIds.length,
        eligible_count: eligibleSensorIds.length,
      });
      lastRunMetaRef.current = {
        kind: runKind,
        trigger,
        startedAtMs: Date.now(),
        focusSensorId,
        mode,
        candidateSource,
        scope: shared.scope,
        candidateLimit,
        maxResults,
        eligibleCount: eligibleSensorIds.length,
        candidatePoolCount: candidatePoolIds.length,
        pinnedCount: pinnedIds.length,
      };

      await analysisJob.run(
        "related_sensors_unified_v2",
        params,
        buildJobKey(params, candidateSnapshot),
      );
      if (opts?.force) {
        autoSuggestKeyRef.current = null;
      }
    },
    [
      analysisJob,
      candidatePoolIds,
      computeWindow,
      controls,
      intervalSeconds,
      mode,
      focusSensorId,
      explicitFocusEvents,
      shared.scope,
      shared.sameTypeOnly,
      shared.sameUnitOnly,
      shared.excludeSystemWideBuckets,
      includeProviderSensors,
      excludeSensorIds,
      candidateSource,
      effectivePinnedSensorIds,
      shared.evaluateAllEligible,
      rangeSelect,
    ],
  );

  useEffect(() => {
    if (!externalFocus) return;
    const pending = pendingExternalFocusRunRef.current;
    if (!pending) return;
    if (analysisJob.isRunning || analysisJob.isSubmitting) return;
    if (!focusSensorId) return;
    if (focusSensorId !== externalFocus.focusSensorId) return;

    pendingExternalFocusRunRef.current = null;
    const timer = globalThis.setTimeout(() => {
      void runAnalysis({ force: true, trigger: "auto" });
    }, 0);
    return () => globalThis.clearTimeout(timer);
  }, [analysisJob.isRunning, analysisJob.isSubmitting, externalFocus, focusSensorId, runAnalysis]);

  useEffect(() => {
    if (mode !== "simple") return;
    if (externalFocus) return;
    if (!focusSensorId || !canComputeWindow) return;
    if (analysisJob.isRunning || analysisJob.isSubmitting) return;

	      const autoKey = [
	        focusSensorId,
	        rangeSelect,
	        rangeSelect === "custom" ? customStartIso : "",
	        rangeSelect === "custom" ? customEndIso : "",
	        rangeSelect === "custom" ? "" : rangeHours,
	        intervalSeconds,
		        shared.scope,
		        shared.sameUnitOnly,
		        shared.sameTypeOnly,
		        excludeDerivedFromFocus,
		        shared.excludeSystemWideBuckets,
		        includeProviderSensors,
		        controls.zThreshold,
	        controls.zCap,
	        controls.minSeparationBuckets,
	        controls.gapMaxBuckets,
	        controls.toleranceBuckets,
	      ].join("|");

    if (autoSuggestKeyRef.current === autoKey) return;
    autoSuggestKeyRef.current = autoKey;
    const timer = globalThis.setTimeout(() => {
      void runAnalysis({ trigger: "auto" });
    }, 0);
    return () => globalThis.clearTimeout(timer);
  }, [
    mode,
    externalFocus,
    focusSensorId,
	    shared.scope,
	    shared.sameTypeOnly,
	    shared.sameUnitOnly,
	    excludeDerivedFromFocus,
	    shared.excludeSystemWideBuckets,
		    includeProviderSensors,
		    controls.zThreshold,
	    controls.zCap,
	    controls.minSeparationBuckets,
		    controls.gapMaxBuckets,
	    controls.toleranceBuckets,
	    intervalSeconds,
    canComputeWindow,
    rangeSelect,
    customStartIso,
    customEndIso,
    rangeHours,
    analysisJob.isRunning,
    analysisJob.isSubmitting,
    runAnalysis,
  ]);

  const focusLabel = focusSensorId ? labelMap.get(focusSensorId) ?? focusSensorId : "";

  const selectedCandidate = useMemo(
    () => candidates.find((candidate) => candidate.sensor_id === selectedCandidateId) ?? null,
    [candidates, selectedCandidateId],
  );

  const handleCandidateOpened = useCallback(
    (sensorId: string, source: string) => {
      setSelectedCandidateId(sensorId);

      const candidate = candidates.find((entry) => entry.sensor_id === sensorId) ?? null;
      const confidenceTier =
        candidate?.raw.type === "unified" ? candidate.raw.data.confidence_tier : null;

      emitRelatedSensorsUxEvent("candidate_opened", {
        panel: "related_sensors",
        source,
        focus_sensor_id: focusSensorId,
        candidate_sensor_id: sensorId,
        rank: candidate?.rank ?? null,
        rank_score: candidate?.score ?? null,
        confidence_tier: confidenceTier,
      });
    },
    [candidates, focusSensorId],
  );

  const handleAddToChartFromResults = useCallback(
    (sensorId: string) => {
      emitRelatedSensorsUxEvent("add_to_chart_clicked", {
        panel: "related_sensors",
        source: "results_list",
        focus_sensor_id: focusSensorId,
        sensor_id: sensorId,
        already_on_chart: selectedSensorIds.includes(sensorId),
        chart_count: selectedSensorIds.length,
        chart_max_series: maxSeries,
      });
      onAddToChart?.(sensorId);
    },
    [focusSensorId, maxSeries, onAddToChart, selectedSensorIds],
  );

  const handleAddToChartFromPreview = useCallback(
    (sensorId: string) => {
      emitRelatedSensorsUxEvent("add_to_chart_clicked", {
        panel: "related_sensors",
        source: "preview",
        focus_sensor_id: focusSensorId,
        sensor_id: sensorId,
        already_on_chart: selectedSensorIds.includes(sensorId),
        chart_count: selectedSensorIds.length,
        chart_max_series: maxSeries,
      });
      onAddToChart?.(sensorId);
    },
    [focusSensorId, maxSeries, onAddToChart, selectedSensorIds],
  );

  const handleTogglePinnedFromResults = useCallback(
    (sensorId: string) => {
      const normalized = sensorId.trim();
      if (!normalized) return;
      const willPin = !pinnedSensorIdSet.has(normalized);
      emitRelatedSensorsUxEvent("pin_toggled", {
        panel: "related_sensors",
        source: "results_list",
        focus_sensor_id: focusSensorId,
        sensor_id: normalized,
        action: willPin ? "pin" : "unpin",
      });
      togglePinnedSensorId(normalized);
    },
    [focusSensorId, pinnedSensorIdSet, togglePinnedSensorId],
  );

  const handleJumpToTimestampWithUx = useCallback(
    (timestampMs: number, source: string) => {
      if (!onJumpToTimestamp) return;
      emitRelatedSensorsUxEvent("jump_to_timestamp_clicked", {
        panel: "related_sensors",
        source,
        focus_sensor_id: focusSensorId,
        timestamp_ms: timestampMs,
      });
      onJumpToTimestamp(timestampMs);
    },
    [focusSensorId, onJumpToTimestamp],
  );

  const relatedMatrixSensorIds = useMemo(
    () =>
      buildRelatedMatrixSensorIds({
        focusSensorId,
        candidates,
        scoreCutoff: controls.matrixScoreCutoff,
        maxSensors: RELATED_MATRIX_MAX_SENSORS,
      }),
    [focusSensorId, candidates, controls.matrixScoreCutoff],
  );

  useEffect(() => {
    if (!analysisJob.isCompleted || !focusSensorId) {
      lastMatrixJobKeyRef.current = null;
      return;
    }
    const window = computeWindow();
    if (!window) {
      lastMatrixJobKeyRef.current = null;
      return;
    }
    if (relatedMatrixSensorIds.length < 2) {
      lastMatrixJobKeyRef.current = null;
      resetMatrixJob();
      return;
    }

    const normalizedMaxLagBuckets = Math.min(
      240,
      Math.max(0, Math.round(correlationMaxLagBuckets)),
    );
    const params: CorrelationMatrixJobParamsV1 = {
      sensor_ids: relatedMatrixSensorIds,
      start: window.startIso,
      end: window.endIso,
      interval_seconds: intervalSeconds,
      method: "pearson",
      min_overlap: 10,
      min_significant_n: 10,
      significance_alpha: 0.05,
      min_abs_r: 0.2,
      bucket_aggregation_mode: "auto",
      value_mode: correlationValueMode,
      lag_mode: correlationLagMode,
      max_lag_buckets:
        correlationLagMode === "best_within_max" ? normalizedMaxLagBuckets : undefined,
      max_sensors: RELATED_MATRIX_MAX_SENSORS + 1,
    };
    const matrixJobKey = generateCorrelationJobKey(params);
    if (lastMatrixJobKeyRef.current === matrixJobKey) return;
    lastMatrixJobKeyRef.current = matrixJobKey;

    void runMatrixJob(
      "correlation_matrix_v1",
      params,
      matrixJobKey,
    );
  }, [
    analysisJob.isCompleted,
    computeWindow,
    correlationLagMode,
    correlationMaxLagBuckets,
    correlationValueMode,
    focusSensorId,
    intervalSeconds,
    relatedMatrixSensorIds,
    resetMatrixJob,
    runMatrixJob,
  ]);

  const rankBySensorId = useMemo(() => {
    const map = new Map<string, number>();
    candidates.forEach((candidate, index) => {
      map.set(candidate.sensor_id, candidate.rank || index + 1);
    });
    return map;
  }, [candidates]);

  const handleMatrixPairSelect = useCallback(
    (rowSensorId: string, colSensorId: string) => {
      if (!focusSensorId) return;

      let nextCandidateId: string | null = null;
      if (rowSensorId === focusSensorId) {
        nextCandidateId = colSensorId;
      } else if (colSensorId === focusSensorId) {
        nextCandidateId = rowSensorId;
      } else {
        const rowRank = rankBySensorId.get(rowSensorId) ?? Number.POSITIVE_INFINITY;
        const colRank = rankBySensorId.get(colSensorId) ?? Number.POSITIVE_INFINITY;
        nextCandidateId = rowRank <= colRank ? rowSensorId : colSensorId;
      }

      if (!nextCandidateId) return;
      if (!candidates.some((candidate) => candidate.sensor_id === nextCandidateId)) return;
      handleCandidateOpened(nextCandidateId, "correlation_matrix");
    },
    [candidates, focusSensorId, handleCandidateOpened, rankBySensorId],
  );

  const runDisabled = useMemo(() => {
    if (analysisJob.isSubmitting || analysisJob.isRunning) return true;
    if (!focusSensorId || !canComputeWindow) return true;
    if (candidatePoolIds.length === 0 && effectivePinnedSensorIds.length === 0) return true;
    return false;
  }, [
    analysisJob.isSubmitting,
    analysisJob.isRunning,
    focusSensorId,
    canComputeWindow,
    candidatePoolIds.length,
    effectivePinnedSensorIds.length,
  ]);

  const eligibleCountEstimate = useMemo(() => {
    const ids = new Set<string>();
    candidatePoolIds.forEach((id) => ids.add(id));
    effectivePinnedSensorIds.forEach((id) => ids.add(id));
    return ids.size;
  }, [candidatePoolIds, effectivePinnedSensorIds]);
  const stabilityEligibleEstimate = eligibleCountEstimate <= 120;

  const progressLabel = useMemo(() => {
    if (!analysisJob.progress) return analysisJob.progressMessage ?? "Running analysis…";
    const { phase, completed, total } = analysisJob.progress;
    const phaseLabel: Record<string, string> = {
      unified_prepare: "Preparing…",
      events: "Scoring event alignment…",
      cooccurrence: "Scoring co-occurrence…",
      merge: "Merging ranked insights…",
      load_series: "Loading data…",
      detect_events: "Detecting events…",
      match_candidates: "Matching candidates…",
      score_buckets: "Scoring buckets…",
    };

    const label = phaseLabel[phase] ?? "Processing…";
    if (total && total > 0) {
      const pct = Math.round((completed / total) * 100);
      return `${label} (${pct}%)`;
    }
    return label;
  }, [analysisJob.progress, analysisJob.progressMessage]);
  const matrixProgressLabel = useMemo(() => {
    if (!matrixJob.progress) return matrixJob.progressMessage ?? "Computing correlation matrix…";
    const phase = matrixJob.progress.phase;
    if (phase === "load_series") return "Loading matrix series…";
    if (phase === "correlate") return "Computing correlation matrix…";
    return "Computing correlation matrix…";
  }, [matrixJob.progress, matrixJob.progressMessage]);

  const matrixCandidateCount = Math.max(0, relatedMatrixSensorIds.length - 1);
  const matrixCutoffLabel = controls.matrixScoreCutoff.toFixed(2);
  const eligibleCountFromResult = analysisJob.result?.counts?.eligible_count ?? 0;
  const evaluatedCountFromResult = analysisJob.result?.counts?.evaluated_count ?? 0;

  const candidateSourceDisclosure = useMemo(() => {
    const source = analysisJob.result?.params?.candidate_source ?? "all_sensors_in_scope";
    if (source === "all_sensors_in_scope") {
      return {
        label: "All sensors in scope (backend query)",
        note: "Results are best among eligible sensors in scope.",
      };
    }
    return {
      label: "Visible in Trends",
      note: "Results are best among the candidate sensors submitted for this run.",
    };
  }, [analysisJob.result?.params?.candidate_source]);

  const renderRelatedMatrixBlock = () => {
    if (!analysisJob.isCompleted || candidates.length === 0) return null;

    const valueModeLabel =
      correlationValueMode === "deltas" ? "bucket deltas" : "bucketed levels";
    const lagModeLabel =
      correlationLagMode === "best_within_max"
        ? `best lag within ±${Math.min(240, Math.max(0, Math.round(correlationMaxLagBuckets)))} buckets`
        : "aligned timestamps";
    const matrixHeading =
      correlationValueMode === "deltas"
        ? "Correlation (bucket deltas, not used for ranking)"
        : "Correlation (bucketed levels, not used for ranking)";
    const matrixSubtitle = `Pearson correlation on ${valueModeLabel} (${lagModeLabel}). Filtered by q ≤ 0.05 and |r| ≥ 0.2 (when enough overlap).`;
    const defaultOpen = mode === "advanced";

    if (relatedMatrixSensorIds.length < 2) {
      return (
        <CollapsibleCard
          key={`relationship-finder-correlation-${mode}`}
          title={matrixHeading}
          description={matrixSubtitle}
          defaultOpen={defaultOpen}
          density="sm"
          className="shadow-none"
        >
          <Card className="rounded-lg gap-0 border-dashed px-4 py-8 text-center">
            <p className="text-sm text-muted-foreground">
              No candidates met the matrix cutoff (<code>{matrixCutoffLabel}</code>).
            </p>
            <p className="mt-2 text-xs text-muted-foreground">
              Lower <strong>Matrix rank score cutoff</strong> in Advanced mode to include more sensors.
            </p>
          </Card>
        </CollapsibleCard>
      );
    }

    if (matrixJob.isRunning || matrixJob.isSubmitting) {
      return (
        <CollapsibleCard
          key={`relationship-finder-correlation-${mode}`}
          title={matrixHeading}
          description={matrixSubtitle}
          defaultOpen={defaultOpen}
          density="sm"
          className="shadow-none"
        >
          <LoadingState label={matrixProgressLabel} />
        </CollapsibleCard>
      );
    }

    if (matrixJob.isFailed) {
      return (
        <CollapsibleCard
          key={`relationship-finder-correlation-${mode}`}
          title={matrixHeading}
          description={matrixSubtitle}
          defaultOpen={defaultOpen}
          density="sm"
          className="shadow-none"
        >
          <ErrorState
            message={matrixJob.error ?? "Correlation matrix failed. Adjust controls and run again."}
          />
        </CollapsibleCard>
      );
    }

    if (!matrixJob.result) {
      return (
        <CollapsibleCard
          key={`relationship-finder-correlation-${mode}`}
          title={matrixHeading}
          description={matrixSubtitle}
          defaultOpen={defaultOpen}
          density="sm"
          className="shadow-none"
        >
          <Card className="rounded-lg gap-0 border-dashed px-4 py-8 text-center text-sm text-muted-foreground">
            Correlation matrix is not available yet.
          </Card>
        </CollapsibleCard>
      );
    }

    const focusIndex = focusSensorId
      ? matrixJob.result.sensor_ids.indexOf(focusSensorId)
      : -1;
    const focusRows =
      focusIndex >= 0
        ? matrixJob.result.sensor_ids
            .map((sensorId, idx) => ({
              sensorId,
              idx,
              rank: rankBySensorId.get(sensorId) ?? Number.POSITIVE_INFINITY,
              cell: matrixJob.result?.matrix?.[focusIndex]?.[idx] ?? null,
            }))
            .filter((row) => row.sensorId !== focusSensorId)
            .sort((a, b) =>
              a.rank === b.rank
                ? a.sensorId.localeCompare(b.sensorId)
                : a.rank - b.rank,
            )
        : [];

    return (
      <CollapsibleCard
        key={`relationship-finder-correlation-${mode}`}
        title={matrixHeading}
        description={matrixSubtitle}
        defaultOpen={defaultOpen}
        density="sm"
        className="shadow-none"
        data-testid="relationship-finder-correlation-block"
      >
        <div className="flex flex-wrap items-end justify-between gap-3">
          <label className="text-sm">
            <span className="text-xs font-semibold text-foreground">
              Value mode
            </span>
            <Select
              className="mt-1 min-w-[160px] shadow-sm"
              value={correlationValueMode}
              onChange={(e) =>
                setCorrelationValueMode(e.target.value as CorrelationValueModeV1)
              }
            >
              <option value="levels">Levels</option>
              <option value="deltas">Deltas (Δ)</option>
            </Select>
          </label>

          <label className="text-sm">
            <span className="text-xs font-semibold text-foreground">Lag mode</span>
            <Select
              className="mt-1 min-w-[180px] shadow-sm"
              value={correlationLagMode}
              onChange={(e) =>
                setCorrelationLagMode(e.target.value as CorrelationLagModeV1)
              }
            >
              <option value="aligned">Aligned (0 lag)</option>
              <option value="best_within_max">Best lag (bounded)</option>
            </Select>
          </label>

          {correlationLagMode === "best_within_max" ? (
            <label className="text-sm">
              <span className="text-xs font-semibold text-foreground">
                Max lag (buckets)
              </span>
              <NumericDraftInput
                value={correlationMaxLagBuckets}
                onValueChange={(value) =>
                  setCorrelationMaxLagBuckets(
                    typeof value === "number" ? value : correlationMaxLagBuckets,
                  )
                }
                min={0}
                max={240}
                integer
                clampOnBlur
                className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
              />
            </label>
          ) : null}

          <NodeButton
            type="button"
            size="xs"
            variant="secondary"
            onClick={() => setShowFullCorrelationMatrix((prev) => !prev)}
          >
            {showFullCorrelationMatrix ? "Hide full matrix" : "Show full matrix"}
          </NodeButton>
        </div>

        {focusRows.length > 0 ? (
          <div className="mt-3 space-y-2">
            {focusRows.map((row) => {
              const { sensorId, cell } = row;
              const label = shortLabel(labelMap.get(sensorId) ?? sensorId);
              const r = cell?.r != null && Number.isFinite(cell.r) ? cell.r : null;
              const lagLabel =
                cell?.lag_sec != null && Number.isFinite(cell.lag_sec) && cell.lag_sec !== 0
                  ? formatLagSeconds(cell.lag_sec)
                  : null;
              const status =
                cell?.status?.replaceAll("_", " ") ?? "not computed";

              return (
                <Card
                  key={sensorId}
                  className="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-white px-3 py-2 text-xs"
                >
                  <button
                    type="button"
                    className="min-w-0 text-left text-xs font-semibold text-foreground hover:underline"
                    onClick={() =>
                      focusSensorId ? handleMatrixPairSelect(focusSensorId, sensorId) : undefined
                    }
                  >
                    {label}
                  </button>

                  <div className="flex flex-wrap items-center justify-end gap-2 text-xs text-muted-foreground">
                    <span className="font-mono text-foreground">
                      r {r != null ? formatNumber(r, { minimumFractionDigits: 2, maximumFractionDigits: 2 }) : "—"}
                    </span>
                    {lagLabel ? (
                      <span className="font-mono" title="Best lag that maximized |r| within the configured bounds.">
                        lag {lagLabel}
                      </span>
                    ) : null}
                    <span className="capitalize">{status}</span>
                    {cell?.n_eff != null ? (
                      <span className="font-mono">
                        n_eff {formatNumber(cell.n_eff, { maximumFractionDigits: 0 })}
                      </span>
                    ) : null}
                    {cell?.q_value != null ? (
                      <span className="font-mono">
                        q {cell.q_value < 0.001 ? "<0.001" : formatNumber(cell.q_value, { maximumFractionDigits: 3 })}
                      </span>
                    ) : null}
                  </div>
                </Card>
              );
            })}
          </div>
        ) : (
          <Card className="mt-3 rounded-lg gap-0 border-dashed px-4 py-6 text-center text-sm text-muted-foreground">
            Correlation list is not available (focus sensor not in matrix).
          </Card>
        )}

        {showFullCorrelationMatrix ? (
          <div className="mt-4">
            <CorrelationMatrix
              result={matrixJob.result}
              labelMap={labelMap}
              focusSensorId={focusSensorId}
              onSelectPair={handleMatrixPairSelect}
              showHeader={false}
            />
          </div>
        ) : null}
        <p className="mt-3 text-xs text-muted-foreground">
          Candidates included: {matrixCandidateCount} (rank score cutoff {matrixCutoffLabel}, cap {RELATED_MATRIX_MAX_SENSORS}).
          Click a row (or matrix cell) to jump the preview selection.
        </p>
      </CollapsibleCard>
    );
  };

  const renderSystemWideEventsPanel = () => {
    if (!analysisJob.isCompleted || !analysisJob.result) return null;
    const buckets = analysisJob.result.system_wide_buckets ?? [];
    if (!Array.isArray(buckets) || buckets.length === 0) return null;

    const totalSensors =
      (analysisJob.result.counts?.cooccurrence_total_sensors as number | undefined) ??
      analysisJob.result.limits_used?.max_sensors_used ??
      null;

    return (
      <CollapsibleCard
        key={`relationship-finder-system-wide-${mode}`}
        title="System-wide events"
        description="Buckets where many sensors spiked together. Useful outage/debug context; not used as “related sensors” evidence."
        defaultOpen={mode === "advanced"}
        density="sm"
        className="shadow-none"
      >
        <div className="space-y-2">
          {buckets.slice(0, 24).map((bucket) => {
            const ratio =
              totalSensors && Number.isFinite(totalSensors) && totalSensors > 0
                ? bucket.group_size / totalSensors
                : null;
            const ratioLabel =
              ratio != null && Number.isFinite(ratio)
                ? `${formatNumber(ratio * 100, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}%`
                : null;

            return (
              <Card key={bucket.ts} className="flex flex-wrap items-center justify-between gap-3 rounded-lg border bg-white p-3">
                <div className="min-w-0">
                  <p className="text-sm font-semibold text-foreground">
                    {formatDateTimeForTimeZone(new Date(bucket.ts), effectiveTimeZone, {
                      month: "numeric",
                      day: "numeric",
                      hour: "numeric",
                      minute: "2-digit",
                    })}
                  </p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    Group size: {bucket.group_size}
                    {ratioLabel ? ` (${ratioLabel} of evaluated sensors)` : ""}
                    {" · "}
                    Severity: {formatNumber(bucket.severity_sum, { maximumFractionDigits: 1 })}
                  </p>
                </div>

                {onJumpToTimestamp ? (
                  <NodeButton
                    type="button"
                    size="xs"
                    variant="secondary"
                    onClick={() =>
                      handleJumpToTimestampWithUx(bucket.ts, "system_wide_buckets")
                    }
                  >
                    Jump to ±1h
                  </NodeButton>
                ) : null}
              </Card>
            );
          })}
        </div>
      </CollapsibleCard>
    );
  };

  return (
    <CollapsibleCard
      title="Related Sensors"
      defaultOpen={true}
      onOpenChange={(next) => {
        if (!next) return;
        emitRelatedSensorsUxEvent("panel_opened", {
          panel: "related_sensors",
          reason: panelOpenedRef.current ? "toggle" : "unknown",
          pathname: globalThis.location?.pathname ?? null,
        });
        panelOpenedRef.current = true;
      }}
      className="mt-6"
      description="Sensors whose change events align with the focus sensor in this time range (optionally with lag). Not causality."
      data-testid="relationship-finder-panel"
    >
      <div className="space-y-4">
        {analysisJob.error && (
          <InlineBanner tone="danger">
            <span className="font-semibold">Analysis error.</span> {analysisJob.error}
          </InlineBanner>
        )}

        {!focusSensorId && (
          <InlineBanner tone="info">
            <span className="font-semibold">Select a focus sensor.</span> Choose a sensor from the chart selection to start related-sensor analysis.
          </InlineBanner>
        )}

        {externalFocus ? (
          <InlineBanner
            tone={externalFocus.focusSensorId === focusSensorId ? "info" : "warning"}
            className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between"
          >
            <div className="space-y-1">
              <p className="text-sm font-semibold text-foreground">
                Pattern Detector → Related Sensors
              </p>
              <p className="text-xs text-muted-foreground">
                {externalFocus.focusSensorId === focusSensorId
                  ? `Using ${externalFocus.windows.length} focus window${externalFocus.windows.length === 1 ? "" : "s"} as explicit focus events (replaces delta‑z focus events for event alignment).`
                  : `Focus windows were selected for sensor ${shortLabel(labelMap.get(externalFocus.focusSensorId) ?? externalFocus.focusSensorId)}. Switch the focus sensor to use them.`}
              </p>
            </div>
            {onClearExternalFocus ? (
              <NodeButton
                size="xs"
                variant="secondary"
                type="button"
                onClick={onClearExternalFocus}
                title="Clear explicit focus events"
              >
                Clear focus
              </NodeButton>
            ) : null}
          </InlineBanner>
        ) : null}

        <div className="space-y-3">
          <div className="flex flex-wrap items-center gap-3">
            <label className="text-sm">
              <span className="text-xs font-semibold text-foreground">Focus sensor</span>
              <Select
                className="mt-1 min-w-[220px] shadow-sm"
                value={focusSensorId ?? ""}
                onChange={(e) =>
                  setShared((prev) => ({ ...prev, focusSensorId: e.target.value || null }))
                }
              >
                <option value="">— Select —</option>
                {focusSensorOptions.map((id) => (
                  <option key={id} value={id}>
                    {shortLabel(labelMap.get(id) ?? id)}
                  </option>
                ))}
              </Select>
            </label>

            <div>
              <span className="block text-xs font-semibold text-foreground">Mode</span>
              <div className="mt-1 inline-flex rounded-lg border border-border bg-white p-1 text-xs font-semibold shadow-sm">
                <button
                  type="button"
                  className={`rounded px-3 py-1.5 transition ${
                    mode === "simple" ? "bg-indigo-600 text-white" : "text-foreground hover:bg-muted"
                  }`}
                  onClick={() => setMode("simple")}
                >
                  Simple
                </button>
                <button
                  type="button"
                  className={`rounded px-3 py-1.5 transition ${
                    mode === "advanced" ? "bg-indigo-600 text-white" : "text-foreground hover:bg-muted"
                  }`}
                  onClick={() => setMode("advanced")}
                >
                  Advanced
                </button>
              </div>
            </div>
          </div>

		          <div className="flex flex-wrap items-end gap-3">
	            <label className="text-sm">
	              <span className="text-xs font-semibold text-foreground">Scope</span>
	              <Select
	                className="mt-1 shadow-sm"
	                value={shared.scope}
	                onChange={(e) =>
	                  setShared((prev) => ({
	                    ...prev,
	                    scope: e.target.value as "same_node" | "all_nodes",
	                  }))
	                }
	              >
	                <option value="same_node">Same node</option>
	                <option value="all_nodes">All nodes</option>
	              </Select>
	            </label>

              {mode === "advanced" ? (
                <label className="text-sm">
                  <span className="text-xs font-semibold text-foreground">
                    Candidate source
                  </span>
                  <Select
                    className="mt-1 min-w-[240px] shadow-sm"
                    value={shared.candidateSource}
                    onChange={(e) =>
                      setShared((prev) => ({
                        ...prev,
                        candidateSource: e.target.value as SharedControlState["candidateSource"],
                      }))
                    }
                  >
                    <option value="all_sensors_in_scope">
                      All sensors in scope (backend query)
                    </option>
                    <option value="visible_in_trends">Visible in Trends</option>
                  </Select>
                </label>
              ) : null}

		            <div className="flex flex-wrap items-center gap-2">
	              <label className="flex items-center gap-2 text-xs text-foreground">
	                <input
	                  type="checkbox"
	                  checked={shared.sameUnitOnly}
                  onChange={(e) => setShared((prev) => ({ ...prev, sameUnitOnly: e.target.checked }))}
                  className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
	                />
	                Same unit
	              </label>
			              <label className="flex items-center gap-2 text-xs text-foreground">
			                <input
			                  type="checkbox"
		                  checked={shared.sameTypeOnly}
		                  onChange={(e) => setShared((prev) => ({ ...prev, sameTypeOnly: e.target.checked }))}
		                  className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
			                />
			                Same type
			              </label>
			              {mode === "advanced" ? (
			                <label
			                  className="flex items-center gap-2 text-xs text-foreground"
			                  title="Include derived sensors that depend on the focus sensor (directly or indirectly). These are often tautological matches (not independent corroboration)."
			                >
			                  <input
			                    type="checkbox"
			                    checked={shared.includeDerivedFromFocus}
			                    onChange={(e) =>
			                      setShared((prev) => ({
			                        ...prev,
			                        includeDerivedFromFocus: e.target.checked,
			                      }))
			                    }
			                    className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
			                  />
			                  Include derived-from-focus candidates
			                </label>
			              ) : null}
			              <label
			                className="flex items-center gap-2 text-xs text-foreground"
			                title="Ignore co-occurrence buckets where many sensors spike together. These are shown separately as system-wide events."
			              >
		                <input
		                  type="checkbox"
		                  checked={shared.excludeSystemWideBuckets}
		                  onChange={(e) =>
		                    setShared((prev) => ({
		                      ...prev,
		                      excludeSystemWideBuckets: e.target.checked,
		                    }))
		                  }
		                  className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
		                />
		                Exclude system-wide buckets
		              </label>
		              {mode === "advanced" && (
		                <label className="flex items-center gap-2 text-xs text-foreground">
		                  <input
	                    type="checkbox"
	                    checked={shared.includeProviderSensors}
	                    onChange={(e) =>
	                      setShared((prev) => ({
	                        ...prev,
	                        includeProviderSensors: e.target.checked,
	                      }))
	                    }
	                    className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
	                  />
	                  Include provider/forecast sensors (may have no history)
	                </label>
	              )}

                {mode === "advanced" ? (
                  <label
                    className="flex items-center gap-2 text-xs text-foreground"
                    title="Evaluate every eligible sensor in scope (default completeness mode). Disable to use Candidate limit."
                  >
                    <input
                      type="checkbox"
                      checked={shared.evaluateAllEligible}
                      onChange={(e) =>
                        setShared((prev) => ({
                          ...prev,
                          evaluateAllEligible: e.target.checked,
                        }))
                      }
                      className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
                    />
                    Evaluate all eligible (default; may take longer)
                  </label>
                ) : null}
	            </div>

            <div className="flex items-center gap-2">
              <NodeButton
                variant="primary"
                disabled={runDisabled}
                onClick={() => void runAnalysis({ trigger: "manual" })}
              >
                {mode === "simple" ? "Find related sensors" : "Advanced (configure scoring)"}
              </NodeButton>

              {analysisJob.canCancel && (
                <NodeButton variant="secondary" onClick={() => void analysisJob.cancel()}>
                  Cancel
                </NodeButton>
              )}

              {analysisJob.status ? (
                <NodePill
                  tone={
                    analysisJob.isFailed
                      ? "danger"
                      : analysisJob.isCompleted
                        ? "success"
                        : analysisJob.isCanceled
                          ? "muted"
                          : "info"
                  }
                >
                  {analysisJob.status.replace(/_/g, " ")}
                </NodePill>
              ) : (
                <NodePill tone="muted">Not run</NodePill>
              )}

              {analysisJob.requestedAt && (
                <NodePill tone="muted" title={analysisJob.requestedAt.toISOString()}>
                  {formatDistanceToNow(analysisJob.requestedAt, { addSuffix: true })}
                </NodePill>
              )}
            </div>
          </div>

		          <div className="space-y-1 text-xs text-muted-foreground">
		            <p>
		              Rankings are relative to the sensors evaluated in this run. Scores are not probabilities and can change when scope/filters change.
		            </p>
		            {mode === "simple" ? (
		              <p>
		                Provider/forecast sensors are excluded in Simple mode because they typically have no stored history for relationship analysis.
		              </p>
		            ) : null}
		            {mode === "simple" && shared.scope === "all_nodes" ? (
		              <p>
		                Simple all-nodes runs apply stricter defaults (higher event threshold and minimum bucket group size) to reduce system-wide noise. Switch to Advanced to tune.
		              </p>
		            ) : null}
		            {excludeDerivedFromFocus ? (
		              <p>
		                {mode === "simple"
		                  ? "Derived sensors that depend on the focus sensor are excluded in Simple mode (dependency evidence is not independent). Switch to Advanced to include them."
		                  : "Derived sensors that depend on the focus sensor are excluded unless you enable “Include derived-from-focus candidates”."}
		              </p>
		            ) : null}
		            {shared.excludeSystemWideBuckets ? (
		              <p>
		                System-wide co-occurrence buckets are excluded from ranking evidence. Review them in the “System-wide events” panel.
		              </p>
		            ) : null}
		            {analysisJob.isCompleted && analysisJob.result ? (
		              <>
		                <p>
		                  Evaluated: {evaluatedCountFromResult} of {eligibleCountFromResult} eligible sensors (limit:{" "}
		                  {analysisJob.result.limits_used.candidate_limit_used}).
		                </p>
                    <p>
                      Candidate source: {candidateSourceDisclosure.label} — {candidateSourceDisclosure.note}
                    </p>
		                {analysisJob.result.counts?.pinned_requested ? (
		                  <p>
		                    Pinned included: {analysisJob.result.counts?.pinned_included ?? 0}
		                    {analysisJob.result.counts?.pinned_requested !== analysisJob.result.counts?.pinned_included
		                      ? ` (requested: ${analysisJob.result.counts?.pinned_requested ?? 0})`
		                      : ""}
		                    {analysisJob.result.counts?.pinned_truncated
		                      ? ` (truncated: ${analysisJob.result.counts?.pinned_truncated ?? 0})`
		                      : ""}
		                    .
		                  </p>
		                ) : null}
	                <p>
	                  Effective interval:{" "}
	                  {analysisJob.result.interval_seconds ?? analysisJob.result.params.interval_seconds ?? intervalSeconds} (requested:{" "}
	                  {analysisJob.result.params.interval_seconds ?? intervalSeconds}).
	                </p>
                  <p
                    title={
                      analysisJob.result.evidence_source === "pattern"
                        ? "Ranking evidence is driven by explicit focus events (Pattern Detector windows) instead of autodetected delta‑z focus events."
                        : analysisJob.result.evidence_source === "blend"
                          ? "Ranking evidence is driven by explicit focus events plus co-occurrence evidence."
                          : "Ranking evidence is driven by autodetected delta‑z focus events on the focus sensor."
                    }
                  >
                    Evidence source:{" "}
                    <span className="font-semibold text-foreground">
                      {analysisJob.result.evidence_source === "pattern"
                        ? "Pattern focus windows"
                        : analysisJob.result.evidence_source === "blend"
                          ? "Pattern + co-occurrence blend"
                          : "Delta‑Z focus events"}
                    </span>
                  </p>
                  {analysisJob.result.stability ? (
                    <p title={analysisJob.result.stability.reason ?? undefined}>
                      Stability:{" "}
                      {analysisJob.result.stability.status === "computed"
                        ? `${analysisJob.result.stability.tier ?? "unknown"} (${formatNumber(
                            analysisJob.result.stability.score ?? 0,
                            { minimumFractionDigits: 2, maximumFractionDigits: 2 },
                          )})`
                        : "skipped"}
                      {analysisJob.result.stability.reason
                        ? ` · ${analysisJob.result.stability.reason}`
                        : ""}
                    </p>
                  ) : null}
                  {mode === "advanced" && analysisJob.result.monitoring ? (
                    <p title="Evidence health monitoring: percentiles of per-sensor peak |Δz| plus gap-suppression and z-cap clipping rates.">
                      Evidence health: z-cap clipped{" "}
                      {formatNumber(analysisJob.result.monitoring.z_clipped_pct * 100, {
                        minimumFractionDigits: 0,
                        maximumFractionDigits: 0,
                      })}
                      % · gap-skipped{" "}
                      {formatNumber(analysisJob.result.monitoring.gap_skipped_pct * 100, {
                        minimumFractionDigits: 0,
                        maximumFractionDigits: 0,
                      })}
                      %
                      {analysisJob.result.monitoring.peak_abs_dz_p95 != null
                        ? ` · peak |Δz| p95 ${formatNumber(
                            analysisJob.result.monitoring.peak_abs_dz_p95,
                            { minimumFractionDigits: 1, maximumFractionDigits: 1 },
                          )}`
                        : ""}
                    </p>
                  ) : null}
                  {analysisJob.result.params?.deseason_mode === "hour_of_day_mean" ? (
                    analysisJob.result.counts?.deseasoning_applied ? (
                      <p>
                        Deseasoning applied: hour‑of‑day mean residuals (UTC).
                      </p>
                    ) : analysisJob.result.counts?.deseasoning_skipped_insufficient_window ? (
                      <p>
                        Deseasoning requested but skipped (window &lt; 2 days).
                      </p>
                    ) : null
                  ) : null}
	                {analysisJob.result.skipped_candidates.length > 0 ? (
	                  <p>
	                    Skipped: {analysisJob.result.skipped_candidates.length} provider/forecast sensors — {PROVIDER_NO_HISTORY_LABEL}
	                  </p>
	                ) : null}
	              </>
	            ) : null}
	          </div>

	          <Card className="rounded-lg bg-card-inset p-3">
	            <div className="flex flex-wrap items-start justify-between gap-2">
	              <div>
	                <p className="text-xs font-semibold text-foreground">Pinned</p>
	                <p className="mt-1 text-xs text-muted-foreground">
	                  Pinned sensors are always evaluated (unless unavailable for relationship analysis).
	                </p>
	              </div>
	              <NodePill tone={effectivePinnedSensorIds.length > 0 ? "info" : "muted"} size="sm">
	                {effectivePinnedSensorIds.length} pinned
	              </NodePill>
	            </div>

	            <div className="mt-2 flex flex-wrap items-end gap-3">
	              <label className="text-sm">
	                <span className="text-xs font-semibold text-foreground">Sensor ID</span>
	                <input
	                  data-testid="relationship-finder-pin-sensor-id"
	                  value={pinDraftSensorId}
	                  onChange={(e) => setPinDraftSensorId(e.target.value)}
	                  list="relationship-finder-pin-sensors"
	                  className="mt-1 w-[320px] max-w-full rounded border border-input bg-white px-2 py-1 text-sm shadow-sm"
	                  placeholder="Select or paste a sensor id…"
	                />
	                <datalist id="relationship-finder-pin-sensors">
	                  {pinOptionSensorIds.slice(0, 750).map((id) => (
	                    <option key={id} value={id}>
	                      {shortLabel(labelMap.get(id) ?? id)}
	                    </option>
	                  ))}
	                </datalist>
	              </label>

	              <NodeButton
	                type="button"
	                size="xs"
	                disabled={!pinDraftSensorId.trim()}
	                onClick={() => {
	                  const normalized = pinDraftSensorId.trim();
	                  if (normalized) {
	                    emitRelatedSensorsUxEvent("pin_toggled", {
	                      panel: "related_sensors",
	                      source: "pin_panel",
	                      action: "pin",
	                      focus_sensor_id: focusSensorId,
	                      sensor_id: normalized,
	                    });
	                  }
	                  pinSensorId(pinDraftSensorId);
	                  setPinDraftSensorId("");
	                }}
	              >
	                Pin
	              </NodeButton>

	              {effectivePinnedSensorIds.length > 0 ? (
	                <NodeButton
	                  type="button"
	                  size="xs"
	                  variant="ghost"
	                  onClick={() => {
	                    emitRelatedSensorsUxEvent("pin_toggled", {
	                      panel: "related_sensors",
	                      source: "pin_panel",
	                      action: "clear_all",
	                      focus_sensor_id: focusSensorId,
	                      pinned_count: effectivePinnedSensorIds.length,
	                    });
	                    clearPinnedSensorIds();
	                  }}
	                >
	                  Clear
	                </NodeButton>
	              ) : null}
	            </div>

	            {effectivePinnedSensorIds.length > 0 ? (
	              <div className="mt-3 flex flex-wrap gap-2">
	                {effectivePinnedSensorIds.map((id) => (
	                  <button
	                    key={id}
	                    type="button"
	                    onClick={() => {
	                      emitRelatedSensorsUxEvent("pin_toggled", {
	                        panel: "related_sensors",
	                        source: "pin_panel",
	                        action: "unpin",
	                        focus_sensor_id: focusSensorId,
	                        sensor_id: id,
	                      });
	                      unpinSensorId(id);
	                    }}
	                    className="inline-flex items-center rounded-full border border-border bg-white px-2.5 py-0.5 text-xs font-semibold text-muted-foreground hover:bg-muted"
	                    title="Unpin"
	                  >
	                    {shortLabel(labelMap.get(id) ?? id)} ×
	                  </button>
	                ))}
	              </div>
	            ) : (
	              <p className="mt-3 text-xs text-muted-foreground">
	                Pin a few suspects to keep them evaluated when the eligible pool is truncated.
	              </p>
	            )}
	          </Card>

	          {mode === "advanced" && (
            <Card className="grid gap-3 rounded-lg gap-0 bg-card-inset p-3 md:grid-cols-4">
              <label
                className="text-sm"
                title="Weights: how much aligned change-event evidence contributes to Rank score. Increase to emphasize time-aligned events; set to 0 to disable event alignment."
              >
                <span className="text-xs font-semibold text-foreground">Events weight</span>
                <NumericDraftInput
                  value={controls.eventsWeight}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      eventsWeight: typeof value === "number" ? value : prev.eventsWeight,
                    }))
                  }
                  min={0}
                  max={5}
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
                />
              </label>
              <label
                className="text-sm"
                title="Weights: how much shared anomaly-bucket evidence contributes to Rank score. Increase to emphasize co-occurrence; set to 0 to ignore co-occurrence buckets."
              >
                <span className="text-xs font-semibold text-foreground">Co-occurrence weight</span>
                <NumericDraftInput
                  value={controls.cooccurrenceWeight}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      cooccurrenceWeight:
                        typeof value === "number" ? value : prev.cooccurrenceWeight,
                    }))
                  }
                  min={0}
                  max={5}
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
                />
              </label>
              <label
                className="text-sm"
                title="How co-occurrence strength is computed. Avg product is more stable; Surprise highlights pairs that are stronger than expected given each sensor’s marginal event severity."
              >
                <span className="text-xs font-semibold text-foreground">Co-occ score mode</span>
                <Select
                  className="mt-1 min-w-[240px] shadow-sm"
                  value={controls.cooccurrenceScoreMode}
                  onChange={(e) =>
                    setControls((prev) => ({
                      ...prev,
                      cooccurrenceScoreMode:
                        e.target.value as UnifiedControlState["cooccurrenceScoreMode"],
                    }))
                  }
                >
                  <option value="avg_product">Avg product (recommended)</option>
                  <option value="surprise">Surprise (relative)</option>
                </Select>
              </label>
              <label
                className="text-sm"
                title="Prefer specific matches downweights system-wide buckets; prefer system-wide matches makes ‘everyone spiked’ buckets easier to surface for outage/debug workflows."
              >
                <span className="text-xs font-semibold text-foreground">Bucket preference</span>
                <Select
                  className="mt-1 min-w-[240px] shadow-sm"
                  value={controls.cooccurrenceBucketPreferenceMode}
                  onChange={(e) =>
                    setControls((prev) => ({
                      ...prev,
                      cooccurrenceBucketPreferenceMode:
                        e.target.value as UnifiedControlState["cooccurrenceBucketPreferenceMode"],
                    }))
                  }
                >
                  <option value="prefer_specific_matches">Prefer specific matches</option>
                  <option value="prefer_system_wide_matches">
                    Prefer system-wide matches (outage/debug)
                  </option>
                </Select>
              </label>
              <label
                className="text-sm"
                title="Weights: optional delta-correlation context channel. Enable “Include Δ corr signal” to use it; not required for good results."
              >
                <span className="text-xs font-semibold text-foreground">Δ corr weight</span>
                <NumericDraftInput
                  value={controls.deltaCorrWeight}
                  disabled={!controls.includeDeltaCorrSignal}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      deltaCorrWeight:
                        typeof value === "number" ? value : prev.deltaCorrWeight,
                    }))
                  }
                  min={0}
                  max={5}
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm disabled:opacity-50"
                />
              </label>
              <label
                className="text-sm"
                title="Candidate limit: max number of sensors evaluated for ranking. Higher = more coverage but slower. Rank score is pool-relative, so changing the limit/scope can change ranks."
              >
                <span className="text-xs font-semibold text-foreground">Candidate limit</span>
                <NumericDraftInput
                  value={controls.candidateLimit}
                  disabled={shared.evaluateAllEligible}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      candidateLimit: typeof value === "number" ? value : prev.candidateLimit,
                    }))
                  }
                  min={20}
                  max={1000}
                  integer
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
                />
              </label>
              <label className="text-sm">
                <span className="text-xs font-semibold text-foreground">Max results</span>
                <NumericDraftInput
                  value={controls.maxResults}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      maxResults: typeof value === "number" ? value : prev.maxResults,
                    }))
                  }
                  min={5}
                  max={300}
                  integer
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
                />
              </label>
              <label className="text-sm">
                <span className="text-xs font-semibold text-foreground">Matrix rank score cutoff</span>
                <NumericDraftInput
                  value={controls.matrixScoreCutoff}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      matrixScoreCutoff:
                        typeof value === "number" ? value : prev.matrixScoreCutoff,
                    }))
                  }
                  min={0}
                  max={1}
                  step={0.05}
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
                />
              </label>
              <label className="text-sm">
                <span className="text-xs font-semibold text-foreground">Deseasoning</span>
                <Select
                  className="mt-1 min-w-[180px] shadow-sm"
                  value={controls.deseasonMode}
                  onChange={(e) =>
                    setControls((prev) => ({
                      ...prev,
                      deseasonMode: e.target.value as UnifiedControlState["deseasonMode"],
                    }))
                  }
                >
                  <option value="none">None</option>
                  <option value="hour_of_day_mean">Hour-of-day mean (UTC)</option>
                </Select>
              </label>

              <label
                className="col-span-full flex items-center gap-2 text-xs text-foreground"
                title="Mitigate diurnal/periodic artifacts by downweighting sensors whose events cluster at fixed times-of-day."
              >
                <input
                  type="checkbox"
                  checked={controls.periodicPenaltyEnabled}
                  onChange={(e) =>
                    setControls((prev) => ({
                      ...prev,
                      periodicPenaltyEnabled: e.target.checked,
                    }))
                  }
                  className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
                />
                Periodic penalty (low-entropy downweight)
              </label>

              <label
                className="col-span-full flex items-center gap-2 text-xs text-foreground"
                title="Signed correlation on bucket deltas at best lag. Not statistical significance. Not used for ranking unless enabled."
              >
                <input
                  type="checkbox"
                  checked={controls.includeDeltaCorrSignal}
                  onChange={(e) =>
                    setControls((prev) => ({
                      ...prev,
                      includeDeltaCorrSignal: e.target.checked,
                    }))
                  }
                  className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
                />
                Include Δ corr signal (ranking)
              </label>

              <p className="col-span-full text-xs text-muted-foreground">
                Mitigate diurnal/periodic artifacts (may reduce true positives for truly periodic mechanisms).
              </p>

              <label className="text-sm">
                <span className="text-xs font-semibold text-foreground">Polarity</span>
                <Select
                  className="mt-1"
                  value={controls.polarity}
                  onChange={(e) =>
                    setControls((prev) => ({
                      ...prev,
                      polarity: e.target.value as "both" | "up" | "down",
                    }))
                  }
                >
                  <option value="both">Both</option>
                  <option value="up">Up only</option>
                  <option value="down">Down only</option>
                </Select>
              </label>
	              <label
                  className="text-sm"
                  title="z threshold: minimum |Δz| for a bucket-to-bucket change to count as an event. Higher = fewer events (less noise, fewer false positives). Lower = more events (more sensitivity, more spurious matches)."
                >
	                <span className="text-xs font-semibold text-foreground">z threshold</span>
	                <NumericDraftInput
	                  value={controls.zThreshold}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      zThreshold: typeof value === "number" ? value : prev.zThreshold,
                    }))
                  }
                  min={0.5}
                  max={10}
	                  clampOnBlur
	                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
	                />
	              </label>
	              <label className="text-sm">
	                <span className="text-xs font-semibold text-foreground">z cap (scoring)</span>
	                <NumericDraftInput
	                  value={controls.zCap}
	                  onValueChange={(value) =>
	                    setControls((prev) => ({
	                      ...prev,
	                      zCap: typeof value === "number" ? value : prev.zCap,
	                    }))
	                  }
	                  min={1}
	                  max={1000}
	                  integer
	                  clampOnBlur
	                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
	                />
	              </label>
	              <label className="text-sm">
	                <span className="text-xs font-semibold text-foreground">Gap max (buckets)</span>
	                <NumericDraftInput
	                  value={controls.gapMaxBuckets}
	                  onValueChange={(value) =>
	                    setControls((prev) => ({
	                      ...prev,
	                      gapMaxBuckets:
	                        typeof value === "number" ? value : prev.gapMaxBuckets,
	                    }))
	                  }
	                  min={0}
	                  max={100}
	                  integer
	                  clampOnBlur
	                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
	                />
	              </label>
	              <label
                  className="text-sm"
                  title="Max lag (buckets): how far candidates are allowed to lead/lag the focus sensor when matching events. Larger values can surface delayed mechanisms, but can introduce accidental matches."
                >
	                <span className="text-xs font-semibold text-foreground">Max lag (buckets)</span>
	                <NumericDraftInput
	                  value={controls.maxLagBuckets}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      maxLagBuckets: typeof value === "number" ? value : prev.maxLagBuckets,
                    }))
                  }
                  min={0}
                  max={120}
                  integer
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
                />
              </label>
              <label
                className="text-sm"
                title="Tolerance (buckets): how close two events must be (after applying lag) to be counted as the same moment. Larger tolerance increases overlap but can inflate evidence for loosely related sensors."
              >
                <span className="text-xs font-semibold text-foreground">Tolerance (buckets)</span>
                <NumericDraftInput
                  value={controls.toleranceBuckets}
                  onValueChange={(value) =>
                    setControls((prev) => ({
                      ...prev,
                      toleranceBuckets:
                        typeof value === "number" ? value : prev.toleranceBuckets,
                    }))
                  }
                  min={0}
                  max={20}
                  integer
                  clampOnBlur
                  className="mt-1 w-24 rounded border border-input bg-white px-2 py-1 text-sm"
                />
              </label>

              <label className="col-span-full mt-1 flex items-center gap-2 text-xs text-foreground">
                <input
                  type="checkbox"
                  checked={controls.includeLowConfidence}
                  onChange={(e) =>
                    setControls((prev) => ({ ...prev, includeLowConfidence: e.target.checked }))
                  }
                  className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
                />
                Include weak evidence
              </label>

              <label
                className="col-span-full mt-1 flex items-center gap-2 text-xs text-foreground"
                title={
                  stabilityEligibleEstimate
                    ? "Reruns the ranker on 3 subwindows and reports overlap@10 stability. Only runs when the eligible pool is small to avoid performance regressions."
                    : `Only available when the eligible pool is ≤ 120 (currently ~${eligibleCountEstimate}).`
                }
              >
                <input
                  type="checkbox"
                  checked={controls.stabilityEnabled && stabilityEligibleEstimate}
                  disabled={!stabilityEligibleEstimate}
                  onChange={(e) =>
                    setControls((prev) => ({
                      ...prev,
                      stabilityEnabled: e.target.checked,
                    }))
                  }
                  className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500 disabled:opacity-50"
                />
                Compute rank stability (overlap@10)
              </label>

              <p className="col-span-full text-xs text-muted-foreground">
                Stability is a trust indicator (rank robustness), not a correctness guarantee.
              </p>

	              <p className="col-span-full text-xs text-muted-foreground">
	                Matrix cutoff controls which related sensors are shown in the correlation matrix only; ranking stays unchanged.
	              </p>
	            </Card>
	          )}

	          {analysisJob.isCompleted && analysisJob.result && lastRunSnapshot ? (
	            <Card className="rounded-lg bg-card-inset p-3">
	              <div className="flex flex-wrap items-center justify-between gap-2">
	                <p className="text-xs font-semibold text-foreground">Why not evaluated?</p>
	                <p className="text-xs text-muted-foreground">
	                  Explain why an expected eligible sensor did not appear in results.
	                </p>
	              </div>

	              <div className="mt-2 flex flex-wrap items-end gap-3">
	                <label className="text-sm">
	                  <span className="text-xs font-semibold text-foreground">Sensor ID</span>
	                  <input
	                    data-testid="relationship-finder-diagnostic-sensor-id"
	                    value={diagnosticSensorId}
	                    onChange={(e) => setDiagnosticSensorId(e.target.value)}
	                    list="relationship-finder-eligible-sensors"
	                    className="mt-1 w-[320px] max-w-full rounded border border-input bg-white px-2 py-1 text-sm shadow-sm"
	                    placeholder="Select or paste a sensor id…"
	                  />
	                  <datalist id="relationship-finder-eligible-sensors">
	                    {lastRunSnapshot.eligibleSensorIds.slice(0, 500).map((id) => (
	                      <option key={id} value={id}>
	                        {shortLabel(labelMap.get(id) ?? id)}
	                      </option>
	                    ))}
	                  </datalist>
	                </label>

	                <p className="text-xs text-muted-foreground">
	                  Eligible in this run: {eligibleCountFromResult}
                    {lastRunSnapshot.eligibleSensorIds.length !== eligibleCountFromResult
                      ? ` (snapshot: ${lastRunSnapshot.eligibleSensorIds.length})`
                      : ""}
	                </p>
	              </div>

	              {diagnosticSensorId.trim() ? (
	                <p
	                  data-testid="relationship-finder-diagnostic-result"
	                  className="mt-2 text-xs text-muted-foreground"
	                >
	                  {diagnoseUnifiedCandidateAbsence({
	                    focusSensorId,
	                    sensorId: diagnosticSensorId,
	                    eligibleSensorIds: lastRunSnapshot.eligibleSensorIds,
	                    result: analysisJob.result,
	                  })}
	                </p>
	              ) : (
	                <p className="mt-2 text-xs text-muted-foreground">
	                  Tip: choose a sensor from the eligible list to see whether it was filtered, truncated by the candidate limit, evaluated but below threshold, or unavailable.
	                </p>
	              )}
	            </Card>
	          ) : null}
	        </div>

        {analysisJob.isRunning && <LoadingState label={progressLabel} />}

        {analysisJob.isFailed && (
          <ErrorState
            message={analysisJob.error ?? "Analysis failed. Adjust settings and try again."}
          />
        )}

        {analysisJob.isCanceled && (
          <InlineBanner tone="info">
            <span className="font-semibold">Canceled.</span> Adjust settings and run again.
          </InlineBanner>
        )}

        {mode === "simple" ? renderRelatedMatrixBlock() : null}

	        {analysisJob.isCompleted && candidates.length > 0 && (
	          <div
	            className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_420px]"
	            data-testid="relationship-finder-results"
	          >
	            <ResultsList
	              candidates={candidates}
	              selectedCandidateId={selectedCandidateId}
	              onSelectCandidate={(sensorId) => handleCandidateOpened(sensorId, "results_list")}
                pinnedSensorIds={effectivePinnedSensorIds}
                onTogglePinnedSensorId={handleTogglePinnedFromResults}
	              sensorsById={sensorsById}
	              nodesById={nodesById}
	              badgeById={badgeById}
	              selectedSensorIds={selectedSensorIds}
	              maxSeries={maxSeries}
	              onAddToChart={handleAddToChartFromResults}
	              weights={{
                  events: controls.eventsWeight,
                  cooccurrence: controls.cooccurrenceWeight,
                  deltaCorr:
                    mode === "advanced" && controls.includeDeltaCorrSignal
                      ? controls.deltaCorrWeight
                      : 0,
                }}
	              maxVisible={visibleCount}
	              hasMore={candidates.length > visibleCount}
	              onShowMore={() => setVisibleCount((prev) => prev + 25)}
	            />

            <PreviewPane
              focusSensorId={focusSensorId}
              focusLabel={focusLabel}
              candidate={selectedCandidate}
              sensorsById={sensorsById}
              labelMap={labelMap}
              selectedSensorIds={selectedSensorIds}
              maxSeries={maxSeries}
              onAddToChart={handleAddToChartFromPreview}
              timeZone={effectiveTimeZone}
              computedThroughTs={computedThroughTs}
              strategy="unified"
	              relationshipMode={mode}
	              series={series}
	              intervalSeconds={intervalSeconds}
	              effectiveIntervalSeconds={analysisJob.result?.interval_seconds ?? null}
	              analysisBucketCount={analysisJob.result?.bucket_count ?? null}
	              onJumpToTimestamp={
	                onJumpToTimestamp
	                  ? (timestampMs) =>
	                      handleJumpToTimestampWithUx(timestampMs, "preview")
	                  : undefined
	              }
	            />
	          </div>
	        )}

	        {renderSystemWideEventsPanel()}

	        {mode === "advanced" ? renderRelatedMatrixBlock() : null}

	        {analysisJob.isCompleted && candidates.length === 0 && (
	          <Card className="rounded-lg gap-0 border-dashed px-4 py-8 text-center">
	            <p className="text-sm text-muted-foreground">
	              No candidates exceeded the evidence threshold in this time range. Evaluated{" "}
	              {evaluatedCountFromResult} of {eligibleCountFromResult} eligible sensors. Expand the time range, lower the event threshold, or include weak evidence in Advanced.
	            </p>
	          </Card>
	        )}

	        <AnalysisKey
	          summary="Triage checklist"
	          overview="A fast checklist when a candidate looks suspicious or noisy."
	          defaultOpen={false}
	        >
	          <ul className="list-disc space-y-1 pl-4 text-xs text-muted-foreground">
	            <li>
	              Toggle <strong>Raw</strong> vs <strong>Normalized</strong> in the preview to confirm you’re not being misled by units or scale.
	            </li>
	            <li>
	              Confirm the <strong>unit</strong> and <strong>sensor type</strong> match your mental model (use Same unit / Same type filters to tighten scope).
	            </li>
	            <li>
	              Watch for <strong>missingness/gaps</strong>: sparse or gappy series can create misleading “matches” (gap suppression helps, but evidence can still be weak).
	            </li>
	            <li>
	              Use <strong>Direction</strong> (same vs opposite) to sanity-check whether the relationship makes mechanical sense.
	            </li>
	            <li>
	              Treat <strong>system-wide buckets</strong> as outage/debug context; don’t confuse “everyone spiked” with a causal/paired relationship.
	            </li>
	          </ul>
	        </AnalysisKey>

	        <AnalysisKey
	          summary="How it works"
	          overview={
	            mode === "simple"
              ? "Simple mode suggests related sensors using aligned change events and shared bucket evidence."
              : "Advanced mode exposes scoring controls while preserving explainable rank score + evidence tiers."
          }
        >
          <div className="space-y-1 text-xs text-muted-foreground">
            <p>
              <strong>Related:</strong> time-aligned change evidence on bucketed series within the selected window/interval (optionally with lag).
            </p>
            <p>
              <strong>Not:</strong> causality, probability, statistical significance, exhaustive search, or correlation-of-levels (as a rank driver).
            </p>
            <p>
              <strong>Rank score:</strong> 0–1 rank score relative to the evaluated candidates in this run (not comparable across runs/scopes).
            </p>
            <p>
              <strong>Evidence:</strong> strong/medium/weak heuristic coverage tier (not a probability).
            </p>
            <p>
              <strong>Correlation:</strong> correlation (levels or deltas; optional bounded lag search) is shown for context and is not used for ranking.
            </p>
            <p className="pt-1">
              <a
                className="font-semibold text-indigo-600 underline hover:text-indigo-500"
                href="/analytics/trends/related-sensors-how-it-works"
              >
                Read the operator guide
              </a>
            </p>
          </div>
        </AnalysisKey>
      </div>
    </CollapsibleCard>
  );
}
