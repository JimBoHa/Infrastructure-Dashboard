import { useQueryClient } from "@tanstack/react-query";
import {
  createAlarmRule,
  deleteAlarmRule,
  disableAlarmRule,
  enableAlarmRule,
  previewAlarmRule,
  updateAlarmRule,
} from "@/lib/api";
import { queryKeys } from "@/lib/queries";
import type {
  AlarmRuleCreateRequest,
  AlarmRulePreviewResponse,
  AlarmRuleUpdateRequest,
} from "@/features/alarms/types/alarmTypes";

export default function useAlarmMutations() {
  const queryClient = useQueryClient();

  const invalidate = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.alarmRules }),
      queryClient.invalidateQueries({ queryKey: queryKeys.alarms }),
      queryClient.invalidateQueries({ queryKey: queryKeys.alarmEvents() }),
    ]);
  };

  return {
    create: async (payload: AlarmRuleCreateRequest) => {
      const response = await createAlarmRule(payload);
      await invalidate();
      return response;
    },
    update: async (id: number, payload: AlarmRuleUpdateRequest) => {
      const response = await updateAlarmRule(id, payload);
      await invalidate();
      return response;
    },
    delete: async (id: number) => {
      await deleteAlarmRule(id);
      await invalidate();
    },
    enable: async (id: number) => {
      const response = await enableAlarmRule(id);
      await invalidate();
      return response;
    },
    disable: async (id: number) => {
      const response = await disableAlarmRule(id);
      await invalidate();
      return response;
    },
    preview: async (payload: AlarmRuleCreateRequest): Promise<AlarmRulePreviewResponse> => {
      return previewAlarmRule(payload);
    },
  };
}
