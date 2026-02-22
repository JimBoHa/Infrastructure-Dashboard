"use client";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import type { AnalyticsFeedStatus } from "@/types/dashboard";

export function FeedHealthSection({ feeds }: { feeds: AnalyticsFeedStatus | undefined }) {
  if (!feeds) return null;
  const entries = Object.entries(feeds.feeds || {}).map(([key, value]) => ({
    name: key,
    status: value?.status ?? "unknown",
  }));
  return (
    <CollapsibleCard
      title="Feed health"
      description={`External analytics connectors (${feeds.enabled ? "enabled" : "disabled"}).`}
      defaultOpen={false}
      bodyClassName="space-y-4"
    >
      {entries.length === 0 ? (
 <p className="text-sm text-muted-foreground">No feeds reported yet.</p>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2">
          {entries.map((entry) => (
            <Card
              key={entry.name}
              className="flex min-w-0 items-center justify-between gap-3 rounded-lg gap-0 bg-card-inset px-3 py-2.5 text-xs shadow-xs"
            >
              <span className="truncate font-semibold text-card-foreground">
                {entry.name}
              </span>
              <span
                className={`shrink-0 whitespace-nowrap rounded-full px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wide ${
                  entry.status === "ok"
 ? "bg-emerald-100 text-emerald-800"
                    : entry.status === "error"
 ? "bg-rose-100 text-rose-800"
 : "bg-muted text-foreground"
                }`}
              >
                {entry.status}
              </span>
            </Card>
          ))}
        </div>
      )}
    </CollapsibleCard>
  );
}

