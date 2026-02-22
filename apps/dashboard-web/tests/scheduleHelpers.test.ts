import { applyEventUpdate } from "@/features/schedules/lib/scheduleUtils";

describe("applyEventUpdate", () => {
  const event = {
    title: "Water Field",
    start: new Date("2025-10-01T05:30:00Z"),
    end: new Date("2025-10-01T06:00:00Z"),
    allDay: false,
    resource: undefined,
    scheduleId: "sched-1",
  };

  it("updates matching event with new boundaries", () => {
    const updated = applyEventUpdate([event], event, new Date("2025-10-01T06:00:00Z"), new Date("2025-10-01T06:30:00Z"));
    expect(updated[0].start).toEqual(new Date("2025-10-01T06:00:00Z"));
    expect(updated[0].end).toEqual(new Date("2025-10-01T06:30:00Z"));
  });

  it("leaves other events untouched", () => {
    const other = { ...event, scheduleId: "sched-2" };
    const updated = applyEventUpdate([event, other], event, new Date("2025-10-01T07:00:00Z"), new Date("2025-10-01T07:30:00Z"));
    expect(updated[1].start).toEqual(other.start);
    expect(updated[1].end).toEqual(other.end);
  });
});
