# RUN-20260120 — Tier A — DW-165 Analytics IA (0.1.9.176)

- **Date:** 2026-01-20
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.176
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.176.dmg`

## Scope

- DW-165: Move **Trends** + **Power** under **Analytics** (Analytics Overview / Trends / Power), keep legacy entrypoints functional, and reorganize Analytics Overview into a clearer hierarchy without feature loss.
- Controller-local time (site time) for Analytics Overview / Trends / Power (via `/api/connection.timezone`).

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

# Build controller bundle DMG (local-path; no remote downloads)
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.176 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.176.dmg \
  --native-deps /usr/local/farm-dashboard/native

# Point setup-daemon at the new bundle DMG
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.176.dmg"}'

# Upgrade (refresh installed controller, no DB reset)
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.176` (previous `0.1.9.175`).

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## CI / Build

```bash
cargo build --manifest-path apps/core-server-rs/Cargo.toml
make ci-web-smoke
cd apps/dashboard-web && npm run build
```

Result: `PASS` (lint warnings exist, no errors).

## Quick functional checks (installed controller UI)

```bash
curl -fsS http://127.0.0.1:8000/api/connection
```

Result: response includes `timezone` (controller-local IANA TZ, e.g. `America/Los_Angeles`).

Manual verification (installed UI at `http://127.0.0.1:8000`):
- Sidebar shows a dedicated **Analytics** group with **Analytics Overview**, **Trends**, **Power**.
- `/analytics`, `/analytics/trends`, `/analytics/power` render and share the Analytics header/tab pattern.
- Legacy `/trends` and `/power` still land on the canonical routes.

## Tier-A screenshots (Playwright)

```bash
# Create a short-lived automation token in the installed controller DB
cargo run --manifest-path apps/core-server-rs/Cargo.toml --bin create_local_api_token -- \
  --name playwright-screenshots \
  --expires-in-days 7 \
  > /Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt

cd apps/dashboard-web
node scripts/web-screenshots.mjs \
  --no-web \
  --no-core \
  --base-url=http://127.0.0.1:8000 \
  --auth-token-file=/Users/Shared/FarmDashboardBuilds/playwright_screenshots_token.txt
```

Result: `PASS`.

Evidence (captured + viewed):
- `manual_screenshots_web/20260119_203247/analytics.png`
- `manual_screenshots_web/20260119_203247/trends.png`
- `manual_screenshots_web/20260119_203247/power.png`

