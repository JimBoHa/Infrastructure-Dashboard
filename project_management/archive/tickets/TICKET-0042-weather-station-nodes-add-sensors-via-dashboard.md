# TICKET-0042: Weather station nodes: add sensors via dashboard

**Status:** Done

## Description
Operators are now adding additional sensors to an existing WS‑2902 weather station node (e.g., a soil moisture probe attached to the station/controller). The dashboard currently has no clear, user-friendly way to add new sensors to an already-adopted weather station node, which forces ad‑hoc config edits and creates drift.

We need a **simple-by-default** flow to add a new sensor to a weather station node from the web dashboard, while still exposing an **Advanced** path for power users to fully specify metric/type/unit/ingest mapping.

This work must also include a Tier‑A validation that confirms the newly-added sensor is receiving data and that the data is visible in the dashboard UI.

## Scope
* [x] Add a WS‑2902 node-specific “Add sensor” entrypoint (Nodes + Sensors & Outputs surfaces as applicable).
* [x] Provide presets for common weather-station‑adjacent sensors (at minimum: soil moisture), with sane defaults.
* [x] Provide an Advanced section that allows explicit configuration (metric/type/unit/source/topic/fields as applicable).
* [x] Ensure the new sensor shows up across the dashboard consistently (Nodes, Sensors & Outputs, Trends, Analytics Overview where applicable).
* [x] Validate end‑to‑end data flow for a newly-added sensor in Tier A (no DB reset).

## Acceptance Criteria
* [x] A user can add a new sensor to an existing WS‑2902 node via the dashboard without manual config file edits.
* [x] The flow is simple (preset-first) but supports Advanced configuration for power users.
* [x] The created sensor is visible in:
  * [x] Nodes (node detail shows the sensor)
  * [x] Sensors & Outputs (listed under the node)
  * [x] Trends (selectable and chart renders points)
  * [x] Analytics Overview (Weather stations section includes the sensor if the section is expanded or a “Custom sensors” subsection exists)
* [x] Tier‑A evidence: captured + viewed screenshots showing the new sensor and at least one non-null datapoint in the UI (run log in `project_management/runs/`).
* [x] `make ci-web-smoke` and `cd apps/dashboard-web && npm run build` pass.

## Notes
If the underlying ingest path requires node-agent changes or additional WS‑2902 upload fields, split the work into:
1) Dashboard + controller config wiring, and
2) Hardware/field validation (blocked until the sensor is present and posting).

Tier A evidence:
- Run: `project_management/runs/RUN-20260121-tier-a-dw179-weather-station-add-sensors-0.1.9.194.md`
- Screenshots (captured + viewed):
  - `manual_screenshots_web/20260121_142426/sensors_add_sensor_ws2902.png`
  - `manual_screenshots_web/20260121_142426/sensors_ws2902_custom.png`
