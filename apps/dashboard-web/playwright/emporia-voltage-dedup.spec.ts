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
    path.resolve(process.cwd(), "..", "..", "manual_screenshots_web", "emporia_voltage");
  await mkdir(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
};

test.describe("Emporia voltage semantics", () => {
  test.beforeEach(async ({ page }) => {
    const isoNow = new Date("2026-01-09T00:00:00.000Z").toISOString();
    await installStubApi(page, {
      jsonByPath: {
        "/api/nodes": [
          {
            id: "emporia-node-1",
            name: "Emporia Main Panel",
            status: "online",
            uptime_seconds: 1234,
            cpu_percent: 12.3,
            storage_used_bytes: 1024 * 1024 * 1024,
            mac_eth: null,
            mac_wifi: null,
            ip_last: "192.168.1.10",
            last_seen: isoNow,
            created_at: isoNow,
            config: { external_provider: "emporia", external_id: "457116", power_provider: "emporia_cloud" },
          },
        ],
        "/api/sensors": [
          {
            sensor_id: "emporia-mains-power",
            node_id: "emporia-node-1",
            name: "Mains Power",
            type: "power",
            unit: "W",
            interval_seconds: 30,
            rolling_avg_seconds: 0,
            latest_value: 4200,
            status: "online",
            location: null,
            created_at: isoNow,
            config: { source: "emporia_cloud", metric: "mains_power_w" },
          },
          {
            sensor_id: "emporia-mains-l1-v",
            node_id: "emporia-node-1",
            name: "Mains L1 Voltage",
            type: "voltage",
            unit: "V",
            interval_seconds: 30,
            rolling_avg_seconds: 0,
            latest_value: 117.5,
            status: "online",
            location: null,
            created_at: isoNow,
            config: { source: "emporia_cloud", metric: "channel_voltage_v", is_mains: true, channel_key: "Mains_A" },
          },
          {
            sensor_id: "emporia-mains-l2-v",
            node_id: "emporia-node-1",
            name: "Mains L2 Voltage",
            type: "voltage",
            unit: "V",
            interval_seconds: 30,
            rolling_avg_seconds: 0,
            latest_value: 118.5,
            status: "online",
            location: null,
            created_at: isoNow,
            config: { source: "emporia_cloud", metric: "channel_voltage_v", is_mains: true, channel_key: "Mains_B" },
          },
          {
            sensor_id: "emporia-circuit-power",
            node_id: "emporia-node-1",
            name: "Closet Telecom",
            type: "power",
            unit: "W",
            interval_seconds: 30,
            rolling_avg_seconds: 0,
            latest_value: 240,
            status: "online",
            location: null,
            created_at: isoNow,
            config: { source: "emporia_cloud", metric: "channel_power_w", channel_key: "6", channel_name: "Closet Telecom" },
          },
          {
            sensor_id: "emporia-circuit-current",
            node_id: "emporia-node-1",
            name: "Closet Telecom Current",
            type: "current",
            unit: "A",
            interval_seconds: 30,
            rolling_avg_seconds: 0,
            latest_value: 2.0,
            status: "online",
            location: null,
            created_at: isoNow,
            config: { source: "emporia_cloud", metric: "channel_current_a", channel_key: "6", channel_name: "Closet Telecom" },
          },
        ],
      },
    });
  });

  test("Power page shows mains leg voltages and derives circuit voltage", async ({ page }) => {
    await page.goto("/power");
    await page.waitForLoadState("networkidle").catch(() => {});

    await expect(page.getByText(/Emporia mains power/i)).toBeVisible();
    await expect(page.getByText(/L1\s+117\.5\s*V/i)).toBeVisible();
    await expect(page.getByText(/L2\s+118\.5\s*V/i)).toBeVisible();

    const table = page.getByRole("table");
    await expect(table.getByText("Closet Telecom")).toBeVisible();

    await expect(
      table.getByTitle(/Computed from Power ÷ Current/i),
    ).toBeVisible();

    await maybeSaveScreenshot({ page, name: "power_emporia_mains_voltage" });
  });
});
