import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

type DemoSensor = { sensor_id: string; node_id: string; config: Record<string, unknown> };
type DemoNode = { id: string; name: string; config?: Record<string, unknown> | null };

function requireEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`Missing required env var: ${name}`);
  return value;
}

function baseUrlFromEnv(): string {
  return (process.env.FARM_PLAYWRIGHT_BASE_URL || "http://127.0.0.1:8000").replace(/\/$/, "");
}

function tierAVersionLabel(): string {
  const value = (process.env.FARM_TIER_A_VERSION || "").trim();
  return value || "unknown";
}

async function apiJson<T>(
  baseURL: string,
  token: string,
  pathName: string,
  init: RequestInit,
): Promise<T> {
  const url = `${baseURL}${pathName}`;
  const res = await fetch(url, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
      ...(init.headers ?? {}),
    },
  });
  if (!res.ok) {
    const text = await res.text().catch(() => "");
    throw new Error(`API ${init.method || "GET"} ${pathName} failed (${res.status}): ${text}`);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

function configString(config: Record<string, unknown>, key: string): string | null {
  const value = config[key];
  return typeof value === "string" ? value : null;
}

test.describe("hide live weather per node", () => {
  test("toggle hides Open-Meteo weather sensors everywhere", async ({ page }) => {
    const baseURL = baseUrlFromEnv();
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_hide_live_weather_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    // Default `/api/sensors` hides hidden sensors; Tier A rigs may have `hide_live_weather` enabled already.
    // Include hidden sensors so we can deterministically find the Open-Meteo sensors to toggle.
    const sensors = await apiJson<DemoSensor[]>(baseURL, token, "/api/sensors?include_hidden=true", { method: "GET" });
    const nodes = await apiJson<DemoNode[]>(baseURL, token, "/api/nodes", { method: "GET" });

    const openMeteoWeatherSensor =
      sensors.find(
        (s) =>
          configString(s.config, "source") === "forecast_points" &&
          configString(s.config, "provider") === "open_meteo" &&
          configString(s.config, "kind") === "weather",
      ) ?? null;

    expect(openMeteoWeatherSensor).not.toBeNull();
    if (!openMeteoWeatherSensor) return;

    const node = nodes.find((n) => n.id === openMeteoWeatherSensor.node_id) ?? null;
    expect(node).not.toBeNull();
    if (!node) return;

    const nodeName = node.name ?? node.id;
    const nodeOpenMeteoSensorIds = sensors
      .filter(
        (s) =>
          s.node_id === node.id &&
          configString(s.config, "source") === "forecast_points" &&
          configString(s.config, "provider") === "open_meteo" &&
          configString(s.config, "kind") === "weather",
      )
      .map((s) => s.sensor_id);

    expect(nodeOpenMeteoSensorIds.length).toBeGreaterThan(0);

    const resetConfig = { ...((node.config ?? {}) as Record<string, unknown>), hide_live_weather: false };
    await apiJson(baseURL, token, `/api/nodes/${encodeURIComponent(node.id)}`, {
      method: "PUT",
      body: JSON.stringify({ config: resetConfig }),
    });

    await page.setViewportSize({ width: 1280, height: 780 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    await page.goto(`/nodes/detail?id=${encodeURIComponent(node.id)}`);
    await expect(page.getByRole("heading", { name: nodeName, exact: true })).toBeVisible();
    await expect(
      page.getByText("Current conditions at the node's mapped location", { exact: false }),
    ).toBeVisible();

    const firstSensorId = nodeOpenMeteoSensorIds[0] ?? openMeteoWeatherSensor.sensor_id;
    const nodeSensorRow = page.getByRole("table").locator("tr", { hasText: firstSensorId }).first();
    await expect(nodeSensorRow).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "01_before_toggle.png"), fullPage: true });

    await page.getByLabel("Hide public provider data (Open-Meteo)").check();
    await expect(
      page.getByText("Current conditions at the node's mapped location", { exact: false }),
    ).toHaveCount(0);
    await expect(page.getByRole("table").locator("tr", { hasText: firstSensorId })).toHaveCount(0);
    await page.screenshot({ path: path.join(screenshotsDir, "02_after_toggle_node_detail.png"), fullPage: true });

    await page.goto(`/sensors?node=${encodeURIComponent(node.id)}`);
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();
    await expect(page.getByRole("table").locator("tr", { hasText: firstSensorId })).toHaveCount(0);
    await page.screenshot({ path: path.join(screenshotsDir, "03_sensors_table_hidden.png"), fullPage: true });

    await page.goto("/map");
    await expect(page.getByRole("heading", { name: /Map/i })).toBeVisible();
    const deviceSearch = page.getByPlaceholder("Search nodes/sensors…");
    await deviceSearch.fill(firstSensorId);
    await expect(page.getByText(firstSensorId)).toHaveCount(0);
    await page.screenshot({ path: path.join(screenshotsDir, "04_map_hidden.png"), fullPage: true });

    await page.goto(`/nodes/detail?id=${encodeURIComponent(node.id)}`);
    await expect(page.getByRole("heading", { name: nodeName, exact: true })).toBeVisible();
    await page.getByLabel("Hide public provider data (Open-Meteo)").uncheck();
    await expect(
      page.getByText("Current conditions at the node's mapped location", { exact: false }),
    ).toBeVisible();
    await expect(page.getByRole("table").locator("tr", { hasText: firstSensorId }).first()).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "05_toggle_off_restored.png"), fullPage: true });
  });
});
