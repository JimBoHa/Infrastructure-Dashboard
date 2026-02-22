"use client";

import { useEffect } from "react";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useAuth } from "@/components/AuthProvider";
import LoadingState from "@/components/LoadingState";

export default function AuthGate({ children }: { children: React.ReactNode }) {
  const { ready, me } = useAuth();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const router = useRouter();

  useEffect(() => {
    if (!ready) return;
    if (pathname === "/login") return;
    if (me) return;
    const next = pathname ?? "/nodes";
    const qs = new URLSearchParams(searchParams?.toString());
    qs.set("next", next);
    router.replace(`/login?${qs.toString()}`);
  }, [ready, me, pathname, router, searchParams]);

  if (!ready) {
    return (
      <div className="p-8">
        <LoadingState label="Checking session…" />
      </div>
    );
  }
  if (!me) {
    return null;
  }
  return <>{children}</>;
}

