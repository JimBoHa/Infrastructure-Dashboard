type ValidationResult<T> = { ok: true; value: T } | { ok: false; error: string };

export const parsePort = (label: string, raw: string): ValidationResult<number> => {
  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value) || value <= 0 || value > 65535) {
    return { ok: false, error: `${label} must be a valid TCP port (1..65535).` };
  }
  return { ok: true, value };
};

export const parseSeconds = (
  label: string,
  raw: string,
  min: number,
): ValidationResult<number> => {
  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value) || value < min) {
    return { ok: false, error: `${label} must be at least ${min} seconds.` };
  }
  return { ok: true, value };
};

export const parseMillis = (label: string, raw: string, min: number): ValidationResult<number> => {
  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value) || value < min) {
    return { ok: false, error: `${label} must be at least ${min} ms.` };
  }
  return { ok: true, value };
};

export const parseCount = (label: string, raw: string, min: number): ValidationResult<number> => {
  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value) || value < min) {
    return { ok: false, error: `${label} must be at least ${min}.` };
  }
  return { ok: true, value };
};

