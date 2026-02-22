"use client";

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { formatDuration, formatNumber } from "@/lib/format";
import { formatNodeStatusLabel } from "@/lib/nodeStatus";
import { formatSensorValueWithUnit } from "@/lib/sensorFormat";
import {
  clearSimLabFault,
  clearSimLabFaults,
  fetchSimLabFaults,
  fetchSimLabScenarios,
  fetchSimLabStatus,
  postSimLabAction,
  postSimLabArm,
  postSimLabFault,
  postSimLabScenario,
  postSimLabSeed,
  postSimLabTimeMultiplier,
} from "@/lib/simLabApi";
import {
  useAlarmsQuery,
  useNodesQuery,
  useOutputsQuery,
  useSensorsQuery,
} from "@/lib/queries";
import { Card } from "@/components/ui/card";
import InlineBanner from "@/components/InlineBanner";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { SimLabFault, SimLabScenario } from "@/types/simLab";

const toneDotClass = {
  ok: "bg-emerald-500",
  warn: "bg-amber-500",
  fault: "bg-rose-500",
 idle: "bg-gray-400",
} as const;

type Tone = keyof typeof toneDotClass;

const toTone = (status?: string | null): Tone => {
  const normalized = status?.toLowerCase();
  if (normalized === "online" || normalized === "active" || normalized === "on") return "ok";
  if (normalized === "offline" || normalized === "critical") return "fault";
  if (
    normalized === "warn" ||
    normalized === "warning" ||
    normalized === "high" ||
    normalized === "paused"
  )
    return "warn";
  return "idle";
};

const formatInterval = (interval?: number | null) => {
  if (interval === null || interval === undefined) return "--";
  if (interval === 0) return "cov";
  return `${interval}s`;
};

const PANEL_HEADER_CLASS =
  "mb-4 flex flex-wrap items-center justify-between gap-3 border-b border-border pb-3";
const PANEL_TAG_CLASS =
 "inline-flex items-center rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-foreground";

const RANGE_CLASS = "w-full accent-blue-600";

const BTN_BASE =
  "inline-flex items-center justify-center gap-x-2 rounded-lg px-3 py-2 text-sm font-semibold transition disabled:pointer-events-none disabled:opacity-50";
const BTN_PRIMARY = `${BTN_BASE} bg-blue-600 text-white hover:bg-blue-700`;
const BTN_WARN = `${BTN_BASE} bg-amber-500 text-white hover:bg-amber-600`;
const BTN_DANGER = `${BTN_BASE} bg-rose-600 text-white hover:bg-rose-700`;
const BTN_NEUTRAL = `${BTN_BASE} border border-border bg-white text-foreground hover:bg-muted`;

const renderStatusBadge = (tone: Tone, label: string) => (
 <span className="inline-flex items-center gap-x-2 rounded-full bg-muted px-2.5 py-1 text-xs font-medium text-foreground">
    <span className={`h-2 w-2 rounded-full ${toneDotClass[tone]}`} />
    {label}
  </span>
);

