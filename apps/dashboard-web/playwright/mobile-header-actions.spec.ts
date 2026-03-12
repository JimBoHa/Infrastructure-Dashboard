import { expect, test } from "@playwright/test";
import type { Locator } from "@playwright/test";

import { installStubApi } from "./stubApi";

const firstVisible = async (locator: Locator, label: string): Promise<Locator> => {
  const count = await locator.count();
  for (let i = 0; i < count; i += 1) {
    const candidate = locator.nth(i);
    if (await candidate.isVisible()) return candidate;
  }
  throw new Error(`No visible match found for ${label} (count=${count})`);
};

test.describe("Mobile header actions", () => {
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

  test("account menu opens on tap and closes on escape", async ({ page }) => {
    await page.goto("/nodes");

    const accountButton = page.getByRole("button", { name: "Account menu" });
    await accountButton.click();

    const logoutItems = page.getByRole("menuitem", { name: "Log out" });
    await expect(await firstVisible(logoutItems, "Log out menuitem")).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(logoutItems.first()).toBeHidden();
  });
});
