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

function chartSettingsCard(page: Page): Locator {
  const heading = page.getByRole("heading", { name: "Chart settings", exact: true });
  return heading.locator("xpath=ancestor::details[1]");
}

async function configureChartWindow(page: Page) {
  const card = chartSettingsCard(page);
  await card.waitFor({ timeout: 20_000 });
  await ensureDetailsOpen(card);

  const rangeSelect = card.getByRole("combobox", { name: /^range$/i }).first();
  if (await rangeSelect.isVisible().catch(() => false)) {
    await rangeSelect.selectOption("72").catch(() => {});
  }

  const intervalSelect = card.getByRole("combobox", { name: /^interval$/i }).first();
  if (await intervalSelect.isVisible().catch(() => false)) {
    await intervalSelect.selectOption("60").catch(() => {});
  }
}

async function selectAnySensorWithMatrixProfile(page: Page) {
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

  const target = Math.min(count, 12);
  const explorerHeading = page.getByRole("heading", { name: "Matrix Profile explorer", exact: true });

  for (let i = 0; i < target; i += 1) {
    const checkbox = checkboxes.nth(i);
    await checkbox.check({ force: true });
    try {
      await explorerHeading.waitFor({ timeout: 10_000 });
      const panel = explorerHeading.locator("xpath=ancestor::details[1]");
      await ensureDetailsOpen(panel);
      const runButton = panel.getByRole("button", { name: "Run analysis", exact: true });
      if (await runButton.isVisible().catch(() => false)) {
        await runButton.click({ force: true });
      }
      await expect(panel.getByText("Top anomalies", { exact: true })).toBeVisible({ timeout: 25_000 });
      return;
    } catch {
      await checkbox.uncheck({ force: true });
    }
  }

  throw new Error("Unable to find a sensor with data for Matrix Profile explorer.");
}

test.describe("trends matrix profile (Tier A)", () => {
  test("renders matrix profile explorer (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_trends_matrix_profile_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 900 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    await page.goto("/trends");
    await expect(page.getByRole("heading", { name: "Trends", exact: true })).toBeVisible();
    await page.getByPlaceholder("Search sensors…").waitFor({ timeout: 10_000 });

    await configureChartWindow(page);
    await selectAnySensorWithMatrixProfile(page);
    const explorerHeading = page.getByRole("heading", { name: "Matrix Profile explorer" });
    await explorerHeading.scrollIntoViewIfNeeded();
    await expect(explorerHeading).toBeVisible();

    const panel = explorerHeading.locator("xpath=ancestor::details[1]");
    await ensureDetailsOpen(panel);
    const keySummary = panel.locator("summary").filter({ hasText: "View details" }).first();
    if (await keySummary.isVisible().catch(() => false)) {
      await keySummary.evaluate((node) => node.scrollIntoView({ block: "center" }));
      await keySummary.click({ force: true });
      await expect(panel.getByText("Key terms", { exact: true })).toBeVisible();
    }

    await page.screenshot({
      path: path.join(screenshotsDir, "01_trends_matrix_profile_key.png"),
      fullPage: true,
    });

    if (await keySummary.isVisible().catch(() => false)) {
      await keySummary.click({ force: true });
    }

    const similarityTab = panel.getByRole("button", { name: "Self-similarity", exact: true });
    await similarityTab.evaluate((node) => node.scrollIntoView({ block: "center" }));
    await similarityTab.click({ force: true });

    await expect(similarityTab).toHaveClass(/border-indigo/, { timeout: 10_000 });
    await page.waitForTimeout(500);

    if (await keySummary.isVisible().catch(() => false)) {
      await keySummary.evaluate((node) => node.scrollIntoView({ block: "center" }));
      await keySummary.click({ force: true });
      await expect(panel.getByText("Key terms", { exact: true })).toBeVisible();
    }

    await page.screenshot({
      path: path.join(screenshotsDir, "02_trends_matrix_profile_self_similarity_key.png"),
      fullPage: true,
    });
  });
});
