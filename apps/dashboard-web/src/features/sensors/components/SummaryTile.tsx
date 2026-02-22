import { Card } from "@/components/ui/card";

export default function SummaryTile({
  label,
  value,
  hint,
}: {
  label: string;
  value: string | number;
  hint?: string;
}) {
  return (
    <Card className="gap-0 bg-card-inset p-4">
 <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </p>
 <p className="mt-1 text-2xl font-semibold text-foreground">{value}</p>
 {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </Card>
  );
}
