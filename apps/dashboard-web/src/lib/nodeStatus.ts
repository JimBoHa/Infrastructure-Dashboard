import { formatDuration } from "@/lib/format";

const parseLastSeen = (lastSeen: Date | string | null | undefined): Date | null => {
  if (!lastSeen) return null;
  if (lastSeen instanceof Date) {
    return Number.isNaN(lastSeen.getTime()) ? null : lastSeen;
  }
  const parsed = new Date(lastSeen);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
};

export const offlineSeconds = (lastSeen: Date | string | null | undefined): number | null => {
  const parsed = parseLastSeen(lastSeen);
  if (!parsed) return null;
  const diffMs = Date.now() - parsed.getTime();
  if (!Number.isFinite(diffMs) || diffMs < 0) return null;
  return Math.floor(diffMs / 1000);
};

export const formatNodeStatusLabel = (
  status: string,
  lastSeen: Date | string | null | undefined,
): string => {
  const normalized = status.trim().toLowerCase();
  if (normalized !== "offline") return status;
  const seconds = offlineSeconds(lastSeen);
  if (seconds == null) return "offline";
  return `offline · ${formatDuration(seconds)}`;
};

