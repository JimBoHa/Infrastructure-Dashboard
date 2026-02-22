import { expect, test } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import path from "node:path";

import { installStubApi } from "./stubApi";

const maybeSaveScreenshot = async ({
  page,
  name,
}: {
  page: import("@playwright/test").Page;
  name: string;
}) => {
  if (!process.env.FARM_PLAYWRIGHT_SAVE_SCREENSHOTS) return;
  const dir =
    process.env.FARM_PLAYWRIGHT_SCREENSHOT_DIR ||
    path.resolve(process.cwd(), "..", "..", "manual_screenshots_web", "tier_a_deployment");
  await mkdir(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
};

test.describe("Deployment end-to-end UX", () => {
  test.beforeEach(async ({ page }) => {
    const isoNow = new Date("2026-01-09T00:00:00.000Z").toISOString();
    await installStubApi(page, {
      jsonByPath: {
        "/api/scan": [
          {
            service_name: "pi5-newnode._iotnode._tcp.local.",
            hostname: "pi5-newnode.local.",
            ip: "10.0.0.42",
            port: 9000,
            mac_eth: "2c:cf:67:8e:38:a8",
            mac_wifi: null,
            properties: {},
          },
        ],
        "/api/deployments/pi5": {
          id: "deploy-123",
          status: "success",
          created_at: isoNow,
          started_at: isoNow,
          finished_at: isoNow,
          steps: [
            { name: "connect", status: "completed", logs: [] },
            { name: "install", status: "completed", logs: [] },
          ],
          error: null,
          outcome: "installed",
          node: {
            node_id: "pi5-newnode",
            node_name: "New Node",
            mac_eth: "2c:cf:67:8e:38:a8",
            mac_wifi: null,
            adoption_token: "debug-token",
            host: "10.0.0.42",
          },
        },
        "/api/adoption/tokens": { token: "controller-issued-token" },
        "/api/adopt": {
          id: "pi5-newnode",
          name: "New Node",
          status: "online",
          uptime_seconds: null,
          cpu_percent: null,
          storage_used_bytes: null,
          mac_eth: "2c:cf:67:8e:38:a8",
          mac_wifi: null,
          ip_last: "10.0.0.42",
          last_seen: isoNow,
          created_at: isoNow,
          config: {},
        },
      },
    });
  });

  test("Provisioning entry points are removed and route redirects", async ({ page }) => {
    await page.goto("/nodes");
    await expect(page.getByText("Provisioning")).toHaveCount(0);

    await page.goto("/provisioning");
    await expect(page).toHaveURL(/deployment/);
    await expect(page.getByRole("heading", { name: /deploy & adopt a pi 5 node/i })).toBeVisible();

    await maybeSaveScreenshot({ page, name: "provisioning_redirects_to_deployment" });
  });

  test("Deploy tab includes adopt step and pre-fills adoption name", async ({ page }) => {
    await page.goto("/deployment");

    await page.getByLabel(/pi ip/i).fill("10.0.0.42");
    await page.getByLabel(/username/i).fill("pi");
    await page.getByLabel(/^password$/i).fill("raspberry");
    await page.getByLabel(/node display name/i).fill("New Node");

    await page.getByRole("button", { name: /connect & deploy/i }).click();

    await expect(page.getByRole("heading", { name: /deployment status/i })).toBeVisible();
    await expect(page.getByRole("heading", { name: /adopt and configure sensors/i })).toBeVisible();

    await page.getByRole("button", { name: /scan lan/i }).click();
    await expect(page.getByText("pi5-newnode.local.")).toBeVisible();

    await maybeSaveScreenshot({ page, name: "deployment_adopt_card_ready" });

    await page.getByRole("button", { name: /adopt now/i }).click();

    const modal = page.getByRole("heading", { name: /adopt node/i });
    await expect(modal).toBeVisible();

    const nameInput = page.getByRole("textbox", { name: "Display name", exact: true });
    await expect(nameInput).toHaveValue("New Node");

    await maybeSaveScreenshot({ page, name: "deployment_adoption_modal_prefilled" });

    await page.getByRole("button", { name: /^adopt$/i }).click();

    await expect(page.getByRole("button", { name: /configure sensors/i })).toBeVisible();

    await maybeSaveScreenshot({ page, name: "deployment_adopted_configure_sensors" });
  });
});
