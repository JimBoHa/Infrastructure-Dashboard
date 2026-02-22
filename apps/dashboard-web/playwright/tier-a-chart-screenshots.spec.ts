import { test, expect } from "@playwright/test";
import { installStubApi } from "./stubApi";

const SCREENSHOT_DIR =
  "/Users/FarmDashboard/farm_dashboard/manual_screenshots_web/tier_a_chart_governance_20260129_065132Z";

/**
 * Walk the DOM tree and return a list of elements whose bounding box
 * overflows their parent's bounding box (ignoring overflow:hidden clips).
 * Useful for catching layout regressions.
 */
async function detectOverflows(page: import("@playwright/test").Page): Promise<string[]> {
  return page.evaluate(() => {
    const violations: string[] = [];
    const walk = (el: Element) => {
      const parent = el.parentElement;
      if (!parent) return;
      const cr = el.getBoundingClientRect();
      const pr = parent.getBoundingClientRect();
      // Skip invisible / zero-size elements
      if (cr.width === 0 || cr.height === 0) return;
      if (pr.width === 0 || pr.height === 0) return;
      // Skip if parent clips overflow
      const style = getComputedStyle(parent);
      if (style.overflow === "hidden" || style.overflowX === "hidden" || style.overflowY === "hidden") return;
      if (style.overflow === "scroll" || style.overflowX === "scroll" || style.overflowY === "scroll") return;
      if (style.overflow === "auto" || style.overflowX === "auto" || style.overflowY === "auto") return;

      const tolerance = 2; // px
      if (
        cr.left < pr.left - tolerance ||
        cr.right > pr.right + tolerance ||
        cr.top < pr.top - tolerance ||
        cr.bottom > pr.bottom + tolerance
      ) {
        const tag = el.tagName.toLowerCase();
        const id = el.id ? `#${el.id}` : "";
        const cls = el.className && typeof el.className === "string"
          ? `.${el.className.split(" ").slice(0, 2).join(".")}`
          : "";
        const desc = `${tag}${id}${cls}`;
        const parentTag = parent.tagName.toLowerCase();
        const parentId = parent.id ? `#${parent.id}` : "";
        violations.push(
          `${desc} overflows ${parentTag}${parentId} by [L:${Math.round(pr.left - cr.left)},R:${Math.round(cr.right - pr.right)},T:${Math.round(pr.top - cr.top)},B:${Math.round(cr.bottom - pr.bottom)}]px`,
        );
      }
    };
    document.querySelectorAll("body *").forEach(walk);
    return violations;
  });
}

/**
 * Select a sensor in the Trends page sensor picker.
 * Node groups are collapsed by default — expand via the node filter dropdown first.
 */
async function selectSensorOnTrends(
  page: import("@playwright/test").Page,
  sensorLabel: string,
) {
  // Filter to specific node to auto-expand the <details>
  const nodeDropdown = page.locator("select").filter({ hasText: "All nodes" });
  if (await nodeDropdown.isVisible({ timeout: 2000 })) {
    await nodeDropdown.selectOption({ index: 1 }); // select first real node
    await page.waitForTimeout(500);
  }

  // Now check the sensor
  const sensorCheckbox = page
    .locator("label")
    .filter({ hasText: sensorLabel })
    .locator('input[type="checkbox"]');
  if (await sensorCheckbox.isVisible({ timeout: 3000 })) {
    await sensorCheckbox.check();
    await page.waitForTimeout(2000);
  }
}

