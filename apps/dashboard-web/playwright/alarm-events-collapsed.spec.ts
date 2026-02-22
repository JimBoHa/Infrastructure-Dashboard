import { expect, test, type Page } from "@playwright/test";

import { installStubApi } from "./stubApi";

const ISO_NOW = new Date("2026-01-09T00:00:00.000Z").toISOString();

const alarmHistory = [
  {
    alarm_id: "alarm-active",
    id: "event-active",
    created_at: ISO_NOW,
    status: "active",
    message: "Active alarm event",
    origin: "playwright",
  },
  {
    alarm_id: "alarm-ack",
    id: "event-ack",
    created_at: ISO_NOW,
    status: "acknowledged",
    message: "Acknowledged alarm event",
    origin: "playwright",
  },
  {
    alarm_id: "alarm-ok",
    id: "event-ok",
    created_at: ISO_NOW,
    status: "ok",
    message: "Cleared alarm event",
    origin: "playwright",
  },
] as const;

async function expectCollapsedBehavior(pagePath: string, page: Page) {
  await page.goto(pagePath, { waitUntil: "domcontentloaded" });
  await expect(page.getByRole("heading", { name: "Alarm Events", exact: true })).toBeVisible();

  const panel = page.locator("section", {
    has: page.getByRole("heading", { name: "Alarm Events", exact: true }),
  });

  await expect(panel.getByText("Active alarm event", { exact: true })).toBeVisible();
  await expect(panel.getByText("Acknowledged alarm event", { exact: true })).toBeHidden();
  await expect(panel.getByText("Cleared alarm event", { exact: true })).toBeHidden();

  const details = panel.locator("details").filter({ hasText: "Acknowledged & cleared" });
  await expect(details).toBeVisible();
  await details.evaluate((node) => {
    (node as HTMLDetailsElement).open = true;
  });

  await expect(panel.getByText("Acknowledged alarm event", { exact: true })).toBeVisible();
  await expect(panel.getByText("Cleared alarm event", { exact: true })).toBeVisible();
}

test.describe("alarm events collapse acknowledged", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page, {
      jsonByPath: {
        "/api/alarms/history": alarmHistory,
      },
    });
  });

  test("Sensors page collapses acknowledged/cleared events by default", async ({ page }) => {
    await expectCollapsedBehavior("/sensors", page);
  });

  test("Nodes page collapses acknowledged/cleared events by default", async ({ page }) => {
    await expectCollapsedBehavior("/nodes", page);
  });
});
