"use client";

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Calendar, dateFnsLocalizer, type SlotInfo } from "react-big-calendar";
import withDragAndDrop from "react-big-calendar/lib/addons/dragAndDrop";
import type { EventInteractionArgs } from "react-big-calendar/lib/addons/dragAndDrop";
import { addMinutes, format, parse, startOfWeek } from "date-fns";
import { enUS } from "date-fns/locale/en-US";
import {
  queryKeys,
  useNodesQuery,
  useOutputsQuery,
  useSchedulesQuery,
  useSensorsQuery,
} from "@/lib/queries";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import { fetchScheduleCalendar, postJson, putJson } from "@/lib/api";
import ScheduleCard from "@/features/schedules/components/ScheduleCard";
import ScheduleForm from "@/features/schedules/components/ScheduleForm";
import {
  ScheduleDraft,
  ScheduleEvent,
  Toast,
  applyEventUpdate,
  draftFromSchedule,
  draftFromSlot,
  parseJsonObjectArray,
  toDate,
  updateBlocksForEvent,
} from "@/features/schedules/lib/scheduleUtils";
import NodeButton from "@/features/nodes/components/NodeButton";
import InlineBanner from "@/components/InlineBanner";
import PageHeaderCard from "@/components/PageHeaderCard";
import CollapsibleCard from "@/components/CollapsibleCard";

import "react-big-calendar/lib/css/react-big-calendar.css";
import "react-big-calendar/lib/addons/dragAndDrop/styles.css";

const locales = {
  "en-US": enUS,
};
const localizer = dateFnsLocalizer({
  format,
  parse,
  startOfWeek: () => startOfWeek(new Date(), { weekStartsOn: 1 }),
  getDay: (date: Date) => date.getDay(),
  locales,
});

const DnDCalendar = withDragAndDrop<ScheduleEvent>(Calendar);
type InteractionArgs = EventInteractionArgs<ScheduleEvent>;

