# TICKET-0067: Docs: operator-facing How Related Sensors works

**Status:** Done

## Description
Create a single operator-facing doc that explains Related Sensors in plain language with strict guardrails against misleading interpretation.

Goal: an operator can understand what the tool *is* (investigation leads), what it *isn’t* (causality/probability), and what knobs change results.

## Scope
* [x] Write a concise operator-grade explanation (≤120 words) that covers:
  - raw points → buckets → deltas → events → (event match + co-occurrence) → rank score
  - pool-relative ranking caveat
  - “Evidence” is heuristic coverage, not statistical confidence
* [x] Include a simple diagram (ASCII or embedded image) showing the pipeline.
* [x] Include a boxed warning section:
  - Rank score is not a probability
  - Evidence is not statistical significance
  - results depend on evaluated candidates and effective interval
* [x] Provide Advanced tooltips text for each key parameter:
  - z threshold, max lag, tolerance, candidate limit, weights
* [x] Place the doc at a stable path under `docs/` and link it from the Trends panel (or from the in-panel “How it works” key).

## Acceptance Criteria
* [x] Doc exists under `docs/` and is referenced from the UI.
* [x] Doc includes the explicit “not causality / not probability” warnings.
* [x] Copy matches current implementation semantics and updated UI labels (Rank score, Evidence).

## Notes
Good sources to build from (do not copy verbatim; rewrite for operators):
- `docs/related-sensors-unified-v2-ds-review-packet.md`
- `docs/related-sensors-unified-v2-explainer.txt`
