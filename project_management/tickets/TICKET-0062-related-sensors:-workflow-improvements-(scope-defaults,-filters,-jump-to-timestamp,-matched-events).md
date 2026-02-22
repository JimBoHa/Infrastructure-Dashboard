# TICKET-0062: Related Sensors: workflow improvements (scope defaults, filters, jump-to-timestamp, matched events)

**Status:** Done

## Description
This ticket improves the Related Sensors workflow for troubleshooting by making Simple mode more precision-biased and adding high-leverage operator actions for triage and navigation.

## Scope
* [x] Simple mode default scope:
  - Default to `Same node` with a one-click “Broaden to all nodes” affordance.
  - When Scope = `All nodes`, apply stricter Simple defaults (decision complete):
    - raise quick-suggest `z_threshold` by +0.5 (or equivalent) and/or require higher overlap for “strong evidence”.
    - set `min_sensors` default to `3` for co-occurrence buckets (reduces pairwise noise across large scopes).
* [x] Result list quick filters:
  - same unit
  - same type
  - exclude derived-from-focus
  - exclude system-wide buckets
* [x] Add a separate “System-wide events” panel (collapsed by default in Simple):
  - Shows the top co-occurrence buckets where `group_size / N_total_sensors >= 0.5` (and/or `group_size >= 10`).
  - Each row shows: timestamp, `group_size`, severity, and a “Jump to timestamp” action.
  - This panel is explicitly framed as outage/debug context, not “related sensors”.
* [x] Explainability overlays:
  - Overlay detected focus and candidate events as markers on the preview chart.
  - Add “show matched events only” toggle to isolate alignment.
* [x] Evidence interpretation helpers:
  - Add an “evidence composition” bar per candidate: percent from events vs co-occurrence.
  - Add “evidence coverage” metrics in preview:
    - `% focus events matched`
    - `% candidate events matched`
    - `% time buckets shared`
* [x] Jump-to-timestamp action:
  - From a candidate’s evidence (top buckets/episodes), let user set Trends window to a selected timestamp ±1h.
* [x] Triage guidance:
  - Add a compact “triage checklist” panel (Raw vs Normalized, units, missingness, directionality, system-wide).
* [x] Tests:
  - UI unit tests for new controls
  - Playwright stub coverage for jump-to-timestamp

## Acceptance Criteria
* [x] Simple mode produces higher precision@k by default (narrower scope, clear broadening path).
* [x] Operators can quickly navigate to a relevant timestamp in Trends with one click from the Related Sensors panel.
* [x] Operators can review system-wide buckets in a separate surface (not conflated with “related sensors”).
* [x] `make ci-web-smoke` passes.

## Notes
Primary UI file: `apps/dashboard-web/src/features/trends/components/RelationshipFinderPanel.tsx`.

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `make ci-web-smoke-build` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsWorkflowImprovements.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:3000 npm run test:playwright -- trends-related-sensors-jump.spec.ts` (pass)
