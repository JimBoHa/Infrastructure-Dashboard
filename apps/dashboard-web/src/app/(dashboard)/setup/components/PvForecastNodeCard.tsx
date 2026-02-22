"use client";

import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import NodeButton from "@/features/nodes/components/NodeButton";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import { Input } from "@/components/ui/input";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import { checkPvForecastPlane, fetchMapFeatures, pollForecasts, updatePvForecastConfig } from "@/lib/api";
import { queryKeys, usePvForecastConfigQuery } from "@/lib/queries";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import type { DemoNode } from "@/types/dashboard";

type Message = { type: "success" | "error"; text: string };

type PvCheckResult = {
  place: string | null;
  timezone: string | null;
  checked_at: string;
};

const readGeoJsonPoint = (
  geometry: unknown,
): { latitude: number; longitude: number } | null => {
  if (!geometry || typeof geometry !== "object") return null;
  const record = geometry as Record<string, unknown>;
  if (record.type !== "Point") return null;
  const coords = record.coordinates;
  if (!Array.isArray(coords) || coords.length < 2) return null;
  const lng = coords[0];
  const lat = coords[1];
  if (typeof lng !== "number" || typeof lat !== "number") return null;
  if (!Number.isFinite(lat) || !Number.isFinite(lng)) return null;
  return { latitude: lat, longitude: lng };
};

