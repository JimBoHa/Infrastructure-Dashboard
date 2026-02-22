# TICKET-0076: Related Sensors: show top lag candidates (top 3 lags)

**Status:** Done

## Description
Event alignment currently reports a single “best lag”. In practice, lag hypotheses can be near-tied (especially with periodic drivers or sparse events), and hiding near-ties makes the lag feel more certain than it is.

This ticket surfaces the **top 3 lag hypotheses** (F1 + overlap) so operators can understand lag uncertainty and pick an alternative lag when it’s essentially equivalent.

## Scope
* [x] Backend:
  - Extend `event_match_v1` to optionally return `top_lags: Vec<EventMatchLagScoreV1>` (max 3), sorted by:
    1) F1 score desc
    2) overlap desc
    3) |lag| asc (tie-break)
  - Add request param `top_k_lags` (default `0` for back-compat; Unified v2 Advanced sets `3`).
  - Ensure tolerance matching (TICKET-0053) is applied consistently when computing top lags.
* [x] UI:
  - In preview (Advanced), show “Top lags” list:
    - `Lag +10m (F1 0.21, matched 4)`
    - `Lag 0 (F1 0.20, matched 4)`
    - `Lag -10m (F1 0.19, matched 4)`
  - Add a one-click “Use this lag for preview alignment” action per lag entry (chart-only; does not change ranking).
* [x] Tests:
  - Backend returns stable top 3 lags for a synthetic event set.
  - UI renders the top-lags list and alignment control.

## Acceptance Criteria
* [x] Operators can see when lag is ambiguous (near ties) instead of over-trusting one number.
* [x] Top-lag display does not change ranking; it only affects preview alignment (unless/until a separate ranking ticket changes this).
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` and `make ci-web-smoke` pass.

## Notes
Primary backend file: `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs`.

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsTopLags.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
