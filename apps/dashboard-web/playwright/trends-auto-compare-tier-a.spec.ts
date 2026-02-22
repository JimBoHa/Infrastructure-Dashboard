import { expect, test, type Locator, type Page } from "@playwright/test";
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

async function fetchLakeComputedThroughTs(page: Page, token: string): Promise<string> {
  const headers = {
    Authorization: `Bearer ${token}`,
    "Content-Type": "application/json",
  };

  const create = await page.request.post("/api/analysis/jobs", {
    headers,
    data: { job_type: "lake_inspect_v1", params: {}, dedupe: false },
  });
  if (!create.ok()) {
    throw new Error(`Failed to create lake_inspect_v1 job: ${create.status()} ${create.statusText()}`);
  }
  const created = (await create.json()) as { job?: { id?: string } };
  const jobId = created.job?.id;
  if (!jobId) throw new Error("lake_inspect_v1 create response missing job.id");

  let status = "pending";
  for (let i = 0; i < 100; i += 1) {
    const poll = await page.request.get(`/api/analysis/jobs/${jobId}`, { headers });
    if (!poll.ok()) {
      throw new Error(`Failed to poll lake_inspect_v1 job: ${poll.status()} ${poll.statusText()}`);
    }
    const payload = (await poll.json()) as { job?: { status?: string } };
    status = payload.job?.status ?? "unknown";
    if (status === "completed") break;
    if (status === "failed" || status === "canceled") {
      throw new Error(`lake_inspect_v1 job did not complete (status=${status})`);
    }
    await page.waitForTimeout(250);
  }
  if (status !== "completed") {
    throw new Error(`Timed out waiting for lake_inspect_v1 job completion (status=${status})`);
  }

  const resultResp = await page.request.get(`/api/analysis/jobs/${jobId}/result`, { headers });
  if (!resultResp.ok()) {
    throw new Error(`Failed to fetch lake_inspect_v1 result: ${resultResp.status()} ${resultResp.statusText()}`);
  }
  const resultJson = (await resultResp.json()) as {
    result?: {
      inspection?: {
        replication?: { computed_through_ts?: string | null };
        datasets?: { [key: string]: { computed_through_ts?: string | null } };
      };
    };
  };
  const computed =
    resultJson.result?.inspection?.replication?.computed_through_ts ??
    resultJson.result?.inspection?.datasets?.["metrics/v1"]?.computed_through_ts ??
    null;
  if (!computed) throw new Error("lake_inspect_v1 result missing computed_through_ts");
  return computed;
}

async function pickHardwareSensorId(page: Page, token: string): Promise<string> {
  const headers = { Authorization: `Bearer ${token}` };
  const resp = await page.request.get("/api/sensors", { headers });
  if (!resp.ok()) {
    throw new Error(`Failed to fetch /api/sensors: ${resp.status()} ${resp.statusText()}`);
  }
  const sensors = (await resp.json()) as Array<{
    sensor_id?: string;
    latest_ts?: string | null;
    unit?: string | null;
    type?: string | null;
    deleted_at?: string | null;
    config?: { provider?: string | null } | null;
  }>;

  for (const sensor of sensors) {
    if (!sensor?.sensor_id) continue;
    if (sensor.deleted_at) continue;
    if (!sensor.latest_ts) continue;
    if (!sensor.unit || !sensor.type) continue;
    if (sensor.config?.provider) continue;
    return sensor.sensor_id;
  }

  throw new Error("No suitable focus sensor found (expected at least one non-provider sensor with data).");
}

async function ensureDetailsOpen(details: Locator) {
  const open = await details.evaluate((node) => (node as HTMLDetailsElement).open).catch(() => false);
  if (!open) {
    await details.locator("summary").first().click({ force: true });
  }
}

function sensorPickerCard(page: Page): Locator {
  const heading = page.getByRole("heading", { name: "Sensor picker", exact: true });
  return heading.locator("xpath=ancestor::details[1]");
}

async function selectAnySensorWithChartData(page: Page) {
  const picker = sensorPickerCard(page);
  await ensureDetailsOpen(picker);

  const nodeSelect = picker.locator("select").first();
  const options = nodeSelect.locator("option");
  const optionCount = await options.count().catch(() => 0);
  if (optionCount > 1) {
    await nodeSelect.selectOption({ index: 1 });
  }

  const checkboxes = picker.locator('details label input[type="checkbox"]');
  const count = await checkboxes.count();
  if (count === 0) throw new Error("No sensors found in Sensor picker.");

  const target = Math.min(count, 25);
  const chartCanvas = page.getByTestId("trend-chart-container").locator("canvas").first();

  for (let i = 0; i < target; i += 1) {
    const checkbox = checkboxes.nth(i);
    await checkbox.check({ force: true });
    try {
      await expect(chartCanvas).toBeVisible({ timeout: 8000 });
      await picker
        .evaluate((node) => {
          (node as HTMLDetailsElement).open = false;
        })
        .catch(() => {});
      return;
    } catch {
      await checkbox.uncheck({ force: true });
    }
  }

  throw new Error("Unable to find a sensor with chart data.");
}

async function waitForSuggestionsOrEmptyState(page: import("@playwright/test").Page) {
  const suggestions = page.getByTestId("auto-compare-suggestions");
  const empty = page.getByText("No candidates returned", { exact: false });
  const noCandidates = page.getByText("No candidate sensors match", { exact: false });
  const loading = page.getByText("Scoring candidates", { exact: false });

  await Promise.race([
    suggestions.waitFor({ state: "visible", timeout: 20_000 }),
    empty.waitFor({ state: "visible", timeout: 20_000 }),
    noCandidates.waitFor({ state: "visible", timeout: 20_000 }),
    loading.waitFor({ state: "visible", timeout: 20_000 }),
  ]);

  return suggestions;
}

