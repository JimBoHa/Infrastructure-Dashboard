export type SimLabRunState = "running" | "paused" | "stopped";

export type SimLabScenario = {
  id: string;
  label: string;
  description: string;
};

export type SimLabFault = {
  id: string;
  kind: string;
  node_id?: string | null;
  sensor_id?: string | null;
  output_id?: string | null;
  config?: Record<string, unknown>;
};

export type SimLabStatus = {
  run_state: SimLabRunState;
  armed: boolean;
  armed_until?: string | null;
  active_scenario?: string | null;
  seed?: number | null;
  time_multiplier: number;
  fault_count: number;
  nodes: Array<{
    node_id: string;
    node_name: string;
    api_base: string;
  }>;
  updated_at: string;
};
