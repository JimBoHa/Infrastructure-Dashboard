import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("analytics weather station", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("renders WS-2902 section with charts", async ({ page }, testInfo) => {
    await page.setViewportSize({ width: 1280, height: 820 });
    await page.goto("/analytics", { waitUntil: "domcontentloaded" });

    await expect(page.getByRole("heading", { name: "Analytics" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Weather stations" })).toBeVisible();
    await expect(page.getByText("Playwright Weather Station")).toBeVisible();

    await expect(page.getByText("Temperature & humidity — past 24 hours")).toBeVisible();

    await page.screenshot({
      path: `/tmp/playwright-analytics-weather-station-${testInfo.project.name}.png`,
      fullPage: true,
    });
  });
});
