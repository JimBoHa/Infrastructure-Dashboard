"use client";

import { useState } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import {
  AnalyticsRangeSelect,
  type AnalyticsHistoryRangeHours,
  useBaseChartOptions,
  ZoomableLineChart,
  buildChartData,
  type ChartSeriesConfig,
} from "@/features/analytics/components/AnalyticsShared";
import { ANALYTICS_COLORS as COLORS } from "@/features/analytics/utils/colors";
import { filterSeriesByHours } from "@/features/analytics/utils/series";
import type { AnalyticsSoil } from "@/types/dashboard";

export function SoilSection({ soil }: { soil: AnalyticsSoil }) {
  const [rangeHours, setRangeHours] = useState<AnalyticsHistoryRangeHours>(168);
  const baseChartOptions = useBaseChartOptions(rangeHours);

  const avgSeries = soil.series_avg?.length ? soil.series_avg : soil.series;
  const minSeries = soil.series_min?.length ? soil.series_min : avgSeries;
  const maxSeries = soil.series_max?.length ? soil.series_max : avgSeries;

  const soilSeries: ChartSeriesConfig[] = [
    { label: "Average", series: filterSeriesByHours(avgSeries, rangeHours), color: COLORS.soilAvg },
    { label: "Min", series: filterSeriesByHours(minSeries, rangeHours), color: COLORS.soilMin },
    { label: "Max", series: filterSeriesByHours(maxSeries, rangeHours), color: COLORS.soilMax },
  ];

  return (
    <CollapsibleCard
      title="Soil moisture"
      description="Fleet-level min/max/avg moisture trends. Use Trends for per-sensor detail."
      defaultOpen
      bodyClassName="space-y-4"
      actions={<AnalyticsRangeSelect value={rangeHours} onChange={setRangeHours} />}
    >
      <ZoomableLineChart
        wrapperClassName="h-[400px]"
        data={buildChartData(soilSeries)}
        options={{
          ...baseChartOptions,
          plugins: {
            ...baseChartOptions.plugins,
            legend: { display: true, position: "bottom" as const },
          },
          scales: {
            ...baseChartOptions.scales,
            y: { ...baseChartOptions.scales.y, title: { display: true, text: "%" } },
          },
        }}
      />
 <div className="flex flex-wrap gap-3 text-sm text-muted-foreground">
        {soil.fields.map((field) => (
          <span
            key={field.name}
 className="rounded-full bg-muted px-3 py-1"
          >
            {field.name}: min {field.min}% / max {field.max}% / avg {field.avg}%
          </span>
        ))}
      </div>
    </CollapsibleCard>
  );
}
