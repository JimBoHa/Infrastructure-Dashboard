export type IncidentStatus = "open" | "snoozed" | "closed" | string;
export type IncidentSeverity = "critical" | "warning" | "info" | string;

export type Incident = {
  id: string;
  rule_id: string | null;
  target_key: string | null;
  severity: IncidentSeverity;
  status: IncidentStatus;
  title: string;
  assigned_to: string | null;
  snoozed_until: Date | null;
  first_event_at: Date;
  last_event_at: Date;
  closed_at: Date | null;
  created_at: Date;
  updated_at: Date;
  total_event_count: number;
  active_event_count: number;
  note_count: number;
  last_message?: string | null;
  last_origin?: string | null;
  last_sensor_id?: string | null;
  last_node_id?: string | null;
};

export type IncidentsListResponse = {
  incidents: Incident[];
  next_cursor: string | null;
};

export type IncidentDetailResponse = {
  incident: Incident;
  events: unknown[];
};

export type IncidentNote = {
  id: string;
  incident_id: string;
  created_by: string | null;
  body: string;
  created_at: Date;
};

export type IncidentNotesListResponse = {
  notes: IncidentNote[];
  next_cursor: string | null;
};

export type ActionLog = {
  id: string;
  schedule_id: string;
  action: unknown;
  status: string;
  message: string | null;
  created_at: Date;
  output_id: string | null;
  node_id: string | null;
};

