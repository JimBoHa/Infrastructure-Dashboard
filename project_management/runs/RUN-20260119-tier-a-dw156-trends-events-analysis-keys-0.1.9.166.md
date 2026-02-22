# RUN-20260119 — Tier A — DW-156 Trends event matching + per-panel analysis keys (0.1.9.166)

- **Date:** 2026-01-19
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.166
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.166.dmg`

## Scope

- DW-156: Extend Trends analysis beyond continuous correlation by adding an event/spike comparison mode (co-occurrence + lag), with an optional conditioning sensor (binned “approx equals constant” filtering). Add per-panel analysis keys that define variables (`n`, `r`, lag, score, window) and expose opt-in “deep” computations.

## Confirm installed controller version

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.166` (previous `0.1.9.165`).

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
FARM_TIER_A_VERSION=0.1.9.166 \
npx playwright test playwright/trends-auto-compare-tier-a.spec.ts --project=chromium-mobile --workers=1

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_TIER_A_VERSION=0.1.9.166 \
npx playwright test playwright/trends-matrix-profile-tier-a.spec.ts --project=chromium-mobile --workers=1
```

Result: `PASS`.

Evidence (viewed):
- `manual_screenshots_web/tier_a_0.1.9.166_trends_auto_compare_2026-01-19_040012388Z/01_trends_auto_compare_events_key.png`
- `manual_screenshots_web/tier_a_0.1.9.166_trends_matrix_profile_2026-01-19_040038832Z/01_trends_matrix_profile.png`

