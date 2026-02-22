# RUN-20260121 — Tier A — DW-179 Weather station add sensors via dashboard (0.1.9.194)

- **Date:** 2026-01-21
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.194
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.194.dmg`

## Scope

- DW-179: Provide a user-friendly way to add **custom sensors** to an existing **WS‑2902 weather station node** via the dashboard, with an Advanced configuration path.
- Validate end-to-end data flow: newly added sensor shows up in the UI and has non-null datapoints.

## Refresh installed controller (Upgrade)

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.194.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.194`.

## Web build + smoke

```bash
make ci-web-smoke
cd apps/dashboard-web && npm run build
```

Result: `PASS` (logs saved under `reports/`).

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS` (logs saved under `reports/`).

## Tier-A screenshots (Playwright)

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Saved screenshots to `manual_screenshots_web/20260121_142426`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260121_142426/sensors_add_sensor_ws2902.png` (WS‑2902 “Add sensor” drawer with Upload field)
- `manual_screenshots_web/20260121_142426/sensors_ws2902_custom.png` (new WS‑2902 custom sensor shows non-null datapoints in Trend preview)
- `manual_screenshots_web/20260121_142426/analytics.png` (Analytics Overview renders)

