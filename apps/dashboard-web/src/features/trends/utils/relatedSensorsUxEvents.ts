"use client";

export type RelatedSensorsUxEventName =
  | "panel_opened"
  | "run_started"
  | "run_completed"
  | "candidate_opened"
  | "episode_selected"
  | "add_to_chart_clicked"
  | "refine_clicked"
  | "pin_toggled"
  | "jump_to_timestamp_clicked";

export type RelatedSensorsUxEventV1 = {
  schema: "fd.related_sensors.ux.v1";
  ts: string;
  session_id: string;
  name: RelatedSensorsUxEventName;
  payload: Record<string, unknown>;
};

type RelatedSensorsUxEventWindow = Window & {
  __fd_related_sensors_ux_session_id?: string;
  __fd_related_sensors_ux_events?: RelatedSensorsUxEventV1[];
};

const UX_EVENTS_ENABLED =
  typeof window !== "undefined" &&
  (process.env.NODE_ENV === "development" ||
    process.env.NEXT_PUBLIC_RELATED_SENSORS_UX_EVENTS === "1");

function getSessionId(): string {
  const w = window as RelatedSensorsUxEventWindow;
  if (w.__fd_related_sensors_ux_session_id) return w.__fd_related_sensors_ux_session_id;

  const candidate =
    globalThis.crypto?.randomUUID?.() ??
    `${Date.now().toString(16)}-${Math.random().toString(16).slice(2)}`;
  w.__fd_related_sensors_ux_session_id = candidate;
  return candidate;
}

export function emitRelatedSensorsUxEvent(
  name: RelatedSensorsUxEventName,
  payload: Record<string, unknown>,
): void {
  if (!UX_EVENTS_ENABLED) return;

  const event: RelatedSensorsUxEventV1 = {
    schema: "fd.related_sensors.ux.v1",
    ts: new Date().toISOString(),
    session_id: getSessionId(),
    name,
    payload,
  };

  const w = window as RelatedSensorsUxEventWindow;
  if (!w.__fd_related_sensors_ux_events) w.__fd_related_sensors_ux_events = [];
  w.__fd_related_sensors_ux_events.push(event);

  // Structured console output for local validation; safe because it contains no raw sensor values.
  // Keep it one-line so it can be copied into scripts/grep easily.
  console.info("[ux][related_sensors]", JSON.stringify(event));
}
