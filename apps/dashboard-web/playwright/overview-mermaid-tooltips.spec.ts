import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

async function ensureDetailsOpen(details: import("@playwright/test").Locator) {
  await expect(details).toBeVisible({ timeout: 15_000 });
  await details.scrollIntoViewIfNeeded();
  const open = await details.evaluate((node) => (node as HTMLDetailsElement).open).catch(() => false);
  if (!open) {
    await details.locator("summary").first().click({ force: true });
  }
}

test.describe("Overview Mermaid tooltips", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("shows tooltips for multiple nodes", async ({ page }) => {
    await page.goto("/overview", { waitUntil: "domcontentloaded" });

    const collapsible = page.locator("details", {
      has: page.getByRole("heading", { name: "Where things live", exact: true }),
    });
    await ensureDetailsOpen(collapsible);

    const diagram = page.locator(".farm-mermaid svg");
    await expect(diagram).toBeVisible();
    await expect
      .poll(async () => diagram.locator("g.node").count())
      .toBeGreaterThan(0);
    await page.waitForSelector('g.node[id*="NODES"][data-farm-tooltip]', { timeout: 10_000 });
    await page.waitForSelector('g.node[id*="MQTT"][data-farm-tooltip]', { timeout: 10_000 });

    await page.evaluate(() => {
      const normalize = (v: string) => v.replace(/\s+/g, " ").trim();
      const svg = document.querySelector(".farm-mermaid svg");
      if (!svg) return;
      const findNode = (label: string) =>
        Array.from(svg.querySelectorAll("g.node")).find(
          (g) => normalize(g.textContent || "") === label,
        ) || null;
      const nodes = findNode("Nodes");
      nodes?.dispatchEvent(new MouseEvent("mouseover", { bubbles: true, cancelable: true }));
    });
    await expect(page.getByRole("tooltip")).toContainText(
      "Scan, adopt, and inspect node health",
    );

    const foundMqtt = await page.evaluate(() => {
      const svg = document.querySelector(".farm-mermaid svg");
      if (!svg) return false;
      const mqtt = svg.querySelector('g.node[id*="MQTT"]') as SVGGElement | null;
      if (!mqtt) return false;
      mqtt.dispatchEvent(new MouseEvent("mouseover", { bubbles: true, cancelable: true }));
      return true;
    });
    expect(foundMqtt).toBeTruthy();
    await expect(page.getByRole("tooltip")).toContainText("Telemetry transport");

    await page.screenshot({
      path: "/tmp/playwright-overview-mermaid-tooltips.png",
      fullPage: true,
    });
  });
});
