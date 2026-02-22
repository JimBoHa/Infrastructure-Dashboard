"use client";

import { useMemo, useState } from "react";
import InlineBanner from "@/components/InlineBanner";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import { Select } from "@/components/ui/select";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useAnalysisJob, generateJobKey } from "@/features/trends/hooks/useAnalysisJob";
import type { BucketAggregationModeV1, AlarmRuleBacktestResultV1 } from "@/types/analysis";
import type { AlarmRuleCreateRequest } from "@/features/alarms/types/alarmTypes";
import { formatNumber } from "@/lib/format";

type RangePreset = "24h" | "7d" | "30d";

const computeWindow = (preset: RangePreset): { startIso: string; endIso: string } => {
  const end = new Date();
  const hours = preset === "24h" ? 24 : preset === "7d" ? 7 * 24 : 30 * 24;
  const start = new Date(end.getTime() - hours * 60 * 60 * 1000);
  return { startIso: start.toISOString(), endIso: end.toISOString() };
};

function formatDuration(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

function formatMaybeNumber(value: number | null | undefined, unit?: string): string {
  if (value == null || !Number.isFinite(value)) return "—";
  const suffix = unit ? ` ${unit}` : "";
  return `${formatNumber(value, { maximumFractionDigits: 4 })}${suffix}`;
}

export default function WizardStepBacktest({ payload }: { payload: AlarmRuleCreateRequest | null }) {
  const [rangePreset, setRangePreset] = useState<RangePreset>("7d");
  const [bucketMode, setBucketMode] = useState<BucketAggregationModeV1>("auto");
  const [selectedTargetKey, setSelectedTargetKey] = useState<string>("");

  const job = useAnalysisJob<AlarmRuleBacktestResultV1>();

  const run = async () => {
    if (!payload) return;
    const window = computeWindow(rangePreset);
    const timing = payload.timing ?? {};
    const params = {
      target_selector: payload.target_selector,
      condition_ast: payload.condition_ast,
      timing,
      start: window.startIso,
      end: window.endIso,
      interval_seconds: null as number | null,
      bucket_aggregation_mode: bucketMode,
    };

    const jobKey = generateJobKey({
      v: 1,
      job_type: "alarm_rule_backtest_v1",
      start: window.startIso,
      end: window.endIso,
      bucket_mode: bucketMode,
      target_selector: payload.target_selector,
      condition_ast: payload.condition_ast,
      timing,
    });

    await job.run("alarm_rule_backtest_v1", params, jobKey);
  };

  const result = job.result;

  const sortedTargets = useMemo(() => {
    const targets = result?.targets ?? [];
    return targets
      .slice()
      .sort((a, b) => (b.summary.time_firing_seconds ?? 0) - (a.summary.time_firing_seconds ?? 0));
  }, [result?.targets]);

  const effectiveTargetKey = useMemo(() => {
    if (selectedTargetKey && sortedTargets.some((t) => t.target_key === selectedTargetKey)) {
      return selectedTargetKey;
    }
    return sortedTargets[0]?.target_key ?? "";
  }, [selectedTargetKey, sortedTargets]);

  const selectedTarget = useMemo(() => {
    if (!effectiveTargetKey) return null;
    return sortedTargets.find((t) => t.target_key === effectiveTargetKey) ?? null;
  }, [effectiveTargetKey, sortedTargets]);

  return (
    <div className="space-y-4">
      <Card className="rounded-xl border border-border p-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
          <div className="min-w-0">
            <p className="text-sm font-semibold text-card-foreground">Backtest</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Replay this rule against historical bucketed series. Use this to tune debounce/hysteresis and reduce false positives.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <NodeButton size="sm" onClick={() => void run()} disabled={!payload || job.isSubmitting || job.isRunning}>
              {job.isSubmitting ? "Starting…" : job.isRunning ? "Running…" : "Run backtest"}
            </NodeButton>
            {job.canCancel ? (
              <NodeButton size="sm" onClick={() => void job.cancel()}>
                Cancel
              </NodeButton>
            ) : null}
          </div>
        </div>

        {!payload ? (
          <InlineBanner tone="danger" className="mt-3">
            Fix Basics/Condition inputs to run a backtest.
          </InlineBanner>
        ) : null}

        <div className="mt-4 grid gap-3 md:grid-cols-12">
          <div className="md:col-span-6">
            <label className="text-xs font-semibold text-muted-foreground">Range</label>
            <Select value={rangePreset} onChange={(e) => setRangePreset(e.target.value as RangePreset)} disabled={job.isRunning}>
              <option value="24h">Last 24h</option>
              <option value="7d">Last 7d</option>
              <option value="30d">Last 30d</option>
            </Select>
          </div>
          <div className="md:col-span-6">
            <label className="text-xs font-semibold text-muted-foreground">Aggregation</label>
            <Select value={bucketMode} onChange={(e) => setBucketMode(e.target.value as BucketAggregationModeV1)} disabled={job.isRunning}>
              <option value="auto">auto (recommended)</option>
              <option value="avg">avg</option>
              <option value="last">last</option>
              <option value="min">min</option>
              <option value="max">max</option>
              <option value="sum">sum</option>
            </Select>
          </div>
        </div>

        {job.error ? (
          <InlineBanner tone="danger" className="mt-3">
            {job.error}
          </InlineBanner>
        ) : null}

        {job.isRunning && job.progressMessage ? (
          <InlineBanner tone="info" className="mt-3">
            {job.progressMessage}
          </InlineBanner>
        ) : null}
      </Card>

      {result ? (
        <>
          <Card className="rounded-xl border border-border p-4">
            <p className="text-sm font-semibold text-card-foreground">Summary</p>
            <div className="mt-3 grid gap-3 md:grid-cols-4">
              <Metric label="Targets" value={String(result.summary.target_count)} />
              <Metric label="Fired" value={String(result.summary.total_fired)} />
              <Metric label="Resolved" value={String(result.summary.total_resolved)} />
              <Metric label="Time firing" value={formatDuration(result.summary.total_time_firing_seconds)} />
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              Window: {new Date(result.params.start).toLocaleString()} → {new Date(result.params.end).toLocaleString()} ·
              interval {result.params.interval_seconds}s · eval step {result.params.eval_step_seconds}s · agg{" "}
              {result.params.bucket_aggregation_mode}
            </p>
          </Card>

          <Card className="rounded-xl border border-border p-4">
            <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
              <div>
                <p className="text-sm font-semibold text-card-foreground">Per-target results</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  Sort order: most time firing first. Use transitions to drill into specific episodes.
                </p>
              </div>
              <div className="min-w-[280px]">
                <label className="text-xs font-semibold text-muted-foreground">Target</label>
                <Select value={effectiveTargetKey} onChange={(e) => setSelectedTargetKey(e.target.value)}>
                  {sortedTargets.map((target) => (
                    <option key={target.target_key} value={target.target_key}>
                      {target.target_key}
                    </option>
                  ))}
                </Select>
              </div>
            </div>

            {selectedTarget ? (
              <div className="mt-4 grid gap-3 md:grid-cols-3">
                <Metric label="Intervals" value={String(selectedTarget.summary.interval_count)} />
                <Metric label="Fired" value={String(selectedTarget.summary.fired_count)} />
                <Metric label="Resolved" value={String(selectedTarget.summary.resolved_count)} />
                <Metric label="Time firing" value={formatDuration(selectedTarget.summary.time_firing_seconds)} />
                <Metric
                  label="Median interval"
                  value={formatMaybeNumber(selectedTarget.summary.median_interval_seconds ?? null, "s")}
                />
                <Metric
                  label="p95 interval"
                  value={formatMaybeNumber(selectedTarget.summary.p95_interval_seconds ?? null, "s")}
                />
              </div>
            ) : null}

            {selectedTarget ? (
              <div className="mt-4 grid gap-4 md:grid-cols-2">
                <CollapsibleCard
                  title="Transitions"
                  description={`${selectedTarget.transitions.length} transitions`}
                  defaultOpen
                >
                  <div className="space-y-2">
                    {selectedTarget.transitions.length ? (
                      selectedTarget.transitions.map((transition, idx) => (
                        <Card
                          key={`${transition.timestamp}-${transition.transition}-${idx}`}
                          className="rounded-lg border border-border bg-card-inset p-3 text-sm"
                        >
                          <p className="font-semibold text-card-foreground">
                            {transition.transition.toUpperCase()} ·{" "}
                            {new Date(transition.timestamp).toLocaleString()}
                          </p>
                          {transition.observed_value != null ? (
                            <p className="mt-1 text-xs text-muted-foreground">
                              observed={formatMaybeNumber(transition.observed_value, "")}
                            </p>
                          ) : null}
                        </Card>
                      ))
                    ) : (
                      <Card className="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
                        No transitions.
                      </Card>
                    )}
                  </div>
                </CollapsibleCard>

                <CollapsibleCard
                  title="Firing intervals"
                  description={`${selectedTarget.firing_intervals.length} intervals`}
                  defaultOpen={false}
                >
                  <div className="space-y-2">
                    {selectedTarget.firing_intervals.length ? (
                      selectedTarget.firing_intervals.map((interval, idx) => (
                        <Card
                          key={`${interval.start_ts}-${interval.end_ts}-${idx}`}
                          className="rounded-lg border border-border bg-card-inset p-3 text-sm"
                        >
                          <p className="font-semibold text-card-foreground">
                            {new Date(interval.start_ts).toLocaleString()} → {new Date(interval.end_ts).toLocaleString()}
                          </p>
                          <p className="mt-1 text-xs text-muted-foreground">
                            Duration: {formatDuration(interval.duration_seconds)}
                          </p>
                        </Card>
                      ))
                    ) : (
                      <Card className="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
                        No firing intervals.
                      </Card>
                    )}
                  </div>
                </CollapsibleCard>
              </div>
            ) : null}
          </Card>
        </>
      ) : null}
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-border bg-card-inset p-3">
      <p className="text-xs font-semibold text-muted-foreground">{label}</p>
      <p className="mt-1 text-sm font-semibold text-card-foreground">{value}</p>
    </div>
  );
}
