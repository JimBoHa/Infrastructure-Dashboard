# TICKET-0053: Unified v2: tolerant event alignment matching (tolerance buckets) + efficient matcher

**Status:** Done

## Description
Unified v2 event alignment (“Event match (F1)”) currently requires **exact bucket timestamp equality** after lag shift:

`overlap(L) = |{ t ∈ T_F : (t + lag_sec) ∈ T_C }|`

This is brittle: sampling jitter and minor bucketization differences create false negatives where charts look aligned but event overlap is near zero.

This ticket adds a **tolerance window** for event matching, reusing the existing `tolerance_buckets` concept:
- A focus event at `t_F` matches a candidate event at `t_C` if:
  `| (t_F + lag_sec) − t_C | ≤ tol_seconds`
- Where `tol_seconds = tolerance_buckets * interval_seconds_eff`

## Scope
* [x] Extend `EventMatchJobParamsV1` to accept `tolerance_buckets` (optional; default `0` for backwards-compatible behavior).
* [x] Update `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs` overlap computation to use tolerant matching:
  - Implement an efficient matcher (two-pointer walk over sorted event time arrays).
  - Matching is **one-to-one**: each candidate event can match at most one shifted focus event at a given lag; choose the nearest candidate event within tolerance.
* [x] Ensure all dependent outputs use the same tolerance semantics:
  - `events_overlap`
  - `events_score` (F1)
  - episode construction (`episodes`) uses the same match rule so counts are consistent.
* [x] Wire Unified v2 (`related_sensors_unified_v2`) to pass `tolerance_buckets` through to `event_match_v1`.
* [x] Add unit tests:
  - `tolerance_buckets = 0` matches exact-equality baseline
  - jittered event times match when tolerance > 0
  - one-to-one matching prevents overlap inflation

## Acceptance Criteria
* [x] With `tolerance_buckets > 0`, visually aligned but jittered event series produce non-zero overlap and sensible F1 values.
* [x] With `tolerance_buckets = 0`, behavior is unchanged vs current exact matching.
* [x] Episodes and overlap counts remain internally consistent for the same candidate.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.

## Notes
Primary files:
- `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs`
- `apps/core-server-rs/src/services/analysis/jobs/related_sensors_unified_v2.rs`

UI copy/tooltips that mention “exact bucket timestamp match” must be updated once tolerance is shipped (tracked in the operator contract/UX ticket).

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke-build` (pass)
