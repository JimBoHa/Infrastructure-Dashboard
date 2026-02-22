/**
 * Candidate Normalizers - Convert strategy-specific results to unified NormalizedCandidate format
 *
 * Each strategy returns different result structures from the backend.
 * These normalizers transform them into a consistent format for the unified UI.
 */

import { formatNumber } from "@/lib/format";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type {
  CorrelationMatrixCellV1,
  CorrelationMatrixResultV1,
  CooccurrenceBucketV1,
  CooccurrenceResultV1,
  EventMatchResultV1,
  RelatedSensorsUnifiedResultV2,
  RelatedSensorsResultV1,
} from "@/types/analysis";
import type {
  Badge,
  BadgeType,
  CandidateStatus,
  CooccurrenceAggregatedSensor,
  NormalizedCandidate,
} from "../types/relationshipFinder";

// ─────────────────────────────────────────────────────────────────────────────
// Shared Utilities
// ─────────────────────────────────────────────────────────────────────────────

type LookupMaps = {
  sensorsById: Map<string, DemoSensor>;
  nodesById: Map<string, DemoNode>;
  labelMap: Map<string, string>;
};

function getSensorInfo(
  sensorId: string,
  lookups: LookupMaps,
): {
  label: string;
  node_name: string | null;
  node_id: string | null;
  sensor_type: string | null;
  unit: string | null;
} {
  const sensor = lookups.sensorsById.get(sensorId);
  const label = lookups.labelMap.get(sensorId) ?? sensor?.name ?? sensorId;
  const node = sensor ? lookups.nodesById.get(sensor.node_id) : null;

  return {
    label,
    node_name: node?.name ?? null,
    node_id: sensor?.node_id ?? null,
    sensor_type: sensor?.type ?? null,
    unit: sensor?.unit ?? null,
  };
}

function createBadge(
  type: BadgeType,
  label: string,
  value: string,
  tone: Badge["tone"],
  tooltip?: string,
): Badge {
  return { type, label, value, tone, tooltip };
}

function formatScore(score: number | null | undefined, decimals = 2): string {
  if (score == null || !Number.isFinite(score)) return "—";
  return formatNumber(score, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  });
}

function formatLagSeconds(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "0";
  if (seconds === 0) return "0";
  const sign = seconds < 0 ? "-" : "+";
  const absSeconds = Math.abs(seconds);
  if (absSeconds >= 3600) {
    const hours = absSeconds / 3600;
    return `${sign}${formatNumber(hours, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}h`;
  }
  if (absSeconds >= 60) {
    const minutes = absSeconds / 60;
    return `${sign}${formatNumber(minutes, { minimumFractionDigits: 0, maximumFractionDigits: 0 })}m`;
  }
  return `${sign}${absSeconds}s`;
}

function formatLagWithSemantics(seconds: number): string {
  const base = formatLagSeconds(seconds);
  if (seconds < 0) return `${base} (candidate earlier)`;
  if (seconds > 0) return `${base} (candidate later)`;
  return base;
}

function formatPercentage(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "—";
  return `${formatNumber(value, { minimumFractionDigits: 0, maximumFractionDigits: 1 })}%`;
}

// ─────────────────────────────────────────────────────────────────────────────
// Similarity (TSSE) Normalizer
// ─────────────────────────────────────────────────────────────────────────────

