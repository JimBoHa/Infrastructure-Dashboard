"use client";

import clsx from "clsx";
import type { ReactNode } from "react";
import CollapsibleCard from "@/components/CollapsibleCard";

export default function AnalysisKey({
  overview,
  children,
  summary = "Key",
  size = "sm",
  className,
  contentClassName,
  defaultOpen = false,
}: {
  overview: ReactNode;
  children: ReactNode;
  summary?: string;
  size?: "xs" | "sm";
  className?: string;
  contentClassName?: string;
  defaultOpen?: boolean;
}) {
  const titleClass =
    size === "xs"
      ? "text-[11px] font-semibold uppercase tracking-wide text-muted-foreground"
      : "text-xs font-semibold uppercase tracking-wide text-muted-foreground";
  const overviewClass =
    size === "xs"
      ? "mt-1 text-[11px] text-foreground"
      : "mt-1 text-xs text-foreground";

  return (
    <CollapsibleCard
      title={<span className={titleClass}>{summary}</span>}
      description={<span className={overviewClass}>{overview}</span>}
      density="sm"
      defaultOpen={defaultOpen}
      className={clsx("bg-card-inset", className)}
    >
      <div
        className={clsx(
          "text-xs text-card-foreground",
          "max-h-[70vh] overflow-auto overscroll-contain",
          contentClassName,
        )}
      >
        {children}
      </div>
    </CollapsibleCard>
  );
}
