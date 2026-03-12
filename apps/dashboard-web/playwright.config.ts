import { defineConfig, devices } from "@playwright/test";

const managedBaseURL = "http://127.0.0.1:3005";
const baseURL =
  process.env.FARM_PLAYWRIGHT_BASE_URL ||
  process.env.FARM_SCREENSHOT_BASE_URL ||
  managedBaseURL;
const useManagedWebServer =
  !process.env.FARM_PLAYWRIGHT_BASE_URL && !process.env.FARM_SCREENSHOT_BASE_URL;

export default defineConfig({
  testDir: "./playwright",
  fullyParallel: true,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: [["line"]],
  webServer: useManagedWebServer
    ? {
        command: "npm run dev -- --hostname 127.0.0.1 --port 3005",
        url: managedBaseURL,
        reuseExistingServer: !process.env.CI,
        timeout: 120_000,
        env: {
          ...process.env,
          FARM_CORE_API_BASE:
            process.env.FARM_CORE_API_BASE || process.env.NEXT_PUBLIC_API_BASE || "http://127.0.0.1:8000",
          NEXT_PUBLIC_API_BASE: process.env.NEXT_PUBLIC_API_BASE || "http://127.0.0.1:8000",
        },
      }
    : undefined,
  use: {
    baseURL,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium-desktop",
      use: {
        ...devices["Desktop Chrome"],
        browserName: "chromium",
      },
    },
    {
      name: "webkit-mobile",
      use: {
        ...devices["iPhone 13"],
        browserName: "webkit",
      },
    },
    {
      name: "chromium-mobile",
      use: {
        ...devices["iPhone 13"],
        browserName: "chromium",
      },
    },
  ],
});
