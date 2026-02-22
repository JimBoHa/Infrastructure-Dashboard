# RUN-20260110 Tier A Smoke (Installed Controller 0.1.9.69)

- **Context:** Tier A production-smoke validation after fixing installed Sensors CRUD crash (SQLx latest_value/latest_ts row mapping).
- **Host:** Installed controller (no DB/settings reset)
- **Bundle version:** `0.1.9.69` (`/usr/local/farm-dashboard/state.json`)

## Commands

- Health:
  - `curl -fsS http://127.0.0.1:8800/healthz`
  - `curl -fsS http://127.0.0.1:8000/healthz`
- UI smoke screenshots (Playwright runner):
  - `npm --prefix apps/dashboard-web run screenshots:web -- --no-core --no-web --api-base=http://127.0.0.1:8000 --base-url=http://127.0.0.1:8000 --auth-token-file=/tmp/farmdashboard_auth_token --browser=chromium --out-dir=manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417`
- Mobile regression suite (Playwright):
  - `cd apps/dashboard-web && FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 FARM_PLAYWRIGHT_SAVE_SCREENSHOTS=1 FARM_PLAYWRIGHT_SCREENSHOT_DIR=manual_screenshots_web/playwright_regressions_0.1.9.69_20260110_1755 npm run test:playwright -- --project=chromium-mobile --project=webkit-mobile`
- Backups exports (download-only):
  - `token=$(tr -d '\n' < /tmp/farmdashboard_auth_token)`
  - `curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:8000/api/backups/app-settings/export`
  - `curl -s -o /dev/null -w "%{http_code}\n" -H "Authorization: Bearer $token" http://127.0.0.1:8000/api/backups/app-settings/export`
  - `curl -s -o /dev/null -w "%{http_code}\n" "http://127.0.0.1:8000/api/backups/database/export?format=raw&scope=full"`
  - `curl -s -o /dev/null -w "%{http_code}\n" -H "Authorization: Bearer $token" "http://127.0.0.1:8000/api/backups/database/export?format=raw&scope=full"`

## Results

- Health: setup-daemon + core-server `ok`.
- Playwright screenshots: saved.
- Playwright mobile tests: `36 passed`.
- Backups exports: both endpoints return `401` without auth and `200` with bearer auth.
- Sensors CRUD: `PUT /api/sensors/{id}` returned `200` and `DELETE /api/sensors/{id}?keep_data=true` returned `204`.

## Artifacts

- UI smoke screenshots: `apps/dashboard-web/manual_screenshots_web/tier_a_smoke_0.1.9.69_20260110_175417/`
- Mobile regression screenshots: `apps/dashboard-web/manual_screenshots_web/playwright_regressions_0.1.9.69_20260110_1755/`
