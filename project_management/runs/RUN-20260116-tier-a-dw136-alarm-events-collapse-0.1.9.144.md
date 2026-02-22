# RUN-20260116 Tier A — DW-136 Alarm Events Collapse (0.1.9.144)

Tier A validation on the installed controller after changing Alarm Events UX so **acknowledged/cleared** events are hidden by default and moved into a collapsed “Acknowledged & cleared” section.

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.144.dmg`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.144.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health check:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.144`)

## Evidence (Tier A)

- Playwright screenshot run:
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.144 npx playwright test playwright/acknowledge-all-alerts.spec.ts --project=chromium-mobile`
- Screenshots captured + viewed:
  - `manual_screenshots_web/tier_a_0.1.9.144_ack_all_alerts_2026-01-16_060129657Z/01_sensors_alarm_events.png`
  - `manual_screenshots_web/tier_a_0.1.9.144_ack_all_alerts_2026-01-16_060129657Z/02_nodes_alarm_events.png`

## Notes

- On both `/sensors` and `/nodes`, the Alarm Events panel now shows **active** events in the default list and keeps acknowledged/cleared events under the collapsed section.
- Acknowledging an event moves it out of the default list after the query refresh (no manual page refresh required).