export function normalizeUnifiedCandidates(
  result: RelatedSensorsUnifiedResultV2,
  lookups: LookupMaps,
): NormalizedCandidate[] {
  const { candidates } = result;
  if (!candidates || !Array.isArray(candidates)) return [];

  return candidates.map((candidate, idx): NormalizedCandidate => {
    const info = getSensorInfo(candidate.sensor_id, lookups);
    const badges: Badge[] = [];
    const derivedFromFocus = candidate.derived_from_focus === true;
    const derivedPath = Array.isArray(candidate.derived_dependency_path)
      ? candidate.derived_dependency_path.filter((entry) => typeof entry === "string" && entry.trim())
      : null;

    const evidenceValue =
      candidate.confidence_tier === "high"
        ? "strong"
        : candidate.confidence_tier === "medium"
          ? "medium"
          : "weak";
    const evidenceTone =
      candidate.confidence_tier === "high"
        ? "success"
        : candidate.confidence_tier === "medium"
          ? "warning"
          : "muted";
    badges.push(
      createBadge(
        "status",
        "Evidence",
        evidenceValue,
        evidenceTone,
        "Heuristic tier based on matched events and/or shared anomaly buckets. Not statistical significance.",
      ),
    );

    if (derivedFromFocus) {
      const pathSuffix =
        derivedPath && derivedPath.length > 1 ? ` Dependency path: ${derivedPath.join(" → ")}.` : "";
      badges.push(
        createBadge(
          "status",
          "Dependency",
          "Derived from focus",
          "warning",
          `This sensor is derived and depends on the focus sensor (directly or via other derived sensors). Treat relationship evidence as non-independent.${pathSuffix}`,
        ),
      );
    }

    const eventsScore = candidate.evidence?.events_score;
    const eventsOverlap = candidate.evidence?.events_overlap;
    const nFocus = candidate.evidence?.n_focus;
    const nCandidate = candidate.evidence?.n_candidate;
    if (eventsScore != null && Number.isFinite(eventsScore)) {
      const suffix =
        eventsOverlap != null && Number.isFinite(eventsOverlap)
          ? ` • matched: ${formatNumber(eventsOverlap, { maximumFractionDigits: 0 })}`
          : "";
      const countsSuffix =
        nFocus != null && nCandidate != null && Number.isFinite(nFocus) && Number.isFinite(nCandidate)
          ? ` Focus events: ${formatNumber(nFocus, { maximumFractionDigits: 0 })}. Candidate events: ${formatNumber(nCandidate, { maximumFractionDigits: 0 })}.`
          : "";
      badges.push(
        createBadge(
          "episodes",
          "Event match (F1)",
          `${formatScore(eventsScore)}${suffix}`,
          "accent",
          `Event match uses F1 overlap of detected change events at the best lag. Matches allow a tolerance window (in buckets) after applying lag.${countsSuffix}`,
        ),
      );
    }

    const cooccCount = candidate.evidence?.cooccurrence_count;
    if (cooccCount != null && Number.isFinite(cooccCount)) {
      badges.push(
        createBadge(
          "overlap",
          "Shared buckets",
          formatNumber(cooccCount, { maximumFractionDigits: 0 }),
          "neutral",
        ),
      );
    }

    const cooccStrength = candidate.evidence?.cooccurrence_strength;
    if (cooccStrength != null && Number.isFinite(cooccStrength)) {
      badges.push(
        createBadge(
          "overlap",
          "Co-occ strength",
          formatScore(cooccStrength),
          "neutral",
          "Normalized to the strongest candidate in this run. Based on shared high-severity event buckets.",
        ),
      );
    }

    const bestLag = candidate.evidence?.best_lag_sec;
    if (bestLag != null && Number.isFinite(bestLag) && bestLag !== 0) {
      badges.push(
        createBadge(
          "lag",
          "Lag",
          formatLagWithSemantics(bestLag),
          "warning",
          "Lag is the candidate time offset that maximizes event-match overlap.",
        ),
      );
    }

    return {
      sensor_id: candidate.sensor_id,
      label: info.label,
      node_name: info.node_name,
      node_id: info.node_id,
      sensor_type: info.sensor_type,
      unit: info.unit,
      rank: candidate.rank > 0 ? candidate.rank : idx + 1,
      score: candidate.blended_score,
      score_label: formatScore(candidate.blended_score),
      badges,
      strategy: "unified",
      status:
        candidate.confidence_tier === "high"
          ? "ok"
          : candidate.confidence_tier === "medium"
            ? "not_significant"
            : "insufficient_overlap",
      raw: { type: "unified", data: candidate },
    };
  });
}

