"use client";

import CollapsibleCard from "@/components/CollapsibleCard";
import { PvForecastSection } from "@/features/analytics/components/PvForecastSection";
import { WeatherForecastSection } from "@/features/analytics/components/WeatherForecastSection";
import { WeatherStationSection } from "@/features/analytics/components/WeatherStationSection";

export function ForecastSection() {
  return (
    <CollapsibleCard
      title="Forecasts"
      description="Weather and PV forecasts (including provider health and overlays)."
      defaultOpen
      bodyClassName="space-y-4"
    >
      <div className="space-y-4">
        <WeatherStationSection />
        <WeatherForecastSection />
      </div>
      <PvForecastSection />
    </CollapsibleCard>
  );
}