export default function PvForecastNodeCard({
  node,
  defaultOpen,
  weatherLocation,
}: {
  node: DemoNode;
  defaultOpen: boolean;
  weatherLocation: { latitude: string; longitude: string } | null;
}) {
  const queryClient = useQueryClient();
  const [open, setOpen] = useState(defaultOpen);
  const pvForecastConfigQuery = usePvForecastConfigQuery(node.id, { enabled: open });

  const [pvEnabled, setPvEnabled] = useState(false);
  const [pvLatitude, setPvLatitude] = useState("");
  const [pvLongitude, setPvLongitude] = useState("");
  const [pvTiltDeg, setPvTiltDeg] = useState(20);
  const [pvAzimuthDeg, setPvAzimuthDeg] = useState(0);
  const [pvKwp, setPvKwp] = useState("1.0");
  const [pvTimeFormat, setPvTimeFormat] = useState<"utc" | "iso8601">("utc");
  const [pvMessage, setPvMessage] = useState<Message | null>(null);
  const [pvCheck, setPvCheck] = useState<PvCheckResult | null>(null);
  const [pvBusy, setPvBusy] = useState<"saving" | "testing" | "locating" | null>(null);
  const [pvDirty, setPvDirty] = useState(false);
  const [pvLoaded, setPvLoaded] = useState(false);

  useEffect(() => {
    if (!open) return;
    const cfg = pvForecastConfigQuery.data;
    if (pvLoaded && pvDirty) return;
    if (!cfg) {
      setPvEnabled(false);
      setPvLatitude("");
      setPvLongitude("");
      setPvTiltDeg(20);
      setPvAzimuthDeg(0);
      setPvKwp("1.0");
      setPvTimeFormat("utc");
    } else {
      setPvEnabled(Boolean(cfg.enabled));
      setPvLatitude(String(cfg.latitude));
      setPvLongitude(String(cfg.longitude));
      setPvTiltDeg(cfg.tilt_deg);
      setPvAzimuthDeg(cfg.azimuth_deg);
      setPvKwp(String(cfg.kwp));
      setPvTimeFormat(cfg.time_format === "iso8601" ? "iso8601" : "utc");
    }
    setPvMessage(null);
    setPvCheck(null);
    setPvDirty(false);
    setPvLoaded(true);
  }, [open, pvDirty, pvForecastConfigQuery.data, pvLoaded]);

  const statusLabel = useMemo(
    () => formatNodeStatusLabel(node.status ?? "unknown", node.last_seen),
    [node.last_seen, node.status],
  );

  const parsePvInputs = () => {
    const latitude = Number.parseFloat(pvLatitude);
    const longitude = Number.parseFloat(pvLongitude);
    const kwp = Number.parseFloat(pvKwp);
    if (!Number.isFinite(latitude) || latitude < -90 || latitude > 90) {
      setPvMessage({ type: "error", text: "Latitude must be -90..90°." });
      return null;
    }
    if (!Number.isFinite(longitude) || longitude < -180 || longitude > 180) {
      setPvMessage({ type: "error", text: "Longitude must be -180..180°." });
      return null;
    }
    if (!Number.isFinite(kwp) || kwp <= 0) {
      setPvMessage({ type: "error", text: "Capacity (kWp) must be > 0." });
      return null;
    }
    if (pvTiltDeg < 0 || pvTiltDeg > 90) {
      setPvMessage({ type: "error", text: "Tilt must be 0..90°." });
      return null;
    }
    if (pvAzimuthDeg < -180 || pvAzimuthDeg > 180) {
      setPvMessage({ type: "error", text: "Azimuth must be -180..180°." });
      return null;
    }

    return { latitude, longitude, kwp };
  };

  const testPvForecastPlane = async () => {
    setPvMessage(null);
    setPvCheck(null);
    const parsed = parsePvInputs();
    if (!parsed) return;
    setPvBusy("testing");
    try {
      const result = await checkPvForecastPlane({
        latitude: parsed.latitude,
        longitude: parsed.longitude,
        tilt_deg: pvTiltDeg,
        azimuth_deg: pvAzimuthDeg,
        kwp: parsed.kwp,
      });
      setPvCheck(result);
      const details =
        result.place || result.timezone
          ? ` (${[result.place, result.timezone].filter(Boolean).join(" • ")})`
          : "";
      setPvMessage({ type: "success", text: `Forecast.Solar check succeeded${details}.` });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Forecast.Solar check failed.";
      setPvMessage({ type: "error", text });
    } finally {
      setPvBusy(null);
    }
  };

	  const savePvForecast = async () => {
	    setPvMessage(null);
	    setPvCheck(null);
	    const parsed = parsePvInputs();
	    if (!parsed) return;
	    setPvBusy("saving");
	    try {
	      const check = await checkPvForecastPlane({
	        latitude: parsed.latitude,
	        longitude: parsed.longitude,
	        tilt_deg: pvTiltDeg,
	        azimuth_deg: pvAzimuthDeg,
	        kwp: parsed.kwp,
	      });
	      setPvCheck(check);

	      await updatePvForecastConfig(node.id, {
	        enabled: pvEnabled,
	        latitude: parsed.latitude,
	        longitude: parsed.longitude,
	        tilt_deg: pvTiltDeg,
	        azimuth_deg: pvAzimuthDeg,
	        kwp: parsed.kwp,
	        time_format: pvTimeFormat,
	      });

	      let savedMessage: Message = { type: "success", text: "Saved PV forecast configuration." };
	      if (pvEnabled) {
	        try {
	          const result = await pollForecasts();
	          const providerStatus = result.providers["Forecast.Solar"];
	          if (providerStatus && providerStatus !== "ok") {
	            savedMessage = {
	              type: "error",
	              text: `Saved PV forecast configuration, but Forecast.Solar refresh returned: ${providerStatus}`,
	            };
	          } else {
	            savedMessage = {
	              type: "success",
	              text: "Saved PV forecast configuration and refreshed Forecast.Solar.",
	            };
	          }
	        } catch (err) {
	          const text = err instanceof Error ? err.message : "Failed to refresh forecasts.";
	          savedMessage = {
	            type: "error",
	            text: `Saved PV forecast configuration, but forecast refresh failed: ${text}`,
	          };
	        }
	      }

	      await Promise.all([
	        queryClient.invalidateQueries({ queryKey: queryKeys.pvForecastConfig(node.id) }),
	        queryClient.invalidateQueries({ queryKey: queryKeys.forecastStatus }),
	        queryClient.invalidateQueries({ queryKey: ["forecast", "pv", "hourly"] }),
	        queryClient.invalidateQueries({ queryKey: ["forecast", "pv", "daily"] }),
	      ]);
	      setPvDirty(false);
	      setPvMessage(savedMessage);
	    } catch (err) {
	      const text = err instanceof Error ? err.message : "Failed to save PV forecast configuration.";
	      setPvMessage({ type: "error", text });
	    } finally {
	      setPvBusy(null);
	    }
	  };

  const applyWeatherLocation = () => {
    if (!weatherLocation) return;
    setPvLatitude(weatherLocation.latitude);
    setPvLongitude(weatherLocation.longitude);
    setPvDirty(true);
    setPvMessage(null);
  };

  const applyMapPlacement = async () => {
    setPvMessage(null);
    setPvBusy("locating");
    try {
      const features = await queryClient.fetchQuery({
        queryKey: queryKeys.mapFeatures,
        queryFn: fetchMapFeatures,
        staleTime: 60_000,
      });
      const nodeFeature = features.find((feature) => feature.node_id === node.id) ?? null;
      if (!nodeFeature) {
        setPvMessage({ type: "error", text: "Node is not placed on the active map." });
        return;
      }
      const coords = readGeoJsonPoint(nodeFeature.geometry);
      if (!coords) {
        setPvMessage({ type: "error", text: "Node placement is not a point geometry." });
        return;
      }
      setPvLatitude(coords.latitude.toFixed(6));
      setPvLongitude(coords.longitude.toFixed(6));
      setPvDirty(true);
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to load map placement.";
      setPvMessage({ type: "error", text });
    } finally {
      setPvBusy(null);
    }
  };

  const quotaHint =
    pvForecastConfigQuery.data?.updated_at || pvCheck?.checked_at
      ? [
          pvForecastConfigQuery.data?.updated_at
            ? `Saved ${new Date(pvForecastConfigQuery.data.updated_at).toLocaleString()}`
            : null,
          pvCheck?.checked_at ? `Checked ${new Date(pvCheck.checked_at).toLocaleString()}` : null,
        ]
          .filter(Boolean)
          .join(" · ")
      : null;

  const configChip = pvForecastConfigQuery.data ? (
    <span
      className={`rounded-full px-3 py-1 text-xs font-semibold ${
        pvForecastConfigQuery.data.enabled
          ? "bg-success-surface text-success-surface-foreground"
          : "bg-card-inset text-card-foreground"
      }`}
    >
      {pvForecastConfigQuery.data.enabled ? "Enabled" : "Disabled"}
    </span>
  ) : null;

  return (
    <CollapsibleCard
      title={node.name}
      description={statusLabel}
      actions={<>{configChip}{pvForecastConfigQuery.isLoading && open ? <span className="text-xs text-muted-foreground">Loading…</span> : null}</>}
      open={open}
      onOpenChange={setOpen}
      density="sm"
      className="shadow-xs"
    >
      <div className="space-y-4">
        {pvMessage && (
          <InlineBanner tone={pvMessage.type === "success" ? "success" : "danger"} className="rounded-lg">
            {pvMessage.text}
          </InlineBanner>
        )}

        <div className="grid gap-6 lg:grid-cols-2">
          <div className="space-y-4">
            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
                <div>
                  <p className="text-sm font-semibold text-card-foreground">
                    Location + capacity
                  </p>
 <p className="text-xs text-muted-foreground">
                    Latitude/longitude should match the charge controller location (node map placement is easiest).
                  </p>
                </div>
 <label className="flex items-center gap-2 text-sm text-foreground">
                  <input
                    type="checkbox"
                    checked={pvEnabled}
                    onChange={(event) => {
                      setPvEnabled(event.target.checked);
                      setPvDirty(true);
                    }}
                  />
                  Enabled
                </label>
	              </div>

	              <div className="mt-3 grid gap-3">
	                <div className="grid gap-3 md:grid-cols-2">
	 <label className="grid gap-1 text-sm text-foreground">
	 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
	                      Latitude (°)
	                    </span>
	                    <Input
	                      inputMode="decimal"
	                      placeholder="37.123456"
	                      value={pvLatitude}
	                      onChange={(event) => {
	                        setPvLatitude(event.target.value);
	                        setPvDirty(true);
	                      }}
	                    />
	                  </label>
	 <label className="grid gap-1 text-sm text-foreground">
	 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
	                      Longitude (°)
	                    </span>
	                    <Input
	                      inputMode="decimal"
	                      placeholder="-122.123456"
	                      value={pvLongitude}
	                      onChange={(event) => {
	                        setPvLongitude(event.target.value);
	                        setPvDirty(true);
	                      }}
	                    />
	                  </label>
	 <label className="grid gap-1 text-sm text-foreground md:col-span-2">
	 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
	                      Installed peak power (kWp)
	                    </span>
	                    <Input
	                      inputMode="decimal"
	                      placeholder="1.0"
	                      value={pvKwp}
	                      onChange={(event) => {
	                        setPvKwp(event.target.value);
	                        setPvDirty(true);
	                      }}
	                    />
	 <p className="text-xs text-muted-foreground">
	                      Rated peak capacity (e.g., 4.8 for a 4.8 kW array).
	                    </p>
	                  </label>
	                  <label className="grid gap-1 md:col-span-2">
	 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
	                      Timestamp format
	                    </span>
                    <Select
                      value={pvTimeFormat}
                      onChange={(event) => {
                        setPvTimeFormat(event.target.value === "iso8601" ? "iso8601" : "utc");
                        setPvDirty(true);
                      }}
                    >
                      <option value="utc">UTC (recommended)</option>
                      <option value="iso8601">ISO-8601 (local offset)</option>
                    </Select>
                  </label>
                </div>

                <div className="flex flex-wrap items-center gap-2">
                  <NodeButton
                    size="xs"
                    onClick={applyMapPlacement}
                    disabled={pvBusy != null}
                    title="Use the node point from the active map save"
                  >
                    {pvBusy === "locating" ? "Locating..." : "Use map placement"}
                  </NodeButton>
                  <NodeButton
                    size="xs"
                    onClick={applyWeatherLocation}
                    disabled={pvBusy != null || !weatherLocation}
                    title="Copy from Weather forecast location (Setup Center)"
                  >
                    Use weather location
                  </NodeButton>
                  {quotaHint ? (
 <span className="text-xs text-muted-foreground">{quotaHint}</span>
                  ) : null}
                </div>

                <div className="flex flex-wrap gap-2">
                  <NodeButton
                    size="xs"
                    onClick={testPvForecastPlane}
                    disabled={pvBusy != null}
                  >
                    {pvBusy === "testing" ? "Testing..." : "Test settings"}
                  </NodeButton>
                  <NodeButton
                    size="xs"
                    variant="primary"
                    onClick={savePvForecast}
                    disabled={pvBusy != null}
                  >
                    {pvBusy === "saving" ? "Saving..." : "Save"}
                  </NodeButton>
                </div>
              </div>
            </Card>
          </div>

          <div className="space-y-4">
            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div>
                  <p className="text-sm font-semibold text-card-foreground">
                    Tilt (degrees)
                  </p>
 <p className="text-xs text-muted-foreground">
                    0° = flat, 90° = vertical. Typical rooftop panels are 20–35°.
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <NumericDraftInput
                    value={pvTiltDeg}
                    onValueChange={(next) => {
                      if (typeof next === "number") {
                        setPvTiltDeg(next);
                        setPvDirty(true);
                      }
                    }}
                    emptyBehavior="keep"
                    min={0}
                    max={90}
                    integer
                    inputMode="numeric"
                    enforceRange
                    clampOnBlur
 className="w-20 rounded-lg border border-border bg-white px-2 py-1 text-sm"
                  />
 <span className="text-sm text-muted-foreground">°</span>
                </div>
              </div>
              <div className="mt-3 flex flex-col gap-4 md:flex-row md:items-center">
                <input
                  type="range"
                  min={0}
                  max={90}
                  step={1}
                  value={pvTiltDeg}
                  onChange={(event) => {
                    setPvTiltDeg(Number(event.target.value));
                    setPvDirty(true);
                  }}
                  className="w-full"
                />
                <TiltDiagram tiltDeg={pvTiltDeg} />
              </div>
            </Card>

            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div>
                  <p className="text-sm font-semibold text-card-foreground">
                    Azimuth (degrees)
                  </p>
 <p className="text-xs text-muted-foreground">
                    Click the compass or drag the slider. South = 0°, West = +90°, East = -90°.
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  <NumericDraftInput
                    value={pvAzimuthDeg}
                    onValueChange={(next) => {
                      if (typeof next === "number") {
                        setPvAzimuthDeg(next);
                        setPvDirty(true);
                      }
                    }}
                    emptyBehavior="keep"
                    min={-180}
                    max={180}
                    integer
                    inputMode="numeric"
                    enforceRange
                    clampOnBlur
 className="w-24 rounded-lg border border-border bg-white px-2 py-1 text-sm"
                  />
 <span className="text-sm text-muted-foreground">°</span>
                </div>
              </div>
              <div className="mt-3 flex flex-col gap-4 md:flex-row md:items-center">
                <input
                  type="range"
                  min={-180}
                  max={180}
                  step={1}
                  value={pvAzimuthDeg}
                  onChange={(event) => {
                    setPvAzimuthDeg(Number(event.target.value));
                    setPvDirty(true);
                  }}
                  className="w-full"
                />
                <AzimuthCompass azimuthDeg={pvAzimuthDeg} onChange={(value) => {
                  setPvAzimuthDeg(value);
                  setPvDirty(true);
                }} />
              </div>
            </Card>
          </div>
        </div>
      </div>
    </CollapsibleCard>
  );
}

