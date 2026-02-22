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

test.describe("map layout", () => {
  test("fills remaining viewport height on desktop (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_map_viewport_fill_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 820 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    await page.goto("/map");
    await expect(page.getByRole("heading", { name: "Map", exact: true })).toBeVisible();
    await expect(page.locator("#map-canvas")).toBeVisible();

    const viewport = page.viewportSize();
    expect(viewport).not.toBeNull();

    const mapBox = await page.locator("#map-canvas").boundingBox();
    expect(mapBox).not.toBeNull();

    const sidebarBox = await page.locator("#map-canvas + div").boundingBox();
    expect(sidebarBox).not.toBeNull();

    const viewportHeight = viewport!.height;
    const tolerancePx = 40;

    expect(mapBox!.y + mapBox!.height).toBeGreaterThanOrEqual(viewportHeight - tolerancePx);
    expect(sidebarBox!.y + sidebarBox!.height).toBeGreaterThanOrEqual(viewportHeight - tolerancePx);

    await page.screenshot({ path: path.join(screenshotsDir, "01_map_viewport_fill.png") });
  });
});

