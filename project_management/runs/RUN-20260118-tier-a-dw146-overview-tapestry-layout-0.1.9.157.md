# RUN-20260118 — Tier A — DW-146 Overview telemetry tapestry layout (0.1.9.157)

- **Date:** 2026-01-18
- **Tier:** A (installed controller refresh; **no DB/settings reset**)
- **Controller version:** 0.1.9.157
- **Controller bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.157.dmg`

## Scope

- DW-146: Fix the Overview “Telemetry tapestry” panel so hovering heatmap cells never causes layout shift and the card never produces internal horizontal scrolling at normal desktop widths. Add deterministic Playwright coverage to prevent regressions.

## Build bundle

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.157 \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.157.dmg \
  --native-deps /usr/local/farm-dashboard/native
```

Result: `Bundle created at /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.157.dmg`.

## Refresh installed controller (Upgrade)

```bash
curl -fsS http://127.0.0.1:8800/healthz
curl -fsS http://127.0.0.1:8000/healthz

curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.157.dmg"}'

curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
curl -fsS http://127.0.0.1:8800/api/status
```

Result: `current_version: 0.1.9.157` (previous `0.1.9.156`).

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
FARM_TIER_A_VERSION=0.1.9.157 \
npx playwright test playwright/overview-tapestry-layout-tier-a.spec.ts --project=chromium-mobile
```

Result: `PASS`.

Evidence (viewed):
- `manual_screenshots_web/tier_a_0.1.9.157_overview_tapestry_layout_2026-01-18_104201563Z/02_overview_tapestry_hover.png`

