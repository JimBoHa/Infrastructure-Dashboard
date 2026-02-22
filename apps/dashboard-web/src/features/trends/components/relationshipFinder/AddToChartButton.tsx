"use client";

import NodeButton from "@/features/nodes/components/NodeButton";

type AddToChartButtonProps = {
  sensorId: string;
  isOnChart: boolean;
  isAtLimit: boolean;
  onAddToChart?: (sensorId: string) => void;
  size?: "sm" | "md";
};

export default function AddToChartButton({
  sensorId,
  isOnChart,
  isAtLimit,
  onAddToChart,
  size = "sm",
}: AddToChartButtonProps) {
  if (!onAddToChart) {
 return <span className="text-xs text-muted-foreground">—</span>;
  }

  if (isOnChart) {
    return (
 <span className="text-xs font-semibold text-emerald-600">
        Added
      </span>
    );
  }

  const disabled = isAtLimit;
  const title = disabled
    ? "Chart limit reached (20 sensors). Remove a sensor to add more."
    : "Add sensor to chart [A]";

  return (
    <NodeButton
      variant="secondary"
      disabled={disabled}
      onClick={(e) => {
        e.stopPropagation();
        onAddToChart(sensorId);
      }}
      title={title}
      className={size === "sm" ? "px-2 py-1 text-xs" : undefined}
    >
      {disabled ? "Limit" : "Add"}
    </NodeButton>
  );
}
