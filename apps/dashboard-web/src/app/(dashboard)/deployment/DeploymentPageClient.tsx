"use client";

import { useEffect, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import clsx from "clsx";
import { fetchAdoptionCandidates } from "@/lib/api";
import { fetchJson, postJson } from "@/lib/http";
import NodeButton from "@/features/nodes/components/NodeButton";
import AdoptionModal from "@/features/nodes/components/AdoptionModal";
import type { DemoAdoptionCandidate, DemoNode } from "@/types/dashboard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import PageHeaderCard from "@/components/PageHeaderCard";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Input } from "@/components/ui/input";

type StepStatus = "pending" | "running" | "completed" | "failed";
type JobStatus = "queued" | "running" | "success" | "failed";

type DeploymentStep = {
  name: string;
  status: StepStatus;
  started_at?: string | null;
  finished_at?: string | null;
  logs: string[];
};

type DeploymentNodeInfo = {
  node_id?: string | null;
  node_name?: string | null;
  adoption_token?: string | null;
  mac_eth?: string | null;
  mac_wifi?: string | null;
  host?: string | null;
};

type DeploymentJob = {
  id: string;
  status: JobStatus;
  created_at: string;
  started_at?: string | null;
  finished_at?: string | null;
  steps: DeploymentStep[];
  error?: string | null;
  node?: DeploymentNodeInfo | null;
  outcome?: string | null;
};

type DeploymentForm = {
  host: string;
  port: string;
  username: string;
  password: string;
  nodeName: string;
  nodeId: string;
  mqttUrl: string;
  mqttUsername: string;
  mqttPassword: string;
};

type HostKeyScanResponse = {
  host: string;
  port: number;
  key_type: string;
  fingerprint_sha256: string;
  known_hosts_entry: string;
};

const initialForm: DeploymentForm = {
  host: "",
  port: "22",
  username: "pi",
  password: "",
  nodeName: "",
  nodeId: "",
  mqttUrl: "",
  mqttUsername: "",
  mqttPassword: "",
};

const statusStyles: Record<JobStatus, string> = {
  queued:
    "border-border bg-card-inset text-card-foreground",
  running:
 "border-blue-200 bg-blue-50 text-blue-800",
  success:
    "border-success-surface-border bg-success-surface text-success-surface-foreground",
  failed:
    "border-red-200 bg-red-50 text-red-800",
};

const stepDotStyles: Record<StepStatus, string> = {
 pending: "bg-gray-300",
  running: "bg-blue-500",
  completed: "bg-emerald-500",
  failed: "bg-rose-500",
};

export default function DeploymentPageClient() {
  const router = useRouter();
  const [form, setForm] = useState<DeploymentForm>(initialForm);
  const [job, setJob] = useState<DeploymentJob | null>(null);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const [polling, setPolling] = useState(false);
  const [hostKey, setHostKey] = useState<HostKeyScanResponse | null>(null);
  const [hostKeyApproved, setHostKeyApproved] = useState(false);

  const canDeploy = form.host.trim() && form.username.trim() && form.password && form.nodeName.trim();
  const canScanHostKey = form.host.trim() && Number.parseInt(form.port, 10);
  const jobId = job?.id;
  const jobStatus = job?.status;
  const canScanAdoption = Boolean(job?.node?.mac_eth || job?.node?.mac_wifi);

  const [adoptCandidate, setAdoptCandidate] = useState<DemoAdoptionCandidate | null>(null);
  const [matchedCandidate, setMatchedCandidate] = useState<DemoAdoptionCandidate | null>(null);
  const [scanState, setScanState] = useState<"idle" | "loading" | "complete" | "error">("idle");
  const [scanError, setScanError] = useState<string | null>(null);
  const [adoptedNode, setAdoptedNode] = useState<DemoNode | null>(null);
  const [didAutoScan, setDidAutoScan] = useState(false);

  useEffect(() => {
    if (!jobId || jobStatus === "success" || jobStatus === "failed") {
      setPolling(false);
      return;
    }
    setPolling(true);
    const interval = window.setInterval(async () => {
      try {
        const updated = await fetchJson<DeploymentJob>(`/api/deployments/pi5/${jobId}`);
        setJob(updated);
        if (updated.status === "success" || updated.status === "failed") {
          setPolling(false);
          window.clearInterval(interval);
        }
      } catch (err) {
        const text = err instanceof Error ? err.message : "Failed to refresh deployment status";
        setMessage({ type: "error", text });
        setPolling(false);
        window.clearInterval(interval);
      }
    }, 2500);

    return () => {
      window.clearInterval(interval);
    };
  }, [jobId, jobStatus]);

  useEffect(() => {
    setHostKey(null);
    setHostKeyApproved(false);
  }, [form.host, form.port]);

  useEffect(() => {
    setMatchedCandidate(null);
    setScanState("idle");
    setScanError(null);
    setAdoptCandidate(null);
    setAdoptedNode(null);
    setDidAutoScan(false);
  }, [jobId]);

  const scanForAdoptionCandidate = async () => {
    if (job?.status !== "success") return;
    const macs = [job.node?.mac_eth, job.node?.mac_wifi].filter(Boolean) as string[];
    if (macs.length === 0) return;

    setScanState("loading");
    setScanError(null);
    try {
      const candidates = await fetchAdoptionCandidates();
      const macSet = new Set(macs.map(normalizeMac));
      const match =
        candidates.find((candidate) => {
          const eth = candidate.mac_eth ? normalizeMac(candidate.mac_eth) : "";
          const wifi = candidate.mac_wifi ? normalizeMac(candidate.mac_wifi) : "";
          return (eth && macSet.has(eth)) || (wifi && macSet.has(wifi));
        }) ?? null;
      setMatchedCandidate(match);
      setScanState("complete");
    } catch (err) {
      setMatchedCandidate(null);
      setScanState("error");
      setScanError(err instanceof Error ? err.message : "Failed to scan for adoptable nodes.");
    }
  };

  useEffect(() => {
    if (didAutoScan) return;
    if (job?.status !== "success") return;
    const macs = [job.node?.mac_eth, job.node?.mac_wifi].filter(Boolean) as string[];
    if (macs.length === 0) return;
    setDidAutoScan(true);
    void scanForAdoptionCandidate();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- scanForAdoptionCandidate intentionally omitted to avoid re-triggering on function identity change
  }, [didAutoScan, job?.status, job?.node?.mac_eth, job?.node?.mac_wifi]);

  const logLines = useMemo(() => {
    if (!job) return [];
    const secrets = [form.password, form.mqttPassword, job.node?.adoption_token ?? ""].filter(Boolean);
    return job.steps.flatMap((step) =>
      step.logs.map((line) => `[${step.name}] ${redactSecrets(line, secrets)}`),
    );
  }, [job, form.password, form.mqttPassword]);

  const scanHostKey = async () => {
    const port = Number.parseInt(form.port, 10);
    if (!form.host.trim() || !Number.isFinite(port)) return;
    setBusy(true);
    setMessage(null);
    try {
      const response = await postJson<HostKeyScanResponse>("/api/deployments/pi5/host-key", {
        host: form.host.trim(),
        port: port,
      });
      setHostKey(response);
      setHostKeyApproved(false);
      setMessage({
        type: "success",
        text: "Host key fetched. Verify the fingerprint matches your Pi, then approve it to deploy.",
      });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to fetch host key fingerprint";
      setMessage({ type: "error", text });
    } finally {
      setBusy(false);
    }
  };

  const startDeployment = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const payload = buildPayload(form, hostKeyApproved ? hostKey : null);
      const response = await postJson<DeploymentJob>("/api/deployments/pi5", payload);
      setJob(response);
      setMessage({ type: "success", text: "Deployment started. Keep this window open for progress." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to start deployment";
      setMessage({ type: "error", text });
    } finally {
      setBusy(false);
    }
  };

  const copyToken = async () => {
    if (!job?.node?.adoption_token) return;
    await navigator.clipboard.writeText(job.node.adoption_token);
    setMessage({ type: "success", text: "Adoption token copied." });
  };

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Deploy & adopt a Pi 5 node"
        description="Connect over SSH to a fresh Raspberry Pi OS Lite install, deploy the Farm node-agent, then adopt the node so you can configure sensors from the dashboard."
      />

      {message && (
        <InlineBanner tone={message.type === "success" ? "success" : "error"}>
          {message.text}
        </InlineBanner>
      )}

      <CollapsibleCard
        title="Deploy over SSH"
        description="Enter SSH credentials and a node name. Passwords are used only for this session."
        defaultOpen
        actions={
          <NodeButton
            type="button"
            onClick={startDeployment}
            disabled={!canDeploy || busy}
            variant="primary"
          >
            {busy ? "Starting..." : "Connect & Deploy"}
          </NodeButton>
        }
      >

 <Card className="mt-4 gap-0 bg-card-inset p-4 text-sm text-muted-foreground">
 <p className="font-semibold text-foreground">Prerequisites (Pi 5)</p>
 <ul className="mt-2 list-disc space-y-1 pl-5 text-sm text-muted-foreground">
            <li>Raspberry Pi OS Lite 64-bit installed and booted on the LAN.</li>
            <li>SSH enabled and reachable from the controller.</li>
            <li>Correct username + password available (or configure key auth later).</li>
            <li>The Pi is on stable power/network (deployment can take several minutes).</li>
          </ul>
        </Card>

        <div className="mt-4 grid gap-4 md:grid-cols-2">
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Pi IP / Host
            </label>
            <Input
              value={form.host}
              onChange={(event) => setForm((prev) => ({ ...prev, host: event.target.value }))}
              aria-label="Pi IP / Host"
              className="mt-1"
              placeholder="192.168.1.42"
            />
          </div>
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              SSH Port
            </label>
            <Input
              value={form.port}
              onChange={(event) => setForm((prev) => ({ ...prev, port: event.target.value }))}
              aria-label="SSH Port"
              className="mt-1"
              inputMode="numeric"
              placeholder="22"
            />
          </div>
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Username
            </label>
            <Input
              value={form.username}
              onChange={(event) => setForm((prev) => ({ ...prev, username: event.target.value }))}
              aria-label="Username"
              className="mt-1"
              placeholder="pi"
            />
          </div>
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Password
            </label>
            <Input
              value={form.password}
              onChange={(event) => setForm((prev) => ({ ...prev, password: event.target.value }))}
              aria-label="Password"
              className="mt-1"
              type="password"
              placeholder="password"
              autoComplete="current-password"
            />
          </div>
          <div className="md:col-span-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Node display name
            </label>
            <Input
              value={form.nodeName}
              onChange={(event) => setForm((prev) => ({ ...prev, nodeName: event.target.value }))}
              aria-label="Node display name"
              className="mt-1"
              placeholder="Pi5 Node 3"
            />
 <p className="mt-1 text-xs text-muted-foreground">
              This name is used during adoption and shows up everywhere in the dashboard.
            </p>
          </div>
        </div>

        <Card className="mt-6 gap-0 bg-card-inset p-4">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <p className="text-sm font-semibold text-card-foreground">
                SSH host key verification
              </p>
 <p className="text-xs text-muted-foreground">
                First-time connections must verify the Pi’s SSH fingerprint. Approving stores it on the controller and blocks mismatches.
              </p>
            </div>
            <NodeButton
              type="button"
              onClick={scanHostKey}
              disabled={!canScanHostKey || busy}
              size="xs"
            >
              {busy ? "Working..." : "Fetch host key"}
            </NodeButton>
          </div>

          {hostKey && (
            <div className="mt-4 space-y-3">
              <div className="grid gap-3 md:grid-cols-2">
                <InfoBlock label="Key type" value={hostKey.key_type} />
                <InfoBlock label="Fingerprint (SHA256)" value={hostKey.fingerprint_sha256} />
              </div>
 <div className="rounded-lg bg-neutral-900 p-3">
                <pre className="overflow-auto whitespace-pre-wrap text-xs text-neutral-100">
                  {hostKey.known_hosts_entry}
                </pre>
              </div>
 <label className="flex items-center gap-2 text-xs text-muted-foreground">
                <input
                  type="checkbox"
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
                  checked={hostKeyApproved}
                  onChange={(event) => setHostKeyApproved(event.target.checked)}
                />
                I verified this fingerprint matches the Pi 5 I intend to deploy to.
              </label>
              {!hostKeyApproved && (
 <p className="text-xs text-amber-700">
                  Deployment can still be attempted, but first-time connects will fail until the host key is approved.
                </p>
              )}
            </div>
          )}
        </Card>

        <CollapsibleCard title="Advanced options (optional)" className="mt-6 bg-card-inset shadow-xs" defaultOpen={false}>
          <div className="grid gap-4 md:grid-cols-2">
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                Node ID
              </label>
              <Input
                value={form.nodeId}
                onChange={(event) => setForm((prev) => ({ ...prev, nodeId: event.target.value }))}
                className="mt-1"
                placeholder="leave blank to auto-generate"
              />
 <p className="mt-1 text-xs text-muted-foreground">
                Use only when you need to preserve a specific node identity (e.g., restoring a disk image).
              </p>
            </div>
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                MQTT URL
              </label>
              <Input
                value={form.mqttUrl}
                onChange={(event) => setForm((prev) => ({ ...prev, mqttUrl: event.target.value }))}
                className="mt-1"
                placeholder="mqtt://core.local:1883"
              />
            </div>
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                MQTT Username
              </label>
              <Input
                value={form.mqttUsername}
                onChange={(event) => setForm((prev) => ({ ...prev, mqttUsername: event.target.value }))}
                className="mt-1"
                placeholder="optional"
              />
            </div>
            <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                MQTT Password
              </label>
              <Input
                value={form.mqttPassword}
                onChange={(event) => setForm((prev) => ({ ...prev, mqttPassword: event.target.value }))}
                className="mt-1"
                type="password"
                placeholder="optional"
              />
            </div>
          </div>
        </CollapsibleCard>
      </CollapsibleCard>

      {job && (
        <CollapsibleCard
          title="Deployment status"
          description={`Job ${job.id}`}
          defaultOpen
          actions={
            <div className="flex flex-wrap items-center gap-2">
              {job.outcome ? (
                <span className="rounded-full border border-border bg-card-inset px-3 py-1 text-xs font-semibold text-card-foreground">
                  {job.outcome}
                </span>
              ) : null}
              <span
                className={clsx(
                  "rounded-full border px-3 py-1 text-xs font-semibold uppercase",
                  statusStyles[job.status],
                )}
              >
                {job.status}
              </span>
            </div>
          }
        >

          {job.error && (
            <InlineBanner tone="danger" className="mt-3 px-3 py-2 text-xs">
              {job.error}
            </InlineBanner>
          )}

          <div className="mt-4 grid gap-3 md:grid-cols-2">
            {job.steps.map((step) => (
              <Card
                key={step.name}
                className="flex items-center justify-between rounded-lg gap-0 bg-card-inset px-3 py-2 text-sm"
              >
                <div className="flex items-center gap-2">
                  <span className={clsx("h-2 w-2 rounded-full", stepDotStyles[step.status])} />
 <span className="font-medium text-foreground">{step.name}</span>
                </div>
 <span className="text-xs text-muted-foreground">{step.status}</span>
              </Card>
            ))}
          </div>

          {job.node && (
            <Card className="mt-6 grid gap-4 bg-card-inset p-4 md:grid-cols-2">
              <InfoBlock label="Node ID" value={job.node.node_id ?? "--"} />
              <InfoBlock label="Node Name" value={job.node.node_name ?? "--"} />
              <InfoBlock label="MAC (eth)" value={job.node.mac_eth ?? "--"} />
              <InfoBlock label="MAC (wifi)" value={job.node.mac_wifi ?? "--"} />
              <InfoBlock label="Adoption token (debug)" value={job.node.adoption_token ?? "--"} />
              <div className="flex items-center gap-2">
                <NodeButton
                  type="button"
                  onClick={copyToken}
                  disabled={!job.node.adoption_token}
                  size="xs"
                >
                  Copy token
                </NodeButton>
                {job.node.host && (
                  <a
                    href={`http://${job.node.host}:9000`}
 className="text-xs font-semibold text-indigo-600 underline hover:text-indigo-700"
                  >
                    Open node UI
                  </a>
                )}
              </div>
            </Card>
          )}

 <p className="mt-4 text-xs text-muted-foreground">
            {polling
              ? "Deployment in progress. Logs update every few seconds."
              : "Deployment finished."}
          </p>
        </CollapsibleCard>
      )}

      {job?.status === "success" && (
        <CollapsibleCard
          title="Adopt and configure sensors"
          description="Adoption registers the node in this controller and unlocks dashboard-driven sensor configuration."
          defaultOpen
          actions={
            <div className="flex flex-wrap items-center gap-2">
              <NodeButton type="button" onClick={() => router.push("/nodes")}>
                Open Nodes
              </NodeButton>
              {adoptedNode ? (
                <NodeButton
                  type="button"
                  variant="primary"
                  onClick={() => router.push(`/sensors?node=${encodeURIComponent(adoptedNode.id)}`)}
                >
                  Configure sensors
                </NodeButton>
              ) : null}
            </div>
          }
        >

          {adoptedNode ? (
            <InlineBanner tone="success" className="mt-4">
              Adopted <span className="font-semibold">{adoptedNode.name}</span>. Open Sensors & Outputs to add hardware sensors and push them to the node-agent.
            </InlineBanner>
          ) : (
            <div className="mt-4 space-y-3">
              <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
 <div className="text-sm text-muted-foreground">
                  {scanState === "loading" && "Scanning for the node’s mDNS broadcast…"}
                  {scanState === "idle" &&
                    (canScanAdoption
                      ? "Scan to find the deployed node on the LAN."
                      : "Waiting for deployment details (MAC address) so the node can be matched for adoption.")}
                  {scanState === "error" && (scanError ?? "Scan failed.")}
                  {scanState === "complete" && matchedCandidate
                    ? "Node broadcast detected. You can adopt it now."
                    : scanState === "complete"
                      ? "No matching broadcast found yet. Wait a few seconds and scan again."
                      : null}
                </div>
                <NodeButton
                  type="button"
                  onClick={scanForAdoptionCandidate}
                  disabled={scanState === "loading" || !canScanAdoption}
                >
                  {scanState === "loading" ? "Scanning..." : "Scan LAN"}
                </NodeButton>
              </div>

              {matchedCandidate && (
                <Card className="gap-0 bg-card-inset p-4">
                  <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <div>
                      <p className="text-sm font-semibold text-card-foreground">
                        {matchedCandidate.hostname ?? matchedCandidate.service_name}
                      </p>
 <p className="text-xs text-muted-foreground">
                        {matchedCandidate.ip
                          ? `${matchedCandidate.ip}${matchedCandidate.port ? `:${matchedCandidate.port}` : ""}`
                          : "IP pending"}
                      </p>
                    </div>
                    <NodeButton
                      type="button"
                      variant="primary"
                      onClick={() => setAdoptCandidate(matchedCandidate)}
                    >
                      Adopt now
                    </NodeButton>
                  </div>
                </Card>
              )}
            </div>
          )}
        </CollapsibleCard>
      )}

      {job && (
        <CollapsibleCard
          title="Deployment log"
          description="Verbose logs for support and troubleshooting."
          defaultOpen={false}
 actions={<span className="text-xs text-muted-foreground">{logLines.length} lines</span>}
        >
 <div className="rounded-xl bg-neutral-900 p-4">
            <pre className="max-h-[360px] overflow-auto whitespace-pre-wrap text-xs text-neutral-100">
              {logLines.length ? logLines.join("\n") : "No log entries yet."}
            </pre>
          </div>
        </CollapsibleCard>
      )}

      <AdoptionModal
        candidate={adoptCandidate}
        initialName={job?.node?.node_name ?? form.nodeName}
        restoreOptions={[]}
        onClose={() => setAdoptCandidate(null)}
        onAdoptedNode={(node) => setAdoptedNode(node)}
        onAdopted={(text) => setMessage({ type: "success", text })}
        onError={(text) => setMessage({ type: "error", text })}
      />
    </div>
  );
}

function buildPayload(form: DeploymentForm, hostKey: HostKeyScanResponse | null) {
  const port = Number.parseInt(form.port, 10);
  return {
    host: form.host.trim(),
    port: Number.isFinite(port) ? port : 22,
    username: form.username.trim(),
    password: form.password,
    node_name: form.nodeName.trim() || undefined,
    node_id: form.nodeId.trim() || undefined,
    mqtt_url: form.mqttUrl.trim() || undefined,
    mqtt_username: form.mqttUsername.trim() || undefined,
    mqtt_password: form.mqttPassword.trim() || undefined,
    host_key_fingerprint: hostKey?.fingerprint_sha256,
  };
}

function normalizeMac(value: string) {
  return value
    .toLowerCase()
    .replace(/[^0-9a-f]/g, "")
    .slice(0, 12);
}

function InfoBlock({ label, value }: { label: string; value: string }) {
  return (
    <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </p>
 <p className="mt-1 text-sm font-medium text-foreground">{value}</p>
    </div>
  );
}

function redactSecrets(line: string, secrets: string[]) {
  return secrets.reduce((current, secret) => {
    if (!secret || secret.length < 4) return current;
    return current.split(secret).join("REDACTED");
  }, line);
}