test.describe("trends auto-compare (Tier A)", () => {
  test("renders related sensor suggestions + preview (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_trends_auto_compare_${runStamp}`,
    );
    fs.mkdirSync(screenshotsDir, { recursive: true });

    await page.setViewportSize({ width: 1280, height: 900 });
    await page.addInitScript(
      ({ token }) => {
        window.sessionStorage.setItem("farmdashboard.auth.token", token);
      },
      { token },
    );

    await page.goto("/trends");
    await expect(page.getByRole("heading", { name: "Trends", exact: true })).toBeVisible();
    await page.getByPlaceholder("Search sensors…").waitFor({ timeout: 10_000 });

    await selectAnySensorWithChartData(page);

    const panel = page.getByTestId("trends-auto-compare");
    await expect(panel).toBeVisible();
    await expect(panel.getByRole("heading", { name: "Related sensors", exact: true })).toBeVisible();

    const run = panel.getByRole("button", { name: "Run analysis" });
    await run.dispatchEvent("click");
    await waitForSuggestionsOrEmptyState(page);

    await expect(panel.getByTestId("auto-compare-computed-through")).toBeVisible({ timeout: 15_000 });

    // Show the per-panel analysis key (Tier A evidence). Open after running suggestions so the expanded key
    // doesn't shift the layout while interacting with other controls.
    const keySummary = panel.locator("summary").filter({ hasText: "View details" });
    if (await keySummary.isVisible().catch(() => false)) {
      // Ensure the summary isn't stuck under the fixed header (Playwright can scroll it into a non-clickable position).
      await keySummary.evaluate((node) => node.scrollIntoView({ block: "center" }));
      await keySummary.click({ force: true });
      await expect(panel.getByText("What this does", { exact: true })).toBeVisible();
    }

    await page.screenshot({
      path: path.join(screenshotsDir, "01_trends_auto_compare_key.png"),
      fullPage: true,
    });

    if (await keySummary.isVisible().catch(() => false)) {
      await keySummary.click({ force: true });
    }

    const suggestions = page.getByTestId("auto-compare-suggestions");
    const firstPreview = suggestions.locator('button[title="Preview relationship"]').first();
    if (await firstPreview.isVisible().catch(() => false)) {
      await firstPreview.dispatchEvent("click");
      await expect(panel.getByText("Preview bucket size")).toBeVisible();
      await Promise.race([
        panel.getByTestId("auto-compare-episodes").waitFor({ state: "visible", timeout: 12_000 }),
        panel.getByText("No episodes were generated", { exact: false }).waitFor({ state: "visible", timeout: 12_000 }),
      ]).catch(() => {});
      await page.screenshot({
        path: path.join(screenshotsDir, "02_trends_auto_compare_preview.png"),
        fullPage: true,
      });
    }
  });

  test("accepts long-range high-res request (cancelable)", async ({ page }) => {
    test.setTimeout(120_000);
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");

    const headers = {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    };

    const computedThroughIso = await fetchLakeComputedThroughTs(page, token);
    const focusSensorId = await pickHardwareSensorId(page, token);

    // High-resolution request: keep the window large enough to stress test bucketing/clamps,
    // but cancel quickly so Tier-A doesn't risk a long-running controller load.
    const end = new Date(computedThroughIso);
    const start = new Date(end.getTime() - 24 * 60 * 60 * 1000);

    const create = await page.request.post("/api/analysis/jobs", {
      headers,
      data: {
        job_type: "related_sensors_v1",
        params: {
          focus_sensor_id: focusSensorId,
          start: start.toISOString(),
          end: computedThroughIso,
          interval_seconds: 1,
          candidate_limit: 50,
          min_pool: 50,
          lag_max_seconds: 60 * 60,
          filters: {
            same_node_only: false,
            same_unit_only: true,
            same_type_only: false,
            exclude_sensor_ids: [],
          },
        },
        dedupe: false,
      },
    });
    if (!create.ok()) {
      throw new Error(`Failed to create related_sensors_v1 job: ${create.status()} ${create.statusText()}`);
    }
    const created = (await create.json()) as { job?: { id?: string } };
    const jobId = created.job?.id;
    if (!jobId) throw new Error("related_sensors_v1 create response missing job.id");

    const cancel = await page.request.post(`/api/analysis/jobs/${encodeURIComponent(jobId)}/cancel`, {
      headers,
      data: {},
    });
    if (!cancel.ok()) {
      throw new Error(`Failed to cancel related_sensors_v1 job: ${cancel.status()} ${cancel.statusText()}`);
    }

    // Poll briefly: accept either canceled or completed (but never failed).
    let status = "pending";
    for (let i = 0; i < 80; i += 1) {
      const poll = await page.request.get(`/api/analysis/jobs/${encodeURIComponent(jobId)}`, { headers });
      if (!poll.ok()) {
        throw new Error(`Failed to poll related_sensors_v1 job: ${poll.status()} ${poll.statusText()}`);
      }
      const payload = (await poll.json()) as { job?: { status?: string } };
      status = payload.job?.status ?? "unknown";
      if (status === "canceled" || status === "completed") break;
      if (status === "failed") {
        throw new Error("related_sensors_v1 job failed for a high-res request (expected cancel/complete).");
      }
      await page.waitForTimeout(250);
    }
  });
});
