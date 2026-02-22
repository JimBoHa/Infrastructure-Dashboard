"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";
import { getNodeSensorsConfig, updateNodeSensorsConfig } from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import type {
  NodeAds1263SettingsDraft,
  NodeAnalogHealth,
  NodeSensorDraft,
} from "@/lib/apiSchemas";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { SensorCard } from "@/app/(dashboard)/provisioning/components/SensorCard";
import { SENSOR_PRESETS } from "@/app/(dashboard)/provisioning/presets";
import type {
  SensorDraft,
  SensorDriverType,
  SensorFieldKey,
  SensorPresetKey,
} from "@/app/(dashboard)/provisioning/types";
import { randomHex } from "@/app/(dashboard)/provisioning/utils";

const DRIVER_TYPES: SensorDriverType[] = ["analog", "pulse", "gpio_pulse"];
const DEFAULT_ADS1263: NodeAds1263SettingsDraft = {
  enabled: false,
  spi_bus: null,
  spi_device: null,
  spi_mode: null,
  spi_speed_hz: null,
  rst_bcm: null,
  cs_bcm: null,
  drdy_bcm: null,
  vref_volts: null,
  gain: null,
  data_rate: null,
  scan_interval_seconds: null,
};

type AnalogHealthSummary = {
  label: string;
  tone: "ok" | "warn";
  detail?: string;
};

function summarizeAnalogHealth(
  backend: string | null,
  health: NodeAnalogHealth | null,
): AnalogHealthSummary {
  if (!backend) {
    return { label: "Unknown", tone: "warn", detail: "No backend reported yet." };
  }
  const normalizedBackend = backend.trim().toLowerCase();
  const ok = Boolean(health?.ok);
  if (ok) {
    const chip = health?.chip_id ? ` · chip ${health.chip_id}` : "";
    return { label: `${normalizedBackend} OK${chip}`, tone: "ok" };
  }

  const lastError = (health?.last_error ?? "").trim();
  const lastErrorLower = lastError.toLowerCase();
  if (lastErrorLower.includes("spi may be disabled") || lastErrorLower.includes("dtparam=spi=on")) {
    return { label: "SPI disabled", tone: "warn", detail: lastError || undefined };
  }
  if (lastErrorLower.includes("chip id mismatch")) {
    return { label: "Not detected", tone: "warn", detail: lastError || undefined };
  }
  if (lastErrorLower.includes("drdy timeout")) {
    return { label: "DRDY timeout", tone: "warn", detail: lastError || undefined };
  }
  if (lastErrorLower.includes("backend disabled")) {
    return { label: "Disabled", tone: "warn", detail: lastError || undefined };
  }
  if (!lastError) {
    return { label: "Not ready", tone: "warn", detail: "Waiting for the node to report ADC health." };
  }
  return { label: "Not ready", tone: "warn", detail: lastError };
}

function coercePreset(value: string): SensorPresetKey {
  return Object.hasOwn(SENSOR_PRESETS, value) ? (value as SensorPresetKey) : "custom";
}

function coerceDriver(value: string): SensorDriverType {
  return DRIVER_TYPES.includes(value as SensorDriverType)
    ? (value as SensorDriverType)
    : "analog";
}

function buildNewSensor(key: string, index: number): SensorDraft {
  return {
    key,
    preset: "custom",
    sensor_id: "",
    name: `Sensor ${index}`,
    type: "analog",
    channel: 0,
    unit: "V",
    location: "",
    interval_seconds: 30,
    rolling_average_seconds: 0,
    input_min: null,
    input_max: null,
    output_min: null,
    output_max: null,
    offset: 0,
    scale: 1,
    pulses_per_unit: null,
    current_loop_shunt_ohms: null,
    current_loop_range_m: null,
  };
}

function normalizeDrafts(raw: NodeSensorDraft[]): SensorDraft[] {
  return raw.map((sensor, index) => ({
    key: `sensor-${index + 1}-${sensor.sensor_id || randomHex(2)}`,
    preset: coercePreset(sensor.preset ?? "custom"),
    sensor_id: sensor.sensor_id ?? "",
    name: sensor.name ?? `Sensor ${index + 1}`,
    type: coerceDriver(sensor.type ?? "analog"),
    channel: Number.isFinite(sensor.channel) ? Number(sensor.channel) : 0,
    unit: sensor.unit ?? "",
    location: sensor.location ?? "",
    interval_seconds: Number.isFinite(sensor.interval_seconds) ? Number(sensor.interval_seconds) : 30,
    rolling_average_seconds: Number.isFinite(sensor.rolling_average_seconds)
      ? Number(sensor.rolling_average_seconds)
      : 0,
    input_min: sensor.input_min ?? null,
    input_max: sensor.input_max ?? null,
    output_min: sensor.output_min ?? null,
    output_max: sensor.output_max ?? null,
    offset: Number.isFinite(sensor.offset) ? Number(sensor.offset) : 0,
    scale: Number.isFinite(sensor.scale) ? Number(sensor.scale) : 1,
    pulses_per_unit: sensor.pulses_per_unit ?? null,
    current_loop_shunt_ohms: sensor.current_loop_shunt_ohms ?? null,
    current_loop_range_m: sensor.current_loop_range_m ?? null,
  }));
}

