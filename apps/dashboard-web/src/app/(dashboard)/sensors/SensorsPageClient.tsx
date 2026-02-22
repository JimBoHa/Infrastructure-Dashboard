"use client";

import { useEffect, useMemo, useState } from "react";
import { useSearchParams } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import { isPredictiveOrigin, type AlarmOriginFilter } from "@/lib/alarms/origin";
import { fetchPredictiveTrace } from "@/lib/api";
import { putJson } from "@/lib/http";
import { useAuth } from "@/components/AuthProvider";
import { useSensorsPageData } from "@/features/sensors/hooks/useSensorsPageData";
import NodeIoPanels from "@/features/sensors/components/NodeIoPanels";
import OutputsGrid from "@/features/sensors/components/OutputsGrid";
import AlarmEventsPanel from "@/features/sensors/components/AlarmEventsPanel";
import PredictiveTracePanel from "@/features/sensors/components/PredictiveTracePanel";
import SensorDetailDrawer from "@/features/sensors/components/SensorDetailDrawer";
import SensorTable from "@/features/sensors/components/SensorTable";
import SensorsOverview from "@/features/sensors/components/SensorsOverview";
import OutputCommandModal from "@/features/sensors/components/OutputCommandModal";
import { Card } from "@/components/ui/card";
import NodeButton from "@/features/nodes/components/NodeButton";
import DisplayOrderModal from "@/features/nodes/components/DisplayOrderModal";
import { getSensorDisplayDecimals } from "@/lib/sensorFormat";
import type { PredictiveTraceEntry } from "@/types/alarms";
import { queryKeys } from "@/lib/queries";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import InlineBanner from "@/components/InlineBanner";
import { Select } from "@/components/ui/select";

const TREND_RANGE_HOURS = 24;
type ActionState = "idle" | "loading" | "complete" | "error";

