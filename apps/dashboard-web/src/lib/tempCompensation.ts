import type { TrendSeriesEntry } from "@/types/dashboard";

export type DriftModelDegree = 1 | 2 | 3;

export type TempCompensationFitResult = {
  degree: DriftModelDegree;
  centerTemp: number;
  /**
   * Center timestamp for the optional time detrending term (milliseconds since epoch).
   * When a time slope is enabled, we model `raw ≈ f(temp) + slope * timeDays`, where
   * `timeDays = (tsMs - centerTimeMs) / 86400_000`.
   */
  centerTimeMs: number;
  /** Polynomial coefficients for raw ≈ b0 + b1*x + b2*x^2 + b3*x^3, where x = temp - centerTemp. */
  coefficients: number[];
  /**
   * Optional linear trend term in raw-units per day. This is included in the fit to avoid
   * biasing the temperature coefficients when the raw value also drifts slowly over time.
   *
   * IMPORTANT: the derived-sensor expression uses only the temperature drift term
   * (`coefficients[1..]`); it does not apply this time trend.
   */
  timeSlopePerDay: number | null;
  r2: number | null;
  sampleCount: number;
  tempMin: number;
  tempMax: number;
  rawMin: number;
  rawMax: number;
};

export type AlignedTempCompensationPoint = {
  timestamp: Date;
  temperature: number;
  raw: number;
};

function isFiniteNumber(value: unknown): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

export function alignSeriesByTimestamp(
  rawSeries: TrendSeriesEntry | null | undefined,
  temperatureSeries: TrendSeriesEntry | null | undefined,
  options?: { temperatureLagSeconds?: number },
): AlignedTempCompensationPoint[] {
  if (!rawSeries || !temperatureSeries) return [];

  const lagSeconds = options?.temperatureLagSeconds;
  const lagMs = isFiniteNumber(lagSeconds) ? Math.round(lagSeconds * 1000) : 0;

  const temperatureByMs = new Map<number, number>();
  for (const pt of temperatureSeries.points ?? []) {
    const temperature = pt?.value;
    if (!isFiniteNumber(temperature)) continue;
    const ms = pt.timestamp instanceof Date ? pt.timestamp.getTime() : new Date(pt.timestamp as never).getTime();
    if (!Number.isFinite(ms)) continue;
    temperatureByMs.set(ms, temperature);
  }

  const out: AlignedTempCompensationPoint[] = [];
  for (const pt of rawSeries.points ?? []) {
    const raw = pt?.value;
    if (!isFiniteNumber(raw)) continue;
    const ms = pt.timestamp instanceof Date ? pt.timestamp.getTime() : new Date(pt.timestamp as never).getTime();
    if (!Number.isFinite(ms)) continue;
    // Positive lagSeconds means we use temperature from the past (raw[t] aligns to temp[t - lag]).
    const temperature = temperatureByMs.get(ms - lagMs);
    if (!isFiniteNumber(temperature)) continue;
    out.push({ timestamp: new Date(ms), temperature, raw });
  }

  return out;
}

function percentile(values: number[], p: number): number | null {
  if (!values.length) return null;
  const clamped = Math.max(0, Math.min(1, p));
  const sorted = values.slice().sort((a, b) => a - b);
  const k = (sorted.length - 1) * clamped;
  const f = Math.floor(k);
  const c = Math.ceil(k);
  if (f === c) return sorted[f] ?? null;
  const d = k - f;
  const lo = sorted[f] ?? null;
  const hi = sorted[c] ?? null;
  if (lo == null || hi == null) return null;
  return lo * (1 - d) + hi * d;
}

function p95p5Swing(values: number[]): number | null {
  const finite = values.filter((v) => isFiniteNumber(v));
  if (finite.length < 2) return null;
  const p5 = percentile(finite, 0.05);
  const p95 = percentile(finite, 0.95);
  if (!isFiniteNumber(p5) || !isFiniteNumber(p95)) return null;
  return p95 - p5;
}

