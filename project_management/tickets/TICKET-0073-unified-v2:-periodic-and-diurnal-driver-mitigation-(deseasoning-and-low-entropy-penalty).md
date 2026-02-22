# TICKET-0073: Unified v2: periodic and diurnal driver mitigation (deseasoning and low-entropy penalty)

**Status:** Done

## Description
Periodic drivers (especially diurnal cycles) can create spurious “related” results across unrelated subsystems:
- PV power, temperature, humidity, and many unrelated sensors show synchronized ramps during daylight windows.
- Episodes recur at the same local times each day.
- Operators misread the top ranks as mechanistic relationships, wasting time (false positives are most costly).

This ticket reduces diurnal/periodic false positives by adding:
1) an optional **deseasoning** pre-step before event detection, and
2) an optional **low time-of-day entropy penalty** (downweights sensors whose events happen at nearly the same time each day).

## Scope
* [x] Add a preprocessing mode for Unified v2 event detection (Advanced option; default off initially):
  - `deseason_mode = none | hour_of_day_mean`
* [x] Implement `hour_of_day_mean` deseasoning on bucketed levels per sensor:
  - Compute hour-of-day (0–23) from bucket timestamps (UTC for now; switch to controller/site timezone once CS-103 lands).
  - For each hour `h`, compute mean `μ_h` across all buckets in that hour.
  - Emit residual bucket values: `x_resid(t) = x(t) - μ_hour(t)`.
  - Run deltas/events on `x_resid` instead of raw `x`.
  - Gate: only enable deseasoning when the window spans ≥ 2 full days (otherwise skip and label as “insufficient window for deseasoning”).
* [x] Add a low-entropy penalty option (Advanced; default on when deseasoning is off):
  - Compute event time-of-day histogram over 24 bins for each sensor.
  - Compute normalized entropy `H_norm = H / ln(24)` in `[0,1]`.
  - Define `entropy_weight = clamp(H_norm, 0.25, 1.0)`.
  - Apply `entropy_weight` to scoring contributions that use `|z|` (co-occurrence severity and episode peak/mean), without changing event thresholding.
  - Surface `H_norm` (or a “Periodic driver suspected” boolean) in evidence/debug fields for UI/tooltips.
* [x] UI:
  - In Advanced mode, surface deseasoning + periodic penalty toggles with explicit copy: “Mitigate diurnal/periodic artifacts (may reduce true positives for truly periodic mechanisms).”
* [x] Tests:
  - Synthetic daily-sine series: deseasoning reduces event counts and reduces spurious matches across unrelated sensors.
  - Entropy penalty reduces scores for sensors whose events cluster at fixed times-of-day.

## Acceptance Criteria
* [x] With deseasoning enabled and ≥2-day windows, diurnal-driven false positives are down-ranked vs baseline on a synthetic test suite.
* [x] Entropy penalty is deterministic and bounded (no per-point heavy compute).
* [x] UI makes it clear this is an artifact-mitigation tool, not a causality proof.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` and `make ci-web-smoke` pass.

## Notes
This ticket is intentionally scoped to Unified v2 (events + co-occurrence ranking). It is complementary to TSSE-37’s diurnal-lag penalty for `related_sensors_v1`.

## Validation
- 2026-02-10: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsAdvancedMitigationControls.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
