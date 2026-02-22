"use client";

import { useMemo, useState } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  AnalyticsRangeSelect,
  type AnalyticsHistoryRangeHours,
  historyRangeLabel,
  AnalyticsChart,
  Metric,
  type ChartSeriesConfig,
} from "@/features/analytics/components/AnalyticsShared";
import { useAnalyticsData } from "@/features/analytics/hooks/useAnalyticsData";
import { ANALYTICS_COLORS as COLORS } from "@/features/analytics/utils/colors";
import { filterSeriesByHours } from "@/features/analytics/utils/series";
import { formatCurrencyValue, formatKwh, formatPercent, formatRate, formatWatts } from "@/lib/format";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { classifyPowerNode } from "@/lib/powerSensors";
import { findSensor, sensorMetric } from "@/lib/sensorOrigin";
import type { AnalyticsIntegration, AnalyticsPower, AnalyticsRateSchedule } from "@/types/dashboard";

export function PowerSection({ power }: { power: AnalyticsPower }) {
  const [rangeHours, setRangeHours] = useState<AnalyticsHistoryRangeHours>(24);

  const solar24h = power.solar_series_24h ?? [];
  const grid24h = power.grid_series_24h ?? [];
  const battery24h = power.battery_series_24h ?? [];
  const solar168h = power.solar_series_168h ?? [];
  const grid168h = power.grid_series_168h ?? [];
  const battery168h = power.battery_series_168h ?? [];
  const seriesForRange = <T extends { timestamp: Date }>(shortSeries: T[], longSeries: T[]): T[] => {
    if (rangeHours === 24) return shortSeries;
    const base = longSeries.length ? longSeries : shortSeries;
    return filterSeriesByHours(base, rangeHours);
  };

  const chartSeries: ChartSeriesConfig[] = [
    {
      label: "Total",
      series: seriesForRange(power.series_24h, power.series_168h),
      color: COLORS.total,
    },
  ];
  const solarSeries = seriesForRange(solar24h, solar168h);
  const gridSeries = seriesForRange(grid24h, grid168h);
  const batterySeries = seriesForRange(battery24h, battery168h);
  if (solarSeries.length) {
    chartSeries.push({ label: "Solar", series: solarSeries, color: COLORS.solar });
  }
  if (gridSeries.length) {
    chartSeries.push({ label: "Grid", series: gridSeries, color: COLORS.grid });
  }
  if (batterySeries.length) {
    chartSeries.push({ label: "Storage", series: batterySeries, color: COLORS.battery });
  }

  return (
    <CollapsibleCard
      title="Power (kW)"
      description="Charts show fleet sums across nodes. Each Renogy controller and Emporia meter is treated as its own node; totals do not imply a coupled system."
      defaultOpen
      bodyClassName="space-y-4"
      actions={<AnalyticsRangeSelect value={rangeHours} onChange={setRangeHours} />}
    >
      <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-[2fr_1fr]">
        <AnalyticsChart title={`Power (kW) — ${historyRangeLabel(rangeHours)}`} series={chartSeries} unit="kW" rangeHours={rangeHours} />
        <aside className="space-y-4">
          <EnergyBreakdown power={power} />
          <RateScheduleCard schedule={power.rate_schedule} />
        </aside>
      </div>

      <IntegrationList integrations={power.integrations ?? []} />

      <PowerNodesBreakdown />
    </CollapsibleCard>
  );
}

