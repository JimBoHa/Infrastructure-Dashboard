"use client";

import { useMemo } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import OfflinePackInstallStatus from "@/features/map/OfflinePackInstallStatus";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useOfflineMapPackInstaller } from "@/features/map/hooks/useOfflineMapPackInstaller";
import { useMapOfflinePacksQuery } from "@/lib/queries";

export default function OfflineMapsSection({
  canEdit,
  onOpenMap,
}: {
  canEdit: boolean;
  onOpenMap: () => void;
}) {
  const offlinePacksQuery = useMapOfflinePacksQuery();
  const {
    install: installSwantonPack,
    busy: swantonInstallBusy,
    error: swantonInstallError,
  } = useOfflineMapPackInstaller({
    packId: "swanton_ca",
    canEdit,
    fallbackError: "Offline map install failed.",
  });

  const swantonPack = useMemo(() => {
    const packs = offlinePacksQuery.data ?? [];
    return packs.find((pack) => pack.id === "swanton_ca") ?? null;
  }, [offlinePacksQuery.data]);

  return (
    <CollapsibleCard
      title="Offline maps"
      description="Download local tiles + glyphs + terrain so the Map tab works without internet after setup (Swanton, CA pack)."
      defaultOpen={false}
      actions={
        <div className="flex flex-wrap gap-2">
          <NodeButton onClick={onOpenMap}>Open Map</NodeButton>
          <NodeButton
            variant="primary"
            disabled={
              !canEdit ||
              swantonInstallBusy ||
              swantonPack?.status === "installing" ||
              swantonPack?.status === "installed" ||
              !swantonPack
            }
            onClick={() => void installSwantonPack()}
          >
            {swantonPack?.status === "installed"
              ? "Installed"
              : swantonPack?.status === "installing"
                ? "Installing…"
                : swantonInstallBusy
                  ? "Starting…"
                  : "Download Swanton pack"}
          </NodeButton>
        </div>
      }
    >
      {offlinePacksQuery.isLoading ? (
 <p className="mt-4 text-sm text-muted-foreground">
          Loading offline map pack status…
        </p>
      ) : null}

      {offlinePacksQuery.error ? (
        <p className="mt-4 text-sm text-rose-600">
          {offlinePacksQuery.error instanceof Error
            ? offlinePacksQuery.error.message
            : "Failed to load offline map packs."}
        </p>
      ) : null}

      <OfflinePackInstallStatus
        pack={swantonPack}
        variant="default"
        showMissing={!offlinePacksQuery.isLoading && !offlinePacksQuery.error}
        notInstalledMode="explicit"
        missingMessage="Swanton offline pack definition is missing. Refresh the controller bundle to apply the migration that seeds map packs."
        failedTitle="Offline pack install failed"
        installedMessage="Swanton offline map pack is installed. The Map tab can use local Streets/Satellite/Topo tiles even without internet."
        notInstalledMessage="Not installed yet. Run this once during setup while internet is available, then the controller can run maps offline."
        installError={swantonInstallError}
        installErrorVariant="inline"
      />
    </CollapsibleCard>
  );
}
