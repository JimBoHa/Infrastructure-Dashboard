import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";

import AdoptionModal from "@/features/nodes/components/AdoptionModal";

const postJson = vi.fn();
vi.mock("@/lib/api", () => ({
  postJson: (...args: unknown[]) => postJson(...args),
}));

const candidate = {
  service_name: "field-node._iotnode._tcp.local.",
  hostname: "field-node.local",
  ip: "192.168.1.200",
  port: 9000,
  mac_eth: "AA:BB:CC:DD:EE:FF",
  mac_wifi: "11:22:33:44:55:66",
  properties: { fw: "1.0.0" },
};

describe("AdoptionModal", () => {
  beforeEach(() => {
    postJson.mockReset();
    postJson.mockImplementation((url: string) => {
      if (url.includes("/api/adoption/tokens")) {
        return Promise.resolve({ token: "token-123" });
      }
      return Promise.resolve({ id: "node-id", name: "Stable Node" });
    });
  });

  it("submits default name when input left blank", async () => {
    const onAdopted = vi.fn();
    render(
      <AdoptionModal
        candidate={candidate}
        restoreOptions={[
          { node_id: "source-node", node_name: "Barn Controller", last_backup: "2025-03-01" },
        ]}
        onClose={vi.fn()}
        onAdopted={onAdopted}
        onError={vi.fn()}
      />,
    );

    const submit = screen.getByRole("button", { name: /Adopt/i });
    fireEvent.click(submit);

    await waitFor(() => expect(postJson).toHaveBeenCalledTimes(2));
    const tokenCall = postJson.mock.calls[0];
    expect(tokenCall[0]).toBe("/api/adoption/tokens");
    const adoptPayload = postJson.mock.calls[1][1];
    expect(adoptPayload.name).toBe("field-node.local");
    expect(adoptPayload.token).toBe("token-123");
    expect(onAdopted).toHaveBeenCalledWith(expect.stringContaining("Stable Node"));
  });

  it("issues controller adoption token even when discovery advertises one", async () => {
    const onAdopted = vi.fn();
    const candidateWithToken = {
      ...candidate,
      properties: { ...candidate.properties, adoption_token: "token-456" },
    };
    render(
      <AdoptionModal
        candidate={candidateWithToken}
        restoreOptions={[]}
        onClose={vi.fn()}
        onAdopted={onAdopted}
        onError={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Adopt/i }));

    await waitFor(() => expect(postJson).toHaveBeenCalledTimes(2));
    expect(postJson.mock.calls[0][0]).toBe("/api/adoption/tokens");
    const adoptCall = postJson.mock.calls[1];
    expect(adoptCall[0]).toBe("/api/adopt");
    expect(adoptCall[1].token).toBe("token-123");
    expect(onAdopted).toHaveBeenCalledWith(expect.stringContaining("Stable Node"));
  });

  it("shows an error when controller token issuance fails", async () => {
    postJson.mockReset();
    postJson.mockImplementation((url: string) => {
      if (url.includes("/api/adoption/tokens")) {
        return Promise.reject(new Error("Request failed (404): Not Found"));
      }
      return Promise.resolve({ id: "node-id", name: "Stable Node" });
    });

    const onAdopted = vi.fn();
    const onError = vi.fn();
    const candidateWithToken = {
      ...candidate,
      properties: { ...candidate.properties, adoption_token: "token-456" },
    };
    render(
      <AdoptionModal
        candidate={candidateWithToken}
        restoreOptions={[]}
        onClose={vi.fn()}
        onAdopted={onAdopted}
        onError={onError}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Adopt/i }));

    await waitFor(() => expect(postJson).toHaveBeenCalledTimes(1));
    expect(postJson.mock.calls[0][0]).toBe("/api/adoption/tokens");
    expect(postJson.mock.calls.find((call) => call[0] === "/api/adopt")).toBeUndefined();
    expect(onAdopted).not.toHaveBeenCalled();
    expect(onError).toHaveBeenCalledWith(expect.stringContaining("Request failed (404)"));
  });

  it("uses custom name when provided", async () => {
    const onAdopted = vi.fn();
    render(
      <AdoptionModal
        candidate={candidate}
        restoreOptions={[
          { node_id: "source-node", node_name: "Barn Controller", last_backup: "2025-03-01" },
        ]}
        onClose={vi.fn()}
        onAdopted={onAdopted}
        onError={vi.fn()}
      />,
    );

    const input = screen.getByLabelText(/Display name/i);
    fireEvent.change(input, { target: { value: "Irrigation Controller" } });
    fireEvent.click(screen.getByRole("button", { name: /Adopt/i }));

    await waitFor(() => expect(postJson).toHaveBeenCalledTimes(2));
    const payload = postJson.mock.calls[1][1];
    expect(payload.name).toBe("Irrigation Controller");
    expect(onAdopted).toHaveBeenCalled();
  });

  it("includes restore_from_node_id when selected", async () => {
    const onAdopted = vi.fn();
    render(
      <AdoptionModal
        candidate={candidate}
        restoreOptions={[
          { node_id: "source-node", node_name: "Barn Controller", last_backup: "2025-03-01" },
        ]}
        onClose={vi.fn()}
        onAdopted={onAdopted}
        onError={vi.fn()}
      />,
    );

    const select = screen.getByLabelText(/Restore from backup/i);
    fireEvent.change(select, { target: { value: "source-node" } });
    fireEvent.click(screen.getByRole("button", { name: /Adopt/i }));

    await waitFor(() => expect(postJson).toHaveBeenCalledTimes(2));
    const payload = postJson.mock.calls.pop()?.[1];
    expect(payload?.restore_from_node_id).toBe("source-node");
    expect(onAdopted).toHaveBeenCalledWith(
      expect.stringContaining("Restore queued from Barn Controller (2025-03-01)"),
    );
  });
});
