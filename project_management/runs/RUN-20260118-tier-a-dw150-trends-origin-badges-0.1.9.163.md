# RUN-20260118 — Tier A — DW-150 (0.1.9.163)

Tier A validation on the installed controller after:

- **DW-150:** Trends shows sensor origin badges (e.g., `WEATHER API`, `PV FCST`, `WS LOCAL`, `DERIVED`) and includes a “Hide external sensors” toggle to filter public-API-backed series from the picker.

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.163.dmg`
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.163 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.163.dmg --native-deps /usr/local/farm-dashboard/native`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.163.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz` (ok)
  - `make e2e-installed-health-smoke` (pass)
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.163`)

## Evidence (Tier A)

Screenshots captured + viewed:

- Trends shows origin badges (and “Hide external sensors” toggle):
  - `manual_screenshots_web/tier_a_0.1.9.163_dw150_trends_origin_badges/trends_weather_api_badge.png`
- Trends with “Hide external sensors” enabled (Weather API sensors filtered from picker):
  - `manual_screenshots_web/tier_a_0.1.9.163_dw150_trends_origin_badges/trends_hide_external_enabled.png`

## Notes

- During `farmctl upgrade`, an `xattr` permission warning was logged while attempting to modify the bundle DMG, but the upgrade completed successfully and the installed controller reported `0.1.9.163`.

