# Prod Audit Follow-up (Analytics + Alarms) — 2026-01-05 06:06

Branch: `fix/installer-admin-launcher`

Goal: finish remaining independent-audit items (analytics power/storage population + alarm panel clarity), ship to the running production controller, and verify live non-zero Renogy-derived analytics values.

## Timeline / Command Log

### 06:06 — Implemented changes (repo)
- Added derived Renogy battery power (“Storage”) to `/api/analytics/power` (live kW, kWh 24h/168h, time series).
- Updated Analytics UI summary card to include live Storage kW.
- Updated Sensor detail drawer to show alarm state (active/ok) and last-fired timestamp.

### Pending
- Build new controller bundle DMG (expected `0.1.8.7`) and upgrade via `farmctl upgrade`.
- Verify `/api/analytics/power` returns non-empty `battery_series_*` + non-zero `live_battery_kw`, and charts populate in the Analytics tab.

## Execution Notes / Issues

### 06:11 — Bundle built
- `FarmDashboardController-0.1.8.7.dmg` created at `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.8.7.dmg`.

### 06:12 — Upgrade attempt (direct farmctl) failed
- Command: `/usr/local/farm-dashboard/bin/farmctl upgrade --bundle /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.8.7.dmg --config /Users/Shared/FarmDashboard/setup/config.json`
- Error: `Permission denied (os error 13)` writing `/Users/Shared/FarmDashboard/setup/config.json` (owned by `_farmdashboard`).
- Fix: trigger upgrade via the setup-daemon API (runs as `_farmdashboard`).

### 06:12 — Upgrade succeeded (setup-daemon API)
- Updated bundle path: `POST http://127.0.0.1:8800/api/config` (HTTP 200)
- Triggered upgrade: `POST http://127.0.0.1:8800/api/upgrade`
- Result: `stdout: Upgraded to 0.1.8.7`
- Non-fatal stderr (known): `xattr: [Errno 13] Permission denied` attempting to mutate DMG xattrs.

### 06:15 — Verification (live data + DB persistence)
- `/api/sensors` shows Renogy values updating (examples at 14:15 UTC):
  - `battery_voltage_v=12.2V`, `battery_soc_percent=50%`, `battery_temp_c=13C`, `controller_temp_c=16C`
  - `pv_*`, `load_*`, and `battery_current_a` were `0.0` at this snapshot (expected overnight/idle).
- `/api/analytics/power` now returns populated storage series:
  - `battery_series_24h.len=25`, `battery_series_168h.len=8`
- `/api/metrics/query` for `battery_voltage_v` returned `25` points over the last 2h (`interval=300s`), confirming history persisted in DB.