export type TempLagCandidateScore = {
  lagSeconds: number;
  sampleCount: number;
  rawSwingP95P5: number | null;
  correctedSwingP95P5: number | null;
  reductionPct: number | null;
  r2: number | null;
};

export function suggestTemperatureLagSeconds({
  rawSeries,
  temperatureSeries,
  intervalSeconds,
  degree,
  centerTemp,
  clampAbs,
  includeTimeSlope,
  maxLagSeconds = 21_600, // 6h
  minSamples = 20,
  minImprovementPct = 5,
}: {
  rawSeries: TrendSeriesEntry | null | undefined;
  temperatureSeries: TrendSeriesEntry | null | undefined;
  intervalSeconds: number;
  degree: DriftModelDegree;
  centerTemp?: number;
  clampAbs?: number | null;
  includeTimeSlope?: boolean;
  maxLagSeconds?: number;
  minSamples?: number;
  minImprovementPct?: number;
}): { lagSeconds: number; baseline: TempLagCandidateScore | null; best: TempLagCandidateScore | null } {
  const stepSeconds = (() => {
    const base = Math.max(1, Math.floor(intervalSeconds));
    const minStep = 300; // keep the scan reasonably cheap even at 30s/60s intervals
    const mult = Math.max(1, Math.ceil(minStep / base));
    return mult * base;
  })();

  const clampCorrectionAbs = isFiniteNumber(clampAbs) && clampAbs > 0 ? clampAbs : null;

  const candidates: TempLagCandidateScore[] = [];
  for (let lag = 0; lag <= maxLagSeconds; lag += stepSeconds) {
    const points = alignSeriesByTimestamp(rawSeries, temperatureSeries, { temperatureLagSeconds: lag });
    if (points.length < minSamples) continue;
    const fit = fitTemperatureDriftModel({ points, degree, centerTemp, includeTimeSlope });
    if (!fit) continue;
    const rawValues = points.map((pt) => pt.raw).filter(isFiniteNumber);
    const correctedValues = points
      .map((pt) => {
        if (!isFiniteNumber(pt.raw) || !isFiniteNumber(pt.temperature)) return null;
        const correction = computeTemperatureDriftCorrection(pt.temperature, fit);
        if (!isFiniteNumber(correction)) return null;
        const clamped =
          clampCorrectionAbs != null
            ? Math.max(-clampCorrectionAbs, Math.min(clampCorrectionAbs, correction))
            : correction;
        const corrected = pt.raw - clamped;
        return Number.isFinite(corrected) ? corrected : null;
      })
      .filter(isFiniteNumber);

    const rawSwing = p95p5Swing(rawValues);
    const correctedSwing = p95p5Swing(correctedValues);
    const reductionPct =
      rawSwing != null && correctedSwing != null && rawSwing > 0
        ? Math.max(0, Math.min(100, (1 - correctedSwing / rawSwing) * 100))
        : null;

    const score: TempLagCandidateScore = {
      lagSeconds: lag,
      sampleCount: points.length,
      rawSwingP95P5: rawSwing,
      correctedSwingP95P5: correctedSwing,
      reductionPct,
      r2: fit.r2,
    };
    candidates.push(score);
  }

  const baseline = candidates.find((c) => c.lagSeconds === 0) ?? null;
  const best =
    candidates.reduce<TempLagCandidateScore | null>((acc, cur) => {
      const curScore = cur.reductionPct ?? -Infinity;
      const accScore = acc?.reductionPct ?? -Infinity;
      if (curScore > accScore) return cur;
      if (curScore === accScore && acc && cur.lagSeconds < acc.lagSeconds) return cur;
      return acc;
    }, null) ?? null;

  if (!best) {
    return { lagSeconds: 0, baseline, best: null };
  }

  const baselinePct = baseline?.reductionPct;
  const bestPct = best?.reductionPct;
  if (baselinePct == null || bestPct == null) {
    return { lagSeconds: 0, baseline, best };
  }
  if (bestPct - baselinePct < minImprovementPct) {
    return { lagSeconds: 0, baseline, best };
  }
  return { lagSeconds: best.lagSeconds, baseline, best };
}