export default function SensorsPage() {
  const queryClient = useQueryClient();
  const { me } = useAuth();
  const searchParams = useSearchParams();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const { sensors, outputs, nodes, schedules, alarms, isLoading, error, refreshAll } =
    useSensorsPageData();
  const visibleSensors = sensors;
  const [selectedSensorId, setSelectedSensorId] = useState<string | null>(null);
  const [selectedOutputId, setSelectedOutputId] = useState<string | null>(null);
  const [nodeFilter, setNodeFilter] = useState<string>("all");
  const [typeFilter, setTypeFilter] = useState<string>("all");
  const [alarmOriginFilter, setAlarmOriginFilter] = useState<AlarmOriginFilter>("all");
  const [groupByNode, setGroupByNode] = useState(true);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(
    null,
  );
  const [traceOpen, setTraceOpen] = useState(false);
  const [trace, setTrace] = useState<PredictiveTraceEntry[]>([]);
  const [traceError, setTraceError] = useState<string | null>(null);
  const [traceLoading, setTraceLoading] = useState(false);
  const [refreshState, setRefreshState] = useState<ActionState>("idle");
  const [displayOrderOpen, setDisplayOrderOpen] = useState(false);

  useEffect(() => {
    const node = searchParams.get("node");
    if (!node) return;
    if (nodes.some((n) => n.id === node)) {
      setNodeFilter(node);
      setGroupByNode(true);
    }
  }, [nodes, searchParams]);

  useEffect(() => {
    const sensorId = searchParams.get("sensor");
    if (!sensorId) return;
    if (visibleSensors.some((sensor) => sensor.sensor_id === sensorId)) {
      setSelectedSensorId(sensorId);
    }
  }, [searchParams, visibleSensors]);

  const [bulkOpen, setBulkOpen] = useState(false);
  const [bulkType, setBulkType] = useState<string>("");
  const [bulkDecimals, setBulkDecimals] = useState<string>("auto");
  const [bulkBusy, setBulkBusy] = useState(false);
  const [bulkProgress, setBulkProgress] = useState<{ done: number; total: number } | null>(null);

  const predictiveAlarmCount = alarms.filter((alarm) =>
    isPredictiveOrigin(alarm.origin ?? alarm.type),
  ).length;

  const armTransientState = (state: ActionState) => {
    setRefreshState(state);
    if (state === "complete" || state === "error") {
      window.setTimeout(() => setRefreshState("idle"), 4000);
    }
  };

  const refreshNow = async () => {
    setMessage(null);
    setRefreshState("loading");
    try {
      refreshAll();
      await Promise.all([
        queryClient.refetchQueries({ queryKey: queryKeys.sensors, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.outputs, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.nodes, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.schedules, type: "active" }),
        queryClient.refetchQueries({ queryKey: queryKeys.alarms, type: "active" }),
      ]);
      armTransientState("complete");
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Refresh failed.",
      });
      armTransientState("error");
    }
  };

  const filteredSensors = useMemo(() => {
    return visibleSensors.filter((sensor) => {
      if (nodeFilter !== "all" && sensor.node_id !== nodeFilter) return false;
      if (typeFilter !== "all" && sensor.type !== typeFilter) return false;
      return true;
    });
  }, [nodeFilter, typeFilter, visibleSensors]);

  const sensorTypes = useMemo(() => {
    return Array.from(new Set(visibleSensors.map((sensor) => sensor.type))).sort();
  }, [visibleSensors]);

  useEffect(() => {
    if (!bulkOpen) return;
    const preferred =
      typeFilter !== "all" ? typeFilter : sensorTypes.length ? sensorTypes[0] : "";
    setBulkType(preferred);
    setBulkDecimals("auto");
    setBulkProgress(null);
  }, [bulkOpen, sensorTypes, typeFilter]);

  const activeSensor = selectedSensorId
    ? visibleSensors.find((sensor) => sensor.sensor_id === selectedSensorId) ?? null
    : null;
  const activeOutput = selectedOutputId
    ? outputs.find((output) => output.id === selectedOutputId)
    : null;

  if (isLoading) return <LoadingState label="Loading sensors..." />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load sensors."} />;
  }

  return (
    <div className="space-y-5">
      <SensorsOverview
        sensorsCount={visibleSensors.length}
        outputsCount={outputs.length}
        predictiveAlarmCount={predictiveAlarmCount}
        nodesOnlineCount={nodes.filter((node) => node.status === "online").length}
        nodes={nodes}
        groupByNode={groupByNode}
        nodeFilter={nodeFilter}
        typeFilter={typeFilter}
        alarmOriginFilter={alarmOriginFilter}
        sensorTypes={sensorTypes}
        canEdit={canEdit}
        onNodeFilterChange={setNodeFilter}
        onTypeFilterChange={setTypeFilter}
        onAlarmOriginChange={setAlarmOriginFilter}
        onGroupByNodeChange={setGroupByNode}
        onBulkDecimals={() => setBulkOpen(true)}
        onReorder={canEdit ? () => setDisplayOrderOpen(true) : undefined}
        onRefresh={() => void refreshNow()}
        refreshLoading={refreshState === "loading"}
        refreshLabel={
          refreshState === "complete" ? "Complete" : refreshState === "error" ? "Error" : undefined
        }
      />

      {message && (
        <InlineBanner tone={message.type === "success" ? "success" : "error"}>
          {message.text}
        </InlineBanner>
      )}

      {!groupByNode ? (
        <SensorTable
          sensors={filteredSensors}
          nodes={nodes}
          alarms={alarms}
          alarmOriginFilter={alarmOriginFilter}
          groupByNode={false}
          onSelectSensor={setSelectedSensorId}
        />
      ) : null}

      {groupByNode ? (
        <NodeIoPanels
          key={nodeFilter}
          nodes={nodes}
          sensors={visibleSensors}
          outputs={outputs}
          schedules={schedules}
          alarms={alarms}
          alarmOriginFilter={alarmOriginFilter}
          nodeFilter={nodeFilter}
          typeFilter={typeFilter}
          onSelectSensor={setSelectedSensorId}
          onCommandOutput={setSelectedOutputId}
        />
      ) : (
        <div className="grid gap-4 xl:grid-cols-2">
          <PredictiveTracePanel
            open={traceOpen}
            onToggle={async () => {
              if (!traceOpen) {
                setTraceLoading(true);
                setTraceError(null);
                try {
                  const data = await fetchPredictiveTrace();
                  setTrace(data);
                } catch (err) {
                  const message = err instanceof Error ? err.message : "Failed to load predictive trace";
                  setTraceError(message);
                } finally {
                  setTraceLoading(false);
                }
              }
              setTraceOpen(!traceOpen);
            }}
            loading={traceLoading}
            error={traceError}
            entries={trace}
          />

          <OutputsGrid outputs={outputs} nodes={nodes} onCommand={setSelectedOutputId} />
        </div>
      )}

      {groupByNode ? (
        <PredictiveTracePanel
          open={traceOpen}
          onToggle={async () => {
            if (!traceOpen) {
              setTraceLoading(true);
              setTraceError(null);
              try {
                const data = await fetchPredictiveTrace();
                setTrace(data);
              } catch (err) {
                const message = err instanceof Error ? err.message : "Failed to load predictive trace";
                setTraceError(message);
              } finally {
                setTraceLoading(false);
              }
            }
            setTraceOpen(!traceOpen);
          }}
          loading={traceLoading}
          error={traceError}
          entries={trace}
        />
      ) : null}

      <AlarmEventsPanel />

      <SensorDetailDrawer
        sensor={activeSensor}
        node={activeSensor ? nodes.find((node) => node.id === activeSensor.node_id) ?? null : null}
        nodes={nodes}
        sensors={visibleSensors}
        alarms={alarms}
        alarmOriginFilter={alarmOriginFilter}
        trendRangeHours={TREND_RANGE_HOURS}
        onClose={() => setSelectedSensorId(null)}
      />

      <OutputCommandModal
        output={activeOutput}
        onClose={() => setSelectedOutputId(null)}
        onComplete={(text) => {
          setMessage({ type: "success", text });
          setSelectedOutputId(null);
          refreshAll();
        }}
        onError={(text) => setMessage({ type: "error", text })}
      />

      <DisplayOrderModal
        open={displayOrderOpen}
        nodes={nodes}
        sensors={visibleSensors}
        onClose={() => setDisplayOrderOpen(false)}
      />

      {bulkOpen ? (
        <BulkDecimalsModal
          canEdit={canEdit}
          sensorTypes={sensorTypes}
          sensors={visibleSensors}
          selectedType={bulkType}
          decimals={bulkDecimals}
          busy={bulkBusy}
          progress={bulkProgress}
          onChangeType={setBulkType}
          onChangeDecimals={setBulkDecimals}
          onClose={() => {
            if (!bulkBusy) setBulkOpen(false);
          }}
          onApply={async () => {
            if (!canEdit) return;
            if (!bulkType) {
              setMessage({ type: "error", text: "Select a sensor type first." });
              return;
            }
            const desired =
              bulkDecimals === "auto"
                ? null
                : Number.parseInt(bulkDecimals, 10);
            if (desired != null && (!Number.isFinite(desired) || desired < 0 || desired > 6)) {
              setMessage({ type: "error", text: "Decimals must be between 0 and 6." });
              return;
            }

            const targets = visibleSensors.filter((sensor) => sensor.type === bulkType);
            if (!targets.length) {
              setMessage({ type: "error", text: "No sensors found for that type." });
              return;
            }

            setBulkBusy(true);
            setBulkProgress({ done: 0, total: targets.length });
            try {
              for (let idx = 0; idx < targets.length; idx += 1) {
                const sensor = targets[idx];
                const config = { ...(sensor.config ?? {}) } as Record<string, unknown>;
                if (desired == null) {
                  delete config.display_decimals;
                } else {
                  config.display_decimals = desired;
                }
                await putJson(`/api/sensors/${encodeURIComponent(sensor.sensor_id)}`, { config });
                setBulkProgress({ done: idx + 1, total: targets.length });
              }
              refreshAll();
              setMessage({
                type: "success",
                text: `Updated display decimals for ${targets.length} ${bulkType} sensor${targets.length === 1 ? "" : "s"}.`,
              });
              setBulkOpen(false);
            } catch (err) {
              setMessage({
                type: "error",
                text: err instanceof Error ? err.message : "Failed to update sensors.",
              });
            } finally {
              setBulkBusy(false);
              setBulkProgress(null);
            }
          }}
        />
      ) : null}
    </div>
  );
}

