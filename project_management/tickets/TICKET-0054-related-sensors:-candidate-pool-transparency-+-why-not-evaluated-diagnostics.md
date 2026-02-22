# TICKET-0054: Related Sensors: candidate pool transparency + why-not-evaluated diagnostics

**Status:** Done

## Description
Related Sensors results are **pool-relative** and the evaluated candidate set is silently truncated by multiple limits (UI candidateLimit, backend caps, and co-occurrence `max_sensors`). Today the UI mostly says “N eligible candidates”, but does not clearly disclose:
- how many were actually evaluated
- what limit was applied (including backend clamping)
- the effective interval after max-buckets clamping
- why a specific expected sensor is missing (filtered out vs not evaluated vs evaluated but below threshold)

This ticket adds **operator-visible coverage disclosure** and a **“why not evaluated?”** diagnostic path to reduce trust-killing confusion.

## Scope
* [x] UI: display candidate coverage disclosure near the run controls:
  - `Evaluated: <evaluated_count> of <eligible_count> eligible sensors (limit: <candidate_limit_used>).`
  - `Effective interval: <interval_seconds_eff> (requested: <interval_seconds_requested>).`
* [x] Backend: ensure Unified v2 result contains stable, unambiguous counts/limits:
  - `counts.candidate_pool` (already) must represent **evaluated** candidates
  - add explicit `limits_used` fields (or equivalent) so UI does not infer:
    - `candidate_limit_used`
    - `max_results_used`
    - `max_sensors_used` (co-occurrence stage)
* [x] Add a “Why not evaluated?” UX:
  - User selects/enters a sensor ID from the eligible list
  - UI shows a deterministic reason, in priority order:
    1) Not eligible (filtered out by scope/unit/type/source)
    2) Eligible but not evaluated (truncated by candidate_limit or backend cap)
    3) Evaluated but did not exceed evidence threshold
    4) Provider/forecast sensor has no stored history in the analysis lake (see TICKET-0072)
* [x] Refactor `truncated_sensor_ids` semantics (backend) so UI can distinguish:
  - truncated due to candidate pool limit
  - truncated due to `max_results`
  (either split into two arrays or add a structured truncation summary)
* [x] Tests:
  - Dashboard-web unit tests for rendering the disclosure lines
  - Deterministic tests for why-not-evaluated reason ordering

## Acceptance Criteria
* [x] After any run, operators can see evaluated vs eligible counts and the effective interval without opening tooltips.
* [x] When a known sensor is missing from results, the UI can explain which category it fell into (filtered/truncated/low evidence/no history).
* [x] Copy is consistent with the “pool-relative” contract (no implication of global/exhaustive search).
* [x] `make ci-web-smoke` passes.

## Notes
Primary UI file: `apps/dashboard-web/src/features/trends/components/RelationshipFinderPanel.tsx`.

Primary backend file: `apps/core-server-rs/src/services/analysis/jobs/related_sensors_unified_v2.rs`.

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsOperatorContract.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsUnifiedDiagnostics.test.ts` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
