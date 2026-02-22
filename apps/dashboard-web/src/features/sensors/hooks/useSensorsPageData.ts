"use client";

import { useQueryClient } from "@tanstack/react-query";
import {
  queryKeys,
  useAlarmsQuery,
  useNodesQuery,
  useOutputsQuery,
  useSchedulesQuery,
  useSensorsQuery,
} from "@/lib/queries";

export function useSensorsPageData() {
  const queryClient = useQueryClient();
  const sensorsQuery = useSensorsQuery();
  const outputsQuery = useOutputsQuery();
  const nodesQuery = useNodesQuery();
  const schedulesQuery = useSchedulesQuery();
  const alarmsQuery = useAlarmsQuery();

  const sensors = sensorsQuery.data ?? [];
  const outputs = outputsQuery.data ?? [];
  const nodes = nodesQuery.data ?? [];
  const schedules = schedulesQuery.data ?? [];
  const alarms = alarmsQuery.data ?? [];

  const isLoading =
    sensorsQuery.isLoading ||
    outputsQuery.isLoading ||
    nodesQuery.isLoading ||
    schedulesQuery.isLoading ||
    alarmsQuery.isLoading;
  const error =
    sensorsQuery.error ||
    outputsQuery.error ||
    nodesQuery.error ||
    schedulesQuery.error ||
    alarmsQuery.error;

  const refreshAll = () => {
    void queryClient.invalidateQueries({ queryKey: queryKeys.sensors });
    void queryClient.invalidateQueries({ queryKey: queryKeys.outputs });
    void queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
    void queryClient.invalidateQueries({ queryKey: queryKeys.schedules });
    void queryClient.invalidateQueries({ queryKey: queryKeys.alarms });
  };

  return {
    sensors,
    outputs,
    nodes,
    schedules,
    alarms,
    isLoading,
    error,
    refreshAll,
  };
}
