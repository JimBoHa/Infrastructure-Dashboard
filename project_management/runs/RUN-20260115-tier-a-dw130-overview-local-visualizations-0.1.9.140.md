# RUN-20260115-tier-a-dw130-overview-local-visualizations-0.1.9.140

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.140
- **Purpose:** Validate new Overview “Local sensors” visualization panels render and are populated from locally acquired telemetry.

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Build + Upgrade
1. Build controller bundle:
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.140 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.140.dmg --native-deps /usr/local/farm-dashboard/native`
2. Upgrade installed controller:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.140.dmg"}'`
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
3. Confirm installed version:
   - `curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'`

## Validation Steps (UI)
Run Playwright to load `/overview`, confirm the “Local sensors” section is visible, and capture a full-page screenshot:

- `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_TIER_A_VERSION=0.1.9.140 npx playwright test playwright/overview-local-visualizations.spec.ts --project=chromium-mobile`

## Evidence
- **Screenshot (captured + viewed):**
  - `manual_screenshots_web/tier_a_0.1.9.140_overview_local_visuals_2026-01-15_204501236Z/01_overview_local_visuals.png`

## Result
- **PASS**

