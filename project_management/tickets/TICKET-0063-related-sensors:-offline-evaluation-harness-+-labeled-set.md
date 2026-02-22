# TICKET-0063: Related Sensors: offline evaluation harness + labeled set

**Status:** Done

## Description
To iterate on Related Sensors scoring, we need offline quality measurement. This ticket builds a lightweight evaluation harness and a small labeled dataset so we can compute precision@k and stability by sensor type and scenario.

## Scope
* [ ] Define a labeled evaluation set format:
  - Focus sensor id
  - Window (start/end)
  - Expected related sensor ids (and optionally direction same/opposite)
  - Scenario tags (outage vs local fault vs derived tautology)
* [ ] Implement an offline runner that:
  - Executes Unified v2 with fixed params on each labeled case
  - Emits metrics: precision@k (k=5/10/20), MRR, coverage (#runs with ≥1 strong candidate)
* [ ] Reporting:
  - Emit a Markdown/JSON report under `reports/` with per-case breakdown and aggregate metrics.
* [ ] CI hook (optional):
  - Add a non-blocking “smoke eval” target for regression detection on a tiny subset.

## Acceptance Criteria
* [ ] A developer can run the harness locally and get a deterministic report for the labeled cases.
* [ ] The harness can be extended by adding new labeled cases without code changes.
* [ ] Baseline metrics are captured for at least 10 cases across 3+ sensor types.

## Notes
Existing TSSE tooling to reuse where possible:
- `apps/core-server-rs/src/bin/tsse_bench.rs`
- `apps/core-server-rs/src/bin/tsse_recall_eval.rs`
