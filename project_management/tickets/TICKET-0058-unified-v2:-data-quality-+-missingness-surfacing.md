# TICKET-0058: Unified v2: data quality + missingness surfacing

**Status:** Done (Tier A validated installed `0.1.9.262-dw249-missingness`; Tier B deferred to `DW-98`)

## Description
Unified v2 currently ignores raw data quality flags and does not surface missingness/coverage clearly. This makes it easy for:
- sparse sensors
- sensors with long offline windows
- bad-quality point bursts
…to produce misleading event evidence.

This ticket adds data quality preprocessing and missingness explainability, without changing the high-level Unified v2 UX.

## Scope
* [x] Quality flags:
  - Decide and implement a default policy for `quality` filtering before bucketing (e.g., drop non-good qualities).
  - Make the policy configurable in Advanced mode (optional follow-up if needed).
* [x] Bucket sample gating:
  - Require a minimum raw sample count per bucket (default: `>= 2`) to emit a bucket for event detection.
* [x] Missingness metrics:
  - Compute per-sensor `% buckets present` over the evaluated window.
  - Surface missingness in the preview header (focus + candidate).
* [x] Chart affordances:
  - Add “data gap markers” or explicit gaps in the preview chart (when missingness is high).
* [x] Tests:
  - Bucketing respects min-samples
  - Missingness % is computed deterministically for a synthetic bucket series

## Acceptance Criteria
* [x] Bad-quality points are excluded (or explicitly included) per a documented default policy.
* [x] Buckets with insufficient samples do not create events.
* [x] Preview surfaces missingness clearly so operators can interpret evidence.
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` and `make ci-web-smoke` pass.

## Notes
Default analysis policy (decision complete):
- `quality_filter = GoodOnly` (`COALESCE(quality, 0) = 0`)
- `min_samples_per_bucket = 2`

Missingness surfaced as deterministic bucket coverage over the evaluated window:
- `bucket_coverage_pct = buckets_present / expected_buckets * 100`

Reference docs:
- `docs/related-sensors-unified-v2-ds-review-packet.md`
- `docs/related-sensors-unified-v2-explainer.txt`

## Validation
- 2026-02-10: Local validation passed:
  - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
  - `make ci-core-smoke`
  - `make ci-web-smoke`
  - `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsMissingnessPreview.test.tsx`
- 2026-02-10: Tier A validated installed controller `0.1.9.262-dw249-missingness` (no DB/settings reset).
  - Run: `project_management/runs/RUN-20260210-tier-a-dw249-missingness-0.1.9.262-dw249-missingness.md`
  - Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.262-dw249-missingness_unified_preview_2026-02-10_223632415Z/trends_related_sensors_unified_preview.png`
