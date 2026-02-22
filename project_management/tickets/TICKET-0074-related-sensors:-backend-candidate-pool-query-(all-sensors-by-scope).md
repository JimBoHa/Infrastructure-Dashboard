# TICKET-0074: Related Sensors: backend candidate pool query (all sensors by scope)

**Status:** Done (validated locally)

## Description
Today Unified v2 candidates are “sensors already loaded in Trends”, then filtered and truncated. This is fast but causes systematic false negatives when operators expect the tool to search *all* sensors in scope.

This ticket adds a backend-driven candidate pool option so operators can evaluate **all eligible sensors by scope/filters** (within bounded caps), even if they aren’t currently loaded into the Trends chart.

## Scope
* [x] UI (Advanced):
  - Add `Candidate source` selector:
    - `Visible in Trends` (current behavior; default)
    - `All sensors in scope (backend query)`
* [x] Backend:
  - When `candidate_sensor_ids` is empty (or when the new param is selected), compute candidate pool via DB query using the same scope/unit/type/source filters.
  - Return `eligible_count` (number of sensors matching filters) and `evaluated_count` (after truncation).
  - Ensure truncation ordering is not lexicographic (depends on TICKET-0055).
* [x] Add “Evaluate all eligible (may take longer)” option (Advanced):
  - Only enabled when `eligible_count <= 500`.
  - When enabled, `candidate_limit_used = eligible_count` (still respecting hard caps for safety).
* [x] Copy + disclosure:
  - Always show `Evaluated X of Y eligible` and the effective limit used.
  - When backend query is active, label that results are “best among eligible sensors in scope”, not “best among visible”.
* [x] Tests:
  - Backend returns eligible/evaluated counts deterministically for a seeded sensor set.
  - UI shows the new candidate-source control and correct disclosure line.

## Acceptance Criteria
* [x] Operators can switch between “Visible in Trends” and “All sensors in scope” without breaking the existing workflow.
* [x] Backend query mode is bounded (no unbounded scans) and discloses truncation clearly.
* [x] `make ci-web-smoke` and `cargo test --manifest-path apps/core-server-rs/Cargo.toml` pass.

## Validation
- 2026-02-10: `make ci-core-smoke` (PASS)
- 2026-02-10: `make ci-web-smoke` (PASS)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx tests/relatedSensorsPinnedSemantics.test.tsx tests/relatedSensorsProviderAvailability.test.tsx tests/relatedSensorsUnifiedDiagnostics.test.ts tests/relatedSensorsWorkflowImprovements.test.tsx` (PASS)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (PASS)

## Notes
Primary backend surfaces:
- `apps/core-server-rs/src/services/analysis/jobs/related_sensors_unified_v2.rs` (candidate selection)
- `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs` (candidate fetch helper)

This ticket is complementary to (and should be implemented after) TICKET-0054 (disclosure) + TICKET-0055 (truncation bias fixes).
