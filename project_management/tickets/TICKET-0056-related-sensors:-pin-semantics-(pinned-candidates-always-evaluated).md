# TICKET-0056: Related Sensors: pin semantics (pinned candidates always evaluated)

**Status:** Done

## Description
Operators often have a short list of “suspects” they want included in Related Sensors analysis even when the candidate pool is truncated (candidate_limit caps, quick-suggest caps, backend clamps).

This ticket adds **pin semantics** so pinned sensors are always evaluated in the candidate pool for a run, improving troubleshooting workflows without forcing “evaluate all”.

## Scope
* [x] UI: allow pin/unpin of sensors from:
  - a result row
  - and/or a searchable eligible list
* [x] Backend: extend `RelatedSensorsUnifiedJobParamsV2` with `pinned_sensor_ids: Vec<String>` (optional).
* [x] Candidate selection rule (decision complete):
  - Always include all pinned sensors (excluding focus).
  - Set `candidate_limit_used = max(requested_candidate_limit, pinned_count)` but clamp to the hard cap (1000).
  - Fill remaining slots (if any) using the standard candidate ordering/truncation logic.
* [x] UI: pinned sensors display:
  - visible pinned section (“Pinned”)
  - pinned badge on candidate rows
  - show in the coverage disclosure line (e.g., “Pinned included: X”)
* [x] Tests:
  - pinned sensors remain evaluated after Refine
  - pinned sensors remain evaluated even when over the default cap (up to hard max)

## Acceptance Criteria
* [x] Pinned sensors are evaluated in every run (unless explicitly filtered out by an incompatible hard constraint like provider/no-history).
* [x] Candidate pool disclosure reflects pinned inclusion and the effective candidate_limit_used.
* [x] `make ci-web-smoke` and `cargo test --manifest-path apps/core-server-rs/Cargo.toml` pass.

## Notes
Primary UI file: `apps/dashboard-web/src/features/trends/components/RelationshipFinderPanel.tsx`.

Primary backend file: `apps/core-server-rs/src/services/analysis/jobs/related_sensors_unified_v2.rs`.

## Validation
- 2026-02-10: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsPinnedSemantics.test.tsx tests/relatedSensorsUnifiedDiagnostics.test.ts` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
