import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

import { expectNoHorizontalOverflow, expectNoVerticalShiftDuring } from "./helpers/layout";

function requireEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`Missing required env var: ${name}`);
  return value;
}

function tierAVersionLabel(): string {
  const value = (process.env.FARM_TIER_A_VERSION || "").trim();
  return value || "unknown";
}

test.describe("overview telemetry tapestry layout (Tier A)", () => {
  test("does not shift on hover (Tier A screenshots)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_overview_tapestry_layout_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 820 });
    await page.addInitScript(
      ({ token }) => {
        window.sessionStorage.setItem("farmdashboard.auth.token", token);
      },
      { token },
    );

    await page.goto("/overview");
    await expect(page.getByRole("heading", { name: "Overview", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Local sensors", exact: true })).toBeVisible();

    const tapestryCard = page.getByTestId("telemetry-tapestry-card");
    await expect(tapestryCard).toBeVisible();
    await expectNoHorizontalOverflow(tapestryCard, { label: "tapestry card" });

    const rows = page.getByTestId("telemetry-tapestry-rows");
    await expect(rows).toBeVisible();

    const details = page.getByTestId("telemetry-tapestry-details");
    await expect(details).toContainText("Hover cells for details");

    await page.screenshot({ path: path.join(screenshotsDir, "01_overview_tapestry_idle.png"), fullPage: true });

    const firstHeatmapRow = rows.getByRole("img").first();
    const firstCell = firstHeatmapRow.locator("div").first();

    await expectNoVerticalShiftDuring(() => firstCell.hover(), rows, { label: "tapestry rows (hover)", tolerancePx: 2 });
    await expect(details).not.toContainText("Hover cells for details");

    await page.screenshot({ path: path.join(screenshotsDir, "02_overview_tapestry_hover.png"), fullPage: true });
  });
});

