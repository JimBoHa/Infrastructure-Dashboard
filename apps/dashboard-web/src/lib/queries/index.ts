import { useQuery } from "@tanstack/react-query";
import { fetchAnalyticsBundle } from "@/lib/analytics";
import {
  fetchAdoptionCandidates,
  fetchAlarmEvents,
  fetchAlarms,
  fetchAlarmRules,
  fetchAnalyticsFeedStatus,
  fetchForecastStatus,
  fetchBackupRetentionConfig,
  fetchBackups,
  fetchConnection,
  fetchMetricsSeries,
  fetchNodes,
  fetchOutputs,
  fetchRecentRestores,
  fetchSchedules,
  fetchSensors,
  fetchMapFeatures,
  fetchMapLayers,
  fetchMapSaves,
  fetchMapSettings,
  fetchOfflineMapPacks,
  fetchPvForecastConfig,
  fetchPvForecastDaily,
  fetchPvForecastHourly,
  fetchDevActivityStatus,
  fetchPredictiveStatus,
  fetchSetupCredentials,
  fetchEmporiaDevices,
  fetchExternalDeviceCatalog,
  fetchExternalDevices,
  fetchWeatherForecastConfig,
  fetchCurrentWeather,
  fetchWeatherForecastDaily,
  fetchWeatherForecastHourly,
  fetchUsers,
  fetchRenogySettingsSchema,
  fetchRenogyDesiredSettings,
  fetchRenogySettingsHistory,
  fetchBatteryConfig,
  fetchPowerRunwayConfig,
} from "@/lib/api";

const STALE_SHORT = 10_000;
const STALE_MEDIUM = 30_000;
const STALE_LONG = 60_000;

type QueryToggleOptions = {
  enabled?: boolean;
};

export const queryKeys = {
  nodes: ["nodes"] as const,
  sensors: ["sensors"] as const,
  outputs: ["outputs"] as const,
  schedules: ["schedules"] as const,
  alarms: ["alarms"] as const,
  alarmRules: ["alarm-rules"] as const,
  predictiveStatus: ["predictive", "status"] as const,
  alarmEvents: (limit = 100) => ["alarms", "history", limit] as const,
  users: ["users"] as const,
  connection: ["connection"] as const,
  adoptionCandidates: ["adoption", "candidates"] as const,
  backups: ["backups"] as const,
  backupRetention: ["backups", "retention"] as const,
  recentRestores: ["backups", "recent-restores"] as const,
  analytics: ["analytics"] as const,
  analyticsFeedStatus: ["analytics", "feeds", "status"] as const,
  forecastStatus: ["forecast", "status"] as const,
  weatherForecastConfig: ["forecast", "weather", "config"] as const,
  weatherCurrent: (nodeId: string) => ["forecast", "weather", "current", nodeId] as const,
  weatherForecastHourly: (hours: number) => ["forecast", "weather", "hourly", hours] as const,
  weatherForecastDaily: (days: number) => ["forecast", "weather", "daily", days] as const,
  pvForecastConfig: (nodeId: string) => ["forecast", "pv", "config", nodeId] as const,
  pvForecastHourly: (nodeId: string, hours: number, historyHours = 0) =>
    ["forecast", "pv", "hourly", nodeId, hours, historyHours] as const,
  pvForecastDaily: (nodeId: string, days: number) =>
    ["forecast", "pv", "daily", nodeId, days] as const,
  batteryConfig: (nodeId: string) => ["battery", "config", nodeId] as const,
  powerRunwayConfig: (nodeId: string) => ["power", "runway", "config", nodeId] as const,
  setupCredentials: ["setup", "credentials"] as const,
  emporiaDevices: ["setup", "emporia", "devices"] as const,
  externalDeviceCatalog: ["integrations", "devices", "catalog"] as const,
  externalDevices: ["integrations", "devices"] as const,
  scheduleCalendar: (start: string, end: string) =>
    ["schedules", "calendar", start, end] as const,
  metrics: (sensorIds: string[], rangeHours: number, interval: number) =>
    ["metrics", sensorIds.join(","), rangeHours, interval] as const,
  metricsWindow: (sensorIds: string[], start: string, end: string, interval: number) =>
    ["metrics", sensorIds.join(","), start, end, interval] as const,
  trendPreview: (sensorId: string, start: string, end: string, interval: number) =>
    ["metrics", "preview", sensorId, start, end, interval] as const,
  mapSaves: ["map", "saves"] as const,
  mapSettings: ["map", "settings"] as const,
  mapLayers: ["map", "layers"] as const,
  mapFeatures: ["map", "features"] as const,
  mapOfflinePacks: ["map", "offline", "packs"] as const,
  devActivity: ["dev", "activity"] as const,
  renogySettingsSchema: (nodeId: string) => ["renogy", nodeId, "settings", "schema"] as const,
  renogyDesiredSettings: (nodeId: string) => ["renogy", nodeId, "settings", "desired"] as const,
  renogySettingsHistory: (nodeId: string) => ["renogy", nodeId, "settings", "history"] as const,
};

