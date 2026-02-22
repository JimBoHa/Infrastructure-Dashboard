import { expect, test, type Locator, type Page } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";

type SensorRecord = {
  sensor_id: string;
  node_id: string;
  config?: { source?: string; read_only?: boolean } | null;
};

type MetricPoint = { timestamp: string; value: number; samples: number };
type MetricSeries = { sensor_id: string; points: MetricPoint[] };
type MetricsResponse = { series: MetricSeries[] };

function requireEnv(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`Missing required env var: ${name}`);
  return value;
}

function tierAVersionLabel(): string {
  const value = (process.env.FARM_TIER_A_VERSION || "").trim();
  return value || "unknown";
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

function chartSettingsCard(page: Page): Locator {
  const heading = page.getByRole("heading", { name: "Chart settings", exact: true });
  return heading.locator("xpath=ancestor::details[1]");
}

async function setChartWindow(page: Page, rangeHours: number, intervalSeconds: number) {
  const card = chartSettingsCard(page);
  await card.waitFor({ timeout: 20_000 });
  await ensureDetailsOpen(card);

  const rangeSelect = card.getByRole("combobox", { name: /^range$/i }).first();
  if (await rangeSelect.isVisible().catch(() => false)) {
    await rangeSelect.selectOption(String(rangeHours)).catch(() => {});
  }

  const intervalSelect = card.getByRole("combobox", { name: /^interval$/i }).first();
  if (await intervalSelect.isVisible().catch(() => false)) {
    await intervalSelect.selectOption(String(intervalSeconds)).catch(() => {});
  }
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
  for (let i = 0; i < 80; i += 1) {
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

async function findSensorsWithDataSameNode(
  page: Page,
  start: string,
  end: string,
  intervalSeconds: number,
  targetCount: number,
) {
  const sensorsResp = await page.request.get("/api/sensors");
  if (!sensorsResp.ok()) throw new Error(`Failed to fetch /api/sensors: ${sensorsResp.status()}`);

  const sensors = (await sensorsResp.json()) as Array<Partial<SensorRecord>>;
  const normalized: SensorRecord[] = sensors
    .map((raw) => ({
      sensor_id: String(raw.sensor_id ?? ""),
      node_id: String(raw.node_id ?? ""),
      config: raw.config ?? null,
    }))
    .filter((s) => s.sensor_id && s.node_id)
    .filter((s) => {
      const source = String(s.config?.source ?? "").trim();
      const readOnly = Boolean(s.config?.read_only);
      if (readOnly) return false;
      if (source === "forecast_points") return false;
      return true;
    });

  const byNode = new Map<string, SensorRecord[]>();
  for (const sensor of normalized) {
    const list = byNode.get(sensor.node_id) ?? [];
    list.push(sensor);
    byNode.set(sensor.node_id, list);
  }

  for (const [, sensorsForNode] of byNode) {
    const candidates = sensorsForNode.slice(0, 18);
    if (candidates.length < targetCount) continue;

    const params = new URLSearchParams();
    candidates.forEach((s) => params.append("sensor_ids[]", s.sensor_id));
    params.set("start", start);
    params.set("end", end);
    params.set("interval", String(intervalSeconds));

    const metricsResp = await page.request.get(`/api/metrics/query?${params.toString()}`);
    if (!metricsResp.ok()) continue;

    const metrics = (await metricsResp.json()) as Partial<MetricsResponse>;
    const series = (metrics.series ?? []).filter(
      (s): s is MetricSeries => Boolean(s && s.sensor_id && Array.isArray(s.points)),
    );
    const idsWithPoints = series
      .filter((s) => (s.points?.length ?? 0) >= 8)
      .map((s) => s.sensor_id)
      .slice(0, targetCount);
    if (idsWithPoints.length >= targetCount) return idsWithPoints;
  }

  throw new Error("Unable to find enough sensors with chart data on a single node.");
}

async function checkSensorById(page: Page, sensorId: string) {
  const picker = sensorPickerCard(page);
  await ensureDetailsOpen(picker);

  const search = picker.getByPlaceholder("Search sensors…");
  await search.fill(sensorId);

  const row = picker.locator("label").filter({ hasText: sensorId }).first();
  await expect(row).toBeVisible();
  const checkbox = row.locator('input[type=\"checkbox\"]').first();
  await checkbox.evaluate((el) => el.scrollIntoView({ block: "center" }));
  await checkbox.check({ force: true });
}

test.describe("trends cooccurrence (Tier A)", () => {
  test("renders co-occurrence key + summary (Tier A screenshot)", async ({ page }) => {
    const token = requireEnv("FARM_PLAYWRIGHT_AUTH_TOKEN");
    const versionLabel = tierAVersionLabel();

    const runStamp = new Date().toISOString().replace(/[:.]/g, "").replace("T", "_").replace("Z", "Z");
    const screenshotsDir = path.join(
      path.resolve(process.cwd(), "..", "..", "manual_screenshots_web"),
      `tier_a_${versionLabel}_trends_cooccurrence_${runStamp}`,
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

    const computedThroughIso = await fetchLakeComputedThroughTs(page, token);
    const end = new Date(computedThroughIso);
    const start = new Date(end.getTime() - 72 * 60 * 60 * 1000);
    const intervalSeconds = 60;
    const sensorIds = await findSensorsWithDataSameNode(
      page,
      start.toISOString(),
      computedThroughIso,
      intervalSeconds,
      4,
    );

    await setChartWindow(page, 72, intervalSeconds);
    for (const id of sensorIds) {
      await checkSensorById(page, id);
    }

    const panel = page.getByTestId("trends-cooccurrence");
    await expect(panel.getByRole("heading", { name: "Co-occurring anomalies" })).toBeVisible();

    const run = panel.getByRole("button", { name: "Run analysis", exact: true });
    await run.click({ force: true });

    const summary = page.getByTestId("cooccurrence-job-summary");
    const error = page.getByText("Analysis job failed", { exact: false });
    await Promise.race([
      summary.waitFor({ state: "visible", timeout: 60_000 }),
      error.waitFor({ state: "visible", timeout: 60_000 }),
    ]);

    const keySummary = panel.locator("summary").filter({ hasText: "Key" }).first();
    if (await keySummary.isVisible().catch(() => false)) {
      await keySummary.evaluate((node) => node.scrollIntoView({ block: "center" }));
      await keySummary.click({ force: true });
    }
    await page.screenshot({ path: path.join(screenshotsDir, "01_trends_cooccurrence_key.png"), fullPage: true });
  });
});

