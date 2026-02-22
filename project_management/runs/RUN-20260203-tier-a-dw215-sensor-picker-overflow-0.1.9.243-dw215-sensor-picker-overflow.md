# RUN-20260203 Tier A — DW-215 (0.1.9.243-dw215-sensor-picker-overflow)

## Context

- **Date:** 2026-02-03
- **Task:** **DW-215** — Trends: Sensor picker must not overflow its container
- **Goal:** Rebuild + refresh the installed controller (Tier A; no DB/settings reset) and confirm the Trends “Sensor picker” sidebar no longer paints outside its card/container border.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.242-dw210-related-selection` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)
- Rollback target (previous bundle):
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.242-dw210-related-selection.dmg`

## Build (controller bundle DMG)

- **Version:** `0.1.9.243-dw215-sensor-picker-overflow`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.243-dw215-sensor-picker-overflow.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.243-dw215-sensor-picker-overflow.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.243-dw215-sensor-picker-overflow \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.243-dw215-sensor-picker-overflow.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.243-dw215-sensor-picker-overflow.log
```

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.243-dw215-sensor-picker-overflow.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Post-upgrade installed version: `0.1.9.243-dw215-sensor-picker-overflow` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)

## Validation

- Installed smoke: `make e2e-installed-health-smoke` → **PASS**

## Evidence

- Screenshot sweep (captured + viewed):
  - Folder: `manual_screenshots_web/tier_a_0.1.9.243_dw215_sensor_picker_overflow_20260203_071210Z/`
  - `manual_screenshots_web/tier_a_0.1.9.243_dw215_sensor_picker_overflow_20260203_071210Z/trends.png`

## Notes

- Root cause: CollapsibleCard’s Radix Collapsible content wrapper did not force a shrinkable grid item (`min-width:auto` behavior), allowing content to paint outside a constrained sidebar column in some layouts.
- Fix: set the wrapper grid to `grid-cols-1` and add `min-w-0` on the immediate content wrapper so CollapsibleCard bodies can shrink within constrained layouts (without changing the intended sidebar width).

