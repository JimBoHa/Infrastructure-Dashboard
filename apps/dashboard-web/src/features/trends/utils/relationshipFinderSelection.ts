import type { NormalizedCandidate } from "../types/relationshipFinder";

export function pickStableCandidateId({
  previousId,
  candidates,
}: {
  previousId: string | null;
  candidates: NormalizedCandidate[];
}): string | null {
  if (candidates.length === 0) return null;
  if (previousId && candidates.some((c) => c.sensor_id === previousId)) return previousId;
  return candidates[0]!.sensor_id;
}

