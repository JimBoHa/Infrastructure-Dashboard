import type { TsseEpisodeV1 } from "@/types/analysis";

function asFiniteNumber(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

/**
 * Pick a "representative" default episode for preview:
 * - Prefer higher coverage (more of the window explained)
 * - Then prefer more points (denser evidence)
 * - Then prefer higher peak score (sharper match)
 * - Stable tie-break by earliest start timestamp
 */
export function pickRepresentativeEpisodeIndex(
  episodes: TsseEpisodeV1[] | null | undefined,
): number {
  if (!episodes || episodes.length === 0) return 0;

  let bestIdx = 0;
  let bestCoverage = asFiniteNumber(episodes[0]?.coverage);
  let bestPoints = asFiniteNumber(episodes[0]?.num_points);
  let bestPeak = asFiniteNumber(episodes[0]?.score_peak);
  let bestStartTs = String(episodes[0]?.start_ts ?? "");

  for (let idx = 1; idx < episodes.length; idx++) {
    const episode = episodes[idx];
    if (!episode) continue;

    const coverage = asFiniteNumber(episode.coverage);
    const points = asFiniteNumber(episode.num_points);
    const peak = asFiniteNumber(episode.score_peak);
    const startTs = String(episode.start_ts ?? "");

    if (coverage !== bestCoverage) {
      if (coverage > bestCoverage) {
        bestIdx = idx;
        bestCoverage = coverage;
        bestPoints = points;
        bestPeak = peak;
        bestStartTs = startTs;
      }
      continue;
    }

    if (points !== bestPoints) {
      if (points > bestPoints) {
        bestIdx = idx;
        bestCoverage = coverage;
        bestPoints = points;
        bestPeak = peak;
        bestStartTs = startTs;
      }
      continue;
    }

    if (peak !== bestPeak) {
      if (peak > bestPeak) {
        bestIdx = idx;
        bestCoverage = coverage;
        bestPoints = points;
        bestPeak = peak;
        bestStartTs = startTs;
      }
      continue;
    }

    if (startTs && (!bestStartTs || startTs < bestStartTs)) {
      bestIdx = idx;
      bestCoverage = coverage;
      bestPoints = points;
      bestPeak = peak;
      bestStartTs = startTs;
    }
  }

  return bestIdx;
}

