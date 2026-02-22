import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

type CreatedNode = { id: string; name: string };
type CreatedSensor = { sensor_id: string; node_id: string; name: string };
type AuthMe = { role: string };
type LoginResponse = { token: string };

function requireEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) {
    throw new Error(`Missing required env var: ${name}`);
  }
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
  if (res.status === 204) {
    return undefined as T;
  }
  return (await res.json()) as T;
}

test.describe("soft delete (nodes + sensors)", () => {
  test("soft-deleted nodes/sensors disappear everywhere and names are reusable", async ({ page }) => {
    const baseURL = baseUrlFromEnv();
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_soft_delete_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    const nodeName = `Soft delete demo node ${runStamp}`;
    const sensorName = `Soft delete demo sensor ${runStamp}`;
    const cascadeSensorName = `Cascade sensor ${runStamp}`;

    const me = await apiJson<AuthMe>(baseURL, token, "/api/auth/me", { method: "GET" });
    let uiToken = token;
    if (me.role !== "admin") {
      const adminEmail = `codex-admin-${runStamp}@farmdashboard.local`;
      const adminPassword = `pw-${runStamp}`;
      await apiJson(baseURL, token, "/api/users", {
        method: "POST",
        body: JSON.stringify({
          name: `Codex Admin ${runStamp}`,
          email: adminEmail,
          role: "admin",
          capabilities: [],
          password: adminPassword,
        }),
      });
      const login = await apiJson<LoginResponse>(baseURL, token, "/api/auth/login", {
        method: "POST",
        body: JSON.stringify({ email: adminEmail, password: adminPassword }),
      });
      uiToken = login.token;
    }

    const node = await apiJson<CreatedNode>(baseURL, token, "/api/nodes", {
      method: "POST",
      body: JSON.stringify({ name: nodeName }),
    });

    const sensor = await apiJson<CreatedSensor>(baseURL, token, "/api/sensors", {
      method: "POST",
      body: JSON.stringify({
        node_id: node.id,
        name: sensorName,
        type: "temperature",
        unit: "degC",
        interval_seconds: 30,
        rolling_avg_seconds: 0,
      }),
    });

    const cascadeSensor = await apiJson<CreatedSensor>(baseURL, token, "/api/sensors", {
      method: "POST",
      body: JSON.stringify({
        node_id: node.id,
        name: cascadeSensorName,
        type: "humidity",
        unit: "%",
        interval_seconds: 30,
        rolling_avg_seconds: 0,
      }),
    });

    await apiJson(baseURL, token, "/api/map/features", {
      method: "POST",
      body: JSON.stringify({
        node_id: node.id,
        sensor_id: null,
        geometry: { type: "Point", coordinates: [-122.0308, 36.9741] },
        properties: { name: nodeName, kind: "node" },
      }),
    });
    await apiJson(baseURL, token, "/api/map/features", {
      method: "POST",
      body: JSON.stringify({
        node_id: null,
        sensor_id: sensor.sensor_id,
        geometry: { type: "Point", coordinates: [-122.0307, 36.97405] },
        properties: { name: sensorName, kind: "sensor" },
      }),
    });

    await page.setViewportSize({ width: 1280, height: 780 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token: uiToken });

    await page.goto("/nodes");
    await expect(page.getByRole("heading", { name: "Nodes", exact: true })).toBeVisible();
    await expect(page.getByText(nodeName)).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "01_nodes_before.png"), fullPage: true });

    await page.goto(`/map`);
    await expect(page.getByRole("heading", { name: /Map/i })).toBeVisible();
    await page.waitForTimeout(1000);
    await page.screenshot({ path: path.join(screenshotsDir, "02_map_before.png"), fullPage: true });

    await page.goto(`/sensors?node=${encodeURIComponent(node.id)}&sensor=${encodeURIComponent(sensor.sensor_id)}`);
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: sensorName, exact: true })).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "03_sensor_drawer_before_delete.png"), fullPage: true });

    const deleteSensorButton = page.getByRole("button", { name: "Delete sensor" }).first();
    await deleteSensorButton.scrollIntoViewIfNeeded();
    await deleteSensorButton.click();
    const confirmDeleteSensorButton = page.getByRole("button", { name: "Delete sensor" }).last();
    await confirmDeleteSensorButton.click();

    await expect(page.getByRole("heading", { name: sensorName, exact: true })).not.toBeVisible();
    await expect(page.getByRole("table").getByText(sensorName)).not.toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "04_sensor_deleted.png"), fullPage: true });

    // Verify the deleted sensor name is reusable by creating a new sensor with the same name.
    const replacementSensor = await apiJson<CreatedSensor>(baseURL, token, "/api/sensors", {
      method: "POST",
      body: JSON.stringify({
        node_id: node.id,
        name: sensorName,
        type: "temperature",
        unit: "degC",
        interval_seconds: 30,
        rolling_avg_seconds: 0,
      }),
    });

    await page.goto(`/sensors?node=${encodeURIComponent(node.id)}&sensor=${encodeURIComponent(replacementSensor.sensor_id)}`);
    await expect(page.getByRole("heading", { name: sensorName, exact: true })).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "05_sensor_name_reused.png"), fullPage: true });

    await page.goto(`/nodes/detail?id=${encodeURIComponent(node.id)}`);
    await expect(page.getByText(nodeName, { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "Delete node" }).first().scrollIntoViewIfNeeded();
    await page.getByRole("button", { name: "Delete node" }).first().click();
    await page.getByRole("button", { name: "Delete node" }).last().click();
    await expect(page).toHaveURL(/\/nodes/);
    await expect(page.getByRole("heading", { name: "Nodes", exact: true })).toBeVisible();
    await expect(page.getByRole("main").getByText(nodeName, { exact: true })).toHaveCount(0);
    await page.screenshot({ path: path.join(screenshotsDir, "06_node_deleted.png"), fullPage: true });

    // Name reuse for nodes: create a new node with the same name; it should appear normally.
    const nodeReuse = await apiJson<CreatedNode>(baseURL, token, "/api/nodes", {
      method: "POST",
      body: JSON.stringify({ name: nodeName }),
    });

    await page.goto("/nodes");
    await expect(page.getByText(nodeName)).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "07_node_name_reused.png"), fullPage: true });

    // Cleanup: soft delete the reused node via API (keep the UI validation focused on the primary flow above).
    await apiJson(baseURL, token, `/api/nodes/${encodeURIComponent(nodeReuse.id)}`, { method: "DELETE" });

    // Sanity: map features API should not return features linked to the deleted node/sensors.
    const features = await apiJson<unknown[]>(baseURL, token, "/api/map/features", { method: "GET" });
    const serialized = JSON.stringify(features);
    expect(serialized).not.toContain(node.id);
    expect(serialized).not.toContain(sensor.sensor_id);
    expect(serialized).not.toContain(cascadeSensor.sensor_id);
  });
});
