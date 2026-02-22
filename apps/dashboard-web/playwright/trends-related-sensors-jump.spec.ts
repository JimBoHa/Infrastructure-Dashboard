import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

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

  const checkbox = page.getByRole("checkbox", { name: new RegExp(name, "i") }).first();
  await expect(checkbox).toBeVisible({ timeout: 10_000 });
  await checkbox.scrollIntoViewIfNeeded();
  const row = checkbox.locator("xpath=ancestor::*[contains(@class,'cursor-pointer')][1]");
  await row.dispatchEvent("click");
  await page.waitForTimeout(150);
};

test.describe("Trends related sensors jump-to-timestamp (stub)", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("system-wide Jump to ±1h sets custom range window", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    const search = page.getByPlaceholder("Search sensors…");
    await search.waitFor({ timeout: 10_000 });

    await selectSensor({ page, search, name: "Playwright Voltage" });
    await expect(page.getByText("1/20 selected")).toBeVisible({ timeout: 10_000 });
    await search.fill("");

    const panel = page.getByTestId("relationship-finder-panel");
    await expect(panel).toBeVisible({ timeout: 10_000 });

    const run = panel.getByRole("button", { name: "Find related sensors", exact: true });
    await expect(run).toBeVisible({ timeout: 10_000 });
    await run.dispatchEvent("click");

    await expect(panel.getByText("completed")).toBeVisible({ timeout: 20_000 });

    await panel.getByRole("button", { name: "Advanced", exact: true }).dispatchEvent("click");
    await expect(panel.getByText("System-wide events")).toBeVisible({ timeout: 15_000 });

    const jump = panel.getByRole("button", { name: "Jump to ±1h" }).first();
    await expect(jump).toBeVisible({ timeout: 15_000 });
    await jump.dispatchEvent("click");

    const rangeSelect = page.getByRole("combobox", { name: /^range$/i }).first();
    await expect(rangeSelect).toHaveValue("custom");

    await expect(page.getByLabel("Start", { exact: true })).toBeVisible();
    await expect(page.getByLabel("End", { exact: true })).toBeVisible();
  });
});
