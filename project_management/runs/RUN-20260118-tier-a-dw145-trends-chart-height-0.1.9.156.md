# RUN-20260118 — Tier A — DW-145 Trends chart height (0.1.9.156)

- **Date:** 2026-01-18
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.156
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.156.dmg`

## Scope

- DW-145: Allow operators to resize the Trends chart container height (persisted locally) so multi-series graphs are easier to inspect.

## Build bundle

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.156 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.156.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.156.dmg`.

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.156.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.156` (previous `0.1.9.155`).

Notes:
- `farmctl upgrade` emitted `xattr: [Errno 13] Permission denied` for the local DMG path, but the upgrade completed successfully.

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier-A screenshots (Playwright)

```bash
cd apps/dashboard-web

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.156 \
npx playwright test playwright/trends-chart-height-tier-a.spec.ts --project=chromium-mobile
```

Result: `PASS`.

Evidence (viewed):
- `manual_screenshots_web/tier_a_0.1.9.156_trends_chart_height_2026-01-18_093921214Z/01_trends_chart_height_resized.png`

