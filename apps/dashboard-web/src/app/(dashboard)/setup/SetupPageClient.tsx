"use client";

import { useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useRouter, useSearchParams } from "next/navigation";

import ErrorState from "@/components/ErrorState";
import { useAuth } from "@/components/AuthProvider";
import InlineBanner from "@/components/InlineBanner";
import LoadingState from "@/components/LoadingState";
import PageHeaderCard from "@/components/PageHeaderCard";
import NodeButton from "@/features/nodes/components/NodeButton";
import { queryKeys, useNodesQuery, useSensorsQuery } from "@/lib/queries";
import { sensorSource } from "@/lib/sensorOrigin";

import { useWeatherForecastDraft } from "./hooks/useWeatherForecastDraft";
import AiAnomalyDetectionSection from "./sections/AiAnomalyDetectionSection";
import AnalyticsFeedsSection from "./sections/AnalyticsFeedsSection";
import ControllerConfigurationSection from "./sections/ControllerConfigurationSection";
import HealthSnapshotSection from "./sections/HealthSnapshotSection";
import HyperlocalWeatherSection from "./sections/HyperlocalWeatherSection";
import InstallerActionsSection from "./sections/InstallerActionsSection";
import IntegrationsSection from "./sections/IntegrationsSection";
import OfflineMapsSection from "./sections/OfflineMapsSection";
import SolarPvForecastSection from "./sections/SolarPvForecastSection";
import BatteryRunwaySection from "./sections/BatteryRunwaySection";
import type { Message } from "./types";

export default function SetupPageClient() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));

  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);
  const isLoading = nodesQuery.isLoading || sensorsQuery.isLoading;
  const error = nodesQuery.error || sensorsQuery.error;

  const [message, setMessage] = useState<Message | null>(null);
  const weatherDraft = useWeatherForecastDraft(setMessage);

  const pvRequestedNodeId = searchParams.get("pvNode");
  const pvChargeControllerNodeIds = useMemo(() => {
    const ids = new Set<string>();
    sensors.forEach((sensor) => {
      const source = sensorSource(sensor);
      if (source === "renogy_bt2") ids.add(sensor.node_id);
    });
    return ids;
  }, [sensors]);

  const pvConfigNodes = useMemo(() => {
    const filtered = nodes.filter((node) => pvChargeControllerNodeIds.has(node.id));
    if (!pvRequestedNodeId) return filtered;
    if (filtered.some((node) => node.id === pvRequestedNodeId)) return filtered;
    const requestedNode = nodes.find((node) => node.id === pvRequestedNodeId);
    return requestedNode ? [requestedNode, ...filtered] : filtered;
  }, [nodes, pvChargeControllerNodeIds, pvRequestedNodeId]);

  const refreshAll = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
    void queryClient.invalidateQueries({ queryKey: queryKeys.sensors });
    void queryClient.invalidateQueries({ queryKey: queryKeys.analyticsFeedStatus });
    void queryClient.invalidateQueries({ queryKey: queryKeys.forecastStatus });
    void queryClient.invalidateQueries({ queryKey: queryKeys.setupCredentials });
    void queryClient.invalidateQueries({ queryKey: queryKeys.emporiaDevices });
    void queryClient.invalidateQueries({ queryKey: queryKeys.weatherForecastConfig });
    void queryClient.invalidateQueries({ queryKey: queryKeys.predictiveStatus });
    void queryClient.invalidateQueries({ queryKey: queryKeys.mapOfflinePacks });
  };

  if (isLoading) return <LoadingState label="Loading setup center..." />;
  if (error) {
    return (
      <ErrorState
        message={error instanceof Error ? error.message : "Failed to load setup center."}
      />
    );
  }

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="System Setup Center"
        description="Installer status, health checks, credentials, and onboarding from one control room."
        actions={
          <>
            <NodeButton onClick={refreshAll}>Refresh</NodeButton>
            <NodeButton onClick={() => router.push("/backups")}>Backups</NodeButton>
            <NodeButton onClick={() => router.push("/deployment")}>Deployment</NodeButton>
          </>
        }
      />

      {message && (
        <InlineBanner tone={message.type === "success" ? "success" : "error"}>
          {message.text}
        </InlineBanner>
      )}

      <section className="grid gap-4 lg:grid-cols-3">
        <InstallerActionsSection onMessage={setMessage} />
        <HealthSnapshotSection />
        <AnalyticsFeedsSection onMessage={setMessage} />
      </section>

      <ControllerConfigurationSection onMessage={setMessage} />

      <HyperlocalWeatherSection model={weatherDraft} />

      <SolarPvForecastSection
        pvConfigNodes={pvConfigNodes}
        pvRequestedNodeId={pvRequestedNodeId}
        pvWeatherLocation={weatherDraft.weatherLocation}
      />

      <BatteryRunwaySection
        batteryConfigNodes={pvConfigNodes}
        requestedNodeId={pvRequestedNodeId}
        sensors={sensors}
        nodes={nodes}
        canEdit={canEdit}
      />

      <OfflineMapsSection canEdit={canEdit} onOpenMap={() => router.push("/map")} />

      <IntegrationsSection canEdit={canEdit} onMessage={setMessage} />

      <AiAnomalyDetectionSection onMessage={setMessage} />
    </div>
  );
}