export default function SimLabPage() {
  const queryClient = useQueryClient();
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const outputsQuery = useOutputsQuery();
  const alarmsQuery = useAlarmsQuery();

  const statusQuery = useQuery({
    queryKey: ["simlab", "status"],
    queryFn: fetchSimLabStatus,
    refetchInterval: 5000,
  });
  const scenariosQuery = useQuery({
    queryKey: ["simlab", "scenarios"],
    queryFn: fetchSimLabScenarios,
    staleTime: 60_000,
  });
  const faultsQuery = useQuery({
    queryKey: ["simlab", "faults"],
    queryFn: fetchSimLabFaults,
    refetchInterval: 5000,
  });

  const nodes = useMemo(() => nodesQuery.data ?? [], [nodesQuery.data]);
  const sensors = useMemo(() => sensorsQuery.data ?? [], [sensorsQuery.data]);
  const outputs = useMemo(() => outputsQuery.data ?? [], [outputsQuery.data]);
  const alarms = useMemo(() => alarmsQuery.data ?? [], [alarmsQuery.data]);
  const status = statusQuery.data;
  const scenarios = useMemo(() => scenariosQuery.data ?? [], [scenariosQuery.data]);
  const faults = useMemo(() => faultsQuery.data ?? [], [faultsQuery.data]);

  const [selectedScenario, setSelectedScenario] = useState<string>("");
  const [selectedNodeId, setSelectedNodeId] = useState<string>("");
  const [selectedSensorId, setSelectedSensorId] = useState<string>("");
  const [selectedOutputId, setSelectedOutputId] = useState<string>("");
  const [seedValue, setSeedValue] = useState<string>("");
  const [seedTouched, setSeedTouched] = useState(false);
  const [timeMultiplier, setTimeMultiplier] = useState<number>(1);
  const [timeTouched, setTimeTouched] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  const scenarioSelection = useMemo(() => {
    if (!scenarios.length) return "";
    if (selectedScenario && scenarios.some((scenario) => scenario.id === selectedScenario)) {
      return selectedScenario;
    }
    return scenarios[0].id;
  }, [scenarios, selectedScenario]);

  const nodeSelection = useMemo(() => {
    if (!nodes.length) return "";
    if (selectedNodeId && nodes.some((node) => node.id === selectedNodeId)) {
      return selectedNodeId;
    }
    return nodes[0].id;
  }, [nodes, selectedNodeId]);

  const sensorSelection = useMemo(() => {
    if (!sensors.length) return "";
    if (selectedSensorId && sensors.some((sensor) => sensor.sensor_id === selectedSensorId)) {
      return selectedSensorId;
    }
    return sensors[0].sensor_id;
  }, [sensors, selectedSensorId]);

  const outputSelection = useMemo(() => {
    if (!outputs.length) return "";
    if (selectedOutputId && outputs.some((output) => output.id === selectedOutputId)) {
      return selectedOutputId;
    }
    return outputs[0].id;
  }, [outputs, selectedOutputId]);

  const seedDisplay = seedTouched ? seedValue : status?.seed != null ? String(status.seed) : "";
  const timeDisplay = timeTouched ? timeMultiplier : (status?.time_multiplier ?? 1);

  const sensorsByNode = useMemo(() => {
    const map = new Map<string, number>();
    sensors.forEach((sensor) => {
      map.set(sensor.node_id, (map.get(sensor.node_id) ?? 0) + 1);
    });
    return map;
  }, [sensors]);

  const outputsByNode = useMemo(() => {
    const map = new Map<string, number>();
    outputs.forEach((output) => {
      map.set(output.node_id, (map.get(output.node_id) ?? 0) + 1);
    });
    return map;
  }, [outputs]);

  const runState = status?.run_state ?? "unknown";
  const runTone: Tone =
    runState === "running" ? "ok" : runState === "paused" ? "warn" : runState === "stopped" ? "fault" : "idle";
  const isArmed = status?.armed ?? false;
  const armLabel = isArmed ? "Disarm" : "Arm Controls";
  const actionDisabled = !isArmed;

  const invalidateSimLab = () => {
    void queryClient.invalidateQueries({ queryKey: ["simlab", "status"] });
    void queryClient.invalidateQueries({ queryKey: ["simlab", "faults"] });
  };

  const runAction = async (fn: () => Promise<unknown>) => {
    setActionError(null);
    try {
      await fn();
      invalidateSimLab();
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Action failed");
    }
  };

  const applyScenario = () =>
    runAction(async () => {
      if (!scenarioSelection) return;
      await postSimLabScenario(scenarioSelection);
    });

  const applySeed = () =>
    runAction(async () => {
      const seed = Number.parseInt(seedDisplay, 10);
      if (!Number.isFinite(seed)) return;
      await postSimLabSeed(seed);
      setSeedTouched(false);
    });

  const applyMultiplier = () =>
    runAction(async () => {
      await postSimLabTimeMultiplier(timeDisplay);
      setTimeTouched(false);
    });

  const applyFault = (payload: Parameters<typeof postSimLabFault>[0]) =>
    runAction(async () => {
      await postSimLabFault(payload);
    });

  const clearFault = (faultId: string) =>
    runAction(async () => {
      await clearSimLabFault(faultId);
    });

  const clearFaults = () =>
    runAction(async () => {
      await clearSimLabFaults();
    });

  const controlError =
    statusQuery.error || scenariosQuery.error || faultsQuery.error || actionError;
  const dataError =
    nodesQuery.error || sensorsQuery.error || outputsQuery.error || alarmsQuery.error;

  return (
    <div className="mx-auto w-full max-w-7xl space-y-6 px-4 py-6 sm:px-6 lg:px-8">
      <Card className="flex-col gap-4 p-5 lg:flex-row lg:items-start lg:justify-between">
        <div className="space-y-2">
          <div className="flex flex-wrap items-center gap-3">
            <span className="inline-flex items-center rounded-full bg-blue-600 px-2.5 py-1 text-xs font-semibold uppercase tracking-wider text-white">
              Sim Lab
            </span>
 <span className="inline-flex items-center rounded-full bg-muted px-2.5 py-1 text-xs font-semibold uppercase tracking-wider text-foreground">
              CTRL-74
            </span>
          </div>
          <div className="text-xl font-semibold tracking-tight">Simulation Operations Console</div>
 <div className="text-sm text-muted-foreground">
            Domain-first monitor + control plane
          </div>
        </div>

        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-end">
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Card className="rounded-lg gap-0 bg-card-inset p-3">
 <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Run state
              </div>
              <div className="mt-1 flex items-center gap-2 text-sm font-semibold">
                <span className={`h-2.5 w-2.5 rounded-full ${toneDotClass[runTone]}`} />
                {runState}
              </div>
            </Card>
            <Card className="rounded-lg gap-0 bg-card-inset p-3">
 <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Scenario
              </div>
              <div className="mt-1 text-sm font-semibold">{status?.active_scenario ?? "--"}</div>
            </Card>
            <Card className="rounded-lg gap-0 bg-card-inset p-3">
 <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Arm
              </div>
              <div className="mt-1 flex items-center gap-2 text-sm font-semibold">
                <span className={`h-2.5 w-2.5 rounded-full ${toneDotClass[isArmed ? "ok" : "idle"]}`} />
                {isArmed ? "armed" : "safe"}
              </div>
            </Card>
            <Card className="rounded-lg gap-0 bg-card-inset p-3">
 <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
                Faults
              </div>
              <div className="mt-1 text-sm font-semibold">{status?.fault_count ?? 0}</div>
            </Card>
          </div>

          <button
            type="button"
            className={`${isArmed ? BTN_DANGER : BTN_PRIMARY} sm:self-start`}
            onClick={() => runAction(() => postSimLabArm(!isArmed, 90))}
          >
            {armLabel}
          </button>
        </div>
      </Card>

      {(controlError || dataError) && (
        <div className="space-y-2">
          {controlError && (
            <InlineBanner tone="warning" className="p-3 text-sm">
              Control API offline or error: {String(controlError)}
            </InlineBanner>
          )}
          {dataError && (
            <InlineBanner tone="danger" className="p-3 text-sm">
              Core API offline or error: {String(dataError)}
            </InlineBanner>
          )}
        </div>
      )}

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-12">
        <Card className="lg:col-span-12 p-5">
          <header className={PANEL_HEADER_CLASS}>
            <h2 className="text-base font-semibold">Sim Engine Controls</h2>
            <span className={PANEL_TAG_CLASS}>Control plane</span>
          </header>

          <div className="grid grid-cols-1 gap-4 lg:grid-cols-4">
            <Card className="space-y-3 rounded-lg gap-0 bg-card-inset p-4">
              <div className="text-sm font-semibold">Run controls</div>
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  className={`${BTN_PRIMARY} w-full`}
                  disabled={actionDisabled}
                  onClick={() => runAction(() => postSimLabAction("start"))}
                >
                  Start
                </button>
                <button
                  type="button"
                  className={`${BTN_WARN} w-full`}
                  disabled={actionDisabled}
                  onClick={() => runAction(() => postSimLabAction("pause"))}
                >
                  Pause
                </button>
                <button
                  type="button"
                  className={`${BTN_DANGER} w-full`}
                  disabled={actionDisabled}
                  onClick={() => runAction(() => postSimLabAction("stop"))}
                >
                  Stop
                </button>
                <button
                  type="button"
                  className={`${BTN_NEUTRAL} w-full`}
                  disabled={actionDisabled}
                  onClick={() => runAction(() => postSimLabAction("reset"))}
                >
                  Reset
                </button>
              </div>
 <div className="flex flex-wrap gap-2 text-xs text-muted-foreground">
 <span className="inline-flex items-center rounded-full bg-white px-2 py-1">
                  Seed {status?.seed ?? "--"}
                </span>
 <span className="inline-flex items-center rounded-full bg-white px-2 py-1">
                  Rate {formatNumber(status?.time_multiplier ?? 1)}x
                </span>
              </div>
            </Card>

            <Card className="space-y-3 rounded-lg gap-0 bg-card-inset p-4">
              <div className="text-sm font-semibold">Scenario loader</div>
 <label className="text-xs font-medium text-muted-foreground" htmlFor="scenario-select">
                Scenario
              </label>
              <Select
                id="scenario-select"
                value={scenarioSelection}
                onChange={(event) => setSelectedScenario(event.target.value)}
              >
                {scenarios.map((scenario: SimLabScenario) => (
                  <option key={scenario.id} value={scenario.id}>
                    {scenario.label}
                  </option>
                ))}
              </Select>
              <button
                type="button"
                className={BTN_PRIMARY}
                disabled={actionDisabled || !scenarioSelection}
                onClick={applyScenario}
              >
                Load scenario
              </button>
 <label className="text-xs font-medium text-muted-foreground" htmlFor="seed-input">
                Seed override
              </label>
              <div className="flex items-center gap-2">
                <Input
                  id="seed-input"
                  value={seedDisplay}
                  onChange={(event) => {
                    setSeedTouched(true);
                    setSeedValue(event.target.value);
                  }}
                  placeholder="Seed"
                />
                <button
                  type="button"
                  className={BTN_NEUTRAL}
                  disabled={actionDisabled || !seedDisplay}
                  onClick={applySeed}
                >
                  Apply
                </button>
              </div>
            </Card>

            <Card className="space-y-3 rounded-lg gap-0 bg-card-inset p-4">
              <div className="text-sm font-semibold">Time warp</div>
 <label className="text-xs font-medium text-muted-foreground" htmlFor="time-multiplier">
                Time multiplier
              </label>
              <input
                id="time-multiplier"
                className={RANGE_CLASS}
                type="range"
                min="0.25"
                max="5"
                step="0.25"
                value={timeDisplay}
                onChange={(event) => {
                  setTimeTouched(true);
                  setTimeMultiplier(Number(event.target.value));
                }}
              />
              <div className="flex items-center justify-between gap-2">
 <div className="text-sm font-semibold text-foreground">
                  x{formatNumber(timeDisplay)}
                </div>
                <button
                  type="button"
                  className={BTN_NEUTRAL}
                  disabled={actionDisabled}
                  onClick={applyMultiplier}
                >
                  Apply
                </button>
              </div>
            </Card>

            <Card className="space-y-3 rounded-lg gap-0 bg-card-inset p-4">
              <div className="text-sm font-semibold">Fault injection</div>
 <label className="text-xs font-medium text-muted-foreground" htmlFor="node-select">
                Node offline
              </label>
              <div className="flex items-center gap-2">
                <Select
                  id="node-select"
                  value={nodeSelection}
                  onChange={(event) => setSelectedNodeId(event.target.value)}
                >
                  {nodes.map((node) => (
                    <option key={node.id} value={node.id}>
                      {node.name}
                    </option>
                  ))}
                </Select>
                <button
                  type="button"
                  className={BTN_DANGER}
                  disabled={actionDisabled || !nodeSelection}
                  onClick={() =>
                    applyFault({
                      kind: "node_offline",
                      node_id: nodeSelection,
                    })
                  }
                >
                  Offline
                </button>
              </div>

 <label className="text-xs font-medium text-muted-foreground" htmlFor="sensor-select">
                Sensor spike / jitter
              </label>
              <div className="flex items-center gap-2">
                <Select
                  id="sensor-select"
                  value={sensorSelection}
                  onChange={(event) => setSelectedSensorId(event.target.value)}
                >
                  {sensors.map((sensor) => (
                    <option key={sensor.sensor_id} value={sensor.sensor_id}>
                      {sensor.name}
                    </option>
                  ))}
                </Select>
                <button
                  type="button"
                  className={BTN_WARN}
                  disabled={actionDisabled || !sensorSelection}
                  onClick={() =>
                    applyFault({
                      kind: "sensor_spike",
                      sensor_id: sensorSelection,
                      config: { every_seconds: 45, magnitude: 4.5, jitter: 0.4 },
                    })
                  }
                >
                  Spike
                </button>
                <button
                  type="button"
                  className={BTN_NEUTRAL}
                  disabled={actionDisabled || !sensorSelection}
                  onClick={() =>
                    applyFault({
                      kind: "sensor_jitter",
                      sensor_id: sensorSelection,
                      config: { sigma: 0.5 },
                    })
                  }
                >
                  Jitter
                </button>
              </div>

 <label className="text-xs font-medium text-muted-foreground" htmlFor="output-select">
                Stuck output
              </label>
              <div className="flex items-center gap-2">
                <Select
                  id="output-select"
                  value={outputSelection}
                  onChange={(event) => setSelectedOutputId(event.target.value)}
                >
                  {outputs.map((output) => (
                    <option key={output.id} value={output.id}>
                      {output.name}
                    </option>
                  ))}
                </Select>
                <button
                  type="button"
                  className={BTN_DANGER}
                  disabled={actionDisabled || !outputSelection}
                  onClick={() =>
                    applyFault({
                      kind: "stuck_output",
                      output_id: outputSelection,
                    })
                  }
                >
                  Stuck
                </button>
              </div>

              <button
                type="button"
                className={`${BTN_NEUTRAL} w-full`}
                disabled={actionDisabled || !faults.length}
                onClick={clearFaults}
              >
                Clear faults
              </button>
            </Card>
          </div>
        </Card>

        <Card className="lg:col-span-6 p-5">
          <header className={PANEL_HEADER_CLASS}>
            <h2 className="text-base font-semibold">Nodes</h2>
            <span className={PANEL_TAG_CLASS}>Controllers + remotes</span>
          </header>
          <div className="overflow-x-auto">
            <table className="min-w-full divide-y divide-border">
              <thead className="bg-card-inset">
                <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Name
                  </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Status
                  </th>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Uptime
                  </th>
 <th className="px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Sensors
                  </th>
 <th className="px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    Outputs
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {nodes.map((node) => (
                  <tr key={node.id}>
 <td className="whitespace-nowrap px-3 py-2 text-sm font-medium text-foreground">
                      {node.name}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-sm">
                      {renderStatusBadge(
                        toTone(node.status),
                        formatNodeStatusLabel(node.status ?? "unknown", node.last_seen),
                      )}
                    </td>
 <td className="whitespace-nowrap px-3 py-2 text-sm text-foreground">
                      {formatDuration(node.uptime_seconds)}
                    </td>
 <td className="whitespace-nowrap px-3 py-2 text-right text-sm text-foreground">
                      {sensorsByNode.get(node.id) ?? 0}
                    </td>
 <td className="whitespace-nowrap px-3 py-2 text-right text-sm text-foreground">
                      {outputsByNode.get(node.id) ?? 0}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {!nodes.length && (
 <div className="mt-3 text-sm text-muted-foreground">No nodes online.</div>
          )}
        </Card>

        <Card className="lg:col-span-6 p-5">
          <header className={PANEL_HEADER_CLASS}>
            <h2 className="text-base font-semibold">Sensors</h2>
            <span className={PANEL_TAG_CLASS}>Telemetry</span>
          </header>
          <ul className="divide-y divide-border text-sm">
            {sensors.slice(0, 10).map((sensor) => (
              <li key={sensor.sensor_id} className="flex items-center justify-between gap-3 py-2">
                <div className="min-w-0">
 <div className="flex items-center gap-2 font-medium text-foreground">
                    <span className={`h-2 w-2 rounded-full ${toneDotClass[toTone(sensor.status)]}`} />
                    <span className="truncate">{sensor.name}</span>
                  </div>
 <div className="mt-1 text-xs text-muted-foreground">
                    {formatSensorValueWithUnit(sensor, sensor.latest_value, "--")} · {formatInterval(sensor.interval_seconds)}
                  </div>
                </div>
                {sensor.status ? renderStatusBadge(toTone(sensor.status), sensor.status) : null}
              </li>
            ))}
          </ul>
          {!sensors.length && (
 <div className="mt-3 text-sm text-muted-foreground">No sensor telemetry.</div>
          )}
        </Card>

        <Card className="lg:col-span-6 p-5">
          <header className={PANEL_HEADER_CLASS}>
            <h2 className="text-base font-semibold">Outputs</h2>
            <span className={PANEL_TAG_CLASS}>Actuators</span>
          </header>
          <ul className="divide-y divide-border text-sm">
            {outputs.slice(0, 8).map((output) => (
              <li key={output.id} className="flex items-center justify-between gap-3 py-2">
                <div className="min-w-0">
 <div className="truncate font-medium text-foreground">
                    {output.name}
                  </div>
 <div className="mt-1 text-xs text-muted-foreground">
                    {output.type}
                  </div>
                </div>
                {renderStatusBadge(toTone(output.state), output.state ?? "unknown")}
              </li>
            ))}
          </ul>
          {!outputs.length && (
 <div className="mt-3 text-sm text-muted-foreground">No outputs configured.</div>
          )}
        </Card>

        <Card className="lg:col-span-6 p-5">
          <header className={PANEL_HEADER_CLASS}>
            <h2 className="text-base font-semibold">Alarms</h2>
            <span className={PANEL_TAG_CLASS}>Active + ack</span>
          </header>
          <ul className="divide-y divide-border text-sm">
            {alarms.slice(0, 8).map((alarm) => (
              <li key={alarm.id} className="flex items-center justify-between gap-3 py-2">
                <div className="min-w-0">
 <div className="truncate font-medium text-foreground">
                    {alarm.name}
                  </div>
 <div className="mt-1 text-xs text-muted-foreground">
                    {alarm.severity}
                  </div>
                </div>
                {renderStatusBadge(toTone(alarm.status), alarm.status ?? "unknown")}
              </li>
            ))}
          </ul>
          {!alarms.length && (
 <div className="mt-3 text-sm text-muted-foreground">No active alarms.</div>
          )}
        </Card>

        <Card className="lg:col-span-12 p-5">
          <header className={PANEL_HEADER_CLASS}>
            <h2 className="text-base font-semibold">Active faults</h2>
            <span className={PANEL_TAG_CLASS}>Overrides</span>
          </header>
          <ul className="divide-y divide-border text-sm">
            {faults.map((fault: SimLabFault) => (
              <li key={fault.id} className="flex flex-wrap items-center justify-between gap-3 py-2">
                <div className="min-w-0">
 <div className="font-medium text-foreground">{fault.kind}</div>
 <div className="mt-1 text-xs text-muted-foreground">
                    {fault.node_id || fault.sensor_id || fault.output_id || "global"}
                  </div>
                </div>
                <button
                  type="button"
                  className={`${BTN_NEUTRAL} px-3 py-1.5 text-xs`}
                  disabled={actionDisabled}
                  onClick={() => clearFault(fault.id)}
                >
                  Clear
                </button>
              </li>
            ))}
          </ul>
          {!faults.length && (
 <div className="mt-3 text-sm text-muted-foreground">No active faults.</div>
          )}
        </Card>
      </div>
    </div>
  );
}
