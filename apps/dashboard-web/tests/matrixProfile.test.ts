import { computeMatrixProfile } from "@/features/trends/utils/matrixProfile";

function argMax(values: number[]): { idx: number; value: number } | null {
  let bestIdx = -1;
  let best = Number.NEGATIVE_INFINITY;
  for (let i = 0; i < values.length; i += 1) {
    const v = values[i] ?? Number.NEGATIVE_INFINITY;
    if (Number.isFinite(v) && v > best) {
      best = v;
      bestIdx = i;
    }
  }
  if (bestIdx < 0 || !Number.isFinite(best)) return null;
  return { idx: bestIdx, value: best };
}

describe("matrix profile", () => {
  it("returns empty arrays when there are not enough points", () => {
    const result = computeMatrixProfile({ values: [1, 2, 3, 4, 5], window: 10 });
    expect(result.profile).toEqual([]);
    expect(result.profileIndex).toEqual([]);
  });

  it("produces a zero-distance profile for constant series", () => {
    const values = Array.from({ length: 50 }, () => 7);
    const result = computeMatrixProfile({ values, window: 10 });

    expect(result.profile).toHaveLength(values.length - result.window + 1);
    expect(result.profileIndex).toHaveLength(values.length - result.window + 1);
    expect(result.profile.every((v) => v === 0)).toBe(true);
    expect(result.profileIndex.every((idx) => idx >= 0)).toBe(true);
  });

  it("surfaces anomaly windows as large matrix profile distances", () => {
    const motif = Array.from({ length: 20 }, (_, i) => Math.sin((2 * Math.PI * i) / 20));
    const anomaly = Array.from({ length: 20 }, (_, i) => (i - 10) / 5);
    const values = [
      ...motif,
      ...motif,
      ...motif,
      ...anomaly,
      ...motif,
      ...motif,
      ...motif,
    ];

    const window = 10;
    const result = computeMatrixProfile({ values, window });
    expect(result.profile).toHaveLength(values.length - result.window + 1);

    const max = argMax(result.profile);
    expect(max).not.toBeNull();
    expect((max?.value as number) > 2.5).toBe(true);

    // Anomaly segment begins after 3 motifs (index 60). Allow some overlap at edges.
    expect(max!.idx).toBeGreaterThanOrEqual(45);
    expect(max!.idx).toBeLessThanOrEqual(95);
  });
});

