# RUN-20260120 — Tier A — DW-177 Trends overlay limit (0.1.9.188)

- **Date:** 2026-01-20
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.188
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.188.dmg`

## Scope

- DW-177: Increase Trends overlay limit from 10 → 20 so operators can compare more sensors on one chart.

## Build controller bundle DMG

```bash
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.188 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.188.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: DMG created.

## Refresh installed controller (Upgrade)

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.188.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.188` (previous `0.1.9.187`).

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

Saved screenshots to `manual_screenshots_web/20260120_133427`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260120_133427/trends.png` (shows `0/20 selected` and copy “Pick up to 20 sensors…”)

