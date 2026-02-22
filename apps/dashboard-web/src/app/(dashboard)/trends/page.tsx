"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import LoadingState from "@/components/LoadingState";

export default function TrendsLegacyPage() {
  const router = useRouter();
  useEffect(() => {
    router.replace("/analytics/trends");
  }, [router]);

  return <LoadingState label="Opening Analytics Trends..." />;
}
