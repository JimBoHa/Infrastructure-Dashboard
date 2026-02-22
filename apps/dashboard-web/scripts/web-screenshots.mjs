#!/usr/bin/env node
import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { chromium, devices, webkit } from "playwright";

const dirname = path.dirname(fileURLToPath(import.meta.url));
const dashboardWebDir = path.resolve(dirname, "..");
const repoRoot = path.resolve(dashboardWebDir, "..", "..");

const pad2 = (value) => String(value).padStart(2, "0");
const timestampSlug = () => {
  const now = new Date();
  return `${now.getFullYear()}${pad2(now.getMonth() + 1)}${pad2(now.getDate())}_${pad2(now.getHours())}${pad2(now.getMinutes())}${pad2(now.getSeconds())}`;
};

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

const manualScreenshotsRoot =
  args.get("out-dir") ||
  process.env.FARM_SCREENSHOT_DIR ||
  path.resolve(repoRoot, "manual_screenshots_web", timestampSlug());

const apiBase =
  args.get("api-base") ||
  process.env.FARM_SCREENSHOT_API_BASE ||
  process.env.FARM_CORE_API_BASE ||
  process.env.NEXT_PUBLIC_API_BASE ||
  "http://127.0.0.1:8000";

const webBaseArg =
  args.get("base-url") ||
  process.env.FARM_SCREENSHOT_BASE_URL ||
  null;

const browserName =
  args.get("browser") || process.env.FARM_SCREENSHOT_BROWSER || "chromium";
const deviceName = args.get("device") || process.env.FARM_SCREENSHOT_DEVICE || null;

const preferredWebHost =
  args.get("web-host") || process.env.FARM_SCREENSHOT_WEB_HOST || "127.0.0.1";
const preferredWebPort = Number(
  args.get("web-port") || process.env.FARM_SCREENSHOT_WEB_PORT || "3000",
);

const startCoreRequested = !flags.has("no-core");
const startWebRequested = !flags.has("no-web");

const stubAuthRequested =
  flags.has("stub-auth") || process.env.FARM_SCREENSHOT_STUB_AUTH === "1";
const allowStubFallback =
  flags.has("allow-stub-fallback") ||
  process.env.FARM_SCREENSHOT_ALLOW_STUB_FALLBACK === "1";
const blockExternal =
  flags.has("block-external") || process.env.FARM_SCREENSHOT_BLOCK_EXTERNAL === "1";
const applyNodeSensor =
  flags.has("apply-node-sensor") || process.env.FARM_SCREENSHOT_APPLY_NODE_SENSOR === "1";
const applyNodeSensorName =
  args.get("apply-node-sensor-name") || process.env.FARM_SCREENSHOT_APPLY_NODE_SENSOR_NAME || "DT64 ADC0 Voltage";

const loginEmailRaw =
  args.get("login-email") ||
  process.env.FARM_SCREENSHOT_LOGIN_EMAIL ||
  process.env.NEXT_PUBLIC_DEV_LOGIN_EMAIL ||
  process.env.FARM_DEV_LOGIN_EMAIL ||
  null;
const loginPasswordRaw =
  args.get("login-password") ||
  process.env.FARM_SCREENSHOT_LOGIN_PASSWORD ||
  process.env.NEXT_PUBLIC_DEV_LOGIN_PASSWORD ||
  process.env.FARM_DEV_LOGIN_PASSWORD ||
  null;
const loginEmail = loginEmailRaw ? loginEmailRaw.trim() : null;
const loginPassword = loginPasswordRaw ? loginPasswordRaw.trim() : null;

const authTokenFile =
  args.get("auth-token-file") ||
  process.env.FARM_SCREENSHOT_AUTH_TOKEN_FILE ||
  null;

const authTokenRaw =
  args.get("auth-token") || process.env.FARM_SCREENSHOT_AUTH_TOKEN || null;

const focusNodeIdRaw =
  args.get("focus-node-id") || process.env.FARM_SCREENSHOT_FOCUS_NODE_ID || null;

const relatedFocusSensorIdRaw =
  args.get("related-focus-sensor-id") ||
  process.env.FARM_SCREENSHOT_RELATED_FOCUS_SENSOR_ID ||
  null;
const relatedFocusSensorId = relatedFocusSensorIdRaw ? relatedFocusSensorIdRaw.trim() : null;

const relatedRangeHoursRaw =
  args.get("related-range-hours") ||
  process.env.FARM_SCREENSHOT_RELATED_RANGE_HOURS ||
  null;
