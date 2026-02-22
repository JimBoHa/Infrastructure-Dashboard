import { fireEvent, render, screen } from "@testing-library/react";
import { vi } from "vitest";

import AdoptionSection from "@/features/nodes/components/AdoptionSection";
import buildRestoreOptions from "@/features/nodes/utils/buildRestoreOptions";
import type { DemoAdoptionCandidate, DemoBackup, DemoNode } from "@/types/dashboard";

describe("AdoptionSection", () => {
  const candidate: DemoAdoptionCandidate = {
    service_name: "node-123._iotnode._tcp.local.",
    hostname: "node-123.local",
    ip: "192.168.1.10",
    port: 9000,
    mac_eth: "AA:BB:CC:DD:EE:FF",
    mac_wifi: "11:22:33:44:55:66",
    properties: { fw: "1.0.0" },
  };

  it("renders empty state", () => {
    render(
      <AdoptionSection
        discovered={[]}
        adoption={[]}
        nodes={[]}
        onRefresh={vi.fn()}
        onAdopt={vi.fn()}
      />,
    );
    expect(screen.getByText(/No new nodes found/i)).toBeInTheDocument();
  });

  it("invokes onAdopt for a candidate", () => {
    const onAdopt = vi.fn();
    render(
      <AdoptionSection
        discovered={[candidate]}
        adoption={[candidate]}
        nodes={[]}
        onRefresh={vi.fn()}
        onAdopt={onAdopt}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Adopt node/i }));
    expect(onAdopt).toHaveBeenCalledWith(candidate);
  });
});

describe("buildRestoreOptions", () => {
  const nodes: DemoNode[] = [
    { id: "node-1", name: "Barn Controller" },
    { id: "node-2", name: "Field Node" },
  ];
  const backups: Record<string, DemoBackup[]> = {
    "node-1": [
      { id: "b-older", node_id: "node-1", captured_at: "2024-04-01T12:00:00Z", size_bytes: 1024, path: "/old" },
      { id: "b-newer", node_id: "node-1", captured_at: "2024-05-02T12:00:00Z", size_bytes: 2048, path: "/new" },
    ],
  };

  it("filters nodes without backups and picks latest backup date", () => {
    const options = buildRestoreOptions(nodes, backups);
    expect(options).toEqual([
      { node_id: "node-1", node_name: "Barn Controller", last_backup: "2024-05-02" },
    ]);
  });
});
