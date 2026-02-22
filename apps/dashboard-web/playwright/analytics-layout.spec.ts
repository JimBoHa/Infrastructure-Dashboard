import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Analytics layout", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("renders without horizontal overflow", async ({ page }) => {
    await page.goto("/analytics", { waitUntil: "domcontentloaded" });

    await expect(page.getByRole("heading", { name: "Analytics" })).toBeVisible();

    const hasOverflow = await page.evaluate(() => {
      const root = document.documentElement;
      return root.scrollWidth > root.clientWidth + 2;
    });
    expect(hasOverflow).toBeFalsy();

    await page.screenshot({
      path: "/tmp/playwright-analytics-layout.png",
      fullPage: true,
    });
  });
});

