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

function baseUrl(): string {
  return (
    process.env.FARM_PLAYWRIGHT_BASE_URL ||
    process.env.FARM_SCREENSHOT_BASE_URL ||
    "http://127.0.0.1:8000"
  ).replace(/\/$/, "");
}

async function waitForSensorByName({
  token,
  sensorName,
  timeoutMs = 12_000,
}: {
  token: string;
  sensorName: string;
  timeoutMs?: number;
}): Promise<{ sensor_id: string } | null> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const res = await fetch(`${baseUrl()}/api/sensors`, {
      headers: { Authorization: `Bearer ${token}` },
    });
    if (res.ok) {
      const sensors = (await res.json()) as Array<{ sensor_id: string; name: string }>;
      const match = sensors.find((sensor) => sensor.name === sensorName);
      if (match) return match;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return null;
}

test.describe("derived sensors (Tier A)", () => {
  test("creates a derived sensor via UI (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date()
      .toISOString()
      .replace(/[:.]/g, "")
      .replace("T", "_")
      .replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_derived_sensors_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    const derivedName = `Derived smoke ${runStamp}`;

    await page.setViewportSize({ width: 1280, height: 820 });
    await page.addInitScript(
      ({ token }) => {
        window.sessionStorage.setItem("farmdashboard.auth.token", token);
      },
      { token },
    );

    await page.goto("/sensors");
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();

    const nodesPanel = page.locator("section", {
      has: page.getByRole("heading", { name: "Nodes", exact: true }),
    });
    const nodeDetails = nodesPanel.locator("details");

    await expect(nodeDetails.first()).toBeVisible();
    const firstDetails = nodeDetails.first();
    const isOpen = await firstDetails.evaluate((node) => (node as HTMLDetailsElement).open);
    expect(isOpen).toBe(false);

    await page.screenshot({
      path: path.join(screenshotsDir, "01_sensors_nodes_collapsed.png"),
      fullPage: true,
    });

    await firstDetails.locator("summary").click();
    await expect(firstDetails.getByText("Sensors", { exact: true })).toBeVisible();
    await firstDetails.getByRole("button", { name: "Add sensor", exact: false }).click();

    await expect(page.getByRole("heading", { name: /Add sensor/i })).toBeVisible();
    await page.getByRole("button", { name: "Derived", exact: true }).click();

    await page.getByPlaceholder("e.g. Delta pressure").fill(derivedName);
    await page.getByPlaceholder("e.g. pressure").fill("derived");
    await page.getByPlaceholder("e.g. kPa").fill("unit");

    await page.getByLabel("Search sensors").fill("temperature");
    const sensorSelect = page.getByLabel("Pick a sensor");
    await sensorSelect.selectOption({ index: 1 });
    await page.getByRole("button", { name: "Add input", exact: true }).click();

    await page.getByPlaceholder("e.g. clamp(avg(a, b), 0, 100)").fill("a");
    await page.getByRole("button", { name: "Create derived sensor", exact: true }).click();

    await expect(page.getByText("Created derived sensor.", { exact: true })).toBeVisible();

    await page.screenshot({
      path: path.join(screenshotsDir, "02_derived_sensor_builder_created.png"),
      fullPage: true,
    });

    const created = await waitForSensorByName({ token, sensorName: derivedName });
    if (created) {
      await fetch(`${baseUrl()}/api/sensors/${encodeURIComponent(created.sensor_id)}?keep_data=true`, {
        method: "DELETE",
        headers: { Authorization: `Bearer ${token}` },
      });
    }
  });
});
