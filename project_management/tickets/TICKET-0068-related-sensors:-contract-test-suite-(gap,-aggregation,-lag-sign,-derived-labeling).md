# TICKET-0068: Related Sensors: contract test suite (gap, aggregation, lag sign, derived labeling)

**Status:** Done

## Description
Related Sensors has several “operator contract” semantics that must remain correct even as we iterate. This ticket adds deterministic contract tests (unit/integration) so changes don’t silently regress interpretation-critical behavior.

## Scope
* [x] Add deterministic tests for pool-relative normalization behavior:
  - Rank score changes when candidate pool changes (expected; documented by test).
* [x] Gap delta suppression correctness:
  - Deltas across long gaps are ignored and gap skipped counts are accurate (covered by `event_utils` unit tests).
* [x] Aggregation mode selection by type string:
  - Mapping is consistent across ranking and correlation jobs (covered by `bucket_reader` unit test).
* [x] Lag sign correctness:
  - Positive lag means candidate later (per UI tooltip contract) (covered by `event_match_v1` unit test).
* [x] Derived sensor dependency labeling:
  - Derived-from-focus detection is correct and bounded (cycle-safe) (covered by dashboard-web contract tests).
* [x] Add a small integration test harness that runs `related_sensors_unified_v2` over synthetic bucket series (no external lake required).

## Acceptance Criteria
* [x] Tests cover the contract points above and fail deterministically on regression.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.

## Notes
This ticket is intentionally “cross-cutting” and will touch both:
- TSSE job math utilities
- the unified job merge/normalization logic

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsContractSuite.test.ts` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
