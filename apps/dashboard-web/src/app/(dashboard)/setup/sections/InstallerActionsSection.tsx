"use client";

import { useState } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import NodeButton from "@/features/nodes/components/NodeButton";
import { runBackupNow } from "@/lib/api";

import { runSetupDaemonAction } from "../api/setupDaemon";
import { asStringValue } from "../lib/setupDaemonParsers";
import type { Message } from "../types";

export default function InstallerActionsSection({
  onMessage,
}: {
  onMessage: (message: Message) => void;
}) {
  const [statusNote, setStatusNote] = useState<string | null>(null);
  const [uninstallOpen, setUninstallOpen] = useState(false);
  const [busyAction, setBusyAction] = useState<string | null>(null);

  const runInstaller = async (
    endpoint: string,
    label: string,
    body: Record<string, unknown> = {},
  ) => {
    setBusyAction(label);
    setStatusNote(`${label} is running. Detailed diagnostics are being written to the setup log.`);
    try {
      const payload = await runSetupDaemonAction(endpoint, body);
      const data = payload && typeof payload === "object" ? (payload as Record<string, unknown>) : {};
      if (data.ok === false) {
        throw new Error(asStringValue(data.error, `${label} failed`));
      }
      const message = asStringValue(data.message, `${label} completed.`);
      setStatusNote(message);
      onMessage({ type: "success", text: message });
    } catch (err) {
      const text = err instanceof Error ? err.message : `${label} failed.`;
      onMessage({ type: "error", text });
      setStatusNote(text);
    } finally {
      setBusyAction(null);
    }
  };

  const runBackup = async () => {
    setBusyAction("Backup");
    setStatusNote("Running backup. Detailed diagnostics are being written to the setup log.");
    try {
      await runBackupNow();
      setStatusNote("Backup completed.");
      onMessage({ type: "success", text: "Backup completed." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Backup failed.";
      onMessage({ type: "error", text });
      setStatusNote(text);
    } finally {
      setBusyAction(null);
    }
  };

  return (
    <>
      <CollapsibleCard
        title="Installer actions"
        description="Apply bundles, roll back, back up, or uninstall from the setup app."
        defaultOpen
        bodyClassName="space-y-4"
        className="h-fit"
      >
        <div className="flex flex-wrap gap-3">
          <NodeButton
            variant="primary"
            onClick={() => runInstaller("install", "Install")}
            loading={busyAction === "Install"}
          >
            Install
          </NodeButton>
          <NodeButton
            onClick={() => runInstaller("upgrade", "Upgrade")}
            loading={busyAction === "Upgrade"}
          >
            Upgrade
          </NodeButton>
          <NodeButton
            onClick={() => runInstaller("rollback", "Rollback")}
            loading={busyAction === "Rollback"}
          >
            Rollback
          </NodeButton>
          <NodeButton
            onClick={() => runInstaller("diagnostics", "Diagnostics")}
            loading={busyAction === "Diagnostics"}
          >
            Diagnostics
          </NodeButton>
          <NodeButton onClick={runBackup} loading={busyAction === "Backup"}>
            Backup now
          </NodeButton>
          <NodeButton onClick={() => setUninstallOpen(true)} loading={busyAction === "Uninstall"}>
            Uninstall…
          </NodeButton>
        </div>
        {statusNote ? (
          <div className="rounded-xl border border-border bg-card-inset px-3 py-3 text-sm text-card-inset-foreground">
            {statusNote}
          </div>
        ) : null}
      </CollapsibleCard>

      <Dialog open={uninstallOpen} onOpenChange={setUninstallOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Uninstall Infrastructure Dashboard</DialogTitle>
            <DialogDescription>
              Choose whether to remove everything or keep a portable archive of trend data and
              sensor names before the controller is removed.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter className="gap-2 sm:justify-start">
            <NodeButton
              onClick={() => {
                setUninstallOpen(false);
                void runInstaller("uninstall", "Uninstall", {
                  preserve_trends_and_sensors: true,
                });
              }}
            >
              Keep trend archive
            </NodeButton>
            <NodeButton
              variant="primary"
              onClick={() => {
                setUninstallOpen(false);
                void runInstaller("uninstall", "Uninstall", {
                  preserve_trends_and_sensors: false,
                });
              }}
            >
              Remove everything
            </NodeButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
