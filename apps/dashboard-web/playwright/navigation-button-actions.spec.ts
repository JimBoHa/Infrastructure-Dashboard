import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Critical button actions", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("setup center action buttons route to their destinations", async ({ page }) => {
    await page.goto("/setup");

    await expect(page.locator("main").getByText(/^System Setup Center$/)).toBeVisible();

    await page.getByRole("button", { name: "Backups", exact: true }).click();
    await expect(page).toHaveURL(/\/backups$/);
    await expect(page.locator("main").getByText(/^Backups$/)).toBeVisible();

    await page.goto("/setup");
    await page.getByRole("button", { name: "Deployment", exact: true }).click();
    await expect(page).toHaveURL(/\/deployment$/);
    await expect(page.locator("main").getByText(/deploy & adopt a pi 5 node/i)).toBeVisible();
  });

  test("sidebar links and nodes page buttons trigger the expected actions", async ({ page }) => {
    await page.goto("/nodes");

    const openSidebarIfNeeded = async () => {
      const openButton = page.getByRole("button", { name: "Open navigation" });
      if (await openButton.isVisible().catch(() => false)) {
        await openButton.click();
      }
    };

    const sidebar = page.locator("#dashboard-sidebar");

    await openSidebarIfNeeded();
    await sidebar.getByRole("link", { name: "Backups" }).scrollIntoViewIfNeeded();
    await sidebar.getByRole("link", { name: "Backups" }).click();
    await expect(page).toHaveURL(/\/backups$/);
    await expect(page.locator("main").getByText(/^Backups$/)).toBeVisible();

    await openSidebarIfNeeded();
    await sidebar.getByRole("link", { name: "Setup Center" }).click();
    await expect(page).toHaveURL(/\/setup$/);
    await expect(page.locator("main").getByText(/^System Setup Center$/)).toBeVisible();

    await page.goto("/nodes");
    await page.getByRole("button", { name: /scan for nodes/i }).click();
    await expect(page.getByText(/scan complete:/i)).toBeVisible();

    await page.getByRole("button", { name: "Refresh" }).click();
    await expect(page.getByRole("button", { name: "Complete", exact: true })).toBeVisible();
  });
});
