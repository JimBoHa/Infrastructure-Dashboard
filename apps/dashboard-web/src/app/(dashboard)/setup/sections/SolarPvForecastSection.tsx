"use client";

import { useMemo } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import { useForecastStatusQuery } from "@/lib/queries";
import type { DemoNode } from "@/types/dashboard";

import PvForecastNodeCard from "../components/PvForecastNodeCard";
import StatusRow from "../components/StatusRow";

type ForecastSolarQuota = {
  period: number;
  limit: number;
  remaining: number;
  zone: string | null;
  lastSeen: string | null;
};

export default function SolarPvForecastSection({
  pvConfigNodes,
  pvRequestedNodeId,
  pvWeatherLocation,
}: {
  pvConfigNodes: DemoNode[];
  pvRequestedNodeId: string | null;
  pvWeatherLocation: { latitude: string; longitude: string } | null;
}) {
  const forecastStatusQuery = useForecastStatusQuery();

  const forecastSolarQuota = useMemo<ForecastSolarQuota | null>(() => {
    const providers = forecastStatusQuery.data?.providers ?? {};
    const forecastSolar = providers["Forecast.Solar"];
    const meta = (forecastSolar?.meta ?? {}) as Record<string, unknown>;
    const ratelimit = meta.ratelimit;
    if (!ratelimit || typeof ratelimit !== "object") return null;
    const record = ratelimit as Record<string, unknown>;
    const period = typeof record.period === "number" ? record.period : null;
    const limit = typeof record.limit === "number" ? record.limit : null;
    const remaining = typeof record.remaining === "number" ? record.remaining : null;
    const zone = typeof record.zone === "string" ? record.zone : null;
    if (period == null || limit == null || remaining == null) return null;
    return {
      period,
      limit,
      remaining,
      zone,
      lastSeen: forecastSolar?.last_seen ?? null,
    };
  }, [forecastStatusQuery.data]);

  return (
    <CollapsibleCard
      title="Solar PV forecast (Forecast.Solar Public)"
      description={
        <>
          Configure Forecast.Solar per node (tilt, azimuth, capacity, location). Each node card
          validates with <code className="px-1">/check</code> first so you can verify without
          consuming rate-limited <code className="px-1">/estimate</code> calls.
        </>
      }
      defaultOpen
      bodyClassName="space-y-4"
      id="pv-forecast"
    >
      <div className="grid gap-6 lg:grid-cols-2">
        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <p className="text-sm font-semibold text-card-foreground">Provider status</p>
          <div className="mt-2 space-y-2 text-sm">
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
                const forecastSolar = providers["Forecast.Solar"];
                const quotaMeta = (() => {
                  if (!forecastSolarQuota) return null;
                  const used = Math.max(
                    0,
                    Math.round(forecastSolarQuota.limit - forecastSolarQuota.remaining),
                  );
                  const remaining = Math.max(0, Math.round(forecastSolarQuota.remaining));
                  const windowMins = Math.max(1, Math.round(forecastSolarQuota.period / 60));
                  return `${used}/${remaining} · ${windowMins}m`;
                })();
                const quotaTitle = forecastSolarQuota
                  ? `Forecast.Solar Public quota (used/remaining · window). Public plan limit: 12 calls per 60m per public IP. Use “Test settings” (/check) to validate; /estimate consumes quota.${forecastSolarQuota.zone ? ` Zone: ${forecastSolarQuota.zone}.` : ""}`
                  : undefined;
                return (
                  <StatusRow
                    name="Forecast.Solar"
                    status={forecastSolar?.status}
                    lastSeen={forecastSolar?.last_seen ?? null}
                    detail={forecastSolar?.details ?? null}
                    meta={quotaMeta}
                    metaTitle={quotaTitle}
                  />
                );
              })()
            )}
          </div>
        </Card>

        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <p className="text-sm font-semibold text-card-foreground">Solar controller nodes</p>
 <p className="mt-2 text-sm text-muted-foreground">
            Cards appear for nodes with Renogy BT‑2 telemetry (plus any node linked from Node
            Detail). Use <span className="font-semibold">Use map placement</span> to copy the
            node’s location from the active Map save.
          </p>
 <p className="mt-2 text-xs text-muted-foreground">
            Forecast.Solar convention: South = 0°, West = +90°, East = -90°, North = ±180°.
          </p>
        </Card>
      </div>

      {pvConfigNodes.length === 0 ? (
 <p className="mt-4 text-sm text-muted-foreground">
          No solar charge controller nodes detected yet. Apply a Renogy BT‑2 preset on a node
          (Power tab) to enable PV telemetry, then configure Forecast.Solar here.
        </p>
      ) : (
        <div className="mt-4 space-y-3">
          {pvConfigNodes.map((node, idx) => (
            <PvForecastNodeCard
              key={node.id}
              node={node}
              defaultOpen={
                pvRequestedNodeId
                  ? pvRequestedNodeId === node.id
                  : idx === 0 && pvConfigNodes.length === 1
              }
              weatherLocation={pvWeatherLocation}
            />
          ))}
        </div>
      )}
    </CollapsibleCard>
  );
}
