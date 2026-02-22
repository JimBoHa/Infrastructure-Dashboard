export function formatDuration(seconds?: number | null) {
  if (!seconds && seconds !== 0) return "—";
  const units = [
    { label: "d", value: 86400 },
    { label: "h", value: 3600 },
    { label: "m", value: 60 },
  ];
  const parts: string[] = [];
  let remaining = seconds;
  for (const unit of units) {
    if (remaining >= unit.value || unit.label === "m") {
      const qty = Math.floor(remaining / unit.value);
      if (qty > 0 || unit.label === "m") {
        parts.push(`${qty}${unit.label}`);
      }
      remaining -= qty * unit.value;
    }
  }
  return parts.join(" ");
}

export function formatBytes(bytes?: number | null) {
  if (!bytes && bytes !== 0) return "—";
  const thresh = 1024;
  if (Math.abs(bytes) < thresh) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let u = -1;
  let value = bytes;
  do {
    value /= thresh;
    ++u;
  } while (Math.abs(value) >= thresh && u < units.length - 1);
  return `${value.toFixed(1)} ${units[u]}`;
}

export const formatPercent = (value?: number | null) =>
  value || value === 0 ? `${value.toFixed(1)}%` : "—";

const DEFAULT_NUMBER_OPTIONS: Intl.NumberFormatOptions = {
  minimumFractionDigits: 1,
  maximumFractionDigits: 1,
};

export const formatNumber = (
  value: number,
  options: Intl.NumberFormatOptions = DEFAULT_NUMBER_OPTIONS,
) =>
  Number.isFinite(value)
    ? value.toLocaleString(undefined, options)
    : Number(0).toLocaleString(undefined, options);

export const formatKw = (value: number) => `${formatNumber(value)} kW`;

export const formatKwh = (value: number) =>
  `${formatNumber(value, DEFAULT_NUMBER_OPTIONS)} kWh`;

export const formatWatts = (value: number) => {
  if (!Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  if (abs >= 1000) return `${formatNumber(value / 1000)} kW`;
  return `${formatNumber(value, { minimumFractionDigits: 0, maximumFractionDigits: 0 })} W`;
};

export const formatVolts = (value: number) =>
  Number.isFinite(value) ? `${formatNumber(value)} V` : "—";

export const formatAmps = (value: number) =>
  Number.isFinite(value) ? `${formatNumber(value)} A` : "—";

export const formatGallons = (value: number) =>
  `${formatNumber(value, { minimumFractionDigits: 0, maximumFractionDigits: 0 })} gal`;

export const formatCurrencyValue = (value: number, currency?: string) => {
  if (!Number.isFinite(value)) return "-";
  try {
    return new Intl.NumberFormat(undefined, {
      style: "currency",
      currency: currency ?? "USD",
      maximumFractionDigits: value >= 100 ? 0 : 2,
    }).format(value);
  } catch {
    return `$${value.toFixed(0)}`;
  }
};

export const formatRate = (value: number, currency?: string) => {
  if (!Number.isFinite(value) || value === 0) return "-";
  const formatted = formatCurrencyValue(value, currency);
  return `${formatted}/kWh`;
};

export const formatRuntime = (hours: number) => {
  if (!Number.isFinite(hours) || hours <= 0) return "-";
  if (hours >= 24) {
    const days = Math.floor(hours / 24);
    const remainingHours = Math.round(hours % 24);
    return `${days}d ${remainingHours}h`;
  }
  return `${formatNumber(hours, DEFAULT_NUMBER_OPTIONS)} h`;
};
