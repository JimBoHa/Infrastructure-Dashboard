# RUN-20260120 — Tier A — DW-170 Analytics Overview battery voltage full width (0.1.9.180)

- **Date:** 2026-01-20
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.180
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.180.dmg`

## Scope

- Analytics Overview: move Battery voltage section to its own full-width row beneath Fleet status.

## Refresh installed controller (Upgrade)

```bash
# Build controller bundle DMG (local-path; no remote downloads)
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.180 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.180.dmg \
  --native-deps /usr/local/farm-dashboard/native

# Point setup-daemon at the new bundle DMG
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.180.dmg"}'

# Upgrade (refresh installed controller, no DB reset)
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.180` (previous `0.1.9.179`).

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
  --name playwright-screenshots-dw170 \
  --expires-in-days 7 \
  > /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token_dw170.txt

cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token_dw170.txt
```

Result: `PASS`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260120_013622/analytics.png` (Battery voltage full width below Fleet status)
