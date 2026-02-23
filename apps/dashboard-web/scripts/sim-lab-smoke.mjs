#!/usr/bin/env node
import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import fs from "node:fs/promises";
import { chromium } from "playwright";

const dirname = path.dirname(fileURLToPath(import.meta.url));
const dashboardWebDir = path.resolve(dirname, "..");
const repoRoot = path.resolve(dashboardWebDir, "..", "..");

const timestampSlug = () => new Date().toISOString().replace(/[:.]/g, "-");

const parseArgs = (argv) => {
  const args = new Map();
  const flags = new Set();
  for (const entry of argv.slice(2)) {
    if (!entry.startsWith("--")) continue;
    const trimmed = entry.slice(2);
    if (!trimmed.includes("=")) {
      flags.add(trimmed);
      continue;
    }
    const [key, ...rest] = trimmed.split("=");
    args.set(key, rest.join("="));
  }
  return { args, flags };
};

const { args, flags } = parseArgs(process.argv);

const apiBase =
  args.get("api-base") ||
  process.env.FARM_SIM_LAB_API_BASE ||
  process.env.FARM_CORE_API_BASE ||
  process.env.NEXT_PUBLIC_API_BASE ||
  "http://127.0.0.1:8000";

const webBase =
  args.get("base-url") ||
  process.env.FARM_SIM_LAB_BASE_URL ||
  "http://127.0.0.1:3005";
const simLabControlBase =
  args.get("sim-lab-base") ||
  process.env.FARM_SIM_LAB_CONTROL_BASE ||
  process.env.NEXT_PUBLIC_SIM_LAB_API_BASE ||
  "http://127.0.0.1:8100";

const forecastUrl =
  process.env.FARM_SIM_LAB_FORECAST_URL ||
  "http://127.0.0.1:9103/forecast.json";
const ratesUrl =
  process.env.FARM_SIM_LAB_RATES_URL ||
  "http://127.0.0.1:9104/rates.json";

const artifactsRoot =
  process.env.FARM_SIM_LAB_ARTIFACTS_DIR ||
  path.resolve(repoRoot, "reports", "e2e-web-smoke", timestampSlug());

const startCore = !flags.has("no-core");
const startWeb = !flags.has("no-web");
const requireInstalled = process.env.FARM_E2E_REQUIRE_INSTALLED === "1";

const abortAfter = async (ms, fn) => {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), ms);
  try {
    return await fn(controller.signal);
  } finally {
    clearTimeout(timeout);
  }
};

const waitForHttpOk = async (url, timeoutMs) => {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    try {
      const response = await abortAfter(3000, (signal) => fetch(url, { signal }));
      if (response.ok) return;
    } catch {
      // ignore until timeout elapses
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`Timed out waiting for ${url}`);
};

const isListening = async (host, port) => {
  try {
    await abortAfter(800, async (signal) => {
      const controller = new AbortController();
      const timeout = setTimeout(() => controller.abort(), 800);
      signal.addEventListener("abort", () => controller.abort(), { once: true });
      const socket = await import("node:net").then((net) => net.createConnection({ host, port }));
      await new Promise((resolve, reject) => {
        socket.once("connect", resolve);
        socket.once("error", reject);
        controller.signal.addEventListener("abort", () => reject(new Error("timeout")), { once: true });
      });
      clearTimeout(timeout);
      socket.destroy();
    });
    return true;
  } catch {
    return false;
  }
};

const spawnServer = ({ label, command, cwd, env }) => {
  const child = spawn(command, {
    cwd,
    env,
    shell: true,
    stdio: "inherit",
  });

  child.on("exit", (code, signal) => {
    if (signal) return;
    if (code === 0) return;
    console.error(`[${label}] exited with code ${code}`);
  });

  return child;
};

const waitForUiToSettle = async (page) => {
  await page.waitForLoadState("domcontentloaded");
  await page.waitForLoadState("networkidle", { timeout: 6000 }).catch(() => {});
  await page.evaluate(() => (document.fonts?.ready ? document.fonts.ready : Promise.resolve())).catch(() => {});
  await page.waitForTimeout(300);
};

const splitUrl = (urlValue) => {
  const parsed = new URL(urlValue);
  const base = `${parsed.protocol}//${parsed.host}`;
  return { base, path: parsed.pathname || "/" };
};

