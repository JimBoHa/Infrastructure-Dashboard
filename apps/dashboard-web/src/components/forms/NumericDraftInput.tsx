"use client";

import type { ComponentPropsWithoutRef } from "react";
import { useMemo, useState } from "react";

type NumericValue = number | null | undefined;

type Props = Omit<ComponentPropsWithoutRef<"input">, "value" | "onChange" | "type"> & {
  value: NumericValue;
  onValueChange: (next: NumericValue) => void;
  emptyValue?: NumericValue;
  emptyBehavior?: "set" | "keep";
  integer?: boolean;
  min?: number;
  max?: number;
  enforceRange?: boolean;
  clampOnBlur?: boolean;
};

function formatValue(value: NumericValue): string {
  return typeof value === "number" && Number.isFinite(value) ? String(value) : "";
}

function isWithinRange(value: number, min?: number, max?: number): boolean {
  if (typeof min === "number" && value < min) return false;
  if (typeof max === "number" && value > max) return false;
  return true;
}

function clampValue(value: number, min?: number, max?: number): number {
  let next = value;
  if (typeof min === "number") next = Math.max(min, next);
  if (typeof max === "number") next = Math.min(max, next);
  return next;
}

export function NumericDraftInput({
  value,
  onValueChange,
  emptyValue = null,
  emptyBehavior = "set",
  integer = false,
  min,
  max,
  enforceRange = false,
  clampOnBlur = false,
  inputMode = "decimal",
  onBlur,
  onFocus,
  ...rest
}: Props) {
  const formattedValue = useMemo(() => formatValue(value), [value]);
  const [raw, setRaw] = useState<string>(formattedValue);
  const [isFocused, setIsFocused] = useState(false);

  return (
    <input
      {...rest}
      type="text"
      inputMode={inputMode}
      value={isFocused ? raw : formattedValue}
      onFocus={(event) => {
        setIsFocused(true);
        setRaw(formattedValue);
        onFocus?.(event);
      }}
      onChange={(event) => {
        const nextRaw = event.target.value;
        setRaw(nextRaw);

        if (!nextRaw.trim()) {
          if (emptyBehavior === "set") {
            onValueChange(emptyValue);
          }
          return;
        }

        const parsed = Number(nextRaw);
        if (!Number.isFinite(parsed)) {
          return;
        }

        if (integer && !Number.isInteger(parsed)) {
          return;
        }

        if (enforceRange && !isWithinRange(parsed, min, max)) {
          return;
        }

        onValueChange(parsed);
      }}
      onBlur={(event) => {
        setIsFocused(false);

        const trimmed = raw.trim();
        if (!trimmed) {
          if (emptyBehavior === "keep") {
            setRaw(formattedValue);
          }
          onBlur?.(event);
          return;
        }

        const parsed = Number(trimmed);
        if (!Number.isFinite(parsed)) {
          setRaw(formattedValue);
          onBlur?.(event);
          return;
        }

        if (integer && !Number.isInteger(parsed)) {
          setRaw(formattedValue);
          onBlur?.(event);
          return;
        }

        if (clampOnBlur) {
          const clamped = clampValue(parsed, min, max);
          onValueChange(clamped);
          setRaw(formatValue(clamped));
          onBlur?.(event);
          return;
        }

        if (enforceRange && !isWithinRange(parsed, min, max)) {
          setRaw(formattedValue);
          onBlur?.(event);
          return;
        }

        onValueChange(parsed);
        setRaw(formatValue(parsed));
        onBlur?.(event);
      }}
    />
  );
}
