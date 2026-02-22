"use client";

import CollapsibleCard from "@/components/CollapsibleCard";
import NodeButton from "@/features/nodes/components/NodeButton";
import OfflinePackInstallStatus from "@/features/map/OfflinePackInstallStatus";
import { useOfflineMapPackInstaller } from "@/features/map/hooks/useOfflineMapPackInstaller";
import type { OfflineMapPack } from "@/types/map";

type MapOfflinePackCardProps = {
  canEdit: boolean;
  swantonPack: OfflineMapPack | null;
};

export default function MapOfflinePackCard({ canEdit, swantonPack }: MapOfflinePackCardProps) {
  const { install, busy, error } = useOfflineMapPackInstaller({
    packId: "swanton_ca",
    canEdit,
    fallbackError: "Offline pack install failed.",
  });

  const installDisabled =
    !canEdit ||
    !swantonPack ||
    busy ||
    swantonPack.status === "installing" ||
    swantonPack.status === "installed";

  const installLabel =
    swantonPack?.status === "installed"
      ? "Installed"
      : swantonPack?.status === "installing"
        ? "Installing…"
        : busy
          ? "Starting…"
          : "Download";

  return (
    <CollapsibleCard
      density="sm"
      title="Offline map pack"
      description="Download Swanton, CA tiles + glyphs + terrain so the map still works without internet after setup."
      actions={
        <NodeButton size="sm" onClick={() => void install()} disabled={installDisabled}>
          {installLabel}
        </NodeButton>
      }
    >
      <OfflinePackInstallStatus
        pack={swantonPack}
        variant="compact"
        notInstalledMode="fallback"
        missingMessage="Offline pack definition is missing. If this is a fresh install, apply migrations and refresh the controller bundle."
        failedTitle="Install failed"
        installedMessage="Offline pack installed. Select Streets/Satellite/Topo above to use local tiles."
        notInstalledMessage="Not installed yet. Install once during setup (internet required), then the map runs offline."
        installError={error}
        installErrorVariant="banner"
      />
    </CollapsibleCard>
  );
}
