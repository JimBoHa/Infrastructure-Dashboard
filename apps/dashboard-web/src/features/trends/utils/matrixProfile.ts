"use client";

const EPS = 1e-12;

function clampInt(value: number, min: number, max: number): number {
  const v = Number.isFinite(value) ? Math.floor(value) : min;
  return Math.max(min, Math.min(max, v));
}

type MatrixProfileParams = {
  values: number[];
  window: number;
  exclusionZone?: number;
};

export type MatrixProfileResult = {
  window: number;
  exclusionZone: number;
  profile: number[];
  profileIndex: number[];
};

export function computeMatrixProfile(params: MatrixProfileParams): MatrixProfileResult {
  const values = params.values;
  const n = values.length;
  const window = clampInt(params.window, 4, Math.max(4, n));
  const k = n - window + 1;
  const exclusionZone = clampInt(params.exclusionZone ?? Math.floor(window / 2), 0, Math.max(0, k - 1));

  if (k <= 1) {
    return {
      window,
      exclusionZone,
      profile: [],
      profileIndex: [],
    };
  }

  const prefix = new Float64Array(n + 1);
  const prefixSq = new Float64Array(n + 1);
  for (let i = 0; i < n; i += 1) {
    const v = values[i] ?? 0;
    prefix[i + 1] = prefix[i] + v;
    prefixSq[i + 1] = prefixSq[i] + v * v;
  }

  const normalized = new Float32Array(k * window);
  const constant = new Uint8Array(k);

  for (let start = 0; start < k; start += 1) {
    const sum = prefix[start + window] - prefix[start];
    const sumSq = prefixSq[start + window] - prefixSq[start];
    const mean = sum / window;
    const variance = Math.max(0, sumSq / window - mean * mean);
    const std = Math.sqrt(variance);
    const inv = std > EPS ? 1 / std : 0;
    if (inv === 0) constant[start] = 1;

    const base = start * window;
    for (let t = 0; t < window; t += 1) {
      normalized[base + t] = (values[start + t]! - mean) * inv;
    }
  }

  const profile = Array.from({ length: k }, () => Number.POSITIVE_INFINITY);
  const profileIndex = Array.from({ length: k }, () => -1);

  const dot = (i: number, j: number) => {
    const baseI = i * window;
    const baseJ = j * window;
    let sum = 0;
    for (let t = 0; t < window; t += 1) {
      sum += normalized[baseI + t]! * normalized[baseJ + t]!;
    }
    return sum;
  };

  for (let i = 0; i < k; i += 1) {
    for (let j = i + 1; j < k; j += 1) {
      if (Math.abs(i - j) <= exclusionZone) continue;

      let dist: number;
      const iConst = constant[i] === 1;
      const jConst = constant[j] === 1;

      if (iConst && jConst) {
        dist = 0;
      } else if (iConst !== jConst) {
        dist = Math.sqrt(window);
      } else {
        const corr = dot(i, j) / window;
        dist = Math.sqrt(Math.max(0, 2 * window * (1 - corr)));
      }

      if (dist < profile[i]!) {
        profile[i] = dist;
        profileIndex[i] = j;
      }
      if (dist < profile[j]!) {
        profile[j] = dist;
        profileIndex[j] = i;
      }
    }
  }

  return { window, exclusionZone, profile, profileIndex };
}

