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

test.describe("Trends matrix profile explorer", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("renders and supports tabs", async ({ page }) => {
    await page.goto("/trends", { waitUntil: "domcontentloaded" });

    await page.getByPlaceholder("Search sensors…").waitFor({ timeout: 10_000 });
    await selectSensor(page, "Playwright Voltage");
    await expect(page.getByText("1/20 selected")).toBeVisible({ timeout: 10_000 });

    const panel = page.getByTestId("trends-matrix-profile");
    await ensureDetailsOpen(panel);
    const runButton = panel.getByRole("button", { name: "Run analysis", exact: true });
    await expect(runButton).toBeEnabled({ timeout: 15_000 });
    const [jobRequest] = await Promise.all([
      page.waitForRequest((request) => request.url().includes("/api/analysis/jobs") && request.method() === "POST"),
      runButton.click(),
    ]);
    const jobPayload = jobRequest.postDataJSON() as { params?: Record<string, unknown> };
    expect(jobPayload?.params?.top_k).toBeTruthy();
    expect(jobPayload?.params?.max_windows).toBeTruthy();
    expect(jobPayload?.params?.exclusion_zone).toBeTruthy();
    await expect(runButton).toBeDisabled();

    await expect(panel.getByRole("button", { name: "Cancel", exact: true })).toBeVisible({ timeout: 10_000 });
    await expect(panel.getByText(/\(\d+\/5\)/)).toBeVisible({ timeout: 10_000 });

    await expect(panel.getByText("Top anomalies", { exact: true })).toBeVisible({ timeout: 15_000 });
    await expect(panel.getByText("Computed through:", { exact: false })).toBeVisible();
    await expect(panel.getByText(/Input downsampled/i)).toBeVisible();
    await expect(panel.getByRole("button", { name: "Motifs", exact: true })).toBeVisible();

    const anomalyCard = panel.locator('button:has-text("Anomaly #1")').first();
    await expect(anomalyCard).toBeVisible();
    await expect(anomalyCard.getByText("Window:", { exact: false })).toBeVisible();

    await page.getByRole("button", { name: "Motifs", exact: true }).click();
    await expect(page.getByText("Top motifs", { exact: true })).toBeVisible();

    const motifCard = panel.locator('button:has-text("Motif #1")').first();
    await expect(motifCard).toBeVisible();
    await expect(motifCard.getByText("A:", { exact: false })).toBeVisible();
    await expect(motifCard.getByText("B:", { exact: false })).toBeVisible();

    await page.getByRole("button", { name: "Self-similarity", exact: true }).click();
    await expect(panel.getByText("Self-similarity heatmap", { exact: true }).first()).toBeVisible();

    const canvas = panel.locator('canvas[width="320"][height="320"]');
    await expect(canvas).toHaveCount(1);
    await canvas.hover({ position: { x: 160, y: 160 } });
    await canvas.click({ position: { x: 160, y: 160 } });

    await expect(page.getByText("Top motifs", { exact: true })).toBeVisible();
  });
});
