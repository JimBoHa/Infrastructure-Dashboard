# TICKET-0057: Unified v2: derived sensor dependency labeling + default exclude in Simple

**Status:** Done

## Description
Derived sensors can be tautological top matches if they depend on the focus sensor (directly or transitively). This is not “independent corroboration” and can mislead operators.

This ticket adds dependency awareness so Unified v2 can:
- label candidates that are derived from the focus sensor
- optionally exclude them by default in Simple mode (while keeping access in Advanced)

## Scope
* [x] Backend: detect derived dependencies:
  - For each candidate sensor `C`, determine whether `C` depends on focus `F` via the derived sensor graph.
  - Output fields (backwards compatible):
    - `derived_from_focus: bool`
    - `derived_dependency_path: [sensor_id...]` (optional; capped length)
* [x] UI: surface derived labeling:
  - Show “Derived from focus” badge in results and preview.
  - In Simple mode, exclude `derived_from_focus` candidates by default.
  - In Advanced mode, add toggle: “Include derived-from-focus candidates”.
* [x] Ensure the filter affects only the candidate pool inclusion, not other strategies’ behavior.
* [x] Tests:
  - Rust unit tests for dependency detection on a small synthetic derived graph
  - Web tests for default exclusion and Advanced toggle

## Acceptance Criteria
* [x] Derived sensors that depend on the focus are labeled deterministically.
* [x] Simple mode excludes derived-from-focus candidates by default; Advanced can include them.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` and `make ci-web-smoke` pass.

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `make ci-web-smoke-build` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsContractSuite.test.ts tests/relatedSensorsWorkflowImprovements.test.tsx` (pass)

## Notes
Primary backend surfaces:
- `apps/core-server-rs/src/services/analysis/bucket_reader.rs` (derived spec parsing/expansion)
- `apps/core-server-rs/src/services/analysis/jobs/related_sensors_unified_v2.rs` (candidate metadata + result payload)

UI surface: `apps/dashboard-web/src/features/trends/components/RelationshipFinderPanel.tsx`.
