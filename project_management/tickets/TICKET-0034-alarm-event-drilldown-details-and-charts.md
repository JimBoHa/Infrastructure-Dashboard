# TICKET-0034 — Alarm Events: drilldown details + context charts

**Status:** Open

## Description

Operators need to understand *why* an alarm fired and what the underlying telemetry looked like at the time. Today, the dashboard surfaces Alarm Events as a flat list, but there is no click-through detail view or contextual charting.

This ticket adds a user-friendly “drilldown” experience: click an Alarm Event to open a detail drawer showing structured metadata (what/when/where), linked targets (sensor/node), and a context chart (when sensor-backed) with transparent rule data (no hidden/obscured values).

## Scope
* [ ] Make Alarm Event cards clickable across all surfaces that render Alarm Events (shared panel).
* [ ] Add an Alarm Event detail drawer with:
  - event metadata (id, status, raised time, origin/anomaly)
  - linked target (sensor/node)
  - context chart for sensor-backed alarms (range presets + optional threshold lines when rule data is available)
  - “Raw event” + “Raw alarm rule” expandable JSON sections for transparency
* [ ] Add deterministic Playwright coverage for open + chart render.

## Acceptance Criteria
* [ ] Clicking an Alarm Event opens a detail drawer with structured information (not only the message string).
* [ ] For sensor-backed alarms, the drawer renders a Trend chart for the alarm’s target sensor over a selectable context window.
* [ ] When a threshold/min/max can be derived from the alarm rule, it is shown explicitly (and graphed where feasible).
* [ ] “Acknowledge” actions still work and do not accidentally trigger the drilldown click action.
* [ ] Deterministic Playwright test covers: open drawer + chart visible, using stubbed API responses.
* [ ] `make ci-web-smoke` passes.
* [ ] Tier A validation (installed controller refresh; no DB/settings reset) includes a screenshot captured + viewed and a run log under `project_management/runs/`.

## Notes

**File targets (expected):**
- UI:
  - `apps/dashboard-web/src/features/sensors/components/AlarmEventsPanel.tsx`
  - `apps/dashboard-web/src/features/sensors/components/AlarmEventDetailDrawer.tsx` (new)
- API typing:
  - `apps/dashboard-web/src/lib/apiSchemas.ts` (alarm event fields like `sensor_id`/`node_id`)
- Tests:
  - `apps/dashboard-web/playwright/alarm-event-drilldown.spec.ts` (new)
- Tracking:
  - `project_management/TASKS.md`, `project_management/BOARD.md`, `project_management/EPICS.md`
