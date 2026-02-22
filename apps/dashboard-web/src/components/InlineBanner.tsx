"use client";

import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

type InlineBannerTone = "success" | "error" | "danger" | "info" | "warning";

export default function InlineBanner({
  tone,
  children,
  className,
}: {
  tone: InlineBannerTone;
  children: ReactNode;
  className?: string;
}) {
  const toneClasses =
    tone === "success"
      ? "border-success-surface-border bg-success-surface text-success-surface-foreground"
      : tone === "error" || tone === "danger"
        ? "border-danger-surface-border bg-danger-surface text-danger-surface-foreground"
        : tone === "warning"
          ? "border-warning-surface-border bg-warning-surface text-warning-surface-foreground"
          : "border-border bg-card-inset text-card-inset-foreground";

  return (
    <div className={cn("rounded-xl border px-4 py-3 text-sm shadow-xs", toneClasses, className)}>
      {children}
    </div>
  );
}
