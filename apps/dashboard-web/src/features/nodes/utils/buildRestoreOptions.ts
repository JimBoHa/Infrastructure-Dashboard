import type { DemoBackup, DemoNode } from "@/types/dashboard";

export type RestoreOption = { node_id: string; node_name: string; last_backup: string };

const toSortKey = (value?: DemoBackup["captured_at"] | string) => {
  if (value instanceof Date) {
    return value.toISOString();
  }
  if (typeof value === "string") {
    return value;
  }
  return "";
};

const toDateLabel = (value?: DemoBackup["captured_at"] | string) => {
  if (value instanceof Date) {
    return value.toISOString().slice(0, 10);
  }
  if (typeof value === "string") {
    return value.slice(0, 10);
  }
  return "Unknown";
};

export default function buildRestoreOptions(
  nodes: DemoNode[],
  backupMap: Record<string, DemoBackup[]>,
): RestoreOption[] {
  return nodes
    .map((node) => {
      const backups = backupMap[node.id] ?? [];
      if (!backups.length) {
        return null;
      }
      const sorted = [...backups].sort((a, b) =>
        toSortKey(b.captured_at).localeCompare(toSortKey(a.captured_at)),
      );
      const lastBackup = toDateLabel(sorted[0]?.captured_at);
      return {
        node_id: node.id,
        node_name: node.name,
        last_backup: lastBackup,
      };
    })
    .filter((item): item is RestoreOption => Boolean(item));
}
