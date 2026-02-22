# TICKET-0051: Unified v2: gap-aware deltas + z-magnitude scoring cap + gap counters

**Status:** Done

## Description
Missing telemetry buckets currently create **deltas across downtime**, which produces extreme robust z-scores and dominates co-occurrence evidence. This shows up as:
- Huge “Peak” values in episodes (e.g., `Peak 199.65`)
- Enormous raw co-occurrence totals (e.g., `Co-occur: 5,437,121.1`)
- High-ranked false positives driven by gaps, not mechanics

This ticket hardens Unified v2 by making event detection **gap-aware** and bounding the **z-magnitude used for scoring**.

**Initial defaults (decision complete):**
- `gap_max_buckets = 5` (skip deltas where `Δt_actual > gap_max_buckets * interval_seconds_eff`)
- `z_cap = 15` (cap `|z|` used in severity/co-occurrence scoring and episode peak/mean computations)

## Scope
* [x] Gap-aware deltas: ignore delta steps when consecutive existing buckets are too far apart.
  - Add a configurable `gap_max_buckets` parameter (default `5`) so Advanced can tune the suppression threshold.
* [x] Add z-magnitude cap for scoring: use `z_used = sign(z_raw) * min(|z_raw|, z_cap)` anywhere `|z|` contributes to scoring/aggregation.
* [x] Quantization-aware robust scale floor (mitigates degenerate MAD/IQR):
  - When robust scale is degenerate (or very small), compute `q_step = median(non-zero |Δ|)` over the delta series.
  - Set `scale := max(scale, q_step)` (if `q_step` is finite and > 0).
  - Keep `z_cap` in place as an additional safety bound for scoring.
* [x] Track `gap_skipped_deltas` per sensor during event detection and surface in job results for explainability (backwards-compatible optional fields).
* [x] Add deterministic unit tests for:
  - gap suppression (`Δt_actual` thresholding)
  - z capping (`z_used` bounded, raw detection still works)
  - quantization scale floor (`q_step` prevents inflated z on step-like sensors)
  - `gap_skipped_deltas` counts
* [x] Ensure Unified v2 remains cancel-aware and perf-bounded (no unbounded per-point scans).

## Acceptance Criteria
* [x] Deltas with `Δt_actual > gap_max_buckets * interval_seconds_eff` do **not** produce events.
* [x] `|z|` contributions to:
  - co-occurrence severity (`severity_sum`)
  - co-occurrence aggregation (`cooccurrence_score`)
  - episode metrics (`score_mean`, `score_peak`)
  are capped at `z_cap`.
* [x] Quantized/step-like sensors no longer inflate to extreme z-scores solely due to degenerate scale fallbacks.
* [x] Job responses include gap counts (or equivalent) so the UI can disclose “gap-driven evidence risk”.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.

## Notes
Primary implementation surfaces:
- `apps/core-server-rs/src/services/analysis/jobs/event_utils.rs` (`detect_change_events`)
- `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs` (episode metrics + overlap consistency)
- `apps/core-server-rs/src/services/analysis/jobs/cooccurrence_v1.rs` (severity/bucket scoring)

Follow-up UX work (separate ticket): show `gap_skipped_deltas` in Related Sensors preview and/or evidence summary.

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
