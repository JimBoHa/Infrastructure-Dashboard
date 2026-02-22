# RUN-20260115-tier-a-dw129-map-viewport-fill-0.1.9.139

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.139
- **Purpose:** Validate Map tab layout fills viewport height (map + right panels reach bottom; no “3/4 height” gap).

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Build + Upgrade
1. Build controller bundle:
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.139 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.139.dmg --native-deps /usr/local/farm-dashboard/native`
2. Upgrade installed controller:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.139.dmg"}'`
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
3. Confirm installed version:
   - `curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'`

## Validation Steps (UI)
Run Playwright to assert the map canvas + right-side panels extend to the bottom of the viewport and capture a screenshot:

- `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_TIER_A_VERSION=0.1.9.139 npx playwright test playwright/map-viewport-fill.spec.ts --project=chromium-mobile`

## Evidence
- **Screenshot (captured + viewed):**
  - `manual_screenshots_web/tier_a_0.1.9.139_map_viewport_fill_2026-01-15_202340332Z/01_map_viewport_fill.png`

## Result
- **PASS**

