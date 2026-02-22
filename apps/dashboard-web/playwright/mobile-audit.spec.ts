import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

const ROUTES = [
  "/overview",
  "/nodes",
  "/map",
  "/sensors",
  "/schedules",
  "/trends",
  "/power",
  "/analytics",
  "/backups",
  "/setup",
  "/provisioning",
  "/deployment",
  "/connection",
  "/users",
] as const;

test.describe("Mobile layout audit", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await installStubApi(page);
  });

  for (const route of ROUTES) {
    test(`no horizontal overflow: ${route}`, async ({ page }) => {
      await page.goto(route);
      await page.waitForLoadState("domcontentloaded");
      await page.waitForTimeout(200);

      await expect(page.getByRole("banner")).toBeVisible();
      await expect(page.getByRole("button", { name: /open navigation/i })).toBeVisible();
      await expect(page.locator("main")).toBeVisible();
      await expect(page.getByRole("button", { name: /account menu/i })).toBeVisible();

      const dimensions = await page.evaluate(() => ({
        scrollWidth: document.documentElement.scrollWidth,
        clientWidth: document.documentElement.clientWidth,
      }));

      expect(dimensions.scrollWidth).toBeLessThanOrEqual(dimensions.clientWidth + 2);
    });
  }
});
