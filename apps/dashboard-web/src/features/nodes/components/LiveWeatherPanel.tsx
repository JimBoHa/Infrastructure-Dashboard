"use client";

import { useCurrentWeatherQuery } from "@/lib/queries";
import { formatNumber } from "@/lib/format";
import NodeButton from "@/features/nodes/components/NodeButton";
import { Card } from "@/components/ui/card";

export default function LiveWeatherPanel({ nodeId }: { nodeId: string }) {
  const weatherQuery = useCurrentWeatherQuery(nodeId);
  const data = weatherQuery.data ?? null;

  const metric = (key: string) => {
    const entry = (data?.metrics ?? {})[key] as { unit?: string; value?: number } | undefined;
    if (!entry || typeof entry.value !== "number" || !Number.isFinite(entry.value)) return null;
    return { unit: typeof entry.unit === "string" ? entry.unit : "", value: entry.value };
  };

  const toCardinal = (deg: number) => {
    if (!Number.isFinite(deg)) return "";
    const dirs = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    const idx = Math.round(((deg % 360) / 45)) % 8;
    return dirs[idx] ?? "";
  };

  const temperature = metric("temperature_c");
  const humidity = metric("humidity_pct");
  const windSpeed = metric("wind_speed_mps");
  const windDir = metric("wind_direction_deg");
  const precipitation = metric("precipitation_mm");
  const cloudCover = metric("cloud_cover_pct");

  return (
    <Card className="rounded-lg gap-0 bg-card-inset p-3 shadow-xs">
      <div className="flex items-start justify-between gap-3">
        <div>
 <p className="text-sm font-semibold text-foreground">
            Public provider data
          </p>
 <p className="text-xs text-muted-foreground">
            Open-Meteo current conditions at the node&apos;s mapped location (active map).
          </p>
        </div>
        {data?.observed_at ? (
 <div className="text-right text-xs text-muted-foreground">
 <div className="font-semibold text-foreground">Updated</div>
            <div>{new Date(data.observed_at).toLocaleTimeString()}</div>
          </div>
        ) : null}
      </div>

      {weatherQuery.isLoading ? (
 <p className="mt-3 text-sm text-muted-foreground">
          Loading public provider data…
        </p>
      ) : weatherQuery.error ? (
        <p className="mt-3 text-sm text-rose-600">
          Failed to load public provider data:{" "}
          {weatherQuery.error instanceof Error ? weatherQuery.error.message : "error"}
        </p>
      ) : !data ? (
        <div className="mt-3 space-y-2">
 <p className="text-sm text-muted-foreground">
            Place this node on the Map tab to enable location-based weather.
          </p>
          <NodeButton size="xs" onClick={() => window.location.assign("/map")}>
            Open Map
          </NodeButton>
        </div>
      ) : (
 <div className="mt-3 grid grid-cols-2 gap-3 text-sm text-foreground">
          <WeatherMetric label="Temperature" value={temperature ? `${formatNumber(temperature.value)} ${temperature.unit}` : "—"} />
          <WeatherMetric label="Humidity" value={humidity ? `${formatNumber(humidity.value)} ${humidity.unit}` : "—"} />
          <WeatherMetric label="Cloud cover" value={cloudCover ? `${formatNumber(cloudCover.value, { maximumFractionDigits: 0 })} ${cloudCover.unit}` : "—"} />
          <WeatherMetric label="Precipitation" value={precipitation ? `${formatNumber(precipitation.value)} ${precipitation.unit}` : "—"} />
          <WeatherMetric
            label="Wind"
            value={
              windSpeed
                ? `${formatNumber(windSpeed.value)} ${windSpeed.unit}${windDir ? ` · ${Math.round(windDir.value)}° ${toCardinal(windDir.value)}` : ""}`
                : "—"
            }
          />
          <WeatherMetric
            label="Location"
            value={data.latitude && data.longitude ? `${data.latitude.toFixed(4)}°, ${data.longitude.toFixed(4)}°` : "—"}
          />
        </div>
      )}
    </Card>
  );
}

function WeatherMetric({ label, value }: { label: string; value: string }) {
  return (
    <div>
 <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
 <div className="mt-0.5 font-semibold text-foreground">{value}</div>
    </div>
  );
}
