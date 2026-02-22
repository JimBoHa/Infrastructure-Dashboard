"use client";

import { useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import {
  applyRenogyBt2Preset,
  applyRenogySettings,
  readRenogyCurrentSettings,
  rollbackRenogyDesiredSettings,
  setRenogyMaintenanceMode,
  updateRenogyDesiredSettings,
  validateRenogyDesiredSettings,
} from "@/lib/api";
import {
  queryKeys,
  useRenogyDesiredSettingsQuery,
  useRenogySettingsHistoryQuery,
  useRenogySettingsSchemaQuery,
} from "@/lib/queries";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import type { RenogyBt2Mode, RenogyHistoryEntry } from "@/types/integrations";
import type { DemoSensor } from "@/types/dashboard";

type RegisterField = {
  key: string;
  label: string;
  group?: string;
  unit?: string | null;
  min?: number | null;
  max?: number | null;
  writable?: boolean;
  risk?: string | null;
};

type RegisterGroup = {
  id: string;
  label: string;
  advanced?: boolean;
};

function asNumberOrNull(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim() !== "" && Number.isFinite(Number(value))) return Number(value);
  return null;
}

function safeRecord(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

function formatDiffSummary(entry: RenogyHistoryEntry): string {
  const diff = entry.diff && typeof entry.diff === "object" && !Array.isArray(entry.diff) ? entry.diff : null;
  if (!diff) return "";
  const keys = Object.keys(diff);
  if (!keys.length) return "";
  if (keys.length <= 2) return keys.join(", ");
  return `${keys.slice(0, 2).join(", ")} +${keys.length - 2}`;
}

export default function RenogyBt2SettingsSection({
  nodeId,
  nodeName,
  nodeConfig,
  nodeStatus,
  nodeSensors,
  canEdit,
}: {
  nodeId: string;
  nodeName: string;
  nodeConfig: Record<string, unknown> | null | undefined;
  nodeStatus: string | null | undefined;
  nodeSensors: DemoSensor[];
  canEdit: boolean;
}) {
  const queryClient = useQueryClient();
  const renogyCfg = useMemo(() => safeRecord(nodeConfig).renogy_bt2, [nodeConfig]);
  const renogyCfgObj = useMemo(() => safeRecord(renogyCfg), [renogyCfg]);
  const enabled = Boolean(renogyCfgObj.enabled);
  const mode = String(renogyCfgObj.mode ?? "ble");
  const bt2Address = String(renogyCfgObj.address ?? "").trim();
  const unitId = typeof renogyCfgObj.unit_id === "number" ? renogyCfgObj.unit_id : 1;
  const pollIntervalSeconds =
    typeof renogyCfgObj.poll_interval_seconds === "number" ? renogyCfgObj.poll_interval_seconds : 10;

  const schemaQuery = useRenogySettingsSchemaQuery(nodeId, { enabled: canEdit });
  const desiredQuery = useRenogyDesiredSettingsQuery(nodeId, { enabled: canEdit });
  const historyQuery = useRenogySettingsHistoryQuery(nodeId, { enabled: canEdit });

  const schema = schemaQuery.data?.schema ? safeRecord(schemaQuery.data.schema) : null;
  const groups = (schema?.groups as unknown as RegisterGroup[]) ?? [];
  const fields = useMemo(() => (schema?.fields as unknown as RegisterField[]) ?? [], [schema?.fields]);

  const [current, setCurrent] = useState<Record<string, unknown> | null>(null);
  const [providerStatus, setProviderStatus] = useState<string | null>(null);
  const [desiredDraft, setDesiredDraft] = useState<Record<string, number | null>>({});
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [validationErrors, setValidationErrors] = useState<string[]>([]);
  const [connDraft, setConnDraft] = useState<{
    address: string;
    unit_id: number;
    poll_interval_seconds: number;
    adapter: string;
    device_name: string;
    request_timeout_seconds: number;
    connect_timeout_seconds: number;
    service_uuid: string;
    write_uuid: string;
    notify_uuid: string;
    mode: RenogyBt2Mode;
  }>({
    address: bt2Address,
    unit_id: unitId,
    poll_interval_seconds: pollIntervalSeconds,
    adapter: typeof renogyCfgObj.adapter === "string" ? renogyCfgObj.adapter : "",
    device_name: typeof renogyCfgObj.device_name === "string" ? renogyCfgObj.device_name : "",
    request_timeout_seconds:
      typeof renogyCfgObj.request_timeout_seconds === "number" ? renogyCfgObj.request_timeout_seconds : 4,
    connect_timeout_seconds:
      typeof renogyCfgObj.connect_timeout_seconds === "number" ? renogyCfgObj.connect_timeout_seconds : 10,
    service_uuid: typeof renogyCfgObj.service_uuid === "string" ? renogyCfgObj.service_uuid : "",
    write_uuid: typeof renogyCfgObj.write_uuid === "string" ? renogyCfgObj.write_uuid : "",
    notify_uuid: typeof renogyCfgObj.notify_uuid === "string" ? renogyCfgObj.notify_uuid : "",
    mode: mode === "external" ? "external" : "ble",
  });
  const [connAdvancedOpen, setConnAdvancedOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<{
    title: string;
    description: string;
    action: () => Promise<void>;
  } | null>(null);

  const desiredFromServer = useMemo(() => safeRecord(desiredQuery.data?.desired), [desiredQuery.data?.desired]);
  const lastApplyResult = useMemo(() => safeRecord(desiredQuery.data?.last_apply_result), [desiredQuery.data?.last_apply_result]);
  const lastApplyFieldResults = useMemo(() => {
    const raw = (lastApplyResult as Record<string, unknown>).field_results;
    return Array.isArray(raw) ? (raw as Array<Record<string, unknown>>) : [];
  }, [lastApplyResult]);

  useEffect(() => {
    if (!desiredQuery.data) return;
    const nextDraft: Record<string, number | null> = {};
    for (const field of fields) {
      nextDraft[field.key] = asNumberOrNull(desiredFromServer[field.key]);
    }
    setDesiredDraft(nextDraft);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- reset draft only when server timestamp or field count changes, not on every fields/desiredFromServer reference change
  }, [desiredQuery.data?.desired_updated_at, fields.length]);

  useEffect(() => {
    setConnDraft((prev) => ({
      ...prev,
      address: bt2Address,
      unit_id: unitId,
      poll_interval_seconds: pollIntervalSeconds,
      adapter: typeof renogyCfgObj.adapter === "string" ? renogyCfgObj.adapter : "",
      device_name: typeof renogyCfgObj.device_name === "string" ? renogyCfgObj.device_name : "",
      request_timeout_seconds:
        typeof renogyCfgObj.request_timeout_seconds === "number" ? renogyCfgObj.request_timeout_seconds : 4,
      connect_timeout_seconds:
        typeof renogyCfgObj.connect_timeout_seconds === "number" ? renogyCfgObj.connect_timeout_seconds : 10,
      service_uuid: typeof renogyCfgObj.service_uuid === "string" ? renogyCfgObj.service_uuid : "",
      write_uuid: typeof renogyCfgObj.write_uuid === "string" ? renogyCfgObj.write_uuid : "",
      notify_uuid: typeof renogyCfgObj.notify_uuid === "string" ? renogyCfgObj.notify_uuid : "",
      mode: mode === "external" ? "external" : "ble",
    }));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodeId, bt2Address, unitId, pollIntervalSeconds, mode]);

  const desiredSanitized = useMemo(() => {
    const out: Record<string, unknown> = {};
    for (const field of fields) {
      if (!field.writable) continue;
      const value = desiredDraft[field.key];
      if (typeof value === "number" && Number.isFinite(value)) {
        out[field.key] = value;
      }
    }
    return out;
  }, [desiredDraft, fields]);

  const changedKeys = useMemo(() => {
    const changed: string[] = [];
    for (const field of fields) {
      if (!field.writable) continue;
      const desiredValue = desiredSanitized[field.key];
      const serverValue = desiredFromServer[field.key];
      if (desiredValue == null && serverValue == null) continue;
      if (typeof desiredValue === "number" && typeof serverValue === "number" && desiredValue === serverValue) continue;
      if (desiredValue !== serverValue) changed.push(field.key);
    }
    return changed;
  }, [desiredFromServer, desiredSanitized, fields]);

  const hasHighRiskChange = useMemo(() => {
    const highRiskKeys = new Set(
      fields.filter((f) => f.risk === "high").map((f) => f.key),
    );
    return changedKeys.some((key) => highRiskKeys.has(key));
  }, [changedKeys, fields]);

  const handleRead = async () => {
    if (!canEdit) return;
    if (!enabled || bt2Address.length === 0 || connDraft.mode !== "ble") {
      setError("Configure Renogy BT-2 (BLE mode) before reading controller settings.");
      return;
    }
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const response = await readRenogyCurrentSettings(nodeId);
      setProviderStatus(response.provider_status);
      setCurrent(safeRecord(response.current));
      setMessage("Read settings from controller.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to read from controller.");
    } finally {
      setBusy(false);
    }
  };

  const handleValidate = async () => {
    if (!canEdit) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    setValidationErrors([]);
    try {
      const response = await validateRenogyDesiredSettings(nodeId, desiredSanitized);
      setValidationErrors(response.errors ?? []);
      setMessage(response.ok ? "Validation passed." : "Validation failed.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to validate settings.");
    } finally {
      setBusy(false);
    }
  };

  const handleSaveDesired = async () => {
    if (!canEdit) return;
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      await updateRenogyDesiredSettings(nodeId, desiredSanitized);
      await queryClient.invalidateQueries({ queryKey: queryKeys.renogyDesiredSettings(nodeId) });
      await queryClient.invalidateQueries({ queryKey: queryKeys.renogySettingsHistory(nodeId) });
      setMessage("Saved desired settings.");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save desired settings.");
    } finally {
      setBusy(false);
    }
  };

  const doApply = async () => {
    setBusy(true);
    setError(null);
    setMessage(null);
    try {
      const result = await applyRenogySettings(nodeId);
      await queryClient.invalidateQueries({ queryKey: queryKeys.renogyDesiredSettings(nodeId) });
      await queryClient.invalidateQueries({ queryKey: queryKeys.renogySettingsHistory(nodeId) });
      setMessage(result.status === "ok" ? "Applied settings." : `Apply status: ${result.status}`);
      await handleRead();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to apply settings.");
    } finally {
      setBusy(false);
    }
  };

  const handleApply = () => {
    if (!canEdit) return;
    if (!enabled || bt2Address.length === 0 || connDraft.mode !== "ble") {
      setError("Configure Renogy BT-2 (BLE mode) before applying controller settings.");
      return;
    }
    if (!changedKeys.length) {
      setMessage("No changes to apply.");
      return;
    }
    if (hasHighRiskChange) {
      setConfirmAction({
        title: "Apply high-risk settings",
        description: "This change includes high-risk settings. Apply anyway? The controller will be read back to verify writes.",
        action: doApply,
      });
      return;
    }
    void doApply();
  };

  const handleRollback = (entryId: number) => {
    if (!canEdit) return;
    setConfirmAction({
      title: "Rollback settings",
      description: "Set desired settings from this history entry? You can then Apply to send it to the controller.",
      action: async () => {
        setBusy(true);
        setError(null);
        setMessage(null);
        try {
          await rollbackRenogyDesiredSettings(nodeId, entryId);
          await queryClient.invalidateQueries({ queryKey: queryKeys.renogyDesiredSettings(nodeId) });
          await queryClient.invalidateQueries({ queryKey: queryKeys.renogySettingsHistory(nodeId) });
          setMessage("Rollback loaded into desired settings.");
        } catch (err) {
          setError(err instanceof Error ? err.message : "Failed to rollback desired settings.");
        } finally {
          setBusy(false);
        }
      },
    });
  };

  const handleRollbackAndApply = (entryId: number) => {
    if (!canEdit) return;
    if (!enabled || bt2Address.length === 0 || connDraft.mode !== "ble") {
      setError("Configure Renogy BT-2 (BLE mode) before applying controller settings.");
      return;
    }
    setConfirmAction({
      title: "Rollback and apply",
      description: "Rollback to this snapshot and apply to the controller now? A read-back verification will run after the write.",
      action: async () => {
        setBusy(true);
        setError(null);
        setMessage(null);
        try {
          await rollbackRenogyDesiredSettings(nodeId, entryId);
          await queryClient.invalidateQueries({ queryKey: queryKeys.renogyDesiredSettings(nodeId) });
          await queryClient.invalidateQueries({ queryKey: queryKeys.renogySettingsHistory(nodeId) });
          const result = await applyRenogySettings(nodeId);
          await queryClient.invalidateQueries({ queryKey: queryKeys.renogyDesiredSettings(nodeId) });
          await queryClient.invalidateQueries({ queryKey: queryKeys.renogySettingsHistory(nodeId) });
          setMessage(result.status === "ok" ? "Rolled back and applied." : `Rollback apply status: ${result.status}`);
          await handleRead();
        } catch (err) {
          setError(err instanceof Error ? err.message : "Failed to rollback/apply settings.");
        } finally {
          setBusy(false);
        }
      },
    });
  };

  const fieldsByGroup = useMemo(() => {
    const groupsMap = new Map<string, RegisterField[]>();
    for (const field of fields) {
      const group = field.group ?? "ungrouped";
      const list = groupsMap.get(group) ?? [];
      list.push(field);
      groupsMap.set(group, list);
    }
    return groupsMap;
  }, [fields]);

  const renogyMetric = (sensor: DemoSensor): string | null => {
    const cfg = safeRecord(sensor.config);
    const metric = cfg.metric;
    return typeof metric === "string" && metric.trim() ? metric.trim() : null;
  };

  const renogySensors = useMemo(() => {
    return nodeSensors.filter((s) => s.type === "renogy_bt2");
  }, [nodeSensors]);

  const metricValue = (metric: string): { value: number | null; unit: string | null } => {
    const match = renogySensors.find((s) => renogyMetric(s) === metric);
    if (!match) return { value: null, unit: null };
    const value = typeof match.latest_value === "number" && Number.isFinite(match.latest_value) ? match.latest_value : null;
    return { value, unit: match.unit ?? null };
  };

  if (!canEdit) {
    return (
      <InlineBanner tone="warning" className="p-4 text-sm">
        You need <span className="font-semibold">config.write</span> to read/apply Renogy controller settings.
      </InlineBanner>
    );
  }

  return (
    <div className="space-y-4">
      <Card className="gap-0 p-4">
        <div className="flex flex-col gap-2 md:flex-row md:items-start md:justify-between">
          <div className="space-y-1">
 <p className="text-sm font-semibold text-foreground">
              Renogy RNG-CTRL-RVR20-US (BT-2)
            </p>
 <p className="text-xs text-muted-foreground">
              Configure the BT-2 connection, then read/validate/apply Modbus settings safely via the controller. The browser never talks to BLE directly.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <NodePill size="md">Node: {nodeName}</NodePill>
            <NodePill size="md">Status: {String(nodeStatus ?? "—")}</NodePill>
            {bt2Address ? <NodePill size="md">BT-2: {bt2Address}</NodePill> : null}
          </div>
        </div>

        <div className="mt-4 grid gap-3 md:grid-cols-2">
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              BT-2 address (MAC)
            </label>
            <Input
              value={connDraft.address}
              onChange={(event) => setConnDraft({ ...connDraft, address: event.target.value })}
              placeholder="AA:BB:CC:DD:EE:FF"
              className="mt-1 rounded-md px-2 py-1"
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Unit id
              </label>
              <NumericDraftInput
                value={connDraft.unit_id}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    setConnDraft((current) => ({ ...current, unit_id: next }));
                  }
                }}
                emptyBehavior="keep"
                min={1}
                max={255}
                integer
                inputMode="numeric"
                enforceRange
                clampOnBlur
 className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1 text-sm text-foreground shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
              />
            </div>
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Poll (s)
              </label>
              <NumericDraftInput
                value={connDraft.poll_interval_seconds}
                onValueChange={(next) => {
                  if (typeof next === "number") {
                    setConnDraft((current) => ({ ...current, poll_interval_seconds: next }));
                  }
                }}
                emptyBehavior="keep"
                min={5}
                max={3600}
                integer
                inputMode="numeric"
                enforceRange
                clampOnBlur
 className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1 text-sm text-foreground shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
              />
            </div>
          </div>
        </div>

        <div className="mt-3 grid gap-3 md:grid-cols-2">
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Mode
            </label>
            <Select
              value={connDraft.mode}
              onChange={(event) =>
                setConnDraft({ ...connDraft, mode: event.target.value === "external" ? "external" : "ble" })
              }
              className="mt-1 rounded-md px-2 py-1"
            >
              <option value="ble">BLE (recommended)</option>
              <option value="external">External ingest</option>
            </Select>
          </div>
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Bluetooth adapter (optional)
            </label>
            <Input
              value={connDraft.adapter}
              onChange={(event) => setConnDraft({ ...connDraft, adapter: event.target.value })}
              placeholder="hci0"
              className="mt-1 rounded-md px-2 py-1"
            />
          </div>
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-2">
          <NodeButton
            size="sm"
            variant="primary"
            disabled={busy || connDraft.address.trim().length === 0}
            onClick={async () => {
              setBusy(true);
              setError(null);
              setMessage(null);
              try {
                const response = await applyRenogyBt2Preset(nodeId, {
                  bt2_address: connDraft.address.trim(),
                  poll_interval_seconds: connDraft.poll_interval_seconds,
                  mode: connDraft.mode,
                  adapter: connDraft.adapter.trim() || undefined,
                  unit_id: connDraft.unit_id,
                  device_name: connDraft.device_name.trim() || undefined,
                  request_timeout_seconds: connDraft.request_timeout_seconds,
                  connect_timeout_seconds: connDraft.connect_timeout_seconds,
                  service_uuid: connDraft.service_uuid.trim() || undefined,
                  write_uuid: connDraft.write_uuid.trim() || undefined,
                  notify_uuid: connDraft.notify_uuid.trim() || undefined,
                });
                await queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
                setMessage(
                  response.status === "stored"
                    ? response.warning ?? "Saved connection. The node will sync when online."
                    : "Saved connection to node.",
                );
              } catch (err) {
                setError(err instanceof Error ? err.message : "Failed to save Renogy BT-2 connection.");
              } finally {
                setBusy(false);
              }
            }}
          >
            Save connection
          </NodeButton>
          <NodeButton
            size="sm"
            disabled={busy}
            onClick={() => setConnAdvancedOpen((v) => !v)}
          >
            {connAdvancedOpen ? "Hide advanced" : "Show advanced"}
          </NodeButton>
          {desiredQuery.data?.maintenance_mode ? (
            <NodePill tone="muted" size="md">
              Maintenance mode
            </NodePill>
          ) : null}
          {desiredQuery.data?.apply_requested ? (
            <NodePill tone="muted" size="md">
              Apply queued
            </NodePill>
          ) : null}
        </div>

        {connAdvancedOpen ? (
          <Card className="mt-3 rounded-lg gap-0 bg-card-inset p-3 text-sm">
            <div className="grid gap-3 md:grid-cols-2">
              <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  Device name (optional)
                </label>
                <Input
                  value={connDraft.device_name}
                  onChange={(event) => setConnDraft({ ...connDraft, device_name: event.target.value })}
                  placeholder="Renogy BT-2"
                  className="mt-1 rounded-md px-2 py-1"
                />
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Req timeout (s)
                  </label>
                  <NumericDraftInput
                    value={connDraft.request_timeout_seconds}
                    onValueChange={(next) => {
                      if (typeof next === "number") {
                        setConnDraft((current) => ({ ...current, request_timeout_seconds: next }));
                      }
                    }}
                    emptyBehavior="keep"
                    min={1}
                    max={60}
                    integer
                    inputMode="numeric"
                    enforceRange
                    clampOnBlur
 className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1 text-sm text-foreground shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                  />
                </div>
                <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Conn timeout (s)
                  </label>
                  <NumericDraftInput
                    value={connDraft.connect_timeout_seconds}
                    onValueChange={(next) => {
                      if (typeof next === "number") {
                        setConnDraft((current) => ({ ...current, connect_timeout_seconds: next }));
                      }
                    }}
                    emptyBehavior="keep"
                    min={1}
                    max={120}
                    integer
                    inputMode="numeric"
                    enforceRange
                    clampOnBlur
 className="mt-1 w-full rounded-md border border-border bg-white px-2 py-1 text-sm text-foreground shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30"
                  />
                </div>
              </div>
              <div className="md:col-span-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                  BLE UUID overrides (optional)
                </p>
                <div className="mt-2 grid gap-3 md:grid-cols-3">
                  <Input
                    value={connDraft.service_uuid}
                    onChange={(event) => setConnDraft({ ...connDraft, service_uuid: event.target.value })}
                    placeholder="Service UUID"
                    className="rounded-md px-2 py-1"
                  />
                  <Input
                    value={connDraft.write_uuid}
                    onChange={(event) => setConnDraft({ ...connDraft, write_uuid: event.target.value })}
                    placeholder="Write UUID"
                    className="rounded-md px-2 py-1"
                  />
                  <Input
                    value={connDraft.notify_uuid}
                    onChange={(event) => setConnDraft({ ...connDraft, notify_uuid: event.target.value })}
                    placeholder="Notify UUID"
                    className="rounded-md px-2 py-1"
                  />
                </div>
              </div>
            </div>
          </Card>
        ) : null}
      </Card>

      <Card className="rounded-lg gap-0 bg-card-inset p-4">
        <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
          <div className="space-y-1">
 <p className="text-sm font-semibold text-foreground">
              Settings apply
            </p>
 <p className="text-xs text-muted-foreground">
              Read the current controller settings, validate, and apply with automatic read-back verification.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <NodePill size="md">Node: {nodeName}</NodePill>
            <NodePill size="md">Mode: {connDraft.mode}</NodePill>
          </div>
        </div>

        <div className="mt-4 flex flex-wrap items-center gap-2">
          <NodeButton size="sm" onClick={() => void handleRead()} disabled={busy}>
            Read from controller
          </NodeButton>
          <NodeButton size="sm" onClick={() => void handleValidate()} disabled={busy}>
            Validate
          </NodeButton>
          <NodeButton size="sm" onClick={() => void handleSaveDesired()} disabled={busy || !changedKeys.length}>
            Save desired
          </NodeButton>
          <NodeButton size="sm" variant="primary" onClick={() => void handleApply()} disabled={busy || !changedKeys.length}>
            Apply (with confirmation)
          </NodeButton>
          {desiredQuery.data?.maintenance_mode ? (
            <NodeButton
              size="sm"
              onClick={() => {
                void (async () => {
                  setBusy(true);
                  setError(null);
                  setMessage(null);
                  try {
                    await setRenogyMaintenanceMode(nodeId, false);
                    await queryClient.invalidateQueries({ queryKey: queryKeys.renogyDesiredSettings(nodeId) });
                    await queryClient.invalidateQueries({ queryKey: queryKeys.renogySettingsHistory(nodeId) });
                    setMessage("Maintenance mode disabled.");
                  } catch (err) {
                    setError(err instanceof Error ? err.message : "Failed to disable maintenance mode.");
                  } finally {
                    setBusy(false);
                  }
                })();
              }}
              disabled={busy}
            >
              Disable maintenance mode
            </NodeButton>
          ) : (
            <NodeButton
              size="sm"
              onClick={() => {
                void (async () => {
                  setBusy(true);
                  setError(null);
                  setMessage(null);
                  try {
                    await setRenogyMaintenanceMode(nodeId, true);
                    await queryClient.invalidateQueries({ queryKey: queryKeys.renogyDesiredSettings(nodeId) });
                    await queryClient.invalidateQueries({ queryKey: queryKeys.renogySettingsHistory(nodeId) });
                    setMessage("Maintenance mode enabled (writes disabled).");
                  } catch (err) {
                    setError(err instanceof Error ? err.message : "Failed to enable maintenance mode.");
                  } finally {
                    setBusy(false);
                  }
                })();
              }}
              disabled={busy}
            >
              Maintenance mode
            </NodeButton>
          )}
          {desiredQuery.data?.pending ? (
            <NodePill tone="muted" size="md">
              Pending
            </NodePill>
          ) : null}
          {desiredQuery.data?.apply_requested ? (
            <NodePill tone="muted" size="md">
              Apply queued
            </NodePill>
          ) : null}
          {providerStatus ? (
            <NodePill tone={providerStatus === "ok" ? "success" : "muted"} size="md">
              {providerStatus}
            </NodePill>
          ) : null}
        </div>

        {message ? (
 <p className="mt-3 text-sm text-foreground">{message}</p>
        ) : null}
        {error ? (
 <p className="mt-3 text-sm text-rose-700">{error}</p>
        ) : null}
        {validationErrors.length ? (
          <InlineBanner tone="error" className="mt-3 px-3 py-2 text-sm">
            <p className="font-semibold">Validation errors</p>
            <ul className="mt-2 list-disc space-y-1 pl-5">
              {validationErrors.map((line) => (
                <li key={line}>{line}</li>
              ))}
            </ul>
          </InlineBanner>
        ) : null}

        {desiredQuery.data?.last_apply_status ? (
          <Card className="mt-4 gap-0 p-3 text-sm">
            <div className="flex flex-wrap items-center justify-between gap-2">
 <p className="font-semibold text-foreground">Last apply</p>
              <div className="flex items-center gap-2">
                <NodePill tone={desiredQuery.data.last_apply_status === "ok" ? "success" : "muted"} size="md">
                  {desiredQuery.data.last_apply_status}
                </NodePill>
 <span className="text-xs text-muted-foreground">
                  {desiredQuery.data.last_applied_at ? new Date(desiredQuery.data.last_applied_at).toLocaleString() : ""}
                </span>
              </div>
            </div>
            {lastApplyFieldResults.length ? (
              <div className="mt-3 overflow-x-auto">
                <table className="min-w-full text-left text-xs">
 <thead className="text-[11px] uppercase tracking-wide text-muted-foreground">
                    <tr>
                      <th className="py-1 pr-3">Setting</th>
                      <th className="py-1 pr-3">Expected</th>
                      <th className="py-1 pr-3">Read-back</th>
                      <th className="py-1 pr-3">Result</th>
                    </tr>
                  </thead>
 <tbody className="text-sm text-foreground">
                    {lastApplyFieldResults.map((row) => (
                      <tr
                        key={`${String(row.address ?? "")}:${String(row.key ?? "")}`}
 className="border-t border-border"
                      >
                        <td className="py-2 pr-3">
 <div className="font-medium text-foreground">
                            {typeof row.label === "string" && row.label.trim()
                              ? row.label
                              : typeof row.key === "string" && row.key.trim()
                                ? row.key
                                : "—"}
                          </div>
 <div className="text-xs text-muted-foreground">
                            {typeof row.key === "string" ? row.key : ""}
                          </div>
                        </td>
                        <td className="py-2 pr-3">{Array.isArray(row.expected) ? row.expected.join(",") : "—"}</td>
                        <td className="py-2 pr-3">{Array.isArray(row.read_back) ? row.read_back.join(",") : "—"}</td>
                        <td className="py-2 pr-3">
                          {row.ok ? (
 <span className="font-semibold text-emerald-700">OK</span>
                          ) : (
 <span className="font-semibold text-rose-700">Check</span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            ) : (
 <p className="mt-2 text-xs text-muted-foreground">
                No per-field results recorded yet. Use Apply to generate a verified read-back.
              </p>
            )}
          </Card>
        ) : null}
      </Card>

      {schemaQuery.isLoading ? (
        <Card className="gap-0 p-4 text-sm text-muted-foreground">
          Loading settings schema…
        </Card>
      ) : schemaQuery.error ? (
        <InlineBanner tone="error" className="p-4 text-sm">
          Failed to load schema.
        </InlineBanner>
      ) : (
        <div className="space-y-3">
          {groups
            .filter((g) => !g.advanced)
            .map((group) => (
              <SettingsGroupCard
                key={group.id}
                group={group}
                fields={fieldsByGroup.get(group.id) ?? []}
                current={current}
                desiredDraft={desiredDraft}
                setDesiredDraft={setDesiredDraft}
                busy={busy}
              />
            ))}

          {groups.some((g) => g.advanced) ? (
            <CollapsibleCard title="Advanced" defaultOpen={false}>
              <div className="space-y-3">
                {groups
                  .filter((g) => g.advanced)
                  .map((group) => (
                    <SettingsGroupCard
                      key={group.id}
                      group={group}
                      fields={fieldsByGroup.get(group.id) ?? []}
                      current={current}
                      desiredDraft={desiredDraft}
                      setDesiredDraft={setDesiredDraft}
                      busy={busy}
                    />
                  ))}
              </div>
            </CollapsibleCard>
          ) : null}
        </div>
      )}

      <Card className="gap-0 p-4">
        <div className="flex items-center justify-between">
 <p className="text-sm font-semibold text-foreground">Live telemetry</p>
 <p className="text-xs text-muted-foreground">From node sensors</p>
        </div>
        <div className="mt-3 grid gap-3 md:grid-cols-3">
          {(() => {
            const tiles: Array<{ label: string; metric: string }> = [
              { label: "PV power (W)", metric: "pv_power_w" },
              { label: "PV voltage (V)", metric: "pv_voltage_v" },
              { label: "PV current (A)", metric: "pv_current_a" },
              { label: "Battery SOC (%)", metric: "battery_soc_percent" },
              { label: "Battery voltage (V)", metric: "battery_voltage_v" },
              { label: "Battery current (A)", metric: "battery_current_a" },
              { label: "Battery temp (°C)", metric: "battery_temp_c" },
              { label: "Controller temp (°C)", metric: "controller_temp_c" },
              { label: "Load power (W)", metric: "load_power_w" },
              { label: "Load voltage (V)", metric: "load_voltage_v" },
              { label: "Load current (A)", metric: "load_current_a" },
            ];
            return tiles.map((tile) => {
              const mv = metricValue(tile.metric);
              const text = mv.value == null ? "—" : `${mv.value.toFixed(2)}${mv.unit ? ` ${mv.unit}` : ""}`;
              return (
                <Card
                  key={tile.metric}
                  className="gap-0 rounded-md bg-card-inset px-3 py-2 text-sm"
                >
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    {tile.label}
                  </p>
 <p className="mt-1 text-sm font-medium text-foreground">{text}</p>
                </Card>
              );
            });
          })()}
        </div>
        {renogySensors.length === 0 ? (
 <p className="mt-3 text-sm text-muted-foreground">
            No Renogy sensors detected yet. Save the connection and wait for telemetry ingestion.
          </p>
        ) : null}
      </Card>

      <Card className="gap-0 p-4">
        <div className="flex items-center justify-between">
 <p className="text-sm font-semibold text-foreground">History</p>
 <p className="text-xs text-muted-foreground">Last 50</p>
        </div>
        {historyQuery.isLoading ? (
 <p className="mt-2 text-sm text-muted-foreground">Loading history…</p>
        ) : historyQuery.data?.length ? (
          <div className="mt-3 divide-y divide-border">
            {historyQuery.data.map((entry) => (
              <div key={entry.id} className="flex flex-col gap-2 py-3 md:flex-row md:items-center md:justify-between">
                <div className="min-w-0">
 <p className="text-sm font-medium text-foreground">
                    {entry.event_type} · {new Date(entry.created_at).toLocaleString()}
                  </p>
                  {formatDiffSummary(entry) ? (
 <p className="truncate text-xs text-muted-foreground">
                      Changed: {formatDiffSummary(entry)}
                    </p>
                  ) : null}
                </div>
                <div className="flex items-center gap-2">
                  <NodeButton size="sm" onClick={() => void handleRollback(entry.id)} disabled={busy}>
                    Rollback
                  </NodeButton>
                  <NodeButton size="sm" variant="primary" onClick={() => void handleRollbackAndApply(entry.id)} disabled={busy}>
                    Rollback + apply
                  </NodeButton>
                </div>
              </div>
            ))}
          </div>
        ) : (
 <p className="mt-2 text-sm text-muted-foreground">No history yet.</p>
        )}
      </Card>

      <AlertDialog open={!!confirmAction} onOpenChange={(open) => { if (!open) setConfirmAction(null); }}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{confirmAction?.title}</AlertDialogTitle>
            <AlertDialogDescription>{confirmAction?.description}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (confirmAction) void confirmAction.action();
                setConfirmAction(null);
              }}
            >
              Confirm
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

function SettingsGroupCard({
  group,
  fields,
  current,
  desiredDraft,
  setDesiredDraft,
  busy,
}: {
  group: RegisterGroup;
  fields: RegisterField[];
  current: Record<string, unknown> | null;
  desiredDraft: Record<string, number | null>;
  setDesiredDraft: (next: Record<string, number | null>) => void;
  busy: boolean;
}) {
  if (!fields.length) return null;
  const currentRecord = current ?? {};

  return (
    <Card className="gap-0 p-4">
 <p className="text-sm font-semibold text-foreground">{group.label}</p>
      <div className="mt-3 space-y-2">
        {fields.map((field) => (
          <Card
            key={field.key}
            className="grid gap-2 rounded-md bg-card-inset p-3 md:grid-cols-3 md:items-center"
          >
            <div className="min-w-0">
 <p className="truncate text-sm font-medium text-foreground">{field.label}</p>
 <p className="text-xs text-muted-foreground">
                {field.key}
                {field.unit ? ` · ${field.unit}` : ""}
                {field.risk ? ` · ${field.risk} risk` : ""}
              </p>
            </div>
            <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Current
              </p>
 <p className="mt-1 text-sm text-foreground">
                {typeof currentRecord[field.key] === "number" ? String(currentRecord[field.key]) : "—"}
              </p>
            </div>
            <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Desired
              </p>
              <div className="mt-1 flex items-center gap-2">
                <NumericDraftInput
                  value={desiredDraft[field.key] ?? null}
                  onValueChange={(next) =>
                    setDesiredDraft({
                      ...desiredDraft,
                      [field.key]: typeof next === "number" ? next : null,
                    })
                  }
                  integer
                  inputMode="numeric"
                  min={typeof field.min === "number" ? field.min : undefined}
                  max={typeof field.max === "number" ? field.max : undefined}
                  enforceRange
                  clampOnBlur
                  disabled={busy || !field.writable}
 className="w-full rounded-md border border-border bg-white px-2 py-1 text-sm text-foreground shadow-xs focus:border-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500/30 disabled:opacity-60"
                />
              </div>
              {typeof field.min === "number" || typeof field.max === "number" ? (
 <p className="mt-1 text-xs text-muted-foreground">
                  Range {field.min ?? "—"}–{field.max ?? "—"}
                </p>
              ) : null}
            </div>
          </Card>
        ))}
      </div>
    </Card>
  );
}
