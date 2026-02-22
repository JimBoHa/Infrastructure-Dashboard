"use client";

import { useMemo, useState } from "react";
import { formatNumber, formatPercent } from "@/lib/format";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { useMetricsQuery } from "@/lib/queries";
import { formatSensorValueWithUnit } from "@/lib/sensorFormat";
import type { DemoNode, DemoSensor, TrendSeriesEntry } from "@/types/dashboard";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  AnalyticsRangeSelect,
  type AnalyticsHistoryRangeHours,
  historyIntervalSeconds,
  historyRangeLabel,
  useBaseChartOptions,
  ZoomableLineChart,
  buildChartData,
} from "@/features/analytics/components/AnalyticsShared";
import { useAnalyticsData } from "@/features/analytics/hooks/useAnalyticsData";

const WEATHER_STATION_KIND = "ws-2902";
const WEATHER_STATION_SENSOR_TYPES = [
  "temperature",
  "humidity",
  "wind_speed",
  "wind_gust",
  "wind_direction",
  "rain",
  "rain_rate",
  "uv",
  "solar_radiation",
  "pressure",
] as const;

type WeatherStationSensorType = (typeof WEATHER_STATION_SENSOR_TYPES)[number];

export function WeatherStationSection() {
  const { nodes, sensorsByNodeId, isLoading, error } = useAnalyticsData();
  const [rangeHours, setRangeHours] = useState<AnalyticsHistoryRangeHours>(24);

  const nodeKind = (node: DemoNode): string | null => {
    const config = node.config ?? {};
    const kind = config["kind"];
    return typeof kind === "string" ? kind : null;
  };

  const stationNodes = useMemo(() => {
    return nodes.filter((node) => nodeKind(node) === WEATHER_STATION_KIND);
  }, [nodes]);

  const errorMessage = error instanceof Error ? error.message : null;

  return (
    <Card>
      <CardHeader>
        <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
          <div className="space-y-1">
            <CardTitle className="text-lg">Weather stations</CardTitle>
 <p className="text-sm text-muted-foreground">
              WS-2902 station telemetry (live readings + history). Expand a station to view charts.
            </p>
          </div>
          <AnalyticsRangeSelect value={rangeHours} onChange={setRangeHours} />
        </div>
      </CardHeader>
      <CardContent>
        {errorMessage ? (
          <p className="text-sm text-rose-600">Failed to load weather stations: {errorMessage}</p>
        ) : isLoading ? (
 <p className="text-sm text-muted-foreground">Loading weather stations…</p>
        ) : stationNodes.length === 0 ? (
 <p className="text-sm text-muted-foreground">
            No weather station nodes detected yet. Add a WS-2902 node and apply the weather station preset to see live data here.
          </p>
        ) : (
          <div className="space-y-3">
            {stationNodes.map((node, idx) => (
              <WeatherStationNodePanel
                key={node.id}
                node={node}
                sensors={sensorsByNodeId.get(node.id) ?? []}
                defaultOpen={stationNodes.length === 1 && idx === 0}
                rangeHours={rangeHours}
              />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

function WeatherStationNodePanel({
  node,
  sensors,
  defaultOpen,
  rangeHours,
}: {
  node: DemoNode;
  sensors: DemoSensor[];
  defaultOpen: boolean;
  rangeHours: AnalyticsHistoryRangeHours;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const baseChartOptions = useBaseChartOptions(rangeHours);

  const sensorsByType = useMemo(() => {
    const map = new Map<WeatherStationSensorType, DemoSensor>();
    sensors.forEach((sensor) => {
      const type = sensor.type as WeatherStationSensorType;
      if (!WEATHER_STATION_SENSOR_TYPES.includes(type)) return;
      if (!map.has(type)) map.set(type, sensor);
    });
    return map;
  }, [sensors]);

  const metricSensorIds = useMemo(() => {
    return WEATHER_STATION_SENSOR_TYPES.map((type) => sensorsByType.get(type)?.sensor_id).filter(
      Boolean,
    ) as string[];
  }, [sensorsByType]);

  const metricsQuery = useMetricsQuery({
    sensorIds: metricSensorIds,
    rangeHours,
    interval: historyIntervalSeconds(rangeHours),
    enabled: open && metricSensorIds.length > 0,
    refetchInterval: 60_000,
  });

  const seriesBySensorId = useMemo(() => {
    const map = new Map<string, TrendSeriesEntry>();
    (metricsQuery.data ?? []).forEach((series) => map.set(series.sensor_id, series));
    return map;
  }, [metricsQuery.data]);

  const seriesForType = (type: WeatherStationSensorType) => {
    const sensor = sensorsByType.get(type);
    if (!sensor) return null;
    return seriesBySensorId.get(sensor.sensor_id) ?? null;
  };

  const temperatureSensor = sensorsByType.get("temperature") ?? null;
  const humiditySensor = sensorsByType.get("humidity") ?? null;
  const windSpeedSensor = sensorsByType.get("wind_speed") ?? null;
  const windGustSensor = sensorsByType.get("wind_gust") ?? null;
  const windDirSensor = sensorsByType.get("wind_direction") ?? null;
  const rainSensor = sensorsByType.get("rain") ?? null;
  const rainRateSensor = sensorsByType.get("rain_rate") ?? null;
  const uvSensor = sensorsByType.get("uv") ?? null;
  const solarRadiationSensor = sensorsByType.get("solar_radiation") ?? null;
  const pressureSensor = sensorsByType.get("pressure") ?? null;

  const customWsSensors = useMemo(() => {
    return sensors
      .map((sensor) => {
        const config = sensor.config ?? {};
        const customSource = typeof config["source"] === "string" ? config["source"].trim() : "";
        const field = typeof config["ws_field"] === "string" ? config["ws_field"].trim() : "";
        if (customSource !== "ws_2902" || !field) return null;
        return { sensor, field };
      })
      .filter((entry): entry is { sensor: DemoSensor; field: string } => entry != null)
      .sort((a, b) => a.sensor.name.localeCompare(b.sensor.name));
  }, [sensors]);

  const formatSensorValue = ({
    value,
    unit,
    decimals,
    unitOverride,
  }: {
    value: number | null | undefined;
    unit: string | null | undefined;
    decimals: number;
    unitOverride?: string;
  }) => {
    if (value == null || !Number.isFinite(value)) return "—";
    const formatted = formatNumber(value, {
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    });
    const suffix = unitOverride ?? unit ?? "";
    if (!suffix) return formatted;
    if (suffix === "%") return `${formatted}%`;
    return `${formatted} ${suffix}`;
  };

  const tempChip = temperatureSensor
    ? formatSensorValue({
        value: temperatureSensor.latest_value,
        unit: temperatureSensor.unit,
        decimals: 1,
      })
    : null;
  const humidityChip = humiditySensor ? formatPercent(humiditySensor.latest_value ?? null) : null;
  const windChip = windSpeedSensor
    ? formatSensorValue({ value: windSpeedSensor.latest_value, unit: windSpeedSensor.unit, decimals: 1 })
    : null;

  const statusLabel = formatNodeStatusLabel(node.status ?? "unknown", node.last_seen);
  const rangeSuffix = historyRangeLabel(rangeHours).toLowerCase();

  const tempSeries = seriesForType("temperature");
  const humiditySeries = seriesForType("humidity");
  const windSpeedSeries = seriesForType("wind_speed");
  const windGustSeries = seriesForType("wind_gust");
  const pressureSeries = seriesForType("pressure");
  const rainSeries = seriesForType("rain");
  const rainRateSeries = seriesForType("rain_rate");
  const uvSeries = seriesForType("uv");
  const solarRadiationSeries = seriesForType("solar_radiation");

  const tempHumidityData = (() => {
    const datasets: Array<{
      label: string;
      data: Array<{ x: Date; y: number | null }>;
      borderColor: string;
      backgroundColor: string;
      borderWidth: number;
      pointRadius: number;
      pointHoverRadius: number;
      tension: number;
      yAxisID?: string;
      borderDash?: number[];
      fill?: boolean;
    }> = [];
    if (tempSeries?.points?.length) {
      datasets.push({
        label: `Temperature (${tempSeries.unit ?? temperatureSensor?.unit ?? "°C"})`,
        data: tempSeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#2563eb",
        backgroundColor: "#2563eb",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        yAxisID: "y",
      });
    }
    if (humiditySeries?.points?.length) {
      datasets.push({
        label: `Humidity (${humiditySeries.unit ?? humiditySensor?.unit ?? "%"})`,
        data: humiditySeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#14b8a6",
        backgroundColor: "#14b8a6",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        yAxisID: "y1",
      });
    }
    return { datasets };
  })();

  const windData = (() => {
    const datasets: Array<{
      label: string;
      data: Array<{ x: Date; y: number | null }>;
      borderColor: string;
      backgroundColor: string;
      borderWidth: number;
      pointRadius: number;
      pointHoverRadius: number;
      tension: number;
      yAxisID?: string;
      borderDash?: number[];
      fill?: boolean;
    }> = [];
    if (windSpeedSeries?.points?.length) {
      datasets.push({
        label: "Wind speed",
        data: windSpeedSeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#0ea5e9",
        backgroundColor: "#0ea5e9",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
      });
    }
    if (windGustSeries?.points?.length) {
      datasets.push({
        label: "Wind gust",
        data: windGustSeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#0284c7",
        backgroundColor: "#0284c7",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        borderDash: [6, 4],
      });
    }
    return { datasets };
  })();

  const rainData = (() => {
    const datasets: Array<{
      label: string;
      data: Array<{ x: Date; y: number | null }>;
      borderColor: string;
      backgroundColor: string;
      borderWidth: number;
      pointRadius: number;
      pointHoverRadius: number;
      tension: number;
      yAxisID?: string;
      borderDash?: number[];
      fill?: boolean;
    }> = [];
    if (rainRateSeries?.points?.length) {
      datasets.push({
        label: `Rain rate (${rainRateSeries.unit ?? rainRateSensor?.unit ?? "mm/h"})`,
        data: rainRateSeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#0ea5e9",
        backgroundColor: "#0ea5e9",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        yAxisID: "y",
      });
    }
    if (rainSeries?.points?.length) {
      datasets.push({
        label: `Daily rain (${rainSeries.unit ?? rainSensor?.unit ?? "mm"})`,
        data: rainSeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#64748b",
        backgroundColor: "rgba(100,116,139,0.25)",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        yAxisID: "y1",
        fill: true,
      });
    }
    return { datasets };
  })();

  const solarUvData = (() => {
    const datasets: Array<{
      label: string;
      data: Array<{ x: Date; y: number | null }>;
      borderColor: string;
      backgroundColor: string;
      borderWidth: number;
      pointRadius: number;
      pointHoverRadius: number;
      tension: number;
      yAxisID?: string;
      borderDash?: number[];
      fill?: boolean;
    }> = [];
    if (solarRadiationSeries?.points?.length) {
      datasets.push({
        label: `Solar radiation (${solarRadiationSeries.unit ?? solarRadiationSensor?.unit ?? "W/m²"})`,
        data: solarRadiationSeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#f59e0b",
        backgroundColor: "#f59e0b",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        yAxisID: "y",
      });
    }
    if (uvSeries?.points?.length) {
      datasets.push({
        label: "UV index",
        data: uvSeries.points.map((p) => ({ x: p.timestamp, y: p.value })),
        borderColor: "#a855f7",
        backgroundColor: "#a855f7",
        borderWidth: 2,
        pointRadius: 0,
        pointHoverRadius: 3,
        tension: 0.25,
        yAxisID: "y1",
      });
    }
    return { datasets };
  })();

  const pressureData =
    pressureSeries?.points?.length
      ? buildChartData([
          {
            label: `Pressure (${pressureSeries.unit ?? pressureSensor?.unit ?? ""})`,
            series: pressureSeries.points,
            color: "#64748b",
          },
        ])
      : null;

  const directionDegrees = windDirSensor?.latest_value ?? null;

  return (
    <CollapsibleCard
      title={node.name}
      description={statusLabel}
      actions={<>
        {tempChip ? <span className="rounded-full bg-muted px-3 py-1 text-xs font-semibold text-foreground">{tempChip}</span> : null}
        {humidityChip ? <span className="rounded-full bg-muted px-3 py-1 text-xs font-semibold text-foreground">{humidityChip}</span> : null}
        {windChip ? <span className="rounded-full bg-muted px-3 py-1 text-xs font-semibold text-foreground">{windChip}</span> : null}
      </>}
      open={open}
      onOpenChange={setOpen}
      density="sm"
    >
        <div className="space-y-4">
          <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_320px]">
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Live readings</CardTitle>
              </CardHeader>
              <CardContent>
                <dl className="grid grid-cols-2 gap-x-6 gap-y-4 text-sm md:grid-cols-3 xl:grid-cols-4">
                  <Metric
                  label="Temperature"
                  value={
                    temperatureSensor
                      ? formatSensorValue({
                          value: temperatureSensor.latest_value,
                          unit: temperatureSensor.unit,
                          decimals: 1,
                        })
                      : "—"
                  }
                />
                <Metric
                  label="Humidity"
                  value={humiditySensor ? formatPercent(humiditySensor.latest_value ?? null) : "—"}
                />
                <Metric
                  label="Pressure"
                  value={
                    pressureSensor
                      ? formatSensorValue({
                          value: pressureSensor.latest_value,
                          unit: pressureSensor.unit,
                          decimals: 1,
                        })
                      : "—"
                  }
                />
                <Metric
                  label="Wind"
                  value={
                    windSpeedSensor
                      ? formatSensorValue({
                          value: windSpeedSensor.latest_value,
                          unit: windSpeedSensor.unit,
                          decimals: 1,
                        })
                      : "—"
                  }
                />
                <Metric
                  label="Wind gust"
                  value={
                    windGustSensor
                      ? formatSensorValue({
                          value: windGustSensor.latest_value,
                          unit: windGustSensor.unit,
                          decimals: 1,
                        })
                      : "—"
                  }
                />
                <Metric
                  label="Rain rate"
                  value={
                    rainRateSensor
                      ? formatSensorValue({
                          value: rainRateSensor.latest_value,
                          unit: rainRateSensor.unit,
                          decimals: 1,
                        })
                      : "—"
                  }
                />
                <Metric
                  label="Daily rain"
                  value={
                    rainSensor
                      ? formatSensorValue({
                          value: rainSensor.latest_value,
                          unit: rainSensor.unit,
                          decimals: 1,
                        })
                      : "—"
                  }
                />
                <Metric
                  label="Solar radiation"
                  value={
                    solarRadiationSensor
                      ? formatSensorValue({
                          value: solarRadiationSensor.latest_value,
                          unit: solarRadiationSensor.unit,
                          decimals: 0,
                        })
                      : "—"
                  }
                />
                <Metric
                  label="UV index"
                  value={
                    uvSensor
                      ? formatSensorValue({
                          value: uvSensor.latest_value,
                          unit: uvSensor.unit,
                          decimals: 1,
                          unitOverride: "UV",
                        })
                      : "—"
                  }
                />
                </dl>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Wind direction</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex items-center gap-4">
                  <WindDirectionCompass degrees={directionDegrees} />
 <div className="text-sm text-muted-foreground">
                    <div className="font-semibold text-card-foreground">
                      {directionDegrees == null || !Number.isFinite(directionDegrees)
                        ? "—"
                        : `${Math.round(directionDegrees)}°`}
                    </div>
 <div className="text-xs text-muted-foreground">0°=N · 90°=E · 180°=S · 270°=W</div>
                  </div>
                </div>
              </CardContent>
            </Card>
          </div>

          {customWsSensors.length ? (
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Custom sensors</CardTitle>
 <p className="text-xs text-muted-foreground">
                  Sensors mapped from station upload fields (e.g., soil moisture probes).
                </p>
              </CardHeader>
              <CardContent>
                <div className="overflow-x-auto md:overflow-x-visible">
                <table className="min-w-full divide-y divide-border text-sm">
                  <thead className="bg-card-inset">
                    <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Sensor
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Upload field
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Type
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Latest
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {customWsSensors.map(({ sensor, field }) => (
                      <tr key={sensor.sensor_id}>
                        <td className="px-3 py-2">
                          <div className="font-medium text-card-foreground">
                            {sensor.name}
                          </div>
 <div className="text-[11px] text-muted-foreground">
                            {sensor.sensor_id}
                          </div>
                        </td>
 <td className="px-3 py-2 font-mono text-xs text-muted-foreground">
                          {field}
                        </td>
 <td className="px-3 py-2 text-muted-foreground">
                          {sensor.type} · {sensor.unit}
                        </td>
 <td className="px-3 py-2 text-muted-foreground">
                          {formatSensorValueWithUnit(sensor, sensor.latest_value, "—")}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                </div>
              </CardContent>
            </Card>
          ) : null}

          {metricsQuery.error ? (
            <p className="text-sm text-rose-600">
              Failed to load weather station trends:{" "}
              {(metricsQuery.error instanceof Error && metricsQuery.error.message) || "Unknown error"}
            </p>
          ) : metricsQuery.isLoading ? (
 <p className="text-sm text-muted-foreground">Loading weather station trends…</p>
          ) : (
            <div className="grid gap-4 [@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-2">
              <Card>
                <CardHeader>
                  <CardTitle className="text-sm">Temperature & humidity — {rangeSuffix}</CardTitle>
                </CardHeader>
                <CardContent>
                  {!tempHumidityData.datasets.length ? (
 <p className="text-sm text-muted-foreground">
                      No temperature/humidity history available.
                    </p>
                  ) : (
                    <ZoomableLineChart
                      wrapperClassName="h-[400px]"
                      data={tempHumidityData}
                      options={{
                        ...baseChartOptions,
                        plugins: {
                          ...baseChartOptions.plugins,
                          legend: { display: true, position: "bottom" as const },
                        },
                        scales: {
                          ...baseChartOptions.scales,
                          y: {
                            ...baseChartOptions.scales.y,
                            beginAtZero: false,
                            title: {
                              display: true,
                              text: tempSeries?.unit ?? temperatureSensor?.unit ?? "°C",
                            },
                          },
                          y1: {
                            type: "linear" as const,
                            position: "right" as const,
                            beginAtZero: true,
                            grid: { drawOnChartArea: false },
                            title: { display: true, text: humiditySeries?.unit ?? humiditySensor?.unit ?? "%" },
                          },
                        },
                      }}
                    />
                  )}
                </CardContent>
              </Card>

              <Card>
                <CardHeader>
                  <CardTitle className="text-sm">Wind speed & gust — {rangeSuffix}</CardTitle>
                </CardHeader>
                <CardContent>
                  {!windData.datasets.length ? (
 <p className="text-sm text-muted-foreground">No wind history available.</p>
                  ) : (
                    <ZoomableLineChart
                      wrapperClassName="h-[400px]"
                      data={windData}
                      options={{
                        ...baseChartOptions,
                        plugins: {
                          ...baseChartOptions.plugins,
                          legend: { display: true, position: "bottom" as const },
                        },
                        scales: {
                          ...baseChartOptions.scales,
                          y: {
                            ...baseChartOptions.scales.y,
                            title: {
                              display: true,
                              text: windSpeedSeries?.unit ?? windSpeedSensor?.unit ?? "m/s",
                            },
                          },
                        },
                      }}
                    />
                  )}
                </CardContent>
              </Card>

              <Card>
                <CardHeader>
                  <CardTitle className="text-sm">Rain — {rangeSuffix}</CardTitle>
                </CardHeader>
                <CardContent>
                  {!rainData.datasets.length ? (
 <p className="text-sm text-muted-foreground">No rain history available.</p>
                  ) : (
                    <ZoomableLineChart
                      wrapperClassName="h-[400px]"
                      data={rainData}
                      options={{
                        ...baseChartOptions,
                        plugins: {
                          ...baseChartOptions.plugins,
                          legend: { display: true, position: "bottom" as const },
                        },
                        scales: {
                          ...baseChartOptions.scales,
                          y: {
                            ...baseChartOptions.scales.y,
                            title: { display: true, text: rainRateSeries?.unit ?? rainRateSensor?.unit ?? "mm/h" },
                          },
                          y1: {
                            type: "linear" as const,
                            position: "right" as const,
                            beginAtZero: true,
                            grid: { drawOnChartArea: false },
                            title: { display: true, text: rainSeries?.unit ?? rainSensor?.unit ?? "mm" },
                          },
                        },
                      }}
                    />
                  )}
                </CardContent>
              </Card>

              {pressureData ? (
                <Card>
                  <CardHeader>
                    <CardTitle className="text-sm">Pressure — {rangeSuffix}</CardTitle>
                  </CardHeader>
                  <CardContent>
                    <ZoomableLineChart
                      wrapperClassName="h-[400px]"
                      data={pressureData}
                      options={{
                        ...baseChartOptions,
                        plugins: {
                          ...baseChartOptions.plugins,
                          legend: { display: true, position: "bottom" as const },
                        },
                        scales: {
                          ...baseChartOptions.scales,
                          y: {
                            ...baseChartOptions.scales.y,
                            beginAtZero: false,
                            title: { display: true, text: pressureSeries?.unit ?? pressureSensor?.unit ?? "" },
                          },
                        },
                      }}
                    />
                  </CardContent>
                </Card>
              ) : null}

              <Card>
                <CardHeader>
                  <CardTitle className="text-sm">Solar / UV — {rangeSuffix}</CardTitle>
                </CardHeader>
                <CardContent>
                  {!solarUvData.datasets.length ? (
 <p className="text-sm text-muted-foreground">
                      No solar/UV history available.
                    </p>
                  ) : (
                    <ZoomableLineChart
                      wrapperClassName="h-[400px]"
                      data={solarUvData}
                      options={{
                        ...baseChartOptions,
                        plugins: {
                          ...baseChartOptions.plugins,
                          legend: { display: true, position: "bottom" as const },
                        },
                        scales: {
                          ...baseChartOptions.scales,
                          y: {
                            ...baseChartOptions.scales.y,
                            title: {
                              display: true,
                              text: solarRadiationSeries?.unit ?? solarRadiationSensor?.unit ?? "W/m²",
                            },
                          },
                          y1: {
                            type: "linear" as const,
                            position: "right" as const,
                            beginAtZero: true,
                            grid: { drawOnChartArea: false },
                            title: { display: true, text: "UV" },
                          },
                        },
                      }}
                    />
                  )}
                </CardContent>
              </Card>
            </div>
          )}
        </div>
    </CollapsibleCard>
  );
}

function WindDirectionCompass({ degrees }: { degrees: number | null }) {
  const safeDegrees = degrees != null && Number.isFinite(degrees) ? degrees : null;
  return (
    <svg
      viewBox="0 0 100 100"
      className="h-24 w-24 shrink-0"
      role="img"
      aria-label={
        safeDegrees == null ? "Wind direction unknown" : `Wind direction ${Math.round(safeDegrees)} degrees`
      }
    >
 <circle cx="50" cy="50" r="44" fill="none" stroke="currentColor" strokeWidth="2" className="text-gray-300" />
 <circle cx="50" cy="50" r="2.5" fill="currentColor" className="text-muted-foreground" />

 <text x="50" y="16" textAnchor="middle" fontSize="10" className="fill-gray-500">
        N
      </text>
 <text x="86" y="54" textAnchor="middle" fontSize="10" className="fill-gray-500">
        E
      </text>
 <text x="50" y="92" textAnchor="middle" fontSize="10" className="fill-gray-500">
        S
      </text>
 <text x="14" y="54" textAnchor="middle" fontSize="10" className="fill-gray-500">
        W
      </text>

      {safeDegrees != null ? (
        <g transform={`rotate(${safeDegrees} 50 50)`}>
 <line x1="50" y1="50" x2="50" y2="18" stroke="currentColor" strokeWidth="3" className="text-indigo-600" />
 <polygon points="50,10 44,22 56,22" fill="currentColor" className="text-indigo-600" />
        </g>
      ) : (
 <text x="50" y="56" textAnchor="middle" fontSize="10" className="fill-gray-400">
          —
        </text>
      )}
    </svg>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="min-w-0">
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground leading-tight">
        {label}
      </dt>
      <dd className="mt-0.5 font-medium text-card-foreground tabular-nums leading-snug">
        {value}
      </dd>
    </div>
  );
}
