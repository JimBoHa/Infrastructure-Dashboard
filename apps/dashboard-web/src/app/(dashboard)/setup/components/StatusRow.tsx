"use client";

import { Card } from "@/components/ui/card";

export default function StatusRow({
  name,
  status,
  lastSeen,
  detail,
  meta,
  metaTitle,
}: {
  name: string;
  status?: string;
  lastSeen: string | null;
  detail: string | null;
  meta?: string | null;
  metaTitle?: string;
}) {
  const tone =
    status === "ok"
 ? "text-emerald-600"
      : status === "error"
 ? "text-rose-600"
 : "text-amber-600";
  return (
    <Card className="flex-col gap-1 rounded-lg px-3 py-2 md:flex-row md:items-center md:justify-between">
      <div className="min-w-0">
        <p className="truncate text-sm font-semibold text-card-foreground">
          {name}
        </p>
 <p className="text-xs text-muted-foreground">
          {detail
            ? detail
            : lastSeen
              ? `Last seen ${new Date(lastSeen).toLocaleString()}`
              : "No data yet"}
        </p>
      </div>
      <div className="flex items-center gap-2">
        {meta ? (
          <span
 className="text-[11px] font-semibold text-muted-foreground"
            title={metaTitle ?? meta}
          >
            {meta}
          </span>
        ) : null}
        <p className={`text-xs font-semibold uppercase tracking-wide ${tone}`}>
          {status ?? "unknown"}
        </p>
      </div>
    </Card>
  );
}

