import type { TrendSeriesEntry } from "@/types/dashboard";

export function computeDomain(minField: string, maxField: string) {
  const trimmedMin = minField.trim();
  const trimmedMax = maxField.trim();
  const parsedMin = trimmedMin !== "" ? Number(trimmedMin) : undefined;
  const parsedMax = trimmedMax !== "" ? Number(trimmedMax) : undefined;
  return {
    min: parsedMin !== undefined && !Number.isNaN(parsedMin) ? parsedMin : undefined,
    max: parsedMax !== undefined && !Number.isNaN(parsedMax) ? parsedMax : undefined,
  };
}

export function exportCsv(sensorIds: string[], series: TrendSeriesEntry[]) {
  if (!series.length) return;

  const csvEscape = (value: string): string => {
    if (!/[",\n\r]/.test(value)) return value;
    return `"${value.replace(/"/g, '""')}"`;
  };

  const header = ["sensor_id", "timestamp", "value", "samples"].join(",");
  const rows = series
    .filter((item) => sensorIds.length === 0 || sensorIds.includes(item.sensor_id))
    .flatMap((item) =>
      item.points.map((point) =>
        [
          csvEscape(item.sensor_id),
          csvEscape(point.timestamp instanceof Date ? point.timestamp.toISOString() : String(point.timestamp ?? "")),
          point.value == null ? "" : String(point.value),
          String(point.samples ?? 1),
        ].join(","),
      ),
    )
    .join("\n");
  const blob = new Blob([`${header}\n${rows}`], { type: "text/csv" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  const fileName = sensorIds.length ? sensorIds.join("-") : "trends";
  link.download = `${fileName}.csv`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}
