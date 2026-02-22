import { describe, expect, it } from "vitest";

import { parseCount, parseMillis, parsePort, parseSeconds } from "@/app/(dashboard)/setup/lib/validation";

describe("parsePort", () => {
  it("accepts valid port ranges", () => {
    expect(parsePort("Core API port", "1")).toEqual({ ok: true, value: 1 });
    expect(parsePort("Core API port", "65535")).toEqual({ ok: true, value: 65535 });
  });

  it("rejects invalid ports", () => {
    const error = "Core API port must be a valid TCP port (1..65535).";
    expect(parsePort("Core API port", "0")).toEqual({ ok: false, error });
    expect(parsePort("Core API port", "70000")).toEqual({ ok: false, error });
    expect(parsePort("Core API port", "abc")).toEqual({ ok: false, error });
  });
});

describe("parseSeconds", () => {
  it("accepts values at or above the minimum", () => {
    expect(parseSeconds("Poll interval", "10", 10)).toEqual({ ok: true, value: 10 });
  });

  it("rejects values below the minimum or invalid", () => {
    const error = "Poll interval must be at least 10 seconds.";
    expect(parseSeconds("Poll interval", "9", 10)).toEqual({ ok: false, error });
    expect(parseSeconds("Poll interval", "nope", 10)).toEqual({ ok: false, error });
  });
});

describe("parseMillis", () => {
  it("accepts values at or above the minimum", () => {
    expect(parseMillis("Flush interval", "50", 50)).toEqual({ ok: true, value: 50 });
  });

  it("rejects values below the minimum or invalid", () => {
    const error = "Flush interval must be at least 50 ms.";
    expect(parseMillis("Flush interval", "20", 50)).toEqual({ ok: false, error });
    expect(parseMillis("Flush interval", "nope", 50)).toEqual({ ok: false, error });
  });
});

describe("parseCount", () => {
  it("accepts values at or above the minimum", () => {
    expect(parseCount("Batch size", "10", 10)).toEqual({ ok: true, value: 10 });
  });

  it("rejects values below the minimum or invalid", () => {
    const error = "Batch size must be at least 10.";
    expect(parseCount("Batch size", "9", 10)).toEqual({ ok: false, error });
    expect(parseCount("Batch size", "nope", 10)).toEqual({ ok: false, error });
  });
});
