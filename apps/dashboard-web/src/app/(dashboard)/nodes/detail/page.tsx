import { Suspense } from "react";
import { Card } from "@/components/ui/card";
import NodeDetailPageClient from "./NodeDetailPageClient";

export default function NodeDetailPage() {
  return (
    <Suspense
      fallback={
        <Card className="p-6 text-sm">
          Loading node…
        </Card>
      }
    >
      <NodeDetailPageClient />
    </Suspense>
  );
}