function PowerNodesBreakdown() {
  const { nodes, sensorsByNodeId, isLoading, error } = useAnalyticsData();

  const rows = useMemo(() => {
    return nodes
      .map((node) => {
        const nodeSensors = sensorsByNodeId.get(node.id) ?? [];
        const kind = classifyPowerNode(node, nodeSensors);
        if (!kind) return null;

        if (kind === "emporia") {
          const config = node.config ?? {};
          const mains = findSensor(nodeSensors, "emporia_cloud", "mains_power_w");
          const circuitCount = nodeSensors.filter((sensor) => sensorMetric(sensor) === "channel_power_w").length;
          return {
            id: node.id,
            name: node.name,
            status: node.status,
            lastSeen: node.last_seen ?? null,
            kind,
            groupLabel: typeof config.group_label === "string" ? (config.group_label as string) : null,
            includeInPowerSummary: config.include_in_power_summary !== false,
            mainsPowerW: mains?.latest_value ?? null,
            pvPowerW: null,
            loadPowerW: null,
            batterySoc: null,
            circuitCount,
          };
        }

        const pv = findSensor(nodeSensors, "renogy_bt2", "pv_power_w");
        const load = findSensor(nodeSensors, "renogy_bt2", "load_power_w");
        const batterySoc = findSensor(nodeSensors, "renogy_bt2", "battery_soc_percent");
        return {
          id: node.id,
          name: node.name,
          status: node.status,
          lastSeen: node.last_seen ?? null,
          kind,
          groupLabel: null,
          includeInPowerSummary: true,
          mainsPowerW: null,
          pvPowerW: pv?.latest_value ?? null,
          loadPowerW: load?.latest_value ?? null,
          batterySoc: batterySoc?.latest_value ?? null,
          circuitCount: null,
        };
      })
      .filter((row): row is NonNullable<typeof row> => Boolean(row));
  }, [nodes, sensorsByNodeId]);

  const emporiaGroups = useMemo(() => {
    const groups = new Map<
      string,
      { label: string; totalW: number; includedW: number; meterCount: number; includedCount: number }
    >();
    rows.forEach((row) => {
      if (row.kind !== "emporia") return;
      const label = (row.groupLabel ?? "").trim() || "Ungrouped";
      const entry = groups.get(label) ?? {
        label,
        totalW: 0,
        includedW: 0,
        meterCount: 0,
        includedCount: 0,
      };
      entry.meterCount += 1;
      const mains = row.mainsPowerW ?? 0;
      entry.totalW += mains;
      if (row.includeInPowerSummary) {
        entry.includedW += mains;
        entry.includedCount += 1;
      }
      groups.set(label, entry);
    });
    return Array.from(groups.values()).sort((a, b) => a.label.localeCompare(b.label));
  }, [rows]);

  if (isLoading) return null;
  if (error) return null;
  if (!rows.length) {
    return (
      <Card>
        <CardHeader>
 <CardTitle className="text-sm uppercase tracking-wide text-muted-foreground">
            Power nodes
          </CardTitle>
        </CardHeader>
        <CardContent>
 <p className="text-sm text-muted-foreground">
            No Renogy/Emporia power nodes detected yet.
          </p>
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
 <CardTitle className="text-sm uppercase tracking-wide text-muted-foreground">
          Power nodes
        </CardTitle>
 <p className="text-sm text-muted-foreground">
          Per-node power context. Use the Power tab for dedicated dashboards (circuits, PV/load/battery trends).
        </p>
      </CardHeader>

      <CardContent className="space-y-4">
        {emporiaGroups.length ? (
          <Card className="rounded-lg gap-0 bg-card-inset p-4 text-sm">
          <p className="text-sm font-semibold text-card-foreground">
            Emporia meters by address group
          </p>
 <p className="mt-1 text-xs text-muted-foreground">
            Address groups and totals inclusion are configured in Setup Center → Integrations → Emporia meters & totals.
          </p>
          <div className="mt-3 overflow-x-auto">
            <table className="min-w-full divide-y divide-border text-sm">
              <thead className="bg-card-inset">
                <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Address group
                  </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Included live power (W)
                  </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Meters included
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {emporiaGroups.map((group) => (
                  <tr key={group.label}>
                    <td className="px-3 py-2 font-medium text-card-foreground">
                      {group.label}
                    </td>
 <td className="px-3 py-2 text-muted-foreground">
                      {formatWatts(group.includedW)}
                    </td>
 <td className="px-3 py-2 text-muted-foreground">
                      {group.includedCount}/{group.meterCount}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Card>
      ) : null}

        <div className="overflow-x-auto">
          <table className="min-w-full divide-y divide-border text-sm">
            <thead className="bg-card-inset">
            <tr>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Node
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Group
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Provider
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                PV power (W)
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Load/mains power (W)
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Battery SOC (Renogy)
              </th>
 <th className="px-4 py-3 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Notes
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {rows.map((row) => (
              <tr key={row.id}>
                <td className="px-4 py-3">
                  <div className="font-medium text-card-foreground">
                    {row.name}
                  </div>
 <div className="text-xs text-muted-foreground">
                    {formatNodeStatusLabel(row.status, row.lastSeen)}
                  </div>
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {row.kind === "emporia" ? (row.groupLabel?.trim() || "—") : "—"}
                  {row.kind === "emporia" && !row.includeInPowerSummary ? (
 <div className="text-xs text-amber-600">
                      excluded from totals
                    </div>
                  ) : null}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {row.kind === "emporia" ? "Emporia" : "Renogy"}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {row.pvPowerW != null ? formatWatts(row.pvPowerW) : "—"}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {row.kind === "emporia"
                    ? row.mainsPowerW != null
                      ? formatWatts(row.mainsPowerW)
                      : "—"
                    : row.loadPowerW != null
                      ? formatWatts(row.loadPowerW)
                      : "—"}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {row.batterySoc != null ? formatPercent(row.batterySoc) : "—"}
                </td>
 <td className="px-4 py-3 text-muted-foreground">
                  {row.kind === "emporia" && row.circuitCount != null
                    ? `${row.circuitCount} circuits`
                    : "—"}
                </td>
              </tr>
            ))}
          </tbody>
          </table>
        </div>
      </CardContent>
    </Card>
  );
}

function EnergyBreakdown({ power }: { power: AnalyticsPower }) {
  return (
    <Card>
      <CardHeader>
 <CardTitle className="text-sm uppercase tracking-wide text-muted-foreground">
          Energy Summary
        </CardTitle>
      </CardHeader>
 <CardContent className="space-y-3 text-sm text-muted-foreground">
        <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Past 24 hours
          </p>
          <dl className="mt-1 grid grid-cols-2 gap-2">
            <Metric label="Consumption" value={formatKwh(power.kwh_24h)} />
            <Metric label="Grid" value={formatKwh(power.grid_kwh_24h ?? 0)} />
            <Metric label="Solar" value={formatKwh(power.solar_kwh_24h ?? 0)} />
            <Metric label="Storage" value={formatKwh(power.battery_kwh_24h ?? 0)} />
          </dl>
        </div>
        <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Past 7 days
          </p>
          <dl className="mt-1 grid grid-cols-2 gap-2">
            <Metric label="Consumption" value={formatKwh(power.kwh_168h)} />
            <Metric label="Grid" value={formatKwh(power.grid_kwh_168h ?? 0)} />
            <Metric label="Solar" value={formatKwh(power.solar_kwh_168h ?? 0)} />
            <Metric label="Storage" value={formatKwh(power.battery_kwh_168h ?? 0)} />
          </dl>
        </div>
      </CardContent>
    </Card>
  );
}

function RateScheduleCard({ schedule }: { schedule: AnalyticsRateSchedule }) {
  return (
    <Card>
      <CardContent className="pt-6">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Utility rate
        </p>
        <p className="mt-2 text-lg font-semibold text-card-foreground">
          {schedule.provider || "Not configured"}
        </p>
 <p className="text-sm text-muted-foreground">
          Current rate {formatRate(schedule.current_rate, schedule.currency)}
        </p>
 <p className="mt-3 text-sm text-muted-foreground">
          Est. cost this period {formatCurrencyValue(schedule.est_monthly_cost, schedule.currency)}
        </p>
        {schedule.period_label && (
 <p className="mt-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {schedule.period_label}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

function IntegrationList({ integrations }: { integrations: AnalyticsIntegration[] }) {
  if (!integrations.length) {
    return (
      <Card>
        <CardContent className="pt-6">
 <p className="text-sm text-muted-foreground">
            No integrations connected yet.
          </p>
        </CardContent>
      </Card>
    );
  }
  return (
    <Card>
      <CardHeader>
 <CardTitle className="text-sm uppercase tracking-wide text-muted-foreground">
          Integration Health
        </CardTitle>
      </CardHeader>
      <CardContent>
 <ul className="space-y-2 text-sm text-muted-foreground">
        {integrations.map((integration) => (
          <li key={integration.name} className="flex items-center justify-between gap-3">
            <div>
              <p className="font-semibold text-card-foreground">
                {integration.name}
              </p>
              {integration.details && (
 <p className="text-xs text-muted-foreground">
                  {integration.details}
                </p>
              )}
            </div>
            <span
              className={`rounded-full px-2 py-0.5 text-xs uppercase tracking-wide ${
                integration.status === "connected"
 ? "bg-emerald-100 text-emerald-800"
                  : integration.status === "pending"
 ? "bg-amber-100 text-amber-800"
 : "bg-muted text-foreground"
              }`}
            >
              {integration.status}
            </span>
          </li>
        ))}
        </ul>
      </CardContent>
    </Card>
  );
}
