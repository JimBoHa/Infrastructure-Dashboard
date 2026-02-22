import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

test.describe("Map placement mode", () => {
  test.beforeEach(async ({ page }) => {
    await installStubApi(page);
  });

  test("places a node by clicking the map", async ({ page }) => {
    const nowIso = new Date("2026-01-12T00:00:00.000Z").toISOString();
    let nextId = 1000;

    type MapFeature = {
      id: number;
      node_id: string | null;
      sensor_id: string | null;
      geometry: unknown;
      properties: Record<string, unknown>;
      created_at: string;
      updated_at: string;
    };

    const features: MapFeature[] = [];

    await page.route("**/api/map/features**", async (route) => {
      const request = route.request();
      const url = new URL(request.url());
      if (url.pathname === "/api/map/features") {
        if (request.method() === "GET") {
          return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(features) });
        }
        if (request.method() === "POST") {
          const body = request.postDataJSON() as unknown;
          const payload =
            body && typeof body === "object" ? (body as Record<string, unknown>) : ({} as Record<string, unknown>);
          const propertiesRaw = payload.properties;

          const created: MapFeature = {
            id: nextId++,
            node_id: typeof payload.node_id === "string" ? payload.node_id : null,
            sensor_id: typeof payload.sensor_id === "string" ? payload.sensor_id : null,
            geometry: payload.geometry,
            properties:
              propertiesRaw && typeof propertiesRaw === "object"
                ? (propertiesRaw as Record<string, unknown>)
                : {},
            created_at: nowIso,
            updated_at: nowIso,
          };
          features.push(created);
          return route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(created) });
        }
      }

      const match = url.pathname.match(/^\/api\/map\/features\/(\d+)$/);
      if (match) {
        const id = Number(match[1]);
        if (request.method() === "DELETE") {
          const idx = features.findIndex((f) => f.id === id);
          if (idx >= 0) features.splice(idx, 1);
          return route.fulfill({ status: 200, contentType: "application/json", body: "null" });
        }
      }

      return route.fallback();
    });

    await page.goto("/map", { waitUntil: "domcontentloaded" });
    await expect(page.getByRole("heading", { name: "Map" })).toBeVisible();

    const place = page.getByRole("button", { name: "Place", exact: true }).first();
    await place.scrollIntoViewIfNeeded();
    await place.click();
    await expect(page.getByText("Placement mode", { exact: true })).toBeVisible();

    const map = page.locator("#map-canvas");
    await map.click({ position: { x: 160, y: 160 } });

    await expect(page.getByText("Placement mode", { exact: true })).toHaveCount(0);
    await expect(page.getByText(/Placed · /)).toBeVisible();
  });
});
