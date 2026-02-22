import { expect, test } from "@playwright/test";

import { installStubApi } from "./stubApi";

const ISO_BASE = "2026-01-09T00:00:00.000Z";

function points({
  startMs,
  stepMs,
  count,
  base,
  noise = 0,
  spikeEvery = 0,
  spikeDelta = 0,
  dipEvery = 0,
  dipDelta = 0,
}: {
  startMs: number;
  stepMs: number;
  count: number;
  base: number;
  noise?: number;
  spikeEvery?: number;
  spikeDelta?: number;
  dipEvery?: number;
  dipDelta?: number;
}) {
  const pts: Array<{ timestamp: string; value: number; samples: number }> = [];
  for (let i = 0; i < count; i += 1) {
    let value = base;
    if (noise) value += Math.sin(i / 3) * noise;
    if (spikeEvery && i > 0 && i % spikeEvery === 0) value += spikeDelta;
    if (dipEvery && i > 0 && i % dipEvery === 0) value -= dipDelta;
    pts.push({
      timestamp: new Date(startMs + i * stepMs).toISOString(),
      value,
      samples: 1,
    });
  }
  return pts;
}

test.describe("Power voltage quality", () => {
  test("shows AC voltage quality for Emporia and DC voltage quality for Renogy", async ({ page }) => {
    const now = new Date(ISO_BASE).getTime();
    const stepMs = 5 * 60 * 1000;

    const EMPORIA_NODE_ID = "playwright-emporia-node";
    const RENOGY_NODE_ID = "playwright-renogy-node";

    const emporia = {
      node: {
        id: EMPORIA_NODE_ID,
        name: "Emporia Main Panel",
        status: "online",
        uptime_seconds: 12345,
        cpu_percent: 1.2,
        storage_used_bytes: 1024,
        mac_eth: null,
        mac_wifi: null,
        ip_last: "192.168.1.20",
        last_seen: ISO_BASE,
        created_at: ISO_BASE,
        config: {},
      },
      sensors: [
        {
          sensor_id: "emporia-mains-power",
          node_id: EMPORIA_NODE_ID,
          name: "Mains Power",
          type: "power",
          unit: "W",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 4200,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "emporia_cloud", metric: "mains_power_w" },
        },
        {
          sensor_id: "emporia-mains-l1-v",
          node_id: EMPORIA_NODE_ID,
          name: "Mains L1 Voltage",
          type: "voltage",
          unit: "V",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 118.5,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: {
            source: "emporia_cloud",
            metric: "channel_voltage_v",
            is_mains: true,
            channel_key: "Mains_A",
            channel_name: "Mains_A",
          },
        },
        {
          sensor_id: "emporia-mains-l2-v",
          node_id: EMPORIA_NODE_ID,
          name: "Mains L2 Voltage",
          type: "voltage",
          unit: "V",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 119.2,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: {
            source: "emporia_cloud",
            metric: "channel_voltage_v",
            is_mains: true,
            channel_key: "Mains_B",
            channel_name: "Mains_B",
          },
        },
        {
          sensor_id: "emporia-mains-l1-a",
          node_id: EMPORIA_NODE_ID,
          name: "Mains L1 Current",
          type: "current",
          unit: "A",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 12.3,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "emporia_cloud", metric: "channel_current_a", is_mains: true, channel_key: "Mains_A" },
        },
        {
          sensor_id: "emporia-mains-l2-a",
          node_id: EMPORIA_NODE_ID,
          name: "Mains L2 Current",
          type: "current",
          unit: "A",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 9.7,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "emporia_cloud", metric: "channel_current_a", is_mains: true, channel_key: "Mains_B" },
        },
        {
          sensor_id: "emporia-circuit-power",
          node_id: EMPORIA_NODE_ID,
          name: "Closet Telecom",
          type: "power",
          unit: "W",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 240,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "emporia_cloud", metric: "channel_power_w", channel_key: "458761:6", channel_name: "Closet Telecom" },
        },
      ],
    };

    const renogy = {
      node: {
        id: RENOGY_NODE_ID,
        name: "Renogy Node",
        status: "online",
        uptime_seconds: 23456,
        cpu_percent: 0.9,
        storage_used_bytes: 2048,
        mac_eth: null,
        mac_wifi: null,
        ip_last: "192.168.1.30",
        last_seen: ISO_BASE,
        created_at: ISO_BASE,
        config: {},
      },
      sensors: [
        {
          sensor_id: "renogy-pv-power",
          node_id: RENOGY_NODE_ID,
          name: "PV Power",
          type: "power",
          unit: "W",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 120,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "renogy_bt2", metric: "pv_power_w" },
        },
        {
          sensor_id: "renogy-load-power",
          node_id: RENOGY_NODE_ID,
          name: "Load Power",
          type: "power",
          unit: "W",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 80,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "renogy_bt2", metric: "load_power_w" },
        },
        {
          sensor_id: "renogy-batt-v",
          node_id: RENOGY_NODE_ID,
          name: "Battery Voltage",
          type: "voltage",
          unit: "V",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 13.2,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "renogy_bt2", metric: "battery_voltage_v" },
        },
        {
          sensor_id: "renogy-pv-v",
          node_id: RENOGY_NODE_ID,
          name: "PV Voltage",
          type: "voltage",
          unit: "V",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 18.7,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "renogy_bt2", metric: "pv_voltage_v" },
        },
        {
          sensor_id: "renogy-load-v",
          node_id: RENOGY_NODE_ID,
          name: "Load Voltage",
          type: "voltage",
          unit: "V",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 12.8,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "renogy_bt2", metric: "load_voltage_v" },
        },
        {
          sensor_id: "renogy-batt-a",
          node_id: RENOGY_NODE_ID,
          name: "Battery Current",
          type: "current",
          unit: "A",
          interval_seconds: 60,
          rolling_avg_seconds: 0,
          latest_value: 5.2,
          status: "online",
          location: null,
          created_at: ISO_BASE,
          config: { source: "renogy_bt2", metric: "battery_current_a" },
        },
      ],
    };

    await installStubApi(page, {
      jsonByPath: {
        "/api/nodes": [emporia.node, renogy.node],
        "/api/sensors": [...emporia.sensors, ...renogy.sensors],
        "/api/metrics/query": {
          series: [
            {
              sensor_id: "emporia-mains-power",
              sensor_name: "Mains Power",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 3800, noise: 400 }),
            },
            {
              sensor_id: "emporia-mains-l1-v",
              sensor_name: "Mains L1 Voltage",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 118.5, noise: 0.7, dipEvery: 13, dipDelta: 6 }),
            },
            {
              sensor_id: "emporia-mains-l2-v",
              sensor_name: "Mains L2 Voltage",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 119.2, noise: 0.6, spikeEvery: 17, spikeDelta: 7 }),
            },
            {
              sensor_id: "emporia-mains-l1-a",
              sensor_name: "Mains L1 Current",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 12.3, noise: 1.1 }),
            },
            {
              sensor_id: "emporia-mains-l2-a",
              sensor_name: "Mains L2 Current",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 9.7, noise: 0.9 }),
            },
            {
              sensor_id: "emporia-circuit-power",
              sensor_name: "Closet Telecom",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 240, noise: 30 }),
            },
            {
              sensor_id: "renogy-pv-power",
              sensor_name: "PV Power",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 140, noise: 25 }),
            },
            {
              sensor_id: "renogy-load-power",
              sensor_name: "Load Power",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 80, noise: 10 }),
            },
            {
              sensor_id: "renogy-batt-v",
              sensor_name: "Battery Voltage",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 13.2, noise: 0.15, dipEvery: 10, dipDelta: 0.7 }),
            },
            {
              sensor_id: "renogy-pv-v",
              sensor_name: "PV Voltage",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 18.7, noise: 0.3, spikeEvery: 9, spikeDelta: 0.8 }),
            },
            {
              sensor_id: "renogy-load-v",
              sensor_name: "Load Voltage",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 12.8, noise: 0.2 }),
            },
            {
              sensor_id: "renogy-batt-a",
              sensor_name: "Battery Current",
              points: points({ startMs: now - stepMs * 40, stepMs, count: 40, base: 5.2, noise: 0.8 }),
            },
          ],
        },
      },
    });

    await page.goto("/power", { waitUntil: "domcontentloaded" });

    await expect(page.getByRole("heading", { name: "Power", exact: true })).toBeVisible();
    await expect(page.getByText("AC voltage quality", { exact: true })).toBeVisible();

    const selector = page.getByRole("combobox").first();
    await selector.selectOption({ label: "Renogy Node (renogy)" });
    await expect(page.getByRole("heading", { name: "DC voltage quality (battery)" })).toBeVisible();

    await page.getByRole("button", { name: "PV" }).click();
    await expect(page.getByText("DC voltage quality (pv)", { exact: true })).toBeVisible();

    await page.getByRole("button", { name: "Load" }).click();
    await expect(page.getByText("DC voltage quality (load)", { exact: true })).toBeVisible();
  });
});
