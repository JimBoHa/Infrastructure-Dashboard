import { describe, expect, it } from "vitest";
import { pickRepresentativeEpisodeIndex } from "@/features/trends/utils/episodeSelection";
import type { TsseEpisodeV1 } from "@/types/analysis";

function ep(overrides: Partial<TsseEpisodeV1>): TsseEpisodeV1 {
  return {
    start_ts: "2026-01-01T00:00:00Z",
    end_ts: "2026-01-01T01:00:00Z",
    window_sec: 3600,
    lag_sec: 0,
    lag_iqr_sec: 0,
    score_mean: 0,
    score_peak: 0,
    coverage: 0,
    num_points: 0,
    ...overrides,
  };
}

describe("pickRepresentativeEpisodeIndex", () => {
  it("returns 0 for empty list", () => {
    expect(pickRepresentativeEpisodeIndex([])).toBe(0);
    expect(pickRepresentativeEpisodeIndex(null)).toBe(0);
    expect(pickRepresentativeEpisodeIndex(undefined)).toBe(0);
  });

  it("prefers higher coverage", () => {
    const episodes = [ep({ coverage: 0.1 }), ep({ coverage: 0.25 }), ep({ coverage: 0.2 })];
    expect(pickRepresentativeEpisodeIndex(episodes)).toBe(1);
  });

  it("breaks ties by num_points", () => {
    const episodes = [
      ep({ coverage: 0.2, num_points: 5, score_peak: 0.99 }),
      ep({ coverage: 0.2, num_points: 12, score_peak: 0.8 }),
      ep({ coverage: 0.2, num_points: 7, score_peak: 1.0 }),
    ];
    expect(pickRepresentativeEpisodeIndex(episodes)).toBe(1);
  });

  it("breaks ties by score_peak", () => {
    const episodes = [
      ep({ coverage: 0.2, num_points: 10, score_peak: 0.9 }),
      ep({ coverage: 0.2, num_points: 10, score_peak: 0.95 }),
    ];
    expect(pickRepresentativeEpisodeIndex(episodes)).toBe(1);
  });

  it("uses stable tie-breaker (earliest start_ts)", () => {
    const episodes = [
      ep({ start_ts: "2026-01-02T00:00:00Z", coverage: 0.2, num_points: 10, score_peak: 0.9 }),
      ep({ start_ts: "2026-01-01T00:00:00Z", coverage: 0.2, num_points: 10, score_peak: 0.9 }),
    ];
    expect(pickRepresentativeEpisodeIndex(episodes)).toBe(1);
  });
});

