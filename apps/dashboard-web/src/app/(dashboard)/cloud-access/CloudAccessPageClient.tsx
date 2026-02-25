"use client";

import { useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import InlineBanner from "@/components/InlineBanner";
import PageHeaderCard from "@/components/PageHeaderCard";
import CollapsibleCard from "@/components/CollapsibleCard";
import NodeButton from "@/features/nodes/components/NodeButton";
import { Input } from "@/components/ui/input";
import {
  deleteCloudSite,
  fetchCloudSiteSnapshot,
  registerCloudSite,
  rotateCloudAccessKey,
  updateCloudAccessConfig,
} from "@/lib/api";
import { queryKeys, useCloudAccessQuery, useCloudSitesQuery } from "@/lib/queries";

export default function CloudAccessPageClient() {
  const queryClient = useQueryClient();
  const cloudAccessQuery = useCloudAccessQuery();
  const role = cloudAccessQuery.data?.role ?? "local";
  const cloudSitesQuery = useCloudSitesQuery({ enabled: role === "cloud" });

  const [message, setMessage] = useState<{ tone: "info" | "danger"; text: string } | null>(null);

  if (cloudAccessQuery.isLoading) return <LoadingState label="Loading cloud access…" />;
  if (cloudAccessQuery.error) {
    return (
      <ErrorState
        message={
          cloudAccessQuery.error instanceof Error
            ? cloudAccessQuery.error.message
            : "Failed to load cloud access settings."
        }
      />
    );
  }

  const config = cloudAccessQuery.data;
  if (!config) return <ErrorState message="Cloud access settings are unavailable." />;

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Cloud Access"
        description="Manage secure key-based replication between local controllers and this cloud instance."
      />

      {message ? <InlineBanner tone={message.tone}>{message.text}</InlineBanner> : null}

      {config.role === "local" ? (
        <LocalCloudAccessSection
          key={`${config.cloud_server_base_url ?? ""}|${config.sync_interval_seconds}|${config.sync_enabled}|${config.local_site_key ?? ""}`}
          config={config}
          onMessage={(tone, text) => setMessage({ tone, text })}
          onRefresh={async () => {
            await queryClient.invalidateQueries({ queryKey: queryKeys.cloudAccess });
          }}
        />
      ) : (
        <CloudRegistrySection
          config={config}
          sites={cloudSitesQuery.data ?? []}
          sitesLoading={cloudSitesQuery.isLoading}
          sitesError={cloudSitesQuery.error}
          onMessage={(tone, text) => setMessage({ tone, text })}
          onRefresh={async () => {
            await Promise.all([
              queryClient.invalidateQueries({ queryKey: queryKeys.cloudAccess }),
              queryClient.invalidateQueries({ queryKey: queryKeys.cloudSites }),
            ]);
          }}
        />
      )}
    </div>
  );
}

function LocalCloudAccessSection({
  config,
  onMessage,
  onRefresh,
}: {
  config: {
    local_site_key: string | null;
    cloud_server_base_url: string | null;
    sync_interval_seconds: number;
    sync_enabled: boolean;
    last_attempt_at: string | null;
    last_success_at: string | null;
    last_error: string | null;
  };
  onMessage: (tone: "info" | "danger", text: string) => void;
  onRefresh: () => Promise<void>;
}) {
  const [cloudServerBaseUrl, setCloudServerBaseUrl] = useState(config.cloud_server_base_url ?? "");
  const [syncIntervalSeconds, setSyncIntervalSeconds] = useState(String(config.sync_interval_seconds || 300));
  const [syncEnabled, setSyncEnabled] = useState(Boolean(config.sync_enabled));

  const saveMutation = useMutation({
    mutationFn: async () => {
      const parsedInterval = Number.parseInt(syncIntervalSeconds, 10);
      if (!Number.isFinite(parsedInterval)) {
        throw new Error("Sync interval must be a number.");
      }
      return updateCloudAccessConfig({
        cloud_server_base_url: cloudServerBaseUrl,
        sync_interval_seconds: parsedInterval,
        sync_enabled: syncEnabled,
      });
    },
    onSuccess: async () => {
      await onRefresh();
      onMessage("info", "Cloud access settings updated.");
    },
    onError: (error: unknown) => {
      onMessage(
        "danger",
        error instanceof Error ? error.message : "Failed to update cloud access settings.",
      );
    },
  });

  const rotateMutation = useMutation({
    mutationFn: rotateCloudAccessKey,
    onSuccess: async () => {
      await onRefresh();
      onMessage("info", "Local site key rotated.");
    },
    onError: (error: unknown) => {
      onMessage(
        "danger",
        error instanceof Error ? error.message : "Failed to rotate local site key.",
      );
    },
  });

  return (
    <>
      <CollapsibleCard
        title="Local controller replication settings"
        description="Set the cloud server endpoint and sync cadence. The local controller pushes data using the 32-character site key."
        defaultOpen
      >
        <div className="space-y-4">
          <div>
            <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Local site key (32 characters)
            </label>
            <Input
              value={config.local_site_key ?? ""}
              readOnly
              className="mt-1 font-mono"
              placeholder="Generating…"
            />
          </div>

          <div>
            <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Cloud server URL (FQDN or IP)
            </label>
            <Input
              value={cloudServerBaseUrl}
              onChange={(event) => setCloudServerBaseUrl(event.target.value)}
              className="mt-1"
              placeholder="https://cloud.example.com"
            />
          </div>

          <div>
            <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Sync interval (seconds)
            </label>
            <Input
              value={syncIntervalSeconds}
              onChange={(event) => setSyncIntervalSeconds(event.target.value)}
              className="mt-1"
              inputMode="numeric"
              placeholder="300"
            />
          </div>

          <label className="flex items-center gap-2 text-sm text-foreground">
            <input
              type="checkbox"
              checked={syncEnabled}
              onChange={(event) => setSyncEnabled(event.target.checked)}
              className="size-4"
            />
            Enable cloud sync
          </label>

          <div className="flex flex-wrap gap-2">
            <NodeButton
              type="button"
              variant="primary"
              onClick={() => saveMutation.mutate()}
              disabled={saveMutation.isPending}
            >
              {saveMutation.isPending ? "Saving…" : "Save settings"}
            </NodeButton>
            <NodeButton
              type="button"
              variant="secondary"
              onClick={() => rotateMutation.mutate()}
              disabled={rotateMutation.isPending}
            >
              {rotateMutation.isPending ? "Rotating…" : "Rotate site key"}
            </NodeButton>
          </div>
        </div>
      </CollapsibleCard>

      <CollapsibleCard
        title="Sync status"
        description="Status reported by the local push worker."
        defaultOpen={false}
      >
        <dl className="grid gap-3 text-sm sm:grid-cols-2">
          <StatusRow label="Last attempt" value={config.last_attempt_at ?? "Never"} />
          <StatusRow label="Last success" value={config.last_success_at ?? "Never"} />
          <StatusRow label="Last error" value={config.last_error ?? "None"} className="sm:col-span-2" />
        </dl>
      </CollapsibleCard>
    </>
  );
}

