# RUN-20260115-tier-a-hide-live-weather-0.1.9.134

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.134
- **Purpose:** Validate per-node toggle to hide live weather (Open‑Meteo) sensors from the dashboard UI while continuing ingestion/storage.

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Build + Upgrade
1. Build controller bundle:
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.134 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.134.dmg --native-deps build/native-deps`
2. Upgrade installed controller:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.134.dmg"}'`
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
3. Confirm installed version:
   - `curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'`

## Validation Steps (UI)
Run Playwright to toggle the setting and verify sensors/panels disappear and reappear:

- `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.134 npx playwright test playwright/hide-live-weather-toggle.spec.ts --project=chromium-mobile`

Assertions covered by the spec:
- Node detail page shows a `config.write`-gated checkbox “Hide live weather”.
- Turning it on hides:
  - Live weather panel on the node detail page.
  - Open‑Meteo weather sensors for that node from the node sensor list, Sensors & Outputs table, and Map sensor listings.
- Turning it off restores those sensors/panels.

## Evidence
- **Screenshots (viewed):** `manual_screenshots_web/tier_a_0.1.9.134_hide_live_weather_2026-01-15_093830083Z/`
  - `01_before_toggle.png`
  - `02_after_toggle_node_detail.png`
  - `03_sensors_table_hidden.png`
  - `04_map_hidden.png`
  - `05_toggle_off_restored.png`

## Result
- **PASS**

