# TICKET-0050: Tier A: TSSE-37 + DW-225 related sensors diurnal penalty + preview defaults (0.1.9.255)

**Status:** Done

## Description
Tier A rebuild + refresh of the installed controller (no DB/settings reset) to validate:

- **TSSE-37**: related sensors scoring down-ranks diurnal (~24h) lag artifacts and prevents weak `|r|` from inflating to near-1.0 “related” scores.
- **DW-225**: Related Sensors preview defaults select a representative episode (coverage → points → peak) and warns when an episode is sparse.

## Scope
* [x] Build controller bundle DMG from repo state.
* [x] Point setup-daemon at the new DMG.
* [x] Upgrade installed controller to the new version.
* [x] Run Tier A smoke and capture/view UI screenshots for Related Sensors.

## Acceptance Criteria
* [x] Setup daemon healthy (`GET /healthz`).
* [x] Core server healthy (`GET /healthz`).
* [x] Installed controller upgraded to `0.1.9.255-related-diurnal-penalty`.
* [x] `make e2e-installed-health-smoke` passes.
* [x] Related Sensors UI evidence captured and viewed.

## Notes
- **Date:** 2026-02-07
- **Installed version (after):** `0.1.9.255-related-diurnal-penalty`
- **Installed version (before):** `0.1.9.254-matrix-refresh-fix`
- **Bundle DMG:** `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.255-related-diurnal-penalty.dmg`
- **Bundle log:** `/Users/Shared/FarmDashboardBuilds/logs/bundle-0.1.9.255-related-diurnal-penalty.log`
- **UI evidence (captured + viewed):**
  - `manual_screenshots_web/20260207_000801/trends_related_sensors_scanning.png`
  - `manual_screenshots_web/20260207_000801/trends_related_sensors_large_scan.png`
