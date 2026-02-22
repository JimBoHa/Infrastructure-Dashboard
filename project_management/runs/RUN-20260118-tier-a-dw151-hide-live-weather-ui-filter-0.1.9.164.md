# RUN-20260118 — Tier A — DW-151 (0.1.9.164)

Tier A validation on the installed controller after:

- **DW-151:** Fix regression where the per-node “Hide live weather (Open‑Meteo)” toggle still showed Open‑Meteo sensors in Nodes/Sensors/Map listings.

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.164.dmg`
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.164 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.164.dmg --native-deps /usr/local/farm-dashboard/native`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.164.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz` (ok)
  - `make e2e-installed-health-smoke` (pass)
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.164`)

## Evidence (Tier A)

Screenshots captured + viewed:

- WS‑2902 node detail shows “Hide live weather” enabled and Open‑Meteo sensors filtered from the node sensor list:
  - `manual_screenshots_web/tier_a_0.1.9.164_dw151_hide_live_weather/ws2902_node_detail_hide_live_weather_filters_open_meteo.png`
- Sensors & Outputs (by node) shows WS‑2902 without Open‑Meteo sensors listed:
  - `manual_screenshots_web/tier_a_0.1.9.164_dw151_hide_live_weather/sensors_outputs_ws2902_open_meteo_hidden.png`

## Notes

- During `farmctl upgrade`, an `xattr` permission warning was logged while attempting to modify the bundle DMG, but the upgrade completed successfully and the installed controller reported `0.1.9.164`.

