import { expect, test } from "@playwright/test";
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

test.describe("overview mermaid diagram", () => {
  test("renders site map diagram (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_overview_mermaid_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 780 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    await page.goto("/overview");
    await expect(page.getByRole("heading", { name: "Overview", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Where things live", exact: true })).toBeVisible();

    // Ensure Mermaid rendered an SVG.
    await expect(page.locator(".farm-mermaid svg")).toHaveCount(1);

    // Regression: Mermaid edge paths must not render with a filled wedge (fill should be none).
    const edgeFills = await page.$$eval(".farm-mermaid svg path.flowchart-link", (paths) =>
      paths.map((path) => window.getComputedStyle(path).fill),
    );
    expect(edgeFills.length).toBeGreaterThan(0);
    for (const fill of edgeFills) {
      expect(fill === "none" || fill === "rgba(0, 0, 0, 0)" || fill === "transparent").toBeTruthy();
    }

    await page.screenshot({ path: path.join(screenshotsDir, "01_overview_mermaid.png"), fullPage: true });
  });
});