function BulkDecimalsModal({
  canEdit,
  sensorTypes,
  sensors,
  selectedType,
  decimals,
  busy,
  progress,
  onChangeType,
  onChangeDecimals,
  onClose,
  onApply,
}: {
  canEdit: boolean;
  sensorTypes: string[];
  sensors: Array<{ sensor_id: string; type: string; config: Record<string, unknown> }>;
  selectedType: string;
  decimals: string;
  busy: boolean;
  progress: { done: number; total: number } | null;
  onChangeType: (value: string) => void;
  onChangeDecimals: (value: string) => void;
  onClose: () => void;
  onApply: () => Promise<void>;
}) {
  const matching = sensors.filter((sensor) => sensor.type === selectedType);
  const currentDecimalsSummary = (() => {
    if (!selectedType) return null;
    const counts = new Map<string, number>();
    for (const sensor of matching) {
      const value = getSensorDisplayDecimals(sensor);
      const key = value == null ? "Auto" : String(value);
      counts.set(key, (counts.get(key) ?? 0) + 1);
    }
    return Array.from(counts.entries())
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([key, count]) => `${key}: ${count}`)
      .join(" · ");
  })();

  return (
    <Dialog open onOpenChange={(v) => { if (!v && !busy) onClose(); }}>
      <DialogContent className="z-[70] w-[min(520px,calc(100vw-32px))] rounded-2xl gap-0">
        <div className="flex items-start justify-between gap-3">
          <div>
            <DialogTitle>Set decimals by type</DialogTitle>
            <DialogDescription className="mt-1">
              Applies a display precision to every sensor of a given type. Individual sensors remain editable.
            </DialogDescription>
          </div>
          <NodeButton size="sm" onClick={onClose} disabled={busy}>
            Close
          </NodeButton>
        </div>

        {!canEdit ? (
 <Card className="mt-4 gap-0 bg-card-inset px-4 py-3 text-sm text-foreground">
            Read-only mode: you need <code className="px-1">config.write</code> to change sensor display settings.
          </Card>
        ) : null}

        <div className="mt-4 grid gap-4 sm:grid-cols-2">
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Sensor type
            <Select
              className="mt-1"
              value={selectedType}
              onChange={(e) => onChangeType(e.target.value)}
              disabled={!canEdit || busy}
            >
              {sensorTypes.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </Select>
          </label>

 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            Display decimals
            <Select
              className="mt-1"
              value={decimals}
              onChange={(e) => onChangeDecimals(e.target.value)}
              disabled={!canEdit || busy}
            >
              <option value="auto">Auto</option>
              {[0, 1, 2, 3, 4, 5, 6].map((value) => (
                <option key={value} value={String(value)}>
                  {value}
                </option>
              ))}
            </Select>
          </label>
        </div>

 <Card className="mt-4 gap-0 bg-card-inset px-4 py-3 text-sm text-foreground">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <span className="font-semibold">{matching.length}</span>{" "}
              {selectedType ? `${selectedType} sensor${matching.length === 1 ? "" : "s"}` : "sensors"} will be updated.
            </div>
            {currentDecimalsSummary ? (
 <div className="text-xs text-muted-foreground">{currentDecimalsSummary}</div>
            ) : null}
          </div>
          {progress ? (
 <div className="mt-2 text-xs text-muted-foreground">
              Updating {progress.done}/{progress.total}…
            </div>
          ) : null}
        </Card>

        <div className="mt-5 flex items-center justify-end gap-2">
          <NodeButton onClick={onClose} disabled={busy}>
            Cancel
          </NodeButton>
          <NodeButton variant="primary" onClick={() => void onApply()} disabled={!canEdit || busy || !selectedType}>
            {busy ? "Applying…" : "Apply"}
          </NodeButton>
        </div>
      </DialogContent>
    </Dialog>
  );
}
