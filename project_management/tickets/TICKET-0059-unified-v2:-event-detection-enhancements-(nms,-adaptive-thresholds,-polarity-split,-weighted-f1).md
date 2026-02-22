# TICKET-0059: Unified v2: event detection enhancements (NMS, adaptive thresholds, polarity split, weighted F1)

**Status:** Done (validated locally; Tier A scheduled)

## Description
The current Unified v2 event detector is intentionally simple (robust z-score on bucket-to-bucket deltas + min-separation + max-events cap). This ticket upgrades event detection to improve precision@k and stability while preserving explainability.

## Scope
* [x] Replace greedy min-separation with non-max suppression (NMS) over a sliding window (time + magnitude).
* [x] Add adaptive thresholds option:
  - Target an event-rate band per sensor type (e.g., 20–200 events/window), bounded.
  - Preserve fixed `z_threshold` mode for backwards compatibility.
* [x] Add boundary artifact handling:
  - Detect/label events within 1 bucket of the window start/end (common “window edge” artifacts).
  - Advanced toggle: exclude boundary events from matching/scoring (default off).
* [x] Add a per-sensor noise-floor guard:
  - When robust scale is tiny, avoid “everything is an event” behavior by applying a minimum effective scale and/or a minimum delta magnitude for events (complements TICKET-0051’s `q_step` scale floor + z cap).
* [x] Add polarity split support:
  - Track #up and #down events and expose in evidence summary.
  - Optionally compute event match separately for up-up / down-down / up-down (inverse).
* [x] Optional ramp detector:
  - Add an Advanced mode to compute events on second differences (Δ²) to catch ramp-like changes without over-triggering on smooth periodic trends.
* [x] Weighted event match:
  - Replace pure overlap-count F1 with a weighted F1 (e.g., weight by `min(|z_F|, |z_C|)`).
* [x] Sparse-series mode:
  - When a sensor is too sparse for stable bucket deltas, add an option to compute “point-events” from raw points (explicitly labeled; bounded).
* [x] Tests:
  - Deterministic outputs for a synthetic series
  - Regression tests for NMS vs greedy separation

## Acceptance Criteria
* [x] Event detector produces fewer redundant near-duplicate events for step/ramp signals.
* [x] Adaptive threshold mode yields stable event counts across different noise floors.
* [x] Weighted event match is explainable and improves precision on continuous sensors where pure overlap is noisy.
* [x] Boundary-event labeling/exclusion is deterministic and does not silently hide real events (must be opt-in if exclusion is enabled).
* [x] `cargo test --manifest-path apps/core-server-rs/Cargo.toml` passes.

## Notes
Primary file: `apps/core-server-rs/src/services/analysis/jobs/event_utils.rs`.

## Validation
- 2026-02-11: Local validation passed:
  - `cargo test --manifest-path apps/core-server-rs/Cargo.toml`
