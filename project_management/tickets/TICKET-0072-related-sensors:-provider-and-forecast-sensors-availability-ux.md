# TICKET-0072: Related Sensors: provider and forecast sensors availability UX

**Status:** Done

## Description
Forecast/provider sensors (`config.source == "forecast_points"`) are selectable in Trends but generally have **no stored history in the analysis lake**. Related Sensors jobs silently skip them as outputs, which makes them appear to “never be related” with no explanation.

This ticket fixes the UX by hiding provider sensors from the candidate pool by default and clearly labeling them when included.

## Scope
* [x] Default behavior (Simple mode):
  - Provider/forecast sensors are excluded from candidate pool by default.
  - UI copy indicates these sensors have no stored history for relationship analysis.
* [x] Advanced mode:
  - Add toggle “Include provider/forecast sensors (may have no history)”.
  - When enabled, provider sensors that can’t be evaluated are labeled:
    - `Not available for relationship analysis (no stored history).`
* [x] Backend:
  - Stop silently skipping provider sensors in a way that looks like “0 evidence”; instead return explicit “skipped” diagnostics for these candidates when they were included in the candidate list.
* [x] Tests:
  - UI default excludes providers
  - “include providers” mode shows the correct not-available labels

## Acceptance Criteria
* [x] Operators are not confused by provider sensors that “never show up”; the UI explains availability explicitly.
* [x] No silent skip behavior remains without a surfaced reason.
* [x] `make ci-web-smoke` passes.

## Notes
Current default `excludePublicProvider` is `false` in `apps/dashboard-web/src/features/trends/types/relationshipFinder.ts`; this ticket changes the default and adds explicit explainability.

## Validation
- 2026-02-10: `make ci-core-smoke` (pass)
- 2026-02-10: `make ci-web-smoke` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm test -- --run tests/relatedSensorsProviderAvailability.test.tsx` (pass)
- 2026-02-10: `cd apps/dashboard-web && npm run build` (pass)
