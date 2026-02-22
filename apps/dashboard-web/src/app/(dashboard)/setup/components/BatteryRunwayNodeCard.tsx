"use client";

import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import CollapsibleCard from "@/components/CollapsibleCard";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";
import { updateBatteryConfig, updatePowerRunwayConfig } from "@/lib/api";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { queryKeys, useBatteryConfigQuery, usePowerRunwayConfigQuery } from "@/lib/queries";
import { sensorMetric, sensorSource } from "@/lib/sensorOrigin";
import type { BatteryModelConfig, PowerRunwayConfig } from "@/types/battery";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

type Message = { type: "success" | "error"; text: string };

type WattSensorCandidate = {
  sensor: DemoSensor;
  nodeName: string;
  source: string | null;
  metric: string | null;
};

function uniq<T>(values: T[]): T[] {
  const out: T[] = [];
  const seen = new Set<T>();
  values.forEach((value) => {
    if (seen.has(value)) return;
    seen.add(value);
    out.push(value);
  });
  return out;
}

export default function BatteryRunwayNodeCard({
  node,
  defaultOpen,
  sensors,
  nodes,
  canEdit,
}: {
  node: DemoNode;
  defaultOpen: boolean;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  canEdit: boolean;
}) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(defaultOpen);

  const batteryConfigQuery = useBatteryConfigQuery(node.id, { enabled: open && canEdit });
  const runwayConfigQuery = usePowerRunwayConfigQuery(node.id, { enabled: open && canEdit });

  const [busy, setBusy] = useState<"saving" | null>(null);
  const [message, setMessage] = useState<Message | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [dirty, setDirty] = useState(false);

  const [batteryDraft, setBatteryDraft] = useState<BatteryModelConfig>({
    enabled: false,
    chemistry: "lifepo4",
    current_sign: "auto",
    sticker_capacity_ah: null,
    soc_cutoff_percent: 20,
    rest_current_abs_a: 2,
    rest_minutes_required: 10,
    soc_anchor_mode: "blend_to_renogy_when_resting",
    soc_anchor_max_step_percent: 1,
    capacity_estimation: {
      enabled: true,
      min_soc_span_percent: 30,
      ema_alpha: 0.1,
      clamp_min_ah: 1,
      clamp_max_ah: 2000,
    },
  });

  const [runwayDraft, setRunwayDraft] = useState<PowerRunwayConfig>({
    enabled: false,
    load_sensor_ids: [],
    history_days: 7,
    pv_derate: 0.75,
    projection_days: 5,
  });

  useEffect(() => {
    if (!open || !canEdit) return;
    if (loaded && dirty) return;
    if (batteryConfigQuery.isLoading || runwayConfigQuery.isLoading) return;
    if (batteryConfigQuery.error || runwayConfigQuery.error) return;

    const batteryModel = batteryConfigQuery.data?.battery_model;
    const powerRunway = runwayConfigQuery.data?.power_runway;
    if (!batteryModel || !powerRunway) return;

    setBatteryDraft({
      ...batteryModel,
      sticker_capacity_ah: batteryModel.sticker_capacity_ah ?? null,
      capacity_estimation: {
        ...batteryModel.capacity_estimation,
      },
    });
    setRunwayDraft({
      ...powerRunway,
      load_sensor_ids: uniq((powerRunway.load_sensor_ids ?? []).map((id) => id.trim()).filter(Boolean)),
    });

    setMessage(null);
    setDirty(false);
    setLoaded(true);
  }, [
    open,
    canEdit,
    loaded,
    dirty,
    batteryConfigQuery.data,
    batteryConfigQuery.isLoading,
    batteryConfigQuery.error,
    runwayConfigQuery.data,
    runwayConfigQuery.isLoading,
    runwayConfigQuery.error,
  ]);

  const statusLabel = useMemo(
    () => formatNodeStatusLabel(node.status ?? "unknown", node.last_seen),
    [node.last_seen, node.status],
  );

  const nodeNameById = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach((n) => map.set(n.id, n.name));
    return map;
  }, [nodes]);

  const wattCandidates = useMemo(() => {
    const candidates: WattSensorCandidate[] = sensors
      .filter((sensor) => sensor.unit === "W")
      .filter((sensor) => sensorSource(sensor) !== "forecast_points")
      .map((sensor) => ({
        sensor,
        nodeName: nodeNameById.get(sensor.node_id) ?? sensor.node_id,
        source: sensorSource(sensor),
        metric: sensorMetric(sensor),
      }));

    const byName = (a: WattSensorCandidate, b: WattSensorCandidate) => {
      const aLocal = a.sensor.node_id === node.id ? 0 : 1;
      const bLocal = b.sensor.node_id === node.id ? 0 : 1;
      if (aLocal !== bLocal) return aLocal - bLocal;
      const nodeCmp = a.nodeName.localeCompare(b.nodeName);
      if (nodeCmp !== 0) return nodeCmp;
      return a.sensor.name.localeCompare(b.sensor.name);
    };

    candidates.sort(byName);
    const local = candidates.filter((c) => c.sensor.node_id === node.id);
    const other = candidates.filter((c) => c.sensor.node_id !== node.id);
    return { local, other };
  }, [node.id, nodeNameById, sensors]);

  const toggleLoadSensor = (sensorId: string, checked: boolean) => {
    setRunwayDraft((prev) => {
      const set = new Set(prev.load_sensor_ids ?? []);
      if (checked) set.add(sensorId);
      else set.delete(sensorId);
      return { ...prev, load_sensor_ids: Array.from(set) };
    });
    setDirty(true);
    setMessage(null);
  };

  const save = async () => {
    setMessage(null);
    if (!canEdit) {
      setMessage({ type: "error", text: "This action requires the config.write capability." });
      return;
    }

    setBusy("saving");
    const errors: string[] = [];
    try {
      await updateBatteryConfig(node.id, {
        battery_model: {
          ...batteryDraft,
          sticker_capacity_ah:
            typeof batteryDraft.sticker_capacity_ah === "number"
              ? batteryDraft.sticker_capacity_ah
              : null,
        },
      });
    } catch (err) {
      errors.push(err instanceof Error ? err.message : "Failed to save battery model configuration.");
    }

    try {
      await updatePowerRunwayConfig(node.id, {
        power_runway: {
          ...runwayDraft,
          load_sensor_ids: uniq(
            (runwayDraft.load_sensor_ids ?? []).map((id) => id.trim()).filter(Boolean),
          ),
        },
      });
    } catch (err) {
      errors.push(err instanceof Error ? err.message : "Failed to save power runway configuration.");
    }

    try {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.batteryConfig(node.id) }),
        queryClient.invalidateQueries({ queryKey: queryKeys.powerRunwayConfig(node.id) }),
      ]);
    } finally {
      // keep going even if invalidation fails
    }

    if (errors.length) {
      setMessage({ type: "error", text: errors.join(" · ") });
    } else {
      setDirty(false);
      setMessage({ type: "success", text: "Saved battery + runway configuration." });
    }

    setBusy(null);
  };

  const resolvedCapacity = batteryConfigQuery.data?.resolved_sticker_capacity_ah ?? null;
  const resolvedSource = batteryConfigQuery.data?.resolved_sticker_capacity_source ?? null;

  const configChips = (
    <div className="flex items-center gap-2">
      <span
        className={`rounded-full px-3 py-1 text-xs font-semibold ${
          (batteryConfigQuery.data?.battery_model.enabled ?? batteryDraft.enabled)
            ? "bg-success-surface text-success-surface-foreground"
            : "bg-card-inset text-card-foreground"
        }`}
      >
        Battery {batteryConfigQuery.data?.battery_model.enabled ? "On" : "Off"}
      </span>
      <span
        className={`rounded-full px-3 py-1 text-xs font-semibold ${
          (runwayConfigQuery.data?.power_runway.enabled ?? runwayDraft.enabled)
            ? "bg-success-surface text-success-surface-foreground"
            : "bg-card-inset text-card-foreground"
        }`}
      >
        Runway {runwayConfigQuery.data?.power_runway.enabled ? "On" : "Off"}
      </span>
    </div>
  );

  return (
    <CollapsibleCard
      title={node.name}
      description={statusLabel}
      actions={
        <>
          {configChips}
          {(batteryConfigQuery.isLoading || runwayConfigQuery.isLoading) && open ? (
            <span className="text-xs text-muted-foreground">Loading…</span>
          ) : null}
        </>
      }
      open={open}
      onOpenChange={setOpen}
      density="sm"
      className="shadow-xs"
    >
      <div className="space-y-4">
        {message ? (
          <InlineBanner tone={message.type === "success" ? "success" : "danger"} className="rounded-lg">
            {message.text}
          </InlineBanner>
        ) : null}

        {batteryConfigQuery.error || runwayConfigQuery.error ? (
          <InlineBanner tone="danger" className="rounded-lg">
            Failed to load configuration:{" "}
            {batteryConfigQuery.error instanceof Error
              ? batteryConfigQuery.error.message
              : runwayConfigQuery.error instanceof Error
                ? runwayConfigQuery.error.message
                : "Unknown error"}
          </InlineBanner>
        ) : null}

        <div className="grid gap-6 lg:grid-cols-2">
          <Card className="rounded-lg gap-0 bg-card-inset p-4">
            <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
              <div>
                <p className="text-sm font-semibold text-card-foreground">Battery model</p>
                <p className="text-xs text-muted-foreground">
                  Sticker capacity powers the remaining-Ah readback. Leave blank to use Renogy
                  desired settings when available.
                </p>
              </div>
              <label className="flex items-center gap-2 text-sm text-foreground">
                <input
                  type="checkbox"
                  checked={batteryDraft.enabled}
                  onChange={(event) => {
                    setBatteryDraft((prev) => ({ ...prev, enabled: event.target.checked }));
                    setDirty(true);
                    setMessage(null);
                  }}
                  disabled={!canEdit}
                />
                Enabled
              </label>
            </div>

            <div className="mt-3 grid gap-3 md:grid-cols-2">
              <label className="grid gap-1 text-sm text-foreground">
                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Sticker capacity (Ah)
                </span>
                <NumericDraftInput
                  value={batteryDraft.sticker_capacity_ah}
                  onValueChange={(next) => {
                    setBatteryDraft((prev) => ({
                      ...prev,
                      sticker_capacity_ah: typeof next === "number" ? next : null,
                    }));
                    setDirty(true);
                  }}
                  emptyValue={null}
                  min={0.0001}
                  inputMode="decimal"
                  clampOnBlur
                  className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                  disabled={!canEdit}
                />
                {resolvedCapacity != null ? (
                  <span className="text-xs text-muted-foreground">
                    Resolved: {Math.round(resolvedCapacity * 100) / 100} Ah
                    {resolvedSource ? ` (${resolvedSource})` : ""}
                  </span>
                ) : (
                  <span className="text-xs text-muted-foreground">Resolved: —</span>
                )}
              </label>

              <label className="grid gap-1 text-sm text-foreground">
                <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  SOC cutoff (%)
                </span>
                <NumericDraftInput
                  value={batteryDraft.soc_cutoff_percent}
                  onValueChange={(next) => {
                    if (typeof next !== "number") return;
                    setBatteryDraft((prev) => ({ ...prev, soc_cutoff_percent: next }));
                    setDirty(true);
                  }}
                  min={0}
                  max={100}
                  enforceRange
                  clampOnBlur
                  className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                  disabled={!canEdit}
                />
                <span className="text-xs text-muted-foreground">Runway stops when projected SOC reaches this.</span>
              </label>

              <div className="md:col-span-2">
                <div className="mt-2 grid gap-3 md:grid-cols-2">
                  <label className="grid gap-1 text-sm text-foreground">
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Chemistry
                    </span>
                    <Select
                      value={batteryDraft.chemistry}
                      onChange={(event) => {
                        setBatteryDraft((prev) => ({
                          ...prev,
                          chemistry: event.target.value === "lead_acid" ? "lead_acid" : "lifepo4",
                        }));
                        setDirty(true);
                      }}
                      disabled={!canEdit}
                    >
                      <option value="lifepo4">LiFePO₄ (recommended)</option>
                      <option value="lead_acid">Lead-acid</option>
                    </Select>
                  </label>

                  <label className="grid gap-1 text-sm text-foreground">
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Current sign
                    </span>
                    <Select
                      value={batteryDraft.current_sign}
                      onChange={(event) => {
                        const value = event.target.value as BatteryModelConfig["current_sign"];
                        const next =
                          value === "positive_is_charging" || value === "positive_is_discharging"
                            ? value
                            : "auto";
                        setBatteryDraft((prev) => ({ ...prev, current_sign: next }));
                        setDirty(true);
                      }}
                      disabled={!canEdit}
                    >
                      <option value="auto">Auto</option>
                      <option value="positive_is_charging">+A is charging</option>
                      <option value="positive_is_discharging">+A is discharging</option>
                    </Select>
                  </label>
                </div>

                <div className="mt-3 grid gap-3 md:grid-cols-2">
                  <label className="grid gap-1 text-sm text-foreground">
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Rest current threshold (A)
                    </span>
                    <NumericDraftInput
                      value={batteryDraft.rest_current_abs_a}
                      onValueChange={(next) => {
                        if (typeof next !== "number") return;
                        setBatteryDraft((prev) => ({ ...prev, rest_current_abs_a: next }));
                        setDirty(true);
                      }}
                      min={0}
                      inputMode="decimal"
                      clampOnBlur
                      className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                      disabled={!canEdit}
                    />
                  </label>

                  <label className="grid gap-1 text-sm text-foreground">
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Rest minutes required
                    </span>
                    <NumericDraftInput
                      value={batteryDraft.rest_minutes_required}
                      onValueChange={(next) => {
                        if (typeof next !== "number") return;
                        setBatteryDraft((prev) => ({ ...prev, rest_minutes_required: next }));
                        setDirty(true);
                      }}
                      min={0}
                      max={1440}
                      integer
                      enforceRange
                      clampOnBlur
                      inputMode="numeric"
                      className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                      disabled={!canEdit}
                    />
                  </label>
                </div>

                <div className="mt-3 grid gap-3 md:grid-cols-2">
                  <label className="grid gap-1 text-sm text-foreground">
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      SOC anchor mode
                    </span>
                    <Select
                      value={batteryDraft.soc_anchor_mode}
                      onChange={(event) => {
                        const value = event.target.value as BatteryModelConfig["soc_anchor_mode"];
                        const next = value === "disabled" ? "disabled" : "blend_to_renogy_when_resting";
                        setBatteryDraft((prev) => ({ ...prev, soc_anchor_mode: next }));
                        setDirty(true);
                      }}
                      disabled={!canEdit}
                    >
                      <option value="blend_to_renogy_when_resting">Blend to Renogy when resting</option>
                      <option value="disabled">Disabled</option>
                    </Select>
                  </label>

                  <label className="grid gap-1 text-sm text-foreground">
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Anchor max step (%)
                    </span>
                    <NumericDraftInput
                      value={batteryDraft.soc_anchor_max_step_percent}
                      onValueChange={(next) => {
                        if (typeof next !== "number") return;
                        setBatteryDraft((prev) => ({ ...prev, soc_anchor_max_step_percent: next }));
                        setDirty(true);
                      }}
                      min={0}
                      max={100}
                      enforceRange
                      clampOnBlur
                      className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                      disabled={!canEdit}
                    />
                  </label>
                </div>
              </div>
            </div>
          </Card>

          <Card className="rounded-lg gap-0 bg-card-inset p-4">
            <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
              <div>
                <p className="text-sm font-semibold text-card-foreground">Power runway</p>
                <p className="text-xs text-muted-foreground">
                  Select the true load sensor(s) in watts (ADC-hat). Runway uses PV forecasts when available,
                  then assumes PV=0 beyond the horizon.
                </p>
              </div>
              <label className="flex items-center gap-2 text-sm text-foreground">
                <input
                  type="checkbox"
                  checked={runwayDraft.enabled}
                  onChange={(event) => {
                    setRunwayDraft((prev) => ({ ...prev, enabled: event.target.checked }));
                    setDirty(true);
                    setMessage(null);
                  }}
                  disabled={!canEdit}
                />
                Enabled
              </label>
            </div>

            <div className="mt-3 grid gap-3">
              {runwayDraft.enabled && !batteryDraft.enabled ? (
                <InlineBanner tone="warning" className="rounded-lg">
                  Power runway requires the Battery model to be enabled (runway uses estimated SOC).
                </InlineBanner>
              ) : null}

              <div className="grid gap-3 md:grid-cols-3">
                <label className="grid gap-1 text-sm text-foreground">
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    History days
                  </span>
                  <NumericDraftInput
                    value={runwayDraft.history_days}
                    onValueChange={(next) => {
                      if (typeof next !== "number") return;
                      setRunwayDraft((prev) => ({ ...prev, history_days: next }));
                      setDirty(true);
                    }}
                    min={1}
                    max={30}
                    integer
                    enforceRange
                    clampOnBlur
                    inputMode="numeric"
                    className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                    disabled={!canEdit}
                  />
                </label>

                <label className="grid gap-1 text-sm text-foreground">
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    PV derate
                  </span>
                  <NumericDraftInput
                    value={runwayDraft.pv_derate}
                    onValueChange={(next) => {
                      if (typeof next !== "number") return;
                      setRunwayDraft((prev) => ({ ...prev, pv_derate: next }));
                      setDirty(true);
                    }}
                    min={0}
                    max={1}
                    enforceRange
                    clampOnBlur
                    inputMode="decimal"
                    className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                    disabled={!canEdit}
                  />
                </label>

                <label className="grid gap-1 text-sm text-foreground">
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Projection days
                  </span>
                  <NumericDraftInput
                    value={runwayDraft.projection_days}
                    onValueChange={(next) => {
                      if (typeof next !== "number") return;
                      setRunwayDraft((prev) => ({ ...prev, projection_days: next }));
                      setDirty(true);
                    }}
                    min={1}
                    max={14}
                    integer
                    enforceRange
                    clampOnBlur
                    inputMode="numeric"
                    className="w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground"
                    disabled={!canEdit}
                  />
                </label>
              </div>

              <div className="rounded-lg border border-border bg-white p-3">
                <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Load sensors (W)
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  Pick the sensor(s) that represent your true consumption. Avoid Renogy{" "}
                  <code className="px-1">load_power_w</code> if the load wiring is not through the controller.
                </p>

                {wattCandidates.local.length === 0 && wattCandidates.other.length === 0 ? (
                  <p className="mt-2 text-sm text-muted-foreground">No watt sensors detected yet.</p>
                ) : (
                  <div className="mt-3 space-y-4">
                    <div className="space-y-2">
                      <p className="text-xs font-semibold text-card-foreground">This node</p>
                      {wattCandidates.local.length ? (
                        <div className="space-y-2">
                          {wattCandidates.local.map((candidate) => (
                            <label
                              key={candidate.sensor.sensor_id}
                              className="flex items-start gap-2 text-sm text-foreground"
                            >
                              <input
                                type="checkbox"
                                checked={runwayDraft.load_sensor_ids.includes(candidate.sensor.sensor_id)}
                                onChange={(event) =>
                                  toggleLoadSensor(candidate.sensor.sensor_id, event.target.checked)
                                }
                                disabled={!canEdit}
                              />
                              <span className="min-w-0 flex-1">
                                <span className="font-semibold">{candidate.sensor.name}</span>
                                <span className="block text-xs text-muted-foreground">
                                  {candidate.source ?? "unknown"} · {candidate.metric ?? "unknown"} ·{" "}
                                  {candidate.sensor.sensor_id}
                                </span>
                              </span>
                            </label>
                          ))}
                        </div>
                      ) : (
                        <p className="text-sm text-muted-foreground">No watt sensors on this node.</p>
                      )}
                    </div>

                    {wattCandidates.other.length ? (
                      <div className="space-y-2">
                        <p className="text-xs font-semibold text-card-foreground">Other nodes</p>
                        <div className="space-y-2">
                          {wattCandidates.other.map((candidate) => (
                            <label
                              key={candidate.sensor.sensor_id}
                              className="flex items-start gap-2 text-sm text-foreground"
                            >
                              <input
                                type="checkbox"
                                checked={runwayDraft.load_sensor_ids.includes(candidate.sensor.sensor_id)}
                                onChange={(event) =>
                                  toggleLoadSensor(candidate.sensor.sensor_id, event.target.checked)
                                }
                                disabled={!canEdit}
                              />
                              <span className="min-w-0 flex-1">
                                <span className="font-semibold">
                                  {candidate.nodeName} — {candidate.sensor.name}
                                </span>
                                <span className="block text-xs text-muted-foreground">
                                  {candidate.source ?? "unknown"} · {candidate.metric ?? "unknown"} ·{" "}
                                  {candidate.sensor.sensor_id}
                                </span>
                              </span>
                            </label>
                          ))}
                        </div>
                      </div>
                    ) : null}
                  </div>
                )}
              </div>

              {runwayDraft.enabled &&
              runwayDraft.load_sensor_ids.length > 0 &&
              runwayConfigQuery.data &&
              !runwayConfigQuery.data.load_sensors_valid ? (
                <InlineBanner tone="warning" className="rounded-lg">
                  Saved load sensors are not valid watt sensors. Select valid sensors and save again.
                </InlineBanner>
              ) : null}

              <div className="flex flex-wrap items-center gap-2">
                <NodeButton
                  variant="primary"
                  size="xs"
                  onClick={() => void save()}
                  disabled={!dirty || busy != null}
                >
                  {busy === "saving" ? "Saving..." : "Save"}
                </NodeButton>
                {!dirty ? <span className="text-xs text-muted-foreground">No unsaved changes</span> : null}
              </div>
            </div>
          </Card>
        </div>
      </div>
    </CollapsibleCard>
  );
}
