# TICKET-0060: Unified v2: co-occurrence scoring refinements (normalize, surprise, focus-centric, prevalence penalty)

**Status:** Done (validated locally; Tier A scheduled)

## Description
Co-occurrence is a useful secondary evidence channel for “shared anomaly buckets”, but the current implementation can be dominated by:
- raw magnitude products
- selection artifacts from top-bucket picking
- candidates that have events “everywhere” (high prevalence)

This ticket refines co-occurrence scoring to improve precision and stability, while keeping the evidence interpretable to operators.

## Scope
* [x] Normalize co-occurrence by count (average strength):
  - Add `cooccurrence_avg = cooccurrence_score / max(1, cooccurrence_count)`
  - Use the average (with clamping) for ranking/normalization rather than raw sum.
* [x] Add “surprise” style scoring option:
  - `( |z_F| * |z_C| ) / E[ |z_F| * |z_C| ]` estimated from marginals.
* [x] Focus-centric top-bucket selection:
  - Select top buckets from buckets where focus has events (rank by focus severity), rather than global buckets.
* [x] Prevalence penalty:
  - Downweight candidates with very high event rate across the window.
* [x] Add an operator toggle (Advanced) to choose bucket preference mode:
  - `Prefer specific matches` (default): downweights system-wide buckets (pairs well with TICKET-0052).
  - `Prefer system-wide matches`: explicitly allows global-event buckets to surface for outage/debug workflows.
* [x] Tests:
  - normalization behaves as expected
  - focus-centric selection excludes non-focus buckets

## Acceptance Criteria
* [x] Co-occurrence evidence is less dominated by single extreme z events and more stable across minor pool changes.
* [x] Focus-centric selection reduces unrelated “global” buckets.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.

## Notes
Primary file: `apps/core-server-rs/src/services/analysis/jobs/cooccurrence_v1.rs`.

Bucket downweighting (system-wide suppression) is tracked separately in TICKET-0052.

## Validation
- 2026-02-11: Local validation passed:
  - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
  - `make ci-web-smoke`
