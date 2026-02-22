"use client";

import clsx from "clsx";
import { useCallback, useMemo, useRef, type KeyboardEvent } from "react";
import NodePill from "@/features/nodes/components/NodePill";
import NodeButton from "@/features/nodes/components/NodeButton";
import NodeTypeBadge from "@/features/nodes/components/NodeTypeBadge";
import SensorOriginBadge from "@/features/sensors/components/SensorOriginBadge";
import { Card } from "@/components/ui/card";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { NormalizedCandidate, SelectedBadge } from "../../types/relationshipFinder";
import AddToChartButton from "./AddToChartButton";

function shortLabel(label: string): string {
  const trimmed = label.trim();
  if (!trimmed) return trimmed;
  const parts = trimmed.split(" — ");
  const tail = parts.length > 1 ? parts.slice(1).join(" — ") : trimmed;
  return tail.replace(/\s*\([^)]*\)\s*$/, "").trim() || trimmed;
}

type ResultsListProps = {
  candidates: NormalizedCandidate[];
  selectedCandidateId: string | null;
  onSelectCandidate: (sensorId: string) => void;
  pinnedSensorIds?: string[];
  onTogglePinnedSensorId?: (sensorId: string) => void;
  sensorsById: Map<string, DemoSensor>;
  nodesById: Map<string, DemoNode>;
  badgeById: Map<string, SelectedBadge>;
  selectedSensorIds: string[];
  maxSeries: number;
  onAddToChart?: (sensorId: string) => void;
  weights?: { events: number; cooccurrence: number; deltaCorr?: number };
  maxVisible?: number;
  onShowMore?: () => void;
  hasMore?: boolean;
  emptyMessage?: string;
};

