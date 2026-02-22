"use client";

import { useMemo, useState } from "react";
import { formatDistanceToNowStrict } from "date-fns";
import { Card } from "@/components/ui/card";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import InlineBanner from "@/components/InlineBanner";
import NodeButton from "@/features/nodes/components/NodeButton";

type AppSettingsBundlePreview = {
  schema_version?: number;
  exported_at?: string;
  setup_credentials?: Array<{ name?: string }>;
  backup_retention?: { policies?: unknown[] };
  map?: { layers?: unknown[]; saves?: unknown[] };
};

const parseBundlePreview = async (file: File): Promise<{ preview: AppSettingsBundlePreview; raw: unknown }> => {
  const text = await file.text();
  const raw = JSON.parse(text) as unknown;
  const preview = (raw && typeof raw === "object" ? (raw as AppSettingsBundlePreview) : {}) satisfies AppSettingsBundlePreview;
  return { preview, raw };
};

export default function AppSettingsRestoreModal({
  open,
  onClose,
  onConfirm,
}: {
  open: boolean;
  onClose: () => void;
  onConfirm: (bundle: unknown) => Promise<void>;
}) {
  const [file, setFile] = useState<File | null>(null);
  const [bundle, setBundle] = useState<unknown>(null);
  const [preview, setPreview] = useState<AppSettingsBundlePreview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmText, setConfirmText] = useState<string>("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const exportedAtLabel = useMemo(() => {
    if (!preview?.exported_at) return null;
    const parsed = new Date(preview.exported_at);
    if (Number.isNaN(parsed.valueOf())) return preview.exported_at;
    return `${formatDistanceToNowStrict(parsed, { addSuffix: true })}`;
  }, [preview?.exported_at]);

  const canSubmit = Boolean(bundle) && confirmText.trim().toUpperCase() === "RESTORE" && !isSubmitting;

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v && !isSubmitting) onClose(); }}>
      <DialogContent className="max-w-lg gap-0">
        <DialogTitle>Restore controller settings</DialogTitle>
        <DialogDescription className="mt-1">
          Imports a settings bundle and replaces the controller&apos;s Setup Center credentials, backup retention policies, and Map configuration.
        </DialogDescription>

 <div className="mt-4 space-y-3 text-sm text-foreground">
          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Settings bundle (.json)
            </label>
            <input
              type="file"
              accept="application/json,.json"
 className="mt-1 block w-full rounded-lg border border-border bg-white px-3 py-2 text-sm focus:border-indigo-500 focus:ring-indigo-500"
              disabled={isSubmitting}
              onChange={(event) => {
                const next = event.target.files?.[0] ?? null;
                setFile(next);
                setBundle(null);
                setPreview(null);
                setError(null);
                setConfirmText("");
                if (!next) return;
                parseBundlePreview(next)
                  .then(({ preview, raw }) => {
                    setPreview(preview);
                    setBundle(raw);
                  })
                  .catch((err) => {
                    setError(err instanceof Error ? err.message : "Invalid JSON file.");
                  });
              }}
            />
            {file ? (
 <p className="mt-1 text-xs text-muted-foreground">
                Selected: <span className="font-semibold">{file.name}</span>
              </p>
            ) : null}
          </div>

          {error ? (
            <InlineBanner tone="danger">
              {error}
            </InlineBanner>
          ) : null}

          {preview ? (
            <Card className="rounded-lg gap-0 bg-card-inset px-3 py-3 text-sm">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <p className="font-semibold">Bundle summary</p>
 <p className="text-xs text-muted-foreground">
                  {exportedAtLabel ? `Exported ${exportedAtLabel}` : "Export time unavailable"}
                </p>
              </div>
              <div className="mt-2 grid gap-2 sm:grid-cols-2">
                <div className="rounded-md bg-card px-3 py-2 shadow-xs">
 <p className="text-xs text-muted-foreground">Schema version</p>
                  <p className="font-semibold">{preview.schema_version ?? "—"}</p>
                </div>
                <div className="rounded-md bg-card px-3 py-2 shadow-xs">
 <p className="text-xs text-muted-foreground">Setup credentials</p>
                  <p className="font-semibold">{preview.setup_credentials?.length ?? 0}</p>
                </div>
                <div className="rounded-md bg-card px-3 py-2 shadow-xs">
 <p className="text-xs text-muted-foreground">Map layers</p>
                  <p className="font-semibold">{preview.map?.layers?.length ?? 0}</p>
                </div>
                <div className="rounded-md bg-card px-3 py-2 shadow-xs">
 <p className="text-xs text-muted-foreground">Map saves</p>
                  <p className="font-semibold">{preview.map?.saves?.length ?? 0}</p>
                </div>
              </div>
            </Card>
          ) : null}

          <div>
 <label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              Type RESTORE to confirm
            </label>
            <Input
              value={confirmText}
              onChange={(event) => setConfirmText(event.target.value)}
              className="mt-1"
              disabled={!bundle || isSubmitting}
              placeholder="RESTORE"
            />
 <p className="mt-1 text-xs text-muted-foreground">
              This is a destructive operation. Download a fresh bundle first if you want a rollback point.
            </p>
          </div>
        </div>

        <div className="mt-6 flex items-center justify-end gap-3">
          <NodeButton onClick={onClose} disabled={isSubmitting}>
            Cancel
          </NodeButton>
          <NodeButton
            variant="primary"
            disabled={!canSubmit}
            onClick={async () => {
              if (!bundle) return;
              setIsSubmitting(true);
              try {
                await onConfirm(bundle);
              } finally {
                setIsSubmitting(false);
              }
            }}
          >
            Restore settings
          </NodeButton>
        </div>
      </DialogContent>
    </Dialog>
  );
}
