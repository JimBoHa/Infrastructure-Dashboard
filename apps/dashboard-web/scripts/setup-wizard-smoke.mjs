#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
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

const { args } = parseArgs(process.argv);

const baseUrl = (args.get("base-url") || "http://127.0.0.1:8800").replace(/\/$/, "");
const installRoot = args.get("install-root");
const dataRoot = args.get("data-root");
const backupRoot = args.get("backup-root");
const logsRoot = args.get("logs-root");
const upgradeBundlePath = args.get("upgrade-bundle-path");

const expectedInstallVersion = args.get("expected-install-version") || "0.0.0-test";
const expectedUpgradeVersion = args.get("expected-upgrade-version") || "0.0.1-test";

if (!installRoot || !dataRoot || !backupRoot) {
  console.error(
    "setup-wizard-smoke: missing required args --install-root, --data-root, --backup-root"
  );
  process.exit(2);
}
if (!upgradeBundlePath) {
  console.error("setup-wizard-smoke: missing required arg --upgrade-bundle-path");
  process.exit(2);
}

const artifactsRoot = path.resolve(
  repoRoot,
  "reports",
  "e2e-setup-smoke",
  timestampSlug()
);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const fetchJson = async (url, init) => {
  const response = await fetch(url, init);
  const text = await response.text();
  let parsed = null;
  if (text) {
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = text;
    }
  }
  if (!response.ok) {
    throw new Error(`Request failed ${response.status}: ${url}\n${text}`);
  }
  return parsed;
};

const pollFor = async (label, fn, { timeoutMs = 90_000, intervalMs = 1000 } = {}) => {
  const started = Date.now();
  let lastErr;
  while (Date.now() - started < timeoutMs) {
    try {
      const result = await fn();
      if (result) return result;
    } catch (err) {
      lastErr = err;
    }
    await sleep(intervalMs);
  }
  if (lastErr) throw lastErr;
  throw new Error(`Timed out waiting for ${label}`);
};

const waitForHealthOk = async () => {
  await pollFor("health ok", async () => {
    const payload = await fetchJson(`${baseUrl}/api/health-report`);
    const report = payload.report || payload;
    if (!report?.core_api || !report?.dashboard || !report?.mqtt || !report?.database || !report?.redis) {
      return false;
    }
    const ok =
      report.core_api.status === "ok" &&
      report.dashboard.status === "ok" &&
      report.mqtt.status === "ok" &&
      report.database.status === "ok" &&
      report.redis.status === "ok";
    return ok ? report : false;
  });
};

const expectVersion = async (version) => {
  await pollFor(`version ${version}`, async () => {
    const payload = await fetchJson(`${baseUrl}/api/status`);
    const state = payload.result || payload;
    if (state?.current_version === version) return state;
    return false;
  });
};

let browser;
let context;

const screenshot = async (page, name) => {
  await fs.mkdir(artifactsRoot, { recursive: true });
  await page.screenshot({ path: path.join(artifactsRoot, `${name}.png`), fullPage: true });
};

try {
  browser = await chromium.launch({ headless: true });
  context = await browser.newContext();
  const page = await context.newPage();

  await page.goto(baseUrl, { waitUntil: "domcontentloaded", timeout: 30_000 });
  await page.waitForSelector("text=Production Setup Wizard", { timeout: 30_000 });

  // Configure step
  await page.click("#next-step"); // Welcome -> Configure
  await page.waitForSelector('input[name="bundle_path"]', { timeout: 10_000 });

  const bundlePathValue = await page.inputValue('input[name="bundle_path"]');
  if (!bundlePathValue || !bundlePathValue.endsWith(".dmg")) {
    throw new Error(`bundle_path was not auto-detected (value=${bundlePathValue || "empty"})`);
  }

  const advancedHidden = !(await page.isVisible('input[name="logs_root"]'));
  if (!advancedHidden) {
    throw new Error("Advanced fields were visible by default (expected hidden)");
  }

  await page.click("#toggle-advanced");
  await page.waitForSelector('input[name="logs_root"]', { timeout: 10_000 });

  const farmctlPathValue = (await page.inputValue('input[name="farmctl_path"]')).trim();
  if (!farmctlPathValue || farmctlPathValue === "farmctl") {
    throw new Error(`farmctl_path was not auto-detected (value=${farmctlPathValue || "empty"})`);
  }
  if (!farmctlPathValue.endsWith("farmctl")) {
    throw new Error(`farmctl_path did not look like a farmctl binary (value=${farmctlPathValue})`);
  }

  await page.fill('input[name="install_root"]', installRoot);
  await page.fill('input[name="data_root"]', dataRoot);
  await page.fill('input[name="backup_root"]', backupRoot);
  if (logsRoot) {
    await page.fill('input[name="logs_root"]', logsRoot);
  } else {
    await page.click("#toggle-advanced"); // leave advanced hidden for minimal-prompt path
  }

  // Preflight step
  await page.click("#next-step"); // Configure -> Preflight
  await page.waitForSelector("#run-preflight", { timeout: 10_000 });
  await page.click("#run-preflight");
  await page.waitForSelector("#preflight-results .check", { timeout: 30_000 });

  // Plan step (also persists config via /api/plan)
  await page.click("#next-step");
  await page.waitForSelector("#generate-plan", { timeout: 10_000 });
  await page.click("#generate-plan");
  await page.waitForSelector("#plan-output pre", { timeout: 30_000 });
  const hasPlanWarn = await page.isVisible("#plan-output .badge.warn");
  if (hasPlanWarn) {
    throw new Error("Launch plan showed warnings on a clean install (expected none)");
  }

  // Operations step
  await page.click("#next-step");
  await page.waitForSelector("#run-install", { timeout: 10_000 });

  await page.click("#run-install");
  await expectVersion(expectedInstallVersion);
  await waitForHealthOk();

  // Upgrade (override bundle path)
  await page.click("#prev-step");
  await page.click("#prev-step");
  await page.click("#prev-step"); // back to Configure
  await page.fill('input[name="bundle_path"]', upgradeBundlePath);
  await page.click("#next-step"); // Preflight
  await page.click("#next-step"); // Plan
  await page.click("#generate-plan");
  await page.click("#next-step"); // Operations

  await page.click("#run-upgrade");
  await expectVersion(expectedUpgradeVersion);
  await waitForHealthOk();

  // Rollback
  await page.click("#run-rollback");
  await expectVersion(expectedInstallVersion);
  await waitForHealthOk();

  console.log("setup-wizard-smoke: PASS");
  process.exit(0);
} catch (err) {
  console.error(`setup-wizard-smoke: FAIL (${err?.message || err})`);
  try {
    if (context) {
      const pages = context.pages();
      if (pages.length) {
        await screenshot(pages[0], "failure");
      }
    }
  } catch {
    // ignore screenshot failure
  }
  if (artifactsRoot) {
    console.error(`Artifacts: ${artifactsRoot}`);
  }
  process.exit(1);
} finally {
  try {
    if (context) await context.close();
    if (browser) await browser.close();
  } catch {
    // ignore cleanup errors
  }
}
