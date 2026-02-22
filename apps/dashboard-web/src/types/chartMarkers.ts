export type AnalysisSource =
  | "matrix_profile"
  | "tsse"
  | "correlation"
  | "event_match"
  | "cooccurrence"
  | "voltage_quality";

export type EphemeralMarker = {
  id: string;
  timestamp: Date;
  label: string;
  source: AnalysisSource;
  detail?: string;
  sensorIds?: string[];
  timeEnd?: Date;
};

export type ChartAnnotation = {
  id: string;
  chart_state: Record<string, unknown>;
  sensor_ids?: string[];
  time_start?: string;
  time_end?: string;
  label?: string;
  created_by?: string;
  created_at: string;
  updated_at: string;
};