function validateSensors(sensors: SensorDraft[]): Map<string, Set<SensorFieldKey>> {
  const invalid = new Map<string, Set<SensorFieldKey>>();
  const seen = new Map<string, string[]>();

  for (const sensor of sensors) {
    const fields = new Set<SensorFieldKey>();

    const name = sensor.name.trim();
    if (!name) fields.add("name");

    const unit = sensor.unit.trim();
    if (!unit) fields.add("unit");

    if (!sensor.type.trim()) fields.add("type");

    if (!Number.isFinite(sensor.channel) || sensor.channel < 0 || !Number.isInteger(sensor.channel)) {
      fields.add("channel");
    }

    if (!Number.isFinite(sensor.interval_seconds) || sensor.interval_seconds < 0) {
      fields.add("interval_seconds");
    }

    if (!Number.isFinite(sensor.rolling_average_seconds) || sensor.rolling_average_seconds < 0) {
      fields.add("rolling_average_seconds");
    }

    const sensorId = sensor.sensor_id.trim();
    if (sensorId && /\s/.test(sensorId)) {
      fields.add("sensor_id");
    }

    if (sensorId) {
      const current = seen.get(sensorId) ?? [];
      current.push(sensor.key);
      seen.set(sensorId, current);
    }

    if (fields.size) {
      invalid.set(sensor.key, fields);
    }

    if (
      sensor.current_loop_shunt_ohms !== null ||
      sensor.current_loop_range_m !== null
    ) {
      const shunt = sensor.current_loop_shunt_ohms ?? NaN;
      const range = sensor.current_loop_range_m ?? NaN;
      if (!sensor.preset.startsWith("water_level")) {
        fields.add("current_loop_shunt_ohms");
        fields.add("current_loop_range_m");
      } else {
        if (!Number.isFinite(shunt) || shunt <= 0) fields.add("current_loop_shunt_ohms");
        if (!Number.isFinite(range) || range <= 0) fields.add("current_loop_range_m");
        if (sensor.current_loop_shunt_ohms === null) fields.add("current_loop_shunt_ohms");
        if (sensor.current_loop_range_m === null) fields.add("current_loop_range_m");
      }
      if (fields.size) {
        invalid.set(sensor.key, fields);
      }
    }
  }

  for (const [, keys] of seen.entries()) {
    if (keys.length < 2) continue;
    for (const key of keys) {
      const set = invalid.get(key) ?? new Set<SensorFieldKey>();
      set.add("sensor_id");
      invalid.set(key, set);
    }
  }

  return invalid;
}

function toApiDraft(sensor: SensorDraft): NodeSensorDraft {
  return {
    preset: sensor.preset,
    sensor_id: sensor.sensor_id,
    name: sensor.name,
    type: sensor.type,
    channel: sensor.channel,
    unit: sensor.unit,
    location: sensor.location,
    interval_seconds: sensor.interval_seconds,
    rolling_average_seconds: sensor.rolling_average_seconds,
    input_min: sensor.input_min,
    input_max: sensor.input_max,
    output_min: sensor.output_min,
    output_max: sensor.output_max,
    offset: sensor.offset,
    scale: sensor.scale,
    pulses_per_unit: sensor.pulses_per_unit,
    current_loop_shunt_ohms: sensor.current_loop_shunt_ohms,
    current_loop_range_m: sensor.current_loop_range_m,
  };
}

