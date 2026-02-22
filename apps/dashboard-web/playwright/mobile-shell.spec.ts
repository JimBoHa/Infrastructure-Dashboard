import { expect, test } from "@playwright/test";
import type { Locator } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import path from "node:path";

import { installStubApi } from "./stubApi";

const firstVisible = async (locator: Locator, label: string): Promise<Locator> => {
  const count = await locator.count();
  for (let i = 0; i < count; i += 1) {
    const candidate = locator.nth(i);
    if (await candidate.isVisible()) return candidate;
  }
  throw new Error(`No visible match found for ${label} (count=${count})`);
};

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
    path.resolve(process.cwd(), "..", "..", "manual_screenshots_web", "mobile_shell");
  await mkdir(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
};

test.describe("Mobile shell interactions", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await installStubApi(page);
  });

  test("hamburger toggles sidebar in mobile viewport", async ({ page }) => {
    await page.goto("/nodes");

    const sidebar = page.locator("#dashboard-sidebar");
    const openButton = page.getByRole("button", { name: "Open navigation" });

    const closedBox = await sidebar.boundingBox();
    expect(closedBox).toBeTruthy();
    expect((closedBox as { x: number }).x).toBeLessThan(0);

    await openButton.click();
    await expect.poll(async () => (await sidebar.boundingBox())?.x).toBeGreaterThanOrEqual(0);

    await maybeSaveScreenshot({ page, name: "sidebar_open" });

    const backdrop = page.getByTestId("sidebar-backdrop");
    const backdropBox = await backdrop.boundingBox();
    expect(backdropBox).toBeTruthy();
    await backdrop.click({
      position: {
        x: (backdropBox as { width: number }).width - 8,
        y: (backdropBox as { height: number }).height / 2,
      },
    });
    await expect.poll(async () => (await sidebar.boundingBox())?.x).toBeLessThan(0);
  });

  test("account dropdown opens on tap and closes on outside tap", async ({ page }) => {
    await page.goto("/nodes");

    const accountButton = page.getByRole("button", { name: "Account menu" });
    await accountButton.click();
    await expect(page.getByRole("button", { name: "Log out" })).toBeVisible();

    await maybeSaveScreenshot({ page, name: "account_menu_open" });

    await page.locator("main").click();
    await expect(page.getByRole("button", { name: "Log out" })).toBeHidden();
  });

  test("sensors detail drawer opens without crashing", async ({ page }) => {
    await page.goto("/sensors");

    const nodeCard = page.locator('details:has(> summary:has-text("Playwright Node"))').first();
    await nodeCard.locator(":scope > summary").click({ force: true });
    await expect
      .poll(async () => nodeCard.evaluate((el) => (el as HTMLDetailsElement).open))
      .toBe(true);

    const sensorRows = nodeCard.getByText("Playwright Sensor", { exact: true });
    await expect(sensorRows.first()).toBeAttached();
    const sensorRow = await firstVisible(sensorRows, "Playwright Sensor row");
    await sensorRow.scrollIntoViewIfNeeded();
    await sensorRow.click();
    await expect(page.getByRole("heading", { name: "Playwright Sensor" })).toBeVisible();

    await maybeSaveScreenshot({ page, name: "sensor_drawer_open" });

    await page.getByRole("button", { name: "Close", exact: true }).click();
    await expect(page.getByRole("heading", { name: "Playwright Sensor" })).toBeHidden();
  });
});
