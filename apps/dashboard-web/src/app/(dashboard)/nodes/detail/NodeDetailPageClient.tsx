"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { formatDistanceToNow } from "date-fns";
import { useQueryClient } from "@tanstack/react-query";
import {
  queryKeys,
  useAlarmsQuery,
  useBackupsQuery,
  useForecastStatusQuery,
  useNodesQuery,
  useOutputsQuery,
  usePvForecastConfigQuery,
  useSensorsQuery,
} from "@/lib/queries";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import { TrendChart } from "@/components/TrendChart";
import CollapsibleCard from "@/components/CollapsibleCard";
import { fetchMetricsSeries } from "@/lib/api";
import DisplayProfileSection from "@/features/nodes/components/DisplayProfileSection";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import NodeTypeBadge from "@/features/nodes/components/NodeTypeBadge";
import { Card } from "@/components/ui/card";
import InlineBanner from "@/components/InlineBanner";
import WeatherStationManageModal from "@/features/nodes/components/WeatherStationManageModal";
import { isCoreNode } from "@/lib/coreNode";
import { formatBytes, formatDuration, formatPercent } from "@/lib/format";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { sha256Hex } from "@/lib/sha256";
import LiveWeatherPanel from "@/features/nodes/components/LiveWeatherPanel";
import RenogyBt2SettingsSection from "@/features/nodes/components/RenogyBt2SettingsSection";
import { deleteJson, putJson } from "@/lib/http";
import { useAuth } from "@/components/AuthProvider";
import { Input } from "@/components/ui/input";
import { formatSensorInterval, formatSensorValueWithUnit } from "@/lib/sensorFormat";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import type { TrendSeriesEntry } from "@/types/dashboard";

