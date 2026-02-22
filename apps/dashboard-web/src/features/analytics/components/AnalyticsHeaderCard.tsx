"use client";

import type { ReactNode } from "react";
import { usePathname } from "next/navigation";
import PageHeaderCard from "@/components/PageHeaderCard";

type AnalyticsTabKey = "overview" | "trends" | "power" | "compensation";

function tabForPathname(pathname: string | null): AnalyticsTabKey {
  if (!pathname) return "overview";
  if (pathname === "/analytics/trends" || pathname.startsWith("/analytics/trends/")) {
    return "trends";
  }
  if (pathname === "/analytics/compensation" || pathname.startsWith("/analytics/compensation/")) {
    return "compensation";
  }
  if (pathname === "/analytics/power" || pathname.startsWith("/analytics/power/")) {
    return "power";
  }
  return "overview";
}

function titleForTab(tab: AnalyticsTabKey): string {
  if (tab === "trends") return "Trends";
  if (tab === "power") return "Power";
  if (tab === "compensation") return "Temperature Drift Compensation";
  return "Analytics Overview";
}

function descriptionForTab(tab: AnalyticsTabKey): string {
  if (tab === "trends") {
    return "Ad-hoc multi-sensor charting. For system totals, use Analytics Overview. For renames/decimals, use Sensors & Outputs.";
  }
  if (tab === "power") {
    return "Node-level Renogy/Emporia dashboards. Fleet totals and forecasts live in Analytics Overview.";
  }
  if (tab === "compensation") {
    return "Assisted workflow to compensate a sensor’s temperature-driven drift using a reference temperature sensor. Preview the correction and create a derived compensated sensor.";
  }
  return "Summary view: forecasts, fleet totals, water/soil, and fleet health. This page shows real data; unavailable endpoints show explicit errors.";
}

export default function AnalyticsHeaderCard({
  tab,
  actions,
  children,
}: {
  tab?: AnalyticsTabKey;
  actions?: ReactNode;
  children?: ReactNode;
}) {
  const pathname = usePathname();
  const effectiveTab = tab ?? tabForPathname(pathname);

  return (
    <PageHeaderCard
      title={titleForTab(effectiveTab)}
      description={descriptionForTab(effectiveTab)}
      actions={actions}
    >
      {children}
    </PageHeaderCard>
  );
}
