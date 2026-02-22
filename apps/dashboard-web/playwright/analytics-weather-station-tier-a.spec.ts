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

test.describe("analytics weather station (Tier A)", () => {
  test("renders weather station section (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_analytics_weather_station_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 820 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    await page.goto("/analytics");
    await expect(page.getByRole("heading", { name: "Analytics", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Weather stations", exact: true })).toBeVisible();

    const weatherStationsCard = page.locator("section", {
      has: page.getByRole("heading", { name: "Weather stations", exact: true }),
    });
    const stationPanels = weatherStationsCard.locator("details");

    if ((await stationPanels.count()) > 0) {
      const first = stationPanels.first();
      const isOpen = await first.evaluate((node) => (node as HTMLDetailsElement).open);
      if (!isOpen) {
        await first.locator("summary").click();
      }

      await expect(
        weatherStationsCard.getByText("Temperature & humidity — past 24 hours", { exact: true }),
      ).toBeVisible();
    } else {
      await expect(
        weatherStationsCard.getByText("No weather station nodes detected yet.", { exact: false }),
      ).toBeVisible();
    }

    await page.screenshot({
      path: path.join(screenshotsDir, "01_analytics_weather_station.png"),
      fullPage: true,
    });
  });
});