function solveLinearSystemGaussian(
  a: number[][],
  b: number[],
): number[] | null {
  const n = a.length;
  if (n === 0) return null;
  if (b.length !== n) return null;
  if (a.some((row) => row.length !== n)) return null;

  const m = a.map((row, i) => [...row, b[i]]);

  for (let col = 0; col < n; col += 1) {
    let pivotRow = col;
    let pivotAbs = Math.abs(m[col]?.[col] ?? 0);
    for (let row = col + 1; row < n; row += 1) {
      const candidateAbs = Math.abs(m[row]?.[col] ?? 0);
      if (candidateAbs > pivotAbs) {
        pivotAbs = candidateAbs;
        pivotRow = row;
      }
    }

    if (!Number.isFinite(pivotAbs) || pivotAbs < 1e-12) return null;
    if (pivotRow !== col) {
      const tmp = m[col];
      m[col] = m[pivotRow];
      m[pivotRow] = tmp;
    }

    const pivot = m[col][col];
    for (let j = col; j <= n; j += 1) {
      m[col][j] /= pivot;
    }

    for (let row = 0; row < n; row += 1) {
      if (row === col) continue;
      const factor = m[row][col];
      if (!Number.isFinite(factor) || factor === 0) continue;
      for (let j = col; j <= n; j += 1) {
        m[row][j] -= factor * m[col][j];
      }
    }
  }

  return m.map((row) => row[n]);
}

function computeR2(y: number[], yHat: number[]): number | null {
  if (y.length !== yHat.length || y.length < 2) return null;
  const mean = y.reduce((acc, v) => acc + v, 0) / y.length;
  let sse = 0;
  let sst = 0;
  for (let i = 0; i < y.length; i += 1) {
    const err = y[i] - yHat[i];
    sse += err * err;
    const d = y[i] - mean;
    sst += d * d;
  }
  if (!Number.isFinite(sse) || !Number.isFinite(sst) || sst <= 0) return null;
  const r2 = 1 - sse / sst;
  if (!Number.isFinite(r2)) return null;
  return Math.max(0, Math.min(1, r2));
}

export function fitTemperatureDriftModel({
  points,
  degree,
  centerTemp,
  includeTimeSlope,
}: {
  points: AlignedTempCompensationPoint[];
  degree: DriftModelDegree;
  centerTemp?: number;
  includeTimeSlope?: boolean;
}): TempCompensationFitResult | null {
  const finitePoints = points.filter((pt) => isFiniteNumber(pt.temperature) && isFiniteNumber(pt.raw));
  if (finitePoints.length < Math.max(10, degree + 2)) return null;

  const temps = finitePoints.map((pt) => pt.temperature);
  const raws = finitePoints.map((pt) => pt.raw);
  const tsMs = finitePoints.map((pt) => pt.timestamp.getTime()).filter((v) => Number.isFinite(v));
  const centerTimeMs =
    tsMs.length > 0 ? tsMs.reduce((acc, v) => acc + v, 0) / tsMs.length : new Date().getTime();
  if (!Number.isFinite(centerTimeMs)) return null;

  const tMin = Math.min(...temps);
  const tMax = Math.max(...temps);
  const yMin = Math.min(...raws);
  const yMax = Math.max(...raws);

  const center =
    isFiniteNumber(centerTemp) ? centerTemp : temps.reduce((acc, v) => acc + v, 0) / temps.length;
  if (!Number.isFinite(center)) return null;

  const useTimeSlope = Boolean(includeTimeSlope);
  const dim = degree + 1 + (useTimeSlope ? 1 : 0);
  const xtx: number[][] = Array.from({ length: dim }, () => Array.from({ length: dim }, () => 0));
  const xty: number[] = Array.from({ length: dim }, () => 0);

  for (let i = 0; i < finitePoints.length; i += 1) {
    const x = finitePoints[i].temperature - center;
    const y = finitePoints[i].raw;
    const powers: number[] = [1];
    for (let p = 1; p <= degree; p += 1) {
      powers.push(powers[p - 1] * x);
    }
    if (useTimeSlope) {
      const timeDays = (finitePoints[i].timestamp.getTime() - centerTimeMs) / 86_400_000;
      powers.push(timeDays);
    }

    for (let r = 0; r < dim; r += 1) {
      xty[r] += powers[r] * y;
      for (let c = 0; c < dim; c += 1) {
        xtx[r][c] += powers[r] * powers[c];
      }
    }
  }

  const beta = solveLinearSystemGaussian(xtx, xty);
  if (!beta || beta.length !== dim) return null;
  if (beta.some((v) => !Number.isFinite(v))) return null;

  const yHat = finitePoints.map((pt) => {
    const x = pt.temperature - center;
    let pred = beta[0] ?? 0;
    let xPow = x;
    for (let k = 1; k < degree + 1; k += 1) {
      pred += (beta[k] ?? 0) * xPow;
      xPow *= x;
    }
    if (useTimeSlope) {
      const timeDays = (pt.timestamp.getTime() - centerTimeMs) / 86_400_000;
      pred += (beta[dim - 1] ?? 0) * timeDays;
    }
    return pred;
  });
  const r2 = computeR2(raws, yHat);

  const coefficients = beta.slice(0, degree + 1);
  const timeSlopePerDay = useTimeSlope ? (beta[dim - 1] ?? null) : null;

  return {
    degree,
    centerTemp: center,
    centerTimeMs,
    coefficients,
    timeSlopePerDay,
    r2,
    sampleCount: finitePoints.length,
    tempMin: tMin,
    tempMax: tMax,
    rawMin: yMin,
    rawMax: yMax,
  };
}

