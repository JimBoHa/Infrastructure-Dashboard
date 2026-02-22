"use client";

import { useEffect, useId, useMemo, useState } from "react";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import {
  createWs2902Integration,
  fetchConnection,
  getWs2902IntegrationStatus,
  rotateWs2902IntegrationToken,
} from "@/lib/api";
import formatWeatherStationMissingFields from "@/features/nodes/utils/formatWeatherStationMissingFields";
import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type {
  Ws2902CreateRequest,
  Ws2902CreateResponse,
  Ws2902Protocol,
  Ws2902StatusResponse,
} from "@/types/integrations";

type Step = "configure" | "instructions";

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

function buildWs2902Instructions(protocol: Ws2902Protocol): string[] {
  const common = [
    "Open the station app/console settings for custom server uploads.",
    "Set the upload interval to 30 seconds (or the closest available).",
    "Save and wait for the next upload, then click “Test ingestion” below.",
  ];

  if (protocol === "ambient") {
    return ["Protocol: Ambient-style custom server upload (querystring).", ...common];
  }
  return ["Protocol: Weather Underground-compatible custom server upload (querystring).", ...common];
}

export default function WeatherStationModal({
  open,
  onClose,
  onCreated,
}: {
  open: boolean;
  onClose: () => void;
  onCreated: (message: string) => void;
}) {
  const nicknameId = useId();
  const intervalId = useId();
  const [step, setStep] = useState<Step>("configure");
  const [nickname, setNickname] = useState("");
  const [protocol, setProtocol] = useState<Ws2902Protocol>("wunderground");
  const [intervalSeconds, setIntervalSeconds] = useState<number>(30);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<Ws2902CreateResponse | null>(null);
  const [status, setStatus] = useState<Ws2902StatusResponse | null>(null);
  const [testBusy, setTestBusy] = useState(false);
  const [testResult, setTestResult] = useState<string | null>(null);
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
  const ingestUrl = useMemo(() => {
    if (!origin || !created) return null;
    const base = lanHostCandidate ? `${origin.protocol}//${lanHostCandidate}:${originPort}` : origin.origin;
    return `${base}${created.ingest_path}`;
  }, [created, lanHostCandidate, origin, originPort]);

  useEffect(() => {
    if (!open) return;
    setStep("configure");
    setNickname("");
    setProtocol("wunderground");
    setIntervalSeconds(30);
    setBusy(false);
    setError(null);
    setCreated(null);
    setStatus(null);
    setTestBusy(false);
    setTestResult(null);
    setLanHost(null);
  }, [open]);

  useEffect(() => {
    if (!open) return;

    let canceled = false;
    fetchConnection()
      .then((connection) => {
        if (canceled) return;
        setLanHost(parseHostname(connection.local_address));
      })
      .catch(() => {
        // Ignore; we'll fall back to the current dashboard origin.
      });

    return () => {
      canceled = true;
    };
  }, [open]);

  const handleCreate = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError(null);
    setTestResult(null);
    try {
      const body: Ws2902CreateRequest = {
        nickname: nickname.trim(),
        protocol,
        interval_seconds: intervalSeconds,
      };
      const response = await createWs2902Integration(body);
      setCreated(response);
      setStep("instructions");
      onCreated(`Created WS-2902 integration “${response.nickname}”.`);
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to create integration";
      setError(message);
    } finally {
      setBusy(false);
    }
  };

  const refreshStatus = async () => {
    if (!created) return null;
    const next = await getWs2902IntegrationStatus(created.id);
    setStatus(next);
    return next;
  };

  const handleTest = async () => {
    if (!created) return;
    setTestBusy(true);
    setTestResult(null);
    setError(null);

    const startedAt = Date.now();
    const timeoutMs = 120_000;
    const pollMs = 2_000;
    const baseline = status?.last_seen ?? null;

    try {
      while (Date.now() - startedAt < timeoutMs) {
        const next = await refreshStatus();
        const lastSeen = next?.last_seen ?? null;
        if (lastSeen && lastSeen !== baseline) {
          const missing = next?.last_missing_fields?.length
            ? ` Missing: ${next.last_missing_fields.join(", ")}.`
            : "";
          setTestResult(`Upload received at ${lastSeen}.${missing}`);
          return;
        }
        await new Promise((resolve) => setTimeout(resolve, pollMs));
      }
      setTestResult("No upload received yet. Confirm the station custom server settings and try again.");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Test failed";
      setError(message);
    } finally {
      setTestBusy(false);
    }
  };

  const handleSimulateUpload = async () => {
    if (!created) return;
    setTestBusy(true);
    setTestResult(null);
    setError(null);

    try {
      const query = new URLSearchParams({
        dateutc: "now",
        tempf: "72.5",
        humidity: "44",
        windspeedmph: "5.4",
        windgustmph: "8.1",
        winddir: "180",
        dailyrainin: "0.12",
        rainin: "0.01",
        uv: "3.1",
        solarradiation: "455",
        baromin: "29.92",
        PASSWORD: "secret",
      });

      const response = await fetch(`${created.ingest_path}?${query.toString()}`);
      if (!response.ok) {
        const body = await response.text().catch(() => "");
        throw new Error(`Simulated upload failed (${response.status}): ${body || response.statusText}`);
      }
      await response.json().catch(() => null);
      const next = await refreshStatus();
      const lastSeen = next?.last_seen ?? null;
      const missing = next?.last_missing_fields?.length ? ` Missing: ${next.last_missing_fields.join(", ")}.` : "";
      setTestResult(
        `Sample upload accepted${lastSeen ? ` at ${lastSeen}` : ""}.${missing}`.trim(),
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : "Simulated upload failed";
      setError(message);
    } finally {
      setTestBusy(false);
    }
  };

  const handleRotateToken = async () => {
    if (!created) return;
    setBusy(true);
    setError(null);
    setTestResult(null);
    try {
      const rotated = await rotateWs2902IntegrationToken(created.id);
      setCreated((prev) =>
        prev
          ? {
              ...prev,
              ingest_path: rotated.ingest_path,
              token: rotated.token,
            }
          : prev,
      );
      await refreshStatus();
      setTestResult("Token rotated. Update the station settings with the new ingest path.");
    } catch (err) {
      const message = err instanceof Error ? err.message : "Token rotation failed";
      setError(message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="max-w-2xl gap-0">
        <DialogTitle>Add weather station (WS-2902)</DialogTitle>
        <DialogDescription className="mt-1">
          Create a secure ingest endpoint and trend default weather sensors every 30 seconds.
        </DialogDescription>

        {step === "configure" && (
          <form className="mt-5 space-y-4" onSubmit={handleCreate}>
            <div>
 <label className="mb-2 block text-sm font-medium text-foreground" htmlFor={nicknameId}>
                Station nickname
              </label>
              <Input
                id={nicknameId}
                value={nickname}
                onChange={(event) => setNickname(event.target.value)}
                placeholder="Weather station"
              />
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              <div>
 <p className="mb-2 block text-sm font-medium text-foreground">Upload protocol</p>
 <div className="space-y-2 text-sm text-foreground">
                  <label className="flex items-start gap-2">
                    <input
                      type="radio"
                      name="wsProtocol"
                      value="wunderground"
                      checked={protocol === "wunderground"}
                      onChange={() => setProtocol("wunderground")}
 className="mt-0.5 shrink-0 rounded-full border-input text-indigo-600 focus:ring-indigo-500"
                    />
                    <span>
 <span className="font-semibold text-foreground">Weather Underground</span>
 <span className="block text-xs text-muted-foreground">
                        Recommended default (common firmware support).
                      </span>
                    </span>
                  </label>
                  <label className="flex items-start gap-2">
                    <input
                      type="radio"
                      name="wsProtocol"
                      value="ambient"
                      checked={protocol === "ambient"}
                      onChange={() => setProtocol("ambient")}
 className="mt-0.5 shrink-0 rounded-full border-input text-indigo-600 focus:ring-indigo-500"
                    />
                    <span>
 <span className="font-semibold text-foreground">Ambient-style</span>
 <span className="block text-xs text-muted-foreground">
                        Alternative mapping for custom server uploads.
                      </span>
                    </span>
                  </label>
                </div>
              </div>
              <div>
 <label className="mb-2 block text-sm font-medium text-foreground" htmlFor={intervalId}>
                  Upload interval (seconds)
                </label>
                <NumericDraftInput
                  id={intervalId}
                  value={intervalSeconds}
                  onValueChange={(next) => {
                    if (typeof next === "number") {
                      setIntervalSeconds(next);
                    }
                  }}
                  emptyBehavior="keep"
                  min={5}
                  max={3600}
                  integer
                  inputMode="numeric"
                  enforceRange
                  clampOnBlur
 className="block w-full rounded-lg border border-border px-3 py-2 text-sm focus:border-indigo-500 focus:ring-indigo-500"
                />
 <p className="mt-2 text-xs text-muted-foreground">
                  Default sensors trend at 30 seconds. Match the station interval if possible.
                </p>
              </div>
            </div>

            {error && (
              <InlineBanner tone="danger" className="rounded-lg px-3 py-2">{error}</InlineBanner>
            )}

            <div className="flex items-center justify-end gap-3">
              <NodeButton type="button" onClick={onClose}>
                Cancel
              </NodeButton>
              <NodeButton type="submit" disabled={busy} variant="primary">
                {busy ? "Creating..." : "Create integration"}
              </NodeButton>
            </div>
          </form>
        )}

        {step === "instructions" && created && (
          <div className="mt-5 space-y-4">
            <Card className="gap-0 p-4 text-sm">
 <p className="font-semibold text-foreground">Server settings</p>
              <div className="mt-3 grid gap-3 md:grid-cols-2">
                <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Host</p>
 <p className="mt-1 font-mono text-xs text-foreground">{stationHost}</p>
                  {lanHostCandidate && lanHostCandidate !== originHost && (
 <p className="mt-1 text-xs text-muted-foreground">
                      LAN host (recommended for on-network devices). Current dashboard host: {originHost}.
                    </p>
                  )}
                </div>
                <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Port</p>
 <p className="mt-1 font-mono text-xs text-foreground">{originPort}</p>
                </div>
                <div className="md:col-span-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Path</p>
 <p className="mt-1 font-mono text-xs text-foreground">{created.ingest_path}</p>
                </div>
                <div className="md:col-span-2">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Full URL</p>
 <p className="mt-1 font-mono text-xs text-foreground">{ingestUrl ?? "(unknown)"}</p>
                </div>
              </div>
            </Card>

            <Card className="gap-0 bg-card-inset p-4 text-sm text-card-foreground shadow-xs">
 <p className="font-semibold text-foreground">Next steps</p>
 <ol className="mt-2 list-decimal space-y-1 pl-5 text-sm text-foreground">
                {buildWs2902Instructions(protocol).map((line) => (
                  <li key={line}>{line}</li>
                ))}
              </ol>
 <p className="mt-3 text-xs text-muted-foreground">
                Token is embedded in the path. If you need to reconfigure later, rotate the token.
              </p>
            </Card>

            <Card className="gap-0 p-4 text-sm">
 <p className="font-semibold text-foreground">Status</p>
 <div className="mt-2 grid gap-2 text-xs text-foreground md:grid-cols-2">
                <p>Last upload: {status?.last_seen ?? "—"}</p>
                <p>
                  Missing fields:{" "}
                  {status?.last_missing_fields?.length
                    ? formatWeatherStationMissingFields(status.last_missing_fields)
                    : "—"}
                </p>
              </div>
              {status?.last_payload && (
                <CollapsibleCard title="Last payload (redacted)" className="mt-3 bg-card-inset shadow-xs" density="sm" defaultOpen={false}>
                  <pre className="max-h-48 overflow-auto rounded bg-white p-2 font-mono text-[11px] text-foreground">
                    {JSON.stringify(status.last_payload, null, 2)}
                  </pre>
                </CollapsibleCard>
              )}
 {testResult && <p className="mt-2 text-xs text-foreground">{testResult}</p>}
 {error && <p className="mt-2 text-xs text-red-700">{error}</p>}
            </Card>

            <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
              <div className="flex flex-wrap gap-3">
                <NodeButton type="button" onClick={refreshStatus} disabled={!created || busy}>
                  Refresh status
                </NodeButton>
                <NodeButton type="button" onClick={handleSimulateUpload} disabled={!created || testBusy}>
                  {testBusy ? "Sending..." : "Send sample upload"}
                </NodeButton>
                <NodeButton type="button" onClick={handleTest} disabled={!created || testBusy}>
                  {testBusy ? "Waiting for upload..." : "Test ingestion"}
                </NodeButton>
                <NodeButton type="button" onClick={handleRotateToken} disabled={!created || busy}>
                  Rotate token
                </NodeButton>
              </div>
              <div className="flex items-center justify-end gap-3">
                <NodeButton type="button" onClick={onClose}>
                  Done
                </NodeButton>
              </div>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
