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

  const target = Math.min(count, 25);
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

test.describe("trends keys (Tier A)", () => {
  test("shows Sensor picker + Trend chart keys (Tier A screenshots)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_trends_keys_${runStamp}`,
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

    const picker = sensorPickerCard(page);
    const pickerKey = picker.locator("summary").filter({ hasText: "View details" }).first();
    if (await pickerKey.isVisible().catch(() => false)) {
      await pickerKey.evaluate((node) => node.scrollIntoView({ block: "center" }));
      await pickerKey.click({ force: true });
      await expect(picker.getByText("What this is", { exact: true })).toBeVisible();
    }

    await page.screenshot({
      path: path.join(screenshotsDir, "01_trends_sensor_picker_key.png"),
      fullPage: true,
    });

    if (await pickerKey.isVisible().catch(() => false)) {
      await pickerKey.click({ force: true });
    }

    const chartContainer = page.getByTestId("trend-chart-container");
    await expect(chartContainer).toBeVisible();
    const chartCard = chartContainer.locator('xpath=ancestor::div[contains(@class,"rounded-xl")][1]');
    const chartKey = chartCard.locator("summary").filter({ hasText: "View details" }).first();
    if (await chartKey.isVisible().catch(() => false)) {
      await chartKey.evaluate((node) => node.scrollIntoView({ block: "center" }));
      await chartKey.click({ force: true });
      await expect(chartCard.getByText("Chart controls", { exact: true })).toBeVisible();
    }

    await page.screenshot({
      path: path.join(screenshotsDir, "02_trends_trend_chart_key.png"),
      fullPage: true,
    });
  });
});