export default function NodeDetailPageClient() {
  const searchParams = useSearchParams();
  const nodeId = searchParams.get("id");
  const router = useRouter();
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const canViewBackups = Boolean(canEdit || me?.capabilities?.includes("backups.view"));
  const canDelete = Boolean(me?.role === "admin");
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const outputsQuery = useOutputsQuery();
  const alarmsQuery = useAlarmsQuery();
  const backupsQuery = useBackupsQuery({ enabled: canViewBackups });
  const pvForecastConfigQuery = usePvForecastConfigQuery(nodeId);
  const forecastStatusQuery = useForecastStatusQuery();
  const isLoading =
    nodesQuery.isLoading ||
    sensorsQuery.isLoading ||
    outputsQuery.isLoading ||
    alarmsQuery.isLoading ||
    backupsQuery.isLoading;
  const error =
    nodesQuery.error ||
    sensorsQuery.error ||
    outputsQuery.error ||
    alarmsQuery.error ||
    backupsQuery.error;
  const nodes = nodesQuery.data ?? [];
  const sensors = sensorsQuery.data ?? [];
  const outputs = outputsQuery.data ?? [];
  const alarms = alarmsQuery.data ?? [];
  const backups = backupsQuery.data ?? [];
  const pvConfig = pvForecastConfigQuery.data;
  const forecastSolarStatus = forecastStatusQuery.data?.providers?.["Forecast.Solar"];
  const node = nodeId ? nodes.find((n) => n.id === nodeId) ?? null : null;

  const coreNode = node ? isCoreNode(node) : false;
  const nodeOutputs = node ? outputs.filter((output) => output.node_id === node.id) : [];
  const nodeBackups = useMemo(() => {
    if (!node) return [];
    return backups
      .filter((backup) => backup.node_id === node.id)
      .slice()
      .sort((a, b) => {
        const aTs = new Date(a.captured_at).getTime();
        const bTs = new Date(b.captured_at).getTime();
        return (Number.isFinite(bTs) ? bTs : 0) - (Number.isFinite(aTs) ? aTs : 0);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed on node?.id to avoid resorting on every query refetch
  }, [backups, node?.id]);

  const config = (node?.config || {}) as Record<string, unknown>;
  const nodeKind = typeof config.kind === "string" ? config.kind : null;
  const isWeatherStation = nodeKind === "ws-2902";
  const hideLiveWeather = config["hide_live_weather"] === true;
  const [hideLiveWeatherDraft, setHideLiveWeatherDraft] = useState(false);
  const ipLast =
    typeof node?.ip_last === "string"
      ? node.ip_last
      : node?.ip_last
        ? JSON.stringify(node.ip_last)
        : "-";
  const configLatitude = typeof config.latitude === "number" ? config.latitude : null;
  const configLongitude = typeof config.longitude === "number" ? config.longitude : null;

  const forwarderConfig =
    config.forwarder && typeof config.forwarder === "object"
      ? (config.forwarder as Record<string, unknown>)
      : null;
  const spoolConfig =
    forwarderConfig?.spool && typeof forwarderConfig.spool === "object"
      ? (forwarderConfig.spool as Record<string, unknown>)
      : null;
  const spoolBytes = typeof spoolConfig?.spool_bytes === "number" ? spoolConfig.spool_bytes : null;
  const maxSpoolBytes =
    typeof spoolConfig?.max_spool_bytes === "number" ? spoolConfig.max_spool_bytes : null;
  const ackedSeq = typeof spoolConfig?.acked_seq === "number" ? spoolConfig.acked_seq : null;
  const nextSeq = typeof spoolConfig?.next_seq === "number" ? spoolConfig.next_seq : null;
  const freeBytes = typeof spoolConfig?.free_bytes === "number" ? spoolConfig.free_bytes : null;
  const keepFreeBytes =
    typeof spoolConfig?.keep_free_bytes === "number" ? spoolConfig.keep_free_bytes : null;
  const backlogSamples =
    typeof spoolConfig?.backlog_samples === "number" ? spoolConfig.backlog_samples : null;
  const estimatedDrainSeconds =
    typeof spoolConfig?.estimated_drain_seconds === "number"
      ? spoolConfig.estimated_drain_seconds
      : null;
  const lossesPending =
    typeof spoolConfig?.losses_pending === "number" ? spoolConfig.losses_pending : null;
  const oldestUnackedTimestampMs =
    typeof spoolConfig?.oldest_unacked_timestamp_ms === "number"
      ? spoolConfig.oldest_unacked_timestamp_ms
      : null;

  const [renameOpen, setRenameOpen] = useState(false);
  const [nameDraft, setNameDraft] = useState<string>("");
  const [renameBusy, setRenameBusy] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);

  const [locationLatDraft, setLocationLatDraft] = useState("");
  const [locationLngDraft, setLocationLngDraft] = useState("");
  const [locationBusy, setLocationBusy] = useState(false);
  const [locationError, setLocationError] = useState<string | null>(null);
  const [locationSavedAt, setLocationSavedAt] = useState<number | null>(null);

  const [healthOpen, setHealthOpen] = useState(false);
  const [healthBusy, setHealthBusy] = useState(false);
  const [healthError, setHealthError] = useState<string | null>(null);
  const [healthSeries, setHealthSeries] = useState<TrendSeriesEntry[]>([]);
  const [sensorSearch, setSensorSearch] = useState("");

  const [weatherStationManageOpen, setWeatherStationManageOpen] = useState(false);

  const [deleteOpen, setDeleteOpen] = useState(false);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const [hideLiveWeatherBusy, setHideLiveWeatherBusy] = useState(false);
  const [hideLiveWeatherError, setHideLiveWeatherError] = useState<string | null>(null);

  const nodeSensors = useMemo(() => {
    if (!node) return [];
    return sensors.filter((sensor) => sensor.node_id === node.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed on node?.id to avoid re-filtering on every object reference change
  }, [node?.id, sensors]);

  const nodeAlarms = useMemo(() => {
    if (!node) return [];
    return alarms.filter((alarm) => {
      if (alarm.target_type === "node") return alarm.target_id === node.id;
      if (alarm.target_type === "sensor") {
        return nodeSensors.some((sensor) => sensor.sensor_id === alarm.target_id);
      }
      return false;
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- keyed on node?.id
  }, [alarms, node?.id, nodeSensors]);

  useEffect(() => {
    if (!node) return;
    setNameDraft(node.name);
    setRenameOpen(false);
    setRenameError(null);
    setWeatherStationManageOpen(false);
    setDeleteOpen(false);
    setDeleteBusy(false);
    setDeleteError(null);
    setHideLiveWeatherBusy(false);
    setHideLiveWeatherError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- reset form state on node change, not on every object reference change
  }, [node?.id, node?.name]);

  useEffect(() => {
    if (!node) return;
    setHideLiveWeatherDraft(hideLiveWeather);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- reset draft on node switch
  }, [hideLiveWeather, node?.id]);

  useEffect(() => {
    if (!node) return;
    const nextConfig = (node.config || {}) as Record<string, unknown>;
    const lat = typeof nextConfig.latitude === "number" ? nextConfig.latitude : null;
    const lng = typeof nextConfig.longitude === "number" ? nextConfig.longitude : null;
    setLocationLatDraft(lat == null ? "" : String(lat));
    setLocationLngDraft(lng == null ? "" : String(lng));
    setLocationError(null);
    setLocationSavedAt(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- reset location draft on node switch
  }, [node?.id]);

  useEffect(() => {
    if (!node || !healthOpen) return;
    let active = true;
    setHealthBusy(true);
    setHealthError(null);

    const nodeHealthSensorId = async (nodeUuid: string, key: string) => {
      const payload = `node_health|${nodeUuid.trim().toLowerCase()}|${key.trim().toLowerCase()}`;
      const hex = await sha256Hex(payload);
      return hex.slice(0, 24);
    };

    const load = async () => {
      const keys = [
        "cpu_percent",
        "memory_percent",
        "storage_used_bytes",
        "memory_used_bytes",
        "ping_ms",
        "ping_p50_30m_ms",
        "ping_jitter_ms",
        "mqtt_broker_rtt_ms",
        "mqtt_broker_rtt_jitter_ms",
        "uptime_percent_24h",
        ...Array.from({ length: 8 }, (_, idx) => `cpu_core_${idx}_percent`),
      ];
      const unitByKey: Record<string, string> = {
        cpu_percent: "%",
        memory_percent: "%",
        storage_used_bytes: "bytes",
        memory_used_bytes: "bytes",
        ping_ms: "ms",
        ping_p50_30m_ms: "ms",
        ping_jitter_ms: "ms",
        mqtt_broker_rtt_ms: "ms",
        mqtt_broker_rtt_jitter_ms: "ms",
        uptime_percent_24h: "%",
      };
      for (let idx = 0; idx < 8; idx += 1) {
        unitByKey[`cpu_core_${idx}_percent`] = "%";
      }

      const pairs = await Promise.all(
        keys.map(async (key) => ({ key, sensorId: await nodeHealthSensorId(node.id, key) })),
      );
      const sensorIds = pairs.map((p) => p.sensorId);
      const now = Date.now();
      const end = new Date(now).toISOString();
      const start = new Date(now - 6 * 60 * 60 * 1000).toISOString();
      const intervalSeconds = 60;
      const series = await fetchMetricsSeries(sensorIds, start, end, intervalSeconds);
      const keyBySensorId = new Map(pairs.map((p) => [p.sensorId, p.key]));
      const normalized = series
        .filter((entry) => entry.points.length > 0)
        .map((entry) => {
          const key = keyBySensorId.get(entry.sensor_id);
          const unit = key ? unitByKey[key] : undefined;
          return { ...entry, unit };
        });

      if (active) {
        setHealthSeries(normalized);
      }
    };

    load()
      .catch((err) => {
        if (!active) return;
        setHealthError(err instanceof Error ? err.message : "Failed to load node health history.");
        setHealthSeries([]);
      })
      .finally(() => {
        if (!active) return;
        setHealthBusy(false);
      });

    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fetch health data on node switch or panel open
  }, [healthOpen, node?.id]);

  const saveName = async () => {
    if (!canEdit || !node) return;
    const name = nameDraft.trim();
    if (!name) {
      setRenameError("Node name cannot be empty.");
      return;
    }
    setRenameBusy(true);
    setRenameError(null);
    try {
      await putJson(`/api/nodes/${encodeURIComponent(node.id)}`, { name });
      await queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
      setRenameOpen(false);
    } catch (err) {
      setRenameError(err instanceof Error ? err.message : "Failed to rename node.");
    } finally {
      setRenameBusy(false);
    }
  };

  const parseLatLng = (value: string) => {
    const trimmed = value.trim();
    if (!trimmed) return null;
    const parsed = Number.parseFloat(trimmed);
    if (!Number.isFinite(parsed)) return null;
    return parsed;
  };

  const saveLocation = async () => {
    if (!canEdit || !node) return;
    setLocationBusy(true);
    setLocationError(null);
    setLocationSavedAt(null);

    try {
      const lat = parseLatLng(locationLatDraft);
      const lng = parseLatLng(locationLngDraft);

      const latPresent = locationLatDraft.trim().length > 0;
      const lngPresent = locationLngDraft.trim().length > 0;

      if (latPresent !== lngPresent) {
        throw new Error("Enter both latitude and longitude (or clear both).");
      }

      const nextConfig = { ...(((node.config || {}) as Record<string, unknown>) ?? {}) };

      if (!latPresent && !lngPresent) {
        delete nextConfig.latitude;
        delete nextConfig.longitude;
      } else {
        if (lat == null || lng == null) {
          throw new Error("Latitude/longitude must be valid numbers.");
        }
        if (lat < -90 || lat > 90) {
          throw new Error("Latitude must be between -90 and 90.");
        }
        if (lng < -180 || lng > 180) {
          throw new Error("Longitude must be between -180 and 180.");
        }
        nextConfig.latitude = lat;
        nextConfig.longitude = lng;
        nextConfig.location_source = "manual";
        nextConfig.location_updated_at = new Date().toISOString();
      }

      await putJson(`/api/nodes/${encodeURIComponent(node.id)}`, { config: nextConfig });
      await queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
      setLocationSavedAt(Date.now());
    } catch (err) {
      setLocationError(err instanceof Error ? err.message : "Failed to update location.");
    } finally {
      setLocationBusy(false);
    }
  };

  const setLiveWeatherHidden = async (nextHidden: boolean) => {
    if (!canEdit || !node) return false;
    setHideLiveWeatherBusy(true);
    setHideLiveWeatherError(null);
    try {
      const nextConfig = { ...(((node.config || {}) as Record<string, unknown>) ?? {}), hide_live_weather: nextHidden };
      await putJson(`/api/nodes/${encodeURIComponent(node.id)}`, { config: nextConfig });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.mapFeatures }),
      ]);
      return true;
    } catch (err) {
      setHideLiveWeatherError(
        err instanceof Error ? err.message : "Failed to update public provider visibility.",
      );
      return false;
    } finally {
      setHideLiveWeatherBusy(false);
    }
  };

  const deleteNode = async () => {
    if (!canDelete || !node || coreNode) return;
    setDeleteBusy(true);
    setDeleteError(null);
    try {
      await deleteJson(`/api/nodes/${encodeURIComponent(node.id)}`);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
        queryClient.invalidateQueries({ queryKey: queryKeys.outputs }),
        queryClient.invalidateQueries({ queryKey: queryKeys.alarms }),
      ]);
      router.push("/nodes");
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : "Failed to delete node.");
    } finally {
      setDeleteBusy(false);
    }
  };

  const visibleSensors = useMemo(() => {
    const query = sensorSearch.trim().toLowerCase();
    if (!query) return nodeSensors;
    return nodeSensors.filter((sensor) => {
      return (
        sensor.name.toLowerCase().includes(query) ||
        sensor.sensor_id.toLowerCase().includes(query) ||
        sensor.type.toLowerCase().includes(query) ||
        sensor.unit.toLowerCase().includes(query)
      );
    });
  }, [nodeSensors, sensorSearch]);

  if (isLoading) return <LoadingState label="Loading node..." />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load node."} />;
  }

  if (!nodeId) {
    return (
      <div className="space-y-4">
        <ErrorState message="Missing node id." />
        <NodeButton onClick={() => router.push("/nodes")}>Back to nodes</NodeButton>
      </div>
    );
  }

  if (!node) {
    return (
      <div className="space-y-4">
        <ErrorState message="Node not found." />
        <NodeButton onClick={() => router.push("/nodes")}>Back to nodes</NodeButton>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <NodeButton onClick={() => router.push("/nodes")} size="sm">
          ← Back to Nodes
        </NodeButton>
      </div>

      <Card className="p-6">
        <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
          <div className="space-y-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Node</p>
            {renameOpen ? (
              <div className="space-y-2">
                <div className="flex w-full max-w-md items-center gap-2">
                  <Input
                    className="min-w-0 flex-1 text-base font-semibold"
                    value={nameDraft}
                    onChange={(e) => setNameDraft(e.target.value)}
                    disabled={!canEdit || renameBusy}
                  />
                  <NodeTypeBadge node={node} size="md" className="shrink-0" />
                </div>
 <p className="text-xs text-muted-foreground">{node.id}</p>
                {renameError ? (
 <p className="text-sm text-rose-600">{renameError}</p>
                ) : null}
              </div>
            ) : (
              <div>
                <div className="flex min-w-0 items-center gap-2">
 <h2 className="min-w-0 truncate text-2xl font-semibold text-foreground">
                    {node.name}
                  </h2>
                  <NodeTypeBadge node={node} size="md" className="shrink-0" />
                </div>
 <p className="text-xs text-muted-foreground">{node.id}</p>
              </div>
            )}
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {canEdit && !renameOpen ? (
              <NodeButton size="sm" onClick={() => setRenameOpen(true)}>
                Rename
              </NodeButton>
            ) : null}
            {renameOpen ? (
              <>
                <NodeButton
                  size="sm"
                  variant="primary"
                  onClick={() => void saveName()}
                  disabled={!canEdit || renameBusy || !nameDraft.trim()}
                >
                  {renameBusy ? "Saving…" : "Save"}
                </NodeButton>
                <NodeButton
                  size="sm"
                  onClick={() => {
                    setRenameOpen(false);
                    setNameDraft(node.name);
                    setRenameError(null);
                  }}
                  disabled={renameBusy}
                >
                  Cancel
                </NodeButton>
              </>
            ) : null}
            <NodePill tone={node.status === "online" ? "success" : "muted"} caps>
              {formatNodeStatusLabel(node.status, node.last_seen)}
            </NodePill>
          </div>
        </div>
      </Card>

      <CollapsibleCard
        title="Overview"
        description="Identity, status, and node health snapshot."
        defaultOpen
        actions={
          <NodePill tone={node.status === "online" ? "success" : "muted"} size="sm" caps>
            {node.status}
          </NodePill>
        }
      >
 <div className="grid grid-cols-2 gap-3 text-sm text-foreground">
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Uptime</p>
            <p>{formatDuration(node.uptime_seconds)}</p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">CPU</p>
            <p>{formatPercent(node.cpu_percent)}</p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">RAM</p>
            <p>
              {formatPercent(node.memory_percent ?? null)}
              {node.memory_used_bytes != null ? ` (${formatBytes(node.memory_used_bytes)})` : ""}
            </p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Storage used
            </p>
            <p>{formatBytes(node.storage_used_bytes)}</p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Ping (latest / 30m)
            </p>
            <p>
              {(() => {
                const ping = node.ping_ms ?? node.network_latency_ms;
                const ping30m = node.ping_p50_30m_ms;
                if (typeof ping !== "number" && typeof ping30m !== "number") return "—";
                const pingLabel =
                  typeof ping === "number" && Number.isFinite(ping) ? `${Math.round(ping)}ms` : "—";
                const ping30mLabel =
                  typeof ping30m === "number" && Number.isFinite(ping30m)
                    ? `${Math.round(ping30m)}ms`
                    : "—";
                return `${pingLabel} / ${ping30mLabel}`;
              })()}
            </p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Ping jitter
            </p>
            <p>
              {(() => {
                const jitter = node.ping_jitter_ms ?? node.network_jitter_ms;
                if (typeof jitter !== "number" || !Number.isFinite(jitter)) return "—";
                return `${Math.round(jitter)}ms`;
              })()}
            </p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Link uptime (24h)
            </p>
            <p>{formatPercent(node.uptime_percent_24h ?? null)}</p>
          </div>
        </div>

 <div className="mt-4 grid grid-cols-2 gap-3 text-sm text-foreground">
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">IP</p>
            <p>{ipLast}</p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Mesh role
            </p>
            <p>{(config["mesh_role"] as string | undefined) ?? "-"}</p>
          </div>
          <div className="col-span-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Location
            </p>
 <p className="text-sm text-foreground">
              {configLatitude != null && configLongitude != null
                ? `${configLatitude.toFixed(6)}°, ${configLongitude.toFixed(6)}°`
                : "Not set"}
            </p>
 <p className="mt-1 text-xs text-muted-foreground">
              Used for location-based features (weather, PV forecast). For pin placement, use the Map tab.
            </p>

            <div className="mt-3 grid grid-cols-2 gap-2">
              <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Latitude (°)
                </span>
                <Input
                  value={locationLatDraft}
                  onChange={(e) => setLocationLatDraft(e.target.value)}
                  disabled={!canEdit || locationBusy}
                  inputMode="decimal"
                  placeholder="e.g. 37.123456"
                />
              </label>
              <label className="space-y-1">
 <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Longitude (°)
                </span>
                <Input
                  value={locationLngDraft}
                  onChange={(e) => setLocationLngDraft(e.target.value)}
                  disabled={!canEdit || locationBusy}
                  inputMode="decimal"
                  placeholder="e.g. -122.123456"
                />
              </label>
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-2">
              <NodeButton
                size="xs"
                variant="primary"
                onClick={() => void saveLocation()}
                disabled={!canEdit || locationBusy}
              >
                {locationBusy ? "Saving…" : "Save location"}
              </NodeButton>
              <NodeButton
                size="xs"
                onClick={() => {
                  setLocationLatDraft("");
                  setLocationLngDraft("");
                  setLocationError(null);
                  setLocationSavedAt(null);
                }}
                disabled={!canEdit || locationBusy}
              >
                Clear
              </NodeButton>
              {locationSavedAt ? (
 <span className="text-xs text-emerald-700">
                  Saved {formatDistanceToNow(new Date(locationSavedAt), { addSuffix: true })}
                </span>
              ) : null}
            </div>
            {locationError ? (
 <p className="mt-2 text-sm text-rose-600">{locationError}</p>
            ) : null}
            {!canEdit ? (
 <p className="mt-2 text-xs text-muted-foreground">
                You don’t have permission to edit location (requires config.write).
              </p>
            ) : null}
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">MAC (eth)</p>
            <p className="break-all">{node.mac_eth ?? "-"}</p>
          </div>
          <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">MAC (wifi)</p>
            <p className="break-all">{node.mac_wifi ?? "-"}</p>
          </div>
        </div>

        <div className="mt-4 flex items-center justify-between gap-2">
          <div className="flex flex-wrap items-center gap-2">
            <NodeButton size="xs" onClick={() => setHealthOpen((prev) => !prev)} disabled={healthBusy}>
              {healthOpen ? "Hide health history" : "Show health history"}
            </NodeButton>
          </div>
          {node.last_seen ? (
 <p className="text-xs text-muted-foreground">
              Last seen{" "}
              {(() => {
                const ts = node.last_seen instanceof Date ? node.last_seen : new Date(node.last_seen);
                if (Number.isNaN(ts.getTime())) return "—";
                return formatDistanceToNow(ts, { addSuffix: true });
              })()}
            </p>
          ) : null}
        </div>

        {healthOpen ? (
          <div className="mt-4 space-y-3">
 {healthError ? <p className="text-sm text-rose-600">{healthError}</p> : null}
            {healthBusy ? (
 <p className="text-sm text-muted-foreground">Loading health history…</p>
            ) : null}
            {!healthBusy && !healthSeries.length ? (
 <p className="text-sm text-muted-foreground">
                No health history yet. Wait for the node status heartbeat to publish and refresh.
              </p>
            ) : null}
            {healthSeries.length ? (
              <>
                <TrendChart
                  data={healthSeries.filter((series) => {
                    const label = (series.label ?? "").toLowerCase();
                    return label.includes("cpu") || label.includes("memory");
                  })}
                  heightClassName="h-56"
                />
                <TrendChart
                  data={healthSeries.filter((series) => {
                    const label = (series.label ?? "").toLowerCase();
                    return (
                      label.includes("ping") ||
                      label.includes("mqtt") ||
                      label.includes("storage") ||
                      label.includes("uptime")
                    );
                  })}
                  independentAxes
                  heightClassName="h-56"
                />
              </>
            ) : null}
          </div>
        ) : null}
      </CollapsibleCard>

      {isWeatherStation ? (
        <CollapsibleCard
          title="Weather station"
          description="Rotate the station token and view last upload status."
          defaultOpen={false}
          actions={
            canEdit ? (
              <NodeButton size="xs" onClick={() => setWeatherStationManageOpen(true)}>
                Rotate token / setup
              </NodeButton>
            ) : (
 <span className="rounded-full bg-muted px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                Read-only
              </span>
            )
          }
        >
 <p className="text-sm text-muted-foreground">
            This node is configured as a WS-2902 weather station. Use the action above to rotate the token and confirm the ingest URL.
          </p>
        </CollapsibleCard>
      ) : null}

      <CollapsibleCard
        title="Telemetry buffering"
        description="Offline-safe disk spool and replay status from node-forwarder."
        defaultOpen={false}
        actions={
          typeof backlogSamples === "number" && backlogSamples > 0 ? (
            <NodePill tone="warning" size="sm" caps>
              backlog {Math.round(backlogSamples).toLocaleString()}
            </NodePill>
          ) : typeof lossesPending === "number" && lossesPending > 0 ? (
            <NodePill tone="danger" size="sm" caps>
              loss {Math.round(lossesPending).toLocaleString()}
            </NodePill>
          ) : (
            <NodePill tone="muted" size="sm" caps>
              idle
            </NodePill>
          )
        }
      >
        {forwarderConfig ? (
          <div className="mt-3 space-y-3">
            {spoolConfig ? (
              <Card className="rounded-lg gap-0 bg-card-inset p-4">
                <p className="text-sm font-semibold text-card-foreground">Spool status</p>
                <dl className="mt-3 grid gap-3 md:grid-cols-2">
                  <InfoBlock
                    label="Backlog samples"
                    value={
                      typeof backlogSamples === "number"
                        ? Math.round(backlogSamples).toLocaleString()
                        : "—"
                    }
                  />
                  <InfoBlock
                    label="ACKed seq"
                    value={(() => {
                      if (typeof ackedSeq !== "number") return "—";
                      const label = Math.round(ackedSeq).toLocaleString();
                      if (typeof nextSeq !== "number") return label;
                      return `${label} / ${Math.round(nextSeq).toLocaleString()}`;
                    })()}
                  />
                  <InfoBlock
                    label="Spool usage"
                    value={(() => {
                      if (typeof spoolBytes !== "number" || typeof maxSpoolBytes !== "number") {
                        return "—";
                      }
                      const pct = maxSpoolBytes > 0 ? (spoolBytes / maxSpoolBytes) * 100 : null;
                      const pctLabel =
                        typeof pct === "number" && Number.isFinite(pct) ? ` (${formatPercent(pct)})` : "";
                      return `${formatBytes(spoolBytes)} / ${formatBytes(maxSpoolBytes)}${pctLabel}`;
                    })()}
                  />
                  <InfoBlock
                    label="Disk free floor"
                    value={(() => {
                      if (typeof freeBytes !== "number" || typeof keepFreeBytes !== "number") return "—";
                      return `${formatBytes(freeBytes)} free · keep ${formatBytes(keepFreeBytes)} free`;
                    })()}
                  />
                  <InfoBlock
                    label="Estimated drain"
                    value={
                      typeof estimatedDrainSeconds === "number"
                        ? formatDuration(Math.round(estimatedDrainSeconds))
                        : "—"
                    }
                  />
                  <InfoBlock
                    label="Loss ranges pending"
                    value={
                      typeof lossesPending === "number"
                        ? Math.round(lossesPending).toLocaleString()
                        : "—"
                    }
                  />
                  <InfoBlock
                    label="Oldest unacked sample"
                    value={(() => {
                      if (typeof oldestUnackedTimestampMs !== "number") return "—";
                      const date = new Date(oldestUnackedTimestampMs);
                      if (Number.isNaN(date.getTime())) return "—";
                      return formatDistanceToNow(date, { addSuffix: true });
                    })()}
                  />
                  <InfoBlock
                    label="Node → forwarder queue"
                    value={(() => {
                      const queueLen =
                        typeof forwarderConfig.queue_len === "number"
                          ? Math.round(forwarderConfig.queue_len).toLocaleString()
                          : "—";
                      const dropped =
                        typeof forwarderConfig.dropped_samples === "number"
                          ? Math.round(forwarderConfig.dropped_samples).toLocaleString()
                          : "—";
                      return `${queueLen} queued · ${dropped} dropped`;
                    })()}
                  />
                </dl>
                {typeof forwarderConfig.last_error === "string" && forwarderConfig.last_error.trim() ? (
                  <InlineBanner tone="warning" className="mt-3 px-3 py-2 text-sm">
                    node-forwarder warning: {forwarderConfig.last_error}
                  </InlineBanner>
                ) : null}
              </Card>
            ) : (
              <InlineBanner tone="warning" className="px-3 py-2 text-sm">
                Node reported forwarder state, but spool details are missing. Verify node-forwarder is
                running and reachable on localhost.
              </InlineBanner>
            )}
          </div>
        ) : (
          <InlineBanner tone="warning" className="mt-3 px-3 py-2 text-sm">
            This node is not reporting node-forwarder spool status yet. Deploy the updated node stack
            to enable offline buffering visibility here.
          </InlineBanner>
        )}
      </CollapsibleCard>

      <CollapsibleCard
        title="Sensors & Outputs"
        description="Manage IO configuration (hardware sensors, alarms, output commands)."
        defaultOpen
        actions={
          <div className="flex flex-wrap items-center gap-2">
 <span className="text-xs text-muted-foreground">
              {nodeSensors.length} sensors · {nodeOutputs.length} outputs
            </span>
            <NodeButton size="xs" onClick={() => router.push(`/sensors?node=${encodeURIComponent(node.id)}`)}>
              Open Sensors & Outputs
            </NodeButton>
          </div>
        }
      >
        <div className="space-y-3">
          <Card className="rounded-lg gap-0 bg-card-inset p-3">
            <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
              <div className="space-y-0.5">
                <p className="text-sm font-semibold text-card-foreground">
                  Public provider data (Open-Meteo weather)
                </p>
 <p className="text-xs text-muted-foreground">
                  Hide public provider weather sensors for this node. Data continues to be stored.
                </p>
              </div>
              {canEdit ? (
 <label className="inline-flex items-center gap-2 text-sm font-semibold text-foreground">
                  <input
                    type="checkbox"
                    checked={hideLiveWeatherDraft}
                    disabled={hideLiveWeatherBusy}
                    onChange={(e) => {
                      const nextHidden = e.target.checked;
                      const previousHidden = hideLiveWeatherDraft;
                      setHideLiveWeatherDraft(nextHidden);
                      void (async () => {
                        const ok = await setLiveWeatherHidden(nextHidden);
                        if (!ok) setHideLiveWeatherDraft(previousHidden);
                      })();
                    }}
                    aria-label="Hide public provider data (Open-Meteo)"
                  />
                  <span>Hide public provider data</span>
                </label>
              ) : (
 <span className="rounded-full bg-muted px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
                  Read-only
                </span>
              )}
            </div>
            {hideLiveWeatherError ? (
 <p className="mt-2 text-sm text-rose-600">{hideLiveWeatherError}</p>
            ) : null}
          </Card>

          {hideLiveWeatherDraft ? null : <LiveWeatherPanel nodeId={node.id} />}
        </div>

        <div className="mt-4 space-y-2">
          {nodeSensors.length ? (
            <>
              <label className="block">
                <span className="sr-only">Search sensors</span>
                <Input
                  placeholder="Search sensors (name, type, id)…"
                  value={sensorSearch}
                  onChange={(e) => setSensorSearch(e.target.value)}
                />
              </label>
              <div className="max-h-96 overflow-y-auto overflow-x-auto rounded-lg border border-border md:overflow-x-visible">
                <table className="min-w-full divide-y divide-border text-sm">
                  <thead className="sticky top-0 bg-card-inset">
                    <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Sensor
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Type
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Interval
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Latest
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {visibleSensors.map((sensor) => {
                      const interval = formatSensorInterval(sensor.interval_seconds);
                      return (
                        <tr
                          key={sensor.sensor_id}
 className="cursor-pointer hover:bg-muted"
                          onClick={() =>
                            router.push(
                              `/sensors?node=${encodeURIComponent(node.id)}&sensor=${encodeURIComponent(sensor.sensor_id)}`,
                            )
                          }
                        >
                          <td className="px-3 py-2">
                            <div className="flex flex-wrap items-center gap-2">
 <div className="font-medium text-foreground">{sensor.name}</div>
                              <SensorOriginBadge sensor={sensor} size="xs" />
                            </div>
 <div className="text-[11px] text-muted-foreground">{sensor.sensor_id}</div>
                          </td>
 <td className="px-3 py-2 text-muted-foreground">
                            {sensor.type} · {sensor.unit}
                          </td>
 <td className="px-3 py-2 text-muted-foreground">
                            <span title={interval.title}>{interval.label}</span>
                          </td>
 <td className="px-3 py-2 text-muted-foreground">
                            {formatSensorValueWithUnit(sensor, sensor.latest_value, "—")}
                          </td>
                        </tr>
                      );
                    })}
                    {nodeSensors.length > 0 && visibleSensors.length === 0 ? (
                      <tr>
 <td colSpan={4} className="px-3 py-3 text-sm text-muted-foreground">
                          No sensors match your search.
                        </td>
                      </tr>
                    ) : null}
                  </tbody>
                </table>
              </div>
            </>
          ) : (
 <p className="text-sm text-muted-foreground">No sensors configured.</p>
          )}
        </div>
      </CollapsibleCard>

      <DisplayProfileSection node={node} sensors={sensors} ipLast={ipLast} />

      <CollapsibleCard
        title="Alarms"
        description="Alarm rules targeting this node or its sensors."
        defaultOpen={false}
      >
        {nodeAlarms.length ? (
          <div className="mt-3 space-y-2 text-sm text-foreground">
            {nodeAlarms.map((alarm) => (
              <Card
                key={alarm.id}
                className="gap-0 rounded-lg px-3 py-2 shadow-xs"
              >
                <div className="flex items-center justify-between">
                  <p className="font-semibold text-card-foreground">{alarm.name}</p>
                  <NodePill tone={alarm.severity === "critical" ? "danger" : "warning"} size="sm" caps>
                    {alarm.severity}
                  </NodePill>
                </div>
 <p className="text-xs text-muted-foreground">{JSON.stringify(alarm.condition)}</p>
              </Card>
            ))}
          </div>
        ) : (
 <p className="mt-3 text-sm text-muted-foreground">No alarms defined for this node.</p>
        )}
      </CollapsibleCard>

      <CollapsibleCard
        title="Backups"
        description="Recent snapshots for rapid restore / reprovisioning."
        defaultOpen={false}
        actions={
          <NodeButton size="xs" onClick={() => window.location.assign("/backups")}>
            Open Backups
          </NodeButton>
        }
      >
        {nodeBackups.length ? (
          <div className="mt-3 space-y-2 text-sm text-foreground">
            {nodeBackups.map((backup) => {
              const capturedAt = backup.captured_at ? new Date(backup.captured_at) : null;
              const capturedLabel =
                capturedAt && !Number.isNaN(capturedAt.getTime())
                  ? `Captured ${formatDistanceToNow(capturedAt, { addSuffix: true })}`
                  : "Capture time unavailable";
              const sizeLabel = backup.size_bytes != null ? `${(backup.size_bytes / 1024).toFixed(1)} KB` : "--";
              return (
                <Card
                  key={backup.id}
                  className="flex-row items-center justify-between gap-0 rounded-lg px-3 py-2 shadow-xs"
                >
                  <div>
                    <p className="font-semibold text-card-foreground">{backup.path}</p>
 <p className="text-xs text-muted-foreground">{capturedLabel}</p>
                  </div>
 <span className="text-xs text-muted-foreground">{sizeLabel}</span>
                </Card>
              );
            })}
          </div>
        ) : (
 <p className="mt-3 text-sm text-muted-foreground">No backups captured yet.</p>
        )}
      </CollapsibleCard>

      <CollapsibleCard
        title="Node hardware"
        description="Configure devices and apply settings safely via the controller."
        defaultOpen={false}
      >
        <RenogyBt2SettingsSection
          nodeId={node.id}
          nodeName={node.name}
          nodeConfig={(node.config as Record<string, unknown> | null | undefined) ?? null}
          nodeStatus={node.status}
          nodeSensors={nodeSensors}
          canEdit={canEdit}
        />
      </CollapsibleCard>

      <CollapsibleCard
        title="PV forecast (Forecast.Solar)"
        description="PV panel parameters are configured in Setup Center and applied per-node."
        defaultOpen={false}
        actions={
          <NodeButton
            size="sm"
            variant="primary"
            onClick={() => router.push(`/setup?pvNode=${encodeURIComponent(node.id)}#pv-forecast`)}
          >
            Configure in Setup Center
          </NodeButton>
        }
      >
        <div className="grid gap-6 lg:grid-cols-2">
          <div className="space-y-4">
            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <p className="text-sm font-semibold text-card-foreground">Current settings</p>
              {pvForecastConfigQuery.isLoading ? (
 <p className="mt-2 text-sm text-muted-foreground">Loading PV forecast settings…</p>
              ) : pvConfig ? (
                <dl className="mt-3 grid gap-3 md:grid-cols-2">
                  <InfoBlock label="Enabled" value={pvConfig.enabled ? "Yes" : "No"} />
                  <InfoBlock label="Capacity (kWp)" value={pvConfig.kwp} />
                  <InfoBlock label="Latitude (°)" value={pvConfig.latitude} />
                  <InfoBlock label="Longitude (°)" value={pvConfig.longitude} />
                  <InfoBlock label="Tilt (°)" value={pvConfig.tilt_deg} />
                  <InfoBlock label="Azimuth (°)" value={pvConfig.azimuth_deg} />
                  <InfoBlock label="Timestamp format" value={pvConfig.time_format} />
                  <InfoBlock
                    label="Updated"
                    value={pvConfig.updated_at ? new Date(pvConfig.updated_at).toLocaleString() : "—"}
                  />
                </dl>
              ) : (
 <p className="mt-2 text-sm text-muted-foreground">PV forecast is not configured for this node yet.</p>
              )}
            </Card>

            <Card className="rounded-lg gap-0 bg-card-inset p-4">
              <p className="text-sm font-semibold text-card-foreground">Provider status</p>
 <div className="mt-2 flex flex-col gap-1 text-sm text-foreground">
                <div className="flex items-center justify-between">
                  <span>Forecast.Solar</span>
                  <span className="text-xs font-semibold uppercase tracking-wide">
                    {forecastSolarStatus?.status ?? "unknown"}
                  </span>
                </div>
 <p className="text-xs text-muted-foreground">
                  {forecastSolarStatus?.details
                    ? forecastSolarStatus.details
                    : forecastSolarStatus?.last_seen
                      ? `Last poll ${new Date(forecastSolarStatus.last_seen).toLocaleString()}`
                      : "No poll recorded yet."}
                </p>
              </div>
            </Card>
          </div>
        </div>
      </CollapsibleCard>

      {canDelete ? (
        <CollapsibleCard
          title="Danger zone"
          description="Soft delete marks the node as deleted and preserves telemetry history. This does not uninstall the node."
          defaultOpen={false}
        >

          {coreNode ? (
 <p className="mt-3 text-sm text-muted-foreground">The Core node cannot be deleted.</p>
          ) : node.status === "deleted" ? (
 <p className="mt-3 text-sm text-muted-foreground">This node is already deleted.</p>
          ) : (
            <div className="mt-3 space-y-3">
              {deleteError ? (
                <InlineBanner tone="danger" className="px-3 py-2 text-sm">
                  {deleteError}
                </InlineBanner>
              ) : null}

                {deleteOpen ? (
                  <div className="space-y-3">
 <p className="text-sm text-foreground">
                      Are you sure? The node will be renamed with a <span className="font-mono">-deleted-</span>{" "}
                      suffix, hidden from the dashboard UI, its MAC/IP bindings will be cleared, and linked sensors will be marked deleted.
                    </p>
                    <div className="flex flex-wrap gap-2">
                      <NodeButton size="sm" onClick={() => setDeleteOpen(false)} disabled={deleteBusy}>
                        Cancel
                      </NodeButton>
                    <NodeButton
                      size="sm"
 className="border border-rose-200 bg-white text-rose-700 shadow-xs hover:bg-rose-50 focus:outline-hidden focus:bg-rose-50"
                      onClick={() => void deleteNode()}
                      loading={deleteBusy}
                    >
                      Delete node
                    </NodeButton>
                  </div>
                </div>
              ) : (
                <NodeButton
                  size="sm"
 className="border border-rose-200 bg-white text-rose-700 shadow-xs hover:bg-rose-50 focus:outline-hidden focus:bg-rose-50"
                  onClick={() => setDeleteOpen(true)}
                >
                  Delete node
                </NodeButton>
              )}
            </div>
          )}
        </CollapsibleCard>
      ) : null}

      <WeatherStationManageModal
        open={weatherStationManageOpen}
        nodeId={isWeatherStation ? node.id : null}
        nodeName={node.name}
        onClose={() => setWeatherStationManageOpen(false)}
      />
    </div>
  );
}

const InfoBlock = ({ label, value }: { label: string; value: React.ReactNode }) => (
  <div>
 <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{label}</dt>
 <dd className="mt-1 text-base font-medium text-foreground">{value}</dd>
  </div>
);
