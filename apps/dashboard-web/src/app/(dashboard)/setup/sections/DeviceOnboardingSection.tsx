"use client";

import { useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { useQueryClient } from "@tanstack/react-query";

import CollapsibleCard from "@/components/CollapsibleCard";
import { Card } from "@/components/ui/card";
import NodeButton from "@/features/nodes/components/NodeButton";
import { fetchAdoptionCandidates } from "@/lib/api";
import {
  queryKeys,
  useAdoptionCandidatesQuery,
  useExternalDeviceCatalogQuery,
  useExternalDevicesQuery,
} from "@/lib/queries";

import type { Message } from "../types";

const MANAGED_VENDOR_IDS = new Set(["generator_ats"]);
const HIGHLIGHTED_INTEGRATIONS = [
  "Emporia Cloud",
  "Renogy BT-2",
  "WS-2902 weather stations",
  "Victron GX",
  "Lutron LIP / LEAP",
  "Schneider / APC meters",
  "Setra / Metasys / Megatron",
  "CPS SunSpec inverters",
  "Tridium Niagara",
  "Multistack",
  "Tesla Energy",
  "Enphase",
];

export default function DeviceOnboardingSection({
  canEdit,
  onMessage,
}: {
  canEdit: boolean;
  onMessage: (message: Message) => void;
}) {
  const router = useRouter();
  const queryClient = useQueryClient();
  const adoptionQuery = useAdoptionCandidatesQuery({ enabled: canEdit });
  const externalCatalogQuery = useExternalDeviceCatalogQuery();
  const externalDevicesQuery = useExternalDevicesQuery();
  const [scanBusy, setScanBusy] = useState(false);

  const supportedVendors = useMemo(() => {
    const vendors = externalCatalogQuery.data?.vendors ?? [];
    return vendors.filter((vendor) => !MANAGED_VENDOR_IDS.has(vendor.id));
  }, [externalCatalogQuery.data]);

  const integratedDeviceCount = externalDevicesQuery.data?.length ?? 0;
  const adoptionCount = adoptionQuery.data?.length ?? 0;
  const catalogCount = supportedVendors.reduce((total, vendor) => total + vendor.models.length, 0);

  const jumpToSection = (sectionId: string) => {
    const target = document.getElementById(sectionId);
    if (target) {
      target.scrollIntoView({ behavior: "smooth", block: "start" });
      return;
    }
    router.push(`/setup#${sectionId}`);
  };

  const scanForLocalNodes = async () => {
    if (!canEdit) {
      onMessage({ type: "error", text: "This action requires the config.write capability." });
      return;
    }
    setScanBusy(true);
    try {
      const candidates = await queryClient.fetchQuery({
        queryKey: queryKeys.adoptionCandidates,
        queryFn: fetchAdoptionCandidates,
      });
      onMessage({
        type: "success",
        text: candidates.length
          ? `Scan complete: found ${candidates.length} adoptable node${candidates.length === 1 ? "" : "s"}.`
          : "Scan complete: no adoptable nodes found.",
      });
    } catch (err) {
      onMessage({
        type: "error",
        text: err instanceof Error ? err.message : "Node discovery failed.",
      });
    } finally {
      setScanBusy(false);
    }
  };

  return (
    <CollapsibleCard
      title="Device onboarding"
      description="Scan for local nodes, review supported integrations, and jump straight into adoption."
      defaultOpen
      bodyClassName="space-y-4"
    >
      <div className="grid gap-3 md:grid-cols-3">
        <Card className="rounded-xl bg-card-inset px-4 py-4">
          <p className="text-sm font-semibold text-card-foreground">Local node discovery</p>
          <p className="mt-2 text-3xl font-semibold text-card-foreground">{adoptionCount}</p>
          <p className="mt-1 text-sm text-muted-foreground">
            Adoptable Infrastructure Dashboard nodes currently visible on the local network.
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <NodeButton onClick={() => void scanForLocalNodes()} loading={scanBusy}>
              Scan now
            </NodeButton>
            <NodeButton onClick={() => router.push("/nodes")}>Open Nodes</NodeButton>
          </div>
        </Card>

        <Card className="rounded-xl bg-card-inset px-4 py-4">
          <p className="text-sm font-semibold text-card-foreground">Configured integrations</p>
          <p className="mt-2 text-3xl font-semibold text-card-foreground">{integratedDeviceCount}</p>
          <p className="mt-1 text-sm text-muted-foreground">
            External devices already configured through Setup Center integrations.
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <NodeButton onClick={() => jumpToSection("setup-integrations-external-devices")}>
              Open external devices
            </NodeButton>
          </div>
        </Card>

        <Card className="rounded-xl bg-card-inset px-4 py-4">
          <p className="text-sm font-semibold text-card-foreground">Supported device profiles</p>
          <p className="mt-2 text-3xl font-semibold text-card-foreground">
            {externalCatalogQuery.isLoading ? "..." : externalCatalogQuery.isError ? "!" : catalogCount}
          </p>
          <p className="mt-1 text-sm text-muted-foreground">
            Catalog-backed model templates ready for manual onboarding now, excluding generator/ATS controllers.
          </p>
          {externalCatalogQuery.isError ? (
            <p className="mt-2 text-xs text-rose-600">
              The device catalog did not load. Open the Integrations section for the exact error.
            </p>
          ) : null}
          <div className="mt-3 flex flex-wrap gap-2">
            <NodeButton onClick={() => jumpToSection("setup-integrations-device-catalog")}>
              View catalog
            </NodeButton>
          </div>
        </Card>
      </div>

      <div className="rounded-xl border border-border bg-background px-4 py-4">
        <p className="text-sm font-semibold text-card-foreground">Ready-to-integrate families</p>
        <p className="mt-1 text-xs text-muted-foreground">
          These are supported integration families, not auto-discovered devices. Use{" "}
          <span className="font-medium text-card-foreground">Scan now</span> for Infrastructure
          Dashboard nodes on your LAN. Use{" "}
          <span className="font-medium text-card-foreground">Open external devices</span> to add
          third-party systems by host/IP and protocol.
        </p>
        <div className="mt-3 flex flex-wrap gap-2">
          {HIGHLIGHTED_INTEGRATIONS.map((label) => (
            <span
              key={label}
              className="rounded-full border border-border bg-card-inset px-3 py-1 text-xs font-medium text-card-inset-foreground"
            >
              {label}
            </span>
          ))}
          {supportedVendors.map((vendor) => (
            <span
              key={vendor.id}
              className="rounded-full border border-border bg-card-inset px-3 py-1 text-xs text-muted-foreground"
            >
              {vendor.name}
            </span>
          ))}
        </div>
      </div>
    </CollapsibleCard>
  );
}
