"use client";

import { useCallback, useEffect, useMemo, useState } from "react";

import {
  fetchControllerRuntimeConfig,
  fetchSetupDaemonConfig,
  fetchSetupDaemonLocalIp,
  fetchSetupDaemonPreflightChecks,
  postControllerRuntimeConfigPatch,
  postSetupDaemonConfigPatch,
} from "../api/setupDaemon";
import type {
  ControllerRuntimeConfig,
  SetupDaemonConfig,
  SetupDaemonConfigDraft,
  SetupDaemonLocalIp,
  SetupDaemonPreflightCheck,
} from "../lib/setupDaemonParsers";
import { parseCount, parseMillis, parsePort, parseSeconds } from "../lib/validation";
import type { Message } from "../types";

const mergeRuntimeConfig = ({
  daemonConfig,
  runtime,
}: {
  daemonConfig: SetupDaemonConfig;
  runtime: ControllerRuntimeConfig | null;
}): SetupDaemonConfig => {
  if (!runtime) return daemonConfig;
  return {
    ...daemonConfig,
    mqtt_username: runtime.mqtt_username,
    mqtt_password_configured: runtime.mqtt_password_configured,
    enable_analytics_feeds: runtime.enable_analytics_feeds,
    enable_forecast_ingestion: runtime.enable_forecast_ingestion,
    analytics_feed_poll_interval_seconds: runtime.analytics_feed_poll_interval_seconds,
    forecast_poll_interval_seconds: runtime.forecast_poll_interval_seconds,
    schedule_poll_interval_seconds: runtime.schedule_poll_interval_seconds,
    offline_threshold_seconds: runtime.offline_threshold_seconds,
    sidecar_mqtt_topic_prefix: runtime.sidecar_mqtt_topic_prefix,
    sidecar_mqtt_keepalive_secs: runtime.sidecar_mqtt_keepalive_secs,
    sidecar_enable_mqtt_listener: runtime.sidecar_enable_mqtt_listener,
    sidecar_batch_size: runtime.sidecar_batch_size,
    sidecar_flush_interval_ms: runtime.sidecar_flush_interval_ms,
    sidecar_max_queue: runtime.sidecar_max_queue,
    sidecar_status_poll_interval_ms: runtime.sidecar_status_poll_interval_ms,
  };
};

const buildDraftFromConfig = (config: SetupDaemonConfig): SetupDaemonConfigDraft => ({
  core_port: String(config.core_port),
  mqtt_host: config.mqtt_host,
  mqtt_port: String(config.mqtt_port),
  mqtt_username: config.mqtt_username ?? "",
  mqtt_password: "",
  redis_port: String(config.redis_port),
  backup_root: config.backup_root,
  backup_retention_days: String(config.backup_retention_days),
  bundle_path: config.bundle_path ?? "",
  database_url: config.database_url,
  install_root: config.install_root,
  data_root: config.data_root,
  logs_root: config.logs_root,
  core_binary: config.core_binary,
  sidecar_binary: config.sidecar_binary,
  service_user: config.service_user,
  service_group: config.service_group,
  farmctl_path: config.farmctl_path,
  launchd_label_prefix: config.launchd_label_prefix,
  setup_port: String(config.setup_port),
  enable_analytics_feeds: config.enable_analytics_feeds,
  enable_forecast_ingestion: config.enable_forecast_ingestion,
  analytics_feed_poll_interval_seconds: String(config.analytics_feed_poll_interval_seconds),
  forecast_poll_interval_seconds: String(config.forecast_poll_interval_seconds),
  schedule_poll_interval_seconds: String(config.schedule_poll_interval_seconds),
  offline_threshold_seconds: String(config.offline_threshold_seconds),
  sidecar_mqtt_topic_prefix: config.sidecar_mqtt_topic_prefix,
  sidecar_mqtt_keepalive_secs: String(config.sidecar_mqtt_keepalive_secs),
  sidecar_enable_mqtt_listener: config.sidecar_enable_mqtt_listener,
  sidecar_batch_size: String(config.sidecar_batch_size),
  sidecar_flush_interval_ms: String(config.sidecar_flush_interval_ms),
  sidecar_max_queue: String(config.sidecar_max_queue),
  sidecar_status_poll_interval_ms: String(config.sidecar_status_poll_interval_ms),
});

