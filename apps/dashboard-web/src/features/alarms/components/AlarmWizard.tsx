import { useMemo, useState } from "react";
import {
  Sheet,
  SheetBody,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import NodeButton from "@/features/nodes/components/NodeButton";
import InlineBanner from "@/components/InlineBanner";
import WizardStepBasics from "@/features/alarms/components/WizardStepBasics";
import WizardStepCondition from "@/features/alarms/components/WizardStepCondition";
import WizardStepGuidance from "@/features/alarms/components/WizardStepGuidance";
import WizardStepBacktest from "@/features/alarms/components/WizardStepBacktest";
import WizardStepAdvanced from "@/features/alarms/components/WizardStepAdvanced";
import { buildRequestFromWizard } from "@/features/alarms/utils/ruleBuilder";
import type {
  AlarmRuleCreateRequest,
  AlarmRulePreviewResponse,
  AlarmWizardState,
} from "@/features/alarms/types/alarmTypes";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

const stepTitle = (step: number) => {
  if (step === 1) return "Basics";
  if (step === 2) return "Target & condition";
  if (step === 3) return "Guidance";
  if (step === 4) return "Backtest";
  return "Review";
};

export default function AlarmWizard({
  open,
  onOpenChange,
  step,
  onStepChange,
  state,
  onPatch,
  sensors,
  nodes,
  canAdvance,
  saving,
  onSave,
  onPreview,
}: {
  open: boolean;
  onOpenChange: (next: boolean) => void;
  step: number;
  onStepChange: (step: number) => void;
  state: AlarmWizardState;
  onPatch: (partial: Partial<AlarmWizardState>) => void;
  sensors: DemoSensor[];
  nodes: DemoNode[];
  canAdvance: boolean;
  saving: boolean;
  onSave: (payload: AlarmRuleCreateRequest, mode: "create" | "edit", id?: number) => Promise<void>;
  onPreview: (payload: AlarmRuleCreateRequest) => Promise<AlarmRulePreviewResponse>;
}) {
  const [error, setError] = useState<string | null>(null);
  const [previewBusy, setPreviewBusy] = useState(false);
  const [preview, setPreview] = useState<AlarmRulePreviewResponse | null>(null);

  const payload = useMemo(() => {
    try {
      return buildRequestFromWizard(state);
    } catch {
      return null;
    }
  }, [state]);

  const submit = async () => {
    setError(null);
    if (!payload) {
      setError("Invalid rule payload. Fix JSON or required fields.");
      return;
    }
    try {
      await onSave(payload, state.mode, state.ruleId);
      onOpenChange(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save alarm rule.");
    }
  };

  const runPreview = async () => {
    setError(null);
    setPreview(null);
    if (!payload) {
      setError("Invalid rule payload. Fix JSON or required fields.");
      return;
    }
    setPreviewBusy(true);
    try {
      const response = await onPreview(payload);
      setPreview(response);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Preview failed.");
    } finally {
      setPreviewBusy(false);
    }
  };

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent>
        <SheetHeader>
          <div>
            <SheetTitle>{state.mode === "create" ? "Create alarm" : "Edit alarm"}</SheetTitle>
            <SheetDescription>
              Step {step}/5: {stepTitle(step)}
            </SheetDescription>
          </div>
        </SheetHeader>
        <SheetBody className="space-y-4">
          {error ? <InlineBanner tone="error">{error}</InlineBanner> : null}

          {step === 1 ? <WizardStepBasics state={state} onPatch={onPatch} /> : null}
          {step === 2 ? (
            <WizardStepCondition state={state} sensors={sensors} nodes={nodes} onPatch={onPatch} />
          ) : null}
          {step === 3 ? (
            <WizardStepGuidance state={state} onPatch={onPatch} payload={payload} sensors={sensors} />
          ) : null}
          {step === 4 ? <WizardStepBacktest payload={payload} /> : null}
          {step === 5 ? <WizardStepAdvanced state={state} onPatch={onPatch} /> : null}

          {preview ? (
            <div className="rounded-xl border border-border bg-card-inset p-3">
              <p className="text-sm font-semibold text-card-foreground">Preview</p>
              <p className="text-xs text-muted-foreground">
                Targets evaluated: {preview.targets_evaluated}
              </p>
              <div className="mt-2 max-h-48 overflow-y-auto text-xs">
                {preview.results.map((result) => (
                  <div key={result.target_key} className="border-b border-border py-2 last:border-b-0">
                    <p className="font-semibold text-card-foreground">{result.target_key}</p>
                    <p className="text-muted-foreground">
                      {result.passed ? "Would fire" : "Would not fire"}
                      {result.observed_value != null ? ` · observed=${result.observed_value}` : ""}
                    </p>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </SheetBody>

        <div className="flex items-center justify-between border-t border-border px-6 py-4">
          <div className="flex items-center gap-2">
            <NodeButton size="sm" onClick={() => onStepChange(Math.max(1, step - 1))} disabled={step === 1}>
              Back
            </NodeButton>
            <NodeButton
              size="sm"
              onClick={() => onStepChange(Math.min(5, step + 1))}
              disabled={step === 5 || !canAdvance}
            >
              Next
            </NodeButton>
          </div>

          <div className="flex items-center gap-2">
            <NodeButton size="sm" onClick={() => void runPreview()} loading={previewBusy}>
              Preview
            </NodeButton>
            <NodeButton variant="primary" size="sm" onClick={() => void submit()} loading={saving}>
              {state.mode === "create" ? "Create alarm" : "Save changes"}
            </NodeButton>
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}
