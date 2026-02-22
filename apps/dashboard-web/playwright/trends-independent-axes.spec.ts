import { expect, test } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import path from "node:path";

import { installStubApi } from "./stubApi";

const maybeSaveScreenshot = async ({
  page,
  name,
}: {
  page: import("@playwright/test").Page;
  name: string;
}) => {
  if (!process.env.FARM_PLAYWRIGHT_SAVE_SCREENSHOTS) return;
  const dir =
    process.env.FARM_PLAYWRIGHT_SCREENSHOT_DIR ||
    path.resolve(process.cwd(), "..", "..", "manual_screenshots_web", "trends_independent_axes");
  await mkdir(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
};

const selectSensor = async ({
  page,
  search,
  name,
}: {
  page: import("@playwright/test").Page;
  search: import("@playwright/test").Locator;
  name: string;
}) => {
  await search.fill(name);
  await page.waitForTimeout(150);

  const checkbox = page
    .locator("label", { hasText: name })
    .locator('input[type="checkbox"]')
    .first();
  await expect(checkbox).toHaveCount(1);
  await checkbox.check();
  await page.waitForTimeout(150);
};

test.describe("Trends independent axes", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("does not runaway-expand page height", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    const search = page.getByPlaceholder("Search sensors…");
    await search.waitFor({ timeout: 10_000 });

    const sensors = ["Playwright Sensor", "Playwright Voltage", "Playwright Current", "Playwright Power"];
    for (const name of sensors) {
      await selectSensor({ page, search, name });
    }

    await search.fill("");

    const independent = page.getByLabel("Independent axes");
    await independent.check();

    await expect(page.locator("canvas").first()).toBeVisible();
    await expect(
      page.getByText(/Independent axes: hover a series in the legend/),
    ).toBeVisible();
    await page.waitForTimeout(750);

    const heights: number[] = [];
    for (let i = 0; i < 12; i += 1) {
      const height = await page.evaluate(() => document.documentElement.scrollHeight);
      heights.push(height);
      if (i === 0 || i === 5 || i === 11) {
        await maybeSaveScreenshot({ page, name: `trends_independent_${i}` });
      }
      await page.waitForTimeout(1000);
    }

    const tail = heights.slice(2);
    const min = Math.min(...tail);
    const max = Math.max(...tail);
    expect(max - min).toBeLessThan(160);
  });
});
