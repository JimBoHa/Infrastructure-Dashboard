# TICKET-0061: Trends: correlation block refinements (delta corr, lagged corr, focus-vs-candidate default)

**Status:** Done

## Description
The correlation matrix is useful context, but it can create “statistics authority bleed” and can contradict the event-based ranking.

This ticket refines the correlation surface so it is:
- clearly labeled as *not used for ranking*
- less cognitively heavy (default focus-vs-candidate list)
- more aligned with troubleshooting needs (optional delta correlation and lagged correlation views)

## Scope
* [x] Default UI:
  - Show a 1D list of correlations between focus sensor and each candidate by default.
  - Keep the full matrix as an opt-in expansion.
* [x] Add optional delta correlation view:
  - Correlation on first differences (bucket deltas), not levels.
* [x] Add lagged correlation option:
  - Show best correlation within ±`maxLagBuckets` (bounded, deterministic).
* [x] Guardrails:
  - Increase default overlap gating for highly autocorrelated series (require `n_eff >= 10` or similar).
  - Explicitly label weak/none correlation cases.
* [x] Tests:
  - UI renders focus-vs-candidate list
  - Backend lagged correlation boundedness tests

## Acceptance Criteria
* [x] Correlation surface is clearly framed as “context, not ranking”.
* [x] Focus-vs-candidate correlation is easy to scan without a full NxN matrix.
* [x] `make ci-web-smoke` and relevant Rust tests pass.

## Notes
Primary files:
- UI: `apps/dashboard-web/src/features/trends/components/relationshipFinder/CorrelationMatrix.tsx`
- Backend: `apps/core-server-rs/src/services/analysis/jobs/correlation_matrix_v1.rs`

## Validation
- 2026-02-10: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsCorrelationBlockRefinements.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
