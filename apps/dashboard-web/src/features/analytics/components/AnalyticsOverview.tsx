"use client";

import AnalyticsHeaderCard from "@/features/analytics/components/AnalyticsHeaderCard";
import { AnalyticsSummaryCards } from "@/features/analytics/components/AnalyticsSummaryCards";
import { BatteryVoltageSection } from "@/features/analytics/components/BatteryVoltageSection";
import { FeedHealthSection } from "@/features/analytics/components/FeedHealthSection";
import { ForecastSection } from "@/features/analytics/components/ForecastSection";
import { PowerSection } from "@/features/analytics/components/PowerSection";
import { SoilSection } from "@/features/analytics/components/SoilSection";
import { StatusSection } from "@/features/analytics/components/StatusSection";
import { WaterSection } from "@/features/analytics/components/WaterSection";
import { AnalyticsDataProvider } from "@/features/analytics/hooks/useAnalyticsData";
import type { AnalyticsBundle, AnalyticsFeedStatus } from "@/types/dashboard";

export default function AnalyticsOverview({
  analytics,
  feeds,
}: {
  analytics: AnalyticsBundle;
  feeds?: AnalyticsFeedStatus;
}) {
  return (
    <AnalyticsDataProvider>
      <div className="analytics-overview w-full min-w-[1024px] space-y-5">
        <AnalyticsHeaderCard tab="overview" />

        <AnalyticsSummaryCards analytics={analytics} />

        <ForecastSection />

        <PowerSection power={analytics.power} />

        <WaterSection water={analytics.water} />

        <SoilSection soil={analytics.soil} />

        <div className="space-y-6">
          <StatusSection status={analytics.status} />
          <BatteryVoltageSection />
        </div>

        <FeedHealthSection feeds={feeds} />
      </div>
    </AnalyticsDataProvider>
  );
}
