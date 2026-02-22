# TICKET-0065: Related Sensors: rank stability scoring + monitoring

**Status:** Done

## Description
Pool-relative normalization and candidate truncation can cause rank flips. Even when correctness is “technically” fine, unstable ranks reduce operator trust.

This ticket adds stability measurement and optional stability-aware signals.

## Scope
* [ ] Add a stability score option:
  - Re-run Unified v2 on 3 subwindows (split the requested range into thirds).
  - Compute overlap@k and/or Kendall tau for top-k (k=10).
  - Report stability as a 0–1 score and surface as “Stability: high/medium/low”.
* [ ] Add monitoring for pathological evidence:
  - Distribution of `Peak |Δz|` (percentiles)
  - percent clipped by z_cap
  - percent skipped by gap suppression
* [ ] Bound compute cost:
  - Stability is opt-in (Advanced) or runs only when eligible_count <= threshold.

## Acceptance Criteria
* [ ] Stability score is deterministic and bounded in runtime.
* [ ] Monitoring outputs can be used to catch regressions in event detection/scoring.
* [ ] No production-path performance regressions for standard runs.

## Notes
This ticket intentionally couples “quality monitoring” with “stability scoring” so regressions can be debugged quickly.

## Implementation notes (2026-02-11)
- Added opt-in stability scoring for Unified v2 (`stability_enabled`): splits the requested window into thirds, reruns Unified v2 on each, and reports overlap@10 stability (0–1) with `high/medium/low` tiering; stability is skipped when `eligible_count > 120` to bound runtime.
- Added evidence health monitoring outputs (peak |Δz| percentiles, z-cap clipping %, gap-suppression %) surfaced on `related_sensors_unified_v2` results.

## Validation
- `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (PASS)
- `make ci-web-smoke` (PASS)
