export type RelatedSensorsExternalFocusSource = "matrix_profile";

export type RelatedSensorsExternalFocusWindow = {
  kind: "anomaly" | "motif";
  startIso: string;
  endIso: string | null;
  severity?: number | null;
};

export type RelatedSensorsExternalFocus = {
  source: RelatedSensorsExternalFocusSource;
  requestedAtMs: number;
  focusSensorId: string;
  windows: RelatedSensorsExternalFocusWindow[];
};

