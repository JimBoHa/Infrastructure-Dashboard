# RUN-20260120 — Tier A — DW-171 Weather forecast layout tighten (0.1.9.181)

- **Date:** 2026-01-20
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.181
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.181.dmg`

## Scope

- Analytics Overview: Weather forecast panel layout uses full width when only one plot is available (no blank half-column).

## Refresh installed controller (Upgrade)

```bash
# Build controller bundle DMG (local-path; no remote downloads)
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.181 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.181.dmg \
  --native-deps /usr/local/farm-dashboard/native

# Point setup-daemon at the new bundle DMG
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.181.dmg"}'

# Upgrade (refresh installed controller, no DB reset)
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.181` (previous `0.1.9.180`).

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
  --name playwright-screenshots-dw171 \
  --expires-in-days 7 \
  > /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token_dw171.txt

cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token_dw171.txt
```

Result: `PASS`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260120_021758/analytics.png` (Weather forecast layout tightened; no empty half-column)
