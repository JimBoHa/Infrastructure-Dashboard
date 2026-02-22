"use client";

import VoltageQualityPanel from "@/features/trends/components/VoltageQualityPanel";
import type { TrendSeriesEntry } from "@/types/dashboard";

export default function DcVoltageQualityPanel({
  series,
  intervalSeconds,
  title = "DC voltage quality",
}: {
  series: TrendSeriesEntry[];
  intervalSeconds: number;
  title?: string;
}) {
  return <VoltageQualityPanel series={series} intervalSeconds={intervalSeconds} mode="dc" title={title} />;
}

