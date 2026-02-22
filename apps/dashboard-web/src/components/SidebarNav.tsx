"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import clsx from "clsx";
import { useDashboardUi } from "@/components/DashboardUiProvider";
import {
  useAdoptionCandidatesQuery,
  useAlarmsQuery,
  useBackupsQuery,
  useConnectionQuery,
  useNodesQuery,
  useOutputsQuery,
  useSchedulesQuery,
  useSensorsQuery,
  useUsersQuery,
} from "@/lib/queries";
import { formatAmps, formatPercent, formatVolts } from "@/lib/format";
import { useAuth } from "@/components/AuthProvider";
import { Card } from "@/components/ui/card";
import type { DemoAdoptionCandidate, DemoNode } from "@/types/dashboard";

type SidebarNavItem = {
  label: string;
  href: string;
  exact?: boolean;
};

type SidebarNavGroup = {
  label: string;
  items: SidebarNavItem[];
};

const NAV_GROUPS: SidebarNavGroup[] = [
  {
    label: "Operations",
    items: [
      { label: "Overview", href: "/overview" },
      { label: "Nodes", href: "/nodes" },
      { label: "Map", href: "/map" },
      { label: "Sensors & Outputs", href: "/sensors" },
      { label: "Alarms", href: "/alarms" },
      { label: "Alarms (Experimental)", href: "/alarms2" },
      { label: "Schedules", href: "/schedules" },
      { label: "Backups", href: "/backups" },
    ],
  },
  {
    label: "Analytics",
    items: [
      { label: "Analytics Overview", href: "/analytics", exact: true },
      { label: "Trends", href: "/analytics/trends" },
      { label: "Temp Compensation", href: "/analytics/compensation" },
      { label: "Power", href: "/analytics/power" },
    ],
  },
  {
    label: "Admin",
    items: [
      { label: "Setup Center", href: "/setup" },
      { label: "Deployment", href: "/deployment" },
      { label: "Connection", href: "/connection" },
      { label: "Users", href: "/users" },
    ],
  },
];

const filterUnadoptedCandidates = (candidates: DemoAdoptionCandidate[], nodes: DemoNode[]) => {
  const macs = new Set<string>();
  nodes.forEach((node) => {
    if (node.mac_eth) macs.add(node.mac_eth.toLowerCase());
    if (node.mac_wifi) macs.add(node.mac_wifi.toLowerCase());
  });
  return candidates.filter((candidate) => {
    const macEth = candidate.mac_eth?.toLowerCase();
    const macWifi = candidate.mac_wifi?.toLowerCase();
    if (!macEth && !macWifi) return false;
    if (macEth && macs.has(macEth)) return false;
    if (macWifi && macs.has(macWifi)) return false;
    return true;
  });
};

