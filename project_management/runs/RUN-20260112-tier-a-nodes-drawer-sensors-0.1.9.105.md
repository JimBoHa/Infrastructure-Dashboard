# RUN-20260112-tier-a-nodes-drawer-sensors-0.1.9.105

## Goal
- Tier A validation (installed controller; no DB/settings reset): Nodes page action placement cleanup + Node drawer shows full sensor list (no truncation).

## Build
- `cd apps/dashboard-web && npm run build` (pass)

## Refresh installed controller (no reset)
- Built bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.105.dmg`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.105.dmg"}'`
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade` → `Upgraded to 0.1.9.105`
  - `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`

## Playwright screenshots (captured + viewed)
- Command:
  - `cd apps/dashboard-web && node scripts/web-screenshots.mjs --no-core --no-web --base-url=http://127.0.0.1:8000 --api-base=http://127.0.0.1:8000 --auth-token-file=/tmp/tier_a_api_token.txt`
- Output folder (viewed):
  - `manual_screenshots_web/20260112_130241/`
- Evidence screenshots (viewed):
  - Nodes page: `manual_screenshots_web/20260112_130241/nodes.png` (header actions removed; actions placed at bottom-right).
  - Node drawer open: `manual_screenshots_web/20260112_130241/nodes_drawer.png` (sensor list not truncated; includes search + scroll).

