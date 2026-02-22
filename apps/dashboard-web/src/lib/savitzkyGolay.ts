export type SavitzkyGolayEdgeMode = "interp" | "nearest" | "mirror" | "clip";

export type SavitzkyGolayOptions = {
  windowLength: number;
  polyOrder: number;
  derivOrder?: number;
  delta?: number;
  edgeMode?: SavitzkyGolayEdgeMode;
};

export type SavitzkyGolayValidation = {
  ok: boolean;
  error?: string;
};

function factorial(value: number): number {
  let result = 1;
  for (let i = 2; i <= value; i += 1) result *= i;
  return result;
}

function invertSquareMatrix(matrix: number[][]): number[][] | null {
  const n = matrix.length;
  if (!n) return null;
  if (!matrix.every((row) => row.length === n)) return null;

  const augmented: number[][] = matrix.map((row, rowIndex) => {
    const identity = Array.from({ length: n }, (_, colIndex) => (colIndex === rowIndex ? 1 : 0));
    return [...row, ...identity];
  });

  for (let col = 0; col < n; col += 1) {
    let pivotRow = col;
    let pivotMagnitude = Math.abs(augmented[col]?.[col] ?? 0);

    for (let row = col + 1; row < n; row += 1) {
      const magnitude = Math.abs(augmented[row]?.[col] ?? 0);
      if (magnitude > pivotMagnitude) {
        pivotMagnitude = magnitude;
        pivotRow = row;
      }
    }

    const pivotValue = augmented[pivotRow]?.[col] ?? 0;
    if (!Number.isFinite(pivotValue) || Math.abs(pivotValue) < 1e-12) return null;

    if (pivotRow !== col) {
      const tmp = augmented[col];
      augmented[col] = augmented[pivotRow];
      augmented[pivotRow] = tmp;
    }

    const denom = augmented[col]?.[col] ?? 1;
    for (let j = 0; j < 2 * n; j += 1) {
      augmented[col]![j] = (augmented[col]![j] ?? 0) / denom;
    }

    for (let row = 0; row < n; row += 1) {
      if (row === col) continue;
      const factor = augmented[row]?.[col] ?? 0;
      if (!factor) continue;
      for (let j = 0; j < 2 * n; j += 1) {
        augmented[row]![j] = (augmented[row]![j] ?? 0) - factor * (augmented[col]![j] ?? 0);
      }
    }
  }

  return augmented.map((row) => row.slice(n, 2 * n));
}

export function validateSavitzkyGolayOptions(options: SavitzkyGolayOptions): SavitzkyGolayValidation {
  const windowLength = Math.floor(options.windowLength);
  const polyOrder = Math.floor(options.polyOrder);
  const derivOrder = Math.floor(options.derivOrder ?? 0);
  const delta = options.delta ?? 1;

  if (!Number.isFinite(windowLength) || windowLength < 3) {
    return { ok: false, error: "Window length must be an odd integer ≥ 3." };
  }
  if (windowLength % 2 === 0) {
    return { ok: false, error: "Window length must be odd (e.g. 5, 7, 9)." };
  }
  if (!Number.isFinite(polyOrder) || polyOrder < 0) {
    return { ok: false, error: "Polynomial degree must be a non-negative integer." };
  }
  if (polyOrder >= windowLength) {
    return { ok: false, error: "Polynomial degree must be smaller than the window length." };
  }
  if (!Number.isFinite(derivOrder) || derivOrder < 0) {
    return { ok: false, error: "Derivative order must be a non-negative integer." };
  }
  if (derivOrder > polyOrder) {
    return { ok: false, error: "Derivative order must be ≤ the polynomial degree." };
  }
  if (!Number.isFinite(delta) || delta <= 0) {
    return { ok: false, error: "Sample spacing (Δt) must be a positive number." };
  }
  return { ok: true };
}

function reflectIndex(index: number, length: number): number {
  let idx = index;
  while (idx < 0 || idx >= length) {
    if (idx < 0) idx = -idx;
    if (idx >= length) idx = 2 * length - idx - 2;
  }
  return idx;
}

function buildSavitzkyGolayCoefficients(
  windowLength: number,
  polyOrder: number,
  derivOrder: number,
  delta: number,
  position: number,
): number[] | null {
  const degree = polyOrder;
  const m = degree + 1;

  const ata: number[][] = Array.from({ length: m }, () => Array.from({ length: m }, () => 0));

  // Compute A^T A where A[i][j] = (x_i)^j and x_i = i - position.
  const xPowersPerRow: number[][] = [];
  for (let i = 0; i < windowLength; i += 1) {
    const x = i - position;
    const powers: number[] = Array.from({ length: m }, () => 1);
    for (let j = 1; j < m; j += 1) powers[j] = powers[j - 1]! * x;
    xPowersPerRow.push(powers);
    for (let r = 0; r < m; r += 1) {
      for (let c = 0; c < m; c += 1) {
        ata[r]![c] += (powers[r] ?? 0) * (powers[c] ?? 0);
      }
    }
  }

  const ataInv = invertSquareMatrix(ata);
  if (!ataInv) return null;
  const row = ataInv[derivOrder];
  if (!row) return null;

  const coefficients: number[] = [];
  const scale = factorial(derivOrder) / Math.pow(delta, derivOrder);
  for (let i = 0; i < windowLength; i += 1) {
    const powers = xPowersPerRow[i]!;
    let coefficient = 0;
    for (let j = 0; j < m; j += 1) {
      coefficient += (row[j] ?? 0) * (powers[j] ?? 0);
    }
    coefficients.push(coefficient * scale);
  }
  return coefficients;
}

