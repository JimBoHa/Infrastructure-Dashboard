"use client";

import { useMemo, useState } from "react";
import PageHeaderCard from "@/components/PageHeaderCard";
import InlineBanner from "@/components/InlineBanner";
import LoadingState from "@/components/LoadingState";
import ErrorState from "@/components/ErrorState";
import CollapsibleCard from "@/components/CollapsibleCard";
import SegmentedControl from "@/components/SegmentedControl";
import NodeButton from "@/features/nodes/components/NodeButton";
import { useAuth } from "@/components/AuthProvider";
import { useAlarmRulesQuery, useNodesQuery, useSensorsQuery, useUsersQuery } from "@/lib/queries";
import type { AlarmRule } from "@/features/alarms/types/alarmTypes";
import AlarmCard from "@/features/alarms/components/AlarmCard";
import AlarmWizard from "@/features/alarms/components/AlarmWizard";
import useAlarmWizard from "@/features/alarms/hooks/useAlarmWizard";
import useAlarmMutations from "@/features/alarms/hooks/useAlarmMutations";
import AlarmHistoryPanel from "@/features/alarms/components/AlarmHistoryPanel";
import RuleHealthPanel from "@/features/alarms/components/RuleHealthPanel";
import IncidentsConsole from "@/features/incidents/components/IncidentsConsole";

const severityOrder: Record<string, number> = {
  critical: 0,
  warning: 1,
  info: 2,
};

export default function AlarmsPageClient() {
  const { me } = useAuth();
  const canEdit = Boolean(me?.capabilities?.includes("config.write"));
  const canAck = Boolean(me?.capabilities?.includes("alerts.ack"));
  const canManageUsers = Boolean(me?.capabilities?.includes("users.manage"));
  const meUserId = me?.id ?? null;

  const [view, setView] = useState<"incidents" | "rules">("incidents");

  const rulesQuery = useAlarmRulesQuery();
  const nodesQuery = useNodesQuery();
  const sensorsQuery = useSensorsQuery();
  const usersQuery = useUsersQuery({ enabled: canManageUsers });

  const wizard = useAlarmWizard();
  const mutations = useAlarmMutations();

  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [saving, setSaving] = useState(false);

  const isLoading =
    rulesQuery.isLoading ||
    nodesQuery.isLoading ||
    sensorsQuery.isLoading ||
    (canManageUsers && usersQuery.isLoading);
  const error =
    rulesQuery.error ||
    nodesQuery.error ||
    sensorsQuery.error ||
    (canManageUsers ? usersQuery.error : null);

  const rules = useMemo(() => {
    const list = (rulesQuery.data ?? []) as AlarmRule[];
    return [...list].sort((a, b) => {
      const aOrder = severityOrder[a.severity] ?? 99;
      const bOrder = severityOrder[b.severity] ?? 99;
      if (aOrder !== bOrder) return aOrder - bOrder;
      return a.name.localeCompare(b.name);
    });
  }, [rulesQuery.data]);

  const nodes = nodesQuery.data ?? [];
  const sensors = sensorsQuery.data ?? [];
  const users = usersQuery.data ?? [];

  const handleSave = async (
    payload: Parameters<typeof mutations.create>[0],
    mode: "create" | "edit",
    id?: number,
  ) => {
    setSaving(true);
    try {
      if (mode === "edit" && id != null) {
        await mutations.update(id, payload);
        setMessage({ type: "success", text: "Alarm rule updated." });
      } else {
        await mutations.create(payload);
        setMessage({ type: "success", text: "Alarm rule created." });
      }
      wizard.reset();
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Failed to save alarm rule.",
      });
      throw err;
    } finally {
      setSaving(false);
    }
  };

  const handleToggle = async (rule: AlarmRule) => {
    if (!canEdit) return;
    try {
      if (rule.enabled) {
        await mutations.disable(rule.id);
      } else {
        await mutations.enable(rule.id);
      }
      setMessage({
        type: "success",
        text: `Alarm rule ${rule.enabled ? "disabled" : "enabled"}.`,
      });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Failed to update alarm rule.",
      });
    }
  };

  const handleDelete = async (rule: AlarmRule) => {
    if (!canEdit) return;
    try {
      await mutations.delete(rule.id);
      setMessage({ type: "success", text: "Alarm rule deleted." });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Failed to delete alarm rule.",
      });
    }
  };

  if (isLoading) return <LoadingState label="Loading alarms..." />;
  if (error) {
    return <ErrorState message={error instanceof Error ? error.message : "Failed to load alarms."} />;
  }

  return (
    <div className="space-y-5">
      <PageHeaderCard
        title="Alarms"
        description="Triage incidents, investigate related signals, and tune alarm rules with guidance + backtests."
        actions={
          canEdit ? (
            <NodeButton variant="primary" onClick={() => wizard.openCreate()}>
              Create alarm
            </NodeButton>
          ) : undefined
        }
      />

      {message ? (
        <InlineBanner tone={message.type === "success" ? "success" : "error"}>{message.text}</InlineBanner>
      ) : null}

      <CardTabs value={view} onChange={setView} />

      {view === "incidents" ? (
        <IncidentsConsole
          canEdit={canEdit}
          canAck={canAck}
          meUserId={meUserId}
          sensors={sensors}
          nodes={nodes}
          users={users}
        />
      ) : (
        <div className="space-y-5">
          <CollapsibleCard title="Rule library" description={`${rules.length} configured rules`} defaultOpen>
            {rules.length ? (
              <div className="grid gap-3 xl:grid-cols-2">
                {rules.map((rule) => (
                  <AlarmCard
                    key={rule.id}
                    rule={rule}
                    canEdit={canEdit}
                    onEdit={(next) => wizard.openEdit(next)}
                    onDuplicate={(next) => wizard.openDuplicate(next)}
                    onToggle={(next) => void handleToggle(next)}
                    onDelete={(next) => void handleDelete(next)}
                  />
                ))}
              </div>
            ) : (
              <p className="text-sm text-muted-foreground">No alarm rules yet. Create one to start monitoring.</p>
            )}
          </CollapsibleCard>

          <RuleHealthPanel rules={rules} />

          <AlarmHistoryPanel />
        </div>
      )}

      <AlarmWizard
        open={wizard.open}
        onOpenChange={(next) => {
          wizard.setOpen(next);
          if (!next) wizard.reset();
        }}
        step={wizard.step}
        onStepChange={wizard.setStep}
        state={wizard.state}
        onPatch={wizard.patch}
        sensors={sensors}
        nodes={nodes}
        canAdvance={wizard.canAdvance}
        saving={saving}
        onSave={handleSave}
        onPreview={mutations.preview}
      />
    </div>
  );
}

function CardTabs({
  value,
  onChange,
}: {
  value: "incidents" | "rules";
  onChange: (next: "incidents" | "rules") => void;
}) {
  return (
    <div className="flex items-center justify-between">
      <SegmentedControl
        value={value}
        options={[
          { value: "incidents", label: "Incidents" },
          { value: "rules", label: "Rules" },
        ]}
        onChange={(next) => onChange(next as "incidents" | "rules")}
      />
    </div>
  );
}
