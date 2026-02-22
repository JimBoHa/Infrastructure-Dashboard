# RUN-20260203 Tier A — DW-210 (0.1.9.242-dw210-related-selection)

## Context

- **Date:** 2026-02-03
- **Task:** **DW-210** — Trends: Related Sensors results selection must not reset
- **Goal:** Rebuild + refresh the installed controller (Tier A; no DB/settings reset) and confirm Trends → Related Sensors selection stays stable during background polling refreshes.

## Preconditions (installed stack)

- Setup daemon health: `curl -fsS http://127.0.0.1:8800/healthz` → **ok**
- Core server health: `curl -fsS http://127.0.0.1:8000/healthz` → **ok**
- Pre-upgrade installed version: `0.1.9.241-analytics-mobile-window` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)
- Rollback target (previous bundle):
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.241-analytics-mobile-window.dmg`

## Build (controller bundle DMG)

- **Version:** `0.1.9.242-dw210-related-selection`
- **Bundle path:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.242-dw210-related-selection.dmg`
- **Build log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.242-dw210-related-selection.log`

Command:

```bash
mkdir -p /Users/Shared/FarmDashboardBuilds /Users/Shared/FarmDashboardBuilds/logs

cargo run --manifest-path apps/farmctl/Cargo.toml -- \
  bundle \
  --version 0.1.9.242-dw210-related-selection \
  --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.242-dw210-related-selection.dmg \
  --native-deps /usr/local/farm-dashboard/native \
  |& tee /Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.242-dw210-related-selection.log
```

## Refresh (upgrade installed controller)

Set bundle path:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/config \
  -H 'Content-Type: application/json' \
  -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.242-dw210-related-selection.dmg"}'
```

Upgrade:

```bash
curl -fsS -X POST http://127.0.0.1:8800/api/upgrade
```

Post-upgrade installed version: `0.1.9.242-dw210-related-selection` (from `http://127.0.0.1:8800/api/status` → `farmctl status`)

## Validation

- Installed smoke: `make e2e-installed-health-smoke` → **PASS**

## Evidence

- Trends → Related Sensors selection stability (Playwright automated check; captured screenshots):
  - Folder: `manual_screenshots_web/tier_a_0.1.9.242_dw210_related_selection_20260203_020057Z/`
  - `manual_screenshots_web/tier_a_0.1.9.242_dw210_related_selection_20260203_020057Z/related_sensors_selected_before_wait.png`
  - `manual_screenshots_web/tier_a_0.1.9.242_dw210_related_selection_20260203_020057Z/related_sensors_selected_after_wait.png`

## Notes

- This validation specifically targets the regression where the Related Sensors “Rank” list selection jumps back to rank 1 after a few seconds (background polling refresh). The Playwright run selects a non-first row, waits, and confirms the selection remains unchanged.

