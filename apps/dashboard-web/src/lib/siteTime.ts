"use client";

import { useMemo } from "react";
import { useConnectionQuery } from "@/lib/queries";
import type { DemoConnection } from "@/types/dashboard";

export function browserTimeZone(): string {
  try {
    return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
  } catch {
    return "UTC";
  }
}

export function controllerTimeZone(connection: DemoConnection | null | undefined): string {
  const tz = typeof connection?.timezone === "string" ? connection.timezone.trim() : "";
  return tz ? tz : browserTimeZone();
}

export function useControllerTimeZone(): string {
  const { data: connection } = useConnectionQuery();
  return useMemo(() => controllerTimeZone(connection), [connection]);
}

type ZonedDateParts = {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
};

function partsInTimeZone(date: Date, timeZone: string): ZonedDateParts | null {
  if (!(date instanceof Date) || !Number.isFinite(date.getTime())) return null;
  try {
    const dtf = new Intl.DateTimeFormat("en-US", {
      timeZone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
    const parts = dtf.formatToParts(date);
    const map: Record<string, string> = {};
    parts.forEach((p) => {
      if (p.type !== "literal") map[p.type] = p.value;
    });
    const year = Number(map.year);
    const month = Number(map.month);
    const day = Number(map.day);
    const hour = Number(map.hour);
    const minute = Number(map.minute);
    const second = Number(map.second);
    if (
      !Number.isFinite(year) ||
      !Number.isFinite(month) ||
      !Number.isFinite(day) ||
      !Number.isFinite(hour) ||
      !Number.isFinite(minute) ||
      !Number.isFinite(second)
    ) {
      return null;
    }
    return {
      year,
      month,
      day,
      hour,
      minute,
      second,
    };
  } catch {
    return null;
  }
}

function timeZoneOffsetMs(date: Date, timeZone: string): number {
  const parts = partsInTimeZone(date, timeZone);
  if (!parts) return 0;
  const asUtc = Date.UTC(
    parts.year,
    parts.month - 1,
    parts.day,
    parts.hour,
    parts.minute,
    parts.second,
    0,
  );
  return asUtc - date.getTime();
}

export function formatDateTimeForTimeZone(
  date: Date,
  timeZone: string,
  options: Intl.DateTimeFormatOptions,
): string {
  try {
    return new Intl.DateTimeFormat(undefined, { timeZone, ...options }).format(date);
  } catch {
    return date.toLocaleString();
  }
}

export function formatChartTickTime(dateOrValue: unknown, timeZone: string): string {
  const date =
    dateOrValue instanceof Date
      ? dateOrValue
      : typeof dateOrValue === "number"
        ? new Date(dateOrValue)
        : typeof dateOrValue === "string" && dateOrValue.trim()
          ? new Date(Number(dateOrValue))
          : null;
  if (!date || !Number.isFinite(date.getTime())) return "";
  return formatDateTimeForTimeZone(date, timeZone, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function formatChartTickTimeOfDay(dateOrValue: unknown, timeZone: string): string {
  const date =
    dateOrValue instanceof Date
      ? dateOrValue
      : typeof dateOrValue === "number"
        ? new Date(dateOrValue)
        : typeof dateOrValue === "string" && dateOrValue.trim()
          ? new Date(Number(dateOrValue))
          : null;
  if (!date || !Number.isFinite(date.getTime())) return "";
  return formatDateTimeForTimeZone(date, timeZone, {
    hour: "numeric",
    minute: "2-digit",
  });
}

export function formatChartTickDate(dateOrValue: unknown, timeZone: string): string {
  const date =
    dateOrValue instanceof Date
      ? dateOrValue
      : typeof dateOrValue === "number"
        ? new Date(dateOrValue)
        : typeof dateOrValue === "string" && dateOrValue.trim()
          ? new Date(Number(dateOrValue))
          : null;
  if (!date || !Number.isFinite(date.getTime())) return "";
  return formatDateTimeForTimeZone(date, timeZone, {
    month: "short",
    day: "numeric",
  });
}

export function formatChartTooltipTime(dateOrValue: unknown, timeZone: string): string {
  const date =
    dateOrValue instanceof Date
      ? dateOrValue
      : typeof dateOrValue === "number"
        ? new Date(dateOrValue)
        : typeof dateOrValue === "string" && dateOrValue.trim()
          ? new Date(Number(dateOrValue))
          : null;
  if (!date || !Number.isFinite(date.getTime())) return "";
  return formatDateTimeForTimeZone(date, timeZone, {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function formatChartTooltipDate(dateOrValue: unknown, timeZone: string): string {
  const date =
    dateOrValue instanceof Date
      ? dateOrValue
      : typeof dateOrValue === "number"
        ? new Date(dateOrValue)
        : typeof dateOrValue === "string" && dateOrValue.trim()
          ? new Date(Number(dateOrValue))
          : null;
  if (!date || !Number.isFinite(date.getTime())) return "";
  return formatDateTimeForTimeZone(date, timeZone, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function formatDateTimeInputValue(date: Date, timeZone: string): string {
  const parts = partsInTimeZone(date, timeZone);
  if (!parts) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${parts.year}-${pad(parts.month)}-${pad(parts.day)}T${pad(parts.hour)}:${pad(parts.minute)}`;
}

export function parseDateTimeInputValueToIso(value: string, timeZone: string): string | null {
  const match = value.trim().match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/);
  if (!match) return null;
  const year = Number(match[1]);
  const month = Number(match[2]);
  const day = Number(match[3]);
  const hour = Number(match[4]);
  const minute = Number(match[5]);
  if (
    !Number.isFinite(year) ||
    !Number.isFinite(month) ||
    !Number.isFinite(day) ||
    !Number.isFinite(hour) ||
    !Number.isFinite(minute)
  ) {
    return null;
  }

  const utcGuess = Date.UTC(year, month - 1, day, hour, minute, 0, 0);
  let utcMs = utcGuess - timeZoneOffsetMs(new Date(utcGuess), timeZone);
  const adjusted = utcGuess - timeZoneOffsetMs(new Date(utcMs), timeZone);
  if (Number.isFinite(adjusted)) utcMs = adjusted;

  const date = new Date(utcMs);
  if (!Number.isFinite(date.getTime())) return null;
  return date.toISOString();
}

export function startOfDayInTimeZone(date: Date, timeZone: string): Date {
  const parts = partsInTimeZone(date, timeZone);
  if (!parts) {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate(), 0, 0, 0, 0);
  }
  const pad = (n: number) => String(n).padStart(2, "0");
  const iso = parseDateTimeInputValueToIso(
    `${parts.year}-${pad(parts.month)}-${pad(parts.day)}T00:00`,
    timeZone,
  );
  if (!iso) {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate(), 0, 0, 0, 0);
  }
  const zonedMidnight = new Date(iso);
  if (!Number.isFinite(zonedMidnight.getTime())) {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate(), 0, 0, 0, 0);
  }
  return zonedMidnight;
}

export function addDaysInTimeZone(date: Date, days: number, timeZone: string): Date {
  const parts = partsInTimeZone(date, timeZone);
  if (!parts) {
    const next = new Date(date.getTime());
    next.setDate(next.getDate() + days);
    return next;
  }
  const shiftedUtc = new Date(Date.UTC(parts.year, parts.month - 1, parts.day + days, 0, 0, 0, 0));
  const pad = (n: number) => String(n).padStart(2, "0");
  const iso = parseDateTimeInputValueToIso(
    `${shiftedUtc.getUTCFullYear()}-${pad(shiftedUtc.getUTCMonth() + 1)}-${pad(shiftedUtc.getUTCDate())}T00:00`,
    timeZone,
  );
  if (!iso) {
    const next = new Date(date.getTime());
    next.setDate(next.getDate() + days);
    return next;
  }
  const zoned = new Date(iso);
  if (!Number.isFinite(zoned.getTime())) {
    const next = new Date(date.getTime());
    next.setDate(next.getDate() + days);
    return next;
  }
  return zoned;
}
