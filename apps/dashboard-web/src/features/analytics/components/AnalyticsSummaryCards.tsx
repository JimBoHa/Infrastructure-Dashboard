"use client";

import { Card, CardContent } from "@/components/ui/card";
import { formatGallons, formatKwh, formatKw } from "@/lib/format";
import type { AnalyticsBundle } from "@/types/dashboard";

export function AnalyticsSummaryCards({ analytics }: { analytics: AnalyticsBundle }) {
  const totalNodes = analytics.status.nodes_online + analytics.status.nodes_offline;
  const remoteNodesOnline = analytics.status.remote_nodes_online ?? 0;
  const remoteNodesOffline = analytics.status.remote_nodes_offline ?? 0;
  const remoteNodesTotal = remoteNodesOnline + remoteNodesOffline;

  return (
    <section className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <Card>
        <CardContent className="pt-6">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Fleet power (kW)
          </p>
 <p className="mt-2 text-2xl font-semibold leading-tight text-foreground">
            {formatKw(analytics.power.live_kw)}
          </p>
 <p className="text-sm text-muted-foreground">
            Sums across all power nodes (no implied coupling). Solar {formatKw(analytics.power.live_solar_kw)} / Storage{" "}
            {formatKw(analytics.power.live_battery_kw ?? 0)} / Grid {formatKw(analytics.power.live_grid_kw)}
          </p>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="pt-6">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Energy totals
          </p>
 <p className="mt-2 text-2xl font-semibold leading-tight text-foreground">
            {formatKwh(analytics.power.kwh_24h)}
          </p>
 <p className="text-sm text-muted-foreground">
            7 days {formatKwh(analytics.power.kwh_168h ?? 0)} / Solar{" "}
            {formatKwh(analytics.power.solar_kwh_24h ?? 0)}
          </p>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="pt-6">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Water usage
          </p>
 <p className="mt-2 text-2xl font-semibold leading-tight text-foreground">
            {formatGallons(analytics.water.domestic_gal_24h)}
          </p>
 <p className="text-sm text-muted-foreground">
            Ag 7 days {formatGallons(analytics.water.ag_gal_168h)}
          </p>
        </CardContent>
      </Card>
      <Card>
        <CardContent className="pt-6">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            System status
          </p>
 <p className="mt-2 text-2xl font-semibold leading-tight text-foreground">
            {analytics.status.nodes_online}/{totalNodes} online
          </p>
 <p className="text-sm text-muted-foreground">
            Remote {remoteNodesOnline}/{remoteNodesTotal} / Alarms {analytics.status.alarms_last_168h}
          </p>
        </CardContent>
      </Card>
    </section>
  );
}