export function normalizeSimilarityCandidates(
  result: RelatedSensorsResultV1,
  lookups: LookupMaps,
): NormalizedCandidate[] {
  const { candidates } = result;

  // Guard against invalid result structure
  if (!candidates || !Array.isArray(candidates)) return [];

  return candidates.map((candidate, idx): NormalizedCandidate => {
    const info = getSensorInfo(candidate.sensor_id, lookups);
    const why = candidate.why_ranked;
    const scoreComponents = why.score_components ?? {};
    const badges: Badge[] = [];
    const penalties = why.penalties ?? [];
    const isDiurnal =
      penalties.includes("diurnal_lag") || scoreComponents["is_diurnal_lag"] === 1;

    // Score badge
    badges.push(
      createBadge("score", "Score", formatScore(candidate.score), "info"),
    );

    // Base signal badge (absolute Pearson r at best lag)
    const lagAbs = scoreComponents["lag_r_abs"] ?? scoreComponents["lag_score"];
    if (lagAbs != null && Number.isFinite(lagAbs)) {
      const tone = lagAbs >= 0.6 ? "success" : lagAbs >= 0.35 ? "info" : "muted";
      badges.push(
        createBadge(
          "correlation",
          "|r|",
          formatScore(lagAbs, 3),
          tone,
          "Absolute Pearson correlation at the best lag (effect size)",
        ),
      );
    }

    // Coverage badge (helps interpret dot-only/short-window episodes)
    if (why.coverage_pct != null) {
      badges.push(
        createBadge("coverage", "Coverage", formatPercentage(why.coverage_pct), "neutral"),
      );
    }

    // Best lag badge (annotate diurnal if flagged)
    if (why.best_lag_sec != null && why.best_lag_sec !== 0) {
      badges.push(
        createBadge(
          "lag",
          "Lag",
          formatLagSeconds(why.best_lag_sec),
          why.best_lag_sec > 0 ? "warning" : "accent",
          isDiurnal
            ? `Best lag: ${why.best_lag_sec}s (near 24h multiple; common daily-cycle artifact)`
            : `Best lag: ${why.best_lag_sec}s`,
        ),
      );
    }

    if (isDiurnal) {
      badges.push(
        createBadge(
          "status",
          "Note",
          "diurnal",
          "warning",
          "Best lag is near an integer multiple of 24h; daily cycles are common and can be non-informative.",
        ),
      );
    }

    // Episode count
    if (why.episode_count > 0) {
      badges.push(
        createBadge(
          "episodes",
          "Episodes",
          String(why.episode_count),
          "muted",
        ),
      );
    }

    const pRaw = scoreComponents["lag_p_raw"];
    if (pRaw != null && Number.isFinite(pRaw)) {
      badges.push(
        createBadge(
          "p_value",
          "p(raw)",
          pRaw < 0.001 ? "<0.001" : formatScore(pRaw, 3),
          pRaw <= 0.05 ? "success" : "warning",
          "Per-test p-value at selected lag (time-series adjusted)",
        ),
      );
    }

    const pLag = scoreComponents["lag_p_lag"];
    if (pLag != null && Number.isFinite(pLag)) {
      badges.push(
        createBadge(
          "p_value",
          "p(lag)",
          pLag < 0.001 ? "<0.001" : formatScore(pLag, 3),
          pLag <= 0.05 ? "success" : "warning",
          "Lag-selection corrected p-value (Sidak-style)",
        ),
      );
    }

    const qValue = scoreComponents["q_value"];
    if (qValue != null && Number.isFinite(qValue)) {
      badges.push(
        createBadge(
          "q_value",
          "q",
          qValue < 0.001 ? "<0.001" : formatScore(qValue, 3),
          qValue <= 0.05 ? "accent" : "warning",
          "FDR-adjusted q-value across candidate set",
        ),
      );
    }

    const nEff = scoreComponents["n_eff"];
    if (nEff != null && Number.isFinite(nEff)) {
      badges.push(
        createBadge(
          "n_eff",
          "n_eff",
          formatNumber(nEff, { maximumFractionDigits: 0 }),
          "muted",
          "Effective sample size (autocorrelation-adjusted)",
        ),
      );
    }

    const mLag = scoreComponents["m_lag"];
    if (mLag != null && Number.isFinite(mLag)) {
      badges.push(
        createBadge(
          "lag",
          "m_lag",
          formatNumber(mLag, { maximumFractionDigits: 0 }),
          "muted",
          "Number of lag hypotheses evaluated",
        ),
      );
    }

    return {
      sensor_id: candidate.sensor_id,
      label: info.label,
      node_name: info.node_name,
      node_id: info.node_id,
      sensor_type: info.sensor_type,
      unit: info.unit,
      rank: candidate.rank > 0 ? candidate.rank : idx + 1,
      score: candidate.score,
      score_label: formatScore(candidate.score),
      badges,
      strategy: "similarity",
      status: "ok",
      raw: { type: "similarity", data: candidate },
    };
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Correlation Normalizer
// ─────────────────────────────────────────────────────────────────────────────

export function normalizeCorrelationCandidates(
  result: CorrelationMatrixResultV1,
  focusSensorId: string,
  lookups: LookupMaps,
): NormalizedCandidate[] {
  const { sensor_ids, matrix } = result;

  // Guard against invalid result structure
  if (!sensor_ids || !Array.isArray(sensor_ids) || !matrix) return [];

  // Find focus sensor index
  const focusIndex = sensor_ids.indexOf(focusSensorId);
  if (focusIndex < 0) return [];

  // Extract cells from the focus row
  const candidates: Array<{
    sensorId: string;
    cell: CorrelationMatrixCellV1;
  }> = [];

  for (let col = 0; col < sensor_ids.length; col++) {
    if (col === focusIndex) continue; // Skip self-correlation
    const cell = matrix[focusIndex]?.[col];
    if (!cell) continue;
    candidates.push({
      sensorId: sensor_ids[col]!,
      cell,
    });
  }

  // Sort by status (ok first) then by |r| descending
  const statusOrder: Record<CandidateStatus, number> = {
    ok: 0,
    not_significant: 1,
    insufficient_overlap: 2,
    not_computed: 3,
  };

  candidates.sort((a, b) => {
    const statusA = (a.cell.status ?? "ok") as CandidateStatus;
    const statusB = (b.cell.status ?? "ok") as CandidateStatus;
    const orderDiff = (statusOrder[statusA] ?? 3) - (statusOrder[statusB] ?? 3);
    if (orderDiff !== 0) return orderDiff;

    // Sort by |r| descending
    const rA = a.cell.r != null ? Math.abs(a.cell.r) : -1;
    const rB = b.cell.r != null ? Math.abs(b.cell.r) : -1;
    return rB - rA;
  });

  return candidates.map(({ sensorId, cell }, idx): NormalizedCandidate => {
    const info = getSensorInfo(sensorId, lookups);
    const badges: Badge[] = [];
    const cellStatus = (cell.status ?? "ok") as CandidateStatus;

    // Correlation coefficient badge
    if (cell.r != null) {
      const rTone = cell.r > 0 ? "success" : cell.r < 0 ? "danger" : "neutral";
      badges.push(
        createBadge(
          "correlation",
          "r",
          formatScore(cell.r, 3),
          rTone,
          cell.r_ci_low != null && cell.r_ci_high != null
            ? `95% CI: [${formatScore(cell.r_ci_low, 3)}, ${formatScore(cell.r_ci_high, 3)}]`
            : undefined,
        ),
      );
    }

    // q-value badge
    if (cell.p_value != null) {
      badges.push(
        createBadge(
          "p_value",
          "p",
          cell.p_value < 0.001 ? "<.001" : formatScore(cell.p_value, 3),
          cell.p_value <= 0.05 ? "success" : "warning",
          "Per-test p-value (time-series adjusted)",
        ),
      );
    }

    if (cell.q_value != null) {
      badges.push(
        createBadge(
          "q_value",
          "q",
          cell.q_value < 0.001 ? "<.001" : formatScore(cell.q_value, 3),
          cell.q_value <= 0.05 ? "accent" : "warning",
          "FDR-adjusted p-value",
        ),
      );
    }

    // n badge
    badges.push(
      createBadge("overlap", "n", String(cell.n), "muted"),
    );

    // n_eff badge (if different from n)
    if (cell.n_eff != null && cell.n_eff !== cell.n) {
      badges.push(
        createBadge(
          "n_eff",
          "n_eff",
          formatNumber(cell.n_eff, { maximumFractionDigits: 0 }),
          "muted",
          "Effective sample size (autocorrelation-adjusted)",
        ),
      );
    }

    // Status badge for non-ok statuses
    if (cellStatus !== "ok") {
      const statusLabels: Record<CandidateStatus, string> = {
        ok: "OK",
        not_significant: "Not significant",
        insufficient_overlap: "Low overlap",
        not_computed: "Not computed",
      };
      const statusTones: Record<CandidateStatus, Badge["tone"]> = {
        ok: "success",
        not_significant: "warning",
        insufficient_overlap: "muted",
        not_computed: "muted",
      };
      badges.push(
        createBadge("status", "Status", statusLabels[cellStatus], statusTones[cellStatus]),
      );
    }

    // Use |r| as score for ranking
    const score = cell.r != null ? Math.abs(cell.r) : 0;

    return {
      sensor_id: sensorId,
      label: info.label,
      node_name: info.node_name,
      node_id: info.node_id,
      sensor_type: info.sensor_type,
      unit: info.unit,
      rank: idx + 1,
      score,
      score_label: cell.r != null ? formatScore(cell.r, 3) : "—",
      badges,
      strategy: "correlation",
      status: cellStatus,
      raw: { type: "correlation", data: { ...cell, sensor_id: sensorId } },
    };
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Events Normalizer
// ─────────────────────────────────────────────────────────────────────────────

export function normalizeEventsCandidates(
  result: EventMatchResultV1,
  lookups: LookupMaps,
): NormalizedCandidate[] {
  const { candidates } = result;

  // Guard against invalid result structure
  if (!candidates || !Array.isArray(candidates)) return [];

  // Sort by score descending
  const sorted = [...candidates].sort((a, b) => {
    const scoreA = a.score ?? -1;
    const scoreB = b.score ?? -1;
    return scoreB - scoreA;
  });

  return sorted.map((candidate, idx): NormalizedCandidate => {
    const info = getSensorInfo(candidate.sensor_id, lookups);
    const badges: Badge[] = [];

    // Score badge (F1 score)
    badges.push(
      createBadge("score", "F1", formatScore(candidate.score), "info"),
    );

    // Overlap badge
    badges.push(
      createBadge("overlap", "Overlap", String(candidate.overlap), "neutral"),
    );

    // Best lag badge
    const bestLag = candidate.best_lag?.lag_sec ?? 0;
    if (bestLag !== 0) {
      badges.push(
        createBadge(
          "lag",
          "Lag",
          formatLagSeconds(bestLag),
          bestLag > 0 ? "warning" : "accent",
        ),
      );
    }

    // Episodes badge
    const episodeCount = candidate.episodes?.length ?? 0;
    if (episodeCount > 0) {
      badges.push(
        createBadge("episodes", "Episodes", String(episodeCount), "muted"),
      );
    }

    return {
      sensor_id: candidate.sensor_id,
      label: info.label,
      node_name: info.node_name,
      node_id: info.node_id,
      sensor_type: info.sensor_type,
      unit: info.unit,
      rank: candidate.rank > 0 ? candidate.rank : idx + 1,
      score: candidate.score ?? 0,
      score_label: formatScore(candidate.score),
      badges,
      strategy: "events",
      status: "ok",
      raw: { type: "events", data: candidate },
    };
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Co-occurrence Normalizer
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Aggregate co-occurrence buckets into per-sensor scores
 *
 * From the spec:
 * - For each other sensor S != F in the bucket:
 *   - sensor_score[S] += |z_F| * |z_S|
 * - Rank by accumulated sensor_score descending
 * - Secondary sort: co_occurrence_count
 */
export function aggregateCooccurrenceBySensor(
  buckets: CooccurrenceBucketV1[],
  focusSensorId: string,
): CooccurrenceAggregatedSensor[] {
  const sensorScores = new Map<
    string,
    {
      score: number;
      count: number;
      maxZ: number;
      zSum: number;
      timestamps: number[];
    }
  >();

  for (const bucket of buckets) {
    // Find focus sensor in bucket
    const focusEntry = bucket.sensors.find((s) => s.sensor_id === focusSensorId);
    if (!focusEntry) continue;

    const focusZ = Math.abs(focusEntry.z);

    // Aggregate other sensors
    for (const sensor of bucket.sensors) {
      if (sensor.sensor_id === focusSensorId) continue;

      const existing = sensorScores.get(sensor.sensor_id) ?? {
        score: 0,
        count: 0,
        maxZ: 0,
        zSum: 0,
        timestamps: [],
      };

      const sensorZ = Math.abs(sensor.z);
      existing.score += focusZ * sensorZ;
      existing.count += 1;
      existing.maxZ = Math.max(existing.maxZ, sensorZ);
      existing.zSum += sensorZ;
      existing.timestamps.push(bucket.ts);

      sensorScores.set(sensor.sensor_id, existing);
    }
  }

  // Convert to array and sort
  const aggregated: CooccurrenceAggregatedSensor[] = [];

  for (const [sensorId, data] of sensorScores) {
    aggregated.push({
      sensor_id: sensorId,
      score: data.score,
      co_occurrence_count: data.count,
      max_bucket_z: data.maxZ,
      avg_bucket_z: data.count > 0 ? data.zSum / data.count : 0,
      total_severity_contribution: data.score,
      top_bucket_timestamps: data.timestamps
        .slice()
        .sort((a, b) => b - a)
        .slice(0, 10),
    });
  }

  // Sort by score descending, then by count
  aggregated.sort((a, b) => {
    const scoreDiff = b.score - a.score;
    if (scoreDiff !== 0) return scoreDiff;
    return b.co_occurrence_count - a.co_occurrence_count;
  });

  return aggregated;
}

export function normalizeCooccurrenceCandidates(
  result: CooccurrenceResultV1,
  focusSensorId: string,
  lookups: LookupMaps,
): NormalizedCandidate[] {
  // Guard against invalid result structure
  if (!result.buckets || !Array.isArray(result.buckets)) return [];

  // Aggregate buckets by sensor
  const aggregated = aggregateCooccurrenceBySensor(result.buckets, focusSensorId);

  return aggregated.map((sensor, idx): NormalizedCandidate => {
    const info = getSensorInfo(sensor.sensor_id, lookups);
    const badges: Badge[] = [];

    // Score badge
    badges.push(
      createBadge(
        "score",
        "Score",
        formatNumber(sensor.score, { maximumFractionDigits: 1 }),
        "info",
        "Accumulated |z_focus| × |z_sensor| across co-occurrences",
      ),
    );

    // Count badge
    badges.push(
      createBadge(
        "overlap",
        "Count",
        String(sensor.co_occurrence_count),
        "neutral",
        "Number of co-occurrence buckets",
      ),
    );

    // Max z badge
    if (sensor.max_bucket_z > 0) {
      badges.push(
        createBadge(
          "p_value",
          "Max z",
          formatScore(sensor.max_bucket_z, 1),
          sensor.max_bucket_z >= 4 ? "danger" : sensor.max_bucket_z >= 3 ? "warning" : "muted",
          "Maximum z-score in any co-occurrence bucket",
        ),
      );
    }

    return {
      sensor_id: sensor.sensor_id,
      label: info.label,
      node_name: info.node_name,
      node_id: info.node_id,
      sensor_type: info.sensor_type,
      unit: info.unit,
      rank: idx + 1,
      score: sensor.score,
      score_label: formatNumber(sensor.score, { maximumFractionDigits: 1 }),
      badges,
      strategy: "cooccurrence",
      status: "ok",
      raw: { type: "cooccurrence", data: sensor },
    };
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Create Lookup Maps Helper
// ─────────────────────────────────────────────────────────────────────────────

export function createLookupMaps(
  sensors: DemoSensor[],
  nodesById: Map<string, DemoNode>,
  labelMap: Map<string, string>,
): LookupMaps {
  return {
    sensorsById: new Map(sensors.map((s) => [s.sensor_id, s])),
    nodesById,
    labelMap,
  };
}