export default function ResultsList({
  candidates,
  selectedCandidateId,
  onSelectCandidate,
  pinnedSensorIds,
  onTogglePinnedSensorId,
  sensorsById,
  nodesById,
  badgeById,
  selectedSensorIds,
  maxSeries,
  onAddToChart,
  weights,
  maxVisible = 50,
  onShowMore,
  hasMore = false,
  emptyMessage = "No results found.",
}: ResultsListProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const rankScoreTooltip =
    "0–1 rank score relative to the evaluated candidates in this run. Not a probability. Not comparable across different runs or scopes.";
  const pinnedSet = useMemo(
    () => new Set((pinnedSensorIds ?? []).map((id) => id.trim()).filter(Boolean)),
    [pinnedSensorIds],
  );

  const handleKeyDown = useCallback(
    (e: KeyboardEvent<HTMLDivElement>) => {
      if (!candidates.length) return;

      const currentIndex = candidates.findIndex((c) => c.sensor_id === selectedCandidateId);
      let nextIndex = currentIndex;

      switch (e.key) {
        case "ArrowDown":
          e.preventDefault();
          nextIndex = Math.min(currentIndex + 1, candidates.length - 1);
          break;
        case "ArrowUp":
          e.preventDefault();
          nextIndex = Math.max(currentIndex - 1, 0);
          break;
        case "Home":
          e.preventDefault();
          nextIndex = 0;
          break;
        case "End":
          e.preventDefault();
          nextIndex = candidates.length - 1;
          break;
        case "Enter":
          e.preventDefault();
          if (
            selectedCandidateId &&
            onAddToChart &&
            !selectedSensorIds.includes(selectedCandidateId) &&
            selectedSensorIds.length < maxSeries
          ) {
            onAddToChart(selectedCandidateId);
          }
          return;
        default:
          return;
      }

      if (nextIndex !== currentIndex && candidates[nextIndex]) {
        onSelectCandidate(candidates[nextIndex]!.sensor_id);
      }
    },
    [candidates, selectedCandidateId, onSelectCandidate, onAddToChart, selectedSensorIds, maxSeries],
  );

  const visibleCandidates = candidates.slice(0, maxVisible);

  if (!candidates.length) {
    return (
      <Card className="rounded-lg gap-0 border-dashed px-4 py-8 text-center text-sm text-muted-foreground">
        {emptyMessage}
      </Card>
    );
  }

  return (
    <div
      ref={listRef}
      role="listbox"
      aria-label="Analysis results"
      aria-activedescendant={selectedCandidateId ? `result-${selectedCandidateId}` : undefined}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className="space-y-2 rounded-lg border border-border bg-card p-2 focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-indigo-500"
    >
      {visibleCandidates.map((candidate) => {
        const sensor = sensorsById.get(candidate.sensor_id) ?? null;
        const badge = badgeById.get(candidate.sensor_id) ?? null;
        const node = candidate.node_id ? nodesById.get(candidate.node_id) ?? null : null;
        const isSelected = candidate.sensor_id === selectedCandidateId;
        const isOnChart = selectedSensorIds.includes(candidate.sensor_id);
        const isPinned = pinnedSet.has(candidate.sensor_id);
        const evidenceMix = (() => {
          if (candidate.raw.type !== "unified") return null;
          const raw = candidate.raw.data.evidence ?? {};
          const eventsScore = typeof raw.events_score === "number" && Number.isFinite(raw.events_score)
            ? Math.max(0, raw.events_score)
            : 0;
          const cooccStrength = typeof raw.cooccurrence_strength === "number" && Number.isFinite(raw.cooccurrence_strength)
            ? Math.max(0, raw.cooccurrence_strength)
            : 0;
          const wEvents = typeof weights?.events === "number" && Number.isFinite(weights.events) ? Math.max(0, weights.events) : 0;
          const wCoocc = typeof weights?.cooccurrence === "number" && Number.isFinite(weights.cooccurrence) ? Math.max(0, weights.cooccurrence) : 0;
          const deltaAbs = typeof raw.delta_corr === "number" && Number.isFinite(raw.delta_corr)
            ? Math.min(1, Math.abs(raw.delta_corr))
            : 0;
          const wDelta = typeof weights?.deltaCorr === "number" && Number.isFinite(weights.deltaCorr) ? Math.max(0, weights.deltaCorr) : 0;
          const eventsContribution = wEvents * eventsScore;
          const cooccContribution = wCoocc * cooccStrength;
          const deltaContribution = wDelta * deltaAbs;
          const denom = eventsContribution + cooccContribution + deltaContribution;
          if (!Number.isFinite(denom) || denom <= 0) return null;
          const eventsPct = Math.max(0, Math.min(100, (eventsContribution / denom) * 100));
          const cooccPct = Math.max(0, Math.min(100, (cooccContribution / denom) * 100));
          const deltaPct = Math.max(0, Math.min(100, 100 - eventsPct - cooccPct));
          const includeDelta = wDelta > 0;
          return {
            includeDelta,
            eventsPct,
            cooccPct,
            deltaPct,
            tooltip:
              "Evidence composition (approx): percent contribution from event-match, co-occurrence buckets, and optional Δ corr using the current weights and normalized strengths. Not a probability.",
          };
        })();

        return (
          <Card
            key={candidate.sensor_id}
            id={`result-${candidate.sensor_id}`}
            role="option"
            aria-selected={isSelected}
            className={clsx(
              "cursor-pointer gap-0 border p-3 transition-colors",
              isSelected
                ? "border-indigo-300 bg-indigo-50/70"
                : "border-border bg-white hover:bg-muted",
            )}
            onClick={() => onSelectCandidate(candidate.sensor_id)}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <NodePill tone="muted" size="sm" weight="semibold">
                    #{candidate.rank}
                  </NodePill>
                  <p className="truncate text-sm font-semibold text-foreground">
                    {shortLabel(candidate.label)}
                  </p>
                  {isPinned ? (
                    <NodePill
                      tone="info"
                      size="sm"
                      weight="semibold"
                      title="Pinned candidates are always evaluated when possible."
                    >
                      Pinned
                    </NodePill>
                  ) : null}
                  {badge ? (
                    <span
                      className="h-2.5 w-2.5 flex-shrink-0 rounded-full"
                      style={{ backgroundColor: badge.color }}
                    />
                  ) : null}
                </div>
                <div className="mt-1 flex flex-wrap items-center gap-1 text-xs text-muted-foreground">
                  {node ? (
                    <>
                      <NodeTypeBadge node={node} />
                      <span className="truncate">{node.name}</span>
                    </>
                  ) : null}
                  {sensor ? <SensorOriginBadge sensor={sensor} /> : null}
                </div>
              </div>

              <div className="text-right">
                <p className="text-xs text-muted-foreground" title={rankScoreTooltip}>
                  Rank score
                </p>
                <p className="font-mono text-sm font-semibold text-foreground">
                  {candidate.score_label}
                </p>
              </div>
            </div>

            <div className="mt-3 flex flex-wrap items-center gap-1">
              {candidate.badges.slice(0, 6).map((b, idx) => (
                <NodePill key={`${b.type}-${idx}`} tone={b.tone} size="sm" weight="normal" title={b.tooltip}>
                  {b.label}: {b.value}
                </NodePill>
              ))}
            </div>

            {evidenceMix ? (
              <div className="mt-2">
                <div className="flex items-center justify-between gap-2">
                  <p className="text-[11px] font-semibold uppercase tracking-wide text-muted-foreground" title={evidenceMix.tooltip}>
                    Evidence mix
                  </p>
                  <p className="text-[11px] tabular-nums text-muted-foreground" title={evidenceMix.tooltip}>
                    Events {Math.round(evidenceMix.eventsPct)}% · Buckets {Math.round(evidenceMix.cooccPct)}%
                    {evidenceMix.includeDelta ? ` · Δ corr ${Math.round(evidenceMix.deltaPct)}%` : ""}
                  </p>
                </div>
                <div className="mt-1 h-2 w-full overflow-hidden rounded-full bg-muted" title={evidenceMix.tooltip}>
                  <div className="flex h-full w-full">
                    <div className="h-full bg-indigo-500/70" style={{ width: `${evidenceMix.eventsPct}%` }} />
                    <div className="h-full bg-amber-500/70" style={{ width: `${evidenceMix.cooccPct}%` }} />
                    {evidenceMix.includeDelta ? (
                      <div className="h-full bg-emerald-500/70" style={{ width: `${evidenceMix.deltaPct}%` }} />
                    ) : null}
                  </div>
                </div>
              </div>
            ) : null}

            <div className="mt-3 flex justify-end gap-2">
              {onTogglePinnedSensorId ? (
                <NodeButton
                  variant={isPinned ? "secondary" : "ghost"}
                  onClick={(e) => {
                    e.stopPropagation();
                    onTogglePinnedSensorId(candidate.sensor_id);
                  }}
                  className="px-2 py-1 text-xs"
                  title={isPinned ? "Unpin candidate" : "Pin candidate"}
                >
                  {isPinned ? "Unpin" : "Pin"}
                </NodeButton>
              ) : null}
              <AddToChartButton
                sensorId={candidate.sensor_id}
                isOnChart={isOnChart}
                isAtLimit={selectedSensorIds.length >= maxSeries}
                onAddToChart={onAddToChart}
              />
            </div>
          </Card>
        );
      })}

      {hasMore && onShowMore && (
        <div className="border-t border-border px-2 pt-2 text-center">
          <button
            type="button"
            onClick={onShowMore}
            className="text-xs font-semibold text-indigo-600 hover:text-indigo-700"
          >
            Show more ({candidates.length - maxVisible} remaining)
          </button>
        </div>
      )}
    </div>
  );
}
