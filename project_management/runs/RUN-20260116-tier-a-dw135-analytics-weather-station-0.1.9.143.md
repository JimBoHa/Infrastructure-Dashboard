# RUN-20260116 Tier A — DW-135 Analytics Weather Station (0.1.9.143)

Tier A validation on the installed controller after adding the Analytics **Weather stations** panel (WS-2902), including rich charts and a wind direction compass.

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.143.dmg`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.143.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health check:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.143`)

## Evidence (Tier A)

- Playwright screenshot run:
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.143 npx playwright test playwright/analytics-weather-station-tier-a.spec.ts --project=chromium-mobile`
- Screenshot captured + viewed:
  - `manual_screenshots_web/tier_a_0.1.9.143_analytics_weather_station_2026-01-16_053903585Z/01_analytics_weather_station.png`

## Notes

- If no WS-2902 station nodes exist yet on the controller, the Weather stations panel shows an explicit empty state.
- Expanding a station triggers history fetches (no background metric polling while collapsed).
