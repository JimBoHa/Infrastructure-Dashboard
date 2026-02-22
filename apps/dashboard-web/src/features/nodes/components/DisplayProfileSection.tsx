"use client";

import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import CollapsibleCard from "@/components/CollapsibleCard";
import { getNodeDisplayProfile, updateNodeDisplayProfile } from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import { sha256Hex } from "@/lib/sha256";
import type { NodeDisplayProfile } from "@/lib/apiSchemas";
import { Card } from "@/components/ui/card";
import InlineBanner from "@/components/InlineBanner";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

const defaultProfile = (): NodeDisplayProfile => ({
  schema_version: 1,
  enabled: false,
  kiosk_autostart: false,
  ui_refresh_seconds: 2,
  latency_sample_seconds: 10,
  latency_window_samples: 12,
  tiles: [],
  outputs_enabled: false,
  local_pin_hash: null,
  trend_ranges: ["1h", "6h", "24h"],
  trends: [],
  core_api_base_url: null,
});

type TileType = NodeDisplayProfile["tiles"][number]["type"];

const BASE_TILES: Array<{ type: Exclude<TileType, "sensor">; label: string }> = [
  { type: "core_status", label: "Core status" },
  { type: "latency", label: "Latency / jitter" },
  { type: "sensors", label: "Sensors table" },
  { type: "trends", label: "Trends page" },
  { type: "outputs", label: "Outputs page" },
];

