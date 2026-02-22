import { expect, test } from "@playwright/test";

import { expectNoHorizontalOverflow, expectNoVerticalShiftDuring } from "./helpers/layout";
import { installStubApi } from "./stubApi";

test.describe("Overview telemetry tapestry layout", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
    await page.setViewportSize({ width: 1280, height: 820 });
  });

  test("does not overflow or shift when hovering buckets", async ({ page }) => {
    await page.goto("/overview", { waitUntil: "domcontentloaded" });

    await expect(page.getByRole("heading", { name: "Overview", exact: true })).toBeVisible();

    const tapestryCard = page.getByTestId("telemetry-tapestry-card");
    await expect(tapestryCard).toBeVisible();

    await expectNoHorizontalOverflow(tapestryCard, { label: "tapestry card" });

    const pageHasOverflow = await page.evaluate(() => {
      const root = document.documentElement;
      return root.scrollWidth > root.clientWidth + 1;
    });
    expect(pageHasOverflow).toBeFalsy();

    const rows = page.getByTestId("telemetry-tapestry-rows");
    await expect(rows).toBeVisible();

    const details = page.getByTestId("telemetry-tapestry-details");
    await expect(details).toContainText("Hover cells for details");

    const firstHeatmapRow = rows.getByRole("img").first();
    const firstCell = firstHeatmapRow.locator("div").first();

    await expectNoVerticalShiftDuring(() => firstCell.hover(), rows, { label: "tapestry rows (hover)" });
    await expect(details).not.toContainText("Hover cells for details");

    const header = page.getByText("Telemetry tapestry", { exact: true });
    await expectNoVerticalShiftDuring(() => header.hover(), rows, { label: "tapestry rows (unhover)" });
    await expect(details).toContainText("Hover cells for details");

    await expectNoHorizontalOverflow(tapestryCard, { label: "tapestry card (post-hover)" });

    await page.screenshot({ path: "/tmp/playwright-overview-tapestry-layout.png", fullPage: true });
  });
});

