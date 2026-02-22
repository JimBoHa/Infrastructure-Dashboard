# RUN-20260116 — Tier A — DW-138 Overview Local Sensors Config (0.1.9.146)

Tier A validation on the installed controller after adding an Overview “Configure” button for local sensor visualizations. The config modal allows choosing which sensors appear and their priority order.

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.146.dmg`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.146.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health check:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.146`)

## Evidence (Tier A)

- Playwright screenshot run:
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.146 npx playwright test playwright/overview-local-visualizations.spec.ts --project=chromium-mobile`
- Screenshots captured + viewed:
  - `manual_screenshots_web/tier_a_0.1.9.146_overview_local_visuals_2026-01-16_070257452Z/01_overview_local_visuals.png`
  - `manual_screenshots_web/tier_a_0.1.9.146_overview_local_visuals_2026-01-16_070257452Z/02_overview_local_sensors_config.png`

## Notes

- The Overview “Local sensors” card now includes a discrete “Configure” button.
- Config changes persist in browser localStorage (`farmdashboard.overview.localSensors.v1`).
