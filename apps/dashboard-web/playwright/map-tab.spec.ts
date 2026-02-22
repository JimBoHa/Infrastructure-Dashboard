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
    path.resolve(process.cwd(), "..", "..", "manual_screenshots_web", "map_tab");
  await mkdir(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
};

test.describe("Map tab UX", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("renders without street view and keeps panels ordered", async ({ page }) => {
    await page.goto("/map");

    await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();
    await expect(page.getByText("Street view", { exact: false })).toHaveCount(0);
    await expect(page.getByText("Viewport height", { exact: true })).toBeVisible();
    await expect(page.getByText("No markup yet", { exact: false })).toHaveCount(0);
    await expect(page.getByText("Playwright Field", { exact: true })).toBeVisible();

    const baseMap = page.getByText("Base map", { exact: true });
    const devices = page.getByText("Devices", { exact: true });
    const markup = page.getByText("Markup", { exact: true });
    const overlays = page.getByText("Overlays", { exact: true });

    const baseBox = await baseMap.boundingBox();
    const devicesBox = await devices.boundingBox();
    const markupBox = await markup.boundingBox();
    const overlaysBox = await overlays.boundingBox();

    expect(baseBox).toBeTruthy();
    expect(devicesBox).toBeTruthy();
    expect(markupBox).toBeTruthy();
    expect(overlaysBox).toBeTruthy();

    expect((devicesBox as { y: number }).y).toBeGreaterThan((baseBox as { y: number }).y);
    expect((markupBox as { y: number }).y).toBeGreaterThan((devicesBox as { y: number }).y);
    expect((overlaysBox as { y: number }).y).toBeGreaterThan((markupBox as { y: number }).y);

    await maybeSaveScreenshot({ page, name: "map_panels_order" });
  });

  test("node expands to show sensors", async ({ page }) => {
    await page.goto("/map");

    await expect(page.getByText("Devices", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: /Playwright Node/i }).click();
    await expect(page.getByText("Playwright Sensor", { exact: true })).toBeVisible();

    await maybeSaveScreenshot({ page, name: "node_expanded" });
  });
});
