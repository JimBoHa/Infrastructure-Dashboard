import { Suspense } from "react";
import { Card } from "@/components/ui/card";
import SensorDetailPageClient from "./SensorDetailPageClient";

export default function SensorDetailPage() {
  return (
    <Suspense
      fallback={
        <Card className="p-6 text-sm">
          Loading sensor…
        </Card>
      }
    >
      <SensorDetailPageClient />
    </Suspense>
  );
}
