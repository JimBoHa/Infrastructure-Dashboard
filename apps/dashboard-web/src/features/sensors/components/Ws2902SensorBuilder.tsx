"use client";

import { useMemo, useState } from "react";
import { useQueryClient, useQuery } from "@tanstack/react-query";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { getWs2902IntegrationStatusByNode } from "@/lib/api";
import { postJson } from "@/lib/http";
import { queryKeys } from "@/lib/queries";
import { configString } from "@/lib/sensorOrigin";
import type { DemoSensor } from "@/types/dashboard";

type Message = { type: "success" | "error"; text: string };

type LastPayloadEntry = {
  key: string;
  raw: string;
  numeric: number | null;
};

const WS_SOIL_MOISTURE_FIELDS = Array.from({ length: 8 }, (_, idx) => `soilmoisture${idx + 1}`);

function parseFloatLoose(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeLastPayload(lastPayload: Record<string, unknown> | null | undefined): LastPayloadEntry[] {
  if (!lastPayload) return [];
  return Object.entries(lastPayload)
    .map(([key, value]) => {
      const raw = typeof value === "string" ? value : value == null ? "" : String(value);
      return { key, raw, numeric: parseFloatLoose(value) };
    })
    .sort((a, b) => a.key.localeCompare(b.key));
}

function configNumber(config: Record<string, unknown> | null | undefined, key: string): number | null {
  if (!config) return null;
  const value = config[key];
  return parseFloatLoose(value);
}

export default function Ws2902SensorBuilder({
  nodeId,
  sensors,
  canEdit,
}: {
  nodeId: string;
  sensors: DemoSensor[];
  canEdit: boolean;
}) {
  const queryClient = useQueryClient();
  const [message, setMessage] = useState<Message | null>(null);
  const [busy, setBusy] = useState(false);

  const statusQuery = useQuery({
    queryKey: ["weather-stations", "ws2902", "status", nodeId],
    queryFn: () => getWs2902IntegrationStatusByNode(nodeId),
    enabled: Boolean(nodeId),
    staleTime: 10_000,
    refetchInterval: 30_000,
  });

  const existingWsFieldSensors = useMemo(() => {
    const list = sensors.filter((sensor) => {
      if (sensor.node_id !== nodeId) return false;
      const config = (sensor.config ?? {}) as Record<string, unknown>;
      if (configString(config, "source") !== "ws_2902") return false;
      const field = configString(config, "ws_field");
      return Boolean(field && field.trim().length);
    });
    return list.slice().sort((a, b) => a.name.localeCompare(b.name));
  }, [nodeId, sensors]);

  const usedFields = useMemo(() => {
    const set = new Set<string>();
    existingWsFieldSensors.forEach((sensor) => {
      const field = configString((sensor.config ?? {}) as Record<string, unknown>, "ws_field");
      if (field) set.add(field.trim());
    });
    return set;
  }, [existingWsFieldSensors]);

  const lastPayloadEntries = useMemo(() => {
    return normalizeLastPayload(statusQuery.data?.last_payload);
  }, [statusQuery.data?.last_payload]);

  const suggestedFields = useMemo(() => {
    const numericKeys = lastPayloadEntries.filter((entry) => entry.numeric != null).map((entry) => entry.key);
    const soilKeys = numericKeys.filter((key) => key.toLowerCase().startsWith("soilmoisture"));
    const filtered = soilKeys.length ? soilKeys : numericKeys;
    return filtered.filter((key) => !usedFields.has(key)).slice(0, 30);
  }, [lastPayloadEntries, usedFields]);

  const defaultIntervalSeconds = useMemo(() => {
    const sample = sensors.find((sensor) => sensor.node_id === nodeId);
    return sample?.interval_seconds ?? 30;
  }, [nodeId, sensors]);

  const [preset, setPreset] = useState<"soil_moisture" | "custom">("soil_moisture");
  const [name, setName] = useState("Soil moisture");
  const [type, setType] = useState("moisture");
  const [unit, setUnit] = useState("%");
  const [intervalSeconds, setIntervalSeconds] = useState<number | null>(defaultIntervalSeconds);
  const [wsField, setWsField] = useState<string>(WS_SOIL_MOISTURE_FIELDS[0]);
  const [wsScale, setWsScale] = useState<number | null>(1);
  const [wsOffset, setWsOffset] = useState<number | null>(0);
  const [wsMin, setWsMin] = useState<number | null>(0);
  const [wsMax, setWsMax] = useState<number | null>(100);

  const presetOptions = [
    { value: "soil_moisture", label: "Soil moisture" },
    { value: "custom", label: "Custom" },
  ] as const;

  const applyPreset = (next: typeof preset) => {
    setPreset(next);
    setMessage(null);
    if (next === "soil_moisture") {
      setName("Soil moisture");
      setType("moisture");
      setUnit("%");
      setWsField((current) => current || WS_SOIL_MOISTURE_FIELDS[0]);
      setWsScale(1);
      setWsOffset(0);
      setWsMin(0);
      setWsMax(100);
    }
  };

  const intervalValue = intervalSeconds ?? NaN;
  const scaleValue = wsScale ?? NaN;
  const offsetValue = wsOffset ?? NaN;
  const minValue = wsMin ?? NaN;
  const maxValue = wsMax ?? NaN;

  const invalid =
    !name.trim().length ||
    !type.trim().length ||
    !unit.trim().length ||
    !wsField.trim().length ||
    !Number.isFinite(intervalValue) ||
    intervalValue < 0 ||
    (wsScale != null && (!Number.isFinite(scaleValue) || scaleValue === 0)) ||
    (wsOffset != null && !Number.isFinite(offsetValue)) ||
    (wsMin != null && !Number.isFinite(minValue)) ||
    (wsMax != null && !Number.isFinite(maxValue)) ||
    (wsMin != null && wsMax != null && Number.isFinite(minValue) && Number.isFinite(maxValue) && minValue > maxValue);

  const canSubmit = canEdit && !busy && !invalid;

  const create = async () => {
    if (!canSubmit) return;
    setBusy(true);
    setMessage(null);
    try {
      const config: Record<string, unknown> = {
        source: "ws_2902",
        ws_field: wsField.trim(),
      };
      if (wsScale != null) config.ws_scale = wsScale;
      if (wsOffset != null) config.ws_offset = wsOffset;
      if (wsMin != null) config.ws_min = wsMin;
      if (wsMax != null) config.ws_max = wsMax;

      const payload = {
        node_id: nodeId,
        name: name.trim(),
        type: type.trim(),
        unit: unit.trim(),
        interval_seconds: Math.floor(intervalValue),
        rolling_avg_seconds: 0,
        config,
      };

      const created = (await postJson<unknown>("/api/sensors", payload)) as { sensor_id?: string };
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
        queryClient.invalidateQueries({ queryKey: queryKeys.analytics }),
      ]);

      const sensorId = typeof created?.sensor_id === "string" ? created.sensor_id : null;
      setMessage({
        type: "success",
        text: sensorId
          ? `Created sensor (${sensorId}). Waiting for next station upload…`
          : "Created sensor. Waiting for next station upload…",
      });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Failed to create weather station sensor.",
      });
    } finally {
      setBusy(false);
    }
  };

  const showSuggestedFields = Boolean(suggestedFields.length);
  const lastSeen = statusQuery.data?.last_seen ?? null;
  const missingFields = statusQuery.data?.last_missing_fields ?? [];

  return (
    <div className="space-y-6">
      {message ? (
        <InlineBanner tone={message.type === "success" ? "success" : "danger"}>{message.text}</InlineBanner>
      ) : null}

      <CollapsibleCard
        title="WS‑2902 sensors"
        description="Create sensors that map directly to fields in the weather station upload payload."
        defaultOpen
        actions={<SensorOriginBadge sensor={{ config: { source: "ws_2902" } }} size="xs" />}
      >
        <div className="space-y-5">
          <Card className="rounded-lg gap-0 bg-card-inset p-4 text-sm">
 <p className="font-semibold text-foreground">Station status</p>
 <div className="mt-2 grid gap-2 text-xs text-foreground md:grid-cols-2">
              <p>Last upload: {lastSeen ?? (statusQuery.isLoading ? "Loading…" : "—")}</p>
              <p>Missing fields: {missingFields.length ? missingFields.join(", ") : "—"}</p>
            </div>
 <p className="mt-2 text-xs text-muted-foreground">
              Tip: If your new sensor isn&apos;t showing up yet, wait for the station to upload once with that sensor enabled.
            </p>
          </Card>

          <div className="grid gap-4 md:grid-cols-2">
            <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Preset
              </span>
              <Select
                value={preset}
                onChange={(e) => applyPreset(e.target.value as typeof preset)}
              >
                {presetOptions.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </Select>
            </label>

            <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Interval (seconds)
              </span>
              <NumericDraftInput
                value={intervalSeconds}
                onValueChange={(next) => setIntervalSeconds(next ?? null)}
                placeholder={String(defaultIntervalSeconds)}
              />
            </label>
          </div>

          <div className="grid gap-4 md:grid-cols-2">
            <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Sensor name
              </span>
              <Input
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Soil moisture"
              />
            </label>

            <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Upload field
              </span>
              <Input
                value={wsField}
                onChange={(e) => setWsField(e.target.value)}
                placeholder="soilmoisture1"
              />
              {showSuggestedFields ? (
                <div className="mt-2 flex flex-wrap gap-2">
                  {suggestedFields.map((key) => (
                    <button
                      key={key}
                      type="button"
 className="rounded-full bg-white px-2 py-1 text-xs font-semibold text-foreground shadow-xs ring-1 ring-ring hover:bg-muted"
                      onClick={() => setWsField(key)}
                      title={`Detected in last upload: ${key}`}
                    >
                      {key}
                    </button>
                  ))}
                </div>
              ) : null}
 <p className="mt-2 text-xs text-muted-foreground">
                Common soil moisture keys: {WS_SOIL_MOISTURE_FIELDS.join(", ")}.
              </p>
            </label>
          </div>

          <CollapsibleCard
            density="sm"
            title="Advanced"
            description="Type/unit and optional value transforms (scale/offset/min/max)."
          >
            <div className="space-y-4">
              <div className="grid gap-4 md:grid-cols-2">
                <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Type
                  </span>
                  <Input
                    value={type}
                    onChange={(e) => setType(e.target.value)}
                    placeholder="moisture"
                  />
                </label>
                <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Unit
                  </span>
                  <Input
                    value={unit}
                    onChange={(e) => setUnit(e.target.value)}
                    placeholder="%"
                  />
                </label>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Scale
                  </span>
                  <NumericDraftInput value={wsScale} onValueChange={(next) => setWsScale(next ?? null)} placeholder="1" />
 <p className="text-xs text-muted-foreground">
                    Stored value = (raw × scale) + offset.
                  </p>
                </label>
                <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Offset
                  </span>
                  <NumericDraftInput value={wsOffset} onValueChange={(next) => setWsOffset(next ?? null)} placeholder="0" />
                </label>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Min (optional)
                  </span>
                  <NumericDraftInput value={wsMin} onValueChange={(next) => setWsMin(next ?? null)} placeholder="(none)" />
                </label>
                <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Max (optional)
                  </span>
                  <NumericDraftInput value={wsMax} onValueChange={(next) => setWsMax(next ?? null)} placeholder="(none)" />
 <p className="text-xs text-muted-foreground">
                    Values outside min/max are ignored (not stored).
                  </p>
                </label>
              </div>

              {existingWsFieldSensors.length ? (
                <Card className="rounded-lg gap-0 bg-card-inset p-3 text-xs text-card-foreground">
                  <p className="font-semibold">Existing WS field sensors</p>
                  <ul className="mt-2 list-disc space-y-1 pl-5">
                    {existingWsFieldSensors.map((sensor) => {
                      const config = (sensor.config ?? {}) as Record<string, unknown>;
                      const field = configString(config, "ws_field") ?? "(unknown)";
                      const scale = configNumber(config, "ws_scale");
                      const offset = configNumber(config, "ws_offset");
                      return (
                        <li key={sensor.sensor_id}>
                          {sensor.name}{" "}
 <span className="text-muted-foreground">
                            ({field}
                            {scale != null ? `, scale=${scale}` : ""}
                            {offset != null ? `, offset=${offset}` : ""}
                            )
                          </span>
                        </li>
                      );
                    })}
                  </ul>
                </Card>
              ) : null}
            </div>
          </CollapsibleCard>

          <div className="flex items-center justify-end gap-2">
            <NodeButton onClick={() => void statusQuery.refetch()} disabled={statusQuery.isFetching}>
              Refresh detected fields
            </NodeButton>
            <NodeButton
              variant="primary"
              onClick={() => void create()}
              disabled={!canSubmit}
            >
              {busy ? "Creating…" : "Create sensor"}
            </NodeButton>
          </div>

          {statusQuery.error ? (
            <p className="text-sm text-rose-600">
              Failed to load station status:{" "}
              {(statusQuery.error instanceof Error && statusQuery.error.message) || "Unknown error"}
            </p>
          ) : null}
        </div>
      </CollapsibleCard>
    </div>
  );
}
