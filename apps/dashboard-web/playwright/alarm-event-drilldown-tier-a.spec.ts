import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

function requireEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`Missing required env var: ${name}`);
  return value;
}

function tierAVersionLabel(): string {
  const value = (process.env.FARM_TIER_A_VERSION || "").trim();
  return value || "unknown";
}

test.describe("alarm event drilldown (Tier A)", () => {
  test("opens an alarm event drawer (Tier A screenshots)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_alarm_event_drilldown_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 860 });
    await page.addInitScript(
      ({ token }) => {
        window.sessionStorage.setItem("farmdashboard.auth.token", token);
      },
      { token },
    );

    await page.goto("/sensors");
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Alarm Events", exact: true })).toBeVisible();

    // Give the alarm events query a chance to resolve so the screenshot reflects the settled state.
    const loading = page.getByText("Loading alarm events…", { exact: true });
    try {
      await loading.waitFor({ state: "hidden", timeout: 20_000 });
    } catch {
      // If the controller is slow, capture whatever state we have (still useful for Tier A evidence).
    }

    await page.screenshot({ path: path.join(screenshotsDir, "01_alarm_events_panel.png"), fullPage: true });

    const openButtons = page.getByRole("button", { name: /View details for alarm event/ });
    const count = await openButtons.count();
    if (count === 0) {
      return;
    }

    await openButtons.first().click();
    const drawer = page.getByTestId("alarm-event-detail-drawer");
    await expect(drawer).toBeVisible();

    await page.screenshot({ path: path.join(screenshotsDir, "02_alarm_event_drawer.png"), fullPage: true });
  });
});
