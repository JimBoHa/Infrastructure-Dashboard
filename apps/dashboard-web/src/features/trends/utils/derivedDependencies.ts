import type { DemoSensor } from "@/types/dashboard";
import { isDerivedSensor } from "@/lib/sensorOrigin";

function derivedInputIds(sensor: Pick<DemoSensor, "config">): string[] {
  if (!isDerivedSensor(sensor)) return [];
  const config = (sensor.config ?? {}) as Record<string, unknown>;
  const rawDerived = config.derived;
  if (!rawDerived || typeof rawDerived !== "object" || Array.isArray(rawDerived)) return [];

  const derived = rawDerived as Record<string, unknown>;
  const rawInputs = derived.inputs;
  if (!Array.isArray(rawInputs)) return [];

  const out: string[] = [];
  for (const entry of rawInputs) {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)) continue;
    const obj = entry as Record<string, unknown>;
    const sensorId = typeof obj.sensor_id === "string" ? obj.sensor_id.trim() : "";
    if (!sensorId) continue;
    out.push(sensorId);
  }
  return out;
}

export function isDerivedFromFocus(
  candidateSensorId: string,
  focusSensorId: string,
  sensorsById: Map<string, DemoSensor>,
  options?: { maxDepth?: number; maxVisited?: number },
): boolean {
  const candidate = candidateSensorId.trim();
  const focus = focusSensorId.trim();
  if (!candidate || !focus) return false;
  if (candidate === focus) return false;

  const maxDepth = Math.max(0, Math.floor(options?.maxDepth ?? 10));
  const maxVisited = Math.max(1, Math.floor(options?.maxVisited ?? 2000));

  const visited = new Set<string>();
  const stack: Array<{ id: string; depth: number }> = [{ id: candidate, depth: 0 }];
  visited.add(candidate);

  while (stack.length) {
    const next = stack.pop();
    if (!next) break;
    if (next.depth >= maxDepth) continue;

    const sensor = sensorsById.get(next.id);
    if (!sensor) continue;

    const inputs = derivedInputIds(sensor);
    for (const inputId of inputs) {
      if (inputId === focus) return true;
      if (visited.size >= maxVisited) continue;
      if (visited.has(inputId)) continue;
      visited.add(inputId);
      stack.push({ id: inputId, depth: next.depth + 1 });
    }
  }

  return false;
}

export function collectDerivedDependentsOfFocus(
  focusSensorId: string,
  sensorsById: Map<string, DemoSensor>,
  options?: { maxDepth?: number; maxVisited?: number },
): Set<string> {
  const focus = focusSensorId.trim();
  if (!focus) return new Set();

  const maxDepth = Math.max(0, Math.floor(options?.maxDepth ?? 10));
  const maxVisited = Math.max(1, Math.floor(options?.maxVisited ?? 2000));

  const reverse = new Map<string, Set<string>>();
  for (const [sensorId, sensor] of sensorsById) {
    const inputs = derivedInputIds(sensor);
    if (inputs.length === 0) continue;
    for (const inputId of inputs) {
      const entry = reverse.get(inputId) ?? new Set<string>();
      entry.add(sensorId);
      reverse.set(inputId, entry);
    }
  }

  const dependents = new Set<string>();
  const visited = new Set<string>([focus]);
  const stack: Array<{ id: string; depth: number }> = [{ id: focus, depth: 0 }];

  while (stack.length) {
    const next = stack.pop();
    if (!next) break;
    if (next.depth >= maxDepth) continue;

    const children = reverse.get(next.id);
    if (!children) continue;

    for (const childId of children) {
      if (visited.size >= maxVisited) continue;
      if (visited.has(childId)) continue;
      visited.add(childId);
      dependents.add(childId);
      stack.push({ id: childId, depth: next.depth + 1 });
    }
  }

  return dependents;
}
