# RUN-20260120 — Tier A — DW-176 Trends Savitzky–Golay smoothing (0.1.9.187)

- **Date:** 2026-01-20
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.187
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.187.dmg`

## Scope

- DW-176: Add an optional Savitzky–Golay (SG) filter (smoothing + derivatives) to Trends → Trend chart:
  - Toggle + advanced settings (window length, polynomial degree, derivative order, edge mode, derivative units).
  - Visualization-only (does not modify stored telemetry); preserves missing-data gaps.

## Build controller bundle DMG

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.187 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.187.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: DMG created.

## Refresh installed controller (Upgrade)

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.187.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.187` (previous `0.1.9.186`).

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier-A screenshots (Playwright)

```bash
cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Saved screenshots to `manual_screenshots_web/20260120_120159`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260120_120159/trends.png` (Trends page renders; SG toggle + Advanced section visible)
- `manual_screenshots_web/20260120_120159/trends_savgol_advanced.png` (SG enabled + Advanced settings expanded with a series selected)

