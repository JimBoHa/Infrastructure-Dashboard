"use client";

import { useEffect, useState } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import { fetchJson } from "@/lib/api";

import { setupDaemonPath } from "../api/setupDaemon";
import type { HealthReport } from "../types";

export default function HealthSnapshotSection() {
  const [health, setHealth] = useState<HealthReport | null>(null);
  const [healthError, setHealthError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    const loadHealth = async () => {
      try {
        const payload = await fetchJson<unknown>(setupDaemonPath("health-report"));
        if (!payload || typeof payload !== "object") {
          throw new Error("Setup app health returned an invalid response.");
        }
        const data = payload as Record<string, unknown>;
        if (!active) return;
        setHealth((data.report as HealthReport) ?? null);
        setHealthError(null);
      } catch (err) {
        if (!active) return;
        setHealthError(err instanceof Error ? err.message : "Setup app unavailable.");
        setHealth(null);
      }
    };
    void loadHealth();
    return () => {
      active = false;
    };
  }, []);

  return (
    <CollapsibleCard
      title="Health snapshot"
      description="Core API, MQTT broker, and database connectivity as reported by the setup app."
      defaultOpen
      bodyClassName="space-y-4"
      className="h-fit"
    >
      <div className="space-y-3 text-sm">
        {healthError && <p className="text-rose-500">{healthError}</p>}
        {!healthError && !health && (
 <p className="text-muted-foreground">Awaiting health report...</p>
        )}
        {health && (
          <>
            <Card className="flex-row items-center justify-between rounded-lg gap-0 bg-card-inset px-3 py-2">
              <span>{health.core_api?.message ?? "Core API"}</span>
              <span
                className={`text-xs font-semibold ${
                  health.core_api?.status === "ok"
 ? "text-emerald-600"
 : "text-rose-600"
                }`}
              >
                {health.core_api?.status ?? "unknown"}
              </span>
            </Card>
            <Card className="flex-row items-center justify-between rounded-lg gap-0 bg-card-inset px-3 py-2">
              <span>{health.mqtt?.message ?? "MQTT"}</span>
              <span
                className={`text-xs font-semibold ${
                  health.mqtt?.status === "ok"
 ? "text-emerald-600"
 : "text-rose-600"
                }`}
              >
                {health.mqtt?.status ?? "unknown"}
              </span>
            </Card>
            <Card className="flex-row items-center justify-between rounded-lg gap-0 bg-card-inset px-3 py-2">
              <span>{health.database?.message ?? "Database"}</span>
              <span
                className={`text-xs font-semibold ${
                  health.database?.status === "ok"
 ? "text-emerald-600"
 : "text-rose-600"
                }`}
              >
                {health.database?.status ?? "unknown"}
              </span>
            </Card>
            <Card className="flex-row items-center justify-between rounded-lg gap-0 bg-card-inset px-3 py-2">
              <span>{health.redis?.message ?? "Redis"}</span>
              <span
                className={`text-xs font-semibold ${
                  health.redis?.status === "ok"
 ? "text-emerald-600"
 : "text-rose-600"
                }`}
              >
                {health.redis?.status ?? "unknown"}
              </span>
            </Card>
            {health.generated_at && (
 <p className="text-xs text-muted-foreground">
                Updated {new Date(health.generated_at).toLocaleString()}
              </p>
            )}
          </>
        )}
      </div>
    </CollapsibleCard>
  );
}

