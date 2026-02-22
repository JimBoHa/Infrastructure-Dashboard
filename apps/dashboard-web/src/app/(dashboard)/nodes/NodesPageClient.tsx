"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import InlineBanner from "@/components/InlineBanner";
import PageHeaderCard from "@/components/PageHeaderCard";
import { useAuth } from "@/components/AuthProvider";
import AdoptionModal from "@/features/nodes/components/AdoptionModal";
import AdoptionSection from "@/features/nodes/components/AdoptionSection";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodeGrid from "@/features/nodes/components/NodeGrid";
import DisplayOrderModal from "@/features/nodes/components/DisplayOrderModal";
import WeatherStationModal from "@/features/nodes/components/WeatherStationModal";
import { useNodesPageData } from "@/features/nodes/hooks/useNodesPageData";
import AlarmEventsPanel from "@/features/sensors/components/AlarmEventsPanel";
import buildRestoreOptions from "@/features/nodes/utils/buildRestoreOptions";
import type { DemoAdoptionCandidate } from "@/types/dashboard";
import { fetchAdoptionCandidates } from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import { filterAdoptionCandidates } from "@/features/nodes/hooks/useNodesPageData";

type ActionState = "idle" | "loading" | "complete" | "error";

export default function NodesPageClient() {
  const router = useRouter();
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const {
    nodes,
    sensors,
    outputs,
    rawAdoption,
    adoption,
    backupMap,
    isLoading,
    error,
    refreshAll,
  } = useNodesPageData();
  const [scanState, setScanState] = useState<ActionState>("idle");
  const [refreshState, setRefreshState] = useState<ActionState>("idle");
  const [adoptCandidate, setAdoptCandidate] = useState<DemoAdoptionCandidate | null>(null);
  const [weatherStationOpen, setWeatherStationOpen] = useState(false);
  const [displayOrderOpen, setDisplayOrderOpen] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(
    null,
  );

  const armTransientState = (setter: (state: ActionState) => void, state: ActionState) => {
    setter(state);
    if (state === "complete" || state === "error") {
      window.setTimeout(() => setter("idle"), 4000);
    }
  };

  const scanForNodes = async () => {
    setScanState("loading");
    try {
      const results = await queryClient.fetchQuery({
        queryKey: queryKeys.adoptionCandidates,
        queryFn: fetchAdoptionCandidates,
      });
      const existingMacs = new Set<string>();
      for (const node of nodes) {
        if (node.mac_eth) existingMacs.add(node.mac_eth.toLowerCase());
        if (node.mac_wifi) existingMacs.add(node.mac_wifi.toLowerCase());
      }
      const newCandidates = filterAdoptionCandidates(results, existingMacs);
      const alreadyAdopted = Math.max(0, results.length - newCandidates.length);
      setMessage({
        type: "success",
        text:
          results.length === 0
            ? "Scan complete: no nodes discovered."
            : newCandidates.length === 0
              ? `Scan complete: found ${results.length} node${results.length === 1 ? "" : "s"} (all already adopted).`
              : `Scan complete: found ${results.length} node${results.length === 1 ? "" : "s"} (${newCandidates.length} new${alreadyAdopted ? `, ${alreadyAdopted} already adopted` : ""}).`,
      });
      armTransientState(setScanState, "complete");
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Scan failed.",
      });
      armTransientState(setScanState, "error");
    }
  };

  const refreshNow = async () => {
    setRefreshState("loading");
    setMessage(null);
    try {
      refreshAll();
      await Promise.all([
        queryClient.refetchQueries({ queryKey: queryKeys.nodes, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.sensors, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.outputs, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.adoptionCandidates, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.backups, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.schedules, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.alarms, type: "active" }),
      ]);
      armTransientState(setRefreshState, "complete");
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Refresh failed.",
      });
      armTransientState(setRefreshState, "error");
    }
  };

  const restoreOptions = buildRestoreOptions(nodes, backupMap);

  if (isLoading) return <LoadingState label="Loading nodes..." />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load nodes."} />;
  }

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Nodes"
        description="Device view for adoption, health, and backups. Use Sensors & Outputs for sensor display settings and output commands."
        actions={
          canEdit ? (
            <NodeButton
              size="sm"
              onClick={() => setDisplayOrderOpen(true)}
              disabled={!nodes.length}
            >
              Reorder…
            </NodeButton>
          ) : undefined
        }
      />

      {message && (
        <InlineBanner tone={message.type === "success" ? "success" : "error"}>
          {message.text}
        </InlineBanner>
      )}

      <NodeGrid
        nodes={nodes}
        sensors={sensors}
        outputs={outputs}
        backupMap={backupMap}
        onOpenNode={(nodeId) => router.push(`/nodes/detail?id=${encodeURIComponent(nodeId)}`)}
      />

      <AdoptionSection
        discovered={rawAdoption}
        adoption={adoption}
        nodes={nodes}
        onAdopt={(candidate) => setAdoptCandidate(candidate)}
        onRefresh={() => void scanForNodes()}
        refreshLoading={scanState === "loading"}
        refreshLabel={
          scanState === "complete" ? "Scan complete" : scanState === "error" ? "Scan failed" : undefined
        }
        onConnectWeatherStation={() => setWeatherStationOpen(true)}
        onOpenNode={(nodeId) => router.push(`/nodes/detail?id=${encodeURIComponent(nodeId)}`)}
      />

      <div className="flex flex-wrap items-center justify-end gap-2">
        <NodeButton
          variant="primary"
          onClick={() => void scanForNodes()}
          loading={scanState === "loading"}
        >
          {scanState === "complete"
            ? "Scan complete"
            : scanState === "error"
              ? "Scan failed"
              : "Scan for nodes"}
        </NodeButton>
        <NodeButton onClick={() => void refreshNow()} loading={refreshState === "loading"}>
          {refreshState === "complete" ? "Complete" : refreshState === "error" ? "Error" : "Refresh"}
        </NodeButton>
      </div>

      <AlarmEventsPanel limit={25} />

      <AdoptionModal
        candidate={adoptCandidate}
        restoreOptions={restoreOptions}
        onClose={() => setAdoptCandidate(null)}
        onAdopted={(text) => {
          setMessage({ type: "success", text });
          setAdoptCandidate(null);
          refreshAll();
        }}
        onError={(text) => setMessage({ type: "error", text })}
      />

      <WeatherStationModal
        open={weatherStationOpen}
        onClose={() => setWeatherStationOpen(false)}
        onCreated={(text) => {
          setMessage({ type: "success", text });
          refreshAll();
        }}
      />

      <DisplayOrderModal
        open={displayOrderOpen}
        nodes={nodes}
        sensors={sensors}
        onClose={() => setDisplayOrderOpen(false)}
      />
    </div>
  );
}
