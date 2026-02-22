# RUN-20260202 Tier A — DW-213 (0.1.9.240-highcharts-zooming-fix)

## Context

- **Date:** 2026-02-02
- **Task:** DW-213 — Fix mobile WebKit crash when Highcharts zoom is disabled
- **Goal:** Rebuild + refresh the installed controller (Tier A; no DB/settings reset) and confirm Analytics pages load on WebKit mobile without `Application error`.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.239-temp-compensation` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)

## Root Cause

On coarse-pointer devices (iOS/WebKit), we disable chart zoom/pan to allow browser pinch-zoom-out. Some Highcharts option builders were passing `chart.zooming: undefined`, which overwrote Highcharts defaults and caused:

- `TypeError: undefined is not an object (evaluating 'e.type')`

This occurred during Highcharts init in `setZoomOptions()` and crashed `/analytics` and `/analytics/compensation` on mobile.

## Build (controller bundle DMG)

- **Version:** `0.1.9.240-highcharts-zooming-fix`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.240-highcharts-zooming-fix.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.240-highcharts-zooming-fix.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.240-highcharts-zooming-fix \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.240-highcharts-zooming-fix.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.240-highcharts-zooming-fix.log
```

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.240-highcharts-zooming-fix.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Post-upgrade installed version: `0.1.9.240-highcharts-zooming-fix` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)

## Validation

- Installed smoke: `make e2e-installed-health-smoke` → **PASS**
- WebKit mobile smoke (Playwright iPhone 13 WebKit emulation):
  - `/analytics` loads (no `Application error`)
  - `/analytics/compensation` loads and selecting sensors does not crash (no `Application error`)

## Evidence

- WebKit screenshots (captured + viewed):
  - `manual_screenshots_web/tier_a_0.1.9.240_dw213_highcharts_zooming_fix_20260202_195802Z/analytics_overview_webkit.png`
  - `manual_screenshots_web/tier_a_0.1.9.240_dw213_highcharts_zooming_fix_20260202_195802Z/analytics_compensation_selected_webkit.png`
  - `manual_screenshots_web/tier_a_0.1.9.240_dw213_highcharts_zooming_fix_20260202_195802Z/analytics_compensation_charts_webkit.png`

