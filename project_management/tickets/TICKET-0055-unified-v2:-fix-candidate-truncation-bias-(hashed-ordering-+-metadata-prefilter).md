# TICKET-0055: Unified v2: fix candidate truncation bias (hashed ordering + metadata prefilter)

**Status:** Done

## Description
Unified v2 candidate inclusion is currently biased and non-representative when `eligible_count > candidate_limit`:
- UI sorts candidates lexicographically by `sensor_id` and truncates.
- Backend re-sorts by `sensor_id` and truncates again.

This causes arbitrary false negatives (“not evaluated”) that correlate with sensor_id prefixes, not relevance.

This ticket replaces lexicographic truncation with a **stable hashed ordering** and adds a **lightweight metadata/coverage prefilter** so the evaluated pool is both more representative and more precision-biased in Simple mode.

## Scope
* [x] Backend: replace lexicographic truncation in `related_sensors_unified_v2` (and any shared candidate fetch) with deterministic ordering:
  - Primary priority groups (descending):
    1) same node as focus
    2) same unit as focus
    3) same type as focus
    4) everything else
  - Within each group, order by **stable hash** of `sensor_id` with seed derived from:
    - `focus_sensor_id` (required)
    - and/or `job_key` (if available)
* [x] Add a lightweight prefilter before expensive scoring:
  - Require minimum bucket coverage (e.g., `bucket_rows >= 3` and `delta_count >= 3`)
  - Optionally require minimum event count for focus/candidate when using event alignment
* [x] Ensure co-occurrence stage does not reintroduce lexicographic truncation:
  - If `candidate_limit > (max_sensors - 1)`, explicitly select which candidates are included in the co-occurrence job using the same deterministic ordering (do not rely on `cooccurrence_v1` truncating a sorted list).
* [x] UI: stop relying on `.sort()` + `.slice(0, limit)` for truncation in Simple mode; let backend decide ordering (UI still provides eligible IDs for “loaded in Trends” semantics).
* [x] Return explicit truncation summary:
  - how many eligible were dropped by ordering/truncation
  - how many were dropped by minimum coverage prefilter
* [x] Tests:
  - deterministic ordering test (same inputs => same evaluated set)
  - regression test that truncation is no longer correlated with lexicographic sensor_id order

## Acceptance Criteria
* [x] When eligible sensors exceed candidate_limit, the evaluated candidate set is stable and not biased by sensor_id prefixes.
* [x] Minimum coverage prefilter reduces “garbage-in” candidates without filtering out well-formed series.
* [x] Result payload reports which truncation/prefilter paths were applied.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` and `make ci-web-smoke` pass.

## Notes
Primary backend file: `apps/core-server-rs/src/services/analysis/jobs/related_sensors_unified_v2.rs`.

Related UX dependency: coverage disclosure and truncation messaging (TICKET-0054 / TICKET-0069).

## Validation
- 2026-02-10: `cargo test --manifest-path apps/core-server-rs/Cargo.toml` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
