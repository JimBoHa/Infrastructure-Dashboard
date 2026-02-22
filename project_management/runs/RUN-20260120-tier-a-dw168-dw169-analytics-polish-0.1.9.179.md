# RUN-20260120 — Tier A — DW-168/DW-169 Analytics polish + reorder modal (0.1.9.179)

- **Date:** 2026-01-20
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.179
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.179.dmg`

## Scope

- DW-168:
  - Weather-history charts: render Solar/UV and Pressure as full-size cards (avoid stacked mini-charts).
  - For 24-hour Analytics Overview charts, use time-only x-axis ticks (avoid full dates on each tick).
  - Forecasts layout: stack Weather stations and Weather forecast vertically (full width).
- DW-169:
  - Fix Display order (Reorder) modal so it never extends outside the viewport; content scrolls inside the dialog.

## Refresh installed controller (Upgrade)

```bash
# Build controller bundle DMG (local-path; no remote downloads)
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.179 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.179.dmg \
  --native-deps /usr/local/farm-dashboard/native

# Point setup-daemon at the new bundle DMG
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.179.dmg"}'

# Upgrade (refresh installed controller, no DB reset)
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.179` (previous `0.1.9.178`).

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## CI / Build

```bash
make ci-web-smoke
cd apps/dashboard-web && npm run build
```

Result: `PASS` (lint warnings exist, no errors).

## Tier-A screenshots (Playwright)

```bash
# Create a short-lived automation token in the installed controller DB
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin create_local_api_token -- \
  --name playwright-screenshots-dw168-dw169 \
  --expires-in-days 7 \
  > /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token_dw168_dw169.txt

cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token_dw168_dw169.txt
```

Result: `PASS`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260120_011123/analytics.png` (Forecasts stacked; 24h ticks time-only; Solar/UV + Pressure charts render as full-size cards)
- `manual_screenshots_web/20260120_011123/nodes_reorder_modal.png` (modal fits viewport)
- `manual_screenshots_web/20260120_011123/sensors_reorder_modal.png` (modal fits viewport)
