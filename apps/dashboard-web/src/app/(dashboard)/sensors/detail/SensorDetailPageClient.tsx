"use client";

import { useEffect } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import { Card } from "@/components/ui/card";

export default function SensorDetailPageClient() {
  const searchParams = useSearchParams();
  const router = useRouter();

  useEffect(() => {
    const sensorId = searchParams.get("id");
    const nodeId = searchParams.get("node");
    const base = "/sensors";
    if (!sensorId) {
      router.replace(base);
      return;
    }
    const nodeParam = nodeId ? `node=${encodeURIComponent(nodeId)}&` : "";
    router.replace(`${base}?${nodeParam}sensor=${encodeURIComponent(sensorId)}`);
  }, [router, searchParams]);

  return (
    <Card className="p-6 text-sm">
      Redirecting to Sensors &amp; Outputs…
    </Card>
  );
}
