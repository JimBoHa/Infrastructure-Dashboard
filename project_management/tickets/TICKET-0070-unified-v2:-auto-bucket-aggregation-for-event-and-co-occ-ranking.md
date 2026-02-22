# TICKET-0070: Unified v2: auto bucket aggregation for event and co-occ ranking

**Status:** Done

## Description
Unified v2 currently buckets all sensors using **mean aggregation**, regardless of sensor type. This breaks semantics for:
- state/bool sensors (means create duty-cycle artifacts)
- counter/flow/pulse sensors (sums are meaningful; means can destroy relationships)

The correlation matrix already supports `bucket_aggregation_mode=auto` (sum/last/avg by type), but Unified v2 ranking does not. This creates contradictory evidence in the same panel.

This ticket switches Unified v2 event/co-occurrence ranking to use **auto bucket aggregation** using the same mapping as the bucket reader.

## Scope
* [x] Update `event_match_v1` and `cooccurrence_v1` to read buckets with auto aggregation:
  - Replace `read_bucket_series_for_sensors(... Avg ...)` with `read_bucket_series_for_sensors_with_aggregation(... Auto ...)`.
* [x] Ensure derived sensors remain supported and stable under auto aggregation.
* [x] Add/confirm sensor-type → aggregation mapping is used consistently:
  - counters/flow/pulse/rain-like → `Sum`
  - state/status/bool/switch/contact/mode-like → `Last`
  - continuous → `Avg`
* [ ] (Optional) Expose an Advanced override `bucket_aggregation_mode` for Unified v2 (default Auto).
* [x] Tests:
  - Rust unit tests for mapping and a small synthetic run where:
    - state sensor correlation/events behave as transitions (Last)
    - counter sensor behaves as cumulative (Sum)

## Acceptance Criteria
* [x] Unified v2 ranking uses `auto` aggregation by default and no longer contradicts correlation matrix aggregation semantics.
* [x] Bool/state sensors no longer produce mean-duty-cycle artifacts in event detection.
* [x] Counter/flow/pulse sensors use sum bucketing so event evidence aligns with mechanical linkage.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.

## Notes
Primary files:
- `apps/core-server-rs/src/services/analysis/jobs/event_match_v1.rs`
- `apps/core-server-rs/src/services/analysis/jobs/cooccurrence_v1.rs`
- `apps/core-server-rs/src/services/analysis/bucket_reader.rs`

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