export const useNodesQuery = () =>
  useQuery({
    queryKey: queryKeys.nodes,
    queryFn: fetchNodes,
    staleTime: STALE_SHORT,
    refetchInterval: STALE_SHORT,
  });

export const useSensorsQuery = () =>
  useQuery({
    queryKey: queryKeys.sensors,
    queryFn: fetchSensors,
    staleTime: STALE_SHORT,
    refetchInterval: STALE_SHORT,
  });

export const useOutputsQuery = () =>
  useQuery({
    queryKey: queryKeys.outputs,
    queryFn: fetchOutputs,
    staleTime: STALE_SHORT,
    refetchInterval: STALE_SHORT,
  });

export const useSchedulesQuery = () =>
  useQuery({
    queryKey: queryKeys.schedules,
    queryFn: fetchSchedules,
    staleTime: STALE_MEDIUM,
  });

export const useAlarmsQuery = () =>
  useQuery({
    queryKey: queryKeys.alarms,
    queryFn: fetchAlarms,
    staleTime: STALE_SHORT,
    refetchInterval: STALE_SHORT,
  });

export const useAlarmRulesQuery = () =>
  useQuery({
    queryKey: queryKeys.alarmRules,
    queryFn: fetchAlarmRules,
    staleTime: STALE_SHORT,
    refetchInterval: STALE_SHORT,
  });

export const useAlarmEventsQuery = (limit = 100) =>
  useQuery({
    queryKey: queryKeys.alarmEvents(limit),
    queryFn: () => fetchAlarmEvents(limit),
    staleTime: STALE_SHORT,
  });

export const useUsersQuery = (options: QueryToggleOptions = {}) =>
  useQuery({
    queryKey: queryKeys.users,
    queryFn: fetchUsers,
    staleTime: STALE_LONG,
    enabled: options.enabled ?? true,
  });

export const useConnectionQuery = () =>
  useQuery({
    queryKey: queryKeys.connection,
    queryFn: fetchConnection,
    staleTime: STALE_MEDIUM,
    refetchInterval: STALE_MEDIUM,
  });

export const useAdoptionCandidatesQuery = (options: { enabled?: boolean } = {}) =>
  useQuery({
    queryKey: queryKeys.adoptionCandidates,
    queryFn: fetchAdoptionCandidates,
    staleTime: STALE_SHORT,
    enabled: options.enabled ?? true,
  });

export const useBackupsQuery = (options: { enabled?: boolean } = {}) =>
  useQuery({
    queryKey: queryKeys.backups,
    queryFn: fetchBackups,
    staleTime: STALE_LONG,
    enabled: options.enabled ?? true,
  });

export const useBackupRetentionConfigQuery = (options: { enabled?: boolean } = {}) =>
  useQuery({
    queryKey: queryKeys.backupRetention,
    queryFn: fetchBackupRetentionConfig,
    staleTime: STALE_LONG,
    enabled: options.enabled ?? true,
  });

export const useRecentRestoresQuery = (options: { enabled?: boolean } = {}) =>
  useQuery({
    queryKey: queryKeys.recentRestores,
    queryFn: fetchRecentRestores,
    staleTime: STALE_SHORT,
    refetchInterval: 15_000,
    enabled: options.enabled ?? true,
  });

export const useAnalyticsQuery = () =>
  useQuery({
    queryKey: queryKeys.analytics,
    queryFn: fetchAnalyticsBundle,
    staleTime: STALE_MEDIUM,
    refetchInterval: 30_000,
  });

export const useAnalyticsFeedStatusQuery = () =>
  useQuery({
    queryKey: queryKeys.analyticsFeedStatus,
    queryFn: fetchAnalyticsFeedStatus,
    staleTime: STALE_MEDIUM,
    refetchInterval: 30_000,
  });

export const useForecastStatusQuery = () =>
  useQuery({
    queryKey: queryKeys.forecastStatus,
    queryFn: fetchForecastStatus,
    staleTime: STALE_MEDIUM,
    refetchInterval: 60_000,
  });