function CloudRegistrySection({
  config,
  sites,
  sitesLoading,
  sitesError,
  onMessage,
  onRefresh,
}: {
  config: { registered_site_count: number };
  sites: Array<{
    site_id: string;
    site_name: string;
    key_fingerprint: string;
    last_ingested_at: string | null;
    last_metrics_count: number | null;
    last_payload_bytes: number | null;
  }>;
  sitesLoading: boolean;
  sitesError: unknown;
  onMessage: (tone: "info" | "danger", text: string) => void;
  onRefresh: () => Promise<void>;
}) {
  const [siteName, setSiteName] = useState("");
  const [siteKey, setSiteKey] = useState("");
  const [selectedSiteId, setSelectedSiteId] = useState<string | null>(null);
  const [selectedSnapshot, setSelectedSnapshot] = useState<Record<string, unknown> | null>(null);
  const [snapshotLoading, setSnapshotLoading] = useState(false);

  const registerMutation = useMutation({
    mutationFn: async () => registerCloudSite({ site_name: siteName, site_key: siteKey }),
    onSuccess: async () => {
      setSiteName("");
      setSiteKey("");
      await onRefresh();
      onMessage("info", "Cloud site registered.");
    },
    onError: (error: unknown) => {
      onMessage("danger", error instanceof Error ? error.message : "Failed to register cloud site.");
    },
  });

  const deleteMutation = useMutation({
    mutationFn: async (siteId: string) => deleteCloudSite(siteId),
    onSuccess: async () => {
      if (selectedSiteId) {
        setSelectedSiteId(null);
        setSelectedSnapshot(null);
      }
      await onRefresh();
      onMessage("info", "Cloud site removed.");
    },
    onError: (error: unknown) => {
      onMessage("danger", error instanceof Error ? error.message : "Failed to remove cloud site.");
    },
  });

  const snapshotSummary = useMemo(() => {
    if (!selectedSnapshot) return null;
    const dashboard =
      typeof selectedSnapshot.dashboard_snapshot === "object" && selectedSnapshot.dashboard_snapshot
        ? (selectedSnapshot.dashboard_snapshot as Record<string, unknown>)
        : null;
    if (!dashboard) return null;

    const asCount = (value: unknown) => (Array.isArray(value) ? value.length : 0);
    return {
      nodes: asCount(dashboard.nodes),
      sensors: asCount(dashboard.sensors),
      outputs: asCount(dashboard.outputs),
      users: asCount(dashboard.users),
      schedules: asCount(dashboard.schedules),
      alarms: asCount(dashboard.alarms),
      metrics: Array.isArray(selectedSnapshot.metrics) ? selectedSnapshot.metrics.length : 0,
      sentAt:
        typeof selectedSnapshot.sent_at === "string" ? selectedSnapshot.sent_at : "Unknown",
      source:
        typeof selectedSnapshot.source_name === "string"
          ? selectedSnapshot.source_name
          : "Unknown",
    };
  }, [selectedSnapshot]);

  const loadSnapshot = async (siteId: string) => {
    setSnapshotLoading(true);
    setSelectedSiteId(siteId);
    try {
      const snapshot = await fetchCloudSiteSnapshot(siteId);
      setSelectedSnapshot(snapshot);
      if (!snapshot) {
        onMessage("info", "No snapshot has been received for this site yet.");
      }
    } catch (error) {
      onMessage(
        "danger",
        error instanceof Error ? error.message : "Failed to load site snapshot.",
      );
    } finally {
      setSnapshotLoading(false);
    }
  };

  return (
    <>
      <CollapsibleCard
        title="Cloud site registry"
        description="Register local-site keys and friendly names so incoming data is isolated by site."
        defaultOpen
      >
        <div className="space-y-4">
          <p className="text-sm text-muted-foreground">
            Registered sites: <span className="font-semibold text-foreground">{config.registered_site_count}</span>
          </p>

          <div>
            <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Site name
            </label>
            <Input
              value={siteName}
              onChange={(event) => setSiteName(event.target.value)}
              className="mt-1"
              placeholder="North Orchard"
            />
          </div>

          <div>
            <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Site key (32 characters)
            </label>
            <Input
              value={siteKey}
              onChange={(event) => setSiteKey(event.target.value)}
              className="mt-1 font-mono"
              placeholder="Paste local site key"
            />
          </div>

          <NodeButton
            type="button"
            variant="primary"
            onClick={() => registerMutation.mutate()}
            disabled={registerMutation.isPending}
          >
            {registerMutation.isPending ? "Registering…" : "Register site"}
          </NodeButton>
        </div>
      </CollapsibleCard>

      <CollapsibleCard
        title="Registered sites"
        description="Each site is stored in an isolated namespace with its own snapshots and metrics history."
        defaultOpen
      >
        {sitesLoading ? (
          <p className="text-sm text-muted-foreground">Loading sites…</p>
        ) : sitesError ? (
          <InlineBanner tone="danger">
            {sitesError instanceof Error ? sitesError.message : "Failed to load sites."}
          </InlineBanner>
        ) : sites.length === 0 ? (
          <p className="text-sm text-muted-foreground">No cloud sites are registered yet.</p>
        ) : (
          <div className="space-y-3">
            {sites.map((site) => (
              <div key={site.site_id} className="rounded-lg border border-border p-3">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <p className="text-sm font-semibold text-foreground">{site.site_name}</p>
                    <p className="text-xs text-muted-foreground">
                      id: {site.site_id} · key hash: {site.key_fingerprint}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      last ingest: {site.last_ingested_at ?? "Never"}
                      {site.last_metrics_count != null ? ` · metrics: ${site.last_metrics_count}` : ""}
                      {site.last_payload_bytes != null ? ` · bytes: ${site.last_payload_bytes}` : ""}
                    </p>
                  </div>
                  <div className="flex gap-2">
                    <NodeButton
                      type="button"
                      size="sm"
                      variant="secondary"
                      onClick={() => loadSnapshot(site.site_id)}
                      disabled={snapshotLoading}
                    >
                      {selectedSiteId === site.site_id && snapshotLoading
                        ? "Loading…"
                        : "View snapshot"}
                    </NodeButton>
                    <NodeButton
                      type="button"
                      size="sm"
                      variant="danger"
                      onClick={() => deleteMutation.mutate(site.site_id)}
                      disabled={deleteMutation.isPending}
                    >
                      Remove
                    </NodeButton>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </CollapsibleCard>

      {snapshotSummary ? (
        <CollapsibleCard
          title="Latest snapshot summary"
          description="Read-only view of the latest pushed payload for the selected site."
          defaultOpen
        >
          <dl className="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
            <StatusRow label="Source" value={snapshotSummary.source} />
            <StatusRow label="Sent at" value={snapshotSummary.sentAt} />
            <StatusRow label="Nodes" value={String(snapshotSummary.nodes)} />
            <StatusRow label="Sensors" value={String(snapshotSummary.sensors)} />
            <StatusRow label="Outputs" value={String(snapshotSummary.outputs)} />
            <StatusRow label="Users" value={String(snapshotSummary.users)} />
            <StatusRow label="Schedules" value={String(snapshotSummary.schedules)} />
            <StatusRow label="Alarms" value={String(snapshotSummary.alarms)} />
            <StatusRow label="Metrics in payload" value={String(snapshotSummary.metrics)} className="sm:col-span-2" />
          </dl>
        </CollapsibleCard>
      ) : null}
    </>
  );
}

function StatusRow({
  label,
  value,
  className,
}: {
  label: string;
  value: string;
  className?: string;
}) {
  return (
    <div className={className}>
      <dt className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">{label}</dt>
      <dd className="mt-1 break-all text-sm text-foreground">{value}</dd>
    </div>
  );
}
