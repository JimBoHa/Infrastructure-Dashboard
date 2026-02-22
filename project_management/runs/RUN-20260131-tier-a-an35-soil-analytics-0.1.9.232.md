# RUN-20260131 — Tier A — AN-35 Analytics Overview soil moisture aggregation (0.1.9.232)

- **Date:** 2026-01-31
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.232
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.232.dmg`

## Scope

- Fix Analytics Overview “Soil moisture” chart flatlining at `0%` by replacing the stubbed `GET /api/analytics/soil` response with real aggregation over `sensors` + `metrics`.
- Ensure the dashboard renders real soil moisture data (avg/min/max lines) consistent with Trends.

## Preflight (installed stack health)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
```

Result: both `{"status":"ok"}`.

## Local CI / Smoke (before Tier A)

```bash
make ci-core-smoke
make ci-web-smoke
```

Result: `PASS` for both.

## Rebuild + refresh installed controller (Tier A)

```bash
python3 tools/rebuild_refresh_installed_controller.py --version 0.1.9.232
```

Confirm version:

```bash
curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'
```

Result: `current_version: 0.1.9.232` (previous `0.1.9.231`).

## Verify soil analytics endpoint is non-zero

```bash
curl -fsS http://127.0.0.1:8000/api/analytics/soil
```

Result: `series` contains non-zero values (not an all-zero placeholder).

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier-A screenshots (Playwright)

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Saved to: `manual_screenshots_web/20260131_162607/`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260131_162607/analytics.png` (Soil moisture chart is non-zero; min/max/avg lines render)

