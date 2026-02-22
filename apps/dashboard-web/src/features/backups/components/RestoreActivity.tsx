import type { RestoreEvent } from "@/features/backups/utils/normalizeRestoreHistory";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";

export default function RestoreActivity({
  history,
}: {
  history: RestoreEvent[];
}) {
  if (!history.length) {
    return null;
  }
  return (
    <CollapsibleCard
      title="Restore activity"
      description="Recent restore requests and outcomes."
      defaultOpen={false}
    >
      <div className="space-y-2 text-sm text-muted-foreground">
        {history.map((item, index) => (
          <Card
            key={`${item.timestamp}-${index}`}
            className="flex-row items-center justify-between gap-0 rounded-lg bg-card-inset px-3 py-2"
          >
            <div className="flex flex-col">
              <span className="font-medium">{item.label}</span>
 <span className="text-xs text-muted-foreground">
                {new Date(item.timestamp).toLocaleString()}
              </span>
            </div>
            <span
              className={`rounded-full px-3 py-1 text-xs font-semibold uppercase tracking-wide ${
                item.status === "ok"
 ? "bg-emerald-100 text-emerald-800"
                  : item.status === "error"
 ? "bg-rose-100 text-rose-800"
 : "bg-muted text-foreground"
              }`}
            >
              {item.status ?? "queued"}
            </span>
          </Card>
        ))}
      </div>
    </CollapsibleCard>
  );
}
