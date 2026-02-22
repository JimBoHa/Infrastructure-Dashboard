# RUN-20260202 Tier A — DW-211 (0.1.9.239-temp-compensation)

## Context

- **Date:** 2026-02-02
- **Task:** DW-211 — Analytics: Assisted temperature drift compensation (wizard + derived sensor output)
- **Goal:** Rebuild + refresh the installed controller (Tier A; no DB/settings reset) and capture evidence for the new dashboard route.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.238-analytics-zoom` (from `http://127.0.0.1:8800/api/status`)

## Build (controller bundle DMG)

- **Version:** `0.1.9.239-temp-compensation`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.239-temp-compensation.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.239-temp-compensation.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.239-temp-compensation \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.239-temp-compensation.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.239-temp-compensation.log
```

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.239-temp-compensation.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Post-upgrade installed version: `0.1.9.239-temp-compensation` (from `http://127.0.0.1:8800/api/status`)

## Validation

- Installed smoke: `make e2e-installed-health-smoke` → **PASS**

## Evidence

- Screenshots: `manual_screenshots_web/tier_a_0.1.9.239_dw211_temp_compensation_20260202_014033/`
  - `analytics_compensation.png` (page load: `/analytics/compensation`)
  - `analytics_compensation_selected.png` (sensors selected + fit summary visible)
  - `analytics_compensation_preview.png` (charts rendered)
  - `analytics_compensation_created.png` (created derived compensated sensor banner)

## Notes

- During Tier A capture, the “Create compensated sensor” flow was exercised on real controller data and the derived sensor was removed afterwards (so the installed controller state remains tidy).
