import { format } from "date-fns";
import { Event } from "react-big-calendar";
import type { DemoSchedule } from "@/types/dashboard";

export const DAY_CODES = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"] as const;
export type DayCode = (typeof DAY_CODES)[number];

export interface ScheduleEvent extends Event {
  scheduleId: string;
  name: string;
  title: string;
  start: Date;
  end: Date;
}

export type Toast = { type: "success" | "error" | "info"; text: string };

type ConditionFormKind =
  | "sensor"
  | "sensor_value_between"
  | "node_status"
  | "forecast"
  | "analytics";
type ActionFormKind = "output" | "alarm" | "mqtt_publish";

export interface ConditionFormState {
  type: ConditionFormKind;
  sensor_id?: string;
  operator?: string;
  threshold?: number;
  fail_open?: boolean;
  min?: number;
  max?: number;
  node_id?: string;
  status?: string;
  field?: string;
  horizon_hours?: number;
  key?: string;
  window_minutes?: number;
}

export interface ActionFormState {
  type: ActionFormKind;
  output_id?: string;
  state?: string;
  duration_seconds?: number;
  severity?: string;
  message?: string;
  topic?: string;
  payload?: string;
}

export interface ScheduleDraft {
  mode: "create" | "edit";
  scheduleId?: string;
  originalBlock?: { day: DayCode; start: string; end: string } | null;
  name: string;
  rrule: string;
  day: DayCode;
  start: string;
  end: string;
  conditionsList: Array<Record<string, unknown>>;
  actionsList: Array<Record<string, unknown>>;
  conditionsJson: string;
  actionsJson: string;
  conditionsMode: "form" | "json";
  actionsMode: "form" | "json";
  showValidation: boolean;
  editingConditionIndex: number | null;
  editingActionIndex: number | null;
}

export type ConditionFieldErrors = Partial<Record<keyof ConditionFormState, string>>;
export type ActionFieldErrors = Partial<Record<keyof ActionFormState, string>>;

export const toDate = (value: Date | string | number | undefined) =>
  value instanceof Date ? value : value ? new Date(value) : new Date();

const toDayCode = (value: Date): DayCode => DAY_CODES[value.getDay()];
const toTimeString = (value: Date) => format(value, "HH:mm");

export const asString = (value: unknown): string | undefined =>
  typeof value === "string" ? value : value != null ? String(value) : undefined;

export const asNumber = (value: unknown): number | undefined => {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return undefined;
};

const asBoolean = (value: unknown): boolean => {
  if (typeof value === "boolean") return value;
  if (typeof value === "string") return value === "true";
  if (typeof value === "number") return value !== 0;
  return false;
};

export const isBlank = (value: string | undefined | null): boolean =>
  !value || value.trim().length === 0;

