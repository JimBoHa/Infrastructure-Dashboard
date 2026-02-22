"use client";

import clsx from "clsx";
import { type ReactNode, useMemo, useState } from "react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  AnalyticsRangeSelect,
  type AnalyticsHistoryRangeHours,
  useBaseChartOptions,
  ZoomableBarChart,
  ZoomableLineChart,
} from "@/features/analytics/components/AnalyticsShared";
import {
  useForecastStatusQuery,
  useWeatherForecastConfigQuery,
  useWeatherForecastDailyQuery,
  useWeatherForecastHourlyQuery,
} from "@/lib/queries";
import {
  formatChartTickDate,
  formatChartTooltipDate,
  formatDateTimeForTimeZone,
  useControllerTimeZone,
} from "@/lib/siteTime";

function WeatherForecastChartGrid({
  primary,
  secondary,
}: {
  primary: ReactNode;
  secondary: ReactNode | null;
}) {
  return (
    <div
      className={clsx(
        "mt-3 grid grid-cols-1 gap-4",
        secondary ? "[@media(min-width:1024px)_and_(pointer:fine)]:grid-cols-2" : undefined,
      )}
    >
      {primary}
      {secondary}
    </div>
  );
}

export function WeatherForecastSection() {
  const timeZone = useControllerTimeZone();
  const [horizonHours, setHorizonHours] = useState<AnalyticsHistoryRangeHours>(24);
  const baseChartOptions = useBaseChartOptions(horizonHours);
  const configQuery = useWeatherForecastConfigQuery();
  const hourlyQuery = useWeatherForecastHourlyQuery(72);
  const dailyQuery = useWeatherForecastDailyQuery(7);
  const statusQuery = useForecastStatusQuery();

  const config = configQuery.data;
  const providers = statusQuery.data?.providers ?? {};
  const openMeteoStatus = providers["Open-Meteo"];

  const configured =
    Boolean(config?.enabled) &&
    config?.provider === "open_meteo" &&
    config.latitude != null &&
    config.longitude != null;

  const hourly = hourlyQuery.data ?? null;
  const daily = dailyQuery.data ?? null;

  const normalizeSeries = <T extends { points: Array<unknown> }>(series: T | undefined): T | undefined => {
    if (!series?.points?.length) return undefined;
    return series;
  };

  const hourlyTemp = normalizeSeries(hourly?.metrics?.temperature_c);
  const hourlyHumidity = normalizeSeries(hourly?.metrics?.humidity_pct);
  const hourlyPrecip = normalizeSeries(hourly?.metrics?.precipitation_mm);
  const hourlyCloud = normalizeSeries(hourly?.metrics?.cloud_cover_pct);
  const dailyMax = normalizeSeries(daily?.metrics?.temperature_max_c);
  const dailyMin = normalizeSeries(daily?.metrics?.temperature_min_c);
  const dailyPrecip = normalizeSeries(daily?.metrics?.precipitation_sum_mm);
  const dailyCloud = normalizeSeries(daily?.metrics?.cloud_cover_mean_pct);

  const dateOnlyChartOptions = useMemo(() => {
    return {
      ...baseChartOptions,
      scales: {
        ...baseChartOptions.scales,
        x: {
          ...baseChartOptions.scales.x,
          ticks: {
            ...baseChartOptions.scales.x.ticks,
            callback: (value: unknown) => formatChartTickDate(value, timeZone),
          },
        },
      },
      plugins: {
        ...baseChartOptions.plugins,
        tooltip: {
          ...baseChartOptions.plugins.tooltip,
          callbacks: {
            title: (items: Array<{ parsed?: { x?: unknown } }> | undefined) => {
              const first = items?.[0];
              const x = first?.parsed?.x;
              return formatChartTooltipDate(x, timeZone);
            },
          },
        },
      },
    };
  }, [baseChartOptions, timeZone]);

  const sliceSeriesPoints = <T extends { points: Array<{ timestamp: string; value: number }> }>(
    series: T | undefined,
    count: number,
  ): T | undefined => {
    if (!series) return undefined;
    const points = series.points.slice(0, count);
    if (!points.length) return undefined;
    return { ...series, points };
  };

  const hourly24Temp = sliceSeriesPoints(hourlyTemp, 24);
  const hourly24Humidity = sliceSeriesPoints(hourlyHumidity, 24);
  const hourly24Precip = sliceSeriesPoints(hourlyPrecip, 24);
  const hourly24Cloud = sliceSeriesPoints(hourlyCloud, 24);

  const dailyTempRanges = useMemo(() => {
    if (!dailyMin || !dailyMax) return [];

    const byDay = <T extends { timestamp: string; value: number }>(points: T[]) => {
      const map = new Map<string, T>();
      points.forEach((p) => map.set(p.timestamp.slice(0, 10), p));
      return map;
    };

    const minByDay = byDay(dailyMin.points);
    const maxByDay = byDay(dailyMax.points);

    const days = Array.from(new Set([...minByDay.keys(), ...maxByDay.keys()])).sort();
    const ranges = days
      .map((day) => {
        const min = minByDay.get(day);
        const max = maxByDay.get(day);
        if (!min || !max) return null;
        const low = Math.min(min.value, max.value);
        const high = Math.max(min.value, max.value);
        return { x: new Date(max.timestamp ?? min.timestamp), y: [low, high] as [number, number] };
      })
      .filter(Boolean) as Array<{ x: Date; y: [number, number] }>;

    return ranges;
  }, [dailyMax, dailyMin]);

  const errorMessage =
    (configQuery.error instanceof Error && configQuery.error.message) ||
    (hourlyQuery.error instanceof Error && hourlyQuery.error.message) ||
    (dailyQuery.error instanceof Error && dailyQuery.error.message) ||
    null;

  const buildTempHumidityData = (temp: typeof hourlyTemp, humidity: typeof hourlyHumidity) => ({
    datasets: [
      ...(temp
        ? [
            {
              label: `Temperature (${temp.unit})`,
              data: temp.points.map((p) => ({ x: new Date(p.timestamp), y: p.value })),
              borderColor: "#2563eb",
              backgroundColor: "#2563eb",
              borderWidth: 2,
              pointRadius: 0,
              pointHoverRadius: 3,
              tension: 0.25,
              yAxisID: "y",
            },
          ]
        : []),
      ...(humidity
        ? [
            {
              label: `Humidity (${humidity.unit})`,
              data: humidity.points.map((p) => ({ x: new Date(p.timestamp), y: p.value })),
              borderColor: "#14b8a6",
              backgroundColor: "#14b8a6",
              borderWidth: 2,
              pointRadius: 0,
              pointHoverRadius: 3,
              tension: 0.25,
              yAxisID: "y1",
            },
          ]
        : []),
    ],
  });

  const buildCloudPrecipData = (
    cloud: typeof hourlyCloud | typeof dailyCloud,
    precip: typeof hourlyPrecip | typeof dailyPrecip,
  ) => ({
    datasets: [
      ...(cloud
        ? [
            {
              label: `Cloud cover (${cloud.unit})`,
              data: cloud.points.map((p) => ({ x: new Date(p.timestamp), y: p.value })),
              borderColor: "#64748b",
              backgroundColor: "rgba(100,116,139,0.25)",
              borderWidth: 2,
              pointRadius: 0,
              pointHoverRadius: 3,
              tension: 0.25,
              yAxisID: "y",
              fill: true as const,
            },
          ]
        : []),
      ...(precip
        ? [
            {
              label: `Precipitation (${precip.unit})`,
              data: precip.points.map((p) => ({ x: new Date(p.timestamp), y: Math.max(0, p.value) })),
              borderColor: "#0ea5e9",
              backgroundColor: "#0ea5e9",
              borderWidth: 2,
              pointRadius: 0,
              pointHoverRadius: 3,
              tension: 0.25,
              yAxisID: "y1",
            },
          ]
        : []),
    ],
  });

  return (
    <Card>
      <CardHeader>
        <div className="flex flex-col gap-2 sm:flex-row sm:items-start sm:justify-between">
          <div className="space-y-1">
            <CardTitle className="text-lg">Weather forecast</CardTitle>
 <p className="text-sm text-muted-foreground">
              Hourly + weekly forecast for the configured coordinates. Configure lat/lon in Setup Center.
            </p>
          </div>
          <AnalyticsRangeSelect value={horizonHours} onChange={setHorizonHours} />
        </div>
      </CardHeader>
      <CardContent>
 <div className="text-xs text-muted-foreground">
        <p>
          Provider: Open-Meteo · Status {openMeteoStatus?.status ?? "unknown"}
          {openMeteoStatus?.last_seen
            ? ` · Last poll ${formatDateTimeForTimeZone(new Date(openMeteoStatus.last_seen), timeZone, {
                dateStyle: "medium",
                timeStyle: "short",
              })}`
            : ""}
        </p>
        {config?.latitude != null && config?.longitude != null && (
          <p>
            Location: {config.latitude.toFixed(4)}°, {config.longitude.toFixed(4)}°
            {config.updated_at
              ? ` · Saved ${formatDateTimeForTimeZone(new Date(config.updated_at), timeZone, {
                  dateStyle: "medium",
                  timeStyle: "short",
                })}`
              : ""}
          </p>
        )}
      </div>

        {!configured ? (
 <p className="mt-4 text-sm text-muted-foreground">
            Weather forecast is not configured yet (or disabled). Enter coordinates in Setup Center → Hyperlocal weather forecast.
          </p>
        ) : errorMessage ? (
          <p className="mt-4 text-sm text-rose-600">Failed to load forecast: {errorMessage}</p>
        ) : hourlyQuery.isLoading || dailyQuery.isLoading ? (
 <p className="mt-4 text-sm text-muted-foreground">Loading weather forecasts…</p>
        ) : (
          <div className="mt-5 space-y-6">
          {horizonHours === 24 ? (
            <div>
 <p className="text-sm font-semibold text-foreground">Next 24 hours</p>
              {!hourly24Temp && !hourly24Humidity && !hourly24Precip && !hourly24Cloud ? (
 <p className="mt-2 text-sm text-muted-foreground">
                  No hourly forecast points available yet.
                </p>
              ) : (
                <WeatherForecastChartGrid
                  primary={
                    <ZoomableLineChart
                    wrapperClassName="h-[400px]"
                    data={buildTempHumidityData(hourly24Temp, hourly24Humidity)}
                    options={{
                      ...baseChartOptions,
                      plugins: {
                        ...baseChartOptions.plugins,
                        legend: { display: true, position: "bottom" as const },
                      },
                      scales: {
                        x: { ...baseChartOptions.scales.x },
                        y: {
                          ...baseChartOptions.scales.y,
                          position: "left" as const,
                          beginAtZero: false,
                          display: Boolean(hourly24Temp),
                          title: hourly24Temp ? { display: true, text: hourly24Temp.unit } : undefined,
                        },
                        y1: {
                          type: "linear" as const,
                          position: "right" as const,
                          grid: { drawOnChartArea: false },
                          display: Boolean(hourly24Humidity),
                          min: 0,
                          max: 100,
                          title: hourly24Humidity ? { display: true, text: hourly24Humidity.unit } : undefined,
                        },
                      },
                    }}
                    />
                  }
                  secondary={
                    hourly24Cloud || hourly24Precip ? (
                      <ZoomableLineChart
                        wrapperClassName="h-[400px]"
                        data={buildCloudPrecipData(hourly24Cloud, hourly24Precip)}
                        options={{
                          ...baseChartOptions,
                          plugins: {
                            ...baseChartOptions.plugins,
                            legend: { display: true, position: "bottom" as const },
                          },
                          scales: {
                            x: { ...baseChartOptions.scales.x },
                            y: {
                              ...baseChartOptions.scales.y,
                              display: Boolean(hourly24Cloud),
                              min: 0,
                              max: 100,
                              title: hourly24Cloud
                                ? { display: true, text: hourly24Cloud.unit }
                                : undefined,
                            },
                            y1: {
                              type: "linear" as const,
                              position: "right" as const,
                              display: Boolean(hourly24Precip),
                              min: 0,
                              grid: { drawOnChartArea: false },
                              title: hourly24Precip
                                ? { display: true, text: hourly24Precip.unit }
                                : undefined,
                            },
                          },
                        }}
                      />
                    ) : null
                  }
                />
              )}
            </div>
          ) : null}

          {horizonHours === 72 ? (
            <div>
 <p className="text-sm font-semibold text-foreground">Next 72 hours</p>
              {!hourlyTemp && !hourlyHumidity && !hourlyPrecip && !hourlyCloud ? (
 <p className="mt-2 text-sm text-muted-foreground">
                  No hourly forecast points available yet.
                </p>
              ) : (
                <WeatherForecastChartGrid
                  primary={
                    <ZoomableLineChart
                    wrapperClassName="h-[400px]"
                    data={buildTempHumidityData(hourlyTemp, hourlyHumidity)}
                    options={{
                      ...baseChartOptions,
                      plugins: {
                        ...baseChartOptions.plugins,
                        legend: { display: true, position: "bottom" as const },
                      },
                      scales: {
                        x: { ...baseChartOptions.scales.x },
                        y: {
                          ...baseChartOptions.scales.y,
                          position: "left" as const,
                          beginAtZero: false,
                          display: Boolean(hourlyTemp),
                          title: hourlyTemp ? { display: true, text: hourlyTemp.unit } : undefined,
                        },
                        y1: {
                          type: "linear" as const,
                          position: "right" as const,
                          grid: { drawOnChartArea: false },
                          display: Boolean(hourlyHumidity),
                          min: 0,
                          max: 100,
                          title: hourlyHumidity ? { display: true, text: hourlyHumidity.unit } : undefined,
                        },
                      },
                    }}
                    />
                  }
                  secondary={
                    hourlyCloud || hourlyPrecip ? (
                      <ZoomableLineChart
                        wrapperClassName="h-[400px]"
                        data={buildCloudPrecipData(hourlyCloud, hourlyPrecip)}
                        options={{
                          ...baseChartOptions,
                          plugins: {
                            ...baseChartOptions.plugins,
                            legend: { display: true, position: "bottom" as const },
                          },
                          scales: {
                            x: { ...baseChartOptions.scales.x },
                            y: {
                              ...baseChartOptions.scales.y,
                              display: Boolean(hourlyCloud),
                              min: 0,
                              max: 100,
                              title: hourlyCloud
                                ? { display: true, text: hourlyCloud.unit }
                                : undefined,
                            },
                            y1: {
                              type: "linear" as const,
                              position: "right" as const,
                              display: Boolean(hourlyPrecip),
                              min: 0,
                              grid: { drawOnChartArea: false },
                              title: hourlyPrecip
                                ? { display: true, text: hourlyPrecip.unit }
                                : undefined,
                            },
                          },
                        }}
                      />
                    ) : null
                  }
                />
              )}
            </div>
          ) : null}

          {horizonHours === 168 ? (
            <div>
 <p className="text-sm font-semibold text-foreground">Next 7 days</p>
              {!dailyMax && !dailyMin && !dailyPrecip && !dailyCloud ? (
 <p className="mt-2 text-sm text-muted-foreground">
                  No daily forecast points available yet.
                </p>
              ) : (
                <WeatherForecastChartGrid
                  primary={
                    dailyMax && dailyMin && dailyTempRanges.length ? (
                      <ZoomableBarChart
                        wrapperClassName="h-[400px]"
                        data={{
                          datasets: [
                            {
                              label: `Temperature range (${dailyMax.unit})`,
                              data: dailyTempRanges.map((p) => ({ x: p.x, y: p.y })),
                              backgroundColor: "rgba(37, 99, 235, 0.25)",
                              borderColor: "#2563eb",
                              borderWidth: 1,
                              borderRadius: 4,
                            },
                          ],
                        }}
                        options={{
                          ...dateOnlyChartOptions,
                          plugins: {
                            ...dateOnlyChartOptions.plugins,
                            legend: { display: false },
                            tooltip: {
                              ...dateOnlyChartOptions.plugins.tooltip,
                              callbacks: {
                                ...dateOnlyChartOptions.plugins.tooltip.callbacks,
                                label: (ctx: unknown) => {
                                  const context = ctx as { raw?: { y?: unknown }; dataset?: { label?: string } };
                                  const raw = context.raw;
                                  const y = raw?.y;
                                  if (Array.isArray(y) && y.length === 2) {
                                    return `Min ${y[0].toFixed(1)} · Max ${y[1].toFixed(1)} ${dailyMax.unit}`;
                                  }
                                  return context.dataset?.label ?? "";
                                },
                              },
                            },
                          },
                          scales: {
                            ...dateOnlyChartOptions.scales,
                            y: {
                              ...dateOnlyChartOptions.scales.y,
                              position: "left" as const,
                              beginAtZero: false,
                              title: { display: true, text: dailyMax.unit },
                            },
                          },
                        }}
                      />
                    ) : (
                      <ZoomableLineChart
                        wrapperClassName="h-[400px]"
                        data={{
                          datasets: [
                            ...(dailyMax
                              ? [
                                  {
                                    label: `Temp max (${dailyMax.unit})`,
                                    data: dailyMax.points.map((p) => ({ x: p.timestamp, y: p.value })),
                                    borderColor: "#ef4444",
                                    backgroundColor: "#ef4444",
                                    borderWidth: 2,
                                    pointRadius: 0,
                                    pointHoverRadius: 3,
                                    tension: 0.25,
                                    yAxisID: "y",
                                  },
                                ]
                              : []),
                            ...(dailyMin
                              ? [
                                  {
                                    label: `Temp min (${dailyMin.unit})`,
                                    data: dailyMin.points.map((p) => ({ x: p.timestamp, y: p.value })),
                                    borderColor: "#3b82f6",
                                    backgroundColor: "#3b82f6",
                                    borderWidth: 2,
                                    pointRadius: 0,
                                    pointHoverRadius: 3,
                                    tension: 0.25,
                                    yAxisID: "y",
                                  },
                                ]
                              : []),
                          ],
                        }}
                        options={{
                          ...dateOnlyChartOptions,
                          plugins: {
                            ...dateOnlyChartOptions.plugins,
                            legend: { display: true, position: "bottom" as const },
                          },
                          scales: {
                            ...dateOnlyChartOptions.scales,
                            y: {
                              ...dateOnlyChartOptions.scales.y,
                              position: "left" as const,
                              beginAtZero: false,
                              title: dailyMax ? { display: true, text: dailyMax.unit } : undefined,
                            },
                          },
                        }}
                      />
                    )
                  }
                  secondary={
                    dailyCloud || dailyPrecip ? (
                      <ZoomableLineChart
                        wrapperClassName="h-[400px]"
                        data={buildCloudPrecipData(dailyCloud, dailyPrecip)}
                        options={{
                          ...dateOnlyChartOptions,
                          plugins: {
                            ...dateOnlyChartOptions.plugins,
                            legend: { display: true, position: "bottom" as const },
                          },
                          scales: {
                            ...dateOnlyChartOptions.scales,
                            y: {
                              ...dateOnlyChartOptions.scales.y,
                              display: Boolean(dailyCloud),
                              min: 0,
                              max: 100,
                              title: dailyCloud ? { display: true, text: dailyCloud.unit } : undefined,
                            },
                            y1: {
                              type: "linear" as const,
                              position: "right" as const,
                              display: Boolean(dailyPrecip),
                              min: 0,
                              grid: { drawOnChartArea: false },
                              title: dailyPrecip ? { display: true, text: dailyPrecip.unit } : undefined,
                            },
                          },
                        }}
                      />
                    ) : null
                  }
                />
              )}
            </div>
          ) : null}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
