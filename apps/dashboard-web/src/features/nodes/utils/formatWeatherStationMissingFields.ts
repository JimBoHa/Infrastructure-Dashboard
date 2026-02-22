const FIELD_LABELS: Record<string, string> = {
  temperature: "Temperature",
  humidity: "Humidity",
  wind_speed: "Wind speed",
  wind_gust: "Wind gust",
  wind_direction: "Wind direction",
  rain: "Rain",
  rain_rate: "Rain rate",
  uv: "UV index",
  solar_radiation: "Solar radiation",
  pressure_relative: "Barometric pressure (relative)",
  pressure_absolute: "Barometric pressure (absolute)",
};

export default function formatWeatherStationMissingFields(
  fields: string[] | null | undefined,
): string {
  if (!fields?.length) return "—";
  return fields.map((field) => FIELD_LABELS[field] ?? field).join(", ");
}

