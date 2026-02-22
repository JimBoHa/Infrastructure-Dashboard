"use client";

import { useMemo } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  queryKeys,
  useAdoptionCandidatesQuery,
  useAlarmsQuery,
  useBackupsQuery,
  useNodesQuery,
  useOutputsQuery,
  useSchedulesQuery,
  useSensorsQuery,
} from "@/lib/queries";
import { useAuth } from "@/components/AuthProvider";
import type { DemoBackup } from "@/types/dashboard";
import type { DemoAdoptionCandidate } from "@/types/dashboard";

export function filterAdoptionCandidates(
  rawAdoption: DemoAdoptionCandidate[],
  existingMacs: Set<string>,
): DemoAdoptionCandidate[] {
  return rawAdoption.filter((candidate) => {
    const macEth = candidate.mac_eth?.toLowerCase();
    const macWifi = candidate.mac_wifi?.toLowerCase();
    if (!macEth && !macWifi) return false;
    if (macEth && existingMacs.has(macEth)) return false;
    if (macWifi && existingMacs.has(macWifi)) return false;
    return true;
  });
}

export function useNodesPageData() {
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canConfigWrite = Boolean(me?.capabilities?.includes("config.write"));
  const canViewBackups = Boolean(canConfigWrite || me?.capabilities?.includes("backups.view"));
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const outputsQuery = useOutputsQuery();
  const adoptionQuery = useAdoptionCandidatesQuery({ enabled: canConfigWrite });
  const backupsQuery = useBackupsQuery({ enabled: canViewBackups });
  const schedulesQuery = useSchedulesQuery();
  const alarmsQuery = useAlarmsQuery();

  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);
  const outputs = useMemo(() => outputsQuery.data ?? [], [outputsQuery.data]);
  const rawAdoption = useMemo(() => adoptionQuery.data ?? [], [adoptionQuery.data]);
  const backups = useMemo(() => backupsQuery.data ?? [], [backupsQuery.data]);
  const schedules = useMemo(() => schedulesQuery.data ?? [], [schedulesQuery.data]);
  const alarms = useMemo(() => alarmsQuery.data ?? [], [alarmsQuery.data]);

  const existingMacs = useMemo(() => {
    const macs = new Set<string>();
    for (const node of nodes) {
      if (node.mac_eth) macs.add(node.mac_eth.toLowerCase());
      if (node.mac_wifi) macs.add(node.mac_wifi.toLowerCase());
    }
    return macs;
  }, [nodes]);

  const adoption = useMemo(
    () => filterAdoptionCandidates(rawAdoption, existingMacs),
    [existingMacs, rawAdoption],
  );

  const backupMap = useMemo(
    () =>
      backups.reduce<Record<string, DemoBackup[]>>((acc, backup) => {
        (acc[backup.node_id] ||= []).push(backup);
        return acc;
      }, {}),
    [backups],
  );

  const isLoading =
    nodesQuery.isLoading ||
    sensorsQuery.isLoading ||
    outputsQuery.isLoading ||
    adoptionQuery.isLoading ||
    backupsQuery.isLoading ||
    schedulesQuery.isLoading ||
    alarmsQuery.isLoading;
  const error =
    nodesQuery.error ||
    sensorsQuery.error ||
    outputsQuery.error ||
    adoptionQuery.error ||
    backupsQuery.error ||
    schedulesQuery.error ||
    alarmsQuery.error;

  const refreshAll = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
    void queryClient.invalidateQueries({ queryKey: queryKeys.sensors });
    void queryClient.invalidateQueries({ queryKey: queryKeys.outputs });
    void queryClient.invalidateQueries({ queryKey: queryKeys.adoptionCandidates });
    void queryClient.invalidateQueries({ queryKey: queryKeys.backups });
    void queryClient.invalidateQueries({ queryKey: queryKeys.schedules });
    void queryClient.invalidateQueries({ queryKey: queryKeys.alarms });
  };

  const refreshAdoption = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.adoptionCandidates });
  };

  return {
    nodes,
    sensors,
    outputs,
    rawAdoption,
    adoption,
    backups,
    schedules,
    alarms,
    backupMap,
    isLoading,
    error,
    refreshAll,
    refreshAdoption,
  };
}
