import type { DemoNode } from "@/types/dashboard";

export const CORE_NODE_ID = "00000000-0000-0000-0000-000000000001";

export function isCoreNode(node: DemoNode | null | undefined): boolean {
  if (!node) return false;
  if (node.id === CORE_NODE_ID) return true;
  const config = (node.config ?? {}) as Record<string, unknown>;
  return typeof config.kind === "string" && config.kind.toLowerCase() === "core";
}

