import { matchesOriginFilter, type AlarmOriginFilter } from "@/lib/alarms/origin";
import type { DemoAlarm } from "@/types/dashboard";

export default function filterAlarmsByOrigin(
  alarms: DemoAlarm[],
  originFilter: AlarmOriginFilter,
): DemoAlarm[] {
  return alarms.filter((alarm) => matchesOriginFilter(alarm.origin ?? alarm.type, originFilter));
}
