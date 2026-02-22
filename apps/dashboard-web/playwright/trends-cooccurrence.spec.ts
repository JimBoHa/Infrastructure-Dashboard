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

test.describe("Trends co-occurrence (stub)", () => {
  test.skip(({ browserName, isMobile }) => browserName !== "chromium" || isMobile, "Desktop-only coverage.");

  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("runs co-occurrence job and shows results", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    await page.getByPlaceholder("Search sensors…").waitFor({ timeout: 10_000 });
    await selectSensor(page, "Playwright Voltage");
    await selectSensor(page, "Playwright Current");
    await expect(page.getByText("2/20 selected")).toBeVisible({ timeout: 10_000 });

    const panel = page.getByTestId("trends-cooccurrence");
    await ensureDetailsOpen(panel);

    const runButton = panel.getByRole("button", { name: "Run analysis", exact: true });
    await expect(runButton).toBeEnabled({ timeout: 15_000 });
    await runButton.dispatchEvent("click");
    await expect(runButton).toBeDisabled();

    const summary = panel.getByTestId("cooccurrence-job-summary");
    await expect(summary).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText("Computed through:", { exact: false })).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText(/Bucket size/i)).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText(/Buckets:/i)).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText(/Sensors truncated/i)).toBeVisible({ timeout: 20_000 });
    await expect(panel.getByTestId("cooccurrence-truncation-watermark")).toBeVisible({ timeout: 20_000 });

    await expect(panel.getByText("Details", { exact: true })).toBeVisible({ timeout: 20_000 });
  });
});
