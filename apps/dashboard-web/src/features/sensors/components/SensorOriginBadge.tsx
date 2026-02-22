import { configString, isDerivedSensor, isPublicProviderSensor, isWs2902UploadSensor } from "@/lib/sensorOrigin";
import type { DemoSensor } from "@/types/dashboard";

type BadgeSize = "xs" | "sm";

type BadgeMeta = {
  label: string;
  title: string;
};

const PUBLIC_PROVIDER_BADGE_LABEL = "PUBLIC";
const PUBLIC_PROVIDER_TITLE_PREFIX = "Public provider data";

function formatProviderLabel(provider: string | null): string | null {
  if (!provider) return null;
  if (provider === "open_meteo") return "Open-Meteo";
  if (provider === "forecast_solar") return "Forecast.Solar";
  return provider;
}

export function nonLocalSensorBadgeMeta(sensor: Pick<DemoSensor, "config">): BadgeMeta | null {
  const config = sensor.config ?? {};
  if (!isPublicProviderSensor(sensor)) return null;

  const provider = configString(config, "provider");
  const kind = configString(config, "kind");
  const mode = configString(config, "mode");

  const providerLabel = formatProviderLabel(provider);
  const kindLabel = kind === "pv" ? "PV" : kind === "weather" ? "weather" : kind;
  const modeLabel = mode ? ` (${mode})` : "";

  const title = (() => {
    if (providerLabel && kindLabel) return `${PUBLIC_PROVIDER_TITLE_PREFIX}: ${providerLabel} ${kindLabel}${modeLabel}`.trim();
    if (providerLabel) return `${PUBLIC_PROVIDER_TITLE_PREFIX}: ${providerLabel}${modeLabel}`.trim();
    if (kindLabel) return `${PUBLIC_PROVIDER_TITLE_PREFIX}: ${kindLabel}${modeLabel}`.trim();
    return PUBLIC_PROVIDER_TITLE_PREFIX;
  })();

  return { label: PUBLIC_PROVIDER_BADGE_LABEL, title };
}

export function sensorOriginBadgeMeta(sensor: Pick<DemoSensor, "config">): BadgeMeta | null {
  if (isPublicProviderSensor(sensor)) {
    return nonLocalSensorBadgeMeta(sensor);
  }
  if (isWs2902UploadSensor(sensor)) {
    return { label: "WS LOCAL", title: "Local: WS‑2902 weather station upload" };
  }
  if (isDerivedSensor(sensor)) {
    return { label: "DERIVED", title: "Derived sensor (computed from other sensors)" };
  }

  return null;
}

export default function SensorOriginBadge({
  sensor,
  size = "sm",
}: {
  sensor: Pick<DemoSensor, "config">;
  size?: BadgeSize;
}) {
  const meta = sensorOriginBadgeMeta(sensor);
  if (!meta) return null;

  const sizeClassName =
    size === "xs" ? "px-1.5 py-0.5 text-[10px]" : "px-2 py-0.5 text-[11px]";

  return (
    <span
      data-testid="sensor-origin-badge"
      title={meta.title}
      className={[
        "inline-flex items-center rounded-full font-semibold tracking-wide",
        sizeClassName,
 "bg-sky-100 text-sky-800",
      ].join(" ")}
    >
      {meta.label}
    </span>
  );
}
