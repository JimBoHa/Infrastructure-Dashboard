import type { DemoSensor } from "@/types/dashboard";

export function configString(config: Record<string, unknown>, key: string): string | null {
  const value = config[key];
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed.length ? trimmed : null;
}

export function sensorSource(sensor: Pick<DemoSensor, "config">): string | null {
  const config = (sensor.config ?? {}) as Record<string, unknown>;
  return configString(config, "source");
}

export function sensorMetric(sensor: Pick<DemoSensor, "config">): string | null {
  const config = (sensor.config ?? {}) as Record<string, unknown>;
  return configString(config, "metric");
}

export function findSensor<T extends Pick<DemoSensor, "config">>(
  sensors: T[],
  source: string,
  metric: string,
): T | null {
  const src = source.trim();
  const met = metric.trim();
  if (!src || !met) return null;
  return sensors.find((sensor) => sensorSource(sensor) === src && sensorMetric(sensor) === met) ?? null;
}

export type SensorOriginKind = "local" | "derived" | "public_provider";

export function sensorOriginKind(sensor: Pick<DemoSensor, "config">): SensorOriginKind {
  const source = sensorSource(sensor);
  if (source === "forecast_points") return "public_provider";
  if (source === "derived") return "derived";
  return "local";
}

export function isPublicProviderSensor(sensor: Pick<DemoSensor, "config">): boolean {
  return sensorOriginKind(sensor) === "public_provider";
}

export function isDerivedSensor(sensor: Pick<DemoSensor, "config">): boolean {
  return sensorOriginKind(sensor) === "derived";
}

export function isWs2902UploadSensor(sensor: Pick<DemoSensor, "config">): boolean {
  return sensorSource(sensor) === "ws_2902";
}
