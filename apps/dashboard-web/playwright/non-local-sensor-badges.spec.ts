import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

type DemoSensor = { sensor_id: string; node_id: string; name: string; config: Record<string, unknown> };
type DemoNode = { id: string; name: string };

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

test.describe("non-local sensor badges", () => {
  test("forecast/API sensors show a non-local badge across key surfaces", async ({ page }) => {
    const baseURL = baseUrlFromEnv();
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_non_local_sensor_badges_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    const sensors = await apiJson<DemoSensor[]>(baseURL, token, "/api/sensors", { method: "GET" });
    const nodes = await apiJson<DemoNode[]>(baseURL, token, "/api/nodes", { method: "GET" });

    const nonLocalSensor =
      sensors.find((s) => configString(s.config, "source") === "forecast_points" && configString(s.config, "provider") === "open_meteo") ||
      sensors.find((s) => configString(s.config, "source") === "forecast_points" && configString(s.config, "provider") === "forecast_solar") ||
      sensors.find((s) => configString(s.config, "source") === "forecast_points") ||
      null;

    expect(nonLocalSensor).not.toBeNull();
    if (!nonLocalSensor) return;

    const nodeName = nodes.find((n) => n.id === nonLocalSensor.node_id)?.name ?? nonLocalSensor.node_id;

    await page.setViewportSize({ width: 1280, height: 780 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    // Sensors & Outputs: drawer + table should show a non-local badge.
    await page.goto(`/sensors?node=${encodeURIComponent(nonLocalSensor.node_id)}&sensor=${encodeURIComponent(nonLocalSensor.sensor_id)}`);
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();
    const drawer = page
      .locator("aside")
      .filter({ has: page.getByRole("button", { name: "Close", exact: true }) })
      .first();
    await expect(drawer.getByTestId("sensor-origin-badge")).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "01_sensors_drawer_badge.png"), fullPage: true });

    // Close the drawer so we can interact with the underlying node tables.
    await page.goto(`/sensors?node=${encodeURIComponent(nonLocalSensor.node_id)}`);
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();

    const nodesPanel = page
      .locator("section")
      .filter({ has: page.getByRole("heading", { name: "Nodes", exact: true }) })
      .first();
    const nodeDetails = nodesPanel.locator("details").first();
    await nodeDetails.locator("summary").click();

    const sensorRow = nodeDetails.getByRole("table").locator("tr", { hasText: nonLocalSensor.sensor_id }).first();
    await expect(sensorRow).toBeVisible();
    await expect(sensorRow.getByTestId("sensor-origin-badge").first()).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "02_sensors_table_badge.png"), fullPage: true });

    // Node detail: sensor list should show the non-local badge.
    await page.goto(`/nodes/detail?id=${encodeURIComponent(nonLocalSensor.node_id)}`);
    await expect(page.getByRole("heading", { name: nodeName, exact: true })).toBeVisible();
    const nodeSensorRow = page.getByRole("table").locator("tr", { hasText: nonLocalSensor.name }).first();
    await expect(nodeSensorRow).toBeVisible();
    await expect(nodeSensorRow.getByTestId("sensor-origin-badge").first()).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "03_node_detail_sensor_badge.png"), fullPage: true });

    // Map: device list should show the badge next to the sensor name.
    await page.goto("/map");
    await expect(page.getByRole("heading", { name: /Map/i })).toBeVisible();
    const deviceSearch = page.getByPlaceholder("Search nodes/sensors…");
    await deviceSearch.fill(nonLocalSensor.sensor_id);
    await page.getByRole("button", { name: nodeName }).first().click();
    await expect(page.getByText(nonLocalSensor.sensor_id)).toBeVisible();
    const mapSensorRow = page
      .locator("div")
      .filter({ has: page.getByText(nonLocalSensor.sensor_id) })
      .filter({ has: page.getByTestId("sensor-origin-badge") })
      .first();
    await expect(mapSensorRow.getByTestId("sensor-origin-badge")).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "04_map_sensor_badge.png"), fullPage: true });
  });
});
