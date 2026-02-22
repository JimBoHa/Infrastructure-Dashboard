export type Message = { type: "success" | "error"; text: string };

export type HealthReport = {
  core_api?: { status?: string; message?: string };
  mqtt?: { status?: string; message?: string };
  database?: { status?: string; message?: string };
  redis?: { status?: string; message?: string };
  generated_at?: string;
};

