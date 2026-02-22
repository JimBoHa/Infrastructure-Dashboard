# TICKET-0037: Trends: show sensor origin badges + hide external toggle

**Status:** Done

## Description

The dashboard already marks non-local / public-API-backed sensors with origin badges (e.g. `WEATHER API`, `PV FCST`) in Sensors & Outputs. Trends needs the same transparency so operators can see, at selection time, whether a series is local telemetry vs external/public API data.

This ticket adds:
- Sensor origin badges in the Trends sensor picker (and selected chips).
- A toggle to hide external/public-API sensors from the picker to reduce clutter and prevent accidental selection.

Constraints:
- This is a UI filter only. It must not synthesize/alter any telemetry.
- External/public-API sensors are defined as sensors with `config.source="forecast_points"` (Open‑Meteo, Forecast.Solar, etc.).

## Scope
* [x] Show `SensorOriginBadge` next to sensors in Trends → Sensor picker UI.
* [x] Add a “Hide external sensors” toggle that filters out `config.source="forecast_points"` sensors from the picker list.
* [x] Persist the toggle locally (browser storage) so the user’s preference survives refreshes.
* [x] Validation: `make ci-web-smoke`.

## Acceptance Criteria
* [x] Trends sensor picker shows origin badges consistent with Sensors & Outputs (same labels/colors).
* [x] “Hide external sensors” hides all `forecast_points` sensors from the picker list (Weather API, PV forecast, etc.).
* [x] Toggle does not remove/modify underlying telemetry (no data writes).
* [x] Toggle preference persists locally across refreshes.
* [x] `make ci-web-smoke` passes.
* [x] Tier A (installed controller refresh; no DB/settings reset) + screenshot evidence is recorded when closing the ticket (defer clean-host E2E to `DW-98`).

## Notes

**Primary file target:**
- `apps/dashboard-web/src/app/(dashboard)/trends/TrendsPageClient.tsx`

## Evidence (Tier A)

- Run: `project_management/runs/RUN-20260118-tier-a-dw150-trends-origin-badges-0.1.9.163.md`
- Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.163_dw150_trends_origin_badges/trends_weather_api_badge.png`
- Screenshot (viewed): `manual_screenshots_web/tier_a_0.1.9.163_dw150_trends_origin_badges/trends_hide_external_enabled.png`
