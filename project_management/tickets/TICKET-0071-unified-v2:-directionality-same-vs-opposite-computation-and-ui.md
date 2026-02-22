# TICKET-0071: Unified v2: directionality same vs opposite computation and UI

**Status:** Done

## Description
Unified v2 currently treats same-direction and opposite-direction relationships identically (it uses `|z|` and timestamp overlap). For troubleshooting, operators need to know if a candidate tends to move **with** or **against** the focus.

This ticket adds an explicit `Direction: same|opposite|unknown` label, computed using **both**:
1) matched-event sign agreement, and
2) signed delta correlation at best lag (when there is enough evidence).

## Scope
* [x] Backend: compute directionality per candidate at `best_lag_sec`:
  - Matched-event sign agreement:
    - compute on matched event pairs (after lag), with tolerance semantics matching the shipped event-match logic
    - `sign_agreement = (#same_sign) / (#matched_pairs)`
  - Signed delta correlation at best lag:
    - compute Pearson correlation on aligned bucket deltas `Δ_F(t)` vs `Δ_C(t + lag)`
    - only compute when `matched_pairs >= 5` (otherwise omit)
* [x] Direction label rule (decision complete):
  - If `matched_pairs < 3`: `unknown`
  - Else if `delta_corr` is present:
    - `same` if `delta_corr >= 0`
    - `opposite` if `delta_corr < 0`
  - Else (no `delta_corr`): use `sign_agreement`:
    - `same` if `sign_agreement >= 0.5`
    - `opposite` otherwise
* [x] Extend Unified v2 evidence payload (backwards compatible optional fields):
  - `direction_label` (`same|opposite|unknown`)
  - `sign_agreement`
  - `delta_corr`
  - `direction_n` (matched pairs count)
* [x] UI: show “Direction” in preview:
  - `Direction: same` / `Direction: opposite` / `Direction: unknown`
  - Tooltip includes the metrics used (agreement + delta corr if present).
* [x] Tests:
  - Synthetic series tests where direction is known (same vs opposite)
  - Gating tests for `unknown` when evidence is too sparse

## Acceptance Criteria
* [x] Related Sensors preview surfaces directionality clearly with explicit “unknown” handling for sparse cases.
* [x] Directionality computation is deterministic and bounded.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` and `make ci-web-smoke` pass.

## Notes
Primary backend file: `apps/core-server-rs/src/services/analysis/jobs/related_sensors_unified_v2.rs` (payload merge).

May require extending `event_match_v1` outputs or computing direction in Unified merge using the same event sets.

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
