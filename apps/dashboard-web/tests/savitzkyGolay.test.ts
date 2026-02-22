import { savitzkyGolayFilter, validateSavitzkyGolayOptions } from "@/lib/savitzkyGolay";

describe("savitzkyGolay", () => {
  it("validates basic constraints", () => {
    expect(validateSavitzkyGolayOptions({ windowLength: 4, polyOrder: 2 }).ok).toBe(false);
    expect(validateSavitzkyGolayOptions({ windowLength: 5, polyOrder: 5 }).ok).toBe(false);
    expect(validateSavitzkyGolayOptions({ windowLength: 5, polyOrder: 2, derivOrder: 3 }).ok).toBe(false);
    expect(validateSavitzkyGolayOptions({ windowLength: 5, polyOrder: 2, delta: 0 }).ok).toBe(false);
    expect(validateSavitzkyGolayOptions({ windowLength: 5, polyOrder: 2 }).ok).toBe(true);
  });

  it("preserves polynomials up to polyOrder (deriv=0)", () => {
    // y = x^2 + 2x + 3
    const values = Array.from({ length: 21 }, (_, idx) => {
      const x = idx - 10;
      return x * x + 2 * x + 3;
    });
    const filtered = savitzkyGolayFilter(values, {
      windowLength: 7,
      polyOrder: 2,
      derivOrder: 0,
      edgeMode: "interp",
      delta: 1,
    });
    expect(filtered).toHaveLength(values.length);
    filtered.forEach((v, idx) => expect(v).toBeCloseTo(values[idx]!, 10));
  });

  it("computes exact derivatives for polynomials (deriv=1)", () => {
    // y = 3x + 5 => dy/dx = 3
    const values = Array.from({ length: 25 }, (_, idx) => {
      const x = idx - 12;
      return 3 * x + 5;
    });
    const filtered = savitzkyGolayFilter(values, {
      windowLength: 9,
      polyOrder: 3,
      derivOrder: 1,
      edgeMode: "interp",
      delta: 1,
    });
    filtered.forEach((v) => expect(v).toBeCloseTo(3, 10));
  });

  it("does not smooth across null gaps", () => {
    const values: Array<number | null> = [1, 2, 3, null, 10, 11, 12];
    const filtered = savitzkyGolayFilter(values, {
      windowLength: 3,
      polyOrder: 1,
      derivOrder: 0,
      edgeMode: "interp",
      delta: 1,
    });
    expect(filtered[3]).toBeNull();
    // Both segments are linear and length=3, so they should remain unchanged.
    filtered.slice(0, 3).forEach((value, idx) => expect(value).toBeCloseTo([1, 2, 3][idx]!, 10));
    filtered.slice(4).forEach((value, idx) => expect(value).toBeCloseTo([10, 11, 12][idx]!, 10));
  });
});
