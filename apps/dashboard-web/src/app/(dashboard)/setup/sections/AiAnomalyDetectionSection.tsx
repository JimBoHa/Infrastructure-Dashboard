"use client";

import { useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import NodeButton from "@/features/nodes/components/NodeButton";
import { updatePredictiveConfig } from "@/lib/api";
import { queryKeys, usePredictiveStatusQuery } from "@/lib/queries";

import type { Message } from "../types";

export default function AiAnomalyDetectionSection({
  onMessage,
}: {
  onMessage: (message: Message) => void;
}) {
  const queryClient = useQueryClient();
  const predictiveQuery = usePredictiveStatusQuery();
  const [predictiveEnabledDraft, setPredictiveEnabledDraft] = useState<boolean | null>(null);
  const [predictiveBaseUrlDraft, setPredictiveBaseUrlDraft] = useState<string | null>(null);
  const [predictiveModelDraft, setPredictiveModelDraft] = useState<string | null>(null);
  const [predictiveTokenDraft, setPredictiveTokenDraft] = useState("");

  const predictiveEnabled = predictiveEnabledDraft ?? Boolean(predictiveQuery.data?.enabled);
  const predictiveBaseUrl = useMemo(
    () => predictiveBaseUrlDraft ?? (predictiveQuery.data?.api_base_url ?? ""),
    [predictiveBaseUrlDraft, predictiveQuery.data?.api_base_url],
  );
  const predictiveModel = useMemo(
    () => predictiveModelDraft ?? (predictiveQuery.data?.model ?? ""),
    [predictiveModelDraft, predictiveQuery.data?.model],
  );

  const savePredictive = async () => {
    if (!predictiveBaseUrl.trim()) {
      onMessage({ type: "error", text: "Enter an API base URL for predictive alarms." });
      return;
    }

    try {
      await updatePredictiveConfig({
        enabled: predictiveEnabled,
        api_base_url: predictiveBaseUrl.trim(),
        model: predictiveModel.trim() ? predictiveModel.trim() : null,
        ...(predictiveTokenDraft.trim() ? { api_token: predictiveTokenDraft.trim() } : {}),
      });
      setPredictiveTokenDraft("");
      void queryClient.invalidateQueries({ queryKey: queryKeys.predictiveStatus });
      onMessage({ type: "success", text: "Predictive alarm settings updated." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to update predictive alarm settings.";
      onMessage({ type: "error", text });
    }
  };

  const clearPredictiveToken = async () => {
    try {
      await updatePredictiveConfig({ api_token: "" });
      setPredictiveTokenDraft("");
      void queryClient.invalidateQueries({ queryKey: queryKeys.predictiveStatus });
      onMessage({ type: "success", text: "Predictive alarm token cleared." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to clear predictive alarm token.";
      onMessage({ type: "error", text });
    }
  };

  return (
    <CollapsibleCard
      title="AI anomaly detection"
      description="Optional LLM-powered alarm trend analysis. Default is disabled on fresh installs."
      defaultOpen={false}
      bodyClassName="space-y-4"
      actions={
        <NodeButton onClick={() => queryClient.invalidateQueries({ queryKey: queryKeys.predictiveStatus })}>
          Refresh status
        </NodeButton>
      }
    >
      <div className="grid gap-4 md:grid-cols-2">
        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-sm font-semibold text-card-foreground">
                Enable predictive alarms
              </p>
 <p className="text-xs text-muted-foreground">
                When enabled, the core server will score telemetry and raise predictive alarms.
              </p>
            </div>
 <label className="flex items-center gap-2 text-sm text-muted-foreground">
              <input
                type="checkbox"
 className="rounded border-input text-indigo-600 focus:ring-indigo-500"
                checked={predictiveEnabled}
                onChange={(event) => setPredictiveEnabledDraft(event.target.checked)}
              />
 <span className="text-foreground">
                {predictiveEnabled ? "On" : "Off"}
              </span>
            </label>
          </div>
          <div className="mt-3 flex items-center justify-between text-xs">
 <span className="text-muted-foreground">
              Status: {predictiveQuery.data?.running ? "running" : "stopped"}
            </span>
            <span
              className={`font-semibold ${
                predictiveQuery.data?.token_present
 ? "text-emerald-600"
 : "text-amber-600"
              }`}
            >
              Token {predictiveQuery.data?.token_present ? "configured" : "optional / missing"}
            </span>
          </div>
        </Card>

        <Card className="rounded-lg gap-0 bg-card-inset p-4">
          <p className="text-sm font-semibold text-card-foreground">
            Endpoint presets
          </p>
 <p className="text-xs text-muted-foreground">
            Local endpoints (Ollama / LM Studio) usually work without a token. Remote providers require a token.
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <NodeButton
              size="xs"
              onClick={() => setPredictiveBaseUrlDraft("http://127.0.0.1:11434/v1")}
            >
              Ollama
            </NodeButton>
            <NodeButton
              size="xs"
              onClick={() => setPredictiveBaseUrlDraft("http://127.0.0.1:1234/v1")}
            >
              LM Studio
            </NodeButton>
            <NodeButton
              size="xs"
              onClick={() => setPredictiveBaseUrlDraft("https://models.github.ai/inference")}
            >
              GitHub Models
            </NodeButton>
            <NodeButton size="xs" onClick={() => setPredictiveBaseUrlDraft("https://api.openai.com/v1")}>
              OpenAI
            </NodeButton>
          </div>
        </Card>

        <Card className="rounded-lg gap-0 bg-card-inset p-4 md:col-span-2">
          <div className="grid gap-3 md:grid-cols-2">
            <div className="md:col-span-2">
 <label className="block text-xs font-semibold text-muted-foreground">
                API base URL
              </label>
              <Input
                type="text"
                placeholder="http://127.0.0.1:11434/v1"
                className="mt-1"
                value={predictiveBaseUrl}
                onChange={(event) => setPredictiveBaseUrlDraft(event.target.value)}
              />
            </div>
            <div>
 <label className="block text-xs font-semibold text-muted-foreground">
                Model
              </label>
              <Input
                type="text"
                placeholder="llama3 (or gpt-4.1)"
                className="mt-1"
                value={predictiveModel}
                onChange={(event) => setPredictiveModelDraft(event.target.value)}
              />
            </div>
            <div>
 <label className="block text-xs font-semibold text-muted-foreground">
                API token (optional)
              </label>
              <Input
                type="password"
                placeholder="Leave blank to keep existing"
                className="mt-1"
                value={predictiveTokenDraft}
                onChange={(event) => setPredictiveTokenDraft(event.target.value)}
              />
            </div>
          </div>

          <div className="mt-4 flex flex-wrap gap-2">
            <NodeButton variant="primary" onClick={savePredictive}>
              Save
            </NodeButton>
            <NodeButton onClick={clearPredictiveToken}>Clear token</NodeButton>
          </div>
        </Card>
      </div>
    </CollapsibleCard>
  );
}
