import { describe, expect, it } from "vitest";

import { configString, findSensor, sensorMetric, sensorOriginKind, sensorSource } from "@/lib/sensorOrigin";

describe("sensorOrigin helpers", () => {
  it("configString trims and returns null for empty strings", () => {
    expect(configString({ source: " emporia_cloud " }, "source")).toBe("emporia_cloud");
    expect(configString({ source: "   " }, "source")).toBeNull();
    expect(configString({ source: 123 }, "source")).toBeNull();
  });

  it("sensorSource and sensorMetric read config strings", () => {
    const sensor = { config: { source: " ws_2902 ", metric: " temperature_c " } };
    expect(sensorSource(sensor)).toBe("ws_2902");
    expect(sensorMetric(sensor)).toBe("temperature_c");
  });

  it("sensorOriginKind handles provider/derived sources", () => {
    expect(sensorOriginKind({ config: { source: "forecast_points" } })).toBe("public_provider");
    expect(sensorOriginKind({ config: { source: " derived " } })).toBe("derived");
    expect(sensorOriginKind({ config: { source: "ws_2902" } })).toBe("local");
  });

  it("findSensor matches by trimmed source + metric", () => {
    const sensors = [
      { config: { source: "emporia_cloud", metric: "mains_power_w" }, sensor_id: "1" },
      { config: { source: "renogy_bt2", metric: "pv_power_w" }, sensor_id: "2" },
    ];

    expect(findSensor(sensors, " emporia_cloud ", " mains_power_w ")).toMatchObject({ sensor_id: "1" });
    expect(findSensor(sensors, "missing", "mains_power_w")).toBeNull();
    expect(findSensor(sensors, "", "mains_power_w")).toBeNull();
  });
});

