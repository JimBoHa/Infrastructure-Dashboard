"use client";

import { useQueryClient } from "@tanstack/react-query";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import NodeButton from "@/features/nodes/components/NodeButton";
import { postJson } from "@/lib/api";
import { queryKeys, useAnalyticsFeedStatusQuery } from "@/lib/queries";

import type { Message } from "../types";

export default function AnalyticsFeedsSection({
  onMessage,
}: {
  onMessage: (message: Message) => void;
}) {
  const queryClient = useQueryClient();
  const feedsQuery = useAnalyticsFeedStatusQuery();

  const pollAnalyticsFeedsNow = async () => {
    try {
      await postJson("/api/analytics/feeds/poll", {});
      await queryClient.invalidateQueries({ queryKey: queryKeys.analyticsFeedStatus });
      onMessage({ type: "success", text: "Triggered analytics feeds poll." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to poll analytics feeds.";
      onMessage({ type: "error", text });
    }
  };

  return (
    <CollapsibleCard
      title="Analytics feeds"
      description="Live integrations and credential status for power, water, and forecast feeds."
      defaultOpen
      bodyClassName="space-y-4"
      className="h-fit"
      actions={
        <div className="flex flex-wrap gap-2">
          <NodeButton
            size="xs"
            onClick={() => queryClient.invalidateQueries({ queryKey: queryKeys.analyticsFeedStatus })}
          >
            Refresh
          </NodeButton>
          <NodeButton size="xs" variant="primary" onClick={pollAnalyticsFeedsNow}>
            Poll now
          </NodeButton>
        </div>
      }
    >
      <div className="space-y-2 text-sm">
        {feedsQuery.data?.feeds && Object.entries(feedsQuery.data.feeds).length > 0 ? (
          Object.entries(feedsQuery.data.feeds).map(([name, entry]) => (
            <Card
              key={name}
              className="flex-row items-center justify-between rounded-lg gap-0 bg-card-inset px-3 py-2"
            >
              <span className="capitalize">{name.replace(/_/g, " ")}</span>
              <span
                className={`text-xs font-semibold ${
                  entry.status === "ok"
 ? "text-emerald-600"
 : "text-amber-600"
                }`}
              >
                {entry.status ?? "unknown"}
              </span>
            </Card>
          ))
        ) : (
 <p className="text-muted-foreground">No analytics feed status available.</p>
        )}
      </div>
    </CollapsibleCard>
  );
}

