"use client";

import { useMemo, useState } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import {
  AnalyticsRangeSelect,
  type AnalyticsHistoryRangeHours,
  historyIntervalSeconds,
  historyRangeLabel,
  useBaseChartOptions,
  ZoomableLineChart,
  buildChartData,
  type ChartSeriesConfig,
} from "@/features/analytics/components/AnalyticsShared";
import { useAnalyticsData } from "@/features/analytics/hooks/useAnalyticsData";
import { BATTERY_COLORS } from "@/features/analytics/utils/colors";
import { useMetricsQuery } from "@/lib/queries";
import type { DemoSensor } from "@/types/dashboard";

export function BatteryVoltageSection() {
  const [rangeHours, setRangeHours] = useState<AnalyticsHistoryRangeHours>(24);
  const baseChartOptions = useBaseChartOptions(rangeHours);

  const { sensors, nodeLabelsById, isLoading, error } = useAnalyticsData();
  const batterySensors = useMemo(
    () => sensors.filter((sensor) => isBatteryVoltageSensor(sensor)),
    [sensors],
  );
  const voltageSensorIds = useMemo(
    () => batterySensors.map((sensor) => sensor.sensor_id).filter(Boolean).sort(),
    [batterySensors],
  );
  const {
    data: voltageSeries,
    isLoading: metricsLoading,
    isFetching: metricsFetching,
    error: metricsError,
  } = useMetricsQuery({
    sensorIds: voltageSensorIds,
    rangeHours,
    interval: historyIntervalSeconds(rangeHours),
    enabled: voltageSensorIds.length > 0,
    refetchInterval: 30_000,
  });

  const chartSeries = useMemo<ChartSeriesConfig[]>(() => {
    if (!voltageSeries?.length) return [];
    const sensorById = new Map(batterySensors.map((sensor) => [sensor.sensor_id, sensor]));
    return voltageSeries
      .flatMap((entry, index) => {
        const sensor = sensorById.get(entry.sensor_id);
        const label = (sensor && nodeLabelsById.get(sensor.node_id)) ?? entry.label ?? sensor?.name ?? entry.sensor_id;
        const series = entry.points;
        if (!series.length) return [];
        return [{
          label,
          series,
          color: BATTERY_COLORS[index % BATTERY_COLORS.length],
        }];
      })
      .sort((a, b) => a.label.localeCompare(b.label));
  }, [batterySensors, nodeLabelsById, voltageSeries]);

  const loading = isLoading || metricsLoading;
  const toMessage = (error: unknown) =>
    error instanceof Error ? error.message : typeof error === "string" ? error : null;
  const errorMessage = toMessage(error) ?? toMessage(metricsError);

  return (
    <CollapsibleCard
      title="Battery voltage"
      description="Single graph showing each node’s battery voltage line; helpful for spotting sagging packs across the fleet."
      defaultOpen={false}
      bodyClassName="space-y-3"
      actions={<AnalyticsRangeSelect value={rangeHours} onChange={setRangeHours} />}
    >
      {errorMessage ? (
        <p className="text-sm text-rose-600">Failed to load battery voltage: {errorMessage}</p>
      ) : voltageSensorIds.length === 0 ? (
 <p className="text-sm text-muted-foreground">
          No battery voltage sensors detected yet. Configure a Renogy BT-2 or other battery source to
          see voltage history here.
        </p>
      ) : loading || (metricsFetching && !voltageSeries?.length) ? (
 <p className="text-sm text-muted-foreground">
          Loading battery voltage trends…
        </p>
      ) : !chartSeries.length ? (
 <p className="text-sm text-muted-foreground">
          Battery voltage series are empty for the selected range.
        </p>
      ) : (
        <>
          <ZoomableLineChart
            wrapperClassName="mt-4 h-[400px]"
            data={buildChartData(chartSeries)}
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
                  title: { display: true, text: "V" },
                },
              },
            }}
          />
 <p className="mt-2 text-xs text-muted-foreground">
            {historyRangeLabel(rangeHours)} · {(() => {
              const intervalSeconds = historyIntervalSeconds(rangeHours);
              if (intervalSeconds % 3600 === 0) {
                const hours = intervalSeconds / 3600;
                return hours === 1 ? "hourly averages" : `${hours}-hour averages`;
              }
              if (intervalSeconds % 60 === 0) {
                const minutes = intervalSeconds / 60;
                return minutes === 1 ? "1-minute averages" : `${minutes}-minute averages`;
              }
              return `${intervalSeconds}s averages`;
            })()} per node.
          </p>
        </>
      )}
    </CollapsibleCard>
  );
}


function isBatteryVoltageSensor(sensor: DemoSensor): boolean {
  const config = sensor.config ?? {};
  const metric = String(config.metric ?? "").toLowerCase();
  const category = String(config.category ?? "").toLowerCase();
  const unit = String(sensor.unit ?? "").toLowerCase();
  const name = String(sensor.name ?? "").toLowerCase();
  const type = String(sensor.type ?? "").toLowerCase();
  if (metric.includes("battery_voltage")) {
    return true;
  }
  if (category === "battery" && unit.startsWith("v")) {
    return true;
  }
  if (type.includes("battery") && unit.startsWith("v")) {
    return true;
  }
  return name.includes("battery voltage");
}
