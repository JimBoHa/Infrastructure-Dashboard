import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { RetentionPolicyTable } from "@/components/backups/RetentionPolicyTable";
import type { BackupRetentionConfig, DemoNode } from "@/types/dashboard";

const buildNode = (id: string, name: string): DemoNode => ({
  id,
  name,
  status: "online",
  uptime_seconds: 120,
  cpu_percent: 12,
  storage_used_bytes: 1024,
});

const baseConfig = (overrides: BackupRetentionConfig["policies"] = []): BackupRetentionConfig => ({
  default_keep_days: 30,
  policies: overrides,
  last_cleanup_at: null,
});

describe("RetentionPolicyTable", () => {
  it("renders nodes with default and override indicators", () => {
    render(
      <RetentionPolicyTable
        config={baseConfig([
          { node_id: "node-2", node_name: "South Field", keep_days: 45 },
        ])}
        nodes={[buildNode("node-1", "North Field"), buildNode("node-2", "South Field")]}
        onSubmit={vi.fn()}
        savingNodeId={null}
      />,
    );

    const northInput = screen.getByLabelText("Retention days for North Field") as HTMLInputElement;
    expect(northInput.value).toBe("30");
    expect(screen.getByText("Default (30 days)")).toBeInTheDocument();

    const southInput = screen.getByLabelText("Retention days for South Field") as HTMLInputElement;
    expect(southInput.value).toBe("45");
    expect(screen.getByText("Override (45 days)")).toBeInTheDocument();
  });

  it("submits updated retention values", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(
      <RetentionPolicyTable
        config={baseConfig()}
        nodes={[buildNode("node-1", "North Field")]}
        onSubmit={onSubmit}
        savingNodeId={null}
      />,
    );

    const input = screen.getByLabelText("Retention days for North Field");
    await user.clear(input);
    await user.type(input, "60");

    const row = screen.getByText("North Field").closest("tr");
    expect(row).not.toBeNull();
    await user.click(within(row as HTMLTableRowElement).getByRole("button", { name: "Save" }));

    expect(onSubmit).toHaveBeenCalledWith("node-1", 60);
  });

  it("reverts to defaults when requested", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(
      <RetentionPolicyTable
        config={baseConfig([
          { node_id: "node-2", node_name: "South Field", keep_days: 45 },
        ])}
        nodes={[buildNode("node-2", "South Field")]}
        onSubmit={onSubmit}
        savingNodeId={null}
      />,
    );

    const row = screen.getByText("South Field").closest("tr");
    expect(row).not.toBeNull();
    const resetButton = within(row as HTMLTableRowElement).getByRole("button", {
      name: "Use default",
    });
    expect(resetButton).not.toBeDisabled();

    await user.click(resetButton);
    expect(onSubmit).toHaveBeenCalledWith("node-2", null);
  });

  it("shows validation feedback for invalid inputs", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    render(
      <RetentionPolicyTable
        config={baseConfig()}
        nodes={[buildNode("node-1", "North Field")]}
        onSubmit={onSubmit}
        savingNodeId={null}
      />,
    );

    const input = screen.getByLabelText("Retention days for North Field");
    await user.clear(input);
    await user.type(input, "0");

    const row = screen.getByText("North Field").closest("tr");
    await user.click(within(row as HTMLTableRowElement).getByRole("button", { name: "Save" }));

    expect(screen.getByText("Enter a retention period of at least 1 day.")).toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });
});

