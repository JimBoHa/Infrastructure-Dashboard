import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Nodes scan UX", () => {
  test.beforeEach(async ({ page }) => {
    const isoNow = new Date("2026-01-09T00:00:00.000Z").toISOString();
    await installStubApi(page, {
      jsonByPath: {
        "/api/nodes": [
          {
            id: "playwright-node-1",
            name: "Playwright Node",
            status: "online",
            uptime_seconds: 1234,
            cpu_percent: 12.3,
            storage_used_bytes: 1024 * 1024 * 1024,
            mac_eth: "2c:cf:67:8e:38:a8",
            mac_wifi: null,
            ip_last: "192.168.1.10",
            last_seen: isoNow,
            created_at: isoNow,
            config: {},
          },
        ],
        "/api/scan": [
          {
            service_name: "pi5-node2._iotnode._tcp.local.",
            hostname: "pi5-node2.local.",
            ip: "10.255.8.20",
            port: 9000,
            mac_eth: "2C:CF:67:8E:38:A8",
            mac_wifi: null,
            adoption_token: "stubtoken",
            properties: {},
          },
        ],
      },
    });
  });

  test("scan results include already-adopted nodes (not silently hidden)", async ({ page }) => {
    await page.goto("/nodes");

    await page.waitForLoadState("networkidle").catch(() => {});

    const scanned = await page.evaluate(async () => {
      const response = await fetch("/api/scan");
      const payload = await response.json();
      return Array.isArray(payload) ? payload.length : null;
    });
    expect(scanned).toBe(1);

    await page.getByRole("button", { name: /scan for nodes/i }).tap();
    await expect(page.getByText(/scan complete: found 1/i)).toBeVisible();

    await page.getByRole("button", { name: /more details/i }).first().tap();

    await expect(page).toHaveURL(/\/nodes\/detail\?/);
    await expect(page.getByRole("heading", { name: /Playwright Node/i })).toBeVisible();
  });
});
