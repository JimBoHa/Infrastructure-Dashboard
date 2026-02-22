# RUN-20260111 — Tier A Phase 5 Operator Surfaces (0.1.9.75)

**Goal:** Production-smoke validate Phase 5 operator surfaces (Power/Analytics + core Emporia ingest plumbing) on the already-installed controller. **No DB/settings reset.**

## Upgrade (no downtime / no resets)

- Built controller bundle: `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.75.dmg`
- Refreshed installed controller via setup-daemon:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.75.dmg"}'`
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`

## Health

- `curl -fsS http://127.0.0.1:8800/healthz` → `{"status":"ok"}`
- `curl -fsS http://127.0.0.1:8000/healthz` → `{"status":"ok"}`
- Installed version: `/usr/local/farm-dashboard/state.json` → `current_version: "0.1.9.75"`

## Power/Analytics (Tier A)

- Analytics series bucket correctness:
  - `GET http://127.0.0.1:8000/api/analytics/power`
  - `series_24h` step: **300s**, `solar_series_24h` step: **60s** (Renogy solar/storage higher granularity).
- Feeds online:
  - `GET http://127.0.0.1:8000/api/analytics/feeds/status` shows `Emporia`, `Forecast.Solar`, `Open‑Meteo` all `ok` with fresh `last_seen`.
- Emporia electrical readbacks present in sensor inventory:
  - `GET http://127.0.0.1:8000/api/sensors` shows Emporia metrics:
    - `channel_power_w` (83), `channel_voltage_v` (71), `channel_current_a` (71)
    - `mains_power_w` (4), `mains_voltage_v` (4), `mains_current_a` (4)

## Notes

- UI-only UX items validated by manual spot-check in browser after upgrade (Nodes/Sensors refresh feedback and Provisioning presets list).

