import { describe, expect, it } from "vitest";

import { sha256Hex } from "@/lib/sha256";

describe("sha256Hex", () => {
  it("computes a known digest", async () => {
    await expect(sha256Hex("abc")).resolves.toBe(
      "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
  });
});

