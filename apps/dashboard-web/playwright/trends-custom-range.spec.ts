import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Trends custom date range", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("supports explicit start/end selection", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    const search = page.getByPlaceholder("Search sensors…");
    await search.waitFor({ timeout: 10_000 });

    await search.fill("Playwright Sensor");
    const checkbox = page
      .locator("label", { hasText: "Playwright Sensor" })
      .locator('input[type="checkbox"]')
      .first();
    await checkbox.check();

    const rangeSelect = page.getByLabel("Range");
    await expect(rangeSelect).toBeVisible();
    await rangeSelect.selectOption("custom");

    const start = page.getByRole("textbox", { name: "Start" });
    const end = page.getByRole("textbox", { name: "End" });
    await expect(start).toBeVisible();
    await expect(end).toBeVisible();

    await start.fill("2026-01-08T00:00");
    await end.fill("2026-01-09T00:00");

    await expect(page.getByText("Start must be before end.")).toHaveCount(0);
    await expect(page.getByText(/Max range is 365d/)).toHaveCount(0);
    await expect(page.locator("canvas").first()).toBeVisible();
  });
});
