import { formatDistanceToNow } from "date-fns";
import type { DemoNode, DemoOutput } from "@/types/dashboard";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useAuth } from "@/components/AuthProvider";
import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";

export default function OutputsGrid({
  outputs,
  nodes,
  onCommand,
}: {
  outputs: DemoOutput[];
  nodes: DemoNode[];
  onCommand: (outputId: string) => void;
}) {
  const { me } = useAuth();
  const canCommand = Boolean(me?.capabilities?.includes("outputs.command"));

  return (
    <CollapsibleCard
      title="Outputs"
      description={
        !canCommand ? (
          <>
            Read-only: you need <code className="px-1">outputs.command</code> to send commands.
          </>
        ) : (
          "Send commands to configured outputs."
        )
      }
      defaultOpen
    >
      <div className="grid gap-4 md:grid-cols-2">
        {outputs.map((output) => {
          const nodeName = nodes.find((node) => node.id === output.node_id)?.name ?? "Unknown";
          return (
            <Card
              key={output.id}
              className="gap-0 bg-card-inset p-4"
            >
              <div className="flex items-center justify-between">
                <div>
 <p className="text-sm font-semibold text-foreground">
                    {output.name}
                  </p>
 <p className="text-xs text-muted-foreground">
                    {nodeName} / {output.type}
                  </p>
                </div>
 <span className="rounded-full bg-muted px-2 py-0.5 text-xs font-semibold uppercase tracking-wide text-foreground">
                  {output.state}
                </span>
              </div>
 <div className="mt-2 text-xs text-muted-foreground">
                Supported states: {output.supported_states?.join(", ") ?? "n/a"}
              </div>
              {output.last_command && (
 <div className="mt-1 text-xs text-muted-foreground">
                  Last command {formatDistanceToNow(new Date(output.last_command), { addSuffix: true })}
                </div>
              )}
              <NodeButton
                fullWidth
                size="sm"
                onClick={() => onCommand(output.id)}
                disabled={!canCommand}
                className="mt-3"
              >
                Send command
              </NodeButton>
            </Card>
          );
        })}
        {!outputs.length && (
 <p className="text-sm text-muted-foreground">No outputs configured.</p>
        )}
      </div>
    </CollapsibleCard>
  );
}
