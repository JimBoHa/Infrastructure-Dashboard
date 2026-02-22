import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";

import SchedulesPage from "@/app/(dashboard)/schedules/page";

const mockUseSchedulesQuery = vi.fn();
const mockUseNodesQuery = vi.fn();
const mockUseSensorsQuery = vi.fn();
const mockUseOutputsQuery = vi.fn();

const mockSetQueryData = vi.fn();
const mockInvalidateQueries = vi.fn();
const mockUseQuery = vi.fn();

vi.mock("@tanstack/react-query", () => ({
  useQuery: (...args: unknown[]) => mockUseQuery(...args),
  useQueryClient: () => ({
    setQueryData: mockSetQueryData,
    invalidateQueries: mockInvalidateQueries,
  }),
}));

vi.mock("@/lib/queries", () => ({
  queryKeys: {
    schedules: ["schedules"],
    scheduleCalendar: (start: string, end: string) => ["schedules", "calendar", start, end],
  },
  useSchedulesQuery: () => mockUseSchedulesQuery(),
  useNodesQuery: () => mockUseNodesQuery(),
  useSensorsQuery: () => mockUseSensorsQuery(),
  useOutputsQuery: () => mockUseOutputsQuery(),
}));

// Provide a lightweight calendar stub that exercises the drop/resize handlers.
type CalendarEvent = {
  title: string;
  start: Date;
  end: Date;
  scheduleId: string;
};

vi.mock("react-big-calendar", () => ({
  __esModule: true,
  Calendar: (props: {
    events: CalendarEvent[];
    onEventDrop?: (args: { event: CalendarEvent; start: Date; end: Date; isAllDay: boolean }) => void;
    onEventResize?: (args: { event: CalendarEvent; start: Date; end: Date; isAllDay: boolean }) => void;
    onSelectSlot?: (slot: { start: Date; end: Date }) => void;
  }) => (
    <div>
      <div>Calendar Stub</div>
      <button
        onClick={() =>
          props.onEventDrop?.({
            event: props.events[0],
            start: new Date(2025, 0, 1, 6, 0, 0),
            end: new Date(2025, 0, 1, 6, 30, 0),
            isAllDay: false,
          })
        }
      >
        Drop
      </button>
      <button
        onClick={() =>
          props.onEventResize?.({
            event: props.events[0],
            start: new Date(2025, 0, 1, 7, 0, 0),
            end: new Date(2025, 0, 1, 7, 30, 0),
            isAllDay: false,
          })
        }
      >
        Resize
      </button>
      <button
        onClick={() =>
          props.onSelectSlot?.({
            start: new Date(2025, 0, 2, 8, 0, 0),
            end: new Date(2025, 0, 2, 9, 0, 0),
          })
        }
      >
        Select Slot
      </button>
    </div>
  ),
  dateFnsLocalizer: vi.fn(() => ({})),
}));

vi.mock("react-big-calendar/lib/addons/dragAndDrop", () => ({
  __esModule: true,
  default: <T,>(component: T) => component,
}));

describe("SchedulesPage calendar interactions", () => {
  beforeEach(() => {
    mockSetQueryData.mockClear();
    mockInvalidateQueries.mockClear();
    mockUseQuery.mockReturnValue({
      data: [
        {
          title: "Irrigation",
          start: new Date(2025, 0, 1, 5, 0, 0),
          end: new Date(2025, 0, 1, 5, 30, 0),
          scheduleId: "sched-1",
        },
      ],
    });
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({ id: "sched-1" }),
    }) as unknown as typeof fetch;
    mockUseSchedulesQuery.mockReturnValue({
      data: [
        {
          id: "sched-1",
          name: "Morning Irrigation",
          rrule: "FREQ=DAILY",
          blocks: [{ day: "WE", start: "05:00", end: "05:30" }],
          conditions: [],
          actions: [],
          next_run: "2025-01-01T05:00:00Z",
        },
      ],
      error: null,
      isLoading: false,
      isSuccess: true,
    });
    mockUseNodesQuery.mockReturnValue({ data: [], error: null, isLoading: false });
    mockUseSensorsQuery.mockReturnValue({ data: [], error: null, isLoading: false });
    mockUseOutputsQuery.mockReturnValue({ data: [], error: null, isLoading: false });
  });

  it("persists calendar events on drop and shows message", async () => {
    render(<SchedulesPage />);
    fireEvent.click(screen.getByText("Drop"));
    await waitFor(() => expect(mockSetQueryData).toHaveBeenCalled());
    const updatedEvents = mockSetQueryData.mock.calls[0][1] as CalendarEvent[];
    expect(updatedEvents[0].start.getHours()).toBe(6);
    expect(updatedEvents[0].end.getHours()).toBe(6);
    expect(updatedEvents[0].end.getMinutes()).toBe(30);
    await screen.findByText(/Schedule updated/i);
    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/schedules/sched-1"),
      expect.objectContaining({ method: "PUT" }),
    );
  });

  it("persists calendar events on resize", async () => {
    render(<SchedulesPage />);
    fireEvent.click(screen.getByText("Resize"));
    await waitFor(() => expect(mockSetQueryData).toHaveBeenCalled());
    const updatedEvents = mockSetQueryData.mock.calls[0][1] as CalendarEvent[];
    expect(updatedEvents[0].start.getHours()).toBe(7);
    expect(updatedEvents[0].end.getHours()).toBe(7);
    expect(updatedEvents[0].end.getMinutes()).toBe(30);
    await screen.findByText(/Schedule updated/i);
  });

  it("creates a new schedule from slot selection", async () => {
    render(<SchedulesPage />);
    fireEvent.click(screen.getByText("Select Slot"));
    expect(await screen.findByText(/Create schedule/i)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Save"));
    await screen.findByText(/Schedule created/i);
    expect(global.fetch).toHaveBeenCalledWith(
      expect.stringContaining("/api/schedules"),
      expect.objectContaining({ method: "POST" }),
    );
  });
});