export const useWeatherForecastConfigQuery = () =>
  useQuery({
    queryKey: queryKeys.weatherForecastConfig,
    queryFn: fetchWeatherForecastConfig,
    staleTime: STALE_LONG,
    refetchInterval: 60_000,
  });

export const useCurrentWeatherQuery = (nodeId: string | null) =>
  useQuery({
    queryKey: queryKeys.weatherCurrent(nodeId ?? "missing"),
    queryFn: () => (nodeId ? fetchCurrentWeather(nodeId) : Promise.resolve(null)),
    enabled: Boolean(nodeId),
    staleTime: STALE_MEDIUM,
    refetchInterval: 60_000,
  });

export const useWeatherForecastHourlyQuery = (hours: number) =>
  useQuery({
    queryKey: queryKeys.weatherForecastHourly(hours),
    queryFn: () => fetchWeatherForecastHourly(hours),
    staleTime: STALE_MEDIUM,
    refetchInterval: 5 * 60_000,
  });

export const useWeatherForecastDailyQuery = (days: number) =>
  useQuery({
    queryKey: queryKeys.weatherForecastDaily(days),
    queryFn: () => fetchWeatherForecastDaily(days),
    staleTime: STALE_LONG,
    refetchInterval: 15 * 60_000,
  });

export const usePvForecastConfigQuery = (nodeId: string | null, options?: QueryToggleOptions) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  return useQuery({
    queryKey: queryKeys.pvForecastConfig(nodeId ?? "missing"),
    queryFn: () => (nodeId ? fetchPvForecastConfig(nodeId) : Promise.resolve(null)),
    enabled,
    staleTime: STALE_LONG,
    refetchInterval: enabled ? 60_000 : false,
  });
};

export const usePvForecastHourlyQuery = (
  nodeId: string | null,
  hours: number,
  options?: QueryToggleOptions & { historyHours?: number },
) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  const historyHours = options?.historyHours ?? 0;
  return useQuery({
    queryKey: queryKeys.pvForecastHourly(nodeId ?? "missing", hours, historyHours),
    queryFn: () => (nodeId ? fetchPvForecastHourly(nodeId, hours, historyHours) : Promise.resolve(null)),
    enabled,
    staleTime: STALE_MEDIUM,
    refetchInterval: enabled ? 5 * 60_000 : false,
  });
};

export const usePvForecastDailyQuery = (
  nodeId: string | null,
  days: number,
  options?: QueryToggleOptions,
) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  return useQuery({
    queryKey: queryKeys.pvForecastDaily(nodeId ?? "missing", days),
    queryFn: () => (nodeId ? fetchPvForecastDaily(nodeId, days) : Promise.resolve(null)),
    enabled,
    staleTime: STALE_LONG,
    refetchInterval: enabled ? 15 * 60_000 : false,
  });
};

export const useBatteryConfigQuery = (nodeId: string | null, options?: QueryToggleOptions) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  return useQuery({
    queryKey: queryKeys.batteryConfig(nodeId ?? "missing"),
    queryFn: () =>
      nodeId ? fetchBatteryConfig(nodeId) : Promise.reject(new Error("Missing node id")),
    enabled,
    staleTime: STALE_LONG,
    refetchInterval: enabled ? 60_000 : false,
  });
};

export const usePowerRunwayConfigQuery = (nodeId: string | null, options?: QueryToggleOptions) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  return useQuery({
    queryKey: queryKeys.powerRunwayConfig(nodeId ?? "missing"),
    queryFn: () =>
      nodeId ? fetchPowerRunwayConfig(nodeId) : Promise.reject(new Error("Missing node id")),
    enabled,
    staleTime: STALE_LONG,
    refetchInterval: enabled ? 60_000 : false,
  });
};

export const useSetupCredentialsQuery = () =>
  useQuery({
    queryKey: queryKeys.setupCredentials,
    queryFn: fetchSetupCredentials,
    staleTime: STALE_LONG,
  });

export const useEmporiaDevicesQuery = () =>
  useQuery({
    queryKey: queryKeys.emporiaDevices,
    queryFn: fetchEmporiaDevices,
    staleTime: STALE_LONG,
  });

export const useExternalDeviceCatalogQuery = () =>
  useQuery({
    queryKey: queryKeys.externalDeviceCatalog,
    queryFn: fetchExternalDeviceCatalog,
    staleTime: STALE_LONG,
  });

