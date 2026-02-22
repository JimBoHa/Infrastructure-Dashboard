"use client";

import { useState } from "react";

import CollapsibleCard from "@/components/CollapsibleCard";
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
  const [installerLog, setInstallerLog] = useState<string | null>(null);

  const runInstaller = async (endpoint: string, label: string) => {
    setInstallerLog(`Running ${label}...`);
    try {
      const payload = await runSetupDaemonAction(endpoint, {});
      const data = payload && typeof payload === "object" ? (payload as Record<string, unknown>) : {};
      if (data.ok === false) {
        throw new Error(asStringValue(data.error, `${label} failed`));
      }
      setInstallerLog(JSON.stringify(payload, null, 2));
      onMessage({ type: "success", text: `${label} completed.` });
    } catch (err) {
      const text = err instanceof Error ? err.message : `${label} failed.`;
      onMessage({ type: "error", text });
      setInstallerLog(text);
    }
  };

  const runBackup = async () => {
    setInstallerLog("Running backup...");
    try {
      const data = await runBackupNow();
      setInstallerLog(JSON.stringify(data, null, 2));
      onMessage({ type: "success", text: "Backup completed." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Backup failed.";
      onMessage({ type: "error", text });
      setInstallerLog(text);
    }
  };

  return (
    <CollapsibleCard
      title="Installer actions"
      description="Apply bundles, roll back, and export diagnostics from the setup app."
      defaultOpen
      bodyClassName="space-y-4"
      className="h-fit"
    >
      <div className="flex flex-wrap gap-3">
        <NodeButton variant="primary" onClick={() => runInstaller("install", "Install")}>
          Install
        </NodeButton>
        <NodeButton onClick={() => runInstaller("upgrade", "Upgrade")}>Upgrade</NodeButton>
        <NodeButton onClick={() => runInstaller("rollback", "Rollback")}>Rollback</NodeButton>
        <NodeButton onClick={() => runInstaller("diagnostics", "Diagnostics")}>Diagnostics</NodeButton>
        <NodeButton onClick={runBackup}>Backup now</NodeButton>
      </div>
      {installerLog && (
 <pre className="max-h-56 overflow-auto rounded-xl bg-neutral-900 p-3 text-xs text-neutral-100">
          {installerLog}
        </pre>
      )}
    </CollapsibleCard>
  );
}

