import { describe, expect, it } from "vitest";
import { isDerivedFromFocus } from "@/features/trends/utils/derivedDependencies";
import { normalizeUnifiedCandidates } from "@/features/trends/utils/candidateNormalizers";
import type { DemoNode, DemoSensor } from "@/types/dashboard";
import type { RelatedSensorsUnifiedResultV2 } from "@/types/analysis";

describe("Related Sensors contract suite (ticket 68)", () => {
  it("detects derived-from-focus dependencies and is bounded", () => {
    const sensors: DemoSensor[] = [
      {
        sensor_id: "focus",
        node_id: "node-1",
        name: "Focus",
        type: "temperature",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {},
      },
      {
        sensor_id: "d1",
        node_id: "node-1",
        name: "Derived 1",
        type: "derived",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {
          source: "derived",
          derived: {
            expression: "a * 2",
            inputs: [{ sensor_id: "focus", var: "a", lag_seconds: 0 }],
          },
        },
      },
      {
        sensor_id: "d2",
        node_id: "node-1",
        name: "Derived 2",
        type: "derived",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {
          source: "derived",
          derived: {
            expression: "b + 1",
            inputs: [{ sensor_id: "d1", var: "b", lag_seconds: 0 }],
          },
        },
      },
      {
        sensor_id: "cycle_a",
        node_id: "node-1",
        name: "Cycle A",
        type: "derived",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {
          source: "derived",
          derived: {
            expression: "x + 1",
            inputs: [{ sensor_id: "cycle_b", var: "x", lag_seconds: 0 }],
          },
        },
      },
      {
        sensor_id: "cycle_b",
        node_id: "node-1",
        name: "Cycle B",
        type: "derived",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {
          source: "derived",
          derived: {
            expression: "y + 1",
            inputs: [{ sensor_id: "cycle_a", var: "y", lag_seconds: 0 }],
          },
        },
      },
    ];

    const sensorsById = new Map(sensors.map((s) => [s.sensor_id, s]));

    expect(isDerivedFromFocus("d2", "focus", sensorsById)).toBe(true);
    expect(isDerivedFromFocus("cycle_a", "focus", sensorsById)).toBe(false);

    // Bounded: with maxDepth=1, we only consider direct derived inputs of d2, which is d1 (not focus).
    expect(isDerivedFromFocus("d2", "focus", sensorsById, { maxDepth: 1 })).toBe(false);
  });

  it("adds a Dependency badge for derived-from-focus unified candidates", () => {
    const nodes: DemoNode[] = [{ id: "node-1", name: "North Field" }];

    const sensors: DemoSensor[] = [
      {
        sensor_id: "focus",
        node_id: "node-1",
        name: "Focus",
        type: "temperature",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {},
      },
      {
        sensor_id: "d1",
        node_id: "node-1",
        name: "Derived 1",
        type: "derived",
        unit: "C",
        created_at: "2026-01-01T00:00:00Z",
        config: {
          source: "derived",
          derived: {
            expression: "a * 2",
            inputs: [{ sensor_id: "focus", var: "a", lag_seconds: 0 }],
          },
        },
      },
    ];

    const result: RelatedSensorsUnifiedResultV2 = {
      job_type: "related_sensors_unified_v2",
      focus_sensor_id: "focus",
      interval_seconds: 60,
      bucket_count: 42,
      params: {
        focus_sensor_id: "focus",
        start: "2026-02-09T00:00:00Z",
        end: "2026-02-10T00:00:00Z",
        interval_seconds: 60,
        candidate_sensor_ids: ["d1"],
        candidate_limit: 80,
        max_results: 60,
        filters: { exclude_sensor_ids: ["focus"] },
      },
      limits_used: {
        candidate_limit_used: 80,
        max_results_used: 60,
        max_sensors_used: 81,
      },
      candidates: [
        {
          sensor_id: "d1",
          derived_from_focus: true,
          derived_dependency_path: ["d1", "focus"],
          rank: 1,
          blended_score: 0.9,
          confidence_tier: "high",
          evidence: {
            events_score: 0.8,
            events_overlap: 5,
            n_focus: 10,
            n_candidate: 10,
            cooccurrence_count: 2,
            cooccurrence_strength: 0.9,
            best_lag_sec: 0,
          },
        },
      ],
      skipped_candidates: [],
      prefiltered_candidate_sensor_ids: [],
      truncated_candidate_sensor_ids: [],
      truncated_result_sensor_ids: [],
    };

    const candidates = normalizeUnifiedCandidates(result, {
      sensorsById: new Map(sensors.map((s) => [s.sensor_id, s])),
      nodesById: new Map(nodes.map((n) => [n.id, n])),
      labelMap: new Map([
        ["focus", "North Field — Focus (C)"],
        ["d1", "North Field — Derived 1 (C)"],
      ]),
    });

    expect(candidates).toHaveLength(1);
    expect(
      candidates[0]!.badges.some(
        (b) => b.label === "Dependency" && b.value === "Derived from focus",
      ),
    ).toBe(true);
  });
});
