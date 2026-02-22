import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { AlarmWizardState } from "@/features/alarms/types/alarmTypes";
import { wizardSummary } from "@/features/alarms/utils/ruleSummary";

export default function WizardStepCondition({
  state,
  sensors,
  nodes,
  onPatch,
}: {
  state: AlarmWizardState;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  onPatch: (partial: Partial<AlarmWizardState>) => void;
}) {
  return (
    <div className="space-y-4">
      <div className="grid gap-4 md:grid-cols-2">
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Template</label>
          <Select
            value={state.template}
            onChange={(event) =>
              onPatch({ template: event.target.value as AlarmWizardState["template"] })
            }
          >
            <option value="threshold">Simple threshold</option>
            <option value="range">Range / band</option>
            <option value="offline">Offline / no data</option>
            <option value="rolling_window">Rolling window aggregate</option>
            <option value="deviation">Deviation from baseline</option>
            <option value="consecutive">Consecutive periods</option>
          </Select>
        </div>
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Target scope</label>
          <Select
            value={state.selectorMode}
            onChange={(event) =>
              onPatch({ selectorMode: event.target.value as AlarmWizardState["selectorMode"] })
            }
          >
            <option value="sensor">Single sensor</option>
            <option value="node">Node sensors</option>
            <option value="filter">Filtered sensors</option>
          </Select>
        </div>
      </div>

      {state.selectorMode === "sensor" ? (
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Sensor</label>
          <Select value={state.sensorId} onChange={(event) => onPatch({ sensorId: event.target.value })}>
            <option value="">Select a sensor</option>
            {sensors.map((sensor) => (
              <option key={sensor.sensor_id} value={sensor.sensor_id}>
                {sensor.name} ({sensor.unit || "unitless"})
              </option>
            ))}
          </Select>
        </div>
      ) : null}

      {state.selectorMode === "node" ? (
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Node</label>
          <Select value={state.nodeId} onChange={(event) => onPatch({ nodeId: event.target.value })}>
            <option value="">Select a node</option>
            {nodes.map((node) => (
              <option key={node.id} value={node.id}>
                {node.name}
              </option>
            ))}
          </Select>
        </div>
      ) : null}

      {state.selectorMode === "filter" ? (
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Provider</label>
            <Input
              value={state.filterProvider}
              onChange={(event) => onPatch({ filterProvider: event.target.value })}
              placeholder="emporia_cloud"
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Metric</label>
            <Input
              value={state.filterMetric}
              onChange={(event) => onPatch({ filterMetric: event.target.value })}
              placeholder="voltage_l1"
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Sensor type</label>
            <Input
              value={state.filterType}
              onChange={(event) => onPatch({ filterType: event.target.value })}
              placeholder="voltage"
            />
          </div>
        </div>
      ) : null}

      {state.template === "threshold" ? (
        <div className="grid gap-4 md:grid-cols-2">
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Operator</label>
            <Select
              value={state.thresholdOp}
              onChange={(event) =>
                onPatch({ thresholdOp: event.target.value as AlarmWizardState["thresholdOp"] })
              }
            >
              <option value="lt">&lt;</option>
              <option value="lte">&lt;=</option>
              <option value="gt">&gt;</option>
              <option value="gte">&gt;=</option>
              <option value="eq">==</option>
              <option value="neq">!=</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Threshold value</label>
            <Input
              value={state.thresholdValue}
              onChange={(event) => onPatch({ thresholdValue: event.target.value })}
              placeholder="15"
            />
          </div>
        </div>
      ) : null}

      {state.template === "range" ? (
        <div className="grid gap-4 md:grid-cols-3">
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Mode</label>
            <Select
              value={state.rangeMode}
              onChange={(event) =>
                onPatch({ rangeMode: event.target.value as AlarmWizardState["rangeMode"] })
              }
            >
              <option value="outside">Outside band</option>
              <option value="inside">Inside band</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Low</label>
            <Input value={state.rangeLow} onChange={(event) => onPatch({ rangeLow: event.target.value })} />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">High</label>
            <Input value={state.rangeHigh} onChange={(event) => onPatch({ rangeHigh: event.target.value })} />
          </div>
        </div>
      ) : null}

      {state.template === "offline" ? (
        <div>
          <label className="text-xs font-semibold text-muted-foreground">Missing for (seconds)</label>
          <Input
            value={state.offlineSeconds}
            onChange={(event) => onPatch({ offlineSeconds: event.target.value })}
            placeholder="5"
          />
        </div>
      ) : null}

      {state.template === "rolling_window" ? (
        <div className="grid gap-4 md:grid-cols-4">
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Window (sec)</label>
            <Input
              value={state.rollingWindowSeconds}
              onChange={(event) => onPatch({ rollingWindowSeconds: event.target.value })}
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Aggregate</label>
            <Select
              value={state.rollingAggregate}
              onChange={(event) =>
                onPatch({ rollingAggregate: event.target.value as AlarmWizardState["rollingAggregate"] })
              }
            >
              <option value="avg">avg</option>
              <option value="min">min</option>
              <option value="max">max</option>
              <option value="stddev">stddev</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Operator</label>
            <Select
              value={state.rollingOp}
              onChange={(event) =>
                onPatch({ rollingOp: event.target.value as AlarmWizardState["rollingOp"] })
              }
            >
              <option value="lt">&lt;</option>
              <option value="lte">&lt;=</option>
              <option value="gt">&gt;</option>
              <option value="gte">&gt;=</option>
              <option value="eq">==</option>
              <option value="neq">!=</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Value</label>
            <Input value={state.rollingValue} onChange={(event) => onPatch({ rollingValue: event.target.value })} />
          </div>
        </div>
      ) : null}

      {state.template === "deviation" ? (
        <div className="grid gap-4 md:grid-cols-4">
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Window (sec)</label>
            <Input
              value={state.deviationWindowSeconds}
              onChange={(event) => onPatch({ deviationWindowSeconds: event.target.value })}
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Baseline</label>
            <Select
              value={state.deviationBaseline}
              onChange={(event) =>
                onPatch({ deviationBaseline: event.target.value as AlarmWizardState["deviationBaseline"] })
              }
            >
              <option value="mean">mean</option>
              <option value="median">median</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Mode</label>
            <Select
              value={state.deviationMode}
              onChange={(event) =>
                onPatch({ deviationMode: event.target.value as AlarmWizardState["deviationMode"] })
              }
            >
              <option value="percent">percent</option>
              <option value="absolute">absolute</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Value</label>
            <Input
              value={state.deviationValue}
              onChange={(event) => onPatch({ deviationValue: event.target.value })}
            />
          </div>
        </div>
      ) : null}

      {state.template === "consecutive" ? (
        <div className="grid gap-4 md:grid-cols-4">
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Base op</label>
            <Select
              value={state.thresholdOp}
              onChange={(event) =>
                onPatch({ thresholdOp: event.target.value as AlarmWizardState["thresholdOp"] })
              }
            >
              <option value="lt">&lt;</option>
              <option value="lte">&lt;=</option>
              <option value="gt">&gt;</option>
              <option value="gte">&gt;=</option>
              <option value="eq">==</option>
              <option value="neq">!=</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Base value</label>
            <Input
              value={state.thresholdValue}
              onChange={(event) => onPatch({ thresholdValue: event.target.value })}
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Period</label>
            <Select
              value={state.consecutivePeriod}
              onChange={(event) =>
                onPatch({ consecutivePeriod: event.target.value as AlarmWizardState["consecutivePeriod"] })
              }
            >
              <option value="eval">Evaluation cycle</option>
              <option value="hour">Hour</option>
              <option value="day">Day</option>
            </Select>
          </div>
          <div>
            <label className="text-xs font-semibold text-muted-foreground">Count</label>
            <Input
              value={state.consecutiveCount}
              onChange={(event) => onPatch({ consecutiveCount: event.target.value })}
            />
          </div>
        </div>
      ) : null}

      <p className="rounded-lg bg-card-inset px-3 py-2 text-xs text-muted-foreground">{wizardSummary(state)}</p>
    </div>
  );
}
