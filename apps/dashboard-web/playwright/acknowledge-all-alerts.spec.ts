import { expect, test } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

type AlarmEvent = { id: string; status?: string | null; message?: string | null };

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

function isAckable(event: AlarmEvent): boolean {
  const status = (event.status ?? "").toLowerCase();
  return status !== "acknowledged" && status !== "ok";
}

test.describe("acknowledge all alerts", () => {
  test("Sensors and Nodes surfaces include acknowledge-all and it works for firing events", async ({ page }) => {
    const baseURL = baseUrlFromEnv();
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_ack_all_alerts_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    const before = await apiJson<AlarmEvent[]>(
      baseURL,
      token,
      "/api/alarms/history?limit=50",
      { method: "GET" },
    );
    const ackable = before.filter(isAckable);
    const sample = ackable[0] ?? null;

    await page.setViewportSize({ width: 1280, height: 780 });
    await page.addInitScript(({ token }) => {
      window.sessionStorage.setItem("farmdashboard.auth.token", token);
    }, { token });

    // Sensors: Alarm Events panel includes the button.
    await page.goto("/sensors");
    await expect(page.getByRole("heading", { name: "Sensors & Outputs", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Alarm Events", exact: true })).toBeVisible();
    const ackAllButton = page.getByRole("button", { name: "Acknowledge all alerts", exact: true });
    await expect(ackAllButton).toBeVisible();

    if (sample) {
      await ackAllButton.click();
      const after = await apiJson<AlarmEvent[]>(
        baseURL,
        token,
        "/api/alarms/history?limit=50",
        { method: "GET" },
      );
      const updated = after.find((e) => e.id === sample.id) ?? null;
      expect(updated).not.toBeNull();
      if (updated) {
        expect((updated.status ?? "").toLowerCase()).toBe("acknowledged");
      }
    }
    await page.screenshot({ path: path.join(screenshotsDir, "01_sensors_alarm_events.png"), fullPage: true });

    // Nodes: Alarm Events panel is also present.
    await page.goto("/nodes");
    await expect(page.getByRole("heading", { name: "Nodes", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Alarm Events", exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "Acknowledge all alerts", exact: true })).toBeVisible();
    await page.screenshot({ path: path.join(screenshotsDir, "02_nodes_alarm_events.png"), fullPage: true });
  });
});