export const useExternalDevicesQuery = () =>
  useQuery({
    queryKey: queryKeys.externalDevices,
    queryFn: fetchExternalDevices,
    staleTime: STALE_SHORT,
    refetchInterval: STALE_SHORT,
  });

export const useMapSavesQuery = () =>
  useQuery({
    queryKey: queryKeys.mapSaves,
    queryFn: fetchMapSaves,
    staleTime: STALE_LONG,
  });

export const useMapSettingsQuery = () =>
  useQuery({
    queryKey: queryKeys.mapSettings,
    queryFn: fetchMapSettings,
    staleTime: STALE_LONG,
  });

export const useMapLayersQuery = () =>
  useQuery({
    queryKey: queryKeys.mapLayers,
    queryFn: fetchMapLayers,
    staleTime: STALE_LONG,
  });

export const useMapFeaturesQuery = () =>
  useQuery({
    queryKey: queryKeys.mapFeatures,
    queryFn: fetchMapFeatures,
    staleTime: STALE_LONG,
  });

export const useMapOfflinePacksQuery = () =>
  useQuery({
    queryKey: queryKeys.mapOfflinePacks,
    queryFn: fetchOfflineMapPacks,
    staleTime: 5_000,
    refetchInterval: (data) => {
      if (!Array.isArray(data)) return false;
      const isInstalling = (pack: unknown) => {
        if (!pack || typeof pack !== "object") return false;
        const status = (pack as Record<string, unknown>).status;
        return status === "installing";
      };
      return data.some(isInstalling)
        ? 2_000
        : false;
    },
  });

export const useDevActivityQuery = () =>
  useQuery({
    queryKey: queryKeys.devActivity,
    queryFn: fetchDevActivityStatus,
    staleTime: 5_000,
    refetchInterval: 10_000,
  });

export const useRenogySettingsSchemaQuery = (nodeId: string | null, options?: QueryToggleOptions) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  return useQuery({
    queryKey: queryKeys.renogySettingsSchema(nodeId ?? "missing"),
    queryFn: () => fetchRenogySettingsSchema(nodeId as string),
    enabled,
    staleTime: STALE_LONG,
  });
};

export const useRenogyDesiredSettingsQuery = (nodeId: string | null, options?: QueryToggleOptions) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  return useQuery({
    queryKey: queryKeys.renogyDesiredSettings(nodeId ?? "missing"),
    queryFn: () => fetchRenogyDesiredSettings(nodeId as string),
    enabled,
    staleTime: STALE_MEDIUM,
  });
};

export const useRenogySettingsHistoryQuery = (nodeId: string | null, options?: QueryToggleOptions) => {
  const enabled = Boolean(nodeId) && (options?.enabled ?? true);
  return useQuery({
    queryKey: queryKeys.renogySettingsHistory(nodeId ?? "missing"),
    queryFn: () => fetchRenogySettingsHistory(nodeId as string),
    enabled,
    staleTime: STALE_MEDIUM,
  });
};

export const usePredictiveStatusQuery = () =>
  useQuery({
    queryKey: queryKeys.predictiveStatus,
    queryFn: fetchPredictiveStatus,
    staleTime: STALE_LONG,
  });

export const useMetricsQuery = ({
  sensorIds,
  rangeHours,
  interval,
  start,
  end,
  enabled,
  refetchInterval,
}: {
  sensorIds: string[];
  rangeHours: number;
  interval: number;
  start?: string;
  end?: string;
  enabled: boolean;
  refetchInterval?: number;
}) =>
  useQuery({
    queryKey:
      start && end
        ? queryKeys.metricsWindow(sensorIds, start, end, interval)
        : queryKeys.metrics(sensorIds, rangeHours, interval),
    queryFn: () => {
      if (start && end) return fetchMetricsSeries(sensorIds, start, end, interval);
      const computedEnd = new Date();
      const computedStart = new Date(computedEnd.getTime() - rangeHours * 60 * 60 * 1000);
      return fetchMetricsSeries(sensorIds, computedStart.toISOString(), computedEnd.toISOString(), interval);
    },
    enabled,
    staleTime: STALE_SHORT,
    refetchInterval,
  });

export const useTrendPreviewQuery = ({
  sensorId,
  start,
  end,
  interval,
  enabled,
}: {
  sensorId: string;
  start: string;
  end: string;
  interval: number;
  enabled: boolean;
}) =>
  useQuery({
    queryKey: queryKeys.trendPreview(sensorId, start, end, interval),
    queryFn: () => fetchMetricsSeries([sensorId], start, end, interval),
    enabled,
    staleTime: STALE_MEDIUM,
  });
