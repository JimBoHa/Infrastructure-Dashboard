# RUN-20260112-tier-a-dashboard-tab-audit-0.1.9.103

## Goal
- Tier A validation (installed controller; no DB/settings reset): cross-tab UI/UX/IA consistency audit with screenshots captured **and viewed**.

## Build
- `cd apps/dashboard-web && npm run build` (pass)

## Refresh installed controller (no reset)
- Built bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.103.dmg`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.103.dmg"}'`
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade` → `Upgraded to 0.1.9.103`
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`

## Playwright screenshots (captured + viewed)
- Command:
  - `cd apps/dashboard-web && node scripts/web-screenshots.mjs --no-core --no-web --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/tmp/tier_a_api_token.txt`
- Output folder (viewed):
  - `manual_screenshots_web/20260112_121326/`
- Tabs verified via viewed screenshots:
  - `manual_screenshots_web/20260112_121326/root.png`
  - `manual_screenshots_web/20260112_121326/nodes.png`
  - `manual_screenshots_web/20260112_121326/map.png`
  - `manual_screenshots_web/20260112_121326/sensors.png`
  - `manual_screenshots_web/20260112_121326/schedules.png`
  - `manual_screenshots_web/20260112_121326/trends.png`
  - `manual_screenshots_web/20260112_121326/power.png`
  - `manual_screenshots_web/20260112_121326/analytics.png`
  - `manual_screenshots_web/20260112_121326/backups.png`
  - `manual_screenshots_web/20260112_121326/setup.png`
  - `manual_screenshots_web/20260112_121326/deployment.png`
  - `manual_screenshots_web/20260112_121326/connection.png`
  - `manual_screenshots_web/20260112_121326/users.png`