const initialDraft: SetupDaemonConfigDraft = {
  core_port: "",
  mqtt_host: "",
  mqtt_port: "",
  mqtt_username: "",
  mqtt_password: "",
  redis_port: "",
  backup_root: "",
  backup_retention_days: "",
  bundle_path: "",
  database_url: "",
  install_root: "",
  data_root: "",
  logs_root: "",
  core_binary: "",
  sidecar_binary: "",
  service_user: "",
  service_group: "",
  farmctl_path: "",
  launchd_label_prefix: "",
  setup_port: "",
  enable_analytics_feeds: true,
  enable_forecast_ingestion: true,
  analytics_feed_poll_interval_seconds: "300",
  forecast_poll_interval_seconds: "3600",
  schedule_poll_interval_seconds: "15",
  offline_threshold_seconds: "5",
  sidecar_mqtt_topic_prefix: "iot",
  sidecar_mqtt_keepalive_secs: "30",
  sidecar_enable_mqtt_listener: true,
  sidecar_batch_size: "500",
  sidecar_flush_interval_ms: "750",
  sidecar_max_queue: "5000",
  sidecar_status_poll_interval_ms: "1000",
};

type SetupDaemonConfigModel = {
  config: SetupDaemonConfig | null;
  draft: SetupDaemonConfigDraft;
  error: string | null;
  busy: "loading" | "saving" | null;
  advanced: boolean;
  mqttPasswordClear: boolean;
  preflight: SetupDaemonPreflightCheck[] | null;
  preflightError: string | null;
  preflightBusy: boolean;
  localIp: SetupDaemonLocalIp | null;
  localIpError: string | null;
  localIpBusy: boolean;
  updateDraft: (patch: Partial<SetupDaemonConfigDraft>) => void;
  loadConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  loadPreflight: () => Promise<void>;
  loadLocalIp: () => Promise<void>;
  useRecommendedMqttHost: () => Promise<void>;
  setAdvanced: (value: boolean) => void;
  setMqttPasswordClear: (value: boolean) => void;
};

