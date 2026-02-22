"use client";

import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import type { TsseCandidateFiltersV1, RelatedSensorsUnifiedJobParamsV2, RelatedSensorsUnifiedResultV2 } from "@/types/analysis";
import type { DemoAlarmEvent, DemoNode, DemoSensor, DemoUser, TrendSeriesEntry, TrendSeriesPoint } from "@/types/dashboard";
import { useAlarmsQuery, useTrendPreviewQuery } from "@/lib/queries";
import { fetchActionLogs, fetchIncidentDetail, fetchIncidentNotes, assignIncident, closeIncident, createIncidentNote, snoozeIncident } from "@/lib/api";
import { postJson } from "@/lib/api";
import InlineBanner from "@/components/InlineBanner";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { Sheet, SheetBody, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import NodeButton from "@/features/nodes/components/NodeButton";
import { TrendChart } from "@/components/TrendChart";
import { useAnalysisJob } from "@/features/trends/hooks/useAnalysisJob";
import { createLookupMaps, normalizeUnifiedCandidates } from "@/features/trends/utils/candidateNormalizers";
import ResultsList from "@/features/trends/components/relationshipFinder/ResultsList";
import PreviewPane from "@/features/trends/components/relationshipFinder/PreviewPane";
import type { Incident } from "@/types/incidents";

type ContextPreset = "auto" | "1h" | "6h" | "24h";
type RelatedSort = "combined" | "significance" | "proximity";

const severityTone = (severity: string): "danger" | "warning" | "info" => {
  if (severity === "critical") return "danger";
  if (severity === "warning") return "warning";
  return "info";
};

const statusTone = (status: string): "danger" | "warning" | "muted" => {
  if (status === "snoozed") return "warning";
  if (status === "closed") return "muted";
  return "danger";
};

function findUser(users: DemoUser[], userId: string | null): DemoUser | null {
  if (!userId) return null;
  return users.find((u) => u.id === userId) ?? null;
}

function formatDateTime(value: Date): string {
  return new Intl.DateTimeFormat(undefined, {
    year: "numeric",
    month: "short",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(value);
}

function buildConstantSeries(
  base: TrendSeriesEntry,
  value: number,
  label: string,
): TrendSeriesEntry {
  const points: TrendSeriesPoint[] = base.points.map((pt) => ({
    timestamp: pt.timestamp,
    value: pt.value == null ? null : value,
    samples: 0,
  }));
  return {
    sensor_id: `ref:${label}`,
    label,
    unit: base.unit,
    display_decimals: base.display_decimals,
    points,
  };
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function numeric(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function text(value: unknown): string | null {
  return typeof value === "string" ? value : value == null ? null : String(value);
}

function extractThresholdMeta(rule: Record<string, unknown> | null): {
  operator: string | null;
  threshold: number | null;
  min: number | null;
  max: number | null;
} {
  const container = rule ?? {};
  const nestedCondition = asRecord(container.condition) ?? asRecord(container.params) ?? null;
  const candidates = [container, ...(nestedCondition ? [nestedCondition] : [])];

  let operator: string | null = null;
  let threshold: number | null = null;
  let min: number | null = null;
  let max: number | null = null;

  for (const obj of candidates) {
    operator =
      operator ??
      text(obj.operator) ??
      text(obj.op) ??
      text(obj.comparator) ??
      text(obj.cmp) ??
      null;

    threshold =
      threshold ??
      numeric(obj.threshold) ??
      numeric(obj.value) ??
      numeric(obj.limit) ??
      numeric(obj.target) ??
      null;

    min =
      min ??
      numeric(obj.min) ??
      numeric(obj.lower) ??
      numeric(obj.low) ??
      numeric(obj.min_value) ??
      null;

    max =
      max ??
      numeric(obj.max) ??
      numeric(obj.upper) ??
      numeric(obj.high) ??
      numeric(obj.max_value) ??
      null;
  }

  return { operator, threshold, min, max };
}

function computeAutoContextWindowSeconds(sensor: DemoSensor | null): number {
  const interval = sensor?.interval_seconds && sensor.interval_seconds > 0 ? sensor.interval_seconds : 60;
  const window = interval * 600;
  return Math.min(24 * 3600, Math.max(3600, window));
}

function contextWindowForPreset(preset: ContextPreset, focusTime: Date, sensor: DemoSensor | null): { start: Date; end: Date } {
  if (preset === "1h") {
    return { start: new Date(focusTime.getTime() - 60 * 60 * 1000), end: new Date(focusTime.getTime() + 60 * 60 * 1000) };
  }
  if (preset === "6h") {
    return { start: new Date(focusTime.getTime() - 6 * 60 * 60 * 1000), end: new Date(focusTime.getTime() + 6 * 60 * 60 * 1000) };
  }
  if (preset === "24h") {
    return { start: new Date(focusTime.getTime() - 24 * 60 * 60 * 1000), end: new Date(focusTime.getTime() + 24 * 60 * 60 * 1000) };
  }
  const windowSeconds = computeAutoContextWindowSeconds(sensor);
  const halfMs = Math.round((windowSeconds * 1000) / 2);
  return { start: new Date(focusTime.getTime() - halfMs), end: new Date(focusTime.getTime() + halfMs) };
}

function intervalSecondsForContext(preset: ContextPreset, sensor: DemoSensor | null): number {
  if (preset === "1h") return 30;
  if (preset === "6h") return 60;
  if (preset === "24h") return 300;
  const interval = sensor?.interval_seconds && sensor.interval_seconds > 0 ? sensor.interval_seconds : 60;
  return Math.min(1800, Math.max(30, interval));
}

function buildRelatedIntervalSeconds(windowStart: Date, windowEnd: Date): number {
  const horizonSeconds = Math.max(1, Math.round((windowEnd.getTime() - windowStart.getTime()) / 1000));
  const approx = Math.ceil(horizonSeconds / 5000);
  return Math.min(3600, Math.max(60, approx));
}

function jobKeyForRelatedSensors(
  focusSensorId: string,
  startIso: string,
  endIso: string,
  intervalSeconds: number,
  filters: TsseCandidateFiltersV1,
): string {
  return JSON.stringify({
    v: 1,
    focusSensorId,
    startIso,
    endIso,
    intervalSeconds,
    filters,
  });
}

export default function IncidentDetailSheet({
  open,
  onOpenChange,
  incidentId,
  canEdit,
  canAck,
  meUserId,
  sensors,
  nodes,
  users,
}: {
  open: boolean;
  onOpenChange: (next: boolean) => void;
  incidentId: string | null;
  canEdit: boolean;
  canAck: boolean;
  meUserId: string | null;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  users: DemoUser[];
}) {
  const queryClient = useQueryClient();
  const alarmsQuery = useAlarmsQuery();

  const sensorsById = useMemo(() => new Map(sensors.map((s) => [s.sensor_id, s])), [sensors]);
  const nodesById = useMemo(() => new Map(nodes.map((n) => [n.id, n])), [nodes]);
  const labelMap = useMemo(() => {
    return new Map(
      sensors.map((sensor) => {
        const nodeName = nodesById.get(sensor.node_id)?.name ?? "Unknown node";
        const unit = sensor.unit ? ` (${sensor.unit})` : "";
        return [sensor.sensor_id, `${nodeName} — ${sensor.name}${unit}`];
      }),
    );
  }, [nodesById, sensors]);

  const lookups = useMemo(() => createLookupMaps(sensors, nodesById, labelMap), [labelMap, nodesById, sensors]);
  const badgeById = useMemo(() => new Map(), []);

  const detailQuery = useQuery({
    queryKey: ["incidents", incidentId ?? "missing"],
    queryFn: () => fetchIncidentDetail(incidentId as string),
    enabled: open && Boolean(incidentId),
    staleTime: 10_000,
  });

  const incident: Incident | null = detailQuery.data?.incident ?? null;
  const events: DemoAlarmEvent[] = useMemo(() => detailQuery.data?.events ?? [], [detailQuery.data?.events]);

  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  useEffect(() => {
    if (!open) return;
    if (selectedEventId) return;
    const first = events[0]?.id ?? null;
    if (first) setSelectedEventId(first);
  }, [events, open, selectedEventId]);

  const selectedEvent: DemoAlarmEvent | null = useMemo(() => {
    if (!selectedEventId) return null;
    return events.find((e) => e.id === selectedEventId) ?? null;
  }, [events, selectedEventId]);

  const focusTime = useMemo(() => {
    if (selectedEvent?.created_at instanceof Date) return selectedEvent.created_at;
    if (selectedEvent?.created_at) return new Date(selectedEvent.created_at);
    return incident?.last_event_at ?? null;
  }, [incident?.last_event_at, selectedEvent?.created_at]);

  const focusSensorId = selectedEvent?.sensor_id ?? incident?.last_sensor_id ?? null;
  const focusSensor = focusSensorId ? sensorsById.get(focusSensorId) ?? null : null;

  const [contextPreset, setContextPreset] = useState<ContextPreset>("auto");
  const contextWindow = useMemo(() => {
    if (!focusTime) {
      const now = new Date();
      return { start: new Date(now.getTime() - 60 * 60 * 1000), end: now };
    }
    return contextWindowForPreset(contextPreset, focusTime, focusSensor);
  }, [contextPreset, focusSensor, focusTime]);

  const chartIntervalSeconds = useMemo(
    () => intervalSecondsForContext(contextPreset, focusSensor),
    [contextPreset, focusSensor],
  );

  const trendQuery = useTrendPreviewQuery({
    sensorId: focusSensorId ?? "",
    start: contextWindow.start.toISOString(),
    end: contextWindow.end.toISOString(),
    interval: chartIntervalSeconds,
    enabled: Boolean(open && focusSensorId),
  });

  const series: TrendSeriesEntry | null = useMemo(() => {
    const first = trendQuery.data?.[0] ?? null;
    if (!first) return null;
    const label = focusSensorId ? labelMap.get(focusSensorId) ?? first.label ?? focusSensorId : first.label;
    return {
      ...first,
      label,
      unit: focusSensor?.unit ?? first.unit,
    };
  }, [focusSensor?.unit, focusSensorId, labelMap, trendQuery.data]);

  const overlaySeries: TrendSeriesEntry[] = useMemo(() => {
    if (!series) return [];
    const alarms = alarmsQuery.data ?? [];
    const alarmForEvent =
      selectedEvent?.alarm_id
        ? (alarms.find((a) => String(a.id) === String(selectedEvent.alarm_id)) ?? null)
        : null;
    const rule = (alarmForEvent?.rule as Record<string, unknown>) ?? null;
    const meta = extractThresholdMeta(rule);

    const out: TrendSeriesEntry[] = [];
    if (meta.threshold != null && Number.isFinite(meta.threshold)) {
      out.push(buildConstantSeries(series, meta.threshold, `Threshold (${meta.operator ?? "op"})`));
    }
    if (meta.min != null && Number.isFinite(meta.min)) {
      out.push(buildConstantSeries(series, meta.min, "Range min"));
    }
    if (meta.max != null && Number.isFinite(meta.max)) {
      out.push(buildConstantSeries(series, meta.max, "Range max"));
    }
    return out;
  }, [alarmsQuery.data, selectedEvent?.alarm_id, series]);

  const relatedIntervalSeconds = useMemo(
    () => buildRelatedIntervalSeconds(contextWindow.start, contextWindow.end),
    [contextWindow.end, contextWindow.start],
  );

  const [relatedFilters, setRelatedFilters] = useState<TsseCandidateFiltersV1>({
    same_node_only: false,
    same_unit_only: false,
    same_type_only: false,
    is_derived: null,
    is_public_provider: null,
    exclude_sensor_ids: [],
  });

  const relatedJob = useAnalysisJob<RelatedSensorsUnifiedResultV2>();
  const [relatedSort, setRelatedSort] = useState<RelatedSort>("combined");

  const runRelated = async () => {
    if (!focusSensorId) return;
    const startIso = contextWindow.start.toISOString();
    const endIso = contextWindow.end.toISOString();
    const params: RelatedSensorsUnifiedJobParamsV2 = {
      focus_sensor_id: focusSensorId,
      start: startIso,
      end: endIso,
      interval_seconds: relatedIntervalSeconds,
      mode: "simple",
      candidate_source: "all_sensors_in_scope",
      quick_suggest: true,
      include_low_confidence: false,
      max_results: 25,
      filters: relatedFilters,
    };
    const jobKey = jobKeyForRelatedSensors(focusSensorId, startIso, endIso, relatedIntervalSeconds, relatedFilters);
    await relatedJob.run("related_sensors_unified_v2", params, jobKey);
  };

  useEffect(() => {
    if (!open) return;
    if (!focusSensorId) return;
    if (relatedJob.isRunning || relatedJob.isSubmitting) return;
    if (relatedJob.result) return;
    void runRelated().catch(() => {
      // handled by hook error state
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, focusSensorId]);

  const relatedCandidates = useMemo(() => {
    const result = relatedJob.result as RelatedSensorsUnifiedResultV2 | null;
    if (!result) return [];
    const candidates = normalizeUnifiedCandidates(result, lookups);

    const focusMs = focusTime ? focusTime.getTime() : null;
    const confidenceWeight = (candidate: (typeof candidates)[number]): number => {
      if (candidate.raw.type !== "unified") return 2;
      const tier = String(candidate.raw.data.confidence_tier ?? "").trim().toLowerCase();
      if (tier === "high") return 0;
      if (tier === "medium") return 1;
      return 2;
    };
    const proximitySeconds = (candidate: (typeof candidates)[number]): number => {
      if (focusMs == null) return Number.POSITIVE_INFINITY;
      if (candidate.raw.type !== "unified") return Number.POSITIVE_INFINITY;
      const timestamps = candidate.raw.data.top_bucket_timestamps;
      if (!timestamps || !Array.isArray(timestamps) || !timestamps.length) return Number.POSITIVE_INFINITY;
      let best = Number.POSITIVE_INFINITY;
      for (const ts of timestamps) {
        if (typeof ts !== "number" || !Number.isFinite(ts)) continue;
        const diff = Math.abs(ts - focusMs) / 1000;
        if (diff < best) best = diff;
      }
      return best;
    };

    const compareAsc = (a: number, b: number): number => {
      if (a === b) return 0;
      const aFinite = Number.isFinite(a);
      const bFinite = Number.isFinite(b);
      if (!aFinite && !bFinite) return 0;
      if (!aFinite) return 1;
      if (!bFinite) return -1;
      return a < b ? -1 : 1;
    };

    const compareDesc = (a: number, b: number): number => compareAsc(b, a);

    const sorted = candidates.slice().sort((a, b) => {
      const conf = compareAsc(confidenceWeight(a), confidenceWeight(b));
      const score = compareDesc(a.score, b.score);
      const prox = compareAsc(proximitySeconds(a), proximitySeconds(b));

      if (relatedSort === "significance") {
        if (conf !== 0) return conf;
        if (score !== 0) return score;
        return a.sensor_id.localeCompare(b.sensor_id);
      }

      if (relatedSort === "proximity") {
        if (prox !== 0) return prox;
        if (conf !== 0) return conf;
        if (score !== 0) return score;
        return a.sensor_id.localeCompare(b.sensor_id);
      }

      if (conf !== 0) return conf;
      if (score !== 0) return score;
      if (prox !== 0) return prox;
      return a.sensor_id.localeCompare(b.sensor_id);
    });

    return sorted.map((candidate, idx) => ({ ...candidate, rank: idx + 1 }));
  }, [focusTime, lookups, relatedJob.result, relatedSort]);

  const [selectedCandidateId, setSelectedCandidateId] = useState<string | null>(null);
  useEffect(() => {
    if (!relatedCandidates.length) return;
    if (selectedCandidateId) return;
    setSelectedCandidateId(relatedCandidates[0]!.sensor_id);
  }, [relatedCandidates, selectedCandidateId]);

  const selectedCandidate = useMemo(() => {
    if (!selectedCandidateId) return null;
    return relatedCandidates.find((c) => c.sensor_id === selectedCandidateId) ?? null;
  }, [relatedCandidates, selectedCandidateId]);

  const notesQuery = useQuery({
    queryKey: ["incidents", incidentId ?? "missing", "notes"],
    queryFn: () => fetchIncidentNotes(incidentId as string, { limit: 50 }),
    enabled: open && Boolean(incidentId),
    staleTime: 10_000,
  });

  const [noteDraft, setNoteDraft] = useState("");
  const [noteBusy, setNoteBusy] = useState(false);
  const [noteError, setNoteError] = useState<string | null>(null);

  const actionLogsQuery = useQuery({
    queryKey: [
      "incidents",
      incidentId ?? "missing",
      "action-logs",
      contextWindow.start.toISOString(),
      contextWindow.end.toISOString(),
      incident?.last_node_id ?? "any",
    ],
    queryFn: () =>
      fetchActionLogs({
        from: contextWindow.start.toISOString(),
        to: contextWindow.end.toISOString(),
        node_id: incident?.last_node_id ?? undefined,
        limit: 100,
      }),
    enabled: open && Boolean(incidentId) && Boolean(incident),
    staleTime: 10_000,
  });

  const [actionLogSearchDraft, setActionLogSearchDraft] = useState("");
  const [actionLogSearch, setActionLogSearch] = useState("");
  const [actionLogScheduleFilter, setActionLogScheduleFilter] = useState<string>("any");
  const [actionLogStatusFilter, setActionLogStatusFilter] = useState<string>("any");

  useEffect(() => {
    const handle = setTimeout(() => setActionLogSearch(actionLogSearchDraft.trim().toLowerCase()), 250);
    return () => clearTimeout(handle);
  }, [actionLogSearchDraft]);

  const actionLogs = useMemo(() => actionLogsQuery.data ?? [], [actionLogsQuery.data]);
  const actionLogScheduleOptions = useMemo(() => {
    const out = new Set<string>();
    for (const log of actionLogs) {
      if (log.schedule_id) out.add(log.schedule_id);
    }
    return Array.from(out).sort();
  }, [actionLogs]);

  const actionLogStatusOptions = useMemo(() => {
    const out = new Set<string>();
    for (const log of actionLogs) {
      if (log.status) out.add(log.status);
    }
    return Array.from(out).sort();
  }, [actionLogs]);

  const filteredActionLogs = useMemo(() => {
    let next = actionLogs.slice();

    if (actionLogScheduleFilter !== "any") {
      next = next.filter((log) => log.schedule_id === actionLogScheduleFilter);
    }

    if (actionLogStatusFilter !== "any") {
      next = next.filter((log) => log.status === actionLogStatusFilter);
    }

    if (actionLogSearch) {
      next = next.filter((log) => {
        const haystack = [
          log.status,
          log.schedule_id,
          log.node_id ?? "",
          log.output_id ?? "",
          log.message ?? "",
        ]
          .join(" ")
          .toLowerCase();
        return haystack.includes(actionLogSearch);
      });
    }

    return next;
  }, [actionLogScheduleFilter, actionLogSearch, actionLogStatusFilter, actionLogs]);

  const [mutateError, setMutateError] = useState<string | null>(null);
  const [mutateBusy, setMutateBusy] = useState(false);

  const updateIncidentInCache = (next: Incident) => {
    queryClient.setQueryData(["incidents", incidentId ?? "missing"], (prev: unknown) => {
      if (!prev || typeof prev !== "object") return prev;
      const record = prev as { incident?: Incident; events?: DemoAlarmEvent[] };
      return { ...record, incident: next };
    });
    void queryClient.invalidateQueries({ queryKey: ["incidents"] });
  };

  const doAssign = async (userId: string | null) => {
    if (!incidentId) return;
    setMutateError(null);
    setMutateBusy(true);
    try {
      const next = await assignIncident(incidentId, userId);
      updateIncidentInCache(next);
    } catch (err) {
      setMutateError(err instanceof Error ? err.message : "Failed to assign incident.");
    } finally {
      setMutateBusy(false);
    }
  };

  const doSnooze = async (untilIso: string | null) => {
    if (!incidentId) return;
    setMutateError(null);
    setMutateBusy(true);
    try {
      const next = await snoozeIncident(incidentId, untilIso);
      updateIncidentInCache(next);
    } catch (err) {
      setMutateError(err instanceof Error ? err.message : "Failed to snooze incident.");
    } finally {
      setMutateBusy(false);
    }
  };

  const doCloseToggle = async (closed: boolean) => {
    if (!incidentId) return;
    setMutateError(null);
    setMutateBusy(true);
    try {
      const next = await closeIncident(incidentId, closed);
      updateIncidentInCache(next);
    } catch (err) {
      setMutateError(err instanceof Error ? err.message : "Failed to update incident.");
    } finally {
      setMutateBusy(false);
    }
  };

  const doAddNote = async () => {
    if (!incidentId) return;
    const body = noteDraft.trim();
    if (!body) return;
    setNoteError(null);
    setNoteBusy(true);
    try {
      await createIncidentNote(incidentId, body);
      setNoteDraft("");
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["incidents", incidentId, "notes"] }),
        queryClient.invalidateQueries({ queryKey: ["incidents"] }),
      ]);
    } catch (err) {
      setNoteError(err instanceof Error ? err.message : "Failed to add note.");
    } finally {
      setNoteBusy(false);
    }
  };

  const ackAllEvents = async () => {
    if (!canAck) return;
    const ackable = events
      .filter((evt) => {
        const status = String(evt.status ?? "").trim().toLowerCase();
        return status !== "acknowledged" && status !== "ok";
      })
      .map((evt) => evt.id)
      .filter(Boolean);
    if (!ackable.length) return;
    setMutateError(null);
    setMutateBusy(true);
    try {
      await postJson("/api/alarms/events/ack-bulk", { event_ids: ackable });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["incidents", incidentId ?? "missing"] }),
        queryClient.invalidateQueries({ queryKey: ["alarms", "history"] }),
        queryClient.invalidateQueries({ queryKey: ["alarms"] }),
      ]);
    } catch (err) {
      setMutateError(err instanceof Error ? err.message : "Failed to acknowledge events.");
    } finally {
      setMutateBusy(false);
    }
  };

  const incidentHeader = incident ? (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        <Badge tone={severityTone(incident.severity)}>{incident.severity}</Badge>
        <Badge tone={statusTone(incident.status)}>{incident.status}</Badge>
        <p className="text-xs text-muted-foreground">
          Last activity{" "}
          {formatDistanceToNow(incident.last_event_at, { addSuffix: true })}
        </p>
      </div>
      <div className="flex flex-col gap-2 md:flex-row md:items-end md:justify-between">
        <div className="min-w-0">
          <p className="text-sm font-semibold text-card-foreground">{incident.title}</p>
          {incident.target_key ? (
            <p className="mt-1 text-xs text-muted-foreground">Target: {incident.target_key}</p>
          ) : null}
        </div>

        <div className="flex flex-col gap-2 md:items-end">
          {canEdit ? (
            <div className="grid gap-2 md:grid-cols-3">
              <div>
                <label className="text-[11px] font-semibold text-muted-foreground">Assign</label>
                <Select
                  value={incident.assigned_to ?? ""}
                  onChange={(e) => void doAssign(e.target.value || null)}
                  disabled={mutateBusy}
                >
                  <option value="">Unassigned</option>
                  {meUserId ? (
                    <option value={meUserId}>Me (you)</option>
                  ) : null}
                  {users
                    .filter((u) => u.id !== meUserId)
                    .map((u) => (
                      <option key={u.id} value={u.id}>
                        {u.name}
                      </option>
                    ))}
                </Select>
              </div>
              <div>
                <label className="text-[11px] font-semibold text-muted-foreground">Snooze</label>
                <Select
                  value=""
                  onChange={(e) => {
                    const value = e.target.value;
                    if (!value) return;
                    if (value === "unsnooze") {
                      void doSnooze(null);
                      return;
                    }
                    const minutes = Number(value);
                    if (!Number.isFinite(minutes) || minutes <= 0) return;
                    const until = new Date(Date.now() + minutes * 60 * 1000).toISOString();
                    void doSnooze(until);
                  }}
                  disabled={mutateBusy}
                >
                  <option value="">Select…</option>
                  <option value="15">15m</option>
                  <option value="60">1h</option>
                  <option value="360">6h</option>
                  <option value="1440">24h</option>
                  <option value="unsnooze">Unsnooze</option>
                </Select>
              </div>
              <div className="flex items-end gap-2">
                <NodeButton
                  size="sm"
                  onClick={() => void doCloseToggle(incident.status !== "closed")}
                  disabled={mutateBusy}
                >
                  {incident.status === "closed" ? "Reopen" : "Close"}
                </NodeButton>
                {canAck ? (
                  <NodeButton
                    size="sm"
                    onClick={() => void ackAllEvents()}
                    disabled={mutateBusy}
                  >
                    Ack all
                  </NodeButton>
                ) : null}
              </div>
            </div>
          ) : (
            <p className="text-xs text-muted-foreground">
              {resolveAssigneeLabel(incident, users, meUserId)}
            </p>
          )}
          {incident.snoozed_until ? (
            <p className="text-[11px] text-muted-foreground">
              Snoozed until {formatDateTime(incident.snoozed_until)}
            </p>
          ) : null}
        </div>
      </div>
    </div>
  ) : null;

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent className="max-w-3xl">
        <SheetHeader>
          <SheetTitle>Incident</SheetTitle>
          <SheetDescription>
            Review the timeline, investigate related signals, and capture operator notes for this incident.
          </SheetDescription>
        </SheetHeader>
        <SheetBody className="space-y-4">
          {detailQuery.isLoading ? <LoadingState label="Loading incident..." /> : null}
          {detailQuery.error ? (
            <ErrorState
              message={
                detailQuery.error instanceof Error ? detailQuery.error.message : "Failed to load incident."
              }
            />
          ) : null}

          {mutateError ? <InlineBanner tone="danger">{mutateError}</InlineBanner> : null}

          {incidentHeader}

          <CollapsibleCard
            title="Context chart"
            description={
              focusSensorId ? `${labelMap.get(focusSensorId) ?? focusSensorId}` : "No focus sensor"
            }
            defaultOpen
          >
            <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
              <div className="max-w-md">
                <label className="text-xs font-semibold text-muted-foreground">Focus event</label>
                <Select
                  value={selectedEventId ?? ""}
                  onChange={(e) => setSelectedEventId(e.target.value)}
                >
                  {events.map((evt) => {
                    const ts = evt.created_at instanceof Date ? evt.created_at : new Date(evt.created_at);
                    const label = `${formatDateTime(ts)} · ${evt.message ?? evt.id}`;
                    return (
                      <option key={evt.id} value={evt.id}>
                        {label}
                      </option>
                    );
                  })}
                </Select>
              </div>

              <div className="max-w-xs">
                <label className="text-xs font-semibold text-muted-foreground">Range</label>
                <Select value={contextPreset} onChange={(e) => setContextPreset(e.target.value as ContextPreset)}>
                  <option value="auto">Auto</option>
                  <option value="1h">±1h</option>
                  <option value="6h">±6h</option>
                  <option value="24h">±24h</option>
                </Select>
              </div>
            </div>

            {trendQuery.isLoading ? (
              <p className="mt-3 text-sm text-muted-foreground">Loading chart…</p>
            ) : null}
            {trendQuery.error ? (
              <InlineBanner tone="danger" className="mt-3">
                {trendQuery.error instanceof Error ? trendQuery.error.message : "Failed to load chart."}
              </InlineBanner>
            ) : null}

            {series ? (
              <div className="mt-3">
                <TrendChart
                  title="Sensor context"
                  description={
                    <span className="text-xs text-muted-foreground">
                      {contextWindow.start.toLocaleString()} → {contextWindow.end.toLocaleString()} · interval {chartIntervalSeconds}s
                    </span>
                  }
                  data={[series, ...overlaySeries]}
                  independentAxes={false}
                  stacked={false}
                  navigator={false}
                  analysisTools={false}
                  heightPx={320}
                />
              </div>
            ) : (
              <Card className="mt-3 rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
                No chart data available.
              </Card>
            )}
          </CollapsibleCard>

          <CollapsibleCard
            title="Related signals"
            description="Controller-wide scan for co-occurring sensor activity."
            defaultOpen
            actions={
              <div className="flex items-center gap-2">
                <NodeButton
                  size="sm"
                  onClick={() => void runRelated()}
                  disabled={!focusSensorId || relatedJob.isRunning || relatedJob.isSubmitting}
                >
                  {relatedJob.isRunning || relatedJob.isSubmitting ? "Running…" : "Re-run"}
                </NodeButton>
                {relatedJob.canCancel ? (
                  <NodeButton size="sm" onClick={() => void relatedJob.cancel()}>
                    Cancel
                  </NodeButton>
                ) : null}
              </div>
            }
          >
            {relatedJob.error ? (
              <InlineBanner tone="danger">{relatedJob.error}</InlineBanner>
            ) : null}

            <div className="grid gap-3 md:grid-cols-2">
              <div>
                <label className="text-xs font-semibold text-muted-foreground">Candidate filters</label>
                <div className="mt-2 space-y-2 text-sm">
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={Boolean(relatedFilters.same_node_only)}
                      onChange={(e) =>
                        setRelatedFilters((prev) => ({ ...prev, same_node_only: e.target.checked }))
                      }
                    />
                    Same node only
                  </label>
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={Boolean(relatedFilters.same_unit_only)}
                      onChange={(e) =>
                        setRelatedFilters((prev) => ({ ...prev, same_unit_only: e.target.checked }))
                      }
                    />
                    Same unit only
                  </label>
                  <label className="flex items-center gap-2">
                    <input
                      type="checkbox"
                      checked={Boolean(relatedFilters.same_type_only)}
                      onChange={(e) =>
                        setRelatedFilters((prev) => ({ ...prev, same_type_only: e.target.checked }))
                      }
                    />
                    Same type only
                  </label>
                  <div className="grid gap-2 md:grid-cols-2">
                    <div>
                      <label className="text-[11px] font-semibold text-muted-foreground">Derived</label>
                      <Select
                        value={
                          relatedFilters.is_derived == null ? "all" : relatedFilters.is_derived ? "only" : "exclude"
                        }
                        onChange={(e) => {
                          const value = e.target.value;
                          setRelatedFilters((prev) => ({
                            ...prev,
                            is_derived: value === "all" ? null : value === "only",
                          }));
                        }}
                      >
                        <option value="all">All</option>
                        <option value="exclude">Exclude derived</option>
                        <option value="only">Only derived</option>
                      </Select>
                    </div>
                    <div>
                      <label className="text-[11px] font-semibold text-muted-foreground">External</label>
                      <Select
                        value={
                          relatedFilters.is_public_provider == null
                            ? "all"
                            : relatedFilters.is_public_provider
                              ? "only"
                              : "exclude"
                        }
                        onChange={(e) => {
                          const value = e.target.value;
                          setRelatedFilters((prev) => ({
                            ...prev,
                            is_public_provider: value === "all" ? null : value === "only",
                          }));
                        }}
                      >
                        <option value="all">All</option>
                        <option value="exclude">Exclude external</option>
                        <option value="only">Only external</option>
                      </Select>
                    </div>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    Window: {formatDateTime(contextWindow.start)} → {formatDateTime(contextWindow.end)} · interval{" "}
                    {relatedIntervalSeconds}s
                  </p>
                </div>
              </div>
              <div className="space-y-2">
                <div className="flex flex-wrap items-end justify-between gap-2">
                  <label className="text-xs font-semibold text-muted-foreground">Results</label>
                  <div className="w-48">
                    <label className="text-[11px] font-semibold text-muted-foreground">Sort</label>
                    <Select value={relatedSort} onChange={(e) => setRelatedSort(e.target.value as RelatedSort)}>
                      <option value="combined">Combined</option>
                      <option value="significance">Significance</option>
                      <option value="proximity">Proximity</option>
                    </Select>
                  </div>
                </div>
                {relatedJob.isRunning && relatedJob.progressMessage ? (
                  <InlineBanner tone="info">{relatedJob.progressMessage}</InlineBanner>
                ) : null}
                <ResultsList
                  candidates={relatedCandidates}
                  selectedCandidateId={selectedCandidateId}
                  onSelectCandidate={setSelectedCandidateId}
                  sensorsById={lookups.sensorsById}
                  nodesById={lookups.nodesById}
                  badgeById={badgeById}
                  selectedSensorIds={[]}
                  maxSeries={20}
                  emptyMessage={focusSensorId ? "No related signals found." : "Select a focus sensor to run analysis."}
                />
              </div>
            </div>

            {focusSensorId && selectedCandidate ? (
              <div className="mt-4">
                <PreviewPane
                  focusSensorId={focusSensorId}
                  focusLabel={labelMap.get(focusSensorId) ?? focusSensorId}
                  candidate={selectedCandidate}
                  sensorsById={lookups.sensorsById}
                  labelMap={labelMap}
                  selectedSensorIds={[]}
                  maxSeries={20}
                  relationshipMode="simple"
                  computedThroughTs={(relatedJob.result as RelatedSensorsUnifiedResultV2 | null)?.computed_through_ts ?? null}
                />
              </div>
            ) : null}
          </CollapsibleCard>

          <CollapsibleCard
            title="Other events"
            description="Schedule/output actions in the same time window."
            defaultOpen={false}
          >
            {actionLogsQuery.isLoading ? (
              <p className="text-sm text-muted-foreground">Loading action logs…</p>
            ) : null}
            {actionLogsQuery.error ? (
              <InlineBanner tone="danger">
                {actionLogsQuery.error instanceof Error ? actionLogsQuery.error.message : "Failed to load action logs."}
              </InlineBanner>
            ) : null}
            {!actionLogsQuery.isLoading && !actionLogsQuery.error ? (
              <div className="space-y-3">
                {actionLogs.length ? (
                  <div className="grid gap-3 md:grid-cols-12">
                    <div className="md:col-span-6">
                      <label className="text-xs font-semibold text-muted-foreground">Search</label>
                      <Input
                        value={actionLogSearchDraft}
                        onChange={(e) => setActionLogSearchDraft(e.target.value)}
                        placeholder="Status, schedule id, output id, message…"
                      />
                    </div>
                    <div className="md:col-span-3">
                      <label className="text-xs font-semibold text-muted-foreground">Schedule</label>
                      <Select
                        value={actionLogScheduleFilter}
                        onChange={(e) => setActionLogScheduleFilter(e.target.value)}
                      >
                        <option value="any">Any</option>
                        {actionLogScheduleOptions.map((scheduleId) => (
                          <option key={scheduleId} value={scheduleId}>
                            {scheduleId}
                          </option>
                        ))}
                      </Select>
                    </div>
                    <div className="md:col-span-3">
                      <label className="text-xs font-semibold text-muted-foreground">Status</label>
                      <Select value={actionLogStatusFilter} onChange={(e) => setActionLogStatusFilter(e.target.value)}>
                        <option value="any">Any</option>
                        {actionLogStatusOptions.map((status) => (
                          <option key={status} value={status}>
                            {status}
                          </option>
                        ))}
                      </Select>
                    </div>
                  </div>
                ) : null}

                {filteredActionLogs.length ? (
                  filteredActionLogs.map((log) => (
                    <Card key={log.id} className="rounded-lg border border-border bg-card-inset p-3 text-sm">
                      <p className="font-semibold text-card-foreground">
                        {log.status} · schedule {log.schedule_id}
                      </p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {formatDateTime(log.created_at)}
                        {log.node_id ? ` · node ${log.node_id}` : ""}
                        {log.output_id ? ` · output ${log.output_id}` : ""}
                      </p>
                      {log.message ? (
                        <p className="mt-1 text-xs text-muted-foreground">{log.message}</p>
                      ) : null}
                    </Card>
                  ))
                ) : (
                  <Card className="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
                    No matching action logs in this window.
                  </Card>
                )}
              </div>
            ) : null}
          </CollapsibleCard>

          <CollapsibleCard
            title="Event history"
            description={`${events.length} events`}
            defaultOpen={false}
          >
            <div className="space-y-2">
              {events.map((evt) => {
                const createdAt = evt.created_at instanceof Date ? evt.created_at : new Date(evt.created_at);
                const status = String(evt.status ?? "").trim().toLowerCase();
                const acked = status === "acknowledged" || status === "ok";
                return (
                  <Card key={evt.id} className="rounded-lg border border-border bg-card-inset p-3 text-sm">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <p className="truncate font-semibold text-card-foreground">{evt.message || "Alarm event"}</p>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {formatDateTime(createdAt)} · status {evt.status}
                          {evt.origin ? ` · ${evt.origin}` : ""}
                        </p>
                      </div>
                      {canAck && !acked ? (
                        <NodeButton
                          size="xs"
                          onClick={() => void postJson(`/api/alarms/events/${evt.id}/ack`)}
                        >
                          Ack
                        </NodeButton>
                      ) : null}
                    </div>
                  </Card>
                );
              })}
            </div>
          </CollapsibleCard>

          <CollapsibleCard title="Notes" description={`${notesQuery.data?.notes.length ?? 0} notes`} defaultOpen={false}>
            {notesQuery.isLoading ? (
              <p className="text-sm text-muted-foreground">Loading notes…</p>
            ) : null}
            {notesQuery.error ? (
              <InlineBanner tone="danger">
                {notesQuery.error instanceof Error ? notesQuery.error.message : "Failed to load notes."}
              </InlineBanner>
            ) : null}
            {!notesQuery.isLoading && !notesQuery.error ? (
              <div className="space-y-3">
                {(notesQuery.data?.notes ?? []).length ? (
                  <div className="space-y-2">
                    {(notesQuery.data?.notes ?? []).map((note) => {
                      const createdBy = findUser(users, note.created_by)?.name ?? (note.created_by ? "User" : "System");
                      return (
                        <Card key={note.id} className="rounded-lg border border-border bg-card-inset p-3 text-sm">
                          <p className="text-xs text-muted-foreground">
                            {createdBy} · {formatDateTime(note.created_at)}
                          </p>
                          <p className="mt-1 whitespace-pre-wrap text-sm text-card-foreground">{note.body}</p>
                        </Card>
                      );
                    })}
                  </div>
                ) : (
                  <Card className="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
                    No notes yet.
                  </Card>
                )}

                {canEdit ? (
                  <div className="space-y-2">
                    {noteError ? <InlineBanner tone="danger">{noteError}</InlineBanner> : null}
                    <label className="text-xs font-semibold text-muted-foreground">Add note</label>
                    <Textarea
                      value={noteDraft}
                      onChange={(e) => setNoteDraft(e.target.value)}
                      rows={3}
                      placeholder="What did you observe? What did you change?"
                    />
                    <div className="flex justify-end">
                      <NodeButton size="sm" onClick={() => void doAddNote()} disabled={noteBusy || !noteDraft.trim()}>
                        {noteBusy ? "Saving…" : "Add note"}
                      </NodeButton>
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
          </CollapsibleCard>
        </SheetBody>
      </SheetContent>
    </Sheet>
  );
}

function resolveAssigneeLabel(incident: Incident, users: DemoUser[], meUserId: string | null): string {
  if (!incident.assigned_to) return "Unassigned";
  if (meUserId && incident.assigned_to === meUserId) return "Assigned to you";
  const user = users.find((u) => u.id === incident.assigned_to);
  return user ? `Assigned to ${user.name}` : "Assigned";
}
