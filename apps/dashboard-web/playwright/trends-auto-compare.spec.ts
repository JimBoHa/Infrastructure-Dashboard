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
    path.resolve(process.cwd(), "..", "..", "manual_screenshots_web", "trends_auto_compare");
  await mkdir(dir, { recursive: true });
  await page.screenshot({ path: path.join(dir, `${name}.png`), fullPage: true });
};

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

  const checkbox = page
    .locator("label", { hasText: name })
    .locator('input[type="checkbox"]')
    .first();
  await expect(checkbox).toHaveCount(1);
  await checkbox.scrollIntoViewIfNeeded();
  await checkbox.check({ force: true });
  await page.waitForTimeout(150);
};

test.describe("Trends auto-compare suggestions", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("suggests related sensors and can add to chart", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    const search = page.getByPlaceholder("Search sensors…");
    await search.waitFor({ timeout: 10_000 });

    await selectSensor({ page, search, name: "Playwright Voltage" });
    await expect(page.getByText("1/20 selected")).toBeVisible({ timeout: 10_000 });
    await search.fill("");

    const panel = page.getByTestId("trends-auto-compare");
    await expect(panel).toBeVisible();
    await expect(panel.getByTestId("auto-compare-computed-through")).toContainText("Computed through");

    await panel.scrollIntoViewIfNeeded();
    const maxCandidates = panel.getByLabel("Max candidates");
    await maxCandidates.scrollIntoViewIfNeeded();
    await maxCandidates.fill("1");
    await maxCandidates.blur();

    const runAnalysis = panel.getByRole("button", { name: "Run analysis" });
    await runAnalysis.dispatchEvent("click");
    await expect(runAnalysis).toBeDisabled();

    await expect(panel.getByRole("button", { name: "Cancel job" })).toBeVisible({ timeout: 10_000 });
    await expect(panel.getByText(/\(\d+\/5\)/)).toBeVisible({ timeout: 10_000 });

    const suggestions = page.getByTestId("auto-compare-suggestions");
    await expect(suggestions).toBeVisible({ timeout: 15_000 });

    await expect(panel.getByText("Computed through:", { exact: false })).toBeVisible();
    await expect(panel.getByText(/Candidate pool trimmed/i)).toBeVisible();
    await expect(suggestions.getByText(/Playwright Power/)).toBeVisible();
    await expect(suggestions.getByText(/Rank #1/)).toBeVisible();

    const previewPower = suggestions
      .locator('button[title="Preview relationship"]', { hasText: /Playwright Power/ })
      .first();
    await previewPower.dispatchEvent("click");
    await expect(panel.getByTestId("auto-compare-episodes")).toBeVisible();
    await expect(panel.getByText("Preview bucket size")).toBeVisible({ timeout: 10_000 });

    const addPower = page.getByTestId("auto-compare-add-playwright-sensor-power");
    await expect(addPower).toBeEnabled();
    // Click can be intercepted in small viewports due to fixed headers/overlays; dispatch a click
    // event to validate behavior without flakiness.
    await addPower.dispatchEvent("click");

    await expect(page.getByText("2/20 selected")).toBeVisible({ timeout: 10_000 });
    await expect(page.locator('button[title="Remove from chart"]').filter({ hasText: /Playwright Power/ })).toBeVisible();

    await maybeSaveScreenshot({ page, name: "01_trends_auto_compare" });
  });

  test("shows progress, cancel state, and computed-through watermark", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    const search = page.getByPlaceholder("Search sensors…");
    await search.waitFor({ timeout: 10_000 });

    await selectSensor({ page, search, name: "Playwright Voltage" });
    await expect(page.getByText("1/20 selected")).toBeVisible({ timeout: 10_000 });
    await search.fill("");

    const panel = page.getByTestId("trends-auto-compare");
    await expect(panel).toBeVisible();
    await panel.scrollIntoViewIfNeeded();

    const runAnalysis = panel.getByRole("button", { name: "Run analysis" });
    await runAnalysis.dispatchEvent("click");

    await expect(panel.getByText(/Scoring candidates/i)).toBeVisible({ timeout: 10_000 });
    await expect(panel.getByTestId("auto-compare-computed-through")).toContainText("Computed through");

    const cancelJob = panel.getByRole("button", { name: "Cancel job" });
    await cancelJob.dispatchEvent("click");

    await expect(panel.getByText(/Analysis job canceled/i)).toBeVisible({ timeout: 10_000 });
    await expect(panel.getByTestId("auto-compare-computed-through")).toContainText("Computed through");
  });
});
