# RUN-20260118 — Tier A — DW-148 Trends auto-compare related sensor suggestions (0.1.9.160)

- **Date:** 2026-01-18
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.160
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.160.dmg`

## Scope

- DW-148: Add a polished Trends “Related sensors” panel that suggests sensors with similar/inverse patterns (and optional lead/lag) relative to a focus sensor, with transparent scoring metadata and one-click add-to-chart + preview.

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.160.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.160` (previous `0.1.9.159`).

Notes:
- `farmctl upgrade` emitted `xattr: [Errno 13] Permission denied` for the local DMG path, but the upgrade completed successfully.

## Installed stack health smoke

```bash
make e2e-installed-health-smoke
```

Result: `PASS`.

## CI / Smoke

```bash
make ci-web-smoke
```

Result: `PASS` (lint warnings exist, no errors).

## Tier-A screenshots (Playwright)

```bash
cd apps/dashboard-web

FARM_PLAYWRIGHT_AUTH_TOKEN=$(cat /tmp/fd_codex_api_token.txt) \
FARM_PLAYWRIGHT_BASE_URL=http://127.0.0.1:8000 \
FARM_TIER_A_VERSION=0.1.9.160 \
npx playwright test playwright/trends-auto-compare-tier-a.spec.ts --project=chromium-mobile
```

Result: `PASS`.

Evidence (viewed):
- `manual_screenshots_web/tier_a_0.1.9.160_trends_auto_compare_2026-01-18_140746175Z/01_trends_auto_compare_panel.png`

