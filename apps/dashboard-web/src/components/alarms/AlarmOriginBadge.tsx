"use client";

import type { AlarmOrigin } from "@/types/alarms";
import { isPredictiveOrigin } from "@/lib/alarms/origin";

export default function AlarmOriginBadge({
  origin,
}: {
  origin: AlarmOrigin | null | undefined;
}) {
  const predictive = isPredictiveOrigin(origin);
  const label = predictive ? "Predictive" : "Standard";
  const className = predictive
 ? "inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2.5 py-1 text-xs font-semibold text-emerald-800"
 : "inline-flex items-center gap-1 rounded-full bg-muted px-2.5 py-1 text-xs font-semibold text-foreground";

  return (
    <span className={className} title={predictive ? "Model-driven alarm" : "Threshold/offline alarm"}>
      {label}
    </span>
  );
}
