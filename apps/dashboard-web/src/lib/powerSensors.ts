import { sensorSource } from "@/lib/sensorOrigin";
import type { DemoNode, DemoSensor } from "@/types/dashboard";

export type PowerNodeKind = "emporia" | "renogy";

export function classifyPowerNode(node: Pick<DemoNode, "config">, sensors: Array<Pick<DemoSensor, "config">>): PowerNodeKind | null {
  const config = node.config ?? {};
  const externalProvider = config["external_provider"];
  if (typeof externalProvider === "string" && externalProvider === "emporia") return "emporia";
  if (sensors.some((sensor) => sensorSource(sensor) === "emporia_cloud")) return "emporia";
  if (sensors.some((sensor) => sensorSource(sensor) === "renogy_bt2")) return "renogy";
  return null;
}

