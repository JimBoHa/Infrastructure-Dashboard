import { describe, expect, it, vi } from "vitest";

vi.mock("next/navigation", () => ({
  redirect: vi.fn(),
}));

import { redirect } from "next/navigation";
import ProvisioningPage from "@/app/(dashboard)/provisioning/page";

describe("ProvisioningPage", () => {
  it("redirects to /deployment", () => {
    ProvisioningPage();
    expect(redirect).toHaveBeenCalledWith("/deployment");
  });
});
