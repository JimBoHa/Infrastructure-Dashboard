"use client";

import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import { Select } from "@/components/ui/select";
import { formatDistanceToNow } from "date-fns";
import { fetchResponse, postJson } from "@/lib/api";
import {
  queryKeys,
  useBackupsQuery,
  useNodesQuery,
  useRecentRestoresQuery,
} from "@/lib/queries";
import RetentionSection from "@/features/backups/components/RetentionSection";
import RestoreActivity from "@/features/backups/components/RestoreActivity";
import RestoreModal from "@/features/backups/components/RestoreModal";
import AppSettingsRestoreModal from "@/features/backups/components/AppSettingsRestoreModal";
import normalizeRestoreHistory, { type RestoreEvent } from "@/features/backups/utils/normalizeRestoreHistory";
import type { DemoBackup } from "@/types/dashboard";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useAuth } from "@/components/AuthProvider";
import InlineBanner from "@/components/InlineBanner";
import PageHeaderCard from "@/components/PageHeaderCard";
import CollapsibleCard from "@/components/CollapsibleCard";

export default function BackupsPageClient() {
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const canViewBackups = Boolean(canEdit || me?.capabilities?.includes("backups.view"));
  const nodesQuery = useNodesQuery();
  const backupsQuery = useBackupsQuery({ enabled: canViewBackups });
  const { data: recentRestores } = useRecentRestoresQuery({ enabled: canViewBackups });
  const [selectedBackup, setSelectedBackup] = useState<DemoBackup | null>(null);
  const [showAppSettingsRestore, setShowAppSettingsRestore] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [restoreHistory, setRestoreHistory] = useState<RestoreEvent[]>([]);
  const [dbExportFormat, setDbExportFormat] = useState<"raw" | "sql" | "csv" | "json">("raw");
  const [dbExportScope, setDbExportScope] = useState<"full" | "app" | "config">("full");

  useEffect(() => {
    try {
      const saved = localStorage.getItem("restore-history");
      if (saved) {
        const parsed = JSON.parse(saved);
        if (Array.isArray(parsed)) {
          setRestoreHistory(parsed as RestoreEvent[]);
        }
      }
    } catch {
      // ignore corrupted local storage
    }
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem("restore-history", JSON.stringify(restoreHistory.slice(0, 25)));
    } catch {
      // ignore write failures (private mode, quota, etc.)
    }
  }, [restoreHistory]);

  if (!canViewBackups) {
    return (
      <div className="p-4">
        <InlineBanner tone="warning">
          You don’t have permission to view backups. Ask an admin for{" "}
          <code className="px-1">backups.view</code> (or <code className="px-1">config.write</code>).
        </InlineBanner>
      </div>
    );
  }

  const isLoading = nodesQuery.isLoading || backupsQuery.isLoading;
  const error = nodesQuery.error || backupsQuery.error;

  if (isLoading) return <LoadingState label="Loading backups..." />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load backups."} />;
  }

  const backups = backupsQuery.data ?? [];
  const nodes = nodesQuery.data ?? [];

  const resolveCapturedAt = (backup: DemoBackup) =>
    backup.captured_at ? new Date(backup.captured_at) : null;

  const triggerDownload = async (backup: DemoBackup) => {
    const capturedAt = resolveCapturedAt(backup);
    if (!capturedAt) {
      setMessage({ type: "error", text: "Backup capture date is unavailable." });
      return;
    }
    const date = capturedAt.toISOString().slice(0, 10);
    try {
      const response = await fetchResponse(`/api/backups/${backup.node_id}/${date}/download`);
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `${backup.node_id}-${date}.json`;
      link.rel = "noopener";
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setMessage({ type: "success", text: `Downloading ${backup.path}...` });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to download backup.";
      setMessage({ type: "error", text });
    }
  };

  const triggerDownloadAppSettings = async () => {
    try {
      const response = await fetchResponse("/api/backups/app-settings/export");
      const blob = await response.blob();
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `farm-dashboard-settings.json`;
      link.rel = "noopener";
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setMessage({ type: "success", text: "Downloading controller settings bundle..." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to download controller settings bundle.";
      setMessage({ type: "error", text });
    }
  };

  const triggerRestoreAppSettings = async (bundle: unknown) => {
    try {
      const result = await postJson<unknown>("/api/backups/app-settings/import", bundle);
      const warnings = (() => {
        if (!result || typeof result !== "object") return [];
        const maybeWarnings = (result as { warnings?: unknown }).warnings;
        return Array.isArray(maybeWarnings) ? maybeWarnings : [];
      })();
      setMessage({
        type: "success",
        text: warnings.length
          ? `Settings restored (with ${warnings.length} warning(s)). Review the warnings in the response payload.`
          : "Settings restored successfully.",
      });
      setShowAppSettingsRestore(false);
      void queryClient.invalidateQueries({ queryKey: queryKeys.backups });
      void queryClient.invalidateQueries({ queryKey: queryKeys.nodes });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to restore controller settings.";
      setMessage({ type: "error", text });
    }
  };

  const triggerDatabaseExport = async () => {
    const scope = dbExportFormat === "csv" || dbExportFormat === "json"
      ? (dbExportScope === "full" ? "app" : dbExportScope)
      : dbExportScope;
    try {
      const response = await fetchResponse(
        `/api/backups/database/export?format=${dbExportFormat}&scope=${scope}`,
      );
      const blob = await response.blob();
      const contentDisposition = response.headers.get("content-disposition") ?? "";
      const match = contentDisposition.match(/filename=\"([^\"]+)\"/i);
      const filename = match?.[1] ?? `farm-dashboard-db.${dbExportFormat}`;
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = filename;
      link.rel = "noopener";
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      setMessage({ type: "success", text: `Downloading database export (${dbExportFormat})...` });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to export database.";
      setMessage({ type: "error", text });
    }
  };

  const grouped = backups.reduce<Record<string, DemoBackup[]>>((acc, backup) => {
    (acc[backup.node_id] ||= []).push(backup);
    return acc;
  }, {});

  const triggerRestore = async (backup: DemoBackup, targetNodeId?: string) => {
    const capturedAt = resolveCapturedAt(backup);
    if (!capturedAt) {
      setMessage({ type: "error", text: "Backup capture date is unavailable." });
      return;
    }
    try {
      await postJson("/api/restore", {
        backup_node_id: backup.node_id,
        date: capturedAt.toISOString().slice(0, 10),
        target_node_id: targetNodeId ?? backup.node_id,
      });
      const targetName = nodes.find((node) => node.id === (targetNodeId ?? backup.node_id))?.name;
      setMessage({ type: "success", text: "Restore queued successfully." });
      setRestoreHistory((prev) => {
        const next = [
          {
            label: `Restore queued for ${targetName ?? backup.node_id}`,
            timestamp: new Date().toISOString(),
            target: targetName,
          },
          ...prev,
        ].slice(0, 25);
        return next;
      });
      void queryClient.invalidateQueries({ queryKey: queryKeys.backups });
      void queryClient.invalidateQueries({ queryKey: queryKeys.recentRestores });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to trigger restore";
      setMessage({ type: "error", text });
    }
  };

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Backups"
        description="Daily configuration snapshots for each node. Restore a backup to rapidly reprovision replacement hardware."
      />

      {message && (
        <InlineBanner tone={message.type === "success" ? "success" : "error"}>
          {message.text}
        </InlineBanner>
      )}

      {!canEdit ? (
        <InlineBanner tone="info" className="border-warning-surface-border bg-warning-surface text-warning-surface-foreground">
          Read-only: you need <code className="px-1">config.write</code> to download, export, or restore backups.
        </InlineBanner>
      ) : null}

      <section className="grid gap-4 lg:grid-cols-2">
        <CollapsibleCard
          title="Controller settings"
          description="Export and restore app-wide configuration (Setup Center credentials, Map configuration, backup retention)."
          defaultOpen
          actions={
            <div className="flex flex-wrap items-center gap-2">
              <NodeButton size="sm" onClick={() => void triggerDownloadAppSettings()} disabled={!canEdit}>
                Download bundle
              </NodeButton>
              <NodeButton
                size="sm"
                variant="secondary"
                onClick={() => (canEdit ? setShowAppSettingsRestore(true) : null)}
                disabled={!canEdit}
              >
                Restore...
              </NodeButton>
            </div>
          }
        >
 <p className="text-xs text-muted-foreground">
            The bundle contains secrets (tokens/passwords). Store it securely.
          </p>
        </CollapsibleCard>

        <CollapsibleCard
          title="Database export"
          description="Export the controller database for offline analysis or migration."
          defaultOpen={false}
          actions={
            <NodeButton size="sm" onClick={() => void triggerDatabaseExport()} disabled={!canEdit}>
              Export database
            </NodeButton>
          }
        >
          <div className="grid gap-3 sm:grid-cols-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Format
              <Select
                className="mt-1"
                value={dbExportFormat}
                onChange={(event) =>
                  setDbExportFormat(event.target.value as "raw" | "sql" | "csv" | "json")
                }
              >
                <option value="raw">Raw (pg_dump custom)</option>
                <option value="sql">SQL (pg_dump)</option>
                <option value="csv">CSV archive (.tar.gz)</option>
                <option value="json">JSONL archive (.tar.gz)</option>
              </Select>
            </label>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Scope
              <Select
                className="mt-1"
                value={dbExportScope}
                onChange={(event) => setDbExportScope(event.target.value as "full" | "app" | "config")}
              >
                <option value="full">Full database</option>
                <option value="app">App tables (includes metrics)</option>
                <option value="config">Config only (fast)</option>
              </Select>
            </label>
          </div>
          {(dbExportFormat === "csv" || dbExportFormat === "json") && dbExportScope === "full" ? (
 <p className="mt-2 text-xs text-amber-600">
              CSV/JSON exports are limited to app tables. Full DB exports are available via Raw/SQL.
            </p>
          ) : null}
        </CollapsibleCard>
      </section>

      <RetentionSection nodes={nodes} onNotify={setMessage} canEdit={canEdit} />

      <RestoreActivity history={normalizeRestoreHistory(recentRestores ?? restoreHistory)} />

      <div className="space-y-4">
        {Object.entries(grouped).map(([nodeId, nodeBackups]) => {
          const nodeName = nodes.find((node) => node.id === nodeId)?.name ?? nodeId;
          return (
            <CollapsibleCard
              key={nodeId}
              title={nodeName}
              description={`${nodeBackups.length} backup(s)`}
              defaultOpen={false}
              actions={
                <NodeButton
                  size="sm"
                  onClick={() => (canEdit ? setSelectedBackup(nodeBackups[0]) : null)}
                  disabled={!canEdit}
                >
                  Restore latest
                </NodeButton>
              }
            >
              <div className="overflow-x-auto md:overflow-x-visible">
                <table className="min-w-full divide-y divide-border text-sm">
                  <thead className="bg-card-inset">
                    <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Captured
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Size
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Path
                      </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Actions
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {nodeBackups.map((backup) => {
                      const capturedAt = resolveCapturedAt(backup);
                      const sizeLabel =
                        backup.size_bytes != null ? `${(backup.size_bytes / 1024).toFixed(1)} KB` : "--";
                      return (
                        <tr key={backup.id}>
 <td className="px-3 py-2 text-muted-foreground">
                            {capturedAt ? formatDistanceToNow(capturedAt, { addSuffix: true }) : "Unknown"}
                          </td>
 <td className="px-3 py-2 text-muted-foreground">
                            {sizeLabel}
                          </td>
 <td className="px-3 py-2 text-muted-foreground">{backup.path}</td>
 <td className="px-3 py-2 text-muted-foreground">
                            <div className="flex gap-2">
                              <NodeButton size="xs" onClick={() => void triggerDownload(backup)} disabled={!canEdit}>
                                Download
                              </NodeButton>
                              <NodeButton
                                size="xs"
                                onClick={() => (canEdit ? setSelectedBackup(backup) : null)}
                                disabled={!canEdit}
                              >
                                Restore...
                              </NodeButton>
                            </div>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            </CollapsibleCard>
          );
        })}
      </div>

      <RestoreModal
        backup={selectedBackup}
        nodes={nodes}
        onClose={() => setSelectedBackup(null)}
        onConfirm={(targetNode) => {
          if (selectedBackup) {
            triggerRestore(selectedBackup, targetNode?.id);
            setSelectedBackup(null);
          }
        }}
      />

      <AppSettingsRestoreModal
        open={showAppSettingsRestore}
        onClose={() => setShowAppSettingsRestore(false)}
        onConfirm={triggerRestoreAppSettings}
      />
    </div>
  );
}
