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

test.describe("overview local sensor visualizations", () => {
  test("renders Overview local panels (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_overview_local_visuals_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 820 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    await page.goto("/overview");
    await expect(page.getByRole("heading", { name: "Overview", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Local sensors", exact: true })).toBeVisible();

    const hasDataPanels = (await page.getByText("Telemetry tapestry", { exact: true }).count()) > 0;
    if (hasDataPanels) {
      await expect(page.getByText("Telemetry tapestry", { exact: true })).toBeVisible();
      await expect(page.getByText("Sparkline mosaic", { exact: true })).toBeVisible();
    } else {
      await expect(page.getByText("No local sensors detected yet.", { exact: true })).toBeVisible();
    }

    await page.screenshot({ path: path.join(screenshotsDir, "01_overview_local_visuals.png"), fullPage: true });

    await page.getByRole("button", { name: "Configure local sensors", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Configure local sensors", exact: true })).toBeVisible();

    await page.screenshot({ path: path.join(screenshotsDir, "02_overview_local_sensors_config.png"), fullPage: true });
  });
});