export default function NodeSensorsConfigSection({
  nodeId,
  canEdit,
  openByDefault = false,
  initialAction = "edit",
  variant = "default",
}: {
  nodeId: string;
  canEdit: boolean;
  openByDefault?: boolean;
  initialAction?: "add" | "edit";
  variant?: "default" | "drawer";
}) {
  const queryClient = useQueryClient();
  const sensorCounter = useRef(1);
  const [open, setOpen] = useState(openByDefault || variant === "drawer");
  const [loaded, setLoaded] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [expandedSensorKey, setExpandedSensorKey] = useState<string | null>(null);
  const [sensors, setSensors] = useState<SensorDraft[]>([]);
  const [ads1263, setAds1263] = useState<NodeAds1263SettingsDraft>(DEFAULT_ADS1263);
  const [analogBackend, setAnalogBackend] = useState<string | null>(null);
  const [analogHealth, setAnalogHealth] = useState<NodeAnalogHealth | null>(null);

  useEffect(() => {
    setLoaded(false);
  }, [nodeId]);

  useEffect(() => {
    if (!open) return;
    if (loaded) return;
    let cancelled = false;
    setLoading(true);
    setError(null);
    setNotice(null);
    getNodeSensorsConfig(nodeId)
      .then((payload) => {
        if (cancelled) return;
        const drafts = normalizeDrafts(payload.sensors ?? []);
        const nextAds1263 = payload.ads1263 ?? DEFAULT_ADS1263;
        setAds1263({ ...DEFAULT_ADS1263, ...nextAds1263 });
        setAnalogBackend(payload.analog_backend ?? null);
        setAnalogHealth(payload.analog_health ?? null);

        sensorCounter.current = drafts.length + 1;
        let nextDrafts = drafts;
        let nextExpanded = drafts.length ? drafts[0].key : null;

        if (initialAction === "add") {
          const key = `sensor-${sensorCounter.current++}`;
          const newSensor = buildNewSensor(key, drafts.length + 1);
          nextDrafts = [...drafts, newSensor];
          nextExpanded = key;
        }

        sensorCounter.current = nextDrafts.length + 1;
        setSensors(nextDrafts);
        setExpandedSensorKey(nextExpanded);
        setLoaded(true);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : "Failed to load node sensor config.");
      })
      .finally(() => {
        if (cancelled) return;
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [initialAction, loaded, nodeId, open]);

  const invalidSensorFields = useMemo(() => validateSensors(sensors), [sensors]);
  const canApply = useMemo(() => invalidSensorFields.size === 0, [invalidSensorFields.size]);
  const analogHealthSummary = useMemo(
    () => summarizeAnalogHealth(analogBackend, analogHealth),
    [analogBackend, analogHealth],
  );
  const hasAnalogSensors = useMemo(
    () => sensors.some((sensor) => sensor.type === "analog"),
    [sensors],
  );
  const actionButtonSize = variant === "drawer" ? "sm" : "xs";
  const editorOpen = open;

  const addSensor = () => {
    const key = `sensor-${sensorCounter.current++}`;
    setSensors((current) => [...current, buildNewSensor(key, current.length + 1)]);
    setExpandedSensorKey(key);
    setNotice(null);
  };

  const updateSensor = (key: string, patch: Partial<SensorDraft>) => {
    setSensors((current) => current.map((sensor) => (sensor.key === key ? { ...sensor, ...patch } : sensor)));
    setNotice(null);
  };

  const removeSensor = (key: string) => {
    setSensors((current) => current.filter((sensor) => sensor.key !== key));
    setExpandedSensorKey((current) => (current === key ? null : current));
    setNotice(null);
  };

  const applyPreset = (key: string, preset: SensorPresetKey) => {
    setSensors((current) =>
      current.map((sensor) => {
        if (sensor.key !== key) return sensor;
        const config = SENSOR_PRESETS[preset];
        if (!config || preset === "custom") {
          return { ...sensor, preset };
        }
        const shouldOverwriteName = !sensor.name.trim() || sensor.name.startsWith("Sensor ");
        return {
          ...sensor,
          preset,
          type: config.driver,
          unit: config.unit,
          interval_seconds: config.interval_seconds,
          rolling_average_seconds: config.rolling_average_seconds,
          input_min: config.input_min ?? sensor.input_min,
          input_max: config.input_max ?? sensor.input_max,
          output_min: config.output_min ?? sensor.output_min,
          output_max: config.output_max ?? sensor.output_max,
          offset: config.offset ?? sensor.offset,
          scale: config.scale ?? sensor.scale,
          pulses_per_unit: config.pulses_per_unit ?? sensor.pulses_per_unit,
          current_loop_shunt_ohms:
            config.current_loop_shunt_ohms ?? sensor.current_loop_shunt_ohms,
          current_loop_range_m:
            config.current_loop_range_m ?? sensor.current_loop_range_m,
          name: shouldOverwriteName ? config.defaultName : sensor.name,
        };
      }),
    );
    setNotice(null);
  };

  const generateSensorId = (key: string) => {
    updateSensor(key, { sensor_id: randomHex(12) });
  };

  const handleApply = async () => {
    if (!canEdit) return;
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      const payload = sensors.map(toApiDraft);
      const result = await updateNodeSensorsConfig(nodeId, payload, ads1263);
      const updated = normalizeDrafts(result.sensors ?? []);
      setSensors((current) =>
        updated.map((next, index) => ({
          ...next,
          key: current[index]?.key ?? next.key,
        })),
      );
      setExpandedSensorKey((currentKey) => currentKey ?? (updated[0]?.key ?? null));
      if (result.warning) {
        setNotice(result.warning);
      } else if (result.status === "applied") {
        setNotice("Sensor config applied to node.");
      } else {
        setNotice("Sensor config saved (node offline).");
      }

      // Read back after apply so ADC health + stored config are verified (and so users see
      // backend health updates without closing/reopening the editor).
      try {
        const refreshed = await getNodeSensorsConfig(nodeId);
        setAnalogBackend(refreshed.analog_backend ?? null);
        setAnalogHealth(refreshed.analog_health ?? null);
        setAds1263({ ...DEFAULT_ADS1263, ...(refreshed.ads1263 ?? DEFAULT_ADS1263) });
      } catch {
        // Ignore readback errors; the apply response already surfaced the actionable status.
      }

      await queryClient.invalidateQueries({ queryKey: queryKeys.sensors });
      await queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save node sensor config.");
    } finally {
      setSaving(false);
    }
  };

  const description =
    variant === "drawer" ? (
      <div className="space-y-1">
        <p>Configure physical sensors connected to the node. Apply to push updates to the node-agent.</p>
        <p>
          Tip: leave <span className="font-semibold">Sensor ID</span> blank to auto-generate a stable identifier.
        </p>
        {!canEdit ? (
          <p>
            Read-only: requires <code className="px-1">config.write</code>.
          </p>
        ) : null}
      </div>
    ) : (
      <div className="space-y-1">
        <p>Configure sensors that run on this node. Saving updates core’s sensor registry and pushes the list to the node-agent.</p>
        <p>
          Quick start: ensure the node is online → add a sensor (choose a preset when possible) → click “Apply to node”
          → verify values in Sensors &amp; Outputs / Trends.
        </p>
        <p>
          Tip: leave Sensor ID blank to auto-generate a stable identifier.
        </p>
        {!canEdit ? (
          <p>
            Read-only: requires <code className="px-1">config.write</code>.
          </p>
        ) : null}
      </div>
    );

  return (
    <CollapsibleCard
      title="Hardware sensors"
      description={description}
      density={variant === "drawer" ? "sm" : "md"}
      open={open}
      onOpenChange={setOpen}
      bodyClassName="space-y-4"
      actions={
        editorOpen ? (
          <>
            <NodeButton
              size={actionButtonSize}
              onClick={() => {
                if (!loaded) return;
                addSensor();
              }}
              disabled={!canEdit || saving || loading || !loaded}
            >
              Add sensor
            </NodeButton>
            <NodeButton
              size={actionButtonSize}
              variant="primary"
              onClick={() => void handleApply()}
              disabled={!canEdit || saving || loading || !loaded || !canApply}
            >
              {saving ? "Applying…" : "Apply to node"}
            </NodeButton>
          </>
        ) : null
      }
    >
      {error ? (
        <InlineBanner tone="error" className="px-3 py-2 text-sm">
          {error}
        </InlineBanner>
      ) : null}

      {notice ? (
        <InlineBanner tone="warning" className="px-3 py-2 text-sm">
          {notice}
        </InlineBanner>
      ) : null}

      {editorOpen ? (
        loading && !loaded ? (
 <p className="text-sm text-muted-foreground">Loading node sensor config…</p>
        ) : (
          <>
            {!canApply ? (
 <p className="text-xs text-rose-700">
                Fix highlighted sensor fields before applying.
              </p>
            ) : null}

            <div className="space-y-3">
              {sensors.map((sensor) => {
                const isExpanded = expandedSensorKey === sensor.key;
                const invalid = invalidSensorFields.get(sensor.key) ?? new Set<SensorFieldKey>();
                return (
                  <SensorCard
                    key={sensor.key}
                    sensor={sensor}
                    invalidFields={invalid}
                    isExpanded={isExpanded}
                    onToggleExpanded={() => setExpandedSensorKey(isExpanded ? null : sensor.key)}
                    onUpdateSensor={(patch) => updateSensor(sensor.key, patch)}
                    onApplyPreset={(preset) => applyPreset(sensor.key, preset)}
                    onGenerateSensorId={() => generateSensorId(sensor.key)}
                    onRemove={() => removeSensor(sensor.key)}
                    onDone={() => setExpandedSensorKey(null)}
                  />
                );
              })}

              {!sensors.length ? (
                <Card className="border-dashed gap-0 bg-card-inset px-4 py-6 text-center text-sm text-muted-foreground">
                  No sensors added yet. Click &quot;Add sensor&quot; to start building the node configuration.
                </Card>
              ) : null}
            </div>

            <CollapsibleCard
              key={`adc-${ads1263.enabled ? "on" : "off"}-${hasAnalogSensors ? "analog" : "no"}`}
              density="sm"
              title="ADC hat (optional)"
              description={
                <>
                  Waveshare High-Precision AD HAT (ADS1263). Enables analog channels 0–9. Requires SPI0 (
                  <code className="px-1">dtparam=spi=on</code>).
                </>
              }
              defaultOpen={Boolean(ads1263.enabled) || hasAnalogSensors}
 className="bg-card-inset"
              bodyClassName="space-y-3"
              actions={
 <label className="inline-flex items-center gap-2 text-xs font-semibold text-foreground">
                  <input
                    type="checkbox"
                    checked={Boolean(ads1263.enabled)}
                    onChange={(event) =>
                      setAds1263((current) => ({ ...current, enabled: event.target.checked }))
                    }
 className="h-4 w-4 rounded border-input text-indigo-600 focus:ring-indigo-500"
                  />
                  Enabled
                </label>
              }
            >
              <div className="space-y-1">
 <p className="text-xs text-muted-foreground">
                  Backend: <span className="font-semibold">{analogBackend ?? "unknown"}</span>{" "}
 <span className="text-muted-foreground">·</span>{" "}
                  <span
                    className={
                      analogHealthSummary.tone === "ok"
 ? "font-semibold text-emerald-700"
 : "font-semibold text-amber-700"
                    }
                  >
                    {analogHealthSummary.label}
                  </span>
                </p>
                {hasAnalogSensors && !analogHealth?.ok ? (
 <p className="text-xs text-amber-700">
                    No analog data until ADS1263 is healthy (production is fail-closed).
                  </p>
                ) : null}
                {analogHealthSummary.tone !== "ok" && analogHealthSummary.detail ? (
 <p className="text-xs text-amber-700">
                    {analogHealthSummary.detail}
                  </p>
                ) : null}
              </div>

              <details>
 <summary className="cursor-pointer select-none text-sm font-semibold text-foreground">
                  Advanced
                </summary>
                <div className="mt-3 grid gap-4 md:grid-cols-2">
                  <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Data rate
                    </label>
                    <Select
                      value={ads1263.data_rate ?? "ADS1263_100SPS"}
                      onChange={(event) =>
                        setAds1263((current) => ({ ...current, data_rate: event.target.value }))
                      }
                      className="mt-1"
                    >
                      <option value="ADS1263_100SPS">100 SPS</option>
                      <option value="ADS1263_60SPS">60 SPS</option>
                      <option value="ADS1263_50SPS">50 SPS</option>
                      <option value="ADS1263_20SPS">20 SPS</option>
                      <option value="ADS1263_10SPS">10 SPS</option>
                    </Select>
 <p className="mt-1 text-xs text-muted-foreground">
                      For multiple channels, use ≥50 SPS (recommend 100 SPS).
                    </p>
                  </div>

                  <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      Scan interval (s)
                    </label>
                    <NumericDraftInput
                      value={ads1263.scan_interval_seconds ?? null}
                      onValueChange={(next) =>
                        setAds1263((current) => ({
                          ...current,
                          scan_interval_seconds: typeof next === "number" ? next : null,
                        }))
                      }
 className="mt-1 w-full rounded-lg border border-border bg-white px-3 py-2 text-sm text-foreground shadow-xs focus:outline-hidden focus:ring-2 focus:ring-indigo-500"
                      placeholder="0.25"
                      inputMode="decimal"
                      autoComplete="off"
                    />
 <p className="mt-1 text-xs text-muted-foreground">
                      How often the node samples the ADS1263 in the background.
                    </p>
                  </div>
                </div>
              </details>
            </CollapsibleCard>
          </>
        )
      ) : null}
    </CollapsibleCard>
  );
}
