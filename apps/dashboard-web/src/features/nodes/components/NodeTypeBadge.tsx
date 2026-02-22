import type { DemoNode } from "@/types/dashboard";
import { isCoreNode } from "@/lib/coreNode";
import NodePill, { type NodePillTone } from "@/features/nodes/components/NodePill";

type NodeTypeMeta = {
  label: string;
  tone: NodePillTone;
  title: string;
};

function getStringConfig(node: DemoNode, key: string): string | null {
  const config = (node.config ?? {}) as Record<string, unknown>;
  const raw = config[key];
  return typeof raw === "string" ? raw.trim() : null;
}

function classifyNodeType(node: DemoNode): NodeTypeMeta {
  if (isCoreNode(node)) {
    return { label: "Core", tone: "muted", title: "Controller / core services" };
  }

  const kind = getStringConfig(node, "kind");
  if (kind?.toLowerCase() === "ws-2902") {
    const protocol = getStringConfig(node, "protocol");
    const title = protocol ? `Weather station (WS-2902, ${protocol})` : "Weather station (WS-2902)";
    return { label: "WS-2902", tone: "info", title };
  }

  const externalProvider = getStringConfig(node, "external_provider");
  const powerProvider = getStringConfig(node, "power_provider");
  const model = getStringConfig(node, "model");
  if (
    externalProvider?.toLowerCase() === "emporia" ||
    powerProvider?.toLowerCase().includes("emporia") === true ||
    model?.toUpperCase().startsWith("VUE") === true
  ) {
    const modelTitle = model ? `Emporia Vue (${model})` : "Emporia Vue";
    return { label: "Vue", tone: "warning", title: modelTitle };
  }

  const agentNodeId = getStringConfig(node, "agent_node_id");
  if (agentNodeId) {
    return { label: "Pi 5", tone: "success", title: `Raspberry Pi 5 node (${agentNodeId})` };
  }

  if (kind) {
    return { label: kind, tone: "neutral", title: `Node kind: ${kind}` };
  }

  return { label: "Node", tone: "neutral", title: "Node type unknown" };
}

export default function NodeTypeBadge({
  node,
  size = "sm",
  className,
}: {
  node: DemoNode;
  size?: "sm" | "md" | "lg";
  className?: string;
}) {
  const meta = classifyNodeType(node);
  return (
    <NodePill tone={meta.tone} size={size} caps className={className} title={meta.title}>
      {meta.label}
    </NodePill>
  );
}

