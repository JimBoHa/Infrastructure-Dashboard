"use client";

import VoltageQualityPanel from "@/features/trends/components/VoltageQualityPanel";
import type { TrendSeriesEntry } from "@/types/dashboard";

export default function AcVoltageQualityPanel({
  series,
  intervalSeconds,
}: {
  series: TrendSeriesEntry[];
  intervalSeconds: number;
}) {
  return <VoltageQualityPanel series={series} intervalSeconds={intervalSeconds} mode="ac" title="AC voltage quality" />;
}

