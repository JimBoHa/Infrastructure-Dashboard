import { describe, expect, it } from "vitest";
import { filterAdoptionCandidates } from "@/features/nodes/hooks/useNodesPageData";

describe("filterAdoptionCandidates", () => {
  it("keeps candidates even when adoption_token is missing", () => {
    const existing = new Set<string>();
    const filtered = filterAdoptionCandidates(
      [
        {
          service_name: "pi5-node2._iotnode._tcp.local.",
          hostname: "pi5-node2.local",
          ip: "10.255.8.20",
          port: 9000,
          mac_eth: "88:a2:9e:65:46:5b",
          mac_wifi: null,
          adoption_token: null,
          properties: {},
        },
      ],
      existing,
    );
    expect(filtered).toHaveLength(1);
  });

  it("filters out candidates missing all MAC bindings", () => {
    const existing = new Set<string>();
    const filtered = filterAdoptionCandidates(
      [
        {
          service_name: "no-mac._iotnode._tcp.local.",
          hostname: null,
          ip: "10.0.0.1",
          port: 9000,
          mac_eth: null,
          mac_wifi: null,
          adoption_token: null,
          properties: {},
        },
      ],
      existing,
    );
    expect(filtered).toHaveLength(0);
  });

  it("filters out candidates whose MACs are already adopted", () => {
    const existing = new Set<string>(["88:a2:9e:65:46:5b"]);
    const filtered = filterAdoptionCandidates(
      [
        {
          service_name: "pi5-node2._iotnode._tcp.local.",
          hostname: "pi5-node2.local",
          ip: "10.255.8.20",
          port: 9000,
          mac_eth: "88:a2:9e:65:46:5b",
          mac_wifi: null,
          adoption_token: null,
          properties: {},
        },
      ],
      existing,
    );
    expect(filtered).toHaveLength(0);
  });
});

