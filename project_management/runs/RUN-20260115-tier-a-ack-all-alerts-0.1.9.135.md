# RUN-20260115-tier-a-ack-all-alerts-0.1.9.135

- **Date (UTC):** 2026-01-15
- **Tier:** A (installed controller; no DB/settings reset)
- **Controller version:** 0.1.9.135
- **Purpose:** Validate “Acknowledge all alerts” buttons on dashboard alarm-event surfaces.

## Preconditions
- Installed controller stack running.
- Auth token present at `/tmp/fd_codex_api_token.txt`.

## Build + Upgrade
1. Build controller bundle:
   - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.135 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.135.dmg --native-deps build/native-deps`
2. Upgrade installed controller:
   - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.135.dmg"}'`
   - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
3. Confirm installed version:
   - `curl -fsS http://127.0.0.1:8800/api/status | jq -r '.logs[0].stdout' | jq '{current_version, previous_version}'`

## Validation Steps (UI)
Run Playwright to verify:
- Sensors & Outputs page shows “Acknowledge all alerts” and it bulk-acks at least one `status="firing"` event (and does not ack `status="ok"`).
- Nodes page also shows “Acknowledge all alerts”.

- `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.135 npx playwright test playwright/acknowledge-all-alerts.spec.ts --project=chromium-mobile`

## Evidence
- **Screenshots (viewed):** `manual_screenshots_web/tier_a_0.1.9.135_ack_all_alerts_2026-01-15_100129420Z/`
  - `01_sensors_alarm_events.png`
  - `02_nodes_alarm_events.png`

## Result
- **PASS**

