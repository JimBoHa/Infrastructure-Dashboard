import { fetchJson, postJson } from "@/lib/api";

import {
  parseControllerRuntimeConfig,
  parseSetupDaemonConfig,
  parseSetupDaemonLocalIp,
  parseSetupDaemonPreflight,
  type ControllerRuntimeConfig,
  type SetupDaemonConfig,
  type SetupDaemonLocalIp,
  type SetupDaemonPreflightCheck,
} from "../lib/setupDaemonParsers";

export const setupDaemonPath = (path: string) =>
  path.startsWith("/")
    ? `/api/setup-daemon${path}`
    : `/api/setup-daemon/${path}`;

export const fetchSetupDaemonConfig = async (): Promise<SetupDaemonConfig> => {
  const payload = await fetchJson<unknown>(setupDaemonPath("config"));
  return parseSetupDaemonConfig(payload);
};

export const postSetupDaemonConfigPatch = async (
  patch: Record<string, unknown>,
): Promise<SetupDaemonConfig> => {
  const payload = await postJson<unknown>(setupDaemonPath("config"), patch);
  return parseSetupDaemonConfig(payload);
};

export const fetchControllerRuntimeConfig = async (): Promise<ControllerRuntimeConfig | null> => {
  try {
    const payload = await fetchJson<unknown>("/api/setup/controller/runtime-config");
    return parseControllerRuntimeConfig(payload);
  } catch {
    return null;
  }
};

export const postControllerRuntimeConfigPatch = async (
  patch: Record<string, unknown>,
): Promise<ControllerRuntimeConfig> => {
  const payload = await postJson<unknown>("/api/setup/controller/runtime-config", patch);
  return parseControllerRuntimeConfig(payload);
};

export const fetchSetupDaemonPreflightChecks = async (): Promise<SetupDaemonPreflightCheck[]> => {
  const payload = await fetchJson<unknown>(setupDaemonPath("preflight"));
  return parseSetupDaemonPreflight(payload);
};

export const fetchSetupDaemonLocalIp = async (): Promise<SetupDaemonLocalIp> => {
  const payload = await fetchJson<unknown>(setupDaemonPath("local-ip"));
  return parseSetupDaemonLocalIp(payload);
};

export const runSetupDaemonAction = async (
  endpoint: string,
  body: Record<string, unknown> = {},
): Promise<unknown> => postJson<unknown>(setupDaemonPath(endpoint), body);

