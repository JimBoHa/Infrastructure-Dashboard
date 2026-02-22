import { test, expect } from "@playwright/test";
import { installStubApi, PLAYWRIGHT_STUB_USER } from "./stubApi";

test.describe("auth / capability gating", () => {
  test("operator cannot see admin navigation", async ({ page }) => {
    await installStubApi(page, {
      jsonByPath: {
        "/api/auth/me": {
          ...PLAYWRIGHT_STUB_USER,
          role: "operator",
          capabilities: ["schedules.write", "outputs.command", "alerts.view", "alerts.ack"],
        },
      },
    });

    await page.goto("/overview");

    await page.getByRole("button", { name: "Open navigation" }).tap();
    const sidebar = page.locator("#dashboard-sidebar");
    await expect(sidebar).toBeVisible();

    await expect(sidebar.getByRole("link", { name: "Users" })).toHaveCount(0);
    await expect(sidebar.getByRole("link", { name: "Setup Center" })).toHaveCount(0);
    await expect(sidebar.getByRole("link", { name: "Deployment" })).toHaveCount(0);
  });

  test("view role sees outputs as read-only", async ({ page }) => {
    await installStubApi(page, {
      jsonByPath: {
        "/api/auth/me": {
          ...PLAYWRIGHT_STUB_USER,
          role: "view",
          capabilities: ["sensors.view", "alerts.view", "analytics.view"],
        },
        "/api/outputs": [
          {
            id: "playwright-output-1",
            node_id: "playwright-node-1",
            name: "Playwright Output",
            type: "relay",
            state: "off",
            last_command: null,
            supported_states: ["off", "on"],
            command_topic: null,
            schedule_ids: [],
            config: {},
          },
        ],
      },
    });

    await page.goto("/sensors");

    await page.getByRole("button", { name: "Flat" }).tap();

    const outputsSection = page.getByRole("heading", { name: "Outputs" }).locator("..");
    await expect(outputsSection.getByText("Read-only: you need", { exact: false })).toBeVisible();
    await expect(outputsSection.getByText("outputs.command", { exact: false })).toBeVisible();
    await expect(outputsSection.getByRole("button", { name: "Send command" })).toBeDisabled();
  });
});
