import { useMemo } from "react";
import {
  actionPayloadToForm,
  conditionPayloadToForm,
  hasFieldErrors,
  isBlank,
  parseTimeToMinutes,
  ScheduleDraft,
  Toast,
  validateActionForm,
  validateConditionForm,
} from "@/features/schedules/lib/scheduleUtils";

export function useScheduleForm({
  draft,
  onChange,
  onSave,
  notify,
}: {
  draft: ScheduleDraft;
  onChange: (draft: ScheduleDraft | null) => void;
  onSave: () => void;
  notify: (toast: Toast) => void;
}) {
  const patchDraft = (patch: Partial<ScheduleDraft>) => {
    onChange({ ...draft, ...patch });
  };

  const timingErrors = useMemo(() => {
    const errors: { start?: string; end?: string } = {};
    const startMinutes = parseTimeToMinutes(draft.start);
    const endMinutes = parseTimeToMinutes(draft.end);

    if (isBlank(draft.start) || startMinutes == null) errors.start = "Start time is required.";
    if (isBlank(draft.end) || endMinutes == null) errors.end = "End time is required.";

    if (startMinutes != null && endMinutes != null && startMinutes >= endMinutes) {
      errors.end = "End must be after start.";
    }
    return errors;
  }, [draft.start, draft.end]);

  const conditionsHaveErrors = useMemo(() => {
    if (draft.conditionsMode !== "form") return false;
    return draft.conditionsList.some((condition) => {
      const formValue = conditionPayloadToForm(condition);
      if (!formValue) return false;
      return hasFieldErrors(validateConditionForm(formValue) as Record<string, string | undefined>);
    });
  }, [draft.conditionsList, draft.conditionsMode]);

  const actionsHaveErrors = useMemo(() => {
    if (draft.actionsMode !== "form") return false;
    return draft.actionsList.some((action) => {
      const formValue = actionPayloadToForm(action);
      if (!formValue) return false;
      return hasFieldErrors(validateActionForm(formValue) as Record<string, string | undefined>);
    });
  }, [draft.actionsList, draft.actionsMode]);

  const draftHasErrors = hasFieldErrors(timingErrors) || conditionsHaveErrors || actionsHaveErrors;

  const handleSave = () => {
    if (!draft.showValidation) {
      patchDraft({ showValidation: true });
    }
    if (draftHasErrors) {
      notify({ type: "error", text: "Fix the highlighted fields before saving." });
      return;
    }
    onSave();
  };

  return {
    patchDraft,
    showValidation: draft.showValidation,
    timingErrors,
    handleSave,
  };
}

