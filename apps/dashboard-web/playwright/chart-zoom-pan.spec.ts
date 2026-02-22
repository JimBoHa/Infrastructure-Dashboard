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
    path.resolve(process.cwd(), "..", "..", "manual_screenshots_web", "chart_zoom_pan");
  await mkdir(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
};

test.describe("Chart pan/zoom interactions", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("supports x-axis wheel zoom, drag pan, and double-click reset", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    const search = page.getByPlaceholder("Search sensors…");
    await search.waitFor({ timeout: 10_000 });

    for (const name of ["Playwright Sensor", "Playwright Voltage", "Playwright Current"]) {
      await search.fill(name);
      await page.waitForTimeout(150);
      const checkbox = page
        .locator("label", { hasText: name })
        .locator('input[type="checkbox"]')
        .first();
      await expect(checkbox).toHaveCount(1);
      await checkbox.check();
      await page.waitForTimeout(150);
    }

    const canvas = page.locator("canvas").first();
    await expect(canvas).toBeVisible();

    await maybeSaveScreenshot({ page, name: "00_before" });

    const box = await canvas.boundingBox();
    expect(box).toBeTruthy();
    if (!box) return;

    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.wheel(0, -800);
    await page.waitForTimeout(300);
    await maybeSaveScreenshot({ page, name: "01_zoomed" });

    await page.mouse.down();
    await page.mouse.move(box.x + box.width / 2 + 220, box.y + box.height / 2);
    await page.mouse.up();
    await page.waitForTimeout(300);
    await maybeSaveScreenshot({ page, name: "02_panned" });

    await canvas.dblclick({
      position: { x: box.width / 2, y: box.height / 2 },
    });
    await page.waitForTimeout(300);
    await maybeSaveScreenshot({ page, name: "03_reset" });
  });
});