test.describe("Tier A Chart Governance Screenshots", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("1 - Overview sparklines", async ({ page }) => {
    await page.goto("/overview");
    await page.waitForTimeout(3000);
    await expect(
      page.getByRole("heading", { name: "Local sensors", exact: true }),
    ).toBeVisible({ timeout: 10000 });
    await page.waitForTimeout(5000);
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/01_overview_sparklines.png`,
      fullPage: true,
    });
  });

  test("2 - Trends main chart", async ({ page }) => {
    await page.goto("/trends");
    await page.waitForTimeout(2000);
    await selectSensorOnTrends(page, "Temperature");
    await page.waitForTimeout(3000);
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/02_trends_main_chart.png`,
      fullPage: true,
    });

    // Element-level: sensor picker card
    const sensorPicker = page.locator("details").filter({ hasText: "Sensor picker" }).first();
    if (await sensorPicker.isVisible({ timeout: 3000 })) {
      await sensorPicker.screenshot({
        path: `${SCREENSHOT_DIR}/02a_trends_sensor_picker_element.png`,
      });
    }

    // Element-level: main chart container
    const mainChart = page.locator('[data-testid="trend-chart-container"]').first();
    if (await mainChart.isVisible({ timeout: 3000 })) {
      await mainChart.screenshot({
        path: `${SCREENSHOT_DIR}/02b_trends_main_chart_element.png`,
      });
    }
  });

  test("3 - Trends matrix profile", async ({ page }) => {
    await page.goto("/trends");
    await page.waitForTimeout(2000);
    await selectSensorOnTrends(page, "Voltage");
    await page.waitForTimeout(2000);
    // Expand Matrix Profile collapsible
    const mpButton = page
      .locator("summary, button, [role=button]")
      .filter({ hasText: /^Matrix Profile$/ })
      .first();
    if (await mpButton.isVisible({ timeout: 3000 })) {
      await mpButton.click();
      await page.waitForTimeout(8000); // Wait for analysis job to poll through and complete
    }
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/03_trends_matrix_profile.png`,
      fullPage: true,
    });
  });

  test("4 - Trends related sensors", async ({ page }) => {
    await page.goto("/trends");
    await page.waitForTimeout(2000);
    await selectSensorOnTrends(page, "Voltage");
    await page.waitForTimeout(2000);
    const rsButton = page
      .locator("summary, button, [role=button]")
      .filter({ hasText: /^Related Sensors$/ })
      .first();
    if (await rsButton.isVisible({ timeout: 3000 })) {
      await rsButton.click();
      await page.waitForTimeout(8000);
    }
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/04_trends_related_sensors.png`,
      fullPage: true,
    });
  });

  test("5 - Trends voltage quality", async ({ page }) => {
    await page.goto("/trends");
    await page.waitForTimeout(2000);
    // Select voltage sensor
    await selectSensorOnTrends(page, "Voltage");
    await page.waitForTimeout(2000);
    const vqButton = page
      .locator("summary, button, [role=button]")
      .filter({ hasText: /Voltage Quality/ })
      .first();
    if (await vqButton.isVisible({ timeout: 3000 })) {
      await vqButton.click();
      await page.waitForTimeout(5000);
    }
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/05_trends_voltage_quality.png`,
      fullPage: true,
    });
  });

  test("6 - Analytics Power page", async ({ page }) => {
    await page.goto("/power");
    await page.waitForTimeout(6000);
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/06_analytics_power.png`,
      fullPage: true,
    });
  });

  test("7 - Analytics Overview page", async ({ page }) => {
    await page.goto("/analytics");
    await page.waitForTimeout(6000);
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/07_analytics_overview.png`,
      fullPage: true,
    });
  });

  test("8 - Sensors page with detail drawer", async ({ page }) => {
    // Use tall viewport so the drawer has enough room to show the chart fully
    await page.setViewportSize({ width: 1440, height: 1400 });
    await page.goto("/sensors");
    await page.waitForTimeout(2000);
    // Expand first node to see sensor rows
    const nodeExpander = page
      .locator("summary")
      .filter({ hasText: "Playwright Node" })
      .first();
    if (await nodeExpander.isVisible({ timeout: 3000 })) {
      await nodeExpander.click();
      await page.waitForTimeout(1000);
    }
    // Click first sensor row to open drawer
    const tableRows = page.locator("tbody tr");
    if ((await tableRows.count()) > 0) {
      await tableRows.first().click();
      await page.waitForTimeout(4000);
    }
    // Scroll the drawer so the TrendChart navigator is fully in view
    const chartContainer = page.locator('[data-testid="trend-chart-container"]');
    if (await chartContainer.isVisible({ timeout: 5000 })) {
      // Find the TrendChart's outer card wrapper and scroll it into the drawer viewport
      await chartContainer.evaluate((el) => {
        // Walk up to the CollapsibleCard <details> and the drawer scroll container
        const card = el.closest("details");
        const scroller = el.closest('[class*="overflow-y"]');
        if (card && scroller) {
          // Scroll so the card's bottom edge + generous padding is visible
          const cardBottom = card.getBoundingClientRect().bottom;
          const scrollerBottom = scroller.getBoundingClientRect().bottom;
          const gap = cardBottom - scrollerBottom + 80;
          if (gap > 0) scroller.scrollBy(0, gap);
        }
      });
      await page.waitForTimeout(3000);
    }
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/08_sensor_detail_drawer.png`,
      fullPage: true,
    });

    // Element-level: drawer trend chart
    if (await chartContainer.isVisible({ timeout: 2000 })) {
      await chartContainer.screenshot({
        path: `${SCREENSHOT_DIR}/08a_drawer_trend_chart_element.png`,
      });
    }
  });

  test("9 - Node Detail page", async ({ page }) => {
    await page.goto("/nodes/detail?id=playwright-node-1");
    await page.waitForTimeout(6000);
    await page.screenshot({
      path: `${SCREENSHOT_DIR}/09_node_detail.png`,
      fullPage: true,
    });
  });

  test("10 - Console errors check", async ({ page }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") errors.push(msg.text());
    });

    for (const route of [
      "/overview",
      "/trends",
      "/power",
      "/analytics",
      "/sensors",
      "/nodes/detail",
    ]) {
      await page.goto(route);
      await page.waitForTimeout(3000);
    }

    const chartErrors = errors.filter(
      (e) =>
        !e.includes("React DevTools") &&
        !e.includes("HMR") &&
        !e.includes("favicon") &&
        !e.includes("Download the React") &&
        !e.includes("Failed to load resource") &&
        !e.includes("net::ERR"),
    );

    if (chartErrors.length > 0) {
      console.log("Chart-related errors found:", chartErrors);
    }
    expect(chartErrors.length).toBe(0);
  });

  test("11 - Overflow detection audit", async ({ page }) => {
    const allViolations: { route: string; violations: string[] }[] = [];

    for (const route of ["/overview", "/trends", "/power", "/analytics", "/sensors"]) {
      await page.goto(route);
      await page.waitForTimeout(4000);

      // On /trends, select a sensor to populate the chart area
      if (route === "/trends") {
        await selectSensorOnTrends(page, "Temperature");
        await page.waitForTimeout(3000);
      }

      const violations = await detectOverflows(page);
      if (violations.length > 0) {
        allViolations.push({ route, violations });
        console.log(`Overflow violations on ${route}:`, violations);
      }
    }

    // Log all violations for debugging but expect zero
    if (allViolations.length > 0) {
      console.log("All overflow violations:", JSON.stringify(allViolations, null, 2));
    }
    expect(allViolations).toHaveLength(0);
  });
});