export default function DisplayProfileSection({
  node,
  sensors,
  ipLast,
}: {
  node: DemoNode;
  sensors: DemoSensor[];
  ipLast: string;
}) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(false);
  const [profile, setProfile] = useState<NodeDisplayProfile | null>(null);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [pinPlaintext, setPinPlaintext] = useState("");
  const [clearPin, setClearPin] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);

  const nodeSensors = useMemo(() => sensors.filter((s) => s.node_id === node.id), [node.id, sensors]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    setNotice(null);
    getNodeDisplayProfile(node.id)
      .then((payload) => {
        if (cancelled) return;
        setProfile({ ...defaultProfile(), ...payload });
      })
      .catch((err) => {
        if (cancelled) return;
        const message = err instanceof Error ? err.message : "Failed to load display profile.";
        setError(message);
        setProfile(defaultProfile());
      })
      .finally(() => {
        if (cancelled) return;
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [node.id]);

  const currentTiles = useMemo(() => {
    const tiles = profile?.tiles ?? [];
    return new Set(tiles.map((tile) => tile.type));
  }, [profile?.tiles]);

  const spotlightSensors = useMemo(() => {
    const tiles = profile?.tiles ?? [];
    return new Set(
      tiles
        .filter((tile) => tile.type === "sensor")
        .map((tile) => tile.sensor_id)
        .filter((value): value is string => typeof value === "string" && value.length > 0),
    );
  }, [profile?.tiles]);

  const trendSensorIds = useMemo(() => new Set((profile?.trends ?? []).map((t) => t.sensor_id)), [profile?.trends]);

  const setTileEnabled = (type: TileType, enabled: boolean, sensorId?: string) => {
    setProfile((prev) => {
      if (!prev) return prev;
      const tiles = [...(prev.tiles ?? [])];
      const existsIndex = tiles.findIndex((tile) => {
        if (tile.type !== type) return false;
        if (type === "sensor") return tile.sensor_id === sensorId;
        return true;
      });
      if (enabled && existsIndex === -1) {
        tiles.push(type === "sensor" ? { type, sensor_id: sensorId } : { type });
      }
      if (!enabled && existsIndex !== -1) {
        tiles.splice(existsIndex, 1);
      }
      return { ...prev, tiles };
    });
  };

  const setTrendEnabled = (sensorId: string, enabled: boolean) => {
    setProfile((prev) => {
      if (!prev) return prev;
      const existing = prev.trends ?? [];
      const has = existing.some((t) => t.sensor_id === sensorId);
      if (enabled && !has) {
        return {
          ...prev,
          trends: [...existing, { sensor_id: sensorId, default_range: "6h" }],
        };
      }
      if (!enabled && has) {
        return {
          ...prev,
          trends: existing.filter((t) => t.sensor_id !== sensorId),
        };
      }
      return prev;
    });
  };

  const displayUrl =
    ipLast && ipLast !== "-" ? `http://${ipLast}:9000/display` : null;

  const handleSave = async () => {
    if (!profile) return;
    setSaving(true);
    setError(null);
    setNotice(null);
    try {
      let localPinHash = profile.local_pin_hash ?? null;
      if (clearPin) {
        localPinHash = null;
      } else if (pinPlaintext.trim()) {
        localPinHash = await sha256Hex(pinPlaintext.trim());
      }
      const payload: NodeDisplayProfile = {
        ...profile,
        local_pin_hash: localPinHash,
      };
      const result = await updateNodeDisplayProfile(node.id, payload);
      setProfile({ ...defaultProfile(), ...result.display });
      setPinPlaintext("");
      setClearPin(false);
      if (result.warning) {
        setNotice(result.warning);
      } else {
        setNotice("Display profile saved.");
      }
      void queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save display profile.");
    } finally {
      setSaving(false);
    }
  };

  return (
    <CollapsibleCard
      title="Local display"
      description="Configure the node kiosk display (what it shows, refresh rate, and optional PIN)."
      defaultOpen={false}
      open={open}
      onOpenChange={setOpen}
      bodyClassName="space-y-3"
      actions={
        open ? (
          <>
            <NodeButton onClick={() => setShowAdvanced((v) => !v)} size="xs">
              {showAdvanced ? "Hide advanced" : "Show advanced"}
            </NodeButton>
            <NodeButton onClick={handleSave} size="xs" disabled={saving || loading || !profile}>
              {saving ? "Saving…" : "Save"}
            </NodeButton>
          </>
        ) : null
      }
    >
      {open ? (
        <>
          {displayUrl && (
 <p className="text-xs text-muted-foreground">
              Display URL:{" "}
              <a
 className="font-semibold text-indigo-600 underline hover:text-indigo-500"
                href={displayUrl}
                target="_blank"
                rel="noreferrer"
              >
                {displayUrl}
              </a>
            </p>
          )}

 {loading && <p className="text-sm text-muted-foreground">Loading display profile…</p>}
          {error && (
            <InlineBanner tone="error" className="px-3 py-2 text-sm">
              {error}
            </InlineBanner>
          )}
          {notice && (
            <InlineBanner tone="warning" className="px-3 py-2 text-sm">
              {notice}
            </InlineBanner>
          )}

          {profile ? (
            <Card className="space-y-4 gap-0 bg-card-inset p-4">
              <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
 <label className="flex items-center gap-2 text-sm text-foreground">
                  <input
                    type="checkbox"
                    checked={Boolean(profile.enabled)}
                    onChange={(ev) => setProfile({ ...profile, enabled: ev.target.checked })}
 className="shrink-0 rounded border-input text-indigo-600 focus:ring-indigo-500"
                  />
                  Enable kiosk display on this node
                </label>
                <div className="flex items-center gap-2">
 <span className="text-xs text-muted-foreground">Refresh (s)</span>
                  <NumericDraftInput
 className="w-20 rounded-lg border border-border bg-white px-2 py-1 text-sm text-foreground focus:border-indigo-500 focus:ring-indigo-500"
                    value={profile.ui_refresh_seconds}
                    onValueChange={(next) => {
                      if (typeof next === "number") {
                        setProfile((current) =>
                          current ? { ...current, ui_refresh_seconds: next } : current,
                        );
                      }
                    }}
                    emptyBehavior="keep"
                    min={1}
                    max={60}
                    integer
                    inputMode="numeric"
                    enforceRange
                    clampOnBlur
                  />
                </div>
              </div>

 <div className="grid grid-cols-2 gap-3 text-sm text-foreground">
            <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Latency probe (s)</p>
              <NumericDraftInput
 className="mt-1 w-full rounded-lg border border-border bg-white px-2 py-1 text-sm text-foreground focus:border-indigo-500 focus:ring-indigo-500"
                value={profile.latency_sample_seconds}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    setProfile((current) =>
                      current ? { ...current, latency_sample_seconds: next } : current,
                    );
                  }
                }}
                emptyBehavior="keep"
                min={1}
                max={300}
                integer
                inputMode="numeric"
                enforceRange
                clampOnBlur
              />
            </div>
            <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Jitter window</p>
              <NumericDraftInput
 className="mt-1 w-full rounded-lg border border-border bg-white px-2 py-1 text-sm text-foreground focus:border-indigo-500 focus:ring-indigo-500"
                value={profile.latency_window_samples}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    setProfile((current) =>
                      current ? { ...current, latency_window_samples: next } : current,
                    );
                  }
                }}
                emptyBehavior="keep"
                min={3}
                max={120}
                integer
                inputMode="numeric"
                enforceRange
                clampOnBlur
              />
            </div>
          </div>

          <div className="space-y-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Tiles</p>
            <div className="grid grid-cols-2 gap-2">
              {BASE_TILES.map((tile) => (
 <label key={tile.type} className="flex items-center gap-2 text-sm text-foreground">
                  <input
                    type="checkbox"
                    checked={currentTiles.has(tile.type)}
                    onChange={(ev) => setTileEnabled(tile.type, ev.target.checked)}
 className="shrink-0 rounded border-input text-indigo-600 focus:ring-indigo-500"
                  />
                  {tile.label}
                </label>
              ))}
            </div>
          </div>

          <div className="space-y-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Spotlight sensors</p>
            {nodeSensors.length ? (
              <div className="grid grid-cols-2 gap-2">
                {nodeSensors.map((sensor) => (
 <label key={sensor.sensor_id} className="flex items-center gap-2 text-sm text-foreground">
                    <input
                      type="checkbox"
                      checked={spotlightSensors.has(sensor.sensor_id)}
                      onChange={(ev) =>
                        setTileEnabled("sensor", ev.target.checked, sensor.sensor_id)
                      }
 className="shrink-0 rounded border-input text-indigo-600 focus:ring-indigo-500"
                    />
                    {sensor.name}
                  </label>
                ))}
              </div>
            ) : (
 <p className="text-sm text-muted-foreground">No sensors available for this node.</p>
            )}
          </div>

          <div className="space-y-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Trend sensors</p>
            {nodeSensors.length ? (
              <div className="grid grid-cols-2 gap-2">
                {nodeSensors.map((sensor) => (
 <label key={sensor.sensor_id} className="flex items-center gap-2 text-sm text-foreground">
                    <input
                      type="checkbox"
                      checked={trendSensorIds.has(sensor.sensor_id)}
                      onChange={(ev) => setTrendEnabled(sensor.sensor_id, ev.target.checked)}
 className="shrink-0 rounded border-input text-indigo-600 focus:ring-indigo-500"
                    />
                    {sensor.name}
                  </label>
                ))}
              </div>
            ) : (
 <p className="text-sm text-muted-foreground">No sensors available for this node.</p>
            )}
          </div>

          <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
 <label className="flex items-center gap-2 text-sm text-foreground">
              <input
                type="checkbox"
                checked={Boolean(profile.outputs_enabled)}
                onChange={(ev) => setProfile({ ...profile, outputs_enabled: ev.target.checked })}
 className="shrink-0 rounded border-input text-indigo-600 focus:ring-indigo-500"
              />
              Enable output controls (advanced)
            </label>
 <span className="text-xs text-muted-foreground">
              Requires a bearer token on the display device.
            </span>
          </div>

          {showAdvanced && (
            <Card className="gap-0 space-y-3 p-3">
 <label className="flex items-center gap-2 text-sm text-foreground">
                <input
                  type="checkbox"
                  checked={Boolean(profile.kiosk_autostart)}
                  onChange={(ev) =>
                    setProfile({ ...profile, kiosk_autostart: ev.target.checked })
                  }
 className="shrink-0 rounded border-input text-indigo-600 focus:ring-indigo-500"
                />
                Auto-start kiosk at boot (image support required)
              </label>

              <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
                <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Core API base URL (optional)</p>
                  <Input
                    className="mt-1 px-2 py-1"
                    placeholder="http://controller.local:8000"
                    value={profile.core_api_base_url ?? ""}
                    onChange={(ev) =>
                      setProfile({ ...profile, core_api_base_url: ev.target.value || null })
                    }
                  />
                </div>

                <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Local display PIN (optional)</p>
                  <Input
                    className="mt-1 px-2 py-1"
                    type="password"
                    placeholder={profile.local_pin_hash ? "PIN is set (enter to rotate)" : "enter PIN"}
                    value={pinPlaintext}
                    onChange={(ev) => {
                      setPinPlaintext(ev.target.value);
                      if (ev.target.value) setClearPin(false);
                    }}
                  />
 <label className="mt-2 flex items-center gap-2 text-xs text-foreground">
                    <input
                      type="checkbox"
                      checked={clearPin}
                      onChange={(ev) => {
                        setClearPin(ev.target.checked);
                        if (ev.target.checked) setPinPlaintext("");
                      }}
 className="shrink-0 rounded border-input text-indigo-600 focus:ring-indigo-500"
                    />
                    Clear existing PIN
                  </label>
                </div>
              </div>
 <p className="text-xs text-muted-foreground">
                The PIN is stored as a SHA-256 hash in the node display profile.
              </p>
            </Card>
          )}
            </Card>
          ) : null}
        </>
      ) : null}
    </CollapsibleCard>
  );
}
