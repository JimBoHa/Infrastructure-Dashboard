"use client";

import { useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import AlarmOriginBadge from "@/components/alarms/AlarmOriginBadge";
import AnomalyScore from "@/components/alarms/AnomalyScore";
import { TrendChart } from "@/components/TrendChart";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { postJson } from "@/lib/api";
import { queryKeys, useAlarmsQuery, useNodesQuery, useSensorsQuery, useTrendPreviewQuery } from "@/lib/queries";
import { getSensorDisplayDecimals } from "@/lib/sensorFormat";
import { Select } from "@/components/ui/select";
import { Sheet, SheetBody, SheetContent, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import NodeButton from "@/features/nodes/components/NodeButton";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { useAuth } from "@/components/AuthProvider";
import type { DemoAlarmEvent, TrendSeriesEntry, TrendSeriesPoint } from "@/types/dashboard";

type RangePreset = "context" | "last_1h" | "last_6h" | "last_24h";

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

function rangePresetLabel(preset: RangePreset): string {
  switch (preset) {
    case "context":
      return "Event context";
    case "last_1h":
      return "Last 1 hour";
    case "last_6h":
      return "Last 6 hours";
    case "last_24h":
      return "Last 24 hours";
  }
}

function intervalSecondsForPreset(preset: RangePreset): number {
  switch (preset) {
    case "last_1h":
      return 30;
    case "context":
      return 60;
    case "last_6h":
      return 60;
    case "last_24h":
      return 300;
  }
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

export default function AlarmEventDetailDrawer({
  event,
  onClose,
}: {
  event: DemoAlarmEvent;
  onClose: () => void;
}) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canAck = Boolean(me?.capabilities?.includes("alerts.ack"));

  const alarmsQuery = useAlarmsQuery();
  const sensorsQuery = useSensorsQuery();
  const nodesQuery = useNodesQuery();

  const alarms = useMemo(() => alarmsQuery.data ?? [], [alarmsQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);
  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);

  const alarm = useMemo(() => {
    if (!event.alarm_id) return null;
    return alarms.find((row) => String(row.id) === String(event.alarm_id)) ?? null;
  }, [alarms, event.alarm_id]);

  const sensorId = alarm?.sensor_id ?? event.sensor_id ?? null;
  const nodeId = alarm?.node_id ?? event.node_id ?? null;

  const sensor = useMemo(() => (sensorId ? sensors.find((row) => row.sensor_id === sensorId) ?? null : null), [
    sensors,
    sensorId,
  ]);
  const node = useMemo(() => (nodeId ? nodes.find((row) => row.id === nodeId) ?? null : null), [nodes, nodeId]);

  const raisedAt = useMemo(() => {
    const raw = event.created_at;
    return raw instanceof Date ? raw : new Date(raw);
  }, [event.created_at]);

  const statusLower = (value: unknown) => String(value ?? "").trim().toLowerCase();
  const isResolved = statusLower(event.status) === "acknowledged" || statusLower(event.status) === "ok";

  const [rangePreset, setRangePreset] = useState<RangePreset>("context");
  const intervalSeconds = intervalSecondsForPreset(rangePreset);

  const trendWindow = useMemo(() => {
    if (rangePreset === "context") {
      const start = new Date(raisedAt.getTime() - 60 * 60 * 1000);
      const end = new Date(raisedAt.getTime() + 60 * 60 * 1000);
      return { start, end };
    }
    const now = new Date();
    const hours =
      rangePreset === "last_1h" ? 1 : rangePreset === "last_6h" ? 6 : rangePreset === "last_24h" ? 24 : 6;
    const start = new Date(now.getTime() - hours * 60 * 60 * 1000);
    return { start, end: now };
  }, [rangePreset, raisedAt]);

  const trendQuery = useTrendPreviewQuery({
    sensorId: sensorId ?? "",
    start: trendWindow.start.toISOString(),
    end: trendWindow.end.toISOString(),
    interval: intervalSeconds,
    enabled: Boolean(sensorId),
  });

  const thresholdMeta = useMemo(() => {
    const rawRule = asRecord(alarm?.rule ?? null);
    return extractThresholdMeta(rawRule);
  }, [alarm?.rule]);

  const chartData = useMemo(() => {
    if (!sensorId) return [];
    if (!trendQuery.data || trendQuery.data.length === 0) return [];
    const base = trendQuery.data[0];
    const decimals = sensor ? getSensorDisplayDecimals(sensor) : null;

    const series: TrendSeriesEntry[] = [
      {
        ...base,
        sensor_id: sensorId,
        label: sensor?.name ?? base.label ?? sensorId,
        unit: sensor?.unit ?? base.unit,
        display_decimals: decimals ?? undefined,
      },
    ];

    if (thresholdMeta.threshold != null) {
      const label = thresholdMeta.operator
        ? `Threshold (${thresholdMeta.operator} ${thresholdMeta.threshold})`
        : `Threshold (${thresholdMeta.threshold})`;
      series.push(buildConstantSeries(series[0], thresholdMeta.threshold, label));
    } else {
      if (thresholdMeta.min != null) series.push(buildConstantSeries(series[0], thresholdMeta.min, `Min (${thresholdMeta.min})`));
      if (thresholdMeta.max != null) series.push(buildConstantSeries(series[0], thresholdMeta.max, `Max (${thresholdMeta.max})`));
    }

    return series;
  }, [sensor, sensorId, thresholdMeta, trendQuery.data]);

  const [ackBusy, setAckBusy] = useState(false);
  const [ackError, setAckError] = useState<string | null>(null);

  const acknowledge = async () => {
    if (!canAck) return;
    if (isResolved) return;
    setAckBusy(true);
    setAckError(null);
    try {
      await postJson(`/api/alarms/events/${encodeURIComponent(event.id)}/ack`);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["alarms", "history"] }),
        queryClient.invalidateQueries({ queryKey: queryKeys.alarms }),
      ]);
    } catch (err) {
      setAckError(err instanceof Error ? err.message : "Failed to acknowledge alarm.");
    } finally {
      setAckBusy(false);
    }
  };

  const title = event.message || alarm?.name || "Alarm event";

  return (
    <Sheet open onOpenChange={(v) => { if (!v) onClose(); }}>
      <SheetContent className="z-[70]" data-testid="alarm-event-detail-drawer">
        <SheetHeader className="gap-4">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
 <SheetTitle className="min-w-0 truncate text-foreground" title={title}>
                {title}
              </SheetTitle>
              <AlarmOriginBadge origin={event.origin ?? null} />
              <AnomalyScore score={event.anomaly_score ?? null} />
            </div>
 <div className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
 <span className="rounded-full bg-muted px-2 py-0.5 font-mono text-[11px] text-foreground">
                {event.id}
              </span>
              <span>·</span>
              <span title={formatDateTime(raisedAt)}>
                Raised {formatDistanceToNow(raisedAt, { addSuffix: true })}
              </span>
              <span>·</span>
              <span>
                Status:{" "}
 <span className={isResolved ? "font-semibold text-foreground" : "font-semibold text-rose-700"}>
                  {event.status}
                </span>
              </span>
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-2">
            {canAck && !isResolved ? (
              <NodeButton size="sm" onClick={() => void acknowledge()} disabled={ackBusy}>
                {ackBusy ? "Acknowledging..." : "Acknowledge"}
              </NodeButton>
            ) : null}
            <NodeButton size="sm" onClick={onClose}>
              Close
            </NodeButton>
          </div>
        </SheetHeader>

        <SheetBody className="py-5">
          {ackError ? (
            <InlineBanner tone="danger" className="mb-4 rounded-lg px-3 py-2">{ackError}</InlineBanner>
          ) : null}

          <CollapsibleCard
            density="sm"
            title="Target"
            defaultOpen
            bodyClassName="space-y-3"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
 <div className="text-sm text-foreground">
                  {sensor ? (
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="font-semibold">{sensor.name}</span>
                      <SensorOriginBadge sensor={sensor} size="xs" />
 <span className="text-xs text-muted-foreground">({sensor.sensor_id})</span>
                    </div>
                  ) : sensorId ? (
                    <div className="text-sm">
                      <span className="font-semibold">Sensor</span>{" "}
 <span className="font-mono text-xs text-muted-foreground">{sensorId}</span>
                    </div>
                  ) : (
 <div className="text-sm text-muted-foreground">No sensor attached to this event.</div>
                  )}
                </div>
                {node ? (
 <div className="mt-1 text-xs text-muted-foreground">
 Node: <span className="font-semibold text-foreground">{node.name}</span>{" "}
                    <span className="font-mono">({node.id})</span>
                  </div>
                ) : nodeId ? (
 <div className="mt-1 text-xs text-muted-foreground">
                    Node: <span className="font-mono">{nodeId}</span>
                  </div>
                ) : null}
              </div>

              <div className="flex shrink-0 flex-col items-end gap-2">
                {sensorId ? (
                  <NodeButton
                    size="xs"
                    onClick={() => {
                      const nodeParam = nodeId ? `node=${encodeURIComponent(nodeId)}&` : "";
                      router.push(`/sensors?${nodeParam}sensor=${encodeURIComponent(sensorId)}`);
                      onClose();
                    }}
                  >
                    Open sensor
                  </NodeButton>
                ) : null}
                {nodeId ? (
                  <NodeButton
                    size="xs"
                    onClick={() => {
                      router.push(`/nodes/detail?id=${encodeURIComponent(nodeId)}`);
                      onClose();
                    }}
                  >
                    Open node
                  </NodeButton>
                ) : null}
              </div>
            </div>
          </CollapsibleCard>

          <CollapsibleCard
            density="sm"
            title="Alarm context chart"
            description="Underlying telemetry around the event (no forecasting or synthetic backfill)."
            defaultOpen
            className="mt-4"
            bodyClassName="space-y-4"
            actions={
 <label className="text-xs font-semibold text-muted-foreground">
                Range
                <Select
                  className="ms-2 h-8 w-auto px-2 text-xs"
                  value={rangePreset}
                  onChange={(event) => setRangePreset(event.target.value as RangePreset)}
                >
                  <option value="context">{rangePresetLabel("context")}</option>
                  <option value="last_1h">{rangePresetLabel("last_1h")}</option>
                  <option value="last_6h">{rangePresetLabel("last_6h")}</option>
                  <option value="last_24h">{rangePresetLabel("last_24h")}</option>
                </Select>
              </label>
            }
          >

 <div className="mt-2 text-xs text-muted-foreground">
              Window:{" "}
              <span className="font-mono">
                {trendWindow.start.toISOString()} → {trendWindow.end.toISOString()}
              </span>{" "}
              · interval {intervalSeconds}s
            </div>

            {sensorId ? (
              <>
                {trendQuery.isLoading ? (
 <div className="mt-4 text-sm text-muted-foreground">Loading chart…</div>
                ) : trendQuery.error ? (
                  <InlineBanner tone="danger" className="mt-4 rounded-lg px-3 py-2">
                    {trendQuery.error instanceof Error ? trendQuery.error.message : "Failed to load chart data."}
                  </InlineBanner>
                ) : (
                  <div data-testid="alarm-event-context-chart" className="mt-4">
                    <TrendChart data={chartData} />
                  </div>
                )}
              </>
            ) : (
              <Card className="mt-4 rounded-lg gap-0 border-dashed bg-card-inset px-3 py-3 text-sm text-card-foreground">
                This event is not linked to a specific sensor, so no trend chart is available yet.
              </Card>
            )}

            {alarm ? (
              <Card className="mt-4 rounded-lg gap-0 bg-card-inset px-3 py-2 text-xs text-card-foreground">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="font-semibold">Alarm definition</span>
 <span className="rounded-full bg-white px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
                    {alarm.id}
                  </span>
 <span className="text-muted-foreground">·</span>
                  <span className="font-semibold">{alarm.name}</span>
                  {alarm.type ? (
                    <>
 <span className="text-muted-foreground">·</span>
                      <span>type={alarm.type}</span>
                    </>
                  ) : null}
                  {alarm.severity ? (
                    <>
 <span className="text-muted-foreground">·</span>
                      <span>severity={alarm.severity}</span>
                    </>
                  ) : null}
                </div>

                {thresholdMeta.threshold != null || thresholdMeta.min != null || thresholdMeta.max != null ? (
                  <div className="mt-2">
                    <span className="font-semibold">Condition:</span>{" "}
                    {thresholdMeta.threshold != null ? (
                      <span className="font-mono">
                        {thresholdMeta.operator ?? "?"} {thresholdMeta.threshold}
                      </span>
                    ) : (
                      <span className="font-mono">
                        {thresholdMeta.min != null ? `min=${thresholdMeta.min}` : ""}
                        {thresholdMeta.min != null && thresholdMeta.max != null ? " " : ""}
                        {thresholdMeta.max != null ? `max=${thresholdMeta.max}` : ""}
                      </span>
                    )}
                  </div>
                ) : (
 <div className="mt-2 text-muted-foreground">
                    Condition details are not structured enough to display; see &ldquo;Raw alarm rule&rdquo; below.
                  </div>
                )}
              </Card>
            ) : null}
          </CollapsibleCard>

          <div className="mt-4 space-y-3">
            <CollapsibleCard density="sm" title="Raw event (JSON)" defaultOpen={false}>
 <pre className="mt-3 overflow-auto rounded-lg bg-card-inset p-3 text-[11px] text-foreground">
                {JSON.stringify(event, null, 2)}
              </pre>
            </CollapsibleCard>

            <CollapsibleCard density="sm" title="Raw alarm rule (JSON)" defaultOpen={false}>
 <pre className="mt-3 overflow-auto rounded-lg bg-card-inset p-3 text-[11px] text-foreground">
                {JSON.stringify(alarm?.rule ?? null, null, 2)}
              </pre>
            </CollapsibleCard>
          </div>
        </SheetBody>
      </SheetContent>
    </Sheet>
  );
}