export const useSetupDaemonConfig = (onMessage: (message: Message) => void): SetupDaemonConfigModel => {
  const [config, setConfig] = useState<SetupDaemonConfig | null>(null);
  const [draft, setDraft] = useState<SetupDaemonConfigDraft>(initialDraft);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<"loading" | "saving" | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [mqttPasswordClear, setMqttPasswordClear] = useState(false);

  const [preflight, setPreflight] = useState<SetupDaemonPreflightCheck[] | null>(null);
  const [preflightError, setPreflightError] = useState<string | null>(null);
  const [preflightBusy, setPreflightBusy] = useState(false);

  const [localIp, setLocalIp] = useState<SetupDaemonLocalIp | null>(null);
  const [localIpError, setLocalIpError] = useState<string | null>(null);
  const [localIpBusy, setLocalIpBusy] = useState(false);

  const updateDraft = useCallback((patch: Partial<SetupDaemonConfigDraft>) => {
    setDraft((prev) => ({ ...prev, ...patch }));
  }, []);

  const loadConfig = useCallback(async () => {
    setBusy("loading");
    setError(null);
    try {
      const [daemonConfig, runtimeConfig] = await Promise.all([
        fetchSetupDaemonConfig(),
        fetchControllerRuntimeConfig(),
      ]);

      const merged = mergeRuntimeConfig({
        daemonConfig,
        runtime: runtimeConfig,
      });

      setConfig(merged);
      setMqttPasswordClear(false);
      setDraft(buildDraftFromConfig(merged));
    } catch (err) {
      const text =
        err instanceof Error ? err.message : "Failed to load setup daemon configuration.";
      setError(text);
      setConfig(null);
    } finally {
      setBusy(null);
    }
  }, []);

  const loadPreflight = useCallback(async () => {
    setPreflightBusy(true);
    setPreflightError(null);
    try {
      const checks = await fetchSetupDaemonPreflightChecks();
      setPreflight(checks);
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to load preflight checks.";
      setPreflightError(text);
      setPreflight(null);
    } finally {
      setPreflightBusy(false);
    }
  }, []);

  const loadLocalIp = useCallback(async () => {
    setLocalIpBusy(true);
    setLocalIpError(null);
    try {
      const info = await fetchSetupDaemonLocalIp();
      setLocalIp(info);
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to detect LAN IP candidates.";
      setLocalIpError(text);
      setLocalIp(null);
    } finally {
      setLocalIpBusy(false);
    }
  }, []);

  const useRecommendedMqttHost = useCallback(async () => {
    setLocalIpBusy(true);
    setLocalIpError(null);
    try {
      const info = await fetchSetupDaemonLocalIp();
      setLocalIp(info);
      if (!info.recommended) {
        setLocalIpError("No LAN IP candidates found.");
        return;
      }
      updateDraft({ mqtt_host: info.recommended });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to detect LAN IP candidates.";
      setLocalIpError(text);
    } finally {
      setLocalIpBusy(false);
    }
  }, [updateDraft]);

  const saveConfig = useCallback(async () => {
    if (!config) {
      onMessage({ type: "error", text: "Load the setup daemon config before saving." });
      return;
    }

    const corePortResult = parsePort("Core API port", draft.core_port);
    if (!corePortResult.ok) {
      onMessage({ type: "error", text: corePortResult.error });
      return;
    }
    const mqttPortResult = parsePort("MQTT port", draft.mqtt_port);
    if (!mqttPortResult.ok) {
      onMessage({ type: "error", text: mqttPortResult.error });
      return;
    }
    const redisPortResult = parsePort("Redis port", draft.redis_port);
    if (!redisPortResult.ok) {
      onMessage({ type: "error", text: redisPortResult.error });
      return;
    }

    const retentionDays = Number.parseInt(draft.backup_retention_days, 10);
    if (!Number.isFinite(retentionDays) || retentionDays < 1) {
      onMessage({ type: "error", text: "Backup retention must be at least 1 day." });
      return;
    }

    const mqttHost = draft.mqtt_host.trim();
    if (!mqttHost) {
      onMessage({ type: "error", text: "MQTT host cannot be empty." });
      return;
    }

    const backupRoot = draft.backup_root.trim();
    if (!backupRoot) {
      onMessage({ type: "error", text: "Backup root path cannot be empty." });
      return;
    }

    const patch: Record<string, unknown> = {};
    if (corePortResult.value !== config.core_port) patch.core_port = corePortResult.value;
    if (mqttHost !== config.mqtt_host) patch.mqtt_host = mqttHost;
    if (mqttPortResult.value !== config.mqtt_port) patch.mqtt_port = mqttPortResult.value;
    if (redisPortResult.value !== config.redis_port) patch.redis_port = redisPortResult.value;
    if (backupRoot !== config.backup_root) patch.backup_root = backupRoot;
    if (retentionDays !== config.backup_retention_days) {
      patch.backup_retention_days = retentionDays;
    }

    const bundlePath = draft.bundle_path.trim();
    if (bundlePath && bundlePath !== (config.bundle_path ?? "")) {
      patch.bundle_path = bundlePath;
    }

    const runtimePatch: Record<string, unknown> = {};

    if (advanced) {
      const databaseUrl = draft.database_url.trim();
      if (!databaseUrl) {
        onMessage({ type: "error", text: "Database URL cannot be empty." });
        return;
      }
      patch.database_url = databaseUrl;

      const mqttUsername = draft.mqtt_username.trim();
      if (mqttUsername !== (config.mqtt_username ?? "")) {
        runtimePatch.mqtt_username = mqttUsername;
      }
      if (mqttPasswordClear) {
        runtimePatch.mqtt_password = "";
      } else {
        const mqttPassword = draft.mqtt_password.trim();
        if (mqttPassword) runtimePatch.mqtt_password = mqttPassword;
      }

      if (draft.enable_analytics_feeds !== config.enable_analytics_feeds) {
        runtimePatch.enable_analytics_feeds = draft.enable_analytics_feeds;
      }
      if (draft.enable_forecast_ingestion !== config.enable_forecast_ingestion) {
        runtimePatch.enable_forecast_ingestion = draft.enable_forecast_ingestion;
      }

      const analyticsPollResult = parseSeconds(
        "Analytics feed poll interval",
        draft.analytics_feed_poll_interval_seconds,
        60,
      );
      if (!analyticsPollResult.ok) {
        onMessage({ type: "error", text: analyticsPollResult.error });
        return;
      }
      const forecastPollResult = parseSeconds(
        "Forecast poll interval",
        draft.forecast_poll_interval_seconds,
        300,
      );
      if (!forecastPollResult.ok) {
        onMessage({ type: "error", text: forecastPollResult.error });
        return;
      }
      const schedulePollResult = parseSeconds(
        "Schedule poll interval",
        draft.schedule_poll_interval_seconds,
        5,
      );
      if (!schedulePollResult.ok) {
        onMessage({ type: "error", text: schedulePollResult.error });
        return;
      }
      const offlineThresholdResult = parseSeconds(
        "Offline threshold",
        draft.offline_threshold_seconds,
        1,
      );
      if (!offlineThresholdResult.ok) {
        onMessage({ type: "error", text: offlineThresholdResult.error });
        return;
      }

      if (analyticsPollResult.value !== config.analytics_feed_poll_interval_seconds) {
        runtimePatch.analytics_feed_poll_interval_seconds = analyticsPollResult.value;
      }
      if (forecastPollResult.value !== config.forecast_poll_interval_seconds) {
        runtimePatch.forecast_poll_interval_seconds = forecastPollResult.value;
      }
      if (schedulePollResult.value !== config.schedule_poll_interval_seconds) {
        runtimePatch.schedule_poll_interval_seconds = schedulePollResult.value;
      }
      if (offlineThresholdResult.value !== config.offline_threshold_seconds) {
        runtimePatch.offline_threshold_seconds = offlineThresholdResult.value;
      }

      const sidecarTopicPrefix = draft.sidecar_mqtt_topic_prefix.trim();
      if (sidecarTopicPrefix !== config.sidecar_mqtt_topic_prefix) {
        runtimePatch.sidecar_mqtt_topic_prefix = sidecarTopicPrefix;
      }

      const sidecarKeepaliveResult = parseSeconds(
        "Telemetry MQTT keepalive",
        draft.sidecar_mqtt_keepalive_secs,
        5,
      );
      if (!sidecarKeepaliveResult.ok) {
        onMessage({ type: "error", text: sidecarKeepaliveResult.error });
        return;
      }

      const sidecarBatchSizeResult = parseCount("Telemetry batch size", draft.sidecar_batch_size, 10);
      if (!sidecarBatchSizeResult.ok) {
        onMessage({ type: "error", text: sidecarBatchSizeResult.error });
        return;
      }

      const sidecarFlushResult = parseMillis(
        "Telemetry flush interval",
        draft.sidecar_flush_interval_ms,
        50,
      );
      if (!sidecarFlushResult.ok) {
        onMessage({ type: "error", text: sidecarFlushResult.error });
        return;
      }

      const sidecarMaxQueueResult = parseCount(
        "Telemetry max queue",
        draft.sidecar_max_queue,
        10,
      );
      if (!sidecarMaxQueueResult.ok) {
        onMessage({ type: "error", text: sidecarMaxQueueResult.error });
        return;
      }

      const sidecarStatusPollResult = parseMillis(
        "Telemetry status poll interval",
        draft.sidecar_status_poll_interval_ms,
        100,
      );
      if (!sidecarStatusPollResult.ok) {
        onMessage({ type: "error", text: sidecarStatusPollResult.error });
        return;
      }

      if (sidecarKeepaliveResult.value !== config.sidecar_mqtt_keepalive_secs) {
        runtimePatch.sidecar_mqtt_keepalive_secs = sidecarKeepaliveResult.value;
      }
      if (draft.sidecar_enable_mqtt_listener !== config.sidecar_enable_mqtt_listener) {
        runtimePatch.sidecar_enable_mqtt_listener = draft.sidecar_enable_mqtt_listener;
      }
      if (sidecarBatchSizeResult.value !== config.sidecar_batch_size) {
        runtimePatch.sidecar_batch_size = sidecarBatchSizeResult.value;
      }
      if (sidecarFlushResult.value !== config.sidecar_flush_interval_ms) {
        runtimePatch.sidecar_flush_interval_ms = sidecarFlushResult.value;
      }
      if (sidecarMaxQueueResult.value !== config.sidecar_max_queue) {
        runtimePatch.sidecar_max_queue = sidecarMaxQueueResult.value;
      }
      if (sidecarStatusPollResult.value !== config.sidecar_status_poll_interval_ms) {
        runtimePatch.sidecar_status_poll_interval_ms = sidecarStatusPollResult.value;
      }

      const installRoot = draft.install_root.trim();
      const dataRoot = draft.data_root.trim();
      const logsRoot = draft.logs_root.trim();
      const coreBinary = draft.core_binary.trim();
      const sidecarBinary = draft.sidecar_binary.trim();
      const serviceUser = draft.service_user.trim();
      const serviceGroup = draft.service_group.trim();
      const farmctlPath = draft.farmctl_path.trim();
      const labelPrefix = draft.launchd_label_prefix.trim();

      if (!installRoot || !dataRoot || !logsRoot || !coreBinary || !sidecarBinary) {
        onMessage({
          type: "error",
          text: "Install/data/logs roots and binary paths cannot be empty.",
        });
        return;
      }
      if (!serviceUser || !serviceGroup) {
        onMessage({ type: "error", text: "Service user/group cannot be empty." });
        return;
      }
      if (!farmctlPath) {
        onMessage({ type: "error", text: "farmctl path cannot be empty." });
        return;
      }
      if (!labelPrefix) {
        onMessage({ type: "error", text: "Launchd label prefix cannot be empty." });
        return;
      }

      const setupPortResult = parsePort("Setup daemon port", draft.setup_port);
      if (!setupPortResult.ok) {
        onMessage({ type: "error", text: setupPortResult.error });
        return;
      }

      patch.install_root = installRoot;
      patch.data_root = dataRoot;
      patch.logs_root = logsRoot;
      patch.core_binary = coreBinary;
      patch.sidecar_binary = sidecarBinary;
      patch.service_user = serviceUser;
      patch.service_group = serviceGroup;
      patch.farmctl_path = farmctlPath;
      patch.launchd_label_prefix = labelPrefix;
      patch.setup_port = setupPortResult.value;
    }

    if (Object.keys(patch).length === 0 && Object.keys(runtimePatch).length === 0) {
      onMessage({ type: "success", text: "Controller configuration is already up to date." });
      return;
    }

    setBusy("saving");
    try {
      let updated = config;
      if (Object.keys(patch).length > 0) {
        updated = await postSetupDaemonConfigPatch(patch);
      }

      const runtimePayload =
        Object.keys(runtimePatch).length > 0
          ? await postControllerRuntimeConfigPatch(runtimePatch)
          : await fetchControllerRuntimeConfig();

      const merged = mergeRuntimeConfig({
        daemonConfig: updated,
        runtime: runtimePayload,
      });

      setConfig(merged);
      setMqttPasswordClear(false);
      setDraft(buildDraftFromConfig(merged));
      onMessage({ type: "success", text: "Saved controller configuration." });
      void loadPreflight();
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to save controller configuration.";
      onMessage({ type: "error", text });
    } finally {
      setBusy(null);
    }
  }, [advanced, config, draft, loadPreflight, mqttPasswordClear, onMessage]);

  useEffect(() => {
    void loadConfig();
    void loadPreflight();
    void loadLocalIp();
  }, [loadConfig, loadLocalIp, loadPreflight]);

  const model = useMemo(
    () => ({
      config,
      draft,
      error,
      busy,
      advanced,
      mqttPasswordClear,
      preflight,
      preflightError,
      preflightBusy,
      localIp,
      localIpError,
      localIpBusy,
      updateDraft,
      loadConfig,
      saveConfig,
      loadPreflight,
      loadLocalIp,
      useRecommendedMqttHost,
      setAdvanced,
      setMqttPasswordClear,
    }),
    [
      advanced,
      busy,
      config,
      draft,
      error,
      localIp,
      localIpBusy,
      localIpError,
      loadConfig,
      loadLocalIp,
      loadPreflight,
      mqttPasswordClear,
      preflight,
      preflightBusy,
      preflightError,
      saveConfig,
      updateDraft,
      useRecommendedMqttHost,
    ],
  );

  return model;
};
