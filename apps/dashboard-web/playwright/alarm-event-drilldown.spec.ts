import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

const ISO_NOW = new Date("2026-01-09T00:00:00.000Z").toISOString();

const SAMPLE_NODE_ID = "playwright-node-1";
const SAMPLE_SENSOR_ID = "playwright-sensor-1";

const alarmId = "42";
const eventId = "9001";

const alarmsResponse = [
  {
    id: Number(alarmId),
    name: "Playwright threshold alarm",
    rule: {
      type: "threshold",
      severity: "warning",
      operator: ">",
      threshold: 30,
    },
    sensor_id: SAMPLE_SENSOR_ID,
    node_id: SAMPLE_NODE_ID,
    status: "active",
    origin: "threshold",
    anomaly_score: null,
    last_fired: ISO_NOW,
    message: "Temperature high",
  },
] as const;

const alarmHistory = [
  {
    alarm_id: alarmId,
    id: eventId,
    sensor_id: SAMPLE_SENSOR_ID,
    node_id: SAMPLE_NODE_ID,
    created_at: ISO_NOW,
    status: "active",
    message: "Temperature high",
    origin: "threshold",
    anomaly_score: null,
  },
] as const;

test.describe("Alarm event drilldown", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page, {
      jsonByPath: {
        "/api/alarms": alarmsResponse,
        "/api/alarms/history": alarmHistory,
      },
    });
  });

  test("opens detail drawer and renders a context chart", async ({ page }) => {
    await page.goto("/sensors", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { name: "Alarm Events", exact: true })).toBeVisible();

    await page.getByRole("button", { name: `View details for alarm event ${eventId}`, exact: true }).click();

    const drawer = page.getByTestId("alarm-event-detail-drawer");
    await expect(drawer).toBeVisible();
    await expect(drawer).toContainText("Temperature high");

    const chart = drawer.getByTestId("alarm-event-context-chart");
    await expect(chart).toBeVisible();
    await expect(chart.locator("canvas").first()).toBeVisible();

    await drawer.getByRole("button", { name: "Close", exact: true }).click();
    await expect(drawer).toBeHidden();
  });
});

