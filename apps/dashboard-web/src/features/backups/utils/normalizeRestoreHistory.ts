export type RestoreEvent = { label: string; timestamp: string; target?: string | null; status?: string };

export type RawRestoreEvent = {
  label?: string;
  target?: string | null;
  target_node_id?: string | null;
  backup_node_id?: string | null;
  timestamp?: string;
  recorded_at?: string;
  status?: string;
};

export default function normalizeRestoreHistory(
  history: RawRestoreEvent[] | null | undefined,
): RestoreEvent[] {
  if (!Array.isArray(history)) return [];
  return history.map((item) => {
    const label =
      item.label ??
      `Restore queued for ${item.target ?? item.target_node_id ?? item.backup_node_id ?? "node"}`;
    const ts = item.timestamp ?? item.recorded_at ?? new Date().toISOString();
    return {
      label,
      timestamp: ts,
      target: item.target ?? item.target_node_id ?? item.backup_node_id ?? null,
      status: item.status ?? "queued",
    };
  });
}