function AzimuthCompass({
  azimuthDeg,
  onChange,
}: {
  azimuthDeg: number;
  onChange: (value: number) => void;
}) {
  const size = 160;
  const center = size / 2;
  const radius = 60;
  const bearing = ((180 + azimuthDeg) % 360 + 360) % 360;
  const theta = (bearing * Math.PI) / 180;
  const x = center + radius * Math.sin(theta);
  const y = center - radius * Math.cos(theta);

  const handleClick = (event: React.MouseEvent<SVGSVGElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const dx = event.clientX - cx;
    const dy = event.clientY - cy;
    const clickBearing = ((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360;
    let value = clickBearing - 180;
    if (value > 180) value -= 360;
    if (value < -180) value += 360;
    onChange(Math.round(value));
  };

  return (
    <svg
      width={size}
      height={size}
      viewBox={`0 0 ${size} ${size}`}
      onClick={handleClick}
      className="cursor-crosshair"
      aria-label="Azimuth compass"
    >
      <circle cx={center} cy={center} r={radius} fill="none" stroke="#cbd5e1" strokeWidth="2" />
      <line
        x1={center}
        y1={center - radius}
        x2={center}
        y2={center + radius}
        stroke="#cbd5e1"
        strokeWidth="1"
      />
      <line
        x1={center - radius}
        y1={center}
        x2={center + radius}
        y2={center}
        stroke="#cbd5e1"
        strokeWidth="1"
      />
      <text x={center} y={22} textAnchor="middle" fontSize="12" fill="#64748b">
        N
      </text>
      <text x={center} y={size - 12} textAnchor="middle" fontSize="12" fill="#64748b">
        S (0°)
      </text>
      <text x={size - 18} y={center + 4} textAnchor="middle" fontSize="12" fill="#64748b">
        E (-90°)
      </text>
      <text x={18} y={center + 4} textAnchor="middle" fontSize="12" fill="#64748b">
        W (+90°)
      </text>
      <line x1={center} y1={center} x2={x} y2={y} stroke="#2563eb" strokeWidth="3" />
      <circle cx={x} cy={y} r={5} fill="#2563eb" />
    </svg>
  );
}

function TiltDiagram({ tiltDeg }: { tiltDeg: number }) {
  const size = 160;
  const baseX = 30;
  const baseY = 120;
  const length = 90;
  const theta = (tiltDeg * Math.PI) / 180;
  const endX = baseX + length * Math.cos(theta);
  const endY = baseY - length * Math.sin(theta);
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-label="Tilt diagram">
      <line x1={20} y1={baseY} x2={size - 20} y2={baseY} stroke="#cbd5e1" strokeWidth="2" />
      <line
        x1={baseX}
        y1={baseY}
        x2={endX}
        y2={endY}
        stroke="#16a34a"
        strokeWidth="5"
        strokeLinecap="round"
      />
      <circle cx={baseX} cy={baseY} r={4} fill="#16a34a" />
      <text x={baseX} y={baseY + 20} textAnchor="start" fontSize="12" fill="#64748b">
        0°
      </text>
      <text x={endX + 6} y={endY} textAnchor="start" fontSize="12" fill="#64748b">
        {tiltDeg}°
      </text>
    </svg>
  );
}
