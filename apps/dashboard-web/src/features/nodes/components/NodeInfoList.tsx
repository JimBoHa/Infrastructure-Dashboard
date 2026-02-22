import { Card } from "@/components/ui/card";
import NodePill from "@/features/nodes/components/NodePill";
import type { ReactNode } from "react";

type NodeInfoListItem = {
  id: string;
  name: string;
  description?: string;
  badges?: ReactNode;
  pill?: string;
  pillTitle?: string;
};

export default function NodeInfoList({
  title,
  emptyLabel,
  items,
  maxItems,
}: {
  title: string;
  emptyLabel: string;
  items: NodeInfoListItem[];
  maxItems?: number;
}) {
  const limit = typeof maxItems === "number" && maxItems > 0 ? Math.floor(maxItems) : null;
  const shouldTruncate = limit != null && items.length > limit;
  const visibleItems = shouldTruncate ? items.slice(0, limit) : items;
  const hiddenCount = shouldTruncate && limit != null ? Math.max(0, items.length - limit) : 0;

  return (
    <div>
 <h4 className="text-sm font-semibold text-foreground">{title}</h4>
      <div className="mt-2 space-y-2 text-sm text-foreground">
        {visibleItems.length ? (
          visibleItems.map((item) => (
            <Card
              key={item.id}
              className="gap-0 rounded-lg px-3 py-2 shadow-xs"
            >
              <div className="flex items-center justify-between gap-3">
                <div className="flex min-w-0 items-center gap-2">
 <p className="min-w-0 truncate font-medium text-foreground">
                    {item.name}
                  </p>
                  {item.badges ? <span className="shrink-0">{item.badges}</span> : null}
                </div>
                {item.pill && (
                  <NodePill tone="neutral" size="sm" caps weight="normal" title={item.pillTitle}>
                    {item.pill}
                  </NodePill>
                )}
              </div>
              {item.description && (
 <p className="text-xs text-muted-foreground">{item.description}</p>
              )}
            </Card>
          ))
        ) : (
          <p className="text-xs text-muted-foreground">{emptyLabel}</p>
        )}
        {hiddenCount > 0 ? (
          <p className="text-xs text-muted-foreground">
            +{hiddenCount} more — open &quot;More details&quot; for the full list.
          </p>
        ) : null}
      </div>
    </div>
  );
}
