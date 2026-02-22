"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import LoadingState from "@/components/LoadingState";

export default function PowerLegacyPage() {
  const router = useRouter();
  useEffect(() => {
    router.replace("/analytics/power");
  }, [router]);

  return <LoadingState label="Opening Analytics Power..." />;
}