export default function SchedulesPageClient() {
  const queryClient = useQueryClient();
  const schedulesQuery = useSchedulesQuery();
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const outputsQuery = useOutputsQuery();
  const [toast, setToast] = useState<Toast | null>(null);
  const [draft, setDraft] = useState<ScheduleDraft | null>(null);
  const [isSavingDraft, setIsSavingDraft] = useState(false);

  const rangeStart = useMemo(() => {
    const now = new Date();
    const monday = startOfWeek(now, { weekStartsOn: 1 });
    return monday;
  }, []);

  const rangeEnd = useMemo(() => addMinutes(rangeStart, 7 * 24 * 60), [rangeStart]);
  const rangeStartIso = rangeStart.toISOString();
  const rangeEndIso = rangeEnd.toISOString();
  const calendarKey = queryKeys.scheduleCalendar(rangeStartIso, rangeEndIso);
  const calendarQuery = useQuery({
    queryKey: calendarKey,
    queryFn: async () => {
      const payload = await fetchScheduleCalendar(rangeStartIso, rangeEndIso);
      return payload.map((event) => ({
        ...event,
        scheduleId: event.scheduleId ?? event.schedule_id ?? "",
        title: event.title ?? event.name ?? "Schedule",
        name: event.name ?? event.title ?? "Schedule",
        start: toDate(event.start),
        end: toDate(event.end),
      }));
    },
    enabled: schedulesQuery.isSuccess,
    staleTime: 30_000,
    placeholderData: (previous) => previous,
  });

  const isLoading =
    schedulesQuery.isLoading ||
    nodesQuery.isLoading ||
    sensorsQuery.isLoading ||
    outputsQuery.isLoading;
  const error =
    schedulesQuery.error ||
    nodesQuery.error ||
    sensorsQuery.error ||
    outputsQuery.error ||
    calendarQuery.error;

  if (isLoading) return <LoadingState label="Loading schedules..." />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load schedules."} />;
  }

  const schedules = schedulesQuery.data ?? [];
  const events = calendarQuery.data ?? [];
  const nodes = nodesQuery.data ?? [];
  const sensors = sensorsQuery.data ?? [];
  const outputs = outputsQuery.data ?? [];

  const persistEventUpdate = async (event: ScheduleEvent, start: Date, end: Date) => {
    const schedule = schedules.find((item) => item.id === event.scheduleId);
    if (!schedule) {
      setToast({ type: "error", text: "Schedule not found for this block." });
      void queryClient.invalidateQueries({ queryKey: calendarKey });
      return;
    }
    const updatedBlocks = updateBlocksForEvent(schedule, event, start, end);
    if (!updatedBlocks) {
      setToast({ type: "error", text: "Unable to match this block in the schedule definition." });
      void queryClient.invalidateQueries({ queryKey: calendarKey });
      return;
    }
    const optimistic = applyEventUpdate(events, event, start, end);
    queryClient.setQueryData(calendarKey, optimistic);
    try {
      await putJson(`/api/schedules/${schedule.id}`, {
        name: schedule.name,
        rrule: schedule.rrule,
        blocks: updatedBlocks,
        conditions: schedule.conditions ?? [],
        actions: schedule.actions ?? [],
      });
      setToast({ type: "success", text: "Schedule updated." });
      void queryClient.invalidateQueries({ queryKey: queryKeys.schedules });
      void queryClient.invalidateQueries({ queryKey: calendarKey });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to update schedule.";
      setToast({ type: "error", text });
      void queryClient.invalidateQueries({ queryKey: calendarKey });
    }
  };

  const onEventResize = async ({ event, start, end }: InteractionArgs) => {
    const nextStart = toDate(start as Date | string | number | undefined);
    const nextEnd = toDate(end as Date | string | number | undefined);
    await persistEventUpdate(event, nextStart, nextEnd);
  };

  const onEventDrop = ({ event, start, end }: InteractionArgs) => {
    void persistEventUpdate(event, toDate(start as Date | string | number | undefined), toDate(end as Date | string | number | undefined));
  };

  const onSelectSlot = (slot: SlotInfo) => {
    if (!slot.start || !slot.end) return;
    const start = toDate(slot.start as Date | string | number | undefined);
    const end = toDate(slot.end as Date | string | number | undefined);
    setDraft(draftFromSlot(start, end));
    setToast({ type: "info", text: "New schedule block prefilled from selection." });
  };

  const onSelectEvent = (event: ScheduleEvent) => {
    const schedule = schedules.find((item) => item.id === event.scheduleId);
    if (!schedule) {
      setToast({ type: "error", text: "Could not find schedule for editing." });
      return;
    }
    setDraft(draftFromSchedule(schedule, event));
  };

  const saveDraft = async () => {
    if (!draft) return;
    setIsSavingDraft(true);
    try {
      let conditionsPayload = draft.conditionsList;
      let actionsPayload = draft.actionsList;
      if (draft.conditionsMode === "json") {
        conditionsPayload = parseJsonObjectArray(draft.conditionsJson, "Conditions");
      }
      if (draft.actionsMode === "json") {
        actionsPayload = parseJsonObjectArray(draft.actionsJson, "Actions");
      }
      const existingSchedule =
        draft.mode === "edit" && draft.scheduleId ? schedules.find((item) => item.id === draft.scheduleId) : undefined;
      const nextBlock: { day: string; start: string; end: string } = {
        day: draft.day,
        start: draft.start,
        end: draft.end,
      };
      let blocksPayload: Array<{ day: string; start: string; end: string }> = [nextBlock];
      if (existingSchedule?.blocks?.length) {
        const original = draft.originalBlock;
        if (original) {
          let updated = false;
          blocksPayload = existingSchedule.blocks.map((block) => {
            const blockDay = (block.day ?? "").toUpperCase();
            if (!updated && blockDay === original.day && block.start === original.start && block.end === original.end) {
              updated = true;
              return nextBlock;
            }
            return block;
          });
          if (!updated) {
            blocksPayload = [...existingSchedule.blocks, nextBlock];
          }
        } else {
          blocksPayload = existingSchedule.blocks;
        }
      }
      const payload = {
        name: draft.name.trim() || "Schedule",
        rrule: draft.rrule.trim() || `FREQ=WEEKLY;BYDAY=${draft.day}`,
        blocks: blocksPayload,
        conditions: conditionsPayload,
        actions: actionsPayload,
      };
      if (draft.mode === "create") {
        await postJson("/api/schedules", payload);
        setToast({ type: "success", text: "Schedule created." });
      } else {
        await putJson(`/api/schedules/${draft.scheduleId}`, payload);
        setToast({ type: "success", text: "Schedule updated." });
      }
      setDraft(null);
      void queryClient.invalidateQueries({ queryKey: queryKeys.schedules });
      void queryClient.invalidateQueries({ queryKey: calendarKey });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to save schedule.";
      setToast({ type: "error", text });
    } finally {
      setIsSavingDraft(false);
    }
  };

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Schedules"
        description="Calendar of automation blocks with condition-aware triggers. Drag or resize to adjust timing and select an empty slot to create a new block."
      />

      {toast && (
        <InlineBanner tone={toast.type === "success" ? "success" : toast.type === "error" ? "error" : "info"}>
          {toast.text}
        </InlineBanner>
      )}

      <CollapsibleCard
        title="Calendar"
        description="Week view. Drag blocks to change timing; select an empty slot to create a new block."
        defaultOpen
      >
        <DnDCalendar
          localizer={localizer}
          events={events}
          startAccessor="start"
          endAccessor="end"
          defaultView="week"
          views={["week"]}
          style={{ height: 520 }}
          onEventDrop={onEventDrop}
          onEventResize={onEventResize}
          onSelectSlot={onSelectSlot}
          onSelectEvent={onSelectEvent}
          selectable
          resizable
        />
      </CollapsibleCard>

      {draft && (
        <CollapsibleCard
          title={draft.mode === "create" ? "New schedule" : "Edit schedule"}
          description="Update blocks, conditions, and actions. Saving applies to the controller immediately."
          defaultOpen
        >
          <ScheduleForm
            draft={draft}
            nodes={nodes}
            sensors={sensors}
            outputs={outputs}
            onChange={setDraft}
            onCancel={() => setDraft(null)}
            onSave={saveDraft}
            saving={isSavingDraft}
            notify={setToast}
          />
        </CollapsibleCard>
      )}

      <CollapsibleCard
        title="Schedule definitions"
        description="Review conditions and actions. Click a block or use Edit to change details."
        defaultOpen
        actions={
          <NodeButton size="sm" onClick={() => setDraft(draftFromSlot(rangeStart, addMinutes(rangeStart, 60)))}>
            New schedule
          </NodeButton>
        }
      >
        <div className="mt-4 grid gap-4 lg:grid-cols-2">
          {schedules.map((schedule) => (
            <ScheduleCard key={schedule.id} schedule={schedule} onEdit={() => setDraft(draftFromSchedule(schedule))} />
          ))}
        </div>
      </CollapsibleCard>
    </div>
  );
}