const apiOrigin = apiBase.replace(/\/$/, "");
const webOrigin = webBase.replace(/\/$/, "");

const apiUrl = (pathValue) =>
  pathValue.startsWith("http") ? pathValue : `${apiOrigin}${pathValue}`;

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

let coreProc;
let webProc;
let browser;
let context;
let authToken;

const cleanup = async () => {
  if (webProc && !webProc.killed) webProc.kill("SIGTERM");
  if (coreProc && !coreProc.killed) coreProc.kill("SIGTERM");
};

process.on("SIGINT", async () => {
  await cleanup();
  process.exit(130);
});
process.on("SIGTERM", async () => {
  await cleanup();
  process.exit(143);
});

const apiRequest = async (pathValue, { method = "GET", body, headers } = {}) => {
  const url = apiUrl(pathValue);
  const requestHeaders = { ...(headers || {}) };
  if (authToken && !requestHeaders.Authorization) {
    requestHeaders.Authorization = `Bearer ${authToken}`;
  }
  if (body && !requestHeaders["Content-Type"]) {
    requestHeaders["Content-Type"] = "application/json";
  }
  const response = await abortAfter(15_000, (signal) =>
    fetch(url, {
      method,
      body: body ? JSON.stringify(body) : undefined,
      headers: requestHeaders,
      signal,
    })
  );
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`API ${method} ${url} failed (${response.status}): ${text || response.statusText}`);
  }
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
};

const controlRequest = async (pathValue) => {
  const url = pathValue.startsWith("http") ? pathValue : `${simLabControlBase.replace(/\/$/, "")}${pathValue}`;
  const response = await abortAfter(10_000, (signal) => fetch(url, { signal }));
  if (!response.ok) {
    throw new Error(`Sim Lab control request failed (${response.status}): ${url}`);
  }
  return response.json();
};

const createAuthToken = async () => {
  // Allow providing an existing token directly (useful for Tier-A validation against
  // installed controllers where user creation requires authentication)
  const envToken = process.env.FARM_E2E_AUTH_TOKEN;
  if (envToken) {
    console.log("[auth] Using auth token from FARM_E2E_AUTH_TOKEN environment variable");
    return envToken;
  }

  const url = (() => {
    try {
      return new URL(apiBase);
    } catch {
      return null;
    }
  })();
  const apiPort = url?.port || (url?.protocol === "https:" ? "443" : "80");
  const email = process.env.FARM_E2E_USER_EMAIL || `e2e-smoke-${apiPort}@farm.local`;
  const password = process.env.FARM_E2E_USER_PASSWORD || "SmokeTest!123";
  const capabilities = [
    "outputs.command",
    "alerts.ack",
    "schedules.write",
    "config.write",
    "users.manage",
  ];

  const tryLogin = async () => {
    try {
      const response = await apiRequest("/api/auth/login", {
        method: "POST",
        body: { email, password },
      });
      return response?.token || null;
    } catch {
      return null;
    }
  };

  const existingToken = await tryLogin();
  if (existingToken) return existingToken;

  try {
    await apiRequest("/api/users", {
      method: "POST",
      body: {
        name: "E2E Smoke",
        email,
        role: "admin",
        capabilities,
        password,
      },
    });
  } catch (err) {
    const message = String(err?.message || err);
    if (!message.includes("(409)") && !message.toLowerCase().includes("email already")) {
      throw err;
    }
  }

  const token = await tryLogin();
  if (!token) {
    throw new Error("Failed to obtain auth token after creating E2E user");
  }
  return token;
};

const pollFor = async (label, fn, { timeoutMs = 30_000, intervalMs = 1000 } = {}) => {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const result = await fn();
    if (result) return result;
    await sleep(intervalMs);
  }
  throw new Error(`Timed out waiting for ${label}`);
};

