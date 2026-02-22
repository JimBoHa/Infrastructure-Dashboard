# RUN-20260115-tier-a-non-local-sensor-badges-0.1.9.133

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.133
- **Purpose:** Validate that non-local sensors (forecast/API-backed) are visually differentiated via badges.

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Validation Steps
1. Upgrade installed controller to `0.1.9.133` via setup daemon.
2. Run Playwright scenario to assert a forecast/API-backed sensor (identified via `config.source=forecast_points`) shows a non-local badge across:
   - Sensors & Outputs (table + sensor drawer)
   - Node detail sensor list
   - Map tab node sensor list

## Commands
- Upgrade:
  - `POST http://127.0.0.1:8800/api/config` (bundle `FarmDashboardController-0.1.9.133.dmg`)
  - `POST http://127.0.0.1:8800/api/upgrade`
- Build (local):
  - `cd apps/dashboard-web && npm run build`
- Playwright:
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.133 npx playwright test playwright/non-local-sensor-badges.spec.ts --project=chromium-mobile`

## Evidence
- **Screenshots (viewed):** `manual_screenshots_web/tier_a_0.1.9.133_non_local_sensor_badges_2026-01-15_090855735Z/`
  - `01_sensors_drawer_badge.png`
  - `02_sensors_table_badge.png`
  - `03_node_detail_sensor_badge.png`
  - `04_map_sensor_badge.png`

## Result
- **PASS**
