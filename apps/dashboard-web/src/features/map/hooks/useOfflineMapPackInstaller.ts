"use client";

import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import { installOfflineMapPack } from "@/lib/api";
import { queryKeys } from "@/lib/queries";

type OfflineMapPackInstallerOptions = {
  packId: string;
  canEdit: boolean;
  fallbackError?: string;
};

type OfflineMapPackInstallerState = {
  install: () => Promise<void>;
  busy: boolean;
  error: string | null;
  resetError: () => void;
};

export function useOfflineMapPackInstaller({
  packId,
  canEdit,
  fallbackError = "Offline map install failed.",
}: OfflineMapPackInstallerOptions): OfflineMapPackInstallerState {
  const queryClient = useQueryClient();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resetError = useCallback(() => setError(null), []);

  const install = useCallback(async () => {
    if (!canEdit) return;
    const id = packId.trim();
    if (!id) return;
    setBusy(true);
    setError(null);
    try {
      await installOfflineMapPack(id);
      await queryClient.invalidateQueries({ queryKey: queryKeys.mapOfflinePacks });
    } catch (err) {
      const message = err instanceof Error ? err.message : fallbackError;
      setError(message);
    } finally {
      setBusy(false);
    }
  }, [canEdit, fallbackError, packId, queryClient]);

  return { install, busy, error, resetError };
}
