import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Node detail sensor deeplink", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("clicking a sensor on the node detail page opens it in Sensors & Outputs", async ({ page }) => {
    await page.goto("/nodes");

    await page.getByRole("button", { name: /more details/i }).first().tap();

    await expect(page).toHaveURL(/\/nodes\/detail\?/);
    await page.getByText("Playwright Sensor", { exact: false }).first().tap();

    await expect(page).toHaveURL(/\/sensors\/?\?/);

    await expect(page.getByRole("heading", { name: /Playwright Sensor/i })).toBeVisible();
  });
});
