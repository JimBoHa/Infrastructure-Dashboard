import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Map navigation stability", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("navigating away from Map does not crash (Sensors, Nodes)", async ({ page }) => {
    const pageErrors: string[] = [];
    const consoleErrors: string[] = [];

    page.on("pageerror", (err) => {
      pageErrors.push(err.stack || err.message);
    });
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        consoleErrors.push(msg.text());
      }
    });

    const assertNoApplicationError = async () => {
      const crash = page.getByText(/application error/i);
      if (await crash.isVisible().catch(() => false)) {
        const details = [
          "Next.js rendered an Application error screen.",
          pageErrors.length ? `pageerror:\n${pageErrors.join("\n\n")}` : null,
          consoleErrors.length ? `console.error:\n${consoleErrors.slice(-10).join("\n")}` : null,
        ]
          .filter(Boolean)
          .join("\n\n");
        throw new Error(details);
      }
    };

    const openSidebarIfClosed = async () => {
      const sidebar = page.locator("#dashboard-sidebar");
      const box = await sidebar.boundingBox();
      if (!box) return;

      if (box.x >= 0) return;

      const openButton = page.getByRole("button", { name: /open navigation/i });
      if (await openButton.isVisible().catch(() => false)) {
        await openButton.click();
      }

      await expect.poll(async () => (await sidebar.boundingBox())?.x).toBeGreaterThanOrEqual(0);
    };

    await page.goto("/map");
    await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();

    // Allow MapLibre to initialize; the bug tends to reproduce during teardown.
    await page.waitForTimeout(750);

    await openSidebarIfClosed();
    const sidebar = page.getByRole("dialog", { name: "Navigation" });
    const sensorsLink = sidebar.getByRole("link", { name: /Sensors & Outputs/i });
    await sensorsLink.scrollIntoViewIfNeeded();
    await sensorsLink.click();
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();
    await assertNoApplicationError();

    await page.goto("/map");
    await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();
    await page.waitForTimeout(750);

    await openSidebarIfClosed();
    const nodesLink = sidebar.getByRole("link", { name: /^Nodes/i });
    await nodesLink.scrollIntoViewIfNeeded();
    await nodesLink.click();
    await expect(page.getByRole("heading", { name: "Nodes", exact: true })).toBeVisible();
    await assertNoApplicationError();
  });
});
