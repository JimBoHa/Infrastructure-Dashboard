"use client";

import { Fragment, useEffect, useMemo, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";

import CollapsibleCard from "@/components/CollapsibleCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import NodeButton from "@/features/nodes/components/NodeButton";
import {
  createExternalDevice,
  deleteExternalDevice,
  deleteSetupCredential,
  loginEmporia,
  postJson,
  syncExternalDevice,
  upsertSetupCredential,
  updateEmporiaDevices,
} from "@/lib/api";
import {
  queryKeys,
  useEmporiaDevicesQuery,
  useSetupCredentialsQuery,
  useExternalDeviceCatalogQuery,
  useExternalDevicesQuery,
} from "@/lib/queries";
import type { EmporiaDeviceUpdate, SetupCredential } from "@/types/setup";

import type { Message } from "../types";

type CredentialDefinition = {
  name: string;
  label: string;
  hint: string;
};

const credentialCatalog: CredentialDefinition[] = [
  {
    name: "emporia",
    label: "Emporia Cloud",
    hint: "Cloud API token for Emporia (or use the username/password flow above).",
  },
  {
    name: "tesla",
    label: "Tesla Energy",
    hint: "Tesla energy API token for battery/solar monitoring.",
  },
  {
    name: "enphase",
    label: "Enphase",
    hint: "Enphase token for solar production monitoring.",
  },
  {
    name: "renogy",
    label: "Renogy",
    hint: "Renogy credentials for charge controller telemetry.",
  },
];

type EmporiaCircuitEdit = {
  enabled: boolean;
  hidden: boolean;
  include_in_power_summary: boolean;
};

type EmporiaDeviceEdit = {
  enabled: boolean;
  hidden: boolean;
  include_in_power_summary: boolean;
  group_label: string;
  circuits: Record<string, EmporiaCircuitEdit>;
};

export default function IntegrationsSection({
  canEdit,
  onMessage,
}: {
  canEdit: boolean;
  onMessage: (message: Message) => void;
}) {
  const queryClient = useQueryClient();
  const credentialsQuery = useSetupCredentialsQuery();
  const emporiaDevicesQuery = useEmporiaDevicesQuery();
  const [credentialDrafts, setCredentialDrafts] = useState<Record<string, string>>({});
  const [emporiaUsername, setEmporiaUsername] = useState("");
  const [emporiaPassword, setEmporiaPassword] = useState("");
  const [emporiaSites, setEmporiaSites] = useState<string[]>([]);
  const [isEmporiaLoading, setIsEmporiaLoading] = useState(false);
  const [emporiaDeviceEdits, setEmporiaDeviceEdits] = useState<Record<string, EmporiaDeviceEdit>>(
    {},
  );
  const [expandedEmporiaMeters, setExpandedEmporiaMeters] = useState<Record<string, boolean>>({});
  const [isEmporiaPrefsSaving, setIsEmporiaPrefsSaving] = useState(false);
  const externalCatalogQuery = useExternalDeviceCatalogQuery();
  const externalDevicesQuery = useExternalDevicesQuery();
  const [externalDraft, setExternalDraft] = useState({
    name: "",
    vendorId: "",
    modelId: "",
    protocol: "",
    host: "",
    port: "",
    unitId: "",
    snmpCommunity: "public",
    httpBaseUrl: "",
    lipUsername: "",
    lipPassword: "",
    lipIntegrationReport: "",
    leapClientCert: "",
    leapClientKey: "",
    leapCaCert: "",
    leapVerifyCa: true,
  });
  const [showLeapHelp, setShowLeapHelp] = useState(false);
  const [leapHelpAcknowledged, setLeapHelpAcknowledged] = useState(false);

  const credentialByName = useMemo(() => {
    const map = new Map<string, SetupCredential>();
    (credentialsQuery.data ?? []).forEach((entry) => map.set(entry.name, entry));
    return map;
  }, [credentialsQuery.data]);

  const externalVendors = useMemo(() => {
    const vendors = externalCatalogQuery.data?.vendors ?? [];
    return vendors;
  }, [externalCatalogQuery.data]);

  const selectedVendor = useMemo(() => {
    return externalVendors.find((vendor) => vendor.id === externalDraft.vendorId) ?? null;
  }, [externalDraft.vendorId, externalVendors]);

  const selectedModel = useMemo(() => {
    return selectedVendor?.models.find((model) => model.id === externalDraft.modelId) ?? null;
  }, [selectedVendor, externalDraft.modelId]);

  useEffect(() => {
    if (!externalCatalogQuery.data) return;
    if (!externalDraft.vendorId && externalVendors.length) {
      const firstVendor = externalVendors[0];
      const firstModel = firstVendor.models[0];
      setExternalDraft((prev) => ({
        ...prev,
        vendorId: firstVendor.id,
        modelId: firstModel?.id ?? "",
        protocol: firstModel?.protocols?.[0] ?? "",
      }));
    }
  }, [externalCatalogQuery.data, externalDraft.vendorId, externalVendors]);

  const requireCanEdit = () => {
    if (canEdit) return true;
    onMessage({ type: "error", text: "This action requires the config.write capability." });
    return false;
  };

  useEffect(() => {
    const payload = emporiaDevicesQuery.data;
    if (!payload?.token_present) {
      setEmporiaDeviceEdits({});
      setExpandedEmporiaMeters({});
      return;
    }

    setEmporiaDeviceEdits((prev) => {
      const next: Record<string, EmporiaDeviceEdit> = { ...prev };
      payload.devices.forEach((device) => {
        const detectedGroup = (device.address ?? device.name ?? "").trim();
        const storedGroup = (device.group_label ?? "").trim();
        const groupLabelDefault =
          storedGroup && detectedGroup && storedGroup === detectedGroup ? "" : storedGroup;
        const existing = next[device.device_gid];
        const circuits = Array.isArray(device.circuits) ? device.circuits : [];

        if (!existing) {
          const circuitEdits: Record<string, EmporiaCircuitEdit> = {};
          circuits.forEach((circuit) => {
            circuitEdits[circuit.circuit_key] = {
              enabled: circuit.enabled,
              hidden: circuit.hidden,
              include_in_power_summary: circuit.include_in_power_summary,
            };
          });

          next[device.device_gid] = {
            enabled: device.enabled,
            hidden: Boolean(device.hidden),
            include_in_power_summary: device.include_in_power_summary,
            group_label: groupLabelDefault,
            circuits: circuitEdits,
          };
          return;
        }

        const circuitEdits: Record<string, EmporiaCircuitEdit> = { ...existing.circuits };
        circuits.forEach((circuit) => {
          if (circuitEdits[circuit.circuit_key]) return;
          circuitEdits[circuit.circuit_key] = {
            enabled: circuit.enabled,
            hidden: circuit.hidden,
            include_in_power_summary: circuit.include_in_power_summary,
          };
        });

        next[device.device_gid] = {
          ...existing,
          circuits: circuitEdits,
        };
      });

      return next;
    });
  }, [emporiaDevicesQuery.data]);

  const emporiaPrefsDirty = useMemo(() => {
    const payload = emporiaDevicesQuery.data;
    if (!payload?.token_present) return false;
    return payload.devices.some((device) => {
      const edit = emporiaDeviceEdits[device.device_gid];
      if (!edit) return false;
      const detectedGroup = (device.address ?? device.name ?? "").trim() || null;
      const groupLabelTrimmed = edit.group_label.trim();
      const desiredGroupLabel = groupLabelTrimmed ? groupLabelTrimmed : detectedGroup;
      const originalGroupLabel = (device.group_label ?? "").trim() || detectedGroup;
      if (
        edit.enabled !== device.enabled ||
        edit.hidden !== Boolean(device.hidden) ||
        edit.include_in_power_summary !== device.include_in_power_summary ||
        desiredGroupLabel !== originalGroupLabel
      ) {
        return true;
      }

      const circuits = Array.isArray(device.circuits) ? device.circuits : [];
      return circuits.some((circuit) => {
        const circuitEdit = edit.circuits[circuit.circuit_key];
        if (!circuitEdit) return false;
        return (
          circuitEdit.enabled !== circuit.enabled ||
          circuitEdit.hidden !== circuit.hidden ||
          circuitEdit.include_in_power_summary !== circuit.include_in_power_summary
        );
      });
    });
  }, [emporiaDeviceEdits, emporiaDevicesQuery.data]);

  const saveCredential = async (name: string) => {
    if (!requireCanEdit()) return;
    const value = credentialDrafts[name];
    if (!value) {
      onMessage({ type: "error", text: `Enter a value for ${name} before saving.` });
      return;
    }
    try {
      await upsertSetupCredential(name, value, { label: name });
      setCredentialDrafts((prev) => ({ ...prev, [name]: "" }));
      void queryClient.invalidateQueries({ queryKey: queryKeys.setupCredentials });
      onMessage({ type: "success", text: `${name} credential saved.` });
    } catch (err) {
      const text = err instanceof Error ? err.message : `Failed to save ${name} credential.`;
      onMessage({ type: "error", text });
    }
  };

  const clearCredential = async (name: string) => {
    if (!requireCanEdit()) return;
    try {
      await deleteSetupCredential(name);
      void queryClient.invalidateQueries({ queryKey: queryKeys.setupCredentials });
      onMessage({ type: "success", text: `${name} credential cleared.` });
    } catch (err) {
      const text = err instanceof Error ? err.message : `Failed to clear ${name} credential.`;
      onMessage({ type: "error", text });
    }
  };

  const createExternalDeviceEntry = async () => {
    if (!requireCanEdit()) return;
    if (!externalDraft.name.trim()) {
      onMessage({ type: "error", text: "Enter a name for the device." });
      return;
    }
    if (!externalDraft.vendorId || !externalDraft.modelId || !externalDraft.protocol) {
      onMessage({ type: "error", text: "Select a vendor, model, and protocol." });
      return;
    }
    const host = externalDraft.host.trim();
    if (!host && externalDraft.protocol !== "http_json") {
      onMessage({ type: "error", text: "Enter a host/IP for this device." });
      return;
    }
    if (externalDraft.protocol === "http_json" && !externalDraft.httpBaseUrl.trim()) {
      onMessage({ type: "error", text: "Enter an HTTP base URL for this device." });
      return;
    }
    if (externalDraft.protocol === "lutron_lip" && !externalDraft.lipIntegrationReport.trim()) {
      onMessage({
        type: "error",
        text: "Paste the LIP integration report so Infrastructure Dashboard can auto-map outputs.",
      });
      return;
    }
    if (externalDraft.protocol === "lutron_leap") {
      if (!leapHelpAcknowledged) {
        setShowLeapHelp(true);
        return;
      }
      if (!externalDraft.leapClientCert.trim() || !externalDraft.leapClientKey.trim()) {
        onMessage({
          type: "error",
          text: "Provide LEAP client cert and key PEMs to connect.",
        });
        return;
      }
      if (externalDraft.leapVerifyCa && !externalDraft.leapCaCert.trim()) {
        onMessage({
          type: "error",
          text: "Provide the LEAP bridge CA cert PEM (or disable verification).",
        });
        return;
      }
    }
    try {
      await createExternalDevice({
        name: externalDraft.name.trim(),
        vendor_id: externalDraft.vendorId,
        model_id: externalDraft.modelId,
        protocol: externalDraft.protocol,
        host: host || null,
        port: externalDraft.port ? Number(externalDraft.port) : null,
        unit_id: externalDraft.unitId ? Number(externalDraft.unitId) : null,
        snmp_community: externalDraft.snmpCommunity?.trim() || null,
        http_base_url: externalDraft.httpBaseUrl?.trim() || null,
        lip_username: externalDraft.lipUsername?.trim() || null,
        lip_password: externalDraft.lipPassword?.trim() || null,
        lip_integration_report: externalDraft.lipIntegrationReport?.trim() || null,
        leap_client_cert_pem: externalDraft.leapClientCert?.trim() || null,
        leap_client_key_pem: externalDraft.leapClientKey?.trim() || null,
        leap_ca_pem: externalDraft.leapCaCert?.trim() || null,
        leap_verify_ca: externalDraft.leapVerifyCa,
      });
      setExternalDraft((prev) => ({
        ...prev,
        name: "",
        host: "",
        port: "",
        unitId: "",
        lipUsername: "",
        lipPassword: "",
        lipIntegrationReport: "",
        leapClientCert: "",
        leapClientKey: "",
        leapCaCert: "",
        leapVerifyCa: true,
      }));
      void queryClient.invalidateQueries({ queryKey: queryKeys.externalDevices });
      onMessage({ type: "success", text: "External device added." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to add external device.";
      onMessage({ type: "error", text });
    }
  };

  const syncExternalDeviceEntry = async (nodeId: string) => {
    if (!requireCanEdit()) return;
    try {
      await syncExternalDevice(nodeId);
      void queryClient.invalidateQueries({ queryKey: queryKeys.externalDevices });
      onMessage({ type: "success", text: "External device sync triggered." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to sync external device.";
      onMessage({ type: "error", text });
    }
  };

  const deleteExternalDeviceEntry = async (nodeId: string) => {
    if (!requireCanEdit()) return;
    try {
      await deleteExternalDevice(nodeId);
      void queryClient.invalidateQueries({ queryKey: queryKeys.externalDevices });
      onMessage({ type: "success", text: "External device removed." });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to remove external device.";
      onMessage({ type: "error", text });
    }
  };

  const loginEmporiaCloud = async () => {
    if (!requireCanEdit()) return;
    if (!emporiaUsername.trim() || !emporiaPassword.trim()) {
      onMessage({ type: "error", text: "Enter Emporia username and password to derive a token." });
      return;
    }
    setIsEmporiaLoading(true);
    try {
      const result = await loginEmporia({
        username: emporiaUsername.trim(),
        password: emporiaPassword,
      });
      setEmporiaSites(result.site_ids ?? []);
      setEmporiaPassword("");
      onMessage({
        type: "success",
        text: `Emporia token saved for ${result.site_ids.length} site(s).`,
      });
      void queryClient.invalidateQueries({ queryKey: queryKeys.setupCredentials });
      void queryClient.invalidateQueries({ queryKey: queryKeys.emporiaDevices });
      void queryClient.invalidateQueries({ queryKey: queryKeys.analyticsFeedStatus });
      try {
        await postJson("/api/analytics/feeds/poll", {});
      } catch (err) {
        console.warn("Emporia feed poll after login failed", err);
      }
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to derive Emporia token.";
      onMessage({ type: "error", text });
    } finally {
      setIsEmporiaLoading(false);
    }
  };

  const saveEmporiaPreferences = async () => {
    if (!requireCanEdit()) return;
    const payload = emporiaDevicesQuery.data;
    if (!payload?.token_present) {
      onMessage({
        type: "error",
        text: "Emporia is not configured yet. Run Emporia cloud login first.",
      });
      return;
    }

    const updates: EmporiaDeviceUpdate[] = [];
    payload.devices.forEach((device) => {
      const edit = emporiaDeviceEdits[device.device_gid];
      if (!edit) return;
      const detectedGroup = (device.address ?? device.name ?? "").trim() || null;
      const groupLabelTrimmed = edit.group_label.trim();
      const desiredGroupLabel = groupLabelTrimmed ? groupLabelTrimmed : detectedGroup;
      const originalGroupLabel = (device.group_label ?? "").trim() || detectedGroup;

      const update: EmporiaDeviceUpdate = { device_gid: device.device_gid };
      if (edit.enabled !== device.enabled) {
        update.enabled = edit.enabled;
      }
      if (edit.hidden !== Boolean(device.hidden)) {
        update.hidden = edit.hidden;
      }
      if (edit.include_in_power_summary !== device.include_in_power_summary) {
        update.include_in_power_summary = edit.include_in_power_summary;
      }
      if (desiredGroupLabel !== originalGroupLabel) {
        update.group_label = groupLabelTrimmed;
      }

      const circuitUpdates: NonNullable<EmporiaDeviceUpdate["circuits"]> = [];
      const circuits = Array.isArray(device.circuits) ? device.circuits : [];
      circuits.forEach((circuit) => {
        const circuitEdit = edit.circuits[circuit.circuit_key];
        if (!circuitEdit) return;
        const circuitUpdate: NonNullable<EmporiaDeviceUpdate["circuits"]>[number] = {
          circuit_key: circuit.circuit_key,
        };
        let changed = false;
        if (circuitEdit.enabled !== circuit.enabled) {
          circuitUpdate.enabled = circuitEdit.enabled;
          changed = true;
        }
        if (circuitEdit.hidden !== circuit.hidden) {
          circuitUpdate.hidden = circuitEdit.hidden;
          changed = true;
        }
        if (circuitEdit.include_in_power_summary !== circuit.include_in_power_summary) {
          circuitUpdate.include_in_power_summary = circuitEdit.include_in_power_summary;
          changed = true;
        }
        if (changed) {
          circuitUpdates.push(circuitUpdate);
        }
      });
      if (circuitUpdates.length) {
        update.circuits = circuitUpdates;
      }

      if (Object.keys(update).length > 1) {
        updates.push(update);
      }
    });

    if (!updates.length) {
      onMessage({ type: "success", text: "Emporia preferences are already up to date." });
      return;
    }

    setIsEmporiaPrefsSaving(true);
    try {
      await updateEmporiaDevices(updates);
      await queryClient.invalidateQueries({ queryKey: queryKeys.emporiaDevices });

      await postJson("/api/analytics/feeds/poll", {});
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: queryKeys.analyticsFeedStatus }),
        queryClient.invalidateQueries({ queryKey: queryKeys.analytics }),
        queryClient.invalidateQueries({ queryKey: queryKeys.nodes }),
        queryClient.invalidateQueries({ queryKey: queryKeys.sensors }),
      ]);

      onMessage({
        type: "success",
        text: "Saved Emporia meter preferences and refreshed analytics.",
      });
    } catch (err) {
      const text = err instanceof Error ? err.message : "Failed to save Emporia preferences.";
      onMessage({ type: "error", text });
    } finally {
      setIsEmporiaPrefsSaving(false);
    }
  };

  return (
    <CollapsibleCard
      title="Integrations"
      description="Configure external integrations, their tokens, and provider-specific preferences."
      defaultOpen
      actions={
        <NodeButton
          onClick={() => queryClient.invalidateQueries({ queryKey: queryKeys.setupCredentials })}
        >
          Refresh tokens
        </NodeButton>
      }
    >
      <Dialog open={showLeapHelp} onOpenChange={(open) => setShowLeapHelp(open)}>
        <DialogContent className="max-w-2xl gap-0">
          <DialogHeader>
            <DialogTitle>Lutron LEAP setup</DialogTitle>
            <DialogDescription>
              LEAP uses certificate-based TLS. You must pair with the bridge/processor to generate
              a client certificate/key and obtain the bridge CA certificate.
            </DialogDescription>
          </DialogHeader>
          <div className="mt-4 space-y-3 text-sm text-muted-foreground">
            <p>Steps:</p>
            <ol className="list-decimal space-y-2 pl-5">
              <li>
                Confirm the controller is LEAP-capable (e.g., Caseta Smart Bridge Pro, RA2 Select,
                RadioRA 3, HomeWorks QSX).
              </li>
              <li>Put the bridge/processor into pairing mode per Lutron guidance.</li>
              <li>
                Use a LEAP pairing tool to generate the client certificate/key and export the
                bridge CA certificate.
              </li>
              <li>
                Paste the PEMs into the fields below. Leave &quot;Verify LEAP TLS certificate&quot;
                enabled if you have the CA cert.
              </li>
            </ol>
            <p>
              Note: LEAP is TLS over TCP/IP (default 8081). LIP uses Telnet on port 23 and does not
              require certificates.
            </p>
          </div>
          <DialogFooter className="mt-4">
            <NodeButton
              onClick={() => {
                setLeapHelpAcknowledged(true);
                setShowLeapHelp(false);
              }}
              variant="primary"
            >
              I have LEAP certificates
            </NodeButton>
            <NodeButton onClick={() => setShowLeapHelp(false)}>Close</NodeButton>
          </DialogFooter>
        </DialogContent>
      </Dialog>
      <div className="space-y-6">
        {!canEdit ? (
          <InlineBanner tone="warning" className="px-4 py-3 text-sm">
            Read-only: you need the <span className="font-semibold">config.write</span> capability
            to edit integrations.
          </InlineBanner>
        ) : null}
        <div className="grid gap-4 lg:grid-cols-2">
          <Card className="rounded-lg gap-0 bg-card-inset p-4">
            <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
              <div>
                <p className="text-sm font-semibold text-card-foreground">
                  Emporia cloud token
                </p>
 <p className="text-xs text-muted-foreground">
                  Derive and store the Emporia cloud token using your account credentials (password is not saved).
                </p>
              </div>
              <NodeButton
                size="xs"
                variant="primary"
                onClick={loginEmporiaCloud}
                disabled={!canEdit || isEmporiaLoading}
              >
                {isEmporiaLoading ? "Saving..." : "Login & save token"}
              </NodeButton>
            </div>
            <div className="mt-3 grid gap-3 md:grid-cols-2">
              <Input
                type="email"
                autoComplete="username"
                placeholder="Emporia username (email)"
                value={emporiaUsername}
                disabled={!canEdit}
                onChange={(event) => setEmporiaUsername(event.target.value)}
              />
              <Input
                type="password"
                autoComplete="current-password"
                placeholder="Emporia password"
                value={emporiaPassword}
                disabled={!canEdit}
                onChange={(event) => setEmporiaPassword(event.target.value)}
              />
            </div>
            {emporiaSites.length > 0 && (
 <p className="mt-2 text-xs text-muted-foreground">
                Linked sites: {emporiaSites.join(", ")}
              </p>
            )}
 <p className="mt-3 text-xs text-muted-foreground">
              Tip: You can also paste a token below under &ldquo;Integration tokens&rdquo; if you already have one.
            </p>
          </Card>

          <Card className="rounded-lg gap-0 bg-card-inset p-4">
            <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
              <div>
                <p className="text-sm font-semibold text-card-foreground">
                  Emporia meters & totals
                </p>
 <p className="text-xs text-muted-foreground">
                  Group meters by site (leave the label blank to use the detected address/name) and optionally exclude meters from system totals.
                </p>
              </div>
              <div className="flex items-center gap-2">
                <NodeButton
                  size="xs"
                  onClick={() => queryClient.invalidateQueries({ queryKey: queryKeys.emporiaDevices })}
                >
                  Refresh meters
                </NodeButton>
                <NodeButton
                  size="xs"
                  variant="primary"
                  onClick={saveEmporiaPreferences}
                  disabled={!canEdit || !emporiaPrefsDirty || isEmporiaPrefsSaving}
                >
                  {isEmporiaPrefsSaving ? "Saving..." : "Save preferences"}
                </NodeButton>
              </div>
            </div>

            {emporiaDevicesQuery.isLoading ? (
 <p className="mt-3 text-sm text-muted-foreground">
                Loading Emporia meters…
              </p>
            ) : emporiaDevicesQuery.error ? (
              <p className="mt-3 text-sm text-rose-600">
                Failed to load Emporia meters:{" "}
                {emporiaDevicesQuery.error instanceof Error
                  ? emporiaDevicesQuery.error.message
                  : "Unknown error"}
              </p>
            ) : !emporiaDevicesQuery.data?.token_present ? (
 <p className="mt-3 text-sm text-muted-foreground">
                No Emporia token saved yet. Login above or paste a token below first.
              </p>
            ) : (
              <div className="mt-3 overflow-x-auto">
                <table className="min-w-full divide-y divide-border text-sm">
                  <thead className="bg-card-inset">
                    <tr>
 <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Meter
                      </th>
 <th className="px-4 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Group label
                      </th>
 <th className="px-4 py-2 text-center text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Poll
                      </th>
 <th className="px-4 py-2 text-center text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        Hidden
                      </th>
 <th className="px-4 py-2 text-center text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        In totals
                      </th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border">
                    {emporiaDevicesQuery.data.devices.map((device) => {
                      const detectedGroup = (device.address ?? device.name ?? "").trim();
                      const storedGroup = (device.group_label ?? "").trim();
                      const groupLabelDefault =
                        storedGroup && detectedGroup && storedGroup === detectedGroup
                          ? ""
                          : storedGroup;
                      const isExpanded = Boolean(expandedEmporiaMeters[device.device_gid]);
                      const circuits = Array.isArray(device.circuits) ? device.circuits : [];
                      const fallbackCircuits: Record<string, EmporiaCircuitEdit> = {};
                      circuits.forEach((circuit) => {
                        fallbackCircuits[circuit.circuit_key] = {
                          enabled: circuit.enabled,
                          hidden: circuit.hidden,
                          include_in_power_summary: circuit.include_in_power_summary,
                        };
                      });
                      const edit = emporiaDeviceEdits[device.device_gid] ?? {
                        enabled: device.enabled,
                        hidden: Boolean(device.hidden),
                        include_in_power_summary: device.include_in_power_summary,
                        group_label: groupLabelDefault,
                        circuits: fallbackCircuits,
                      };
                      const sortedCircuits = [...circuits].sort((a, b) => {
                        if (a.circuit_key === "mains") return -1;
                        if (b.circuit_key === "mains") return 1;
                        const aNum = Number.parseInt(a.circuit_key, 10);
                        const bNum = Number.parseInt(b.circuit_key, 10);
                        if (!Number.isNaN(aNum) && !Number.isNaN(bNum)) return aNum - bNum;
                        return a.name.localeCompare(b.name);
                      });
                      const updateDeviceEdit = (patch: Partial<EmporiaDeviceEdit>) => {
                        setEmporiaDeviceEdits((prev) => {
                          const current = prev[device.device_gid] ?? edit;
                          return {
                            ...prev,
                            [device.device_gid]: {
                              ...current,
                              ...patch,
                            },
                          };
                        });
                      };
                      const updateCircuitEdit = (
                        circuitKey: string,
                        patch: Partial<EmporiaCircuitEdit>,
                      ) => {
                        setEmporiaDeviceEdits((prev) => {
                          const current = prev[device.device_gid] ?? edit;
                          const fallback = fallbackCircuits[circuitKey] ?? {
                            enabled: true,
                            hidden: false,
                            include_in_power_summary: false,
                          };
                          const currentCircuit = current.circuits[circuitKey] ?? fallback;
                          return {
                            ...prev,
                            [device.device_gid]: {
                              ...current,
                              circuits: {
                                ...current.circuits,
                                [circuitKey]: {
                                  ...currentCircuit,
                                  ...patch,
                                },
                              },
                            },
                          };
                        });
                      };

                      return (
                        <Fragment key={device.device_gid}>
                          <tr className={edit.enabled ? undefined : "opacity-70"}>
                            <td className="px-4 py-3">
                              <button
                                type="button"
                                className="flex items-start gap-2 text-left"
                                onClick={() =>
                                  setExpandedEmporiaMeters((prev) => ({
                                    ...prev,
                                    [device.device_gid]: !isExpanded,
                                  }))
                                }
                              >
 <span className="mt-0.5 w-4 text-muted-foreground">
                                  {isExpanded ? "▾" : "▸"}
                                </span>
                                <span>
 <span className="block font-medium text-foreground">
                                    {device.name ?? `Emporia ${device.device_gid}`}
                                  </span>
 <span className="block text-xs text-muted-foreground">
                                    {device.device_gid}
                                  </span>
                                  {device.address ? (
 <span className="block text-xs text-muted-foreground">
                                      Detected address: {device.address}
                                    </span>
                                  ) : null}
                                </span>
                              </button>
                            </td>
                            <td className="px-4 py-3">
                              <Input
                                type="text"
                                aria-label="Meter group label override"
                                placeholder={detectedGroup || "Use detected label"}
                                className="w-56 px-2 py-1"
                                value={edit.group_label}
                                disabled={!canEdit}
                                onChange={(event) => updateDeviceEdit({ group_label: event.target.value })}
                              />
                            </td>
                            <td className="px-4 py-3 text-center">
                              <input
                                type="checkbox"
                                aria-label="Poll meter"
                                checked={edit.enabled}
                                disabled={!canEdit}
                                onChange={(event) => updateDeviceEdit({ enabled: event.target.checked })}
                              />
                            </td>
                            <td className="px-4 py-3 text-center">
                              <input
                                type="checkbox"
                                aria-label="Hide meter"
                                checked={edit.hidden}
                                disabled={!canEdit}
                                onChange={(event) => updateDeviceEdit({ hidden: event.target.checked })}
                              />
                            </td>
                            <td className="px-4 py-3 text-center">
                              <input
                                type="checkbox"
                                aria-label="Include meter in totals"
                                checked={edit.include_in_power_summary}
                                disabled={!canEdit || !edit.enabled}
                                onChange={(event) =>
                                  updateDeviceEdit({ include_in_power_summary: event.target.checked })
                                }
                              />
                            </td>
                          </tr>
                          {isExpanded ? (
                            <tr className="bg-card">
                              <td colSpan={5} className="px-4 pb-4">
                                <Card className="rounded-lg gap-0 p-3">
                                  <div className="flex flex-col gap-1 md:flex-row md:items-center md:justify-between">
                                    <p className="text-sm font-semibold text-card-foreground">
                                      Circuits
                                    </p>
 <p className="text-xs text-muted-foreground">
                                      Poll stores data. Hidden hides in UI but still stores. In totals contributes to system totals.
                                    </p>
                                  </div>
                                  {sortedCircuits.length === 0 ? (
 <p className="mt-2 text-sm text-muted-foreground">
                                      No circuits reported yet. Poll the Emporia feed to load channels.
                                    </p>
                                  ) : (
                                    <div className="mt-3 overflow-x-auto">
                                      <table className="min-w-full divide-y divide-border text-sm">
                                        <thead className="bg-card-inset">
                                          <tr>
 <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                              Circuit
                                            </th>
 <th className="px-3 py-2 text-center text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                              Poll
                                            </th>
 <th className="px-3 py-2 text-center text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                              Hidden
                                            </th>
 <th className="px-3 py-2 text-center text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                                              In totals
                                            </th>
                                          </tr>
                                        </thead>
                                        <tbody className="divide-y divide-border">
                                          {sortedCircuits.map((circuit) => {
                                            const circuitEdit =
                                              edit.circuits[circuit.circuit_key] ??
                                              fallbackCircuits[circuit.circuit_key];
                                            if (!circuitEdit) return null;
                                            return (
                                              <tr key={circuit.circuit_key}>
                                                <td className="px-3 py-2">
 <div className="font-medium text-foreground">
                                                    {circuit.name}
                                                  </div>
 <div className="text-xs text-muted-foreground">
                                                    {circuit.circuit_key === "mains"
                                                      ? "Mains total"
                                                      : `Channel ${circuit.raw_channel_num ?? circuit.circuit_key}`}
                                                    {circuit.nested_device_gid
                                                      ? ` · Device ${circuit.nested_device_gid}`
                                                      : ""}
                                                  </div>
                                                </td>
                                                <td className="px-3 py-2 text-center">
                                                  <input
                                                    type="checkbox"
                                                    aria-label={`Poll circuit ${circuit.name}`}
                                                    checked={circuitEdit.enabled}
                                                    disabled={!canEdit || !edit.enabled}
                                                    onChange={(event) =>
                                                      updateCircuitEdit(circuit.circuit_key, {
                                                        enabled: event.target.checked,
                                                      })
                                                    }
                                                  />
                                                </td>
                                                <td className="px-3 py-2 text-center">
                                                  <input
                                                    type="checkbox"
                                                    aria-label={`Hide circuit ${circuit.name}`}
                                                    checked={circuitEdit.hidden}
                                                    disabled={!canEdit || !edit.enabled}
                                                    onChange={(event) =>
                                                      updateCircuitEdit(circuit.circuit_key, {
                                                        hidden: event.target.checked,
                                                      })
                                                    }
                                                  />
                                                </td>
                                                <td className="px-3 py-2 text-center">
                                                  <input
                                                    type="checkbox"
                                                    aria-label={`Include circuit ${circuit.name} in totals`}
                                                    checked={circuitEdit.include_in_power_summary}
                                                    disabled={!canEdit || !edit.enabled || !circuitEdit.enabled}
                                                    onChange={(event) =>
                                                      updateCircuitEdit(circuit.circuit_key, {
                                                        include_in_power_summary: event.target.checked,
                                                      })
                                                    }
                                                  />
                                                </td>
                                              </tr>
                                            );
                                          })}
                                        </tbody>
                                      </table>
                                    </div>
                                  )}
                                </Card>
                              </td>
                            </tr>
                          ) : null}
                        </Fragment>
                      );
                    })}
                    {!emporiaDevicesQuery.data.devices.length ? (
                      <tr>
 <td colSpan={5} className="px-4 py-6 text-center text-sm text-muted-foreground">
                          No Emporia meters found on this account.
                        </td>
                      </tr>
                    ) : null}
                  </tbody>
                </table>
              </div>
            )}

 <p className="mt-3 text-xs text-muted-foreground">
              Tip: Expand a meter to pick circuits. Default is Mains in totals; disable it and select circuits if you need circuit-level totals.
            </p>
          </Card>
        </div>

        <div>
          <div className="flex flex-col gap-1 md:flex-row md:items-center md:justify-between">
            <div>
              <p className="text-sm font-semibold text-card-foreground">External devices (TCP/IP)</p>
              <p className="text-xs text-muted-foreground">
                Add commercially available devices and auto-map their points using the device
                catalog.
              </p>
            </div>
            <NodeButton size="xs" onClick={createExternalDeviceEntry} disabled={!canEdit}>
              Add device
            </NodeButton>
          </div>
          <Card className="mt-3 rounded-lg gap-0 bg-card-inset p-4">
            <div className="grid gap-3 md:grid-cols-2">
              <Input
                placeholder="Device name"
                value={externalDraft.name}
                disabled={!canEdit}
                onChange={(event) =>
                  setExternalDraft((prev) => ({ ...prev, name: event.target.value }))
                }
              />
              <Input
                placeholder="Host / IP address"
                value={externalDraft.host}
                disabled={!canEdit}
                onChange={(event) =>
                  setExternalDraft((prev) => ({ ...prev, host: event.target.value }))
                }
              />
              <Select
                value={externalDraft.vendorId}
                disabled={!canEdit}
                onChange={(event) => {
                  const vendorId = event.target.value;
                  const vendor = externalVendors.find((entry) => entry.id === vendorId);
                  const model = vendor?.models[0];
                  setExternalDraft((prev) => ({
                    ...prev,
                    vendorId,
                    modelId: model?.id ?? "",
                    protocol: model?.protocols?.[0] ?? "",
                  }));
                }}
              >
                <option value="" disabled>
                  Select vendor
                </option>
                {externalVendors.map((vendor) => (
                  <option key={vendor.id} value={vendor.id}>
                    {vendor.name}
                  </option>
                ))}
              </Select>
              <Select
                value={externalDraft.modelId}
                disabled={!canEdit || !selectedVendor}
                onChange={(event) => {
                  const modelId = event.target.value;
                  const model = selectedVendor?.models.find((entry) => entry.id === modelId);
                  setExternalDraft((prev) => ({
                    ...prev,
                    modelId,
                    protocol: model?.protocols?.[0] ?? prev.protocol,
                  }));
                }}
              >
                <option value="" disabled>
                  Select model
                </option>
                {(selectedVendor?.models ?? []).map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.name}
                  </option>
                ))}
              </Select>
              <Select
                value={externalDraft.protocol}
                disabled={!canEdit || !selectedModel}
                onChange={(event) =>
                  setExternalDraft((prev) => ({ ...prev, protocol: event.target.value }))
                }
              >
                <option value="" disabled>
                  Select protocol
                </option>
                {(selectedModel?.protocols ?? []).map((protocol) => (
                  <option key={protocol} value={protocol}>
                    {protocol}
                  </option>
                ))}
              </Select>
              <Input
                type="number"
                placeholder="Port (optional)"
                value={externalDraft.port}
                disabled={!canEdit}
                onChange={(event) =>
                  setExternalDraft((prev) => ({ ...prev, port: event.target.value }))
                }
              />
              <Input
                type="number"
                placeholder="Unit ID (Modbus)"
                value={externalDraft.unitId}
                disabled={!canEdit}
                onChange={(event) =>
                  setExternalDraft((prev) => ({ ...prev, unitId: event.target.value }))
                }
              />
              {externalDraft.protocol === "snmp" ? (
                <Input
                  placeholder="SNMP community"
                  value={externalDraft.snmpCommunity}
                  disabled={!canEdit}
                  onChange={(event) =>
                    setExternalDraft((prev) => ({ ...prev, snmpCommunity: event.target.value }))
                  }
                />
              ) : null}
              {externalDraft.protocol === "http_json" ? (
                <Input
                  placeholder="HTTP base URL"
                  value={externalDraft.httpBaseUrl}
                  disabled={!canEdit}
                  onChange={(event) =>
                    setExternalDraft((prev) => ({ ...prev, httpBaseUrl: event.target.value }))
                  }
                />
              ) : null}
              {externalDraft.protocol === "lutron_lip" ? (
                <Fragment>
                  <Input
                    placeholder="LIP username (optional)"
                    value={externalDraft.lipUsername}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setExternalDraft((prev) => ({ ...prev, lipUsername: event.target.value }))
                    }
                  />
                  <Input
                    type="password"
                    placeholder="LIP password (optional)"
                    value={externalDraft.lipPassword}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setExternalDraft((prev) => ({ ...prev, lipPassword: event.target.value }))
                    }
                  />
                  <Textarea
                    className="md:col-span-2"
                    placeholder="Paste Lutron Integration Report text"
                    value={externalDraft.lipIntegrationReport}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setExternalDraft((prev) => ({
                        ...prev,
                        lipIntegrationReport: event.target.value,
                      }))
                    }
                  />
                </Fragment>
              ) : null}
              {externalDraft.protocol === "lutron_leap" ? (
                <Fragment>
                  <Textarea
                    className="md:col-span-2"
                    placeholder="LEAP client cert PEM"
                    value={externalDraft.leapClientCert}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setExternalDraft((prev) => ({ ...prev, leapClientCert: event.target.value }))
                    }
                  />
                  <Textarea
                    className="md:col-span-2"
                    placeholder="LEAP client key PEM"
                    value={externalDraft.leapClientKey}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setExternalDraft((prev) => ({ ...prev, leapClientKey: event.target.value }))
                    }
                  />
                  <Textarea
                    className="md:col-span-2"
                    placeholder="LEAP CA cert PEM (bridge certificate)"
                    value={externalDraft.leapCaCert}
                    disabled={!canEdit}
                    onChange={(event) =>
                      setExternalDraft((prev) => ({ ...prev, leapCaCert: event.target.value }))
                    }
                  />
                  <label className="flex items-center gap-2 text-xs text-muted-foreground md:col-span-2">
                    <input
                      type="checkbox"
                      checked={externalDraft.leapVerifyCa}
                      disabled={!canEdit}
                      onChange={(event) =>
                        setExternalDraft((prev) => ({ ...prev, leapVerifyCa: event.target.checked }))
                      }
                    />
                    Verify LEAP TLS certificate
                  </label>
                </Fragment>
              ) : null}
            </div>
            <p className="mt-3 text-xs text-muted-foreground">
              The catalog defines default point maps; use custom sensors to fill gaps when a
              vendor-specific profile is missing.
            </p>
          </Card>
          <div className="mt-4 grid gap-3">
            {(externalDevicesQuery.data ?? []).length === 0 ? (
              <Card className="rounded-lg bg-card-inset p-4 text-xs text-muted-foreground">
                No external devices configured yet.
              </Card>
            ) : (
              (externalDevicesQuery.data ?? []).map((device) => (
                <Card key={device.node_id} className="rounded-lg bg-card-inset p-4">
                  <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
                    <div>
                      <p className="text-sm font-semibold text-card-foreground">
                        {device.name}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {device.external_provider ?? "external"} · {device.node_id}
                      </p>
                    </div>
                    <div className="flex gap-2">
                      <NodeButton
                        size="xs"
                        onClick={() => syncExternalDeviceEntry(device.node_id)}
                        disabled={!canEdit}
                      >
                        Sync now
                      </NodeButton>
                      <NodeButton
                        size="xs"
                        onClick={() => deleteExternalDeviceEntry(device.node_id)}
                        disabled={!canEdit}
                      >
                        Remove
                      </NodeButton>
                    </div>
                  </div>
                </Card>
              ))
            )}
          </div>
        </div>

        <div>
          <div className="flex flex-col gap-1 md:flex-row md:items-center md:justify-between">
            <div>
              <p className="text-sm font-semibold text-card-foreground">
                Integration tokens
              </p>
 <p className="text-xs text-muted-foreground">
                Store provider tokens used by connectors and map integrations.
              </p>
            </div>
          </div>
          <div className="mt-3 grid gap-4 md:grid-cols-2">
            {credentialCatalog.map((credential) => {
              const current = credentialByName.get(credential.name);
              return (
                <Card
                  key={credential.name}
                  className="rounded-lg gap-0 bg-card-inset p-4"
                >
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-sm font-semibold text-card-foreground">
                        {credential.label}
                      </p>
 <p className="text-xs text-muted-foreground">
                        {credential.hint}
                      </p>
                    </div>
                    <span
                      className={`text-xs font-semibold ${
                        current?.has_value
 ? "text-emerald-600"
 : "text-amber-600"
                      }`}
                    >
                      {current?.has_value ? "Configured" : "Missing"}
                    </span>
                  </div>
                  <div className="mt-3 flex flex-col gap-3">
                    <Input
                      type="password"
                      placeholder="Enter token"
                      value={credentialDrafts[credential.name] ?? ""}
                      disabled={!canEdit}
                      onChange={(event) =>
                        setCredentialDrafts((prev) => ({
                          ...prev,
                          [credential.name]: event.target.value,
                        }))
                      }
                    />
                    <div className="flex gap-2">
                      <NodeButton
                        size="xs"
                        variant="primary"
                        onClick={() => saveCredential(credential.name)}
                        disabled={!canEdit}
                      >
                        Save
                      </NodeButton>
                      <NodeButton
                        size="xs"
                        onClick={() => clearCredential(credential.name)}
                        disabled={!canEdit}
                      >
                        Clear
                      </NodeButton>
                    </div>
                  </div>
                </Card>
              );
            })}
          </div>
        </div>
      </div>
    </CollapsibleCard>
  );
}
