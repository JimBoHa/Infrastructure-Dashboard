"use client";

import { useEffect, useMemo, useState } from "react";
import { useInfiniteQuery } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import InlineBanner from "@/components/InlineBanner";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import SegmentedControl from "@/components/SegmentedControl";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";
import { fetchIncidents, type FetchIncidentsParams } from "@/lib/api";
import type { DemoNode, DemoSensor, DemoUser } from "@/types/dashboard";
import type { Incident } from "@/types/incidents";
import IncidentDetailSheet from "./IncidentDetailSheet";

type RangePreset = "24h" | "7d" | "30d" | "all";

const severityTone = (severity: string): "danger" | "warning" | "info" => {
  if (severity === "critical") return "danger";
  if (severity === "warning") return "warning";
  return "info";
};

const statusTone = (status: string): "danger" | "warning" | "muted" => {
  if (status === "snoozed") return "warning";
  if (status === "closed") return "muted";
  return "danger";
};

function resolveAssigneeLabel(incident: Incident, users: DemoUser[], meUserId: string | null): string {
  if (!incident.assigned_to) return "Unassigned";
  if (meUserId && incident.assigned_to === meUserId) return "Assigned to you";
  const user = users.find((u) => u.id === incident.assigned_to);
  return user ? `Assigned to ${user.name}` : "Assigned";
}

function computeFromIso(preset: RangePreset): string | undefined {
  if (preset === "all") return undefined;
  const now = new Date();
  const hours =
    preset === "24h" ? 24 : preset === "7d" ? 7 * 24 : preset === "30d" ? 30 * 24 : 24;
  return new Date(now.getTime() - hours * 60 * 60 * 1000).toISOString();
}

