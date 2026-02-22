import { formatNumber } from "@/lib/format";
import type { DemoSensor } from "@/types/dashboard";

export const COV_INTERVAL_LABEL = "COV";
export const COV_INTERVAL_TOOLTIP = "Change of value (COV): recorded only when the value changes.";

export const formatSensorInterval = (
  intervalSeconds: number | null | undefined,
): { label: string; title?: string } => {
  if (intervalSeconds == null || !Number.isFinite(intervalSeconds)) {
    return { label: "—" };
  }
  if (intervalSeconds === 0) {
    return { label: COV_INTERVAL_LABEL, title: COV_INTERVAL_TOOLTIP };
  }
  return { label: `${Math.max(0, Math.floor(intervalSeconds))}s` };
};

export const getSensorDisplayDecimals = (sensor: Pick<DemoSensor, "config">): number | null => {
  const config = sensor.config ?? {};
  const raw = (config as Record<string, unknown>)["display_decimals"];
  const fromNumber = typeof raw === "number" && Number.isFinite(raw) ? Math.floor(raw) : null;
  const fromString =
    typeof raw === "string" && raw.trim().length > 0 ? Number.parseInt(raw.trim(), 10) : null;
  const value = fromNumber ?? fromString;
  if (value == null) return null;
  if (!Number.isFinite(value) || value < 0 || value > 6) return null;
  return value;
};

export const formatSensorValue = (sensor: Pick<DemoSensor, "config">, value: number): string => {
  if (!Number.isFinite(value)) return "—";
  const decimals = getSensorDisplayDecimals(sensor);
  if (decimals == null) {
    return formatNumber(value, { minimumFractionDigits: 0, maximumFractionDigits: 2 });
  }
  return formatNumber(value, { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
};

export const formatSensorValueWithUnit = (
  sensor: Pick<DemoSensor, "config" | "unit">,
  value?: number | null,
  placeholder = "—",
): string => {
  if (value == null || !Number.isFinite(value)) return placeholder;
  const unit = sensor.unit?.trim() ?? "";
  const formatted = formatSensorValue(sensor, value);
  return unit ? `${formatted} ${unit}` : formatted;
};
