import filterAlarmsByOrigin from "@/features/sensors/utils/filterAlarmsByOrigin";
import { matchesOriginFilter } from "@/lib/alarms/origin";
import type { DemoAlarm } from "@/types/dashboard";

const sampleAlarms: DemoAlarm[] = [
  {
    id: "a1",
    name: "Standard Threshold",
    type: "threshold",
    severity: "warning",
    target_type: "sensor",
    target_id: "sensor-1",
    condition: { type: "threshold" },
    active: true,
    origin: "threshold",
  },
  {
    id: "a2",
    name: "Predictive",
    type: "predictive",
    severity: "warning",
    target_type: "sensor",
    target_id: "sensor-1",
    condition: { type: "predictive" },
    active: true,
    origin: "predictive",
    anomaly_score: 0.82,
  },
];

describe("alarm origin helpers", () => {
  it("filters alarms by origin", () => {
    expect(filterAlarmsByOrigin(sampleAlarms, "all")).toHaveLength(2);
    expect(filterAlarmsByOrigin(sampleAlarms, "predictive")).toEqual([sampleAlarms[1]]);
    expect(filterAlarmsByOrigin(sampleAlarms, "standard")).toEqual([sampleAlarms[0]]);
  });

  it("matchesOriginFilter respects defaults", () => {
    expect(matchesOriginFilter("threshold", "standard")).toBe(true);
    expect(matchesOriginFilter("threshold", "predictive")).toBe(false);
    expect(matchesOriginFilter(null, "predictive")).toBe(false);
  });
});
