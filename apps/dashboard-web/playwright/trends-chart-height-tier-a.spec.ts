import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

function requireEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`Missing required env var: ${name}`);
  return value;
}

function tierAVersionLabel(): string {
  const value = (process.env.FARM_TIER_A_VERSION || "").trim();
  return value || "unknown";
}

async function ensureDetailsOpen(details: Locator) {
  const open = await details.evaluate((node) => (node as HTMLDetailsElement).open).catch(() => false);
  if (!open) {
    await details.locator("summary").first().click({ force: true });
  }
}

function sensorPickerCard(page: Page): Locator {
  const heading = page.getByRole("heading", { name: "Sensor picker", exact: true });
  return heading.locator("xpath=ancestor::details[1]");
}

async function selectAnySensorWithChartData(page: Page) {
  const picker = sensorPickerCard(page);
  await ensureDetailsOpen(picker);

  const nodeSelect = picker.locator("select").first();
  const options = nodeSelect.locator("option");
  const optionCount = await options.count().catch(() => 0);
  if (optionCount > 1) {
    await nodeSelect.selectOption({ index: 1 });
  }

  const checkboxes = picker.locator('details label input[type="checkbox"]');
  const count = await checkboxes.count();
  if (count === 0) throw new Error("No sensors found in Sensor picker.");

  const target = Math.min(count, 20);
  const chartCanvas = page.getByTestId("trend-chart-container").locator("canvas").first();

  for (let i = 0; i < target; i += 1) {
    const checkbox = checkboxes.nth(i);
    await checkbox.check({ force: true });
    try {
      await expect(chartCanvas).toBeVisible({ timeout: 8000 });
      return;
    } catch {
      await checkbox.uncheck({ force: true });
    }
  }

  throw new Error("Unable to find a sensor with chart data.");
}

test.describe("trends chart height (Tier A)", () => {
  test("allows resizing the chart taller (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_trends_chart_height_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 900 });
    await page.addInitScript(
      ({ token }) => {
        window.sessionStorage.setItem("farmdashboard.auth.token", token);
      },
      { token },
    );

    await page.goto("/trends");
    await expect(page.getByRole("heading", { name: "Trends", exact: true })).toBeVisible();
    await page.getByPlaceholder("Search sensors…").waitFor({ timeout: 10_000 });

    await selectAnySensorWithChartData(page);

    const slider = page.getByRole("slider", { name: "Chart height" });
    await expect(slider).toBeVisible();

    const chartContainer = page.getByTestId("trend-chart-container");
    await expect(chartContainer).toBeVisible();
    const before = await chartContainer.evaluate((node) => node.getBoundingClientRect().height);

    const initialValue = await slider.inputValue();
    const targetHeight = 720;
    const min = Number((await slider.getAttribute("min")) ?? "0");
    const max = Number((await slider.getAttribute("max")) ?? "100");
    const ratio = Math.max(0, Math.min(1, (targetHeight - min) / Math.max(1, max - min)));
    const box = await slider.boundingBox();
    if (!box) throw new Error("Unable to locate slider bounding box");
    await page.mouse.click(box.x + box.width * ratio, box.y + box.height / 2);

    const nextValue = await slider.inputValue();
    expect(nextValue).not.toBe(initialValue);
    await expect(slider).toHaveValue(nextValue);
    await expect
      .poll(async () => page.evaluate(() => window.localStorage.getItem("fd_trends_chart_height_px")))
      .toBe(nextValue);
    await page.waitForTimeout(500);
    const after = await chartContainer.evaluate((node) => node.getBoundingClientRect().height);
    expect(after).toBeGreaterThan(before + 150);

    await page.screenshot({
      path: path.join(screenshotsDir, "01_trends_chart_height_resized.png"),
      fullPage: true,
    });
  });
});
