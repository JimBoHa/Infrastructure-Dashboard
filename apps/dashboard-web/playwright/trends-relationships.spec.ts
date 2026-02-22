import { expect, test, type Locator } from "@playwright/test";

import { installStubApi } from "./stubApi";

async function ensureDetailsOpen(details: Locator) {
  await expect(details).toBeVisible({ timeout: 15_000 });
  await details.scrollIntoViewIfNeeded();
  const open = await details.evaluate((node) => (node as HTMLDetailsElement).open).catch(() => false);
  if (!open) {
    await details.locator("summary").first().click({ force: true });
  }
}

const selectSensor = async (page: import("@playwright/test").Page, name: string) => {
  const search = page.getByPlaceholder("Search sensors…");
  await search.fill(name);
  await page.waitForTimeout(150);

  const checkbox = page
    .locator("label", { hasText: name })
    .locator('input[type="checkbox"]')
    .first();
  await expect(checkbox).toHaveCount(1);
  await checkbox.scrollIntoViewIfNeeded();
  await checkbox.check({ force: true });

  await search.fill("");
  await page.waitForTimeout(250);
};

test.describe("Trends relationships (stub)", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("runs correlation matrix job and shows pair analysis", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    await page.getByPlaceholder("Search sensors…").waitFor({ timeout: 10_000 });
    await selectSensor(page, "Playwright Voltage");
    await selectSensor(page, "Playwright Current");
    await selectSensor(page, "Playwright Power");
    await expect(page.getByText("3/20 selected")).toBeVisible({ timeout: 10_000 });

    const panel = page.getByTestId("trends-relationships");
    await ensureDetailsOpen(panel);

    const runButton = panel.getByRole("button", { name: "Run analysis", exact: true });
    await expect(runButton).toBeEnabled({ timeout: 15_000 });
    await runButton.dispatchEvent("click");
    await expect(runButton).toBeDisabled();

    await expect(panel.getByRole("button", { name: "Cancel", exact: true })).toBeVisible({ timeout: 10_000 });
    await expect(panel.getByText(/\(\d+\/5\)/)).toBeVisible({ timeout: 10_000 });

    const matrixSummary = panel.locator("summary").filter({ hasText: "Correlation matrix" }).first();
    await expect(matrixSummary).toBeVisible({ timeout: 15_000 });
    await expect(panel.getByText("Computed through:", { exact: false })).toBeVisible();
    await expect(panel.getByText("Bucket size:", { exact: false })).toBeVisible();
    await expect(panel.getByText("Buckets:", { exact: false })).toBeVisible();
    await expect(panel.getByText("Interval adjusted from", { exact: false })).toBeVisible();
    await expect(panel.getByTestId("relationships-truncation-watermark")).toBeVisible();

    const firstCell = panel.locator("tbody button:not([disabled])").first();
    await expect(firstCell).toBeVisible();
    await firstCell.click();
    await expect(panel.getByText("Pair analysis", { exact: true })).toBeVisible();
  });
});
