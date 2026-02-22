"use client";

import { useEffect, useId, useMemo, useState } from "react";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodePill from "@/features/nodes/components/NodePill";
import { NumericDraftInput } from "@/components/forms/NumericDraftInput";
import { applyRenogyBt2Preset } from "@/lib/api";
import { formatSensorInterval } from "@/lib/sensorFormat";
import type { DemoNode } from "@/types/dashboard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import type {
  ApplyRenogyBt2PresetRequest,
  ApplyRenogyBt2PresetResponse,
  RenogyBt2Mode,
} from "@/types/integrations";

type Step = "configure" | "result";

function nodeIp(node: DemoNode | null): string | null {
  if (!node?.ip_last) return null;
  if (typeof node.ip_last === "string") return node.ip_last;
  if (typeof node.ip_last === "object" && node.ip_last && "value" in node.ip_last) {
    const value = (node.ip_last as { value?: unknown }).value;
    return typeof value === "string" ? value : null;
  }
  return null;
}

export default function RenogyBt2Modal({
  open,
  node,
  onClose,
  onApplied,
}: {
  open: boolean;
  node: DemoNode | null;
  onClose: () => void;
  onApplied: (message: string) => void;
}) {
  const addressId = useId();
  const intervalId = useId();
  const adapterId = useId();
  const [step, setStep] = useState<Step>("configure");
  const [bt2Address, setBt2Address] = useState("");
  const [pollIntervalSeconds, setPollIntervalSeconds] = useState<number>(30);
  const [advanced, setAdvanced] = useState(false);
  const [mode, setMode] = useState<RenogyBt2Mode>("ble");
  const [adapter, setAdapter] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ApplyRenogyBt2PresetResponse | null>(null);

  const ip = useMemo(() => nodeIp(node), [node]);

  useEffect(() => {
    if (!open) return;
    setStep("configure");
    setBt2Address("");
    setPollIntervalSeconds(30);
    setAdvanced(false);
    setMode("ble");
    setAdapter("");
    setBusy(false);
    setError(null);
    setResult(null);
  }, [open, node?.id]);

  const handleApply = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!node) return;
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const payload: ApplyRenogyBt2PresetRequest = {
        bt2_address: bt2Address.trim(),
        poll_interval_seconds: pollIntervalSeconds,
        ...(advanced ? { mode, adapter: adapter.trim() || undefined } : {}),
      };
      const response = await applyRenogyBt2Preset(node.id, payload);
      setResult(response);
      setStep("result");
      onApplied(
        response.status === "already_configured"
          ? `Renogy BT-2 preset already configured for ${node.name}.`
          : response.status === "stored"
            ? `Saved Renogy BT-2 settings for ${node.name} (will apply when online).`
            : `Applied Renogy BT-2 preset to ${node.name}.`,
      );
    } catch (err) {
      const message = err instanceof Error ? err.message : "Failed to apply Renogy preset";
      setError(message);
    } finally {
      setBusy(false);
    }
  };

  const canApply = Boolean(node) && bt2Address.trim().length > 0 && !busy;

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) onClose(); }}>
      <DialogContent className="max-w-2xl gap-0">
        <DialogTitle>Connect Renogy BT-2</DialogTitle>
        <DialogDescription className="mt-1">
          Apply the Renogy BT-2 preset to a node and create default power (W), voltage (V), and current (A) sensors (30s
          cadence).
        </DialogDescription>

 <div className="mt-4 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <NodePill size="md">{node ? node.name : "Unknown node"}</NodePill>
          <NodePill size="md">IP {ip ?? "-"}</NodePill>
        </div>

        {!node && (
          <InlineBanner tone="warning" className="mt-4 px-4 py-3 text-sm shadow-xs">
            Select a node first to apply this preset.
          </InlineBanner>
        )}

        {error && (
          <InlineBanner tone="danger" className="mt-4">{error}</InlineBanner>
        )}

        {step === "configure" && (
          <form className="mt-5 space-y-4" onSubmit={handleApply}>
            <div>
 <label className="mb-2 block text-sm font-medium text-foreground" htmlFor={addressId}>
                BT-2 address (MAC)
              </label>
              <Input
                id={addressId}
                value={bt2Address}
                onChange={(event) => setBt2Address(event.target.value)}
                placeholder="AA:BB:CC:DD:EE:FF"
              />
 <p className="mt-2 text-xs text-muted-foreground">
                The node must have Bluetooth enabled and be within range of the BT-2 module.
              </p>
            </div>

            <div className="grid gap-3 md:grid-cols-2">
              <div>
 <label className="mb-2 block text-sm font-medium text-foreground" htmlFor={intervalId}>
                  Poll interval (seconds)
                </label>
                <NumericDraftInput
                  id={intervalId}
                  value={pollIntervalSeconds}
                  onValueChange={(next) => {
                    if (typeof next === "number") {
                      setPollIntervalSeconds(next);
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
              </div>
              <div className="flex items-end justify-end">
                <NodeButton type="button" size="sm" onClick={() => setAdvanced((v) => !v)}>
                  {advanced ? "Hide advanced" : "Show advanced"}
                </NodeButton>
              </div>
            </div>

            {advanced && (
              <Card className="gap-0 bg-card-inset p-4 shadow-xs">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Advanced</p>
                <div className="mt-3 grid gap-3 md:grid-cols-2">
                  <div>
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Mode</p>
 <div className="mt-2 space-y-2 text-sm text-foreground">
                      <label className="flex items-start gap-2">
                        <input
                          type="radio"
                          name="renogyMode"
                          value="ble"
                          checked={mode === "ble"}
                          onChange={() => setMode("ble")}
 className="mt-0.5 shrink-0 rounded-full border-input text-indigo-600 focus:ring-indigo-500"
                        />
                        <span>
 <span className="font-semibold text-foreground">BLE</span>
 <span className="block text-xs text-muted-foreground">
                            Node reads BT-2 directly (recommended for one-click).
                          </span>
                        </span>
                      </label>
                      <label className="flex items-start gap-2">
                        <input
                          type="radio"
                          name="renogyMode"
                          value="external"
                          checked={mode === "external"}
                          onChange={() => setMode("external")}
 className="mt-0.5 shrink-0 rounded-full border-input text-indigo-600 focus:ring-indigo-500"
                        />
                        <span>
 <span className="font-semibold text-foreground">External ingest</span>
 <span className="block text-xs text-muted-foreground">
                            An external service posts telemetry into the node-agent ingest endpoint.
                          </span>
                        </span>
                      </label>
                    </div>
                  </div>
                  <div>
 <label className="mb-2 block text-sm font-medium text-foreground" htmlFor={adapterId}>
                      Bluetooth adapter
                    </label>
                    <Input
                      id={adapterId}
                      value={adapter}
                      onChange={(event) => setAdapter(event.target.value)}
                      placeholder="hci0"
                    />
 <p className="mt-2 text-xs text-muted-foreground">Leave blank to use the default adapter.</p>
                  </div>
                </div>
              </Card>
            )}

            <div className="mt-4 flex items-center justify-end gap-3">
              <NodeButton type="button" onClick={onClose} disabled={busy}>
                Cancel
              </NodeButton>
              <NodeButton type="submit" variant="primary" disabled={!canApply}>
                {busy ? "Applying..." : "Apply preset"}
              </NodeButton>
            </div>
          </form>
        )}

        {step === "result" && result && (
          <div className="mt-5 space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <NodePill tone={result.status === "applied" ? "success" : "accent"} size="md" caps>
                {result.status}
              </NodePill>
              <NodePill size="md">mode {result.mode}</NodePill>
              <NodePill size="md">interval {result.poll_interval_seconds}s</NodePill>
            </div>

            <Card className="gap-0 bg-card-inset p-4 shadow-xs">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">Node-agent target</p>
 <p className="mt-1 font-mono text-xs text-foreground">{result.node_agent_url}</p>
            </Card>

            <div>
 <p className="text-sm font-semibold text-foreground">Sensors added</p>
              <div className="mt-2 grid gap-2 md:grid-cols-2">
                {result.sensors.map((sensor) => (
                  <Card
                    key={sensor.sensor_id}
                    className="gap-0 p-3 text-sm"
                  >
 <p className="font-semibold text-foreground">{sensor.name}</p>
 <p className="mt-0.5 text-xs text-muted-foreground">
                      {sensor.type} / {sensor.metric} / {formatSensorInterval(sensor.interval_seconds).label}
                    </p>
 <p className="mt-1 font-mono text-[11px] text-muted-foreground">{sensor.sensor_id}</p>
                  </Card>
                ))}
              </div>
            </div>

            <div>
 <p className="text-sm font-semibold text-foreground">What to check if data is missing</p>
 <ul className="mt-2 list-disc space-y-1 pl-5 text-sm text-foreground">
                {result.what_to_check.map((line) => (
                  <li key={line}>{line}</li>
                ))}
              </ul>
            </div>

            <div className="mt-4 flex items-center justify-end gap-3">
              <NodeButton onClick={onClose} variant="primary">
                Done
              </NodeButton>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
