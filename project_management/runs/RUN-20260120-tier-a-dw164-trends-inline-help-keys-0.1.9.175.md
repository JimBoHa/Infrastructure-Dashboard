# RUN-20260120 — Tier A — DW-164 Trends inline help keys (0.1.9.175)

- **Date:** 2026-01-20
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.175
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.175.dmg`

## Scope

- DW-164: Replace Trends “AnalysisKey” popovers with inline bottom “help keys” that always show a short overview and expand/collapse for details per container.

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

# Build controller bundle DMG (local-path; no remote downloads)
cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle \
  --version 0.1.9.175 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.175.dmg \
  --native-deps /usr/local/farm-dashboard/native

# Point setup-daemon at the new bundle DMG
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.175.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.175` (previous `0.1.9.174`).

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
cd apps/dashboard-web

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.175 \
npx playwright test playwright/trends-keys-tier-a.spec.ts --project=chromium-mobile

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.175 \
npx playwright test playwright/trends-auto-compare-tier-a.spec.ts --project=chromium-mobile

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.175 \
npx playwright test playwright/trends-relationships-tier-a.spec.ts --project=chromium-mobile

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.175 \
npx playwright test playwright/trends-matrix-profile-tier-a.spec.ts --project=chromium-mobile

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.175 \
npx playwright test playwright/trends-chart-height-tier-a.spec.ts --project=chromium-mobile
```

Result: `PASS`.

Evidence (captured + viewed):
- `manual_screenshots_web/tier_a_0.1.9.175_trends_keys_2026-01-20_002031118Z/01_trends_sensor_picker_key.png`
- `manual_screenshots_web/tier_a_0.1.9.175_trends_keys_2026-01-20_002031118Z/02_trends_trend_chart_key.png`
- `manual_screenshots_web/tier_a_0.1.9.175_trends_auto_compare_2026-01-20_002126208Z/01_trends_auto_compare_events_key.png`
- `manual_screenshots_web/tier_a_0.1.9.175_trends_relationships_2026-01-20_002136758Z/01_trends_relationships_key.png`
- `manual_screenshots_web/tier_a_0.1.9.175_trends_matrix_profile_2026-01-20_002352329Z/01_trends_matrix_profile_key.png`