export const parseTimeToMinutes = (value: string): number | null => {
  const match = /^(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const hours = Number(match[1]);
  const minutes = Number(match[2]);
  if (!Number.isInteger(hours) || !Number.isInteger(minutes)) return null;
  if (hours < 0 || hours > 23) return null;
  if (minutes < 0 || minutes > 59) return null;
  return hours * 60 + minutes;
};

export const hasFieldErrors = (errors: Record<string, string | undefined>): boolean =>
  Object.values(errors).some((value) => Boolean(value && value.trim()));

export const validateConditionForm = (value: ConditionFormState): ConditionFieldErrors => {
  const errors: ConditionFieldErrors = {};
  if ((value.type === "sensor" || value.type === "sensor_value_between") && isBlank(value.sensor_id)) {
    errors.sensor_id = "Select a sensor.";
  }
  if (value.type === "sensor") {
    if (isBlank(value.operator)) errors.operator = "Select an operator.";
    if (value.threshold == null || !Number.isFinite(value.threshold)) errors.threshold = "Enter a numeric threshold.";
  }
  if (value.type === "sensor_value_between") {
    const hasMin = value.min != null && Number.isFinite(value.min);
    const hasMax = value.max != null && Number.isFinite(value.max);
    if (!hasMin && !hasMax) {
      errors.min = "Enter at least a min or max.";
      errors.max = "Enter at least a min or max.";
    }
    if (hasMin && hasMax && (value.min as number) > (value.max as number)) {
      errors.min = "Min must be <= max.";
      errors.max = "Max must be >= min.";
    }
  }
  if (value.type === "node_status") {
    if (isBlank(value.node_id)) errors.node_id = "Select a node.";
    if (isBlank(value.status)) errors.status = "Enter a status (online/offline).";
  }
  if (value.type === "forecast") {
    if (isBlank(value.field)) errors.field = "Enter a forecast field (e.g., rain_mm).";
    if (isBlank(value.operator)) errors.operator = "Select an operator.";
    if (value.threshold == null || !Number.isFinite(value.threshold)) errors.threshold = "Enter a numeric threshold.";
    if (value.horizon_hours == null || !Number.isFinite(value.horizon_hours) || value.horizon_hours <= 0) {
      errors.horizon_hours = "Enter a positive horizon.";
    }
  }
  if (value.type === "analytics") {
    if (isBlank(value.key)) errors.key = "Enter an analytics key.";
    if (isBlank(value.operator)) errors.operator = "Select an operator.";
    if (value.threshold == null || !Number.isFinite(value.threshold)) errors.threshold = "Enter a numeric threshold.";
    if (value.window_minutes != null && (!Number.isFinite(value.window_minutes) || value.window_minutes <= 0)) {
      errors.window_minutes = "Enter a positive window or leave blank.";
    }
  }
  return errors;
};

export const validateActionForm = (value: ActionFormState): ActionFieldErrors => {
  const errors: ActionFieldErrors = {};
  if (value.type === "output") {
    if (isBlank(value.output_id)) errors.output_id = "Select an output.";
    if (isBlank(value.state)) errors.state = "Select a state.";
    if (value.duration_seconds != null && (!Number.isFinite(value.duration_seconds) || value.duration_seconds <= 0)) {
      errors.duration_seconds = "Enter a positive duration or leave blank.";
    }
  }
  if (value.type === "alarm") {
    if (isBlank(value.severity)) errors.severity = "Select a severity.";
    if (isBlank(value.message)) errors.message = "Enter a message.";
  }
  if (value.type === "mqtt_publish") {
    if (isBlank(value.topic)) errors.topic = "Enter a topic.";
  }
  return errors;
};

export function applyEventUpdate(events: ScheduleEvent[], event: ScheduleEvent, start: Date, end: Date) {
  return events.map((item) =>
    item === event
      ? {
          ...item,
          start,
          end,
        }
      : item,
  );
}

export function updateBlocksForEvent(
  schedule: DemoSchedule,
  event: ScheduleEvent,
  start: Date,
  end: Date,
): DemoSchedule["blocks"] | null {
  const blocks = schedule.blocks ?? [];
  if (!blocks.length) {
    return null;
  }
  const previousDay = toDayCode(event.start);
  const previousStart = toTimeString(event.start);
  const previousEnd = toTimeString(event.end);
  const nextDay = toDayCode(start);
  const nextStart = toTimeString(start);
  const nextEnd = toTimeString(end);

  let matched = false;
  const updated = blocks.map((block) => {
    if (!matched && block.day.toUpperCase() === previousDay && block.start === previousStart && block.end === previousEnd) {
      matched = true;
      return { ...block, day: nextDay, start: nextStart, end: nextEnd };
    }
    return block;
  });

  return matched ? updated : null;
}

const defaultBlock = (): { day: DayCode; start: string; end: string } => ({
  day: "MO",
  start: "06:00",
  end: "07:00",
});

export const emptyConditionForm = (): ConditionFormState => ({
  type: "sensor",
  sensor_id: "",
  operator: "<",
  threshold: 0,
  fail_open: false,
});

export const emptyActionForm = (): ActionFormState => ({
  type: "output",
  output_id: "",
  state: "on",
});

export const draftFromSlot = (start: Date, end: Date): ScheduleDraft => {
  const day = toDayCode(start);
  return {
    mode: "create",
    name: "New schedule",
    rrule: `FREQ=WEEKLY;BYDAY=${day}`,
    originalBlock: { day, start: toTimeString(start), end: toTimeString(end) },
    day,
    start: toTimeString(start),
    end: toTimeString(end),
    conditionsList: [],
    actionsList: [],
    conditionsJson: "[]",
    actionsJson: "[]",
    conditionsMode: "form",
    actionsMode: "form",
    showValidation: false,
    editingConditionIndex: null,
    editingActionIndex: null,
  };
};

export const draftFromSchedule = (schedule: DemoSchedule, event?: ScheduleEvent): ScheduleDraft => {
  const blocks = schedule.blocks ?? [];
  const referenceBlock = event
    ? { day: toDayCode(event.start), start: toTimeString(event.start), end: toTimeString(event.end) }
    : blocks[0] ?? defaultBlock();
  const day = (referenceBlock.day.toUpperCase() as DayCode) || "MO";
  const conditionsList = (schedule.conditions ?? []).map(
    (condition) => ({ ...condition }) as Record<string, unknown>,
  );
  const actionsList = (schedule.actions ?? []).map(
    (action) => ({ ...(action as Record<string, unknown>) }) as Record<string, unknown>,
  );
  return {
    mode: "edit",
    scheduleId: schedule.id,
    originalBlock: { day, start: referenceBlock.start, end: referenceBlock.end },
    name: schedule.name,
    rrule: schedule.rrule,
    day,
    start: referenceBlock.start,
    end: referenceBlock.end,
    conditionsList,
    actionsList,
    conditionsJson: JSON.stringify(conditionsList, null, 2),
    actionsJson: JSON.stringify(actionsList, null, 2),
    conditionsMode: "form",
    actionsMode: "form",
    showValidation: false,
    editingConditionIndex: null,
    editingActionIndex: null,
  };
};

const parseJsonArray = (value: string, label: string): unknown[] => {
  if (!value.trim()) return [];
  try {
    const parsed = JSON.parse(value);
    if (Array.isArray(parsed)) return parsed;
  } catch {
    throw new Error(`${label} must be valid JSON.`);
  }
  throw new Error(`${label} must be an array.`);
};

export const parseJsonObjectArray = (
  value: string,
  label: string,
): Array<Record<string, unknown>> => {
  const parsed = parseJsonArray(value, label);
  const objects = parsed.filter(
    (entry): entry is Record<string, unknown> => typeof entry === "object" && entry !== null && !Array.isArray(entry),
  );
  if (objects.length !== parsed.length) {
    throw new Error(`${label} must be an array of objects.`);
  }
  return objects;
};

export const conditionFormToPayload = (form: ConditionFormState): Record<string, unknown> => {
  switch (form.type) {
    case "sensor":
      return {
        type: "sensor",
        sensor_id: form.sensor_id ?? "",
        operator: form.operator ?? "<",
        threshold: form.threshold ?? 0,
        fail_open: Boolean(form.fail_open),
      };
    case "sensor_value_between":
      return {
        type: "sensor_value_between",
        sensor_id: form.sensor_id ?? "",
        min: form.min ?? null,
        max: form.max ?? null,
      };
    case "node_status":
      return {
        type: "node_status",
        node_id: form.node_id ?? "",
        status: form.status ?? "online",
      };
    case "forecast":
      return {
        type: "forecast",
        field: form.field ?? "rain_mm",
        operator: form.operator ?? "<=",
        threshold: form.threshold ?? 0,
        horizon_hours: form.horizon_hours ?? 24,
        fail_open: Boolean(form.fail_open),
      };
    case "analytics":
      return {
        type: "analytics",
        key: form.key ?? "power_kw",
        operator: form.operator ?? ">",
        threshold: form.threshold ?? 0,
        window_minutes: form.window_minutes ?? null,
        fail_open: Boolean(form.fail_open),
      };
    default:
      return {};
  }
};

export const conditionPayloadToForm = (payload: Record<string, unknown>): ConditionFormState | null => {
  const type = asString(payload.type);
  if (type === "sensor") {
    return {
      type: "sensor",
      sensor_id: asString(payload.sensor_id) ?? "",
      operator: asString(payload.operator) ?? "<",
      threshold: asNumber(payload.threshold) ?? 0,
      fail_open: asBoolean(payload.fail_open),
    };
  }
  if (type === "sensor_value_between") {
    return {
      type: "sensor_value_between",
      sensor_id: asString(payload.sensor_id) ?? "",
      min: asNumber(payload.min),
      max: asNumber(payload.max),
    };
  }
  if (type === "node_status") {
    return {
      type: "node_status",
      node_id: asString(payload.node_id) ?? "",
      status: asString(payload.status) ?? "online",
    };
  }
  if (type === "forecast") {
    return {
      type: "forecast",
      field: asString(payload.field) ?? "rain_mm",
      operator: asString(payload.operator) ?? "<=",
      threshold: asNumber(payload.threshold) ?? 0,
      horizon_hours: asNumber(payload.horizon_hours) ?? 24,
      fail_open: asBoolean(payload.fail_open),
    };
  }
  if (type === "analytics") {
    return {
      type: "analytics",
      key: asString(payload.key) ?? "power_kw",
      operator: asString(payload.operator) ?? ">",
      threshold: asNumber(payload.threshold) ?? 0,
      window_minutes: asNumber(payload.window_minutes),
      fail_open: asBoolean(payload.fail_open),
    };
  }
  return null;
};

export const actionFormToPayload = (form: ActionFormState): Record<string, unknown> => {
  switch (form.type) {
    case "output":
      return {
        type: "output",
        output_id: form.output_id ?? "",
        state: form.state ?? "on",
        duration_seconds: form.duration_seconds ?? null,
      };
    case "alarm":
      return {
        type: "alarm",
        severity: form.severity ?? "warning",
        message: form.message ?? "Automation alarm",
      };
    case "mqtt_publish":
      return {
        type: "mqtt_publish",
        topic: form.topic ?? "farm/topic",
        payload: form.payload ? safeParseJson(form.payload) ?? form.payload : {},
      };
    default:
      return {};
  }
};

export const actionPayloadToForm = (payload: Record<string, unknown>): ActionFormState | null => {
  const type = asString(payload.type);
  if (type === "output") {
    return {
      type: "output",
      output_id: asString(payload.output_id) ?? "",
      state: asString(payload.state) ?? "on",
      duration_seconds: asNumber(payload.duration_seconds),
    };
  }
  if (type === "alarm") {
    return {
      type: "alarm",
      severity: asString(payload.severity) ?? "warning",
      message: asString(payload.message) ?? "",
    };
  }
  if (type === "mqtt_publish") {
    const rawPayload = payload.payload;
    const payloadText =
      typeof rawPayload === "string"
        ? rawPayload
        : rawPayload == null
          ? ""
          : JSON.stringify(rawPayload, null, 2);
    return {
      type: "mqtt_publish",
      topic: asString(payload.topic) ?? "",
      payload: payloadText,
    };
  }
  return null;
};

const safeParseJson = (value: string): unknown => {
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
};
