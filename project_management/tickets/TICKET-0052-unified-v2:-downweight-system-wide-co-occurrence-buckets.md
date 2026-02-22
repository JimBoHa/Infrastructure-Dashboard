# TICKET-0052: Unified v2: downweight system-wide co-occurrence buckets

**Status:** Done

## Description
Unified v2 co-occurrence bucket selection currently **upweights** buckets where many sensors spike together:

`bucket_score = C(|S|, 2) * Σ|z|`

This systematically promotes **system-wide/global events** (reboots, power blips, network outages) and pushes unrelated sensors into the top ranks.

This ticket changes co-occurrence bucket scoring to **downweight** high-`|S|` buckets, so the algorithm prioritizes **specific** co-occurrences over global ones.

**Scoring change (decision complete):**
- Let `S` = sensors with events in the bucket (after tolerance expansion).
- Let `N` = total sensors evaluated by the co-occurrence job (i.e., `params.sensor_ids.len()`).
- Let `severity_sum = Σ min(|z|, z_cap)` (z-cap from TICKET-0051; if not merged yet, implement local cap here too).
- Replace the current upweight:
  - `pair_weight_old = C(|S|, 2)`
  with:
  - `pair_weight = 1 / ln(2 + |S|)`
  - `idf = ln((N + 1) / (|S| + 1))`
  - `bucket_score = severity_sum * pair_weight * idf`

Properties:
- If `|S| == N`, then `idf == 0` so `bucket_score == 0` (pure “everyone spiked” buckets don’t dominate).
- As `|S|` grows, `pair_weight` decreases (specificity bias).

## Scope
* [x] Update `apps/core-server-rs/src/services/analysis/jobs/cooccurrence_v1.rs` bucket scoring:
  - Replace `pair_weight = C(|S|,2)` with the downweight + IDF scheme above.
* [x] Update `CooccurrenceBucketV1` payload to preserve explainability:
  - Keep `group_size`
  - Keep `severity_sum`
  - Keep `pair_weight` (now downweight)
  - Add optional `idf` field (or include an equivalent explainability value in `score_components`/debug fields).
* [x] Add regression tests:
  - With constant `severity_sum`, bucket_score decreases as `group_size` increases.
  - For `group_size == N`, bucket_score is `0` (or near-0 within float tolerance).
* [x] Ensure selection suppression logic (±tolerance buckets) still works and remains deterministic.

## Acceptance Criteria
* [x] Co-occurrence bucket ranking no longer systematically prefers global “many-sensor” buckets.
* [x] The top selected buckets for a focus sensor are more specific (smaller `group_size`) when severity is comparable.
* [x] Co-occurrence job remains bounded: `max_results` and `max_sensors` caps still enforced.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.

## Notes
Primary file: `apps/core-server-rs/src/services/analysis/jobs/cooccurrence_v1.rs`.

Related UI follow-up: surface “system-wide event buckets” separately from “related sensors” results (tracked in TICKET-0062).

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
