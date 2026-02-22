"use client";

import { useMemo } from "react";

import InlineBanner from "@/components/InlineBanner";
import { summarizeOfflinePackProgress } from "@/features/map/utils/offlinePackProgress";
import type { OfflineMapPack } from "@/types/map";

type Variant = "compact" | "default";
type InstallErrorVariant = "inline" | "banner";
type NotInstalledMode = "explicit" | "fallback";

type OfflinePackInstallStatusProps = {
  pack: OfflineMapPack | null;
  missingMessage: string;
  failedTitle: string;
  installedMessage: string;
  notInstalledMessage: string;
  showMissing?: boolean;
  variant?: Variant;
  installError?: string | null;
  installErrorVariant?: InstallErrorVariant;
  notInstalledMode?: NotInstalledMode;
};

const variantStyles = {
  compact: {
    blockSpacing: "mt-3",
    text: "text-xs",
    padding: "px-3 py-2",
    failedDetail: "mt-1 truncate",
  },
  default: {
    blockSpacing: "mt-4",
    text: "text-sm",
    padding: "px-4 py-3",
    failedDetail: "mt-1 break-words text-xs",
  },
} as const;

const isFallbackNotInstalled = (status: string): boolean =>
  status !== "failed" && status !== "installing" && status !== "installed";

export default function OfflinePackInstallStatus({
  pack,
  missingMessage,
  failedTitle,
  installedMessage,
  notInstalledMessage,
  showMissing = true,
  variant = "default",
  installError,
  installErrorVariant = "inline",
  notInstalledMode = "explicit",
}: OfflinePackInstallStatusProps) {
  const progressSummary = useMemo(() => summarizeOfflinePackProgress(pack), [pack]);
  const styles = variantStyles[variant];

  const showNotInstalled =
    pack &&
    (notInstalledMode === "explicit"
      ? pack.status === "not_installed"
      : isFallbackNotInstalled(pack.status));

  return (
    <>
      {!pack && showMissing ? (
        <InlineBanner tone="warning" className={`${styles.blockSpacing} ${styles.padding} ${styles.text}`}>
          {missingMessage}
        </InlineBanner>
      ) : null}

      {pack?.status === "failed" ? (
        <InlineBanner tone="danger" className={`${styles.blockSpacing} ${styles.padding} ${styles.text}`}>
          <div className="font-semibold">{failedTitle}</div>
          <div className={styles.failedDetail}>{pack.error ?? "Unknown error."}</div>
        </InlineBanner>
      ) : null}

      {pack?.status === "installing" && progressSummary ? (
        <div className={styles.blockSpacing}>
 <div className="flex items-center justify-between text-xs text-muted-foreground">
            <div>
              Downloading {progressSummary.downloaded.toLocaleString()}/
              {progressSummary.total.toLocaleString()} tiles
              {progressSummary.failed ? ` · ${progressSummary.failed.toLocaleString()} failed` : ""}
            </div>
            <div className="font-semibold">{progressSummary.pct.toFixed(0)}%</div>
          </div>
 <div className="mt-2 h-2 overflow-hidden rounded-full bg-muted">
            <div
 className="h-full rounded-full bg-indigo-600 transition-[width]"
              style={{ width: `${progressSummary.pct}%` }}
            />
          </div>
        </div>
      ) : null}

      {pack?.status === "installed" ? (
        <InlineBanner tone="success" className={`${styles.blockSpacing} ${styles.padding} ${styles.text}`}>
          {installedMessage}
        </InlineBanner>
      ) : null}

      {showNotInstalled ? (
 <p className={`${styles.blockSpacing} ${styles.text} text-muted-foreground`}>
          {notInstalledMessage}
        </p>
      ) : null}

      {installError ? (
        installErrorVariant === "banner" ? (
          <InlineBanner tone="danger" className={`mt-3 ${styles.padding} ${styles.text}`}>
            {installError}
          </InlineBanner>
        ) : (
          <p className={`mt-3 ${styles.text} text-rose-600`}>{installError}</p>
        )
      ) : null}
    </>
  );
}
