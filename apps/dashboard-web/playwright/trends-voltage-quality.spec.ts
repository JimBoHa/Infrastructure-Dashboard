import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Trends voltage quality", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("does not show voltage quality panel (moved to Power tab)", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    const search = page.getByPlaceholder("Search sensors…");
    await search.waitFor({ timeout: 10_000 });
    await search.fill("Playwright Voltage");

    const checkbox = page
      .locator("label", { hasText: "Playwright Voltage" })
      .locator('input[type="checkbox"]')
      .first();
    await checkbox.check();

    await expect(page.getByText("AC voltage quality", { exact: true })).toHaveCount(0);

    await page.screenshot({
      path: "/tmp/playwright-trends-voltage-quality.png",
      fullPage: true,
    });
  });
});
