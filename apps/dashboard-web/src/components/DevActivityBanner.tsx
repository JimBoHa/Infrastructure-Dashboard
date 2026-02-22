"use client";

import { formatDistanceToNow } from "date-fns";
import { useDevActivityQuery } from "@/lib/queries";
import InlineBanner from "@/components/InlineBanner";

export default function DevActivityBanner() {
  const { data } = useDevActivityQuery();
  if (!data?.active) return null;

  const updatedAt = data.updated_at ? new Date(data.updated_at) : null;
  const expiresAt = data.expires_at ? new Date(data.expires_at) : null;

  const updatedLabel = updatedAt
    ? formatDistanceToNow(updatedAt, { addSuffix: true })
    : null;
  const expiresLabel = expiresAt
    ? formatDistanceToNow(expiresAt, { addSuffix: true })
    : null;

  return (
    <InlineBanner tone="warning" className="p-4 shadow-xs">
      <div className="space-y-1">
        <p className="text-xs font-semibold uppercase tracking-wide text-warning-surface-foreground">
          Active Development
        </p>
 <p className="text-sm text-amber-950">
          {data.message ?? "This dashboard is under active development."}
        </p>
        {updatedLabel || expiresLabel ? (
 <p className="text-xs text-amber-900/80">
            {updatedLabel ? `Last heartbeat ${updatedLabel}.` : null}{" "}
            {expiresLabel ? `Auto-hides ${expiresLabel}.` : null}
          </p>
        ) : null}
      </div>
    </InlineBanner>
  );
}