export default function IncidentsConsole({
  canEdit,
  canAck,
  meUserId,
  sensors,
  nodes,
  users,
}: {
  canEdit: boolean;
  canAck: boolean;
  meUserId: string | null;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  users: DemoUser[];
}) {
  const [tab, setTab] = useState<"open" | "all">("open");
  const [status, setStatus] = useState<string>("open");
  const [severity, setSeverity] = useState<string>("all");
  const [assigned, setAssigned] = useState<string>("any");
  const [rangePreset, setRangePreset] = useState<RangePreset>("7d");
  const [searchDraft, setSearchDraft] = useState<string>("");
  const [search, setSearch] = useState<string>("");
  const [selectedIncidentId, setSelectedIncidentId] = useState<string | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);

  useEffect(() => {
    const handle = setTimeout(() => setSearch(searchDraft.trim()), 250);
    return () => clearTimeout(handle);
  }, [searchDraft]);

  const filters = useMemo((): FetchIncidentsParams => {
    const effectiveStatus = tab === "open" ? "open" : status;
    const from = computeFromIso(rangePreset);
    const to = rangePreset === "all" ? undefined : new Date().toISOString();
    const assigned_to = assigned === "me" ? meUserId ?? undefined : undefined;
    const unassigned = assigned === "unassigned" ? true : undefined;
    return {
      status: effectiveStatus === "all" ? undefined : effectiveStatus,
      severity: severity === "all" ? undefined : severity,
      assigned_to,
      unassigned,
      from,
      to,
      search: search || undefined,
      limit: 50,
    };
  }, [assigned, meUserId, rangePreset, search, severity, status, tab]);

  const incidentsQuery = useInfiniteQuery({
    queryKey: ["incidents", filters],
    queryFn: ({ pageParam }) => fetchIncidents({ ...filters, cursor: pageParam as string | undefined }),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (lastPage) => lastPage.next_cursor ?? undefined,
    staleTime: 10_000,
    refetchInterval: 15_000,
  });

  const incidents = useMemo(() => {
    return incidentsQuery.data?.pages.flatMap((page) => page.incidents) ?? [];
  }, [incidentsQuery.data?.pages]);

  const openIncident = (id: string) => {
    setSelectedIncidentId(id);
    setDetailOpen(true);
  };

  return (
    <div className="space-y-4">
      <Card className="rounded-xl border border-border p-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
          <div className="min-w-0">
            <p className="text-sm font-semibold text-card-foreground">Incidents</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Search, triage, assign, snooze, and investigate related signals.
            </p>
          </div>
          <SegmentedControl
            value={tab}
            options={[
              { value: "open", label: "Open" },
              { value: "all", label: "All" },
            ]}
            onChange={(next) => setTab(next as "open" | "all")}
          />
        </div>

        <div className="mt-4 grid gap-3 lg:grid-cols-12">
          <div className="lg:col-span-4">
            <label className="text-xs font-semibold text-muted-foreground">Search</label>
            <Input
              value={searchDraft}
              onChange={(e) => setSearchDraft(e.target.value)}
              placeholder="Title, sensor, node, message…"
            />
          </div>
          <div className="lg:col-span-2">
            <label className="text-xs font-semibold text-muted-foreground">Status</label>
            <Select
              value={tab === "open" ? "open" : status}
              onChange={(e) => setStatus(e.target.value)}
              disabled={tab === "open"}
            >
              <option value="all">All</option>
              <option value="open">Open</option>
              <option value="snoozed">Snoozed</option>
              <option value="closed">Closed</option>
            </Select>
          </div>
          <div className="lg:col-span-2">
            <label className="text-xs font-semibold text-muted-foreground">Severity</label>
            <Select value={severity} onChange={(e) => setSeverity(e.target.value)}>
              <option value="all">All</option>
              <option value="critical">Critical</option>
              <option value="warning">Warning</option>
              <option value="info">Info</option>
            </Select>
          </div>
          <div className="lg:col-span-2">
            <label className="text-xs font-semibold text-muted-foreground">Assigned</label>
            <Select value={assigned} onChange={(e) => setAssigned(e.target.value)}>
              <option value="any">Anyone</option>
              <option value="me" disabled={!meUserId}>
                Me
              </option>
              <option value="unassigned">Unassigned</option>
            </Select>
          </div>
          <div className="lg:col-span-2">
            <label className="text-xs font-semibold text-muted-foreground">Range</label>
            <Select value={rangePreset} onChange={(e) => setRangePreset(e.target.value as RangePreset)}>
              <option value="24h">Last 24h</option>
              <option value="7d">Last 7d</option>
              <option value="30d">Last 30d</option>
              <option value="all">All time</option>
            </Select>
          </div>
        </div>
      </Card>

      {incidentsQuery.isLoading ? <LoadingState label="Loading incidents..." /> : null}
      {incidentsQuery.error ? (
        <ErrorState
          message={
            incidentsQuery.error instanceof Error
              ? incidentsQuery.error.message
              : "Failed to load incidents."
          }
        />
      ) : null}

      {!incidentsQuery.isLoading && !incidentsQuery.error ? (
        <>
          {!incidents.length ? (
            <Card className="rounded-xl border border-dashed border-border p-6 text-sm text-muted-foreground">
              No incidents found for the current filters.
            </Card>
          ) : (
            <div className="space-y-2">
              {incidents.map((incident) => {
                const lastActivity = incident.last_event_at;
                const assigneeLabel = resolveAssigneeLabel(incident, users, meUserId);
                return (
                  <Card
                    key={incident.id}
                    className="cursor-pointer rounded-xl border border-border p-4 transition-colors hover:bg-muted"
                    onClick={() => openIncident(incident.id)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        openIncident(incident.id);
                      }
                    }}
                  >
                    <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                      <div className="min-w-0">
                        <div className="flex flex-wrap items-center gap-2">
                          <Badge tone={severityTone(incident.severity)}>{incident.severity}</Badge>
                          <Badge tone={statusTone(incident.status)}>{incident.status}</Badge>
                          <p className="truncate text-sm font-semibold text-card-foreground">
                            {incident.title}
                          </p>
                        </div>
                        {incident.last_message ? (
                          <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                            {incident.last_message}
                          </p>
                        ) : null}
                        <p className="mt-2 text-xs text-muted-foreground">
                          {lastActivity
                            ? `Last activity ${formatDistanceToNow(lastActivity, { addSuffix: true })}`
                            : "Last activity unavailable"}
                          {incident.last_origin ? ` · ${incident.last_origin}` : ""}
                        </p>
                      </div>

                      <div className="flex shrink-0 flex-col items-start gap-2 md:items-end">
                        <p className="text-xs font-semibold text-muted-foreground">{assigneeLabel}</p>
                        <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                          <span>Active: {incident.active_event_count}</span>
                          <span>•</span>
                          <span>Events: {incident.total_event_count}</span>
                          <span>•</span>
                          <span>Notes: {incident.note_count}</span>
                        </div>
                        <p className="text-[11px] font-semibold text-indigo-700">Open →</p>
                      </div>
                    </div>
                  </Card>
                );
              })}

              {incidentsQuery.hasNextPage ? (
                <div className="flex justify-center pt-2">
                  <NodeButton
                    size="sm"
                    onClick={() => void incidentsQuery.fetchNextPage()}
                    disabled={incidentsQuery.isFetchingNextPage}
                  >
                    {incidentsQuery.isFetchingNextPage ? "Loading…" : "Load more"}
                  </NodeButton>
                </div>
              ) : null}
            </div>
          )}
        </>
      ) : null}

      <IncidentDetailSheet
        open={detailOpen}
        onOpenChange={(next) => {
          setDetailOpen(next);
          if (!next) setSelectedIncidentId(null);
        }}
        incidentId={selectedIncidentId}
        canEdit={canEdit}
        canAck={canAck}
        meUserId={meUserId}
        sensors={sensors}
        nodes={nodes}
        users={users}
      />

      {!canEdit ? (
        <InlineBanner tone="info">
          You have view-only permissions. Incident assignment and rule editing require `config.write`.
        </InlineBanner>
      ) : null}
    </div>
  );
}