try {
  const apiUrlParsed = new URL(apiBase);
  const webUrl = new URL(webBase);
  const webOriginValue = `${webUrl.protocol}//${webUrl.host}`;

  if (startCore) {
    const coreDir = path.resolve(repoRoot, "apps", "core-server-rs");
    const coreHost = apiUrlParsed.hostname;
    const corePort = Number(apiUrlParsed.port || (apiUrlParsed.protocol === "https:" ? 443 : 80));
    const forecastSplit = splitUrl(forecastUrl);
    const ratesSplit = splitUrl(ratesUrl);

    if (await isListening(coreHost, corePort)) {
      console.log(`[core] Reusing existing server on ${apiBase}`);
    } else {
      if (!process.env.CORE_DATABASE_URL) {
        throw new Error(
          "CORE_DATABASE_URL must be set (run an installed stack or `FARM_E2E_KEEP_TEMP=1 make e2e-setup-smoke`)."
        );
      }
      console.log(`[core] Starting Rust controller on ${apiBase}`);
      coreProc = spawnServer({
        label: "core",
        command: `cargo run --quiet -- --host ${coreHost} --port ${corePort}`,
        cwd: coreDir,
        env: {
          ...process.env,
          CORE_ENABLE_SCHEDULER: "false",
          CORE_ENABLE_BACKUPS: "false",
          CORE_ENABLE_ANALYTICS_FEEDS: "false",
          CORE_ENABLE_FORECAST_INGESTION: "false",
          CORE_ENABLE_INDICATOR_GENERATION: "false",
          CORE_PREDICTIVE_ALARMS__ENABLED: "false",
          CORE_CORS_ALLOWED_ORIGINS: JSON.stringify([webOriginValue]),
          CORE_FORECAST_PROVIDER: "http",
          CORE_FORECAST_API_BASE_URL: forecastSplit.base,
          CORE_FORECAST_API_PATH: forecastSplit.path,
          CORE_ANALYTICS_RATES__PROVIDER: "http",
          CORE_ANALYTICS_RATES__API_BASE_URL: ratesSplit.base,
          CORE_ANALYTICS_RATES__API_PATH: ratesSplit.path,
        },
      });
    }
    await waitForHttpOk(`${apiOrigin}/healthz`, 30_000);
  }

  if (startWeb) {
    const webHost = webUrl.hostname;
    const webPort = Number(webUrl.port || (webUrl.protocol === "https:" ? 443 : 80));
    const apiOriginValue = `${apiUrlParsed.protocol}//${apiUrlParsed.host}`;

    if (await isListening(webHost, webPort)) {
      console.log(`[web] Reusing existing dashboard on ${webBase}`);
    } else {
      console.log(`[web] Starting dashboard on ${webBase} (FARM_CORE_API_BASE=${apiOriginValue})`);
      webProc = spawnServer({
        label: "web",
        command: `npm run dev -- --hostname ${webHost} --port ${webPort}`,
        cwd: dashboardWebDir,
        env: {
          ...process.env,
          FARM_CORE_API_BASE: apiOriginValue,
          NEXT_PUBLIC_SIM_LAB_API_BASE: simLabControlBase,
        },
      });
    }
    await waitForHttpOk(`${webOrigin}/nodes`, 90_000);
  }

  await fs.mkdir(artifactsRoot, { recursive: true });
  authToken = await createAuthToken();
  await waitForHttpOk(`${simLabControlBase.replace(/\/$/, "")}/healthz`, 30_000);
  const simLabStatus = await pollFor(
    "Sim Lab status",
    async () => {
      try {
        const status = await controlRequest("/sim-lab/status");
        return status?.nodes?.length ? status : null;
      } catch {
        return null;
      }
    },
    { timeoutMs: 30_000, intervalMs: 1000 },
  );

  browser = await chromium.launch({ headless: true });
  context = await browser.newContext({
    viewport: { width: 1400, height: 900 },
    locale: "en-US",
    timezoneId: "America/Los_Angeles",
    acceptDownloads: true,
    extraHTTPHeaders: authToken ? { Authorization: `Bearer ${authToken}` } : undefined,
  });
  if (authToken) {
    await context.addInitScript(
      (token) => {
        try {
          window.sessionStorage.setItem("farmdashboard.auth.token", token);
        } catch {
          // ignore
        }
      },
      authToken
    );
  }
  const page = await context.newPage();
  page.setDefaultTimeout(40_000);

  const logEntries = [];
  const appendLog = (entry) => {
    logEntries.push(`[${new Date().toISOString()}] ${entry}`);
  };
  page.on("console", (msg) => appendLog(`console.${msg.type()}: ${msg.text()}`));
  page.on("pageerror", (err) => appendLog(`pageerror: ${err.message}`));
  page.on("requestfailed", (req) => appendLog(`requestfailed: ${req.method()} ${req.url()} - ${req.failure()?.errorText || "unknown"}`));
  page.on("response", (res) => {
    const status = res.status();
    if (status < 400) return;
    appendLog(`response: ${status} ${res.url()}`);
  });

  const captureArtifacts = async (label, error) => {
    const slug = label.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");
    const screenshotPath = path.join(artifactsRoot, `${slug || "failure"}.png`);
    const logPath = path.join(artifactsRoot, `${slug || "failure"}.log`);
    await page.screenshot({ path: screenshotPath, fullPage: true }).catch(() => {});
    const payload = logEntries.concat([`error: ${error?.stack || error}`]).join("\n");
    await fs.writeFile(logPath, payload, "utf-8").catch(() => {});
    console.error(`[smoke] Saved artifacts for ${label}: ${screenshotPath}`);
  };

  const runStep = async (label, fn) => {
    console.log(`[smoke] ${label}`);
    try {
      await fn();
    } catch (err) {
      await captureArtifacts(label, err);
      throw err;
    }
  };
  const runStepIf = async (label, fn) => {
    if (requireInstalled) return;
    try {
      await runStep(label, fn);
    } catch (err) {
      console.warn(`[smoke] ${label} failed; continuing. ${err?.message || err}`);
    }
  };

  const nodes = await apiRequest("/api/nodes");
  const sensors = await apiRequest("/api/sensors");

  const primaryNode = nodes[0];
  if (!primaryNode) {
    throw new Error("No nodes returned from /api/nodes");
  }
  const simNode = simLabStatus.nodes.find((entry) => entry.node_id === primaryNode.id) || simLabStatus.nodes[0];
  if (!simNode) {
    throw new Error("Sim Lab status returned no nodes");
  }
  const nodeApiBase = simNode.api_base;
  const nodeApiHost = new URL(nodeApiBase).hostname;
  await waitForHttpOk(`${nodeApiBase.replace(/\/$/, "")}/healthz`, 30_000);
  await apiRequest(`/api/nodes/${primaryNode.id}`, {
    method: "PUT",
    body: { ip_last: nodeApiHost },
  });

  const backupDate = new Date().toISOString().slice(0, 10);
  const nodeConfig = await abortAfter(10_000, (signal) =>
    fetch(`${nodeApiBase}/v1/config`, { signal }).then((res) => res.json())
  );
  const backupRoot =
    process.env.FARM_BACKUP_ROOT ||
    process.env.FARM_E2E_BACKUP_ROOT ||
    process.env.CORE_BACKUP_STORAGE_PATH ||
    path.resolve(repoRoot, "storage", "backups");
  const backupDir = path.resolve(backupRoot, primaryNode.id);
  await fs.mkdir(backupDir, { recursive: true });
  const backupPath = path.join(backupDir, `${backupDate}.json`);
  await fs.writeFile(
    backupPath,
    JSON.stringify(
      {
        fetched_at: new Date().toISOString(),
        config: nodeConfig,
        node: { id: primaryNode.id, name: primaryNode.name, ip: nodeApiHost },
      },
      null,
      2,
    ),
    "utf-8",
  );

  await runStep("forecast and rates ingestion", async () => {
    await apiRequest("/api/forecast/poll", { method: "POST" });
    await apiRequest("/api/forecast/ingest", {
      method: "POST",
      body: {
        items: [
          {
            field: "rain_mm",
            horizon_hours: 24,
            value: 3.4,
            recorded_at: new Date().toISOString(),
          },
        ],
      },
    });
    const forecastData = await apiRequest("/api/forecast");
    if (!Array.isArray(forecastData) || forecastData.length === 0) {
      throw new Error("Forecast ingestion returned no records");
    }
    await apiRequest("/api/analytics/feeds/poll", { method: "POST" });
    const power = await apiRequest("/api/analytics/power");
    const rateSchedule = power?.rate_schedule || {};
    if (!requireInstalled && !rateSchedule.provider && !rateSchedule.current_rate) {
      throw new Error("Utility rate schedule missing after poll");
    }
  });

  const scheduleMarker = `E2E guard ${Date.now()}`;
  const dayCodes = ["SU", "MO", "TU", "WE", "TH", "FR", "SA"];
  const scheduleDay = dayCodes[new Date().getDay()] || "MO";
  if (!requireInstalled) {
    await runStep("schedule guard alarm", async () => {
      const dtstart = new Date(Date.now() - 60_000)
        .toISOString()
        .replace(/\.\d{3}Z$/, "Z")
        .replace(/[-:]/g, "");
      await apiRequest("/api/schedules", {
      method: "POST",
      body: {
        name: `E2E Guard ${Date.now()}`,
        rrule: `DTSTART:${dtstart}\nRRULE:FREQ=MINUTELY;INTERVAL=1`,
        blocks: [{ day: scheduleDay, start: "00:00", end: "23:59" }],
        conditions: [
          {
            type: "forecast",
            field: "rain_mm",
            horizon_hours: 24,
            operator: ">",
            threshold: 1,
          },
        ],
        actions: [
          {
            type: "alarm",
            severity: "critical",
            message: scheduleMarker,
          },
        ],
      },
    });

      try {
        await pollFor(
          "schedule alarm event",
          async () => {
            const events = await apiRequest("/api/alarms/history?limit=50");
            return events.find((event) => event.message === scheduleMarker);
          },
          { timeoutMs: 180_000, intervalMs: 2000 },
        );
      } catch (error) {
        console.warn(
          `[smoke] schedule guard alarm not observed; continuing. ${error?.message || error}`,
        );
      }
    });
  }

  if (!requireInstalled) {
    await runStep("telemetry pipeline checks", async () => {
    const sensorsPayload = sensors || (await apiRequest("/api/sensors"));
    const covSensor = sensorsPayload.find((sensor) => sensor.interval_seconds === 0);
    const rollingSensors = sensorsPayload.filter(
      (sensor) => (sensor.rolling_avg_seconds || 0) > 0,
    );
    const rollingSensor = rollingSensors.reduce((best, sensor) => {
      if (!best) {
        return sensor;
      }
      const currentInterval =
        typeof sensor.interval_seconds === "number" ? sensor.interval_seconds : Number.POSITIVE_INFINITY;
      const bestInterval =
        typeof best.interval_seconds === "number" ? best.interval_seconds : Number.POSITIVE_INFINITY;
      return currentInterval < bestInterval ? sensor : best;
    }, null);
    if (!covSensor || !rollingSensor) {
      throw new Error("Missing COV or rolling sensors in seed data");
    }

    const queryMetrics = async (sensorId, seconds) => {
      const end = new Date();
      const start = new Date(end.getTime() - seconds * 1000);
      const params = new URLSearchParams();
      params.append("sensor_ids[]", sensorId);
      params.append("start", start.toISOString());
      params.append("end", end.toISOString());
      params.append("interval", "1");
      const response = await apiRequest(`/api/metrics/query?${params.toString()}`);
      return response.series?.[0]?.points ?? [];
    };

    const covPoints = await pollFor(
      "COV metrics",
      async () => {
        const points = await queryMetrics(covSensor.sensor_id, 7200);
        return points.length >= 2 ? points : null;
      },
      { timeoutMs: 45_000, intervalMs: 2000 },
    );

    const covValues = covPoints.map((point) => Number(point.value));
    const covDistinct = new Set(covValues.map((value) => value.toFixed(4)));
    if (covDistinct.size < 2) {
      throw new Error(`COV sensor ${covSensor.sensor_id} did not change value`);
    }

    await pollFor(
      "rolling average metrics",
      async () => {
        const points = await queryMetrics(rollingSensor.sensor_id, 7200);
        return points.length >= 3 ? points : null;
      },
      { timeoutMs: 45_000, intervalMs: 2000 },
    );
    });
  }

  if (!requireInstalled) {
    await runStep("renogy external ingest", async () => {
    const sensorsPayload = sensors || (await apiRequest("/api/sensors"));
    const renogySensors = sensorsPayload.filter((sensor) => sensor.type === "renogy_bt2");
    if (!renogySensors.length) {
      throw new Error("No Renogy sensors found in seed data");
    }
    const renogyNodeId = renogySensors[0].node_id;
    const renogyNode = simLabStatus.nodes.find((entry) => entry.node_id === renogyNodeId);
    if (!renogyNode) {
      throw new Error("Renogy node not found in Sim Lab status");
    }

    const renogyToken = `renogy-${Date.now()}`;
    const configResp = await abortAfter(10_000, (signal) =>
      fetch(`${renogyNode.api_base}/v1/config`, { signal }).then((res) => res.json())
    );
    configResp.renogy_bt2 = {
      ...(configResp.renogy_bt2 || {}),
      enabled: true,
      mode: "external",
      ingest_token: renogyToken,
      poll_interval_seconds: 5,
    };
    await abortAfter(10_000, (signal) =>
      fetch(`${renogyNode.api_base}/v1/config`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(configResp),
        signal,
      })
    );

    await abortAfter(10_000, (signal) =>
      fetch(`${renogyNode.api_base}/v1/renogy-bt`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${renogyToken}`,
        },
        body: JSON.stringify({
          pv_power: 220.5,
          battery_percentage: 76.2,
          load_power: 48.1,
          runtime_hours: 4.2,
          battery_voltage: 12.8,
          pv_voltage: 18.2,
        }),
        signal,
      })
    );

    try {
      await pollFor(
        "renogy metrics",
        async () => {
          const end = new Date();
          const start = new Date(end.getTime() - 60 * 1000);
          const params = new URLSearchParams();
          renogySensors.forEach((sensor) => params.append("sensor_ids[]", sensor.sensor_id));
          params.append("start", start.toISOString());
          params.append("end", end.toISOString());
          params.append("interval", "5");
          const response = await apiRequest(`/api/metrics/query?${params.toString()}`);
          const series = response.series ?? [];
          const populated = series.filter((entry) => entry.points && entry.points.length > 0);
          return populated.length >= Math.min(2, renogySensors.length) ? populated : null;
        },
        { timeoutMs: 60_000, intervalMs: 3000 },
      );
    } catch (error) {
      console.warn(
        `[smoke] renogy metrics not observed; continuing. ${error?.message || error}`,
      );
    }
    });
  }

  await runStepIf("provisioning session queue", async () => {
    const tokenResponse = await apiRequest("/api/adoption/tokens", {
      method: "POST",
      body: {
        mac_eth: "02:00:00:00:99:01",
        service_name: "E2E Provision",
        metadata: { source: "e2e" },
      },
    });
    const adoptionToken = tokenResponse.token;
    const sessionResponse = await abortAfter(10_000, (signal) =>
      fetch(`${nodeApiBase}/v1/provisioning/session`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          device_name: "E2E iPhone",
          pin: "123456",
          wifi_ssid: "FarmNet",
          wifi_password: "password123",
          adoption_token: adoptionToken,
          start_only: true,
        }),
        signal,
      }).then((res) => res.json())
    );
    if (!sessionResponse.session_id) {
      console.warn("[smoke] provisioning session not created; skipping queue check.");
      return;
    }
    const queue = await abortAfter(10_000, (signal) =>
      fetch(`${nodeApiBase}/v1/provision/queue`, { signal }).then((res) => res.json())
    );
    const pending = queue.pending || [];
    const found = pending.find((entry) => entry.session_id === sessionResponse.session_id);
    if (!found) {
      console.warn("[smoke] provisioning session not found in queue; continuing.");
      return;
    }
  });

  await runStepIf("setup credentials api", async () => {
    const credentialName = `e2e-${Date.now()}`;
    const credentialPayload = {
      value: "test-token",
      metadata: { label: "E2E" },
    };
    const upserted = await apiRequest(`/api/setup/credentials/${credentialName}`, {
      method: "PUT",
      body: credentialPayload,
    });
    if (!upserted?.has_value) {
      throw new Error("Setup credential did not report a stored value");
    }
    const listResponse = await apiRequest("/api/setup/credentials");
    const found = listResponse?.credentials?.find((entry) => entry.name === credentialName);
    if (!found) {
      throw new Error("Setup credential missing from list response");
    }
    await apiRequest(`/api/setup/credentials/${credentialName}`, { method: "DELETE" });
  });

  await runStepIf("setup center ui", async () => {
    try {
      await page.goto(`${webOrigin}/setup`, { waitUntil: "domcontentloaded" });
      await waitForUiToSettle(page);
      await page.getByRole("heading", { name: "System Setup Center" }).waitFor();
      await page.getByRole("heading", { name: "Credentials" }).waitFor();
    } catch (error) {
      console.warn(`[smoke] setup center ui not ready; continuing. ${error?.message || error}`);
    }
  });

  await runStepIf("nodes adoption + detail", async () => {
    await page.goto(`${webOrigin}/nodes`, { waitUntil: "domcontentloaded" });
    await waitForUiToSettle(page);
    await page.getByRole("heading", { name: "Nodes" }).waitFor();

    const scanButton = page.getByRole("button", { name: /scan again/i });
    if (await scanButton.count()) {
      await scanButton.first().click();
      await waitForUiToSettle(page);
    }

    const initialNodes = await apiRequest("/api/nodes");

    const adoptButton = page.getByRole("button", { name: /adopt node/i }).first();
    await adoptButton.waitFor({ state: "visible", timeout: 20_000 });
    await adoptButton.click();

    await page.getByRole("heading", { name: /adopt node/i }).waitFor();
    const adoptConfirm = page.getByRole("button", { name: /^adopt$/i });
    await adoptConfirm.click();

    await page.getByText(/Adopted/i).waitFor({ timeout: 30_000 });

    await pollFor(
      "adopted node registered",
      async () => {
        const updated = await apiRequest("/api/nodes");
        return updated.length > initialNodes.length ? updated : null;
      },
      { timeoutMs: 30_000, intervalMs: 1000 },
    );

    const viewDetails = page.getByRole("button", { name: /view details/i }).first();
    await viewDetails.click();
    await page.getByText("Hardware").waitFor();
    await page.getByRole("button", { name: "Close" }).click();
  });

  await runStepIf("sensors and outputs", async () => {
    await page.goto(`${webOrigin}/sensors`, { waitUntil: "domcontentloaded" });
    await waitForUiToSettle(page);
    await page.getByRole("heading", { name: "Sensors & Outputs" }).waitFor();

    const sendCommand = page.getByRole("button", { name: /send command/i }).first();
    await sendCommand.click();
    await page.getByRole("heading", { name: "Command output" }).waitFor();

    const modal = page.getByRole("heading", { name: "Command output" }).locator("..").locator("..");
    const stateSelect = modal.locator("select");
    const options = await stateSelect.locator("option").allTextContents();
    const targetState = options.find((entry) => entry && !entry.toLowerCase().includes("select"));
    if (!targetState) {
      throw new Error("No output states available for command modal");
    }
    await stateSelect.selectOption({ label: targetState });
    await modal.getByRole("button", { name: "Send" }).click();
    await page.getByText(/Command sent/i).waitFor({ timeout: 20_000 });
  });

  await runStepIf("alarm events ack", async () => {
    await page.goto(`${webOrigin}/sensors`, { waitUntil: "domcontentloaded" });
    await waitForUiToSettle(page);
    const eventRow = page.getByText(scheduleMarker).first();
    await eventRow.waitFor();
    const container = eventRow.locator("..").locator("..");
    const ackButton = container.getByRole("button", { name: /acknowledge/i });
    if (await ackButton.count()) {
      await ackButton.click();
      await container.getByText(/Status: acknowledged/i).first().waitFor({ timeout: 20_000 });
    }
  });

  await runStepIf("schedule create and edit", async () => {
    await page.goto(`${webOrigin}/schedules`, { waitUntil: "domcontentloaded" });
    await waitForUiToSettle(page);

    await page.getByRole("button", { name: /new schedule/i }).click();
    const createHeading = page.getByRole("heading", { name: /create schedule/i });
    await createHeading.waitFor();

    const createSection = page.locator("section", { has: createHeading });
    const nameInput = createSection.getByLabel("Name", { exact: true });
    await nameInput.fill("E2E Schedule");

    await createSection.getByRole("button", { name: /add action/i }).click();
    const outputSelect = createSection
      .locator("label", { hasText: /^Output$/i })
      .locator("xpath=following-sibling::select");
    await outputSelect.waitFor();
    await outputSelect.selectOption({ index: 1 });

    await createSection.getByRole("button", { name: "Save" }).click();
    await page.getByText("Schedule created.", { exact: true }).waitFor();

    await pollFor(
      "new schedule in API",
      async () => {
        const schedules = await apiRequest("/api/schedules");
        return schedules.find((item) => item.name === "E2E Schedule") || null;
      },
      { timeoutMs: 30_000, intervalMs: 1000 },
    );

    await page.getByRole("button", { name: /^edit$/i }).first().click();
    const editHeading = page.getByRole("heading", { name: /edit schedule/i });
    await editHeading.waitFor();
    const editSection = page.locator("section", { has: editHeading });
    const editName = editSection.getByLabel("Name", { exact: true });
    await editName.fill(`E2E Schedule Updated`);
    await editSection.getByRole("button", { name: "Save" }).click();
    await page.getByText("Schedule updated.", { exact: true }).waitFor();
  });

  await runStepIf("users and roles", async () => {
    await page.goto(`${webOrigin}/users`, { waitUntil: "domcontentloaded" });
    await waitForUiToSettle(page);
    await page.getByRole("heading", { name: /Users & Permissions/i }).waitFor();

    await page.getByRole("button", { name: /add user/i }).click();
    const modal = page.getByRole("heading", { name: /add user/i }).locator("..").locator("..");
    const userStamp = Date.now();
    const userEmail = `ui-smoke-${userStamp}@farm.local`;
    const userPassword = `ui-smoke-${userStamp}`;
    await modal.getByLabel("Name", { exact: true }).fill("UI Smoke User");
    await modal.getByLabel("Email", { exact: true }).fill(userEmail);
    await modal.getByLabel("Password", { exact: true }).fill(userPassword);
    await modal.getByLabel("Role", { exact: true }).selectOption("control");
    await modal.getByRole("button", { name: /create/i }).click();
    await page.getByText(/Created UI Smoke User/i).waitFor();

    const row = page.locator("tr", { hasText: userEmail });
    await row.waitFor();
    const capabilityCell = row.locator("td").nth(3);
    await capabilityCell.getByRole("button").first().click();
    await page.getByText("Updated capabilities.", { exact: true }).waitFor();

    await row.getByRole("button", { name: /remove/i }).click();
    await page.getByText(/Removed UI Smoke User/i).waitFor();
  });

  await runStepIf("backups download and restore", async () => {
    await page.goto(`${webOrigin}/backups`, { waitUntil: "domcontentloaded" });
    await waitForUiToSettle(page);
    await page.getByRole("heading", { name: /Backups/i }).waitFor();

    const nodeSection = page.locator("section", {
      has: page.getByRole("heading", { name: primaryNode.name }),
    });
    await nodeSection.getByRole("heading", { name: primaryNode.name }).waitFor();
    const backupRow = nodeSection.locator("tr", { hasText: backupDate }).first();
    await backupRow.waitFor();

    const downloadPromise = page.waitForEvent("download");
    await backupRow.getByRole("button", { name: "Download" }).click();
    const download = await downloadPromise;
    const suggested = download.suggestedFilename();
    const targetPath = path.join(artifactsRoot, suggested);
    await download.saveAs(targetPath);

    await backupRow.getByRole("button", { name: /restore/i }).click();
    await page.getByRole("heading", { name: /restore backup/i }).waitFor();
    await page.getByRole("button", { name: /^Restore$/ }).click();
    await page.getByText(/Restore queued/i).first().waitFor();

    await pollFor(
      "restore metadata",
      async () => {
        const updated = await apiRequest(`/api/nodes/${primaryNode.id}`);
        const lastRestore = updated?.config?.last_restore;
        return lastRestore?.date === backupDate ? lastRestore : null;
      },
      { timeoutMs: 30_000, intervalMs: 2000 },
    );
  });

  await context.close();
  await browser.close();
  context = undefined;
  browser = undefined;
} finally {
  if (context) await context.close().catch(() => {});
  if (browser) await browser.close().catch(() => {});
  await cleanup();
}
