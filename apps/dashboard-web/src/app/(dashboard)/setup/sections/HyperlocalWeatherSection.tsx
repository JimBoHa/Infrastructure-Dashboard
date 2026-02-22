"use client";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useForecastStatusQuery } from "@/lib/queries";

import StatusRow from "../components/StatusRow";
import type { WeatherForecastDraftModel } from "../hooks/useWeatherForecastDraft";

export default function HyperlocalWeatherSection({
  model,
}: {
  model: WeatherForecastDraftModel;
}) {
  const forecastStatusQuery = useForecastStatusQuery();

  return (
    <CollapsibleCard
      title="Hyperlocal weather forecast"
      description="Configure hourly + weekly forecasts (Open-Meteo) using latitude/longitude in degrees."
      defaultOpen
      bodyClassName="space-y-4"
      actions={
        <div className="flex flex-wrap gap-2">
          <NodeButton size="xs" onClick={model.refreshNow}>
            Refresh now
          </NodeButton>
          <NodeButton size="xs" variant="primary" onClick={model.save} disabled={model.isSaving}>
            {model.isSaving ? "Saving..." : "Save"}
          </NodeButton>
        </div>
      }
    >
      <div className="grid gap-4 md:grid-cols-3">
        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <p className="text-sm font-semibold text-card-foreground">Location</p>
 <p className="text-xs text-muted-foreground">
            Example: latitude 37.77, longitude -122.42 (negative longitude = west).
          </p>
          <div className="mt-3 grid gap-3">
 <label className="flex items-center gap-2 text-sm text-foreground">
              <input
                type="checkbox"
                checked={model.enabled}
                onChange={(event) => model.setEnabled(event.target.checked)}
              />
              Enabled
            </label>
            <Input
              inputMode="decimal"
              placeholder="Latitude (°)"
              value={model.latitude}
              onChange={(event) => model.setLatitude(event.target.value)}
            />
            <Input
              inputMode="decimal"
              placeholder="Longitude (°)"
              value={model.longitude}
              onChange={(event) => model.setLongitude(event.target.value)}
            />
          </div>
        </Card>

        <Card className="rounded-lg gap-0 bg-card-inset p-4 md:col-span-2">
          <p className="text-sm font-semibold text-card-foreground">Provider status</p>
          <div className="mt-3 space-y-2 text-sm">
            {forecastStatusQuery.isLoading ? (
 <p className="text-muted-foreground">Loading forecast providers…</p>
            ) : forecastStatusQuery.error ? (
              <p className="text-rose-600">
                Failed to load forecast status:{" "}
                {forecastStatusQuery.error instanceof Error
                  ? forecastStatusQuery.error.message
                  : "Unknown error"}
              </p>
            ) : (
              (() => {
                const providers = forecastStatusQuery.data?.providers ?? {};
                const openMeteo = providers["Open-Meteo"];
                return (
                  <>
                    <StatusRow
                      name="Open-Meteo"
                      status={openMeteo?.status}
                      lastSeen={openMeteo?.last_seen ?? null}
                      detail={openMeteo?.details ?? null}
                    />
                    {model.savedAt && (
 <p className="pt-2 text-xs text-muted-foreground">
                        Location saved {new Date(model.savedAt).toLocaleString()}
                      </p>
                    )}
                  </>
                );
              })()
            )}
          </div>
        </Card>
      </div>
    </CollapsibleCard>
  );
}
