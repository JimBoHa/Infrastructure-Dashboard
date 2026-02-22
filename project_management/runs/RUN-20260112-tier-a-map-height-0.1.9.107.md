# RUN-20260112-tier-a-map-height-0.1.9.107

## Goal
- Tier A validation (installed controller; no DB/settings reset): Map canvas height is constrained so a standard desktop viewport can see the full map and the right-side controls without excessive scrolling.

## Build
- `cd apps/dashboard-web && npm run build` (pass)

## Refresh installed controller (no reset)
- Built bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.107.dmg`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.107.dmg"}'`
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade` → `Upgraded to 0.1.9.107`
  - `curl -fsS http://127.0.0.1:8800/api/status | rg '"current_version"'` → `0.1.9.107`

## Playwright screenshots (captured + viewed)
- Command:
  - `cd apps/dashboard-web && node scripts/web-screenshots.mjs --no-core --no-web --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/tmp/tier_a_api_token.txt`
- Output folder (viewed):
  - `manual_screenshots_web/20260112_133625/`
- Evidence screenshot (viewed):
  - `manual_screenshots_web/20260112_133625/map.png` (map no longer consumes ~2x height; right rail visible without excessive scroll).

