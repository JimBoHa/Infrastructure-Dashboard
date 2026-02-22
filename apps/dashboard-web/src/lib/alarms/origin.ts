import type { AlarmOrigin } from "@/types/alarms";

export function isPredictiveOrigin(origin: AlarmOrigin | null | undefined): boolean {
  return Boolean(origin && origin !== "threshold");
}

export type AlarmOriginFilter = "all" | "predictive" | "standard";

export function matchesOriginFilter(
  origin: AlarmOrigin | null | undefined,
  filter: AlarmOriginFilter,
): boolean {
  if (filter === "all") return true;
  const predictive = isPredictiveOrigin(origin);
  return filter === "predictive" ? predictive : !predictive;
}
