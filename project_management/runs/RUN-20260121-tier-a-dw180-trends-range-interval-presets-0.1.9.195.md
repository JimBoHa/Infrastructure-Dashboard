# RUN-20260121 — Tier A — DW-180 Trends range + interval presets (0.1.9.195)

- **Date:** 2026-01-21
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.195
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.195.dmg`

## Scope

- DW-180: Improve Trends “Range” and “Interval” presets for short windows and higher-resolution buckets:
  - Add **Last 10 minutes** and **Last hour**; remove **Last 180 days**.
  - Add **1s** and **30s**; remove **15 min** and **2 hours**.
  - Allow custom interval minimum of **1s** (no 10s clamp).

## Refresh installed controller (Upgrade)

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.195.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.195`.

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

Saved screenshots to `manual_screenshots_web/20260121_161614`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260121_161614/trends_short_range.png` (Range set to **Last 10 minutes**, Interval set to **1s**)
- `manual_screenshots_web/20260121_161614/trends.png` (Range/Interval presets list visible in Chart settings)