function applyCoefficients(values: number[], start: number, coefficients: number[]): number {
  let sum = 0;
  for (let i = 0; i < coefficients.length; i += 1) {
    sum += (coefficients[i] ?? 0) * (values[start + i] ?? 0);
  }
  return sum;
}

export function savitzkyGolayFilter(
  input: Array<number | null>,
  options: SavitzkyGolayOptions,
): Array<number | null> {
  const validation = validateSavitzkyGolayOptions(options);
  if (!validation.ok) return [...input];

  const windowLength = Math.floor(options.windowLength);
  const polyOrder = Math.floor(options.polyOrder);
  const derivOrder = Math.floor(options.derivOrder ?? 0);
  const delta = options.delta ?? 1;
  const edgeMode: SavitzkyGolayEdgeMode = options.edgeMode ?? "interp";
  const half = Math.floor(windowLength / 2);

  const output: Array<number | null> = Array.from({ length: input.length }, () => null);

  const centeredCoefficients = buildSavitzkyGolayCoefficients(
    windowLength,
    polyOrder,
    derivOrder,
    delta,
    half,
  );
  if (!centeredCoefficients) return [...input];

  const coeffCache = new Map<number, number[]>();
  coeffCache.set(half, centeredCoefficients);

  let segmentStart: number | null = null;
  const flushSegment = (segmentEndExclusive: number) => {
    if (segmentStart == null) return;
    const start = segmentStart;
    const end = segmentEndExclusive;
    const segmentValues: number[] = [];
    for (let idx = start; idx < end; idx += 1) {
      const v = input[idx];
      if (typeof v === "number" && Number.isFinite(v)) segmentValues.push(v);
    }

    // Sanity: segmentValues length should match segment length when segmentation is correct.
    const segmentLength = end - start;
    if (segmentValues.length !== segmentLength) {
      for (let idx = start; idx < end; idx += 1) output[idx] = input[idx];
      segmentStart = null;
      return;
    }

    if (segmentLength < windowLength) {
      for (let idx = start; idx < end; idx += 1) {
        output[idx] = derivOrder === 0 ? input[idx] : null;
      }
      segmentStart = null;
      return;
    }

    for (let i = 0; i < segmentLength; i += 1) {
      if (edgeMode === "clip") {
        if (i < half || i > segmentLength - half - 1) {
          output[start + i] = null;
          continue;
        }
        output[start + i] = applyCoefficients(segmentValues, i - half, centeredCoefficients);
        continue;
      }

      if (edgeMode === "interp") {
        let windowStart = i - half;
        let position = half;
        if (windowStart < 0) {
          windowStart = 0;
          position = i;
        } else if (windowStart + windowLength > segmentLength) {
          windowStart = segmentLength - windowLength;
          position = i - windowStart;
        }
        let coefficients = coeffCache.get(position);
        if (!coefficients) {
          const computed = buildSavitzkyGolayCoefficients(
            windowLength,
            polyOrder,
            derivOrder,
            delta,
            position,
          );
          if (!computed) {
            output[start + i] = input[start + i];
            continue;
          }
          coeffCache.set(position, computed);
          coefficients = computed;
        }
        output[start + i] = applyCoefficients(segmentValues, windowStart, coefficients);
        continue;
      }

      const windowStart = i - half;
      const paddedValues: number[] = [];
      for (let k = 0; k < windowLength; k += 1) {
        const rawIndex = windowStart + k;
        const mapped =
          edgeMode === "nearest"
            ? Math.min(segmentLength - 1, Math.max(0, rawIndex))
            : reflectIndex(rawIndex, segmentLength);
        paddedValues.push(segmentValues[mapped] ?? 0);
      }
      output[start + i] = paddedValues.reduce((acc, value, idx) => {
        return acc + (centeredCoefficients[idx] ?? 0) * value;
      }, 0);
    }

    segmentStart = null;
  };

  for (let idx = 0; idx < input.length; idx += 1) {
    const v = input[idx];
    const finite = typeof v === "number" && Number.isFinite(v);
    if (finite) {
      if (segmentStart == null) segmentStart = idx;
      continue;
    }
    flushSegment(idx);
    output[idx] = null;
  }
  flushSegment(input.length);

  return output;
}
