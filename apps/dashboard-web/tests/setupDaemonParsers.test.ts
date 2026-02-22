import { describe, expect, it } from "vitest";

import {
  asBoolValue,
  asIntValue,
  asNullableString,
  asStringValue,
  parseControllerRuntimeConfig,
  parseSetupDaemonConfig,
  parseSetupDaemonLocalIp,
  parseSetupDaemonPreflight,
} from "@/app/(dashboard)/setup/lib/setupDaemonParsers";

describe("setupDaemonParsers coercion helpers", () => {
  it("coerces string values with fallbacks", () => {
    expect(asStringValue("hello")).toBe("hello");
    expect(asStringValue(42)).toBe("42");
    expect(asStringValue(null, "fallback")).toBe("fallback");
    expect(asStringValue(undefined)).toBe("");
  });

  it("coerces nullable strings", () => {
    expect(asNullableString("value")).toBe("value");
    expect(asNullableString(null)).toBeNull();
    expect(asNullableString(undefined)).toBeNull();
    expect(asNullableString(5)).toBe("5");
  });

  it("coerces ints and bools", () => {
    expect(asIntValue(12.9, 5)).toBe(12);
    expect(asIntValue(" 17 ", 5)).toBe(17);
    expect(asIntValue("not-a-number", 5)).toBe(5);
    expect(asIntValue(Number.NaN, 5)).toBe(5);

    expect(asBoolValue(true, false)).toBe(true);
    expect(asBoolValue(0, true)).toBe(false);
    expect(asBoolValue(2, false)).toBe(true);
    expect(asBoolValue(" YES ", false)).toBe(true);
    expect(asBoolValue("off", true)).toBe(false);
    expect(asBoolValue("maybe", true)).toBe(true);
  });
});

describe("parseSetupDaemonConfig", () => {
  it("applies defaults and computes queue sizes", () => {
    const parsed = parseSetupDaemonConfig({
      core_port: "9001",
      mqtt_password: "  secret ",
      enable_analytics_feeds: "false",
      enable_forecast_ingestion: 0,
      mqtt_username: 1234,
      sidecar_batch_size: "25",
      sidecar_max_queue: "oops",
    });

    expect(parsed.core_port).toBe(9001);
    expect(parsed.mqtt_password_configured).toBe(true);
    expect(parsed.enable_analytics_feeds).toBe(false);
    expect(parsed.enable_forecast_ingestion).toBe(false);
    expect(parsed.mqtt_username).toBe("1234");
    expect(parsed.sidecar_batch_size).toBe(25);
    expect(parsed.sidecar_max_queue).toBe(250);
    expect(parsed.profile).toBe("prod");
    expect(parsed.bundle_path).toBeNull();
    expect(parsed.mqtt_host).toBe("127.0.0.1");
  });

  it("rejects non-object payloads", () => {
    expect(() => parseSetupDaemonConfig(null)).toThrow("Invalid setup daemon config payload.");
  });
});

describe("parseControllerRuntimeConfig", () => {
  it("coerces runtime fields and falls back on defaults", () => {
    const parsed = parseControllerRuntimeConfig({
      mqtt_password_configured: "true",
      enable_analytics_feeds: "no",
      analytics_feed_poll_interval_seconds: "120",
      sidecar_batch_size: 12.7,
    });

    expect(parsed.mqtt_password_configured).toBe(true);
    expect(parsed.enable_analytics_feeds).toBe(false);
    expect(parsed.analytics_feed_poll_interval_seconds).toBe(120);
    expect(parsed.sidecar_batch_size).toBe(12);
    expect(parsed.sidecar_max_queue).toBe(120);
    expect(parsed.schedule_poll_interval_seconds).toBe(15);
  });

  it("rejects non-object payloads", () => {
    expect(() => parseControllerRuntimeConfig("nope")).toThrow(
      "Invalid controller runtime config payload.",
    );
  });
});

describe("parseSetupDaemonPreflight", () => {
  it("filters and normalizes preflight checks", () => {
    const parsed = parseSetupDaemonPreflight({
      checks: [
        { id: "disk", status: "ok", message: "ready" },
        { id: "network", message: 42 },
        { status: "fail" },
        "junk",
        null,
      ],
    });

    expect(parsed).toEqual([
      { id: "disk", status: "ok", message: "ready" },
      { id: "network", status: "unknown", message: "42" },
    ]);
  });

  it("rejects non-object payloads", () => {
    expect(() => parseSetupDaemonPreflight(12)).toThrow(
      "Invalid setup daemon preflight payload.",
    );
  });
});

describe("parseSetupDaemonLocalIp", () => {
  it("coerces recommended ip and filters candidates", () => {
    const parsed = parseSetupDaemonLocalIp({
      recommended: 192,
      candidates: ["10.0.0.1", 5, "", null],
    });

    expect(parsed.recommended).toBe("192");
    expect(parsed.candidates).toEqual(["10.0.0.1", "5"]);
  });

  it("defaults to null recommended when missing", () => {
    const parsed = parseSetupDaemonLocalIp({ candidates: [] });
    expect(parsed.recommended).toBeNull();
  });

  it("rejects non-object payloads", () => {
    expect(() => parseSetupDaemonLocalIp("bad")).toThrow(
      "Invalid setup daemon local-ip payload.",
    );
  });
});
