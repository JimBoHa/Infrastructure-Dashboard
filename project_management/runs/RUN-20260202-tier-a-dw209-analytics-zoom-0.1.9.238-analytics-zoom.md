# RUN-20260202 — Tier A — DW-209 Analytics Overview mobile zoom-out (0.1.9.238-analytics-zoom)

- **Date:** 2026-02-02
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.238-analytics-zoom
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.238-analytics-zoom.dmg`
- **Rollback bundle (last-known-stable):** `/Users/Shared/FarmDashboardBuildsDirty/FarmDashboardController-0.1.9.237-trends-keys.dmg`

## Scope

- DW-209: Analytics Overview mobile UX improvements:
  - Keep Forecast/Power cards behaving correctly on mobile without narrowing charts.
  - Migrate the 24h/72h/7d range selector to shadcn/ui styling.
  - Allow mobile pinch-zoom out on `/analytics` without triggering desktop breakpoints (left nav should remain in mobile mode).

## Preconditions (installed controller)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz
```

Repo hard-gate (clean worktree):

```bash
cd /Users/FarmDashboard/farm_dashboard
git status --porcelain=v1 -b
```

Installed version before:

```bash
curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'
```

Result: current `0.1.9.237-trends-keys` (previous `0.1.9.236-trends-height`).

## Build controller bundle DMG

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.238-analytics-zoom \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.238-analytics-zoom.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.238-analytics-zoom.log
```

Result: `PASS` (DMG created).

## Refresh installed controller (Upgrade)

Point setup-daemon at the new bundle:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.238-analytics-zoom.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Installed version after:

```bash
curl -fsS http://127.0.0.1:8800/api/status | rg 'current_version|previous_version'
```

Result: current `0.1.9.238-analytics-zoom` (previous `0.1.9.237-trends-keys`).

## Installed smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## Tier‑A screenshots (captured)

```bash
node apps/dashboard-web/scripts/web-screenshots.mjs \
  --no-web --no-core \
  --base-url=http://127.0.0.1:8000 \
  --api-base=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Evidence directory:
- `manual_screenshots_web/tier_a_0.1.9.238_dw209_analytics_zoom_20260202_005131`

Notes:
- The screenshot script emitted warnings for some Trends flows (panel locators timing out). Screenshots were still captured for `/analytics` and the other standard pages.
- Mobile pinch-zoom behavior requires manual verification on a real mobile browser.

