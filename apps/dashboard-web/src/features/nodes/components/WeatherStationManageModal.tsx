"use client";

import { useEffect, useMemo, useState } from "react";
import NodeButton from "@/features/nodes/components/NodeButton";
import {
  fetchConnection,
  getWs2902IntegrationStatusByNode,
  rotateWs2902IntegrationTokenByNode,
} from "@/lib/api";
import formatWeatherStationMissingFields from "@/features/nodes/utils/formatWeatherStationMissingFields";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type { Ws2902RotateTokenResponse, Ws2902StatusResponse } from "@/types/integrations";

function originUrl(): URL | null {
  if (typeof window === "undefined") return null;
  try {
    return new URL(window.location.origin);
  } catch {
    return null;
  }
}

function formatHost(url: URL | null): string {
  if (!url) return "(unknown host)";
  return url.hostname;
}

function formatPort(url: URL | null): string {
  if (!url) return "(unknown)";
  if (url.port) return url.port;
  return url.protocol === "https:" ? "443" : "80";
}

function parseHostname(urlString: string | null | undefined): string | null {
  if (!urlString) return null;
  try {
    const url = new URL(urlString);
    const hostname = url.hostname.trim();
    if (!hostname) return null;
    return hostname;
  } catch {
    return null;
  }
}

export default function WeatherStationManageModal({
  open,
  nodeId,
  nodeName,
  onClose,
}: {
  open: boolean;
  nodeId: string | null;
  nodeName?: string | null;
  onClose: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<Ws2902StatusResponse | null>(null);
  const [rotated, setRotated] = useState<Ws2902RotateTokenResponse | null>(null);
  const [lanHost, setLanHost] = useState<string | null>(null);

  const origin = useMemo(() => originUrl(), []);
  const originHost = useMemo(() => formatHost(origin), [origin]);
  const originPort = useMemo(() => formatPort(origin), [origin]);
  const lanHostCandidate = useMemo(() => {
    if (!lanHost) return null;
    const trimmed = lanHost.trim();
    if (!trimmed) return null;
    const lower = trimmed.toLowerCase();
    if (lower === "localhost" || lower === "127.0.0.1" || lower === "::1") return null;
    return trimmed;
  }, [lanHost]);
  const stationHost = lanHostCandidate ?? originHost;

  const ingestPath = rotated?.ingest_path ?? null;
  const ingestUrl = useMemo(() => {
    if (!origin || !ingestPath) return null;
    const base = lanHostCandidate ? `${origin.protocol}//${lanHostCandidate}:${originPort}` : origin.origin;
    return `${base}${ingestPath}`;
  }, [ingestPath, lanHostCandidate, origin, originPort]);

  const refreshStatus = async () => {
    if (!nodeId) return null;
    const next = await getWs2902IntegrationStatusByNode(nodeId);
    setStatus(next);
    return next;
  };

  useEffect(() => {
    if (!open) return;
    setBusy(false);
    setError(null);
    setStatus(null);
    setRotated(null);
    setLanHost(null);

    let canceled = false;
    fetchConnection()
      .then((connection) => {
        if (canceled) return;
        setLanHost(parseHostname(connection.local_address));
      })
      .catch(() => {
        // Ignore; we'll fall back to the current dashboard origin.
      });

    setBusy(true);
    refreshStatus()
      .catch((err) => {
        setError(err instanceof Error ? err.message : "Failed to load weather station status.");
      })
      .finally(() => {
        if (!canceled) setBusy(false);
      });

    return () => {
      canceled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, nodeId]);

  const handleRotateToken = async () => {
    if (!nodeId) return;
    setBusy(true);
    setError(null);
    try {
      const next = await rotateWs2902IntegrationTokenByNode(nodeId);
      setRotated(next);
      await refreshStatus();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Token rotation failed.");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="max-w-2xl gap-0">
        <div className="flex items-start justify-between gap-4">
          <div>
            <DialogTitle>Weather station (WS-2902)</DialogTitle>
            <DialogDescription className="mt-1">
              Rotate the token to get a fresh ingest path and reconfigure the station upload settings.
            </DialogDescription>
            {nodeName ? (
 <p className="mt-1 text-xs text-muted-foreground">
 Node: <span className="font-medium text-foreground">{nodeName}</span>
              </p>
            ) : null}
          </div>
          <NodeButton onClick={onClose} size="sm">
            Close
          </NodeButton>
        </div>

        {error && (
          <InlineBanner tone="danger" className="mt-4 rounded-lg px-3 py-2">{error}</InlineBanner>
        )}

        <div className="mt-5 space-y-4">
          <Card className="gap-0 p-4 text-sm">
 <p className="font-semibold text-foreground">Server settings</p>
            <div className="mt-3 grid gap-3 md:grid-cols-2">
              <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Host</p>
 <p className="mt-1 font-mono text-xs text-foreground">{stationHost}</p>
                {lanHostCandidate && lanHostCandidate !== originHost ? (
 <p className="mt-1 text-xs text-muted-foreground">
                    LAN host (recommended for on-network devices). Current dashboard host: {originHost}.
                  </p>
                ) : null}
              </div>
              <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Port</p>
 <p className="mt-1 font-mono text-xs text-foreground">{originPort}</p>
              </div>
              <div className="md:col-span-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Path</p>
 <p className="mt-1 font-mono text-xs text-foreground">
                  {ingestPath ?? status?.ingest_path_template ?? "(unknown)"}
                </p>
                {!ingestPath ? (
 <p className="mt-2 text-xs text-muted-foreground">
                    Rotate the token to generate a new ingest path that includes the token.
                  </p>
                ) : null}
              </div>
              <div className="md:col-span-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Full URL</p>
 <p className="mt-1 font-mono text-xs text-foreground">{ingestUrl ?? "(rotate to reveal)"}</p>
              </div>
              {rotated?.token ? (
                <div className="md:col-span-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Token</p>
 <p className="mt-1 font-mono text-xs text-foreground">{rotated.token}</p>
 <p className="mt-1 text-xs text-muted-foreground">
                    Token is embedded in the path; some station UIs also show it separately.
                  </p>
                </div>
              ) : null}
            </div>
          </Card>

          <Card className="gap-0 p-4 text-sm">
 <p className="font-semibold text-foreground">Status</p>
 <div className="mt-2 grid gap-2 text-xs text-foreground md:grid-cols-2">
              <p>Nickname: {status?.nickname ?? "—"}</p>
              <p>Protocol: {status?.protocol ?? "—"}</p>
              <p>Enabled: {typeof status?.enabled === "boolean" ? (status.enabled ? "Yes" : "No") : "—"}</p>
              <p>Last upload: {status?.last_seen ?? "—"}</p>
              <p className="md:col-span-2">
                Missing fields:{" "}
                {status?.last_missing_fields?.length
                  ? formatWeatherStationMissingFields(status.last_missing_fields)
                  : "—"}
              </p>
            </div>
          </Card>

          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div className="flex flex-wrap gap-3">
              <NodeButton type="button" onClick={() => void refreshStatus()} disabled={!nodeId || busy}>
                {busy ? "Loading..." : "Refresh status"}
              </NodeButton>
              <NodeButton type="button" onClick={() => void handleRotateToken()} disabled={!nodeId || busy} variant="primary">
                {busy ? "Rotating..." : "Rotate token"}
              </NodeButton>
            </div>
 <div className="text-xs text-muted-foreground">
              {rotated?.rotated_at ? `Rotated at ${rotated.rotated_at}.` : null}
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
