# TICKET-0066: Trends: Pattern Detector integration with Related Sensors

**Status:** Done

## Description
Related Sensors and the Pattern & Anomaly Detector both identify “interesting windows” in the same time-series, but they are currently separate workflows.

This ticket integrates them so operators can use detected anomalies/motifs as the focus event set for Related Sensors (instead of only delta-z events), and so the UI can show when the same windows are driving both tools.

## Scope
* [x] Add a “Send focus events to Related Sensors” action from Pattern Detector results.
* [x] Extend Related Sensors job params to accept an optional explicit focus-event list (timestamps + optional severity).
* [x] Update evidence summary to indicate whether ranking was driven by:
  - delta-z events
  - pattern detector anomalies
  - or a blend
* [x] UI: show shared anomaly windows across both panels (simple visual indicator).
* [x] Tests:
  - job accepts explicit focus events and produces deterministic results

## Acceptance Criteria
* [x] Operators can move from Pattern Detector → Related Sensors in one click without manual timestamp hunting.
* [x] Related Sensors makes it clear which evidence source is being used.
* [x] `make ci-web-smoke` and relevant Rust tests pass.

## Notes
Primary UI surfaces:
- `apps/dashboard-web/src/features/trends/components/MatrixProfilePanel.tsx`
- `apps/dashboard-web/src/features/trends/components/RelationshipFinderPanel.tsx`
