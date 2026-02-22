"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";
import { TrendChart } from "@/components/TrendChart";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import AlarmOriginBadge from "@/components/alarms/AlarmOriginBadge";
import AnomalyScore from "@/components/alarms/AnomalyScore";
import { type AlarmOriginFilter } from "@/lib/alarms/origin";
import { formatSensorInterval, getSensorDisplayDecimals } from "@/lib/sensorFormat";
import { queryKeys, useTrendPreviewQuery } from "@/lib/queries";
import filterAlarmsByOrigin from "@/features/sensors/utils/filterAlarmsByOrigin";
import { deleteJson, putJson } from "@/lib/http";
import { useAuth } from "@/components/AuthProvider";
import NodeButton from "@/features/nodes/components/NodeButton";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Sheet, SheetBody, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { DemoAlarm, DemoNode, DemoSensor } from "@/types/dashboard";

export default function SensorDetailDrawer({
  sensor,
  node,
  nodes,
  sensors,
  alarms,
  alarmOriginFilter,
  trendRangeHours,
  onClose,
}: {
  sensor: DemoSensor | null | undefined;
  node: DemoNode | null;
  nodes: DemoNode[];
  sensors: DemoSensor[];
  alarms: DemoAlarm[];
  alarmOriginFilter: AlarmOriginFilter;
  trendRangeHours: number;
  onClose: () => void;
}) {
  const { me } = useAuth();
  const router = useRouter();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const canDelete = canEdit;
  const queryClient = useQueryClient();

  const [nameDraft, setNameDraft] = useState("");
  const [decimalsDraft, setDecimalsDraft] = useState<string>("auto");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const previewRange = useMemo(() => {
    if (!sensor) {
      const now = new Date();
      return {
        start: now.toISOString(),
        end: now.toISOString(),
      };
    }
    const end = new Date();
    const start = new Date(end.getTime() - trendRangeHours * 60 * 60 * 1000);
    return { start: start.toISOString(), end: end.toISOString() };
  }, [sensor, trendRangeHours]);
  const { data: previewSeries = [], error: previewError } = useTrendPreviewQuery({
    sensorId: sensor?.sensor_id ?? "",
    start: previewRange.start,
    end: previewRange.end,
    interval: 300,
    enabled: Boolean(sensor),
  });
  const preview =
    previewSeries.length > 0 && sensor
      ? { ...previewSeries[0], label: sensor.name, unit: sensor.unit, display_decimals: getSensorDisplayDecimals(sensor) ?? undefined }
      : null;

  useEffect(() => {
    if (!sensor) return;
    setNameDraft(sensor.name);
    setDecimalsDraft(String(getSensorDisplayDecimals(sensor) ?? "auto"));
    setMessage(null);
    setDeleteOpen(false);
    setDeleteError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed on specific sensor fields to avoid resetting draft on every object reference change
  }, [sensor?.sensor_id, sensor?.name, sensor?.config]);

  const nodeNameById = useMemo(() => new Map(nodes.map((row) => [row.id, row.name])), [nodes]);
  const sensorById = useMemo(
    () => new Map(sensors.map((row) => [row.sensor_id, row])),
    [sensors],
  );

  const derivedMeta = useMemo(() => {
    const config = ((sensor?.config ?? {}) as Record<string, unknown>) ?? {};
    const source = typeof config.source === "string" ? config.source : null;
    if (source !== "derived") return null;

    const rawDerived = config.derived;
    if (!rawDerived || typeof rawDerived !== "object" || Array.isArray(rawDerived)) {
      return { expression: "", inputs: [] as Array<{ sensorId: string; variable: string }> };
    }
    const derived = rawDerived as Record<string, unknown>;
    const expression = typeof derived.expression === "string" ? derived.expression : "";
    const rawInputs = derived.inputs;
    const inputs = Array.isArray(rawInputs)
      ? rawInputs
          .map((entry) => {
            if (!entry || typeof entry !== "object" || Array.isArray(entry)) return null;
            const obj = entry as Record<string, unknown>;
            const sensorId = typeof obj.sensor_id === "string" ? obj.sensor_id : "";
            const variable = typeof obj.var === "string" ? obj.var : "";
            return sensorId && variable ? { sensorId, variable } : null;
          })
          .filter((entry): entry is { sensorId: string; variable: string } => Boolean(entry))
      : [];

    return { expression, inputs };
  }, [sensor?.config]);

  const sensorAlarms = sensor
    ? filterAlarmsByOrigin(
        alarms.filter(
          (alarm) => alarm.target_type === "sensor" && alarm.target_id === sensor.sensor_id,
        ),
        alarmOriginFilter,
      )
    : [];

  const initialDecimals = sensor ? getSensorDisplayDecimals(sensor) : null;
  const desiredDecimals =
    decimalsDraft === "auto" ? null : Number.isFinite(Number(decimalsDraft)) ? Number.parseInt(decimalsDraft, 10) : null;

  const dirtyName = sensor ? (nameDraft.trim() && nameDraft.trim() !== sensor.name) : false;
  const dirtyDecimals = desiredDecimals !== initialDecimals;
  const dirty = dirtyName || dirtyDecimals;

  const deleteSensor = async () => {
    if (!canDelete) return;
    if (!sensor) return;
    setDeleteBusy(true);
    setDeleteError(null);
    setMessage(null);
    try {
      await deleteJson(`/api/sensors/${encodeURIComponent(sensor.sensor_id)}?keep_data=true`);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
        queryClient.invalidateQueries({ queryKey: queryKeys.alarms }),
      ]);
      onClose();
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : "Failed to delete sensor.");
    } finally {
      setDeleteBusy(false);
    }
  };

  const save = async () => {
    if (!canEdit || !sensor) return;
    const name = nameDraft.trim();
    if (!name) {
      setMessage({ type: "error", text: "Sensor name cannot be empty." });
      return;
    }

    const config = { ...(sensor.config ?? {}) } as Record<string, unknown>;
    if (desiredDecimals == null) {
      delete config.display_decimals;
    } else {
      const clamped = Math.max(0, Math.min(6, Math.floor(desiredDecimals)));
      config.display_decimals = clamped;
    }

    setBusy(true);
    setMessage(null);
    try {
      await putJson(`/api/sensors/${encodeURIComponent(sensor.sensor_id)}`, { name, config });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
      ]);
      setMessage({ type: "success", text: "Saved sensor settings." });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Failed to save sensor settings.",
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Sheet open={!!sensor} onOpenChange={(open) => { if (!open) onClose(); }}>
      <SheetContent className="max-w-xl">
        <SheetHeader>
          <div className="space-y-1">
            <div className="flex flex-wrap items-center gap-2">
              <SheetTitle className="text-lg font-semibold text-foreground">{sensor?.name}</SheetTitle>
              {sensor && <SensorOriginBadge sensor={sensor} />}
            </div>
            <SheetDescription className="text-xs text-muted-foreground">{sensor?.sensor_id}</SheetDescription>
          </div>
          <div className="flex items-center gap-2">
            <NodeButton
              size="sm"
              onClick={() => {
                if (!sensor) return;
                const base = "/sensors";
                const nodeParam = node?.id ? `node=${encodeURIComponent(node.id)}&` : "";
                router.push(`${base}?${nodeParam}sensor=${encodeURIComponent(sensor.sensor_id)}`);
              }}
            >
              Link
            </NodeButton>
            {node?.id ? (
              <NodeButton
                size="sm"
                onClick={() => {
                  router.push(`/nodes/detail?id=${encodeURIComponent(node.id)}`);
                  onClose();
                }}
              >
                Open node
              </NodeButton>
            ) : null}
            <NodeButton onClick={onClose} size="sm">
              Close
            </NodeButton>
          </div>
        </SheetHeader>
        <SheetBody className="space-y-6">
          {sensor && <>
          {message && (
            <InlineBanner tone={message.type === "success" ? "success" : "danger"}>{message.text}</InlineBanner>
          )}

          <CollapsibleCard
            density="sm"
            title="Summary"
            description="Node, type, interval, and location."
            defaultOpen
          >
 <div className="space-y-2 text-sm text-foreground">
              <p>
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Node
                </span>
                <br />
                <span className="font-medium">{node?.name ?? "Unknown"}</span>
              </p>
              <p>
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Type
                </span>
                <br />
                <span className="font-medium">
                  {sensor.type} / {sensor.unit}
                </span>
              </p>
              <p>
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Interval
                </span>
                <br />
                <span className="font-medium">
                  {(() => {
                    const interval = formatSensorInterval(sensor.interval_seconds);
                    return <span title={interval.title}>{interval.label}</span>;
                  })()}
                </span>
                {(sensor.rolling_avg_seconds ?? 0) > 0 &&
                  ` / rolling ${sensor.rolling_avg_seconds}s`}
              </p>
              {sensor.location && (
                <p>
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Location
                  </span>
                  <br />
                  <span className="font-medium">{sensor.location}</span>
                </p>
              )}
            </div>
          </CollapsibleCard>

          {derivedMeta ? (
            <CollapsibleCard
              density="sm"
              title="Derived definition"
              description="Computed by the controller from other sensors. Missing inputs result in missing derived values (no hidden fills/backfills)."
              defaultOpen
            >
              <div className="space-y-4">
                <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Expression
                  </p>
 <pre className="mt-2 overflow-auto rounded-lg bg-card-inset p-3 text-xs text-foreground">
                    <code className="font-mono">{derivedMeta.expression || "—"}</code>
                  </pre>
                </div>

                <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Inputs
                  </p>
                  {derivedMeta.inputs.length ? (
                    <div className="mt-2 space-y-2">
                      {derivedMeta.inputs.map((input) => {
                        const inputSensor = sensorById.get(input.sensorId) ?? null;
                        const nodeName = inputSensor ? nodeNameById.get(inputSensor.node_id) : null;
                        return (
                          <Card
                            key={`${input.variable}:${input.sensorId}`}
                            className="flex-col gap-2 rounded-lg bg-card-inset px-3 py-2 shadow-xs sm:flex-row sm:items-start sm:justify-between"
                          >
                            <div className="min-w-0 space-y-1">
                              <div className="flex flex-wrap items-center gap-2">
 <span className="rounded bg-muted px-2 py-0.5 font-mono text-xs font-semibold text-foreground">
                                  {input.variable}
                                </span>
                                {inputSensor ? <SensorOriginBadge sensor={inputSensor} size="xs" /> : null}
                              </div>
 <p className="truncate text-sm font-semibold text-foreground">
                                {inputSensor
                                  ? `${nodeName ?? inputSensor.node_id} — ${inputSensor.name}`
                                  : input.sensorId}
                              </p>
 <p className="text-xs text-muted-foreground">{input.sensorId}</p>
                            </div>

 <div className="text-xs text-muted-foreground">
                              {inputSensor ? `${inputSensor.type} / ${inputSensor.unit}` : "Sensor not found"}
                            </div>
                          </Card>
                        );
                      })}
                    </div>
                  ) : (
 <p className="mt-2 text-sm text-muted-foreground">No inputs defined.</p>
                  )}
                </div>
              </div>
            </CollapsibleCard>
          ) : null}

          <CollapsibleCard
            density="sm"
            title="Display"
            description="Rename sensors and control the number of decimals shown across tables and trends."
            defaultOpen
            bodyClassName="space-y-4"
            actions={
              <>
                {!canEdit ? (
 <span className="rounded-full bg-muted px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                    Read-only
                  </span>
                ) : null}
                {canEdit ? (
                  <NodeButton
                    variant="primary"
                    size="sm"
                    onClick={() => void save()}
                    disabled={busy || !dirty}
                  >
                    {busy ? "Saving…" : "Save"}
                  </NodeButton>
                ) : null}
              </>
            }
          >
            <div className="grid gap-3 sm:grid-cols-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Sensor name
                <Input
                  className="mt-1"
                  value={nameDraft}
                  onChange={(e) => setNameDraft(e.target.value)}
                  disabled={!canEdit || busy}
                />
              </label>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Display decimals
                <Select
                  className="mt-1"
                  value={decimalsDraft}
                  onChange={(e) => setDecimalsDraft(e.target.value)}
                  disabled={!canEdit || busy}
                >
                  <option value="auto">Auto</option>
                  {[0, 1, 2, 3, 4, 5, 6].map((value) => (
                    <option key={value} value={String(value)}>
                      {value}
                    </option>
                  ))}
                </Select>
              </label>
            </div>

 <p className="text-xs text-muted-foreground">
              {dirty ? "Unsaved changes." : "No changes."}
            </p>
          </CollapsibleCard>

          <CollapsibleCard
            density="sm"
            title="Configuration"
            description="Read-only JSON config (advanced/debug)."
            defaultOpen={false}
          >
 <pre className="max-h-48 overflow-auto rounded-lg bg-card-inset p-3 text-xs text-muted-foreground">
              {JSON.stringify(sensor.config ?? {}, null, 2)}
            </pre>
          </CollapsibleCard>

          <CollapsibleCard
            density="sm"
            title={`Trend preview (last ${trendRangeHours}h)`}
            defaultOpen
            actions={
              previewError ? (
 <span className="text-xs font-semibold text-rose-600">
                  Unable to load preview
                </span>
              ) : null
            }
          >
            <TrendChart data={preview ? [preview] : []} stacked={false} independentAxes={false} />
          </CollapsibleCard>

          <CollapsibleCard
            density="sm"
            title="Alarms"
            defaultOpen={sensorAlarms.some((alarm) => alarm.active)}
            actions={
              sensorAlarms.length ? (
 <span className="text-xs text-muted-foreground">
                  {sensorAlarms.length} configured
                </span>
              ) : null
            }
          >
            {alarmOriginFilter !== "all" ? (
 <p className="text-xs text-muted-foreground">
                Filtering alarms by {alarmOriginFilter === "predictive" ? "Predictive" : "Standard"} origin.
              </p>
            ) : null}
            {sensorAlarms.length ? (
              <div className="mt-2 space-y-2 text-sm text-foreground">
                {sensorAlarms.map((alarm) => (
                  <Card
                    key={alarm.id}
                    className="gap-0 rounded-lg px-3 py-2 shadow-xs"
                  >
                    <div className="flex items-center justify-between gap-3">
                      <div className="space-y-1">
                        <div className="flex flex-wrap items-center gap-2">
 <p className="font-semibold text-foreground">{alarm.name}</p>
                          <AlarmOriginBadge origin={alarm.origin ?? alarm.type} />
                          <span
                            className={`rounded-full px-2 py-0.5 text-xs font-semibold uppercase tracking-wide ${
                              alarm.active
 ? "bg-rose-100 text-rose-700"
 : "bg-emerald-100 text-emerald-700"
                            }`}
                          >
                            {alarm.active ? "active" : "ok"}
                          </span>
                          <span
                            className={`rounded-full px-2 py-0.5 text-xs font-semibold uppercase tracking-wide ${
                              alarm.severity === "critical"
 ? "bg-red-50 text-red-700"
 : "bg-amber-50 text-amber-700"
                            }`}
                          >
                            {alarm.severity}
                          </span>
                          {alarm.status && alarm.status !== (alarm.active ? "active" : "ok") && (
 <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-foreground">
                              {alarm.status}
                            </span>
                          )}
                        </div>
 <p className="text-xs text-muted-foreground">
                          Last fired:{" "}
                          {alarm.last_fired ? alarm.last_fired.toLocaleString() : "—"}
                        </p>
                        {alarm.message && (
 <p className="text-xs text-muted-foreground">{alarm.message}</p>
                        )}
                      </div>
                      <AnomalyScore score={alarm.anomaly_score} />
                    </div>
 <p className="mt-1 text-xs text-muted-foreground">
                      {JSON.stringify(alarm.condition)}
                    </p>
                  </Card>
                ))}
              </div>
            ) : (
 <p className="text-sm text-muted-foreground">No alarms configured for this sensor.</p>
            )}
          </CollapsibleCard>

          {canDelete ? (
            <CollapsibleCard
              density="sm"
              title="Danger zone"
              description="Soft delete removes the sensor from the dashboard UI and preserves telemetry history."
              defaultOpen={false}
 className="border-rose-200 bg-rose-50"
 bodyClassName="space-y-3 border-rose-200"
            >
              {deleteError ? (
 <div className="rounded-lg border border-rose-200 bg-white px-3 py-2 text-sm text-rose-800 shadow-xs">
                  {deleteError}
                </div>
              ) : null}

              {deleteOpen ? (
                <div className="space-y-3">
 <p className="text-sm text-rose-900">
                    Are you sure? The sensor will be renamed with a <span className="font-mono">-deleted-</span>{" "}
                    suffix and will no longer appear in the dashboard.
                  </p>
                  <div className="flex flex-wrap gap-2">
                    <NodeButton size="sm" onClick={() => setDeleteOpen(false)} disabled={deleteBusy}>
                      Cancel
                    </NodeButton>
                    <NodeButton
                      size="sm"
                      variant="danger"
                      onClick={() => void deleteSensor()}
                      loading={deleteBusy}
                    >
                      Delete sensor
                    </NodeButton>
                  </div>
                </div>
              ) : (
                <NodeButton size="sm" variant="danger" onClick={() => setDeleteOpen(true)}>
                  Delete sensor
                </NodeButton>
              )}
            </CollapsibleCard>
          ) : null}
          </>}
        </SheetBody>
      </SheetContent>
    </Sheet>
  );
}