const SidebarNav = () => {
  const pathname = usePathname();
  const { sidebarOpen, closeSidebar } = useDashboardUi();
  const { me } = useAuth();
  const canConfigWrite = Boolean(me?.capabilities?.includes("config.write"));
  const canManageUsers = Boolean(me?.capabilities?.includes("users.manage"));
  const canViewBackups = Boolean(canConfigWrite || me?.capabilities?.includes("backups.view"));
  const { data: nodes = [] } = useNodesQuery();
  const { data: sensors = [] } = useSensorsQuery();
  const { data: outputs = [] } = useOutputsQuery();
  const { data: alarms = [] } = useAlarmsQuery();
  const { data: adoption = [] } = useAdoptionCandidatesQuery({ enabled: canConfigWrite });
  const { data: schedules = [] } = useSchedulesQuery();
  const { data: users = [] } = useUsersQuery({ enabled: canManageUsers });
  const { data: backups = [] } = useBackupsQuery({ enabled: canViewBackups });
  const { data: connection } = useConnectionQuery();

  const nodesOnline = nodes.filter((node) => node.status === "online").length;
  const nodesTotal = nodes.length;
  const sensorsTotal = sensors.length;
  const outputsTotal = outputs.length;
  const predictiveAlarmCount = alarms.filter(
    (alarm) => (alarm.origin ?? alarm.type) === "predictive",
  ).length;
  const activeAlarmCount = alarms.filter((alarm) => alarm.status === "active").length;
  const adoptionNewCount = filterUnadoptedCandidates(adoption, nodes).length;

  const sensorsByNode = new Map<string, typeof sensors>();
  sensors.forEach((sensor) => {
    const list = sensorsByNode.get(sensor.node_id) ?? [];
    list.push(sensor);
    sensorsByNode.set(sensor.node_id, list);
  });

  const powerNodes = nodes.filter((node) => {
    const config = node.config ?? {};
    if (typeof config["external_provider"] === "string") return true;
    const nodeSensors = sensorsByNode.get(node.id) ?? [];
    return nodeSensors.some((sensor) => {
      const source = sensor.config?.["source"];
      return source === "renogy_bt2" || source === "emporia_cloud";
    });
  });

  const navMeta: Record<string, string | number | null> = {
    "/overview": null,
    "/nodes": nodesTotal
      ? `${nodesOnline}/${nodesTotal}${adoptionNewCount ? ` +${adoptionNewCount}` : ""}`
      : adoptionNewCount
        ? `+${adoptionNewCount}`
        : null,
    "/map": null,
    "/sensors": sensorsTotal || null,
    "/alarms": activeAlarmCount || null,
    "/alarms2": activeAlarmCount || null,
    "/users": canManageUsers ? users.length || null : null,
    "/schedules": schedules.length || null,
    "/analytics": null,
    "/analytics/trends": sensorsTotal || null,
    "/analytics/compensation": null,
    "/analytics/power": powerNodes.length || null,
    "/backups": backups.length || null,
    "/setup": null,
    "/provisioning": null,
    "/deployment": null,
    "/connection": connection?.status ?? null,
  };

  const batteryVoltageSensors = sensors.filter(
    (sensor) => sensor.config?.["metric"] === "battery_voltage_v",
  );
  const batteryCurrentSensors = sensors.filter(
    (sensor) => sensor.config?.["metric"] === "battery_current_a",
  );
  const batterySocSensors = sensors.filter((sensor) => {
    const metric = sensor.config?.["metric"];
    return metric === "battery_soc_percent" || metric === "battery_soc";
  });
  const batteryLine = (() => {
    const nodeNameFor = (nodeId: string) =>
      nodes.find((node) => node.id === nodeId)?.name ?? "Unknown";

    if (batteryVoltageSensors.length === 1) {
      const batteryV = batteryVoltageSensors[0];
      const nodeName = nodeNameFor(batteryV.node_id);
      const volts = batteryV.latest_value;
      const amps =
        batteryCurrentSensors.find((sensor) => sensor.node_id === batteryV.node_id)?.latest_value ??
        null;
      if (volts != null && amps != null) {
        return `Battery (${nodeName}): ${formatVolts(volts)} · ${formatAmps(amps)}`;
      }
      if (volts != null) {
        return `Battery (${nodeName}): ${formatVolts(volts)}`;
      }
      return `Battery (${nodeName}): —`;
    }

    if (batteryVoltageSensors.length > 1) {
      return `Battery: ${batteryVoltageSensors.length} nodes`;
    }

    if (batterySocSensors.length === 1) {
      const value = batterySocSensors[0].latest_value;
      const nodeName = nodeNameFor(batterySocSensors[0].node_id);
      return `Battery (${nodeName}): ${value != null ? formatPercent(value) : "—"}`;
    }

    if (batterySocSensors.length > 1) {
      return `Battery: ${batterySocSensors.length} nodes`;
    }

    return "Battery: —";
  })();

  return (
    <>
      <div
        className={clsx(
          "fixed inset-0 z-[55] bg-black/40 transition-opacity [@media(min-width:1024px)_and_(pointer:fine)]:hidden",
          sidebarOpen ? "opacity-100" : "pointer-events-none opacity-0",
        )}
        data-testid="sidebar-backdrop"
        aria-hidden="true"
        onClick={closeSidebar}
      />

      <aside
        id="dashboard-sidebar"
        className={clsx(
          "fixed inset-y-0 start-0 z-[60] w-64 transform border-e border-border bg-card transition-transform duration-300 [@media(min-width:1024px)_and_(pointer:fine)]:z-40 [@media(min-width:1024px)_and_(pointer:fine)]:translate-x-0",
          sidebarOpen ? "translate-x-0" : "-translate-x-full [@media(min-width:1024px)_and_(pointer:fine)]:translate-x-0",
        )}
        role="dialog"
        tabIndex={-1}
        aria-label="Navigation"
      >
        <div className="flex h-full flex-col">
          <div className="flex items-center justify-between gap-2 border-b border-border px-4 py-3">
          <div className="flex items-center gap-2">
            <div className="inline-flex size-9 items-center justify-center rounded-xl bg-indigo-600 text-sm font-semibold text-white">
              FD
            </div>
            <div className="min-w-0">
              <div className="truncate text-sm font-semibold text-card-foreground">
                Farm Dashboard
              </div>
 <div className="truncate text-xs text-muted-foreground">
                Realtime control & insight
              </div>
            </div>
          </div>

          <button
            type="button"
 className="inline-flex size-8 items-center justify-center rounded-lg border border-border text-foreground hover:bg-muted focus:outline-hidden focus:bg-muted [@media(min-width:1024px)_and_(pointer:fine)]:hidden"
            aria-haspopup="dialog"
            aria-expanded={sidebarOpen}
            aria-controls="dashboard-sidebar"
            aria-label="Close navigation"
            onClick={closeSidebar}
          >
            <svg
              className="size-4"
              xmlns="http://www.w3.org/2000/svg"
              width="24"
              height="24"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M18 6 6 18" />
              <path d="m6 6 12 12" />
            </svg>
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-3 py-4">
          {connection ? (
 <div className="mb-4 inline-flex w-full items-center gap-2 rounded-xl bg-muted px-3 py-2 text-xs font-medium text-foreground">
              <span
                className={
                  connection.status === "online"
                    ? "size-2 rounded-full bg-emerald-500"
                    : "size-2 rounded-full bg-gray-400"
                }
                aria-hidden
              />
              <span className="truncate">
                {connection.mode} · {connection.status}
              </span>
            </div>
          ) : null}

          {me ? (
 <Card className="mb-4 gap-0 px-3 py-2 text-xs text-muted-foreground">
              <div className="truncate font-semibold text-card-foreground">{me.email}</div>
 <div className="truncate text-muted-foreground">{me.role}</div>
            </Card>
          ) : null}

          <nav className="space-y-5">
            {NAV_GROUPS.map((group) => {
              const filteredItems = group.items.filter((item) => {
                if (item.href === "/backups") return canViewBackups;
                if (group.label === "Admin") {
                  if (item.href === "/users") return canManageUsers;
                  if (item.href === "/setup" || item.href === "/deployment") return canConfigWrite;
                }
                return true;
              });

              if (filteredItems.length === 0) return null;

              return (
                <div key={group.label}>
 <p className="px-3 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  {group.label}
                </p>
                <ul className="mt-2 space-y-1">
                  {filteredItems.map((item) => {
                    const active = item.exact
                      ? pathname === item.href
                      : pathname === item.href || pathname?.startsWith(`${item.href}/`);
                    const badge = navMeta[item.href];

                    return (
                      <li key={item.href}>
                        <Link
                          href={item.href}
                          onClick={() => closeSidebar()}
                          className={clsx(
 "flex items-center justify-between gap-3 rounded-xl px-3 py-2 text-sm text-foreground hover:bg-muted focus:outline-hidden focus:bg-muted",
                            active &&
 "bg-muted font-semibold text-foreground",
                          )}
                        >
                          <span className="truncate">{item.label}</span>
                          {badge != null && badge !== "" ? (
 <span className="inline-flex items-center rounded-full bg-muted px-2 py-0.5 text-[11px] font-semibold text-foreground">
                              {badge}
                            </span>
                          ) : null}
                        </Link>
                      </li>
                    );
                  })}
                </ul>
              </div>
              );
            })}
          </nav>
        </div>

 <div className="border-t border-border px-4 py-3 text-xs text-muted-foreground">
          <div className="flex flex-col gap-1">
            <span>
              Nodes: {nodesOnline}/{nodesTotal || "–"} online
            </span>
            <span>
              I/O: {sensorsTotal} sensors · {outputsTotal} outputs
            </span>
            <span>Predictive alarms: {predictiveAlarmCount}</span>
            <span>{batteryLine}</span>
          </div>
        </div>
      </div>
      </aside>
    </>
  );
};

export default SidebarNav;
