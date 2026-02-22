import type { SimLabFault, SimLabScenario, SimLabStatus } from "@/types/simLab";

export const SIM_LAB_API_BASE =
  process.env.NEXT_PUBLIC_SIM_LAB_API_BASE || "http://127.0.0.1:8100";

const normalizeBase = (value: string) => value.replace(/\/$/, "");

const simLabBaseNormalized = normalizeBase(SIM_LAB_API_BASE);

const simLabUrl = (path: string) =>
  path.startsWith("http") ? path : `${SIM_LAB_API_BASE}${path}`;

async function simLabFetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const url = simLabUrl(path);
  const response = await fetch(url, {
    next: { revalidate: 0 },
    ...init,
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(
      `Sim Lab request failed (${response.status}): ${text || response.statusText}`,
    );
  }
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

async function simLabMutate<T>(
  path: string,
  body?: unknown,
  method: "POST" | "PUT" | "PATCH" | "DELETE" = "POST",
): Promise<T> {
  return simLabFetchJson<T>(path, {
    method,
    headers: { "Content-Type": "application/json" },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
}

export const fetchSimLabStatus = () =>
  simLabFetchJson<SimLabStatus>("/sim-lab/status");

export const fetchSimLabScenarios = () =>
  simLabFetchJson<SimLabScenario[]>("/sim-lab/scenarios");

export const fetchSimLabFaults = () =>
  simLabFetchJson<SimLabFault[]>("/sim-lab/faults");

export const postSimLabArm = (armed: boolean, ttlSeconds?: number) =>
  simLabMutate<{ armed: boolean }>("/sim-lab/arm", {
    armed,
    ttl_seconds: ttlSeconds,
  });

export const postSimLabAction = (action: string) =>
  simLabMutate("/sim-lab/actions/" + action);

export const postSimLabSeed = (seed: number) =>
  simLabMutate("/sim-lab/actions/seed", { seed });

export const postSimLabTimeMultiplier = (multiplier: number) =>
  simLabMutate("/sim-lab/actions/time-multiplier", { multiplier });

export const postSimLabScenario = (scenarioId: string) =>
  simLabMutate(`/sim-lab/scenarios/${scenarioId}/apply`);

export const postSimLabFault = (payload: {
  kind: string;
  node_id?: string | null;
  sensor_id?: string | null;
  output_id?: string | null;
  config?: Record<string, unknown>;
}) => simLabMutate<SimLabFault>("/sim-lab/faults/apply", payload);

export const clearSimLabFault = (faultId: string) =>
  simLabMutate(`/sim-lab/faults/${faultId}/clear`);

export const clearSimLabFaults = () => simLabMutate("/sim-lab/faults/clear");

export const simLabBase = simLabBaseNormalized;
