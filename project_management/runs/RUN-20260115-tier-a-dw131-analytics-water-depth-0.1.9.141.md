# RUN-20260115-tier-a-dw131-analytics-water-depth-0.1.9.141

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.141
- **Purpose:** Validate Analytics no longer charts reservoir depth as gallons, and the new depth chart + live gauges render correctly.

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Build + Upgrade
1. Build controller bundle:
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.141 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.141.dmg --native-deps /usr/local/farm-dashboard/native`
2. Upgrade installed controller:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.141.dmg"}'`
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
3. Confirm installed version:
   - `curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'`

## Validation Steps (UI)
Run Playwright to load `/analytics`, confirm the new depth surfaces are present, and capture a full-page screenshot:

- `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_TIER_A_VERSION=0.1.9.141 npx playwright test playwright/analytics-water-depth.spec.ts --project=chromium-mobile`

## Evidence
- **Screenshot (captured + viewed):**
  - `manual_screenshots_web/tier_a_0.1.9.141_analytics_water_depth_2026-01-15_210340433Z/01_analytics_water_depth.png`

## Result
- **PASS**

