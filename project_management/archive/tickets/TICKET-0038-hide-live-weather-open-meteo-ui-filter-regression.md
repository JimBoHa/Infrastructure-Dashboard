# TICKET-0038: Hide live weather (Open‑Meteo) UI filter regression

**Status:** Done

## Description

The per-node “Hide live weather (Open‑Meteo)” toggle is intended to hide public-API-backed weather sensors in the dashboard UI while continuing to store the underlying data.

A regression causes Open‑Meteo sensors to remain visible in some sensor lists even when the toggle is enabled.

This ticket fixes the regression by applying a shared UI visibility filter for Open‑Meteo weather sensors (defined as `sensor.config.source="forecast_points"`, `provider="open_meteo"`, `kind="weather"`) anywhere sensors are listed for selection/inspection on that node.

Constraints:
- UI filter only (no telemetry synthesis; no writes to sensor time-series data).
- Must not hide non-weather forecast sensors (e.g., PV forecasts).

## Scope

* [x] Implement a shared helper to determine whether a sensor should be hidden due to node config.
* [x] Apply the helper to:
  - Nodes → node detail sensor list
  - Sensors & Outputs → grouped-by-node and table views
  - Map → Devices list/search results
* [x] Validation: `make ci-web-smoke`.

## Acceptance Criteria

* [x] Enabling “Hide live weather (Open‑Meteo)” hides Open‑Meteo weather sensors from:
  - Nodes → node detail sensor list
  - Sensors & Outputs → table and grouped-by-node views
  - Map → Devices list/search results (and any sensor markers that rely on the sensors list)
* [x] Disabling the toggle restores those sensors/panels.
* [x] Telemetry ingest/storage remains unchanged (UI filter only).
* [x] `make ci-web-smoke` passes.
* [x] Tier A (installed controller refresh; no DB/settings reset) + screenshot evidence is recorded when closing the ticket (defer clean-host E2E to `DW-98`).

## File Targets

- `apps/dashboard-web/src/lib/sensorVisibility.ts`
- `apps/dashboard-web/src/app/(dashboard)/nodes/detail/NodeDetailPageClient.tsx`
- `apps/dashboard-web/src/app/(dashboard)/sensors/SensorsPageClient.tsx`
- `apps/dashboard-web/src/features/sensors/components/NodeIoPanels.tsx`
- `apps/dashboard-web/src/app/(dashboard)/map/MapPageClient.tsx`

## Evidence (Tier A)

- Run: `project_management/runs/RUN-20260118-tier-a-dw151-hide-live-weather-ui-filter-0.1.9.164.md`
- Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.164_dw151_hide_live_weather/ws2902_node_detail_hide_live_weather_filters_open_meteo.png`
- Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.164_dw151_hide_live_weather/sensors_outputs_ws2902_open_meteo_hidden.png`
