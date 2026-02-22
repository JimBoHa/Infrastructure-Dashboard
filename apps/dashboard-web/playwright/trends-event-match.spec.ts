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

test.describe("Trends event match (stub)", () => {
  test.skip(({ browserName, isMobile }) => browserName !== "chromium" || isMobile, "Desktop-only coverage.");

  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("runs event match job and shows preview drilldown", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    await page.getByPlaceholder("Search sensors…").waitFor({ timeout: 10_000 });
    await selectSensor(page, "Playwright Voltage");
    await selectSensor(page, "Playwright Power");
    await expect(page.getByText("2/20 selected")).toBeVisible({ timeout: 10_000 });

    const panel = page.getByTestId("trends-event-match");
    await ensureDetailsOpen(panel);

    const runButton = panel.getByRole("button", { name: "Run job", exact: true });
    await expect(runButton).toBeEnabled({ timeout: 15_000 });
    await runButton.dispatchEvent("click");
    await expect(runButton).toBeDisabled();

    const summary = panel.getByTestId("event-match-job-summary");
    await expect(summary).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText("Computed through:", { exact: false })).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText(/Bucket size/i)).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText(/Buckets:/i)).toBeVisible({ timeout: 20_000 });
    await expect(summary.getByText(/Candidates truncated/i)).toBeVisible({ timeout: 20_000 });

    const results = panel.getByTestId("event-match-results");
    await expect(results).toBeVisible({ timeout: 20_000 });
    await expect(panel.getByTestId("event-match-truncation-watermark")).toBeVisible({ timeout: 20_000 });

    const candidateButton = panel.getByTestId("event-match-candidate-playwright-sensor-power");
    await candidateButton.click();

    const preview = panel.getByTestId("event-match-preview");
    await expect(preview).toBeVisible();
    await expect(preview.getByText("Preview bucket size", { exact: false })).toBeVisible({ timeout: 10_000 });
  });
});
