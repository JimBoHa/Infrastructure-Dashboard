"use client";

import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { formatDistanceToNow } from "date-fns";
import { postJson } from "@/lib/api";
import { queryKeys, useAlarmEventsQuery } from "@/lib/queries";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useAuth } from "@/components/AuthProvider";
import AlarmEventDetailDrawer from "@/features/sensors/components/AlarmEventDetailDrawer";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";

export default function AlarmEventsPanel({ limit = 50 }: { limit?: number }) {
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canAck = Boolean(me?.capabilities?.includes("alerts.ack"));
  const { data: events = [], isLoading, error } = useAlarmEventsQuery(limit);
  const [busyEventId, setBusyEventId] = useState<string | null>(null);
  const [busyAll, setBusyAll] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);

  const statusLower = (value: unknown) => String(value ?? "").trim().toLowerCase();
  const isResolved = (event: { status?: string | null }) => {
    const status = statusLower(event.status);
    return status === "acknowledged" || status === "ok";
  };

  const activeEvents = events.filter((event) => !isResolved(event));
  const resolvedEvents = events.filter((event) => isResolved(event));
  const ackableEvents = activeEvents.filter((event) => !isResolved(event));
  const selectedEvent = selectedEventId ? events.find((event) => event.id === selectedEventId) ?? null : null;

  const acknowledge = async (eventId: string) => {
    if (!canAck) return;
    setBusyEventId(eventId);
    setMessage(null);
    try {
      await postJson(`/api/alarms/events/${eventId}/ack`);
      void queryClient.invalidateQueries({ queryKey: queryKeys.alarmEvents(limit) });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alarms });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to acknowledge alarm.";
      setMessage(text);
    } finally {
      setBusyEventId(null);
    }
  };

  const acknowledgeAll = async () => {
    if (!canAck) return;
    if (!ackableEvents.length) return;
    setBusyAll(true);
    setMessage(null);
    try {
      const ids = ackableEvents.map((event) => event.id).filter(Boolean);
      await postJson("/api/alarms/events/ack-bulk", { event_ids: ids });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alarmEvents(limit) });
      void queryClient.invalidateQueries({ queryKey: queryKeys.alarms });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to acknowledge alerts.";
      setMessage(text);
    } finally {
      setBusyAll(false);
    }
  };

  return (
    <CollapsibleCard
      title="Alarm events"
      description={`${activeEvents.length} active · ${events.length} total`}
      actions={
        canAck ? (
          <NodeButton
            size="sm"
            onClick={() => void acknowledgeAll()}
            disabled={busyAll || ackableEvents.length === 0}
          >
            {busyAll ? "Acknowledging..." : "Acknowledge all"}
          </NodeButton>
        ) : null
      }
    >
      {message && (
        <InlineBanner tone="danger" className="mt-3 rounded px-3 py-2 text-xs">{message}</InlineBanner>
      )}
      {isLoading && (
 <p className="mt-3 text-sm text-muted-foreground">Loading alarm events…</p>
      )}
      {error && (
        <p className="mt-3 text-sm text-rose-600">
          {error instanceof Error ? error.message : "Failed to load alarm events."}
        </p>
      )}
      {!isLoading && !error && (
        <div className="mt-4 space-y-3">
          {activeEvents.map((event) => {
            const createdAt = event.created_at ? new Date(event.created_at) : null;
            const showAck = canAck && !isResolved(event);
            return (
              <Card
                key={event.id}
                className="flex flex-col gap-3 rounded-lg bg-card-inset p-4 text-sm text-card-foreground md:flex-row md:items-center md:justify-between"
              >
                <button
                  type="button"
                  className="min-w-0 flex-1 text-left focus:outline-hidden"
                  onClick={() => setSelectedEventId(event.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      setSelectedEventId(event.id);
                    }
                  }}
                  aria-label={`View details for alarm event ${event.id}`}
                >
                  <p className="truncate font-semibold">{event.message || "Alarm event"}</p>
 <p className="mt-1 text-xs text-muted-foreground">
                    {createdAt
                      ? `Raised ${formatDistanceToNow(createdAt, { addSuffix: true })}`
                      : "Timestamp unavailable"}
                    {event.origin ? ` · ${event.origin}` : ""}
                  </p>
 <p className="mt-1 text-xs text-muted-foreground">
                    Status: {event.status}
                  </p>
 <p className="mt-1 text-[11px] font-semibold text-indigo-700">
                    View details →
                  </p>
                </button>
                {showAck ? (
                  <NodeButton
                    size="xs"
                    onClick={(e) => {
                      e.stopPropagation();
                      void acknowledge(event.id);
                    }}
                    disabled={busyEventId === event.id}
                  >
                    {busyEventId === event.id ? "Acknowledging..." : "Acknowledge"}
                  </NodeButton>
                ) : null}
              </Card>
            );
          })}

          {!events.length ? (
 <p className="text-sm text-muted-foreground">No alarm events yet.</p>
          ) : activeEvents.length === 0 ? (
 <p className="text-sm text-muted-foreground">No active alarm events.</p>
          ) : null}

          {resolvedEvents.length ? (
            <CollapsibleCard
              title="Acknowledged & cleared"
              description="Hidden by default to keep active alarms high-signal."
              actions={<span className="shrink-0 rounded-full bg-muted px-2.5 py-1 text-xs font-semibold text-foreground">{resolvedEvents.length}</span>}
              defaultOpen={false}
            >
              <div className="space-y-3">
                {resolvedEvents.map((event) => {
                  const createdAt = event.created_at ? new Date(event.created_at) : null;
                  return (
                    <Card
                      key={event.id}
                      className="w-full rounded-lg gap-0 bg-card-inset p-4 text-left text-sm text-card-foreground hover:bg-muted"
                    >
                    <button
                      type="button"
 className="w-full text-left focus:outline-hidden"
                      onClick={() => setSelectedEventId(event.id)}
                      aria-label={`View details for alarm event ${event.id}`}
                    >
                      <p className="font-semibold">{event.message || "Alarm event"}</p>
 <p className="mt-1 text-xs text-muted-foreground">
                        {createdAt
                          ? `Raised ${formatDistanceToNow(createdAt, { addSuffix: true })}`
                          : "Timestamp unavailable"}
                        {event.origin ? ` · ${event.origin}` : ""}
                      </p>
 <p className="mt-1 text-xs text-muted-foreground">
                        Status: {event.status}
                      </p>
                    </button>
                    </Card>
                  );
                })}
              </div>
            </CollapsibleCard>
          ) : null}
        </div>
      )}

      {selectedEvent ? (
        <AlarmEventDetailDrawer
          event={selectedEvent}
          onClose={() => setSelectedEventId(null)}
        />
      ) : null}
    </CollapsibleCard>
  );
}
