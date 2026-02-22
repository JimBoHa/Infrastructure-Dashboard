#!/usr/bin/env node
import { mkdir, copyFile, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const dirname = path.dirname(fileURLToPath(import.meta.url));
const dashboardWebDir = path.resolve(dirname, "..");

const source = path.resolve(
  dashboardWebDir,
  "node_modules",
  "maplibre-gl",
  "dist",
  "maplibre-gl-csp-worker.js",
);
const destDir = path.resolve(dashboardWebDir, "public", "vendor");
const dest = path.resolve(destDir, "maplibre-gl-csp-worker.js");

const exists = async (filePath) => {
  try {
    await stat(filePath);
    return true;
  } catch {
    return false;
  }
};

try {
  if (!(await exists(source))) {
    console.warn(`[maplibre] worker not found at ${source}; skipping copy`);
    process.exit(0);
  }

  await mkdir(destDir, { recursive: true });
  await copyFile(source, dest);
  console.log(`[maplibre] copied CSP worker to ${dest}`);
} catch (error) {
  console.error("[maplibre] failed to copy worker:", error);
  process.exit(1);
}