const relatedRangeHours = (() => {
  if (!relatedRangeHoursRaw) return 168;
  const parsed = Number(relatedRangeHoursRaw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 168;
})();

const relatedIntervalSecondsRaw =
  args.get("related-interval-seconds") ||
  process.env.FARM_SCREENSHOT_RELATED_INTERVAL_SECONDS ||
  null;
const relatedIntervalSeconds = (() => {
  if (!relatedIntervalSecondsRaw) return null;
  const parsed = Number(relatedIntervalSecondsRaw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
})();

const relatedScopeRaw =
  args.get("related-scope") ||
  process.env.FARM_SCREENSHOT_RELATED_SCOPE ||
  null;
const relatedScope = (() => {
  const normalized = (relatedScopeRaw ?? "all_nodes").trim().toLowerCase();
  return normalized === "same_node" ? "same_node" : "all_nodes";
})();

const relatedTimeoutMsRaw =
  args.get("related-timeout-ms") ||
  process.env.FARM_SCREENSHOT_RELATED_TIMEOUT_MS ||
  null;
const relatedTimeoutMs = (() => {
  if (!relatedTimeoutMsRaw) return 180_000;
  const parsed = Number(relatedTimeoutMsRaw);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 180_000;
})();

const readAuthToken = async () => {
  if (authTokenRaw) return authTokenRaw.trim();
  if (!authTokenFile) return null;
  const token = await readFile(authTokenFile, "utf8");
  const trimmed = token.trim();
  return trimmed || null;
};

const escapeRegex = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

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
      // ignore until timeout
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

const safeFileName = (value) =>
  value
    .replace(/^\//, "")
    .replace(/\/+/g, "_")
    .replace(/[^a-zA-Z0-9_.-]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase() || "root";

const tryFetchJson = async (url, { authToken } = {}) => {
  const headers = new Headers();
  if (authToken) headers.set("Authorization", `Bearer ${authToken}`);
  const response = await fetch(url, { headers });
  if (!response.ok) throw new Error(`Request failed (${response.status}) for ${url}`);
  return response.json();
};

const tryPostJson = async (url, body, { timeoutMs = 10_000 } = {}) => {
  const response = await abortAfter(timeoutMs, (signal) =>
    fetch(url, {
      signal,
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    }),
  );
  if (!response.ok) {
    const text = await response.text().catch(() => "");
    throw new Error(`Request failed (${response.status}) for ${url}: ${text || response.statusText}`);
  }
  return response.json();
};

const tryLoginWithPassword = async ({ apiBase, email, password }) => {
  const payload = await tryPostJson(
    `${apiBase.replace(/\/$/, "")}/api/auth/login`,
    { email, password },
    { timeoutMs: 10_000 },
  );
  const token = typeof payload?.token === "string" ? payload.token.trim() : "";
  if (!token) {
    throw new Error("Login succeeded but response token was empty.");
  }
  return token;
};

const isDashboardServing = async (origin) => {
  try {
    const response = await abortAfter(1200, (signal) =>
      fetch(`${origin.replace(/\/$/, "")}/login`, { signal }),
    );
    return response.ok;
  } catch {
    return false;
  }
};

const discoverDashboardBase = async ({ host, ports }) => {
  for (const port of ports) {
    if (!Number.isFinite(port) || port <= 0) continue;
    if (!(await isListening(host, port))) continue;
    const origin = `http://${host}:${port}`;
    if (await isDashboardServing(origin)) return origin;
  }
  return null;
};

const findAvailablePort = async ({ host, startPort, maxAttempts = 20 }) => {
  const base = Number.isFinite(startPort) && startPort > 0 ? startPort : 3000;
  for (let offset = 0; offset < maxAttempts; offset += 1) {
    const port = base + offset;
    if (!(await isListening(host, port))) return port;
  }
  throw new Error(`Unable to find a free port on ${host} starting at ${base}.`);
};

const waitForTextMatch = async (locator, regex, timeoutMs) => {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const text = (await locator.innerText().catch(() => "")) || "";
    if (regex.test(text)) return true;
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return false;
};

let coreProc;
let webProc;

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

try {
  const apiUrl = new URL(apiBase);
  const apiOrigin = `${apiUrl.protocol}//${apiUrl.host}`;

  const preferredPort =
    Number.isFinite(preferredWebPort) && preferredWebPort > 0 ? preferredWebPort : 3000;
  const candidatePorts = Array.from(
    new Set([preferredPort, ...Array.from({ length: 10 }, (_, idx) => 3000 + idx), 3005]),
  );

  let stubAuth = stubAuthRequested;
  let startCore = startCoreRequested && !stubAuth;
  const startWeb = startWebRequested;

  let webBase = webBaseArg;
  if (!webBase) {
    const discovered = await discoverDashboardBase({
      host: preferredWebHost,
      ports: candidatePorts,
    });
    if (discovered) {
      webBase = discovered;
      console.log(`[web] Auto-detected running dashboard on ${webBase}`);
    } else if (!startWebRequested) {
      throw new Error(
        `Dashboard base URL not provided and no running dev server found on ${preferredWebHost} (ports ${candidatePorts.join(
          ", ",
        )}). Either start one (e.g., \`cd apps/dashboard-web && npm run dev\`) or omit --no-web.`,
      );
    } else {
      const port = await findAvailablePort({ host: preferredWebHost, startPort: preferredPort });
      webBase = `http://${preferredWebHost}:${port}`;
    }
  }
  const webUrl = new URL(webBase);

  await mkdir(manualScreenshotsRoot, { recursive: true });

  if (startCore) {
    try {
      const coreDir = path.resolve(repoRoot, "apps", "core-server-rs");
      const coreHost = apiUrl.hostname;
      const corePort = Number(apiUrl.port || (apiUrl.protocol === "https:" ? 443 : 80));

      if (await isListening(coreHost, corePort)) {
        console.log(`[core] Reusing existing server on ${apiBase}`);
      } else {
        if (!process.env.CORE_DATABASE_URL) {
          throw new Error(
            "CORE_DATABASE_URL must be set (run an installed stack or enable stub fallback).",
          );
        }
        console.log(`[core] Starting Rust controller on ${apiBase}`);
        coreProc = spawnServer({
          label: "core",
          command: `cargo run --quiet -- --host ${coreHost} --port ${corePort}`,
          cwd: coreDir,
          env: { ...process.env },
        });
      }
      await waitForHttpOk(`${apiBase.replace(/\/$/, "")}/healthz`, 30_000);
    } catch (error) {
      if (!allowStubFallback) throw error;
      console.warn(`[core] ${String(error)}\n[core] Falling back to stub-auth (explicitly enabled).`);
      await cleanup();
      coreProc = undefined;
      stubAuth = true;
      startCore = false;
    }
  } else if (stubAuth && startCoreRequested) {
    console.log("[core] stub-auth enabled; skipping core-server startup");
  }

  if (startWeb) {
    const webHost = webUrl.hostname;
    const webPort = Number(webUrl.port || (webUrl.protocol === "https:" ? 443 : 80));
    const webOrigin = webUrl.origin;

    if (await isDashboardServing(webOrigin)) {
      console.log(`[web] Reusing existing dashboard on ${webOrigin}`);
    } else {
      console.log(`[web] Starting dashboard on ${webOrigin} (FARM_CORE_API_BASE=${apiOrigin})`);
      webProc = spawnServer({
        label: "web",
        command: `npm run dev -- --hostname ${webHost} --port ${webPort}`,
        cwd: dashboardWebDir,
        env: { ...process.env, FARM_CORE_API_BASE: apiOrigin },
      });
    }
    await waitForHttpOk(`${webOrigin}/nodes`, 90_000);
  } else if (!(await isDashboardServing(webUrl.origin))) {
    throw new Error(`--no-web was set, but dashboard is not reachable at ${webUrl.origin}`);
  }

  const explicitToken = await readAuthToken();
  let authMode = "none";
  let authToken = explicitToken;
  const authTokenFromEnv = process.env.NEXT_PUBLIC_AUTH_TOKEN
    ? String(process.env.NEXT_PUBLIC_AUTH_TOKEN).trim()
    : null;

  if (stubAuth) {
    authMode = stubAuthRequested ? "stub" : "stub-fallback";
    authToken = "playwright-stub-token";
  } else if (authToken) {
    authMode = "token";
  } else if (authTokenFromEnv) {
    authMode = "env-token";
    authToken = authTokenFromEnv;
  } else if (loginEmail && loginPassword) {
    try {
      await waitForHttpOk(`${apiBase.replace(/\/$/, "")}/healthz`, 10_000);
      authToken = await tryLoginWithPassword({
        apiBase,
        email: loginEmail,
        password: loginPassword,
      });
      authMode = "login";
    } catch (error) {
      if (!allowStubFallback) throw error;
      console.warn(`[auth] ${String(error)}\n[auth] Falling back to stub-auth (explicitly enabled).`);
      stubAuth = true;
      authMode = "stub-fallback";
      authToken = "playwright-stub-token";
    }
  } else if (allowStubFallback) {
    console.warn("[auth] No token/login credentials provided; using stub-auth (explicitly enabled).");
    stubAuth = true;
    authMode = "stub-fallback";
    authToken = "playwright-stub-token";
  } else {
    throw new Error(
      "No auth configured. Provide --auth-token/--auth-token-file, set FARM_SCREENSHOT_AUTH_TOKEN(_FILE), " +
        "or provide login credentials via --login-email/--login-password (or FARM_SCREENSHOT_LOGIN_EMAIL/PASSWORD), " +
        "or run with --stub-auth.",
    );
  }
  console.log(`[auth] mode=${authMode}`);

  const stubIsoNow = new Date("2026-01-09T00:00:00.000Z").toISOString();
  const stubNodes = [
    {
      id: "00000000-0000-0000-0000-000000000001",
      name: "Controller",
      status: "online",
      uptime_seconds: 172800,
      cpu_percent: 14.2,
      storage_used_bytes: 88_000_000_000,
      mac_eth: "aa:bb:cc:dd:ee:01",
      mac_wifi: "aa:bb:cc:dd:ee:02",
      ip_last: "10.0.0.10",
      last_seen: stubIsoNow,
      created_at: stubIsoNow,
      config: { kind: "core" },
    },
    {
      id: "11111111-1111-1111-1111-111111111111",
      name: "Pi Node A",
      status: "online",
      uptime_seconds: 54321,
      cpu_percent: 6.7,
      storage_used_bytes: 12_000_000_000,
      mac_eth: "aa:bb:cc:dd:ee:11",
      mac_wifi: "aa:bb:cc:dd:ee:12",
      ip_last: "10.0.0.21",
      last_seen: stubIsoNow,
      created_at: stubIsoNow,
      config: { agent_node_id: "pi5-01" },
    },
    {
      id: "22222222-2222-2222-2222-222222222222",
      name: "Weather Station",
      status: "offline",
      uptime_seconds: null,
      cpu_percent: null,
      storage_used_bytes: null,
      mac_eth: null,
      mac_wifi: "aa:bb:cc:dd:ee:22",
      ip_last: "10.0.0.31",
      last_seen: null,
      created_at: stubIsoNow,
      config: { kind: "ws-2902", protocol: "http" },
    },
  ];

  const stubSensors = [
    {
      sensor_id: "111111111111111111111101",
      node_id: "11111111-1111-1111-1111-111111111111",
      name: "Soil moisture (Field 7 / Zone A)",
      type: "percentage",
      unit: "%",
      interval_seconds: 1800,
      rolling_avg_seconds: 0,
      latest_value: 42.3,
      latest_ts: stubIsoNow,
      status: "ok",
      location: "Field 7",
      created_at: stubIsoNow,
      config: { metric: "soil_moisture_pct", display_decimals: 1 },
    },
    {
      sensor_id: "111111111111111111111102",
      node_id: "11111111-1111-1111-1111-111111111111",
      name: "Ambient temperature",
      type: "temperature",
      unit: "°C",
      interval_seconds: 1800,
      rolling_avg_seconds: 0,
      latest_value: 18.2,
      latest_ts: stubIsoNow,
      status: "ok",
      location: null,
      created_at: stubIsoNow,
      config: { metric: "temperature_c" },
    },
    {
      sensor_id: "111111111111111111111103",
      node_id: "11111111-1111-1111-1111-111111111111",
      name: "Pump current (rolling avg 300s)",
      type: "current",
      unit: "A",
      interval_seconds: 1,
      rolling_avg_seconds: 300,
      latest_value: 2.14,
      latest_ts: stubIsoNow,
      status: "ok",
      location: "Pump house",
      created_at: stubIsoNow,
      config: { metric: "pump_current_a", display_decimals: 2 },
    },
    {
      sensor_id: "111111111111111111111104",
      node_id: "11111111-1111-1111-1111-111111111111",
      name: "Derived: pump power",
      type: "power",
      unit: "W",
      interval_seconds: 1,
      rolling_avg_seconds: 0,
      latest_value: 420.5,
      latest_ts: stubIsoNow,
      status: "ok",
      location: null,
      created_at: stubIsoNow,
      config: {
        source: "derived",
        derived: {
          expression: "a * b",
          inputs: [
            { sensor_id: "111111111111111111111103", var: "a" },
            { sensor_id: "111111111111111111111105", var: "b" },
          ],
        },
        display_decimals: 1,
      },
    },
    {
      sensor_id: "111111111111111111111105",
      node_id: "11111111-1111-1111-1111-111111111111",
      name: "Pump voltage",
      type: "voltage",
      unit: "V",
      interval_seconds: 1,
      rolling_avg_seconds: 0,
      latest_value: 196.4,
      latest_ts: stubIsoNow,
      status: "ok",
      location: null,
      created_at: stubIsoNow,
      config: { metric: "pump_voltage_v", display_decimals: 1 },
    },
    {
      sensor_id: "000000000000000000000001",
      node_id: "00000000-0000-0000-0000-000000000001",
      name: "Forecast: cloud cover",
      type: "percentage",
      unit: "%",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 12,
      latest_ts: stubIsoNow,
      status: "ok",
      location: null,
      created_at: stubIsoNow,
      config: { source: "forecast_points", provider: "open_meteo", kind: "weather", mode: "current" },
    },
    {
      sensor_id: "222222222222222222222201",
      node_id: "22222222-2222-2222-2222-222222222222",
      name: "WS outdoor temperature",
      type: "temperature",
      unit: "°C",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 16.7,
      latest_ts: stubIsoNow,
      status: "ok",
      location: "North field",
      created_at: stubIsoNow,
      config: { source: "ws_2902", metric: "temperature_outdoor_c" },
    },
    {
      sensor_id: "222222222222222222222202",
      node_id: "22222222-2222-2222-2222-222222222222",
      name: "WS rain rate",
      type: "flow",
      unit: "mm/h",
      interval_seconds: 30,
      rolling_avg_seconds: 0,
      latest_value: 0,
      latest_ts: stubIsoNow,
      status: "ok",
      location: "North field",
      created_at: stubIsoNow,
      config: { source: "ws_2902", metric: "rain_rate_mm_h" },
    },
  ];

  const stubOutputs = [
    {
      id: "output-111",
      node_id: "11111111-1111-1111-1111-111111111111",
      name: "Pump relay",
      type: "relay",
      state: "off",
      last_command: stubIsoNow,
      supported_states: ["on", "off"],
      schedule_ids: ["schedule-1"],
      history: [],
      config: {},
    },
    {
      id: "output-112",
      node_id: "11111111-1111-1111-1111-111111111111",
      name: "Valve zone A",
      type: "relay",
      state: "on",
      last_command: stubIsoNow,
      supported_states: ["on", "off"],
      schedule_ids: ["schedule-2"],
      history: [],
      config: {},
    },
  ];

  const stubSchedules = [
    {
      id: "schedule-1",
      name: "Irrigation AM",
      rrule: "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=6;BYMINUTE=0;BYSECOND=0",
      blocks: [{ day: "mon", start: "06:00", end: "06:15" }],
      conditions: [],
      actions: [],
      next_run: stubIsoNow,
    },
    {
      id: "schedule-2",
      name: "Irrigation PM",
      rrule: "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR;BYHOUR=18;BYMINUTE=0;BYSECOND=0",
      blocks: [{ day: "mon", start: "18:00", end: "18:10" }],
      conditions: [],
      actions: [],
      next_run: stubIsoNow,
    },
  ];

  const stubAlarms = [
    {
      id: 1,
      name: "Soil moisture low",
      sensor_id: "111111111111111111111101",
      node_id: "11111111-1111-1111-1111-111111111111",
      status: "active",
      origin: "predictive",
      anomaly_score: 0.72,
      last_fired: stubIsoNow,
      last_raised: stubIsoNow,
      message: "Soil moisture trending low",
      type: "predictive",
      severity: "warning",
      target_type: "sensor",
      target_id: "111111111111111111111101",
      condition: {},
      active: true,
    },
    {
      id: 2,
      name: "Node offline",
      sensor_id: null,
      node_id: "22222222-2222-2222-2222-222222222222",
      status: "active",
      origin: "standard",
      anomaly_score: null,
      last_fired: stubIsoNow,
      last_raised: stubIsoNow,
      message: "Weather Station offline",
      type: "node_offline",
      severity: "warning",
      target_type: "node",
      target_id: "22222222-2222-2222-2222-222222222222",
      condition: {},
      active: true,
    },
  ];

  const nodeList = stubAuth
    ? stubNodes
    : await tryFetchJson(`${apiBase.replace(/\/$/, "")}/api/nodes`, { authToken }).catch(() => []);
  const sensorList = stubAuth
    ? stubSensors
    : await tryFetchJson(`${apiBase.replace(/\/$/, "")}/api/sensors`, { authToken }).catch(() => []);

  const focusNodeId = focusNodeIdRaw ? focusNodeIdRaw.trim() : null;
  const firstNodeId = (() => {
    if (!Array.isArray(nodeList)) return null;
    if (focusNodeId) {
      const match = nodeList.find((node) => node?.id === focusNodeId);
      if (match?.id) return match.id;
      console.warn(`[warn] Focus node id "${focusNodeId}" not found; falling back to first node.`);
    }
    return typeof nodeList?.[0]?.id === "string" ? nodeList[0].id : null;
  })();
  const firstAgentNodeId = (() => {
    if (!Array.isArray(nodeList)) return null;
    const match = nodeList.find((node) => {
      const agentNodeId = node?.config?.agent_node_id;
      return typeof agentNodeId === "string" && agentNodeId.trim().length > 0;
    });
    return typeof match?.id === "string" ? match.id : null;
  })();
  const firstWeatherStationNodeId = (() => {
    if (!Array.isArray(nodeList)) return null;
    const match = nodeList.find((node) => {
      const kind = node?.config?.kind;
      return typeof kind === "string" && kind.trim().toLowerCase() === "ws-2902";
    });
    return typeof match?.id === "string" ? match.id : null;
  })();
  const nodeNameById = Array.isArray(nodeList)
    ? new Map(nodeList.map((node) => [node.id, node.name]))
    : new Map();
  const firstAgentNodeName =
    typeof firstAgentNodeId === "string" ? nodeNameById.get(firstAgentNodeId) ?? null : null;
  const firstWeatherStationNodeName =
    typeof firstWeatherStationNodeId === "string" ? nodeNameById.get(firstWeatherStationNodeId) ?? null : null;
  const coreNodeId = "00000000-0000-0000-0000-000000000001";
  const firstSensorId = typeof sensorList?.[0]?.sensor_id === "string" ? sensorList[0].sensor_id : null;
  const sensorDetailId = (() => {
    if (!Array.isArray(sensorList)) return firstSensorId;
    const priority = ["voltage", "current", "power", "percentage"];
    for (const type of priority) {
      const match = sensorList.find(
        (sensor) => sensor?.type === type && typeof sensor?.sensor_id === "string",
      );
      if (match) return match.sensor_id;
    }
    return firstSensorId;
  })();
  const trendsExampleSensorIds = (() => {
    if (!Array.isArray(sensorList) || sensorList.length < 2) return [];
    const pool = firstAgentNodeId
      ? sensorList.filter((sensor) => sensor?.node_id === firstAgentNodeId)
      : sensorList;

    const selectByName = (needle) => {
      const match = pool.find((sensor) => {
        const name = sensor?.name;
        return typeof name === "string" && name.toLowerCase().includes(needle);
      });
      return typeof match?.sensor_id === "string" ? match.sensor_id : null;
    };

    const preferred = [
      selectByName("pv power"),
      selectByName("battery current"),
      selectByName("load power"),
    ].filter(Boolean);

    const fallback = pool
      .map((sensor) => (typeof sensor?.sensor_id === "string" ? sensor.sensor_id : null))
      .filter(Boolean);

    return Array.from(new Set([...preferred, ...fallback])).slice(0, 3);
  })();
  const ws2902CustomSensorId = (() => {
    if (!Array.isArray(sensorList)) return null;
    const match = sensorList.find((sensor) => {
      const source = sensor?.config?.source;
      const wsField = sensor?.config?.ws_field;
      return (
        source === "ws_2902" &&
        typeof wsField === "string" &&
        wsField.trim().length > 0 &&
        typeof sensor?.sensor_id === "string"
      );
    });
    return typeof match?.sensor_id === "string" ? match.sensor_id : null;
  })();
  const ws2902RainDailySensorId = (() => {
    if (!Array.isArray(sensorList)) return null;
    const match = sensorList.find((sensor) => {
      const source = sensor?.config?.source;
      const type = sensor?.type;
      return (
        source === "ws_2902" &&
        typeof type === "string" &&
        type.trim().toLowerCase() === "rain" &&
        typeof sensor?.sensor_id === "string"
      );
    });
    return typeof match?.sensor_id === "string" ? match.sensor_id : null;
  })();
  const ws2902WindDirectionSensorId = (() => {
    if (!Array.isArray(sensorList)) return null;
    const match = sensorList.find((sensor) => {
      const source = sensor?.config?.source;
      const type = sensor?.type;
      return (
        source === "ws_2902" &&
        typeof type === "string" &&
        type.trim().toLowerCase() === "wind_direction" &&
        typeof sensor?.sensor_id === "string"
      );
    });
    return typeof match?.sensor_id === "string" ? match.sensor_id : null;
  })();

  const browserType = browserName === "webkit" ? webkit : chromium;
  const browser = await browserType.launch({ headless: true });

  const device = deviceName && devices[deviceName] ? devices[deviceName] : null;
  if (deviceName && !device) {
    console.warn(`[warn] Unknown Playwright device "${deviceName}". Falling back to desktop viewport.`);
  }

  const createContext = async ({ withAuth }) => {
    const baseOptions = device
      ? { ...device }
      : {
          viewport: { width: 1440, height: 900 },
        };
    const context = await browser.newContext({
      ...baseOptions,
      locale: "en-US",
      timezoneId: "America/Los_Angeles",
      extraHTTPHeaders: withAuth && authToken ? { Authorization: `Bearer ${authToken}` } : undefined,
    });
    if (withAuth && authToken) {
      await context.addInitScript(({ token }) => {
        try {
          window.sessionStorage.setItem("farmdashboard.auth.token", token);
          window.localStorage.removeItem("farmdashboard.auth.token");
        } catch {
          // ignore
        }
      }, { token: authToken });
    }
    if (withAuth && stubAuth) {
      const stubUser = {
        id: "playwright-user",
        email: "playwright@farmdashboard.local",
        role: "admin",
        source: "playwright",
        capabilities: [
          "config.write",
          "users.manage",
          "schedules.write",
          "outputs.command",
          "alerts.view",
          "alerts.ack",
          "analytics.view",
        ],
      };
      const connectionHost = (() => {
        try {
          const parsed = new URL(apiBase);
          return parsed.host;
        } catch {
          return "127.0.0.1:8000";
        }
      })();

      const emptyArrayEndpoints = new Set([
        "/api/nodes",
        "/api/sensors",
        "/api/outputs",
        "/api/alarms",
        "/api/alarms/history",
        "/api/schedules",
        "/api/users",
        "/api/scan",
        "/api/backups",
        "/api/backups/recent-restores",
        "/api/api-tokens",
        "/api/forecast/history",
        "/api/map/features",
      ]);

      const defaultJsonByPath = {
        "/api/auth/me": stubUser,
        "/api/nodes": stubNodes,
        "/api/sensors": stubSensors,
        "/api/outputs": stubOutputs,
        "/api/schedules": stubSchedules,
        "/api/alarms": stubAlarms,
        "/api/alarms/history": [],
        "/api/connection": {
          status: "online",
          mode: "local",
          local_address: connectionHost,
          cloud_address: "unknown",
          last_switch: null,
        },
        "/api/predictive/status": {
          enabled: false,
          running: false,
          token_present: false,
          api_base_url: "",
          model: null,
          fallback_models: [],
          bootstrap_on_start: false,
          bootstrap_max_sensors: 25,
          bootstrap_lookback_hours: 24,
        },
        "/api/predictive/trace": [],
        "/api/dev/activity": { active: false, message: null, updated_at: null, expires_at: null },
        "/api/backups/retention": {
          default_keep_days: 30,
          policies: [],
          last_cleanup_at: null,
          last_cleanup: null,
          last_cleanup_time: null,
        },
        "/api/setup/credentials": { credentials: [] },
        "/api/setup/emporia/devices": { token_present: false, site_ids: [], devices: [] },
        "/api/setup/integrations/mapillary/token": { configured: false, access_token: null },
        "/api/forecast/status": { enabled: false, providers: {} },
        "/api/forecast/weather/config": {
          enabled: false,
          provider: null,
          latitude: null,
          longitude: null,
          updated_at: stubIsoNow,
        },
        "/api/analytics/feeds/status": { enabled: false, feeds: {}, history: [] },
        "/api/analytics/power": {},
        "/api/analytics/water": {},
        "/api/analytics/soil": {},
        "/api/analytics/status": {},
        "/api/map/saves": [
          { id: 1, name: "Default", created_at: stubIsoNow, updated_at: stubIsoNow },
        ],
        "/api/map/settings": {
          active_save_id: 1,
          active_save_name: "Default",
          active_base_layer_id: null,
          center_lat: 36.9741,
          center_lng: -122.0308,
          zoom: 16,
          bearing: 0,
          pitch: 0,
          updated_at: stubIsoNow,
        },
        "/api/map/layers": [],
      };

      await context.route("**/*", async (route) => {
        const requestUrl = route.request().url();
        if (!requestUrl.startsWith("http")) return route.continue();
        const url = new URL(requestUrl);
        if (!url.pathname.startsWith("/api/")) return route.continue();

        if (url.pathname in defaultJsonByPath) {
          return route.fulfill({
            status: 200,
            contentType: "application/json",
            body: JSON.stringify(defaultJsonByPath[url.pathname]),
          });
        }
        if (emptyArrayEndpoints.has(url.pathname)) {
          return route.fulfill({
            status: 200,
            contentType: "application/json",
            body: "[]",
          });
        }

        return route.fulfill({
          status: 200,
          contentType: "application/json",
          body: "null",
        });
      });
    }
    if (blockExternal && !stubAuth) {
      const allow = new Set([apiUrl.origin, webUrl.origin]);
      await context.route("**/*", async (route) => {
        const requestUrl = route.request().url();
        if (!requestUrl.startsWith("http")) return route.continue();
        const origin = new URL(requestUrl).origin;
        if (!allow.has(origin)) {
          return route.abort("failed");
        }
        return route.continue();
      });
    }
    return context;
  };

  const publicContext = await createContext({ withAuth: false });
  const publicPage = await publicContext.newPage();
  publicPage.setDefaultTimeout(30_000);

  const authedContext = await createContext({ withAuth: true });
  const page = await authedContext.newPage();
  page.setDefaultTimeout(30_000);

  const manifest = [];

  const cardByTitle = (currentPage, titlePattern) =>
    currentPage
      .locator("h3")
      .filter({ hasText: titlePattern })
      .first()
      .locator("xpath=ancestor::div[contains(@class,'bg-card')][1]");

  const ensureCardOpen = async (card) => {
    const stateHost = card.locator("xpath=ancestor::*[@data-state][1]").first();
    const state = await stateHost.getAttribute("data-state").catch(() => null);
    if (state === "closed") {
      const trigger = card.locator("button").first();
      if (await trigger.count()) {
        await trigger.click({ timeout: 4000 }).catch(() => {});
      }
    }
  };

  const expandSensorNodeCards = async ({ currentPage, sensorPickerCard }) => {
    const headings = sensorPickerCard.locator("h3");
    const headingCount = Math.min(await headings.count(), 60);
    for (let idx = 0; idx < headingCount; idx += 1) {
      const heading = headings.nth(idx);
      const text = (await heading.textContent().catch(() => "") || "").trim();
      if (!text || /sensor picker|key|how it works/i.test(text)) continue;
      const nestedCard = heading.locator("xpath=ancestor::div[contains(@class,'bg-card')][1]");
      const nestedStateHost = nestedCard.locator("xpath=ancestor::*[@data-state][1]").first();
      const nestedState = await nestedStateHost.getAttribute("data-state").catch(() => null);
      if (nestedState === "closed") {
        const trigger = nestedCard.locator("button").first();
        if (await trigger.count()) {
          await trigger.click({ timeout: 3000 }).catch(() => {});
          await currentPage.waitForTimeout(120);
        }
      }
    }
  };

  const selectSensorInPicker = async ({ currentPage, sensorPickerCard, sensorId }) => {
    const searchInput = sensorPickerCard.getByPlaceholder("Search sensors…").first();
    await searchInput.waitFor({ timeout: 15_000, state: "visible" });
    await searchInput.fill(sensorId);
    await currentPage.waitForTimeout(250);
    await expandSensorNodeCards({ currentPage, sensorPickerCard });

    const checkbox = sensorPickerCard.locator(`input[id="sensor-pick-${sensorId}"]`).first();
    if (!(await checkbox.count())) {
      throw new Error(`Sensor checkbox not found for ${sensorId}`);
    }
    await checkbox.waitFor({ timeout: 15_000, state: "visible" });
    const isChecked = await checkbox.isChecked().catch(() => false);
    if (!isChecked) {
      const rowCard = checkbox.locator("xpath=ancestor::div[contains(@class,'rounded-xl')][1]");
      if (await rowCard.count()) {
        await rowCard.first().click({ timeout: 10_000 });
      } else {
        await checkbox.click({ timeout: 10_000 }).catch(() => {});
      }
      await currentPage.waitForTimeout(180);
    }
  };

  const ensureSelectedSensors = async ({ currentPage, sensorPickerCard, minimum }) => {
    const min = Math.max(1, minimum);
    const countChecked = async () =>
      sensorPickerCard.locator("input[id^='sensor-pick-']:checked").count();

    if ((await countChecked()) >= min) return;
    await expandSensorNodeCards({ currentPage, sensorPickerCard });

    const searchInput = sensorPickerCard.getByPlaceholder("Search sensors…").first();
    if (await searchInput.count()) {
      await searchInput.fill("");
      await currentPage.waitForTimeout(200);
    }

    const checkboxes = sensorPickerCard.locator("input[id^='sensor-pick-']");
    const total = Math.min(await checkboxes.count(), 120);
    for (let idx = 0; idx < total; idx += 1) {
      if ((await countChecked()) >= min) break;
      const checkbox = checkboxes.nth(idx);
      await checkbox.scrollIntoViewIfNeeded().catch(() => {});
      const isVisible = await checkbox.isVisible().catch(() => false);
      if (!isVisible) continue;
      const isChecked = await checkbox.isChecked().catch(() => false);
      if (!isChecked) {
        const rowCard = checkbox.locator("xpath=ancestor::div[contains(@class,'rounded-xl')][1]");
        if (await rowCard.count()) {
          await rowCard.first().click({ timeout: 5000 }).catch(() => {});
        } else {
          await checkbox.click({ timeout: 5000 }).catch(() => {});
        }
        await currentPage.waitForTimeout(120);
      }
    }
  };

  const capture = async ({
    page: currentPage,
    name,
    url,
    beforeScreenshot,
    fullPage = true,
    screenshotLocator = null,
  }) => {
    console.log(`[shot] ${name}: ${url}`);
    await currentPage.goto(url, { waitUntil: "domcontentloaded" });
    await waitForUiToSettle(currentPage);
    if (beforeScreenshot) {
      await beforeScreenshot(currentPage);
      await waitForUiToSettle(currentPage);
    }
    const fileName = `${safeFileName(name)}.png`;
    const fullPath = path.resolve(manualScreenshotsRoot, fileName);
    if (screenshotLocator) {
      const target =
        typeof screenshotLocator === "function"
          ? await screenshotLocator(currentPage)
          : screenshotLocator;
      const resolved = target?.first ? target.first() : target;
      if (resolved) {
        const hasCount = typeof resolved.count === "function";
        const count = hasCount ? await resolved.count().catch(() => 0) : 1;
        if (count > 0) {
          const visible = await resolved
            .waitFor({ timeout: 8_000, state: "visible" })
            .then(() => true)
            .catch(() => false);
          if (visible) {
            await resolved.scrollIntoViewIfNeeded().catch(() => {});
            await currentPage.waitForTimeout(200);
            await resolved.screenshot({ path: fullPath });
          } else {
            console.warn(`[warn] ${name}: screenshot locator was not visible; falling back to viewport capture.`);
            await currentPage.screenshot({ path: fullPath, fullPage });
          }
        } else {
          console.warn(`[warn] ${name}: screenshot locator was not found; falling back to viewport capture.`);
          await currentPage.screenshot({ path: fullPath, fullPage });
        }
      } else {
        await currentPage.screenshot({ path: fullPath, fullPage });
      }
    } else {
      await currentPage.screenshot({ path: fullPath, fullPage });
    }
    manifest.push({ name, url, file: fileName });
  };

  const base = webBase.replace(/\/$/, "");

  await capture({ page: publicPage, name: "root_anon", url: `${base}/` });
  await capture({ page: publicPage, name: "login", url: `${base}/login` });

  await capture({ page, name: "root", url: `${base}/` });
  await capture({ page, name: "nodes", url: `${base}/nodes` });
  await capture({
    page,
    name: "nodes_reorder_modal",
    url: `${base}/nodes`,
    fullPage: false,
    beforeScreenshot: async (currentPage) => {
      const reorderButton = currentPage.getByRole("button", { name: /reorder/i });
      if (!(await reorderButton.count())) return;
      await reorderButton.first().click();
      await currentPage.getByRole("heading", { name: /display order/i }).waitFor({ timeout: 5000 }).catch(() => {});

      const dialog = currentPage.getByRole("dialog", { name: /display order/i });
      if (await dialog.count()) {
        await dialog.evaluate((el) => {
          el.scrollTop = el.scrollHeight;
        });
      }
    },
  });
  if (device) {
    await capture({
      page,
      name: "mobile_nav_open",
      url: `${base}/nodes`,
      beforeScreenshot: async (currentPage) => {
        const openNav = currentPage.getByRole("button", { name: /open navigation/i });
        if (await openNav.count()) {
          await openNav.first().click();
        }
      },
    });

    await capture({
      page,
      name: "mobile_account_menu_open",
      url: `${base}/nodes`,
      beforeScreenshot: async (currentPage) => {
        const account = currentPage.getByRole("button", { name: /account menu/i });
        if (await account.count()) {
          await account.first().click();
        }
      },
    });
  }
  await capture({
    page,
    name: "nodes_adoption_modal",
    url: `${base}/nodes`,
    beforeScreenshot: async (currentPage) => {
      const adoptButton = currentPage.getByRole("button", { name: /adopt node/i });
      if (await adoptButton.count()) {
        await adoptButton.first().click();
        await currentPage
          .getByRole("heading", { name: /adopt node/i })
          .waitFor({ timeout: 5000 })
          .catch(() => {});
      }
    },
  });
  if (firstNodeId) {
    await capture({
      page,
      name: `nodes_${firstNodeId}`,
      url: `${base}/nodes/detail?id=${encodeURIComponent(firstNodeId)}`,
      beforeScreenshot: async (currentPage) => {
        const buffering = currentPage.getByRole("button", { name: /telemetry buffering/i });
        if (await buffering.count()) {
          await buffering.first().click();
          await currentPage.getByText(/spool status/i).first().waitFor({ timeout: 5000 }).catch(() => {});
        }
      },
    });
  }

  await capture({ page, name: "map", url: `${base}/map` });
  await capture({ page, name: "power", url: `${base}/analytics/power` });

  await capture({ page, name: "sensors", url: `${base}/sensors`, fullPage: false });
  await capture({
    page,
    name: "sensors_reorder_modal",
    url: `${base}/sensors`,
    fullPage: false,
    beforeScreenshot: async (currentPage) => {
      const reorderButton = currentPage.getByRole("button", { name: /reorder/i });
      if (!(await reorderButton.count())) return;
      await reorderButton.first().click();
      await currentPage.getByRole("heading", { name: /display order/i }).waitFor({ timeout: 5000 }).catch(() => {});

      const dialog = currentPage.getByRole("dialog", { name: /display order/i });
      if (await dialog.count()) {
        await dialog.evaluate((el) => {
          el.scrollTop = el.scrollHeight;
        });
      }
    },
  });
  await capture({
    page,
    name: "sensors_add_sensor",
    url: firstAgentNodeId ? `${base}/sensors?node=${encodeURIComponent(firstAgentNodeId)}` : `${base}/sensors`,
    fullPage: false,
    beforeScreenshot: async (currentPage) => {
      if (firstAgentNodeName) {
        const nodeHeading = currentPage
          .getByRole("heading", { name: new RegExp(escapeRegex(firstAgentNodeName), "i") })
          .first();
        if (await nodeHeading.count()) {
          await nodeHeading.scrollIntoViewIfNeeded();
          await nodeHeading.click();
        }
      }

      const addSensorButton = currentPage.getByRole("button", { name: /^add sensor/i });
      if (!(await addSensorButton.count())) return;
      await addSensorButton.first().scrollIntoViewIfNeeded();
      await addSensorButton.first().click();
      await currentPage
        .getByRole("heading", { name: /^add sensor/i })
        .first()
        .waitFor({ timeout: 5000 })
        .catch(() => {});

      if (!applyNodeSensor || stubAuth) return;

      const drawer = currentPage.locator("aside").first();
      const presetSelect = drawer
        .locator("label", { hasText: "Preset" })
        .first()
        .locator("xpath=..")
        .locator("select");
      if (await presetSelect.count()) {
        await presetSelect.selectOption("voltage").catch(() => {});
      }

      const nameInput = drawer
        .locator("label", { hasText: "Display name" })
        .first()
        .locator("xpath=..")
        .locator("input");
      if (await nameInput.count()) {
        await nameInput.fill(applyNodeSensorName);
      }

      const intervalInput = drawer
        .locator("label", { hasText: "Interval (seconds)" })
        .first()
        .locator("xpath=..")
        .locator("input");
      if (await intervalInput.count()) {
        await intervalInput.fill("1");
      }

      const applyButton = drawer.getByRole("button", { name: /^apply to node/i });
      if (!(await applyButton.count())) return;
      await applyButton.first().click();

      // Wait for any apply feedback (applied / saved offline / error message).
      await waitForTextMatch(
        drawer,
        /(sensor config applied to node|sensor config saved|node agent|spi disabled|not detected)/i,
        15_000,
      );
    },
  });
  if (firstWeatherStationNodeId) {
    await capture({
      page,
      name: "sensors_add_sensor_ws2902",
      url: `${base}/sensors?node=${encodeURIComponent(firstWeatherStationNodeId)}`,
      fullPage: false,
      beforeScreenshot: async (currentPage) => {
        if (firstWeatherStationNodeName) {
          const nodeHeading = currentPage
            .getByRole("heading", { name: new RegExp(escapeRegex(firstWeatherStationNodeName), "i") })
            .first();
          if (await nodeHeading.count()) {
            await nodeHeading.scrollIntoViewIfNeeded();
            await nodeHeading.click();
          }
        }

        const addSensorButton = currentPage.getByRole("button", { name: /^add sensor/i });
        if (!(await addSensorButton.count())) return;
        await addSensorButton.first().scrollIntoViewIfNeeded();
        await addSensorButton.first().click();
        await currentPage
          .getByRole("heading", { name: /^add sensor/i })
          .first()
          .waitFor({ timeout: 5000 })
          .catch(() => {});

        const wsModeButton = currentPage.getByRole("button", { name: /^ws-2902$/i }).first();
        if (await wsModeButton.count()) {
          await wsModeButton.waitFor({ timeout: 5000 }).catch(() => {});
        }

        const uploadFieldLabel = currentPage.getByText(/upload field/i).first();
        if (await uploadFieldLabel.count()) {
          await uploadFieldLabel.waitFor({ timeout: 5000 }).catch(() => {});
        }
      },
    });
  }
  if (ws2902CustomSensorId) {
    await capture({
      page,
      name: "sensors_ws2902_custom",
      url: `${base}/sensors/detail?id=${encodeURIComponent(ws2902CustomSensorId)}`,
    });
  }
  if (applyNodeSensor && firstAgentNodeId && !stubAuth) {
    await capture({
      page,
      name: "sensors_node_after_apply",
      url: `${base}/sensors?node=${encodeURIComponent(firstAgentNodeId)}`,
      fullPage: false,
      beforeScreenshot: async (currentPage) => {
        const row = currentPage.getByRole("row", { name: new RegExp(applyNodeSensorName, "i") }).first();
        await row.waitFor({ timeout: 15_000 }).catch(() => {});
        await waitForTextMatch(row, /\bV\b/, 20_000);
      },
    });
  }
  if (firstAgentNodeId && coreNodeId) {
    await capture({
      page,
      name: "sensors_core",
      url: `${base}/sensors?node=${encodeURIComponent(coreNodeId)}`,
      fullPage: false,
    });
  }
  if (sensorDetailId) {
    await capture({
      page,
      name: `sensors_${sensorDetailId}`,
      url: `${base}/sensors/detail?id=${encodeURIComponent(sensorDetailId)}`,
    });
  }

  await capture({ page, name: "users", url: `${base}/users` });

  await capture({ page, name: "schedules", url: `${base}/schedules` });
  await capture({
    page,
    name: "schedules_new",
    url: `${base}/schedules`,
    beforeScreenshot: async (currentPage) => {
      const newButton = currentPage.getByRole("button", { name: /new schedule/i });
      if (await newButton.count()) {
        await newButton.first().click();
        await currentPage.getByRole("heading", { name: /create schedule/i }).waitFor();
      }
    },
  });
  await capture({
    page,
    name: "schedules_edit",
    url: `${base}/schedules`,
    beforeScreenshot: async (currentPage) => {
      const editButtons = currentPage.getByRole("button", { name: /^edit$/i });
      if (await editButtons.count()) {
        await editButtons.first().click();
        await currentPage.getByRole("heading", { name: /edit schedule/i }).waitFor().catch(() => {});
      }
    },
  });

  const trendsUrl = `${base}/analytics/trends`;
  await capture({ page, name: "trends", url: trendsUrl });
  if (relatedFocusSensorId) {
    await capture({
      page,
      name: "trends_chart_settings_7d",
      url: trendsUrl,
      fullPage: false,
      screenshotLocator: (currentPage) => cardByTitle(currentPage, /chart settings/i),
      beforeScreenshot: async (currentPage) => {
        const chartSettingsCard = cardByTitle(currentPage, /chart settings/i);
        await chartSettingsCard.waitFor({ timeout: 20_000 });
        await ensureCardOpen(chartSettingsCard);
        const rangeSelect = chartSettingsCard.getByRole("combobox", { name: /^range$/i }).first();
        if (await rangeSelect.count()) {
          await rangeSelect.selectOption(String(relatedRangeHours)).catch(() => {});
        }
        const intervalSelect = chartSettingsCard.getByRole("combobox", { name: /^interval$/i }).first();
        if (relatedIntervalSeconds && (await intervalSelect.count())) {
          await intervalSelect.selectOption(String(relatedIntervalSeconds)).catch(() => {});
        }
      },
    });

    await capture({
      page,
      name: "trends_related_sensors_reservoir_depth_7d_wind_rain",
      url: trendsUrl,
      fullPage: false,
      screenshotLocator: (currentPage) => currentPage.getByTestId("relationship-finder-panel"),
      beforeScreenshot: async (currentPage) => {
        try {
          if (!ws2902RainDailySensorId || !ws2902WindDirectionSensorId) {
            console.warn("[warn] Unable to locate WS-2902 rain/wind sensors; skipping related evidence capture.");
            return;
          }

          const clearButton = currentPage.getByRole("button", { name: /^clear$/i });
          if (await clearButton.count()) {
            await clearButton.first().click().catch(() => {});
            await currentPage.waitForTimeout(250);
          }

          const chartSettingsCard = cardByTitle(currentPage, /chart settings/i);
          const sensorPickerCard = cardByTitle(currentPage, /sensor picker/i);
          await chartSettingsCard.waitFor({ timeout: 20_000 });
          await sensorPickerCard.waitFor({ timeout: 20_000 });
          await ensureCardOpen(chartSettingsCard);
          await ensureCardOpen(sensorPickerCard);

          const rangeSelect = chartSettingsCard.getByRole("combobox", { name: /^range$/i }).first();
          if (await rangeSelect.count()) {
            await rangeSelect.selectOption(String(relatedRangeHours)).catch(() => {});
          }
          const intervalSelect = chartSettingsCard.getByRole("combobox", { name: /^interval$/i }).first();
          if (relatedIntervalSeconds && (await intervalSelect.count())) {
            await intervalSelect.selectOption(String(relatedIntervalSeconds)).catch(() => {});
          }

          await selectSensorInPicker({
            currentPage,
            sensorPickerCard,
            sensorId: relatedFocusSensorId,
          });
          await ensureSelectedSensors({ currentPage, sensorPickerCard, minimum: 1 }).catch(() => {});

          const panel = currentPage.getByTestId("relationship-finder-panel").first();
          await panel.waitFor({ timeout: relatedTimeoutMs });
          await panel.scrollIntoViewIfNeeded().catch(() => {});

          const similarityTab = panel.getByRole("tab", { name: /^similarity$/i }).first();
          if (await similarityTab.count()) {
            await similarityTab.click({ timeout: 5000 }).catch(() => {});
            await currentPage.waitForTimeout(250);
          }

          const advancedTab = panel.getByRole("tab", { name: /^advanced$/i }).first();
          const advancedButton = panel.getByRole("button", { name: /^advanced$/i }).first();
          if (await advancedTab.count()) {
            await advancedTab.click({ timeout: 5000 }).catch(() => {});
            await currentPage.waitForTimeout(250);
          } else if (await advancedButton.count()) {
            await advancedButton.click({ timeout: 5000 }).catch(() => {});
            await currentPage.waitForTimeout(250);
          }

          const includeWeak = panel.getByRole("checkbox", { name: /include weak evidence/i }).first();
          if (await includeWeak.count()) {
            await includeWeak.check({ timeout: 5000 }).catch(() => {});
          }

          const includeDelta = panel
            .getByRole("checkbox", { name: /include Δ corr signal/i })
            .first();
          if (await includeDelta.count()) {
            await includeDelta.check({ timeout: 5000 }).catch(() => {});
          }

          const maxResultsInput = panel.getByLabel(/max results/i).first();
          if (await maxResultsInput.count()) {
            await maxResultsInput.fill("300").catch(() => {});
            await maxResultsInput.blur().catch(() => {});
          }

          const zThresholdInput = panel.getByLabel(/z threshold/i).first();
          if (await zThresholdInput.count()) {
            await zThresholdInput.fill("3.5").catch(() => {});
            await zThresholdInput.blur().catch(() => {});
          }

          const scopeSelect = panel.getByRole("combobox", { name: /^scope$/i }).first();
          if (await scopeSelect.count()) {
            await scopeSelect.selectOption(relatedScope).catch(() => {});
          }

          const focusSelect = panel.getByRole("combobox", { name: /^focus sensor$/i }).first();
          if (await focusSelect.count()) {
            await focusSelect.selectOption(relatedFocusSensorId).catch(() => {});
          }

          const statusCompleted = panel
            .locator("[data-slot='badge']")
            .filter({ hasText: /^completed$/i })
            .first();
          const statusInProgress = panel
            .locator("[data-slot='badge']")
            .filter({ hasText: /^(pending|running)$/i })
            .first();
          const statusFailed = panel
            .locator("[data-slot='badge']")
            .filter({ hasText: /^failed$/i })
            .first();

          const resultsGrid = currentPage.getByTestId("relationship-finder-results").first();
          const hadResultsBefore = (await resultsGrid.count().catch(() => 0)) > 0;

          const runAdvanced = panel.getByRole("button", { name: /configure scoring/i }).first();
          const runSimple = panel.getByRole("button", { name: /find related sensors/i }).first();
          const runButton = (await runAdvanced.count()) ? runAdvanced : runSimple;
          if (!(await runButton.count())) throw new Error("Run related sensors button not found.");
          await runButton.click({ timeout: 10_000 });

          if (hadResultsBefore) {
            await resultsGrid.waitFor({ state: "hidden", timeout: 20_000 }).catch(() => {});
          }
          await statusCompleted.waitFor({ state: "hidden", timeout: 20_000 }).catch(() => {});
          await Promise.race([
            statusInProgress.waitFor({ timeout: 20_000 }),
            statusCompleted.waitFor({ timeout: 20_000 }),
            statusFailed.waitFor({ timeout: 20_000 }),
          ]).catch(() => {});
          await statusCompleted.waitFor({ timeout: relatedTimeoutMs });
          await statusFailed.waitFor({ state: "hidden", timeout: 1000 }).catch(() => {});

          await resultsGrid.waitFor({ timeout: relatedTimeoutMs });
          await resultsGrid.locator("[id^='result-']").first().waitFor({ timeout: relatedTimeoutMs });

          const ensureCandidateRendered = async (sensorId, maxAttempts) => {
            const row = currentPage.locator(`#result-${sensorId}`).first();
            for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
              const found = (await row.count().catch(() => 0)) > 0;
              if (found) return true;
              const showMore = resultsGrid.getByRole("button", { name: /show more/i }).first();
              if (!(await showMore.count())) break;
              await showMore.scrollIntoViewIfNeeded().catch(() => {});
              await showMore.click({ timeout: 5000 }).catch(() => {});
              await currentPage.waitForTimeout(250);
            }
            return false;
          };

          await ensureCandidateRendered(ws2902RainDailySensorId, 30);
          await ensureCandidateRendered(ws2902WindDirectionSensorId, 30);

          const rainRow = currentPage.locator(`#result-${ws2902RainDailySensorId}`).first();
          const windRow = currentPage.locator(`#result-${ws2902WindDirectionSensorId}`).first();

          if (await rainRow.count()) {
            await rainRow.scrollIntoViewIfNeeded().catch(() => {});
            await currentPage.waitForTimeout(250);
            const fileName = "trends_related_sensors_reservoir_depth_7d_rain_daily.png";
            const fullPath = path.resolve(manualScreenshotsRoot, fileName);
            await rainRow.screenshot({ path: fullPath }).catch(async () => {
              await currentPage.screenshot({ path: fullPath, fullPage: false });
            });
            manifest.push({
              name: "trends_related_sensors_reservoir_depth_7d_rain_daily",
              url: trendsUrl,
              file: fileName,
            });
          }

          if (await windRow.count()) {
            await windRow.scrollIntoViewIfNeeded().catch(() => {});
            await currentPage.waitForTimeout(250);
            const fileName = "trends_related_sensors_reservoir_depth_7d_wind_direction.png";
            const fullPath = path.resolve(manualScreenshotsRoot, fileName);
            await windRow.screenshot({ path: fullPath }).catch(async () => {
              await currentPage.screenshot({ path: fullPath, fullPage: false });
            });
            manifest.push({
              name: "trends_related_sensors_reservoir_depth_7d_wind_direction",
              url: trendsUrl,
              file: fileName,
            });
          }
        } catch (err) {
          console.warn(`[warn] trends_related_sensors_reservoir_depth_7d_wind_rain setup failed: ${err?.message || err}`);
        }
      },
    });
  }
  if (!stubAuth) {
    await capture({
      page,
      name: "trends_short_range",
      url: `${base}/analytics/trends`,
      fullPage: false,
      beforeScreenshot: async (currentPage) => {
        try {
          const chartSettingsCard = cardByTitle(currentPage, /chart settings/i);
          await chartSettingsCard.waitFor({ timeout: 15_000 });
          await ensureCardOpen(chartSettingsCard);

          const rangeSelect = chartSettingsCard.getByRole("combobox", { name: /^range$/i }).first();
          if (await rangeSelect.count()) {
            await rangeSelect.selectOption(String(10 / 60));
          }

          const intervalSelect = chartSettingsCard.getByRole("combobox", { name: /^interval$/i }).first();
          if (await intervalSelect.count()) {
            await intervalSelect.selectOption("1");
          }

          await currentPage.waitForTimeout(300);
        } catch (err) {
          console.warn("[warn] trends_short_range setup failed:", err);
        }
      },
    });
  }
  if (!stubAuth) {
    const setupRelationshipPanel = async (currentPage, strategy) => {
      const clearButton = currentPage.getByRole("button", { name: /^clear$/i });
      if (await clearButton.count()) {
        await clearButton.first().click().catch(() => {});
        await currentPage.waitForTimeout(250);
      }

      const chartSettingsCard = cardByTitle(currentPage, /chart settings/i);
      const sensorPickerCard = cardByTitle(currentPage, /sensor picker/i);
      await chartSettingsCard.waitFor({ timeout: 15_000 });
      await sensorPickerCard.waitFor({ timeout: 15_000 });
      await ensureCardOpen(chartSettingsCard);
      await ensureCardOpen(sensorPickerCard);

      const rangeSelect = chartSettingsCard.getByRole("combobox", { name: /^range$/i }).first();
      const intervalSelect = chartSettingsCard.getByRole("combobox", { name: /^interval$/i }).first();
      if (await rangeSelect.count()) await rangeSelect.selectOption("24").catch(() => {});
      if (await intervalSelect.count()) await intervalSelect.selectOption("300").catch(() => {});

      const uniqueIds = Array.from(new Set(trendsExampleSensorIds)).slice(0, 2);
      for (const sensorId of uniqueIds) {
        await selectSensorInPicker({ currentPage, sensorPickerCard, sensorId }).catch(() => {});
      }
      await ensureSelectedSensors({ currentPage, sensorPickerCard, minimum: 2 }).catch(() => {});

      const panel = currentPage.getByTestId("relationship-finder-panel").first();
      await panel.waitFor({ timeout: 20_000 });
      await panel.scrollIntoViewIfNeeded().catch(() => {});

      const tab = panel.getByRole("tab", { name: new RegExp(`^${escapeRegex(strategy)}$`, "i") }).first();
      if (await tab.count()) {
        await tab.click({ timeout: 5000 }).catch(() => {});
        await currentPage.waitForTimeout(250);
      }

      const keyToggle = panel.getByRole("button", { name: /how it works/i }).first();
      if (await keyToggle.count()) {
        const statsLine = panel.getByText(/Stats:/i).first();
        const hasStats = await statsLine.isVisible().catch(() => false);
        if (!hasStats) {
          await keyToggle.click({ timeout: 5000 }).catch(() => {});
        }
      }

      await currentPage.waitForTimeout(300);
      return { panel };
    };

    await capture({
      page,
      name: "trends_related_sensors_large_scan",
      url: `${base}/analytics/trends`,
      fullPage: false,
      screenshotLocator: (currentPage) => currentPage.getByTestId("relationship-finder-panel"),
      beforeScreenshot: async (currentPage) => {
        try {
          await setupRelationshipPanel(currentPage, "Similarity");
        } catch (err) {
          console.warn(`[warn] trends_related_sensors_large_scan setup failed: ${err?.message || err}`);
        }
      },
    });

    await capture({
      page,
      name: "trends_related_sensors_scanning",
      url: `${base}/analytics/trends`,
      fullPage: false,
      screenshotLocator: (currentPage) => currentPage.getByTestId("relationship-finder-panel"),
      beforeScreenshot: async (currentPage) => {
        try {
          const { panel } = await setupRelationshipPanel(currentPage, "Correlation");
          // Keep a stable, meaningful evidence region visible even when analysis isn't run.
          await panel.getByText(/p.*q.*n_eff/i).first().waitFor({ timeout: 5000 }).catch(() => {});
        } catch (err) {
          console.warn(`[warn] trends_related_sensors_scanning setup failed: ${err?.message || err}`);
        }
      },
    });
  }
  if (!stubAuth) {
    await capture({
      page,
      name: "trends_cooccurrence",
      url: `${base}/analytics/trends`,
      fullPage: false,
      screenshotLocator: (currentPage) => currentPage.getByTestId("relationship-finder-panel"),
      beforeScreenshot: async (currentPage) => {
        try {
          const panel = currentPage.getByTestId("relationship-finder-panel").first();
          await panel.waitFor({ timeout: 20_000 });
          await panel.scrollIntoViewIfNeeded().catch(() => {});

          const tab = panel.getByRole("tab", { name: /^co-occurrence$/i }).first();
          if (await tab.count()) {
            await tab.click({ timeout: 5000 }).catch(() => {});
          }

          const keyToggle = panel.getByRole("button", { name: /how it works/i }).first();
          if (await keyToggle.count()) {
            await keyToggle.click({ timeout: 5000 }).catch(() => {});
          }
          await currentPage.waitForTimeout(300);
        } catch (err) {
          console.warn(`[warn] trends_cooccurrence setup failed: ${err?.message || err}`);
        }
      },
    });
  }
  await capture({ page, name: "analytics", url: `${base}/analytics` });
  await capture({ page, name: "backups", url: `${base}/backups` });
  await capture({ page, name: "provisioning", url: `${base}/provisioning` });
  await capture({ page, name: "deployment", url: `${base}/deployment` });
  await capture({ page, name: "setup", url: `${base}/setup` });
  await capture({ page, name: "sim_lab", url: `${base}/sim-lab` });
  await capture({ page, name: "connection", url: `${base}/connection` });

  await writeFile(path.resolve(manualScreenshotsRoot, "manifest.json"), JSON.stringify(manifest, null, 2));

  await publicContext.close();
  await authedContext.close();
  await browser.close();

  console.log(`\nSaved screenshots to ${manualScreenshotsRoot}`);
} finally {
  await cleanup();
}
