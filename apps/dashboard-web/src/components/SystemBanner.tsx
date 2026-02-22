"use client";

import Link from "next/link";
import { useQueryClient } from "@tanstack/react-query";
import {
  queryKeys,
  useAnalyticsQuery,
  useConnectionQuery,
  useNodesQuery,
  useSchedulesQuery,
} from "@/lib/queries";
import { formatDistanceToNow } from "date-fns";
import { Card } from "@/components/ui/card";

const formatKw = (value: number) => `${value.toFixed(1)}\u00A0kW`;

const formatCurrencyValue = (value: number, currency?: string) => {
  if (!Number.isFinite(value) || value === 0) {
    return "—";
  }
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency: currency ?? "USD",
      maximumFractionDigits: value >= 100 ? 0 : 2,
    }).format(value);
  } catch {
    return `$${value.toFixed(0)}`;
  }
};

const formatRate = (rate: number, currency?: string) => {
  if (!Number.isFinite(rate) || rate === 0) {
    return "—";
  }
  return `${formatCurrencyValue(rate, currency)}/kWh`;
};

const SystemBanner = () => {
  const queryClient = useQueryClient();
  const { data: nodes = [], isLoading: nodesLoading } = useNodesQuery();
  const { data: schedules = [], isLoading: schedulesLoading } = useSchedulesQuery();
  const { data: connection, isLoading: connectionLoading } = useConnectionQuery();
  const {
    data: analytics,
    error: analyticsError,
    isLoading: analyticsLoading,
  } = useAnalyticsQuery();

  const isLoading = nodesLoading || schedulesLoading || connectionLoading;

  if (isLoading || !connection || (analyticsLoading && !analyticsError && !analytics)) {
    return (
      <Card className="p-6">
 <p className="text-sm text-muted-foreground">Loading system status…</p>
      </Card>
    );
  }

  const upcomingSchedule = schedules[0];
  const onlineNodes = nodes.filter((node) => node.status === "online").length;
  const analyticsAvailable = Boolean(analytics) && !analyticsError;
  const power = analyticsAvailable ? analytics!.power : null;
  const status = analyticsAvailable ? analytics!.status : null;
  const rateSchedule = power ? power.rate_schedule : null;
  const remoteNodesOnline = status?.remote_nodes_online ?? null;
  const remoteNodesOffline = status?.remote_nodes_offline ?? null;
  const remoteNodesTotal =
    remoteNodesOnline != null && remoteNodesOffline != null
      ? remoteNodesOnline + remoteNodesOffline
      : null;
  const refreshOverview = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
    void queryClient.invalidateQueries({ queryKey: queryKeys.schedules });
    void queryClient.invalidateQueries({ queryKey: queryKeys.connection });
    void queryClient.invalidateQueries({ queryKey: queryKeys.analytics });
  };

  return (
    <div className="grid gap-4 lg:grid-cols-[2fr_1fr]">
      <Card className="gap-4 p-6">
        <div className="flex items-center justify-between gap-3">
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Farm Dashboard
            </p>
            <h1 className="text-2xl font-semibold text-card-foreground">
              System Overview
            </h1>
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2">
 <span className="inline-flex items-center gap-2 rounded-full bg-muted px-3 py-1 text-xs font-semibold text-foreground">
              <span
                className={
                  connection.status === "online"
                    ? "size-2 rounded-full bg-emerald-500"
                    : "size-2 rounded-full bg-gray-400"
                }
                aria-hidden
              />
              {connection.mode} · {connection.status}
            </span>
          </div>
        </div>
 <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
          {onlineNodes} of {nodes.length} nodes online · Remote{" "}
          {remoteNodesTotal != null ? `${remoteNodesOnline}/${remoteNodesTotal}` : "—"}
        </p>
        <div className="mt-4 grid gap-3 md:grid-cols-2">
          <Card className="gap-0 bg-card-inset p-4">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Power (kW)
            </p>
            <div className="mt-2 grid gap-2 text-sm text-card-foreground">
              {power ? (
                <>
 <span className="font-semibold text-indigo-700">
                    Live {formatKw(power.live_kw)} · Solar {formatKw(power.live_solar_kw)}
                  </span>
                  {rateSchedule?.current_rate ? (
 <span className="text-muted-foreground">
                      Rate {formatRate(rateSchedule.current_rate, rateSchedule.currency)}
                    </span>
                  ) : null}
                  {rateSchedule?.est_monthly_cost ? (
 <span className="text-muted-foreground">
                      Est. cost{" "}
                      {formatCurrencyValue(rateSchedule.est_monthly_cost, rateSchedule.currency)}
                    </span>
                  ) : null}
                </>
              ) : (
 <span className="text-muted-foreground">
                  Analytics unavailable
                </span>
              )}
            </div>
          </Card>
          <Card className="gap-0 bg-card-inset p-4">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Connectivity
            </p>
            <div className="mt-2 flex flex-col gap-2 text-sm text-card-foreground">
 <span className="inline-flex items-center rounded-full bg-card px-3 py-1 text-xs font-medium text-foreground shadow-xs">
                Local {connection.local_address}
              </span>
 <span className="inline-flex items-center rounded-full bg-card px-3 py-1 text-xs font-medium text-foreground shadow-xs">
                Cloud {connection.cloud_address}
              </span>
            </div>
          </Card>
        </div>
      </Card>

      {upcomingSchedule ? (
        <Card className="gap-3 p-5">
          <div className="space-y-3 text-sm">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Next schedule
            </p>
            <div>
              <p className="text-lg font-semibold text-card-foreground">
                {upcomingSchedule.name}
              </p>
 <p className="text-muted-foreground">
                Next run{" "}
                {upcomingSchedule.next_run
                  ? formatDistanceToNow(new Date(upcomingSchedule.next_run), { addSuffix: true })
                  : "tbd"}
              </p>
            </div>

            <button
              onClick={refreshOverview}
 className="inline-flex w-full items-center justify-center gap-x-2 rounded-lg bg-indigo-600 px-4 py-2.5 text-sm font-semibold text-white hover:bg-indigo-700 focus:outline-hidden focus:bg-indigo-700 disabled:pointer-events-none disabled:opacity-50"
            >
              Refresh data
            </button>
          </div>
        </Card>
      ) : (
        <Card className="gap-3 p-5">
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Schedules
            </p>
 <p className="mt-1 text-sm text-muted-foreground">No schedules configured.</p>
          </div>
          <div>
            <Link
              href="/schedules"
 className="inline-flex w-full items-center justify-center gap-x-2 rounded-lg border border-border bg-white px-4 py-2.5 text-sm font-semibold text-foreground shadow-xs hover:bg-muted focus:outline-hidden focus:bg-card-inset"
            >
              Open schedules
            </Link>
 <p className="mt-2 text-xs text-muted-foreground">
              Schedules are created and edited on the Schedules tab.
            </p>
          </div>
        </Card>
      )}
    </div>
  );
};

export default SystemBanner;
