import type { AlarmOriginFilter } from "@/lib/alarms/origin";
import type { DemoNode } from "@/types/dashboard";
import { Select } from "@/components/ui/select";
import SegmentedControl from "@/components/SegmentedControl";
import SummaryTile from "@/features/sensors/components/SummaryTile";
import NodeButton from "@/features/nodes/components/NodeButton";
import PageHeaderCard from "@/components/PageHeaderCard";

export default function SensorsOverview({
  sensorsCount,
  outputsCount,
  predictiveAlarmCount,
  nodesOnlineCount,
  nodes,
  groupByNode,
  nodeFilter,
  typeFilter,
  alarmOriginFilter,
  sensorTypes,
  canEdit = false,
  onNodeFilterChange,
  onTypeFilterChange,
  onAlarmOriginChange,
  onGroupByNodeChange,
  onBulkDecimals,
  onReorder,
  onRefresh,
  refreshLoading = false,
  refreshLabel,
}: {
  sensorsCount: number;
  outputsCount: number;
  predictiveAlarmCount: number;
  nodesOnlineCount: number;
  nodes: DemoNode[];
  groupByNode: boolean;
  nodeFilter: string;
  typeFilter: string;
  alarmOriginFilter: AlarmOriginFilter;
  sensorTypes: string[];
  canEdit?: boolean;
  onNodeFilterChange: (value: string) => void;
  onTypeFilterChange: (value: string) => void;
  onAlarmOriginChange: (value: AlarmOriginFilter) => void;
  onGroupByNodeChange: (value: boolean) => void;
  onBulkDecimals?: () => void;
  onReorder?: () => void;
  onRefresh: () => void;
  refreshLoading?: boolean;
  refreshLabel?: string;
}) {
  return (
    <PageHeaderCard
      title="Sensors & Outputs"
      description="Sensor view for naming/formatting, quick alarm triage, and output commands. Use Nodes for adoption, hardware health, and backups."
      actions={
        <div className="flex items-center gap-2">
          <SegmentedControl
            value={groupByNode ? "node" : "flat"}
            onChange={(next) => onGroupByNodeChange(next === "node")}
            options={[
              { value: "node", label: "By node" },
              { value: "flat", label: "Flat" },
            ]}
            variant="inset"
            size="xs"
          />
          {canEdit && onBulkDecimals ? (
            <NodeButton size="sm" onClick={onBulkDecimals}>
              Set decimals…
            </NodeButton>
          ) : null}
          {canEdit && onReorder ? (
            <NodeButton size="sm" onClick={onReorder}>
              Reorder…
            </NodeButton>
          ) : null}
          <NodeButton size="sm" onClick={onRefresh} loading={refreshLoading}>
            {refreshLabel ?? "Refresh"}
          </NodeButton>
        </div>
      }
    >

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        <SummaryTile label="Sensors" value={sensorsCount} hint="active" />
        <SummaryTile label="Outputs" value={outputsCount} hint="configured" />
        <SummaryTile label="Predictive alarms" value={predictiveAlarmCount} hint="model-generated" />
        <SummaryTile
          label="Nodes"
          value={`${nodesOnlineCount}/${nodes.length}`}
          hint="online"
        />
      </div>

      <div className="flex flex-wrap gap-3">
        <Select
          value={nodeFilter}
          onChange={(event) => onNodeFilterChange(event.target.value)}
        >
          <option value="all">All nodes</option>
          {nodes.map((node) => (
            <option key={node.id} value={node.id}>
              {node.name}
            </option>
          ))}
        </Select>
        <Select
          value={typeFilter}
          onChange={(event) => onTypeFilterChange(event.target.value)}
        >
          <option value="all">All sensor types</option>
          {sensorTypes.map((type) => (
            <option key={type} value={type}>
              {type}
            </option>
          ))}
        </Select>
        <SegmentedControl
          value={alarmOriginFilter}
          onChange={(next) => onAlarmOriginChange(next as AlarmOriginFilter)}
          options={[
            { value: "all", label: "All alarms" },
            { value: "predictive", label: "Predictive" },
            { value: "standard", label: "Standard" },
          ]}
          variant="inset"
          size="xs"
        />
      </div>
    </PageHeaderCard>
  );
}
