# RUN-20260116 — Tier A — DW-137 Analytics Water Depth Gauge Scale (0.1.9.145)

Tier A validation on the installed controller after changing Analytics “Live reservoir depths” gauges to default to a **15 ft** full-scale view (instead of 10 ft) so typical reservoir depths do not appear saturated.

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.145.dmg`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.145.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health check:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.145`)

## Evidence (Tier A)

- Playwright screenshot run:
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_TIER_A_VERSION=0.1.9.145 npx playwright test playwright/analytics-water-depth.spec.ts --project=chromium-mobile`
- Screenshot captured + viewed:
  - `manual_screenshots_web/tier_a_0.1.9.145_analytics_water_depth_2026-01-16_062105261Z/01_analytics_water_depth.png`

## Notes

- The “Live reservoir depths” gauge now shows a full-scale label of **0 → 15.0 ft**.
