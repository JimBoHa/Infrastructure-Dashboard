# RUN-20260117 — Tier A — CS-84 (0.1.9.151)

Tier A validation on the installed controller after:

- **CS-84:** WS‑2902 barometric pressure integrity: do **not** backfill “local weather station” telemetry from Open‑Meteo (or any public API), and make pressure reference explicit (relative vs absolute).

## Upgrade / refresh (installed controller)

- Built bundle:
  - `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.151.dmg`
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.151 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.151.dmg --native-deps /usr/local/farm-dashboard/native`
- Set setup-daemon bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.151.dmg"}'`
- Upgrade:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz` (ok)
  - `curl -fsS http://127.0.0.1:8800/api/status` (reports `current_version: 0.1.9.151`)

## Evidence (Tier A)

Screenshots captured + viewed:

- Weather station node detail (WS‑2902) shows WS-local sensor origin badges and pressure is no longer “magic backfilled”:
  - `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/ws2902_node_detail.png`
- Sensors & Outputs filtered to the weather station node shows two explicit pressure sensors:
  - `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/ws2902_sensors_tab.png`

Additional screenshot sweep artifacts (captured):

- `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/nodes.png`
- `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/sensors.png`
- `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/trends.png`
- `manual_screenshots_web/tier_a_0.1.9.151_cs84_ws_pressure_integrity/map.png`

## Data integrity cleanup (installed controller DB)

The legacy WS “Barometric pressure” sensor (old `type=pressure`) had contaminated rows written by the earlier CS‑83 Open‑Meteo backfill approach. These were removed from the installed controller DB so the historical graph is not misleading.

## Notes

- The WS‑2902 station is currently uploading pressure as `baromin=-9999` (missing). The controller now reflects this truth: the new `pressure_relative` / `pressure_absolute` sensors may remain empty until the station is configured to report valid barometric pressure.
- Open‑Meteo “Weather pressure (kPa)” is still available as a separate sensor (`config.source=forecast_points`) and is clearly labeled as non-local. It is never mixed into WS-local sensor series.
