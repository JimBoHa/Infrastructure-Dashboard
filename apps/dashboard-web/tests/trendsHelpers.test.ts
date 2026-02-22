import { vi } from "vitest";
import { computeDomain, exportCsv } from "@/features/trends/utils/trendsUtils";
import type { TrendSeriesEntry } from "@/types/dashboard";

if (!window.URL.createObjectURL) { window.URL.createObjectURL = vi.fn(); }
if (!window.URL.revokeObjectURL) { window.URL.revokeObjectURL = vi.fn(); }


describe("computeDomain", () => {
  it("parses min and max values", () => {
    const domain = computeDomain("10", "100");
    expect(domain.min).toBe(10);
    expect(domain.max).toBe(100);
  });
  it("returns undefined for empty inputs", () => {
    const domain = computeDomain("", "");
    expect(domain.min).toBeUndefined();
    expect(domain.max).toBeUndefined();
  });
});
describe("exportCsv", () => {
  const series: TrendSeriesEntry[] = [
    {
      sensor_id: "soil-moisture",
      label: "Soil Moisture",
      points: [
        { timestamp: "2025-10-30T12:00:00Z", value: 32, samples: 3 },
        { timestamp: "2025-10-30T13:00:00Z", value: 33, samples: 2 },
      ],
    },
  ];
  it("builds CSV rows filtered by sensor IDs", () => {
    const sensorIds: string[] = ["soil-moisture"];
    const blobSpy = vi.spyOn(window.URL, "createObjectURL").mockReturnValue("blob:csv");
    const linkSpy = vi.spyOn(document.body, "appendChild");
    const removeSpy = vi.spyOn(document.body, "removeChild");
    exportCsv(sensorIds, series);
    expect(blobSpy).toHaveBeenCalled();
    expect(linkSpy).toHaveBeenCalled();
    expect(removeSpy).toHaveBeenCalled();
    blobSpy.mockRestore();
    linkSpy.mockRestore();
    removeSpy.mockRestore();
  });
});
