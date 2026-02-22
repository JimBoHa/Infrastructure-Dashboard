"use client";

import { useAnalyticsFeedStatusQuery, useAnalyticsQuery } from "@/lib/queries";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import AnalyticsOverview from "@/features/analytics/components/AnalyticsOverview";

export default function AnalyticsPageClient() {
  const { data: analytics, error, isLoading } = useAnalyticsQuery();
  const { data: feeds } = useAnalyticsFeedStatusQuery();

  if (isLoading) return <LoadingState label="Loading analytics..." />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load analytics."} />;
  }
  if (!analytics) return <ErrorState message="No analytics available." />;

  return <AnalyticsOverview analytics={analytics} feeds={feeds} />;
}