export function computeTemperatureDriftCorrection(
  temperature: number,
  fit: Pick<TempCompensationFitResult, "centerTemp" | "coefficients">,
): number | null {
  if (!isFiniteNumber(temperature)) return null;
  const center = fit.centerTemp;
  const coefficients = fit.coefficients;
  if (!isFiniteNumber(center) || !Array.isArray(coefficients) || coefficients.length < 2) return null;

  const x = temperature - center;
  let correction = 0;
  let xPow = x;
  for (let k = 1; k < coefficients.length; k += 1) {
    const c = coefficients[k];
    if (!isFiniteNumber(c)) return null;
    correction += c * xPow;
    xPow *= x;
  }
  if (!Number.isFinite(correction)) return null;
  return correction;
}

export function applyTemperatureCompensation(
  raw: number,
  temperature: number,
  fit: Pick<TempCompensationFitResult, "centerTemp" | "coefficients">,
): number | null {
  if (!isFiniteNumber(raw)) return null;
  const correction = computeTemperatureDriftCorrection(temperature, fit);
  if (!isFiniteNumber(correction)) return null;
  const corrected = raw - correction;
  if (!Number.isFinite(corrected)) return null;
  return corrected;
}

export function buildTempCompensationExpression({
  rawVar,
  temperatureVar,
  centerTemp,
  coefficients,
  clampAbs,
}: {
  rawVar: string;
  temperatureVar: string;
  centerTemp: number;
  coefficients: number[];
  clampAbs?: number | null;
}): { expression: string; driftOnlyExpression: string } | null {
  const degree = Math.max(0, coefficients.length - 1);
  if (degree < 1) return null;
  if (!isFiniteNumber(centerTemp)) return null;

  const x = `(${temperatureVar} - ${centerTemp})`;
  const terms: string[] = [];
  for (let k = 1; k < coefficients.length; k += 1) {
    const c = coefficients[k];
    if (!isFiniteNumber(c)) return null;
    const pow = k === 1 ? x : `pow(${x}, ${k})`;
    terms.push(`${c} * ${pow}`);
  }
  const drift = terms.length === 1 ? terms[0] : `(${terms.join(" + ")})`;
  const driftClamped =
    isFiniteNumber(clampAbs) && clampAbs > 0 ? `clamp(${drift}, -${clampAbs}, ${clampAbs})` : drift;
  const expression = `${rawVar} - ${driftClamped}`;

  return { expression, driftOnlyExpression: driftClamped };
}
