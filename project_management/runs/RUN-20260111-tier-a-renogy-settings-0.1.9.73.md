# RUN-20260111-tier-a-renogy-settings-0.1.9.73

- **Context:** Ship Renogy “controller settings over Modbus” scaffolding (core orchestration + node-agent apply endpoint + dashboard UI) and validate Tier‑A upgrade stability on the installed controller (no DB/settings reset).
- **Host:** Installed controller (Tier A; no DB/settings reset)
- **Bundle:** `FarmDashboardController-0.1.9.73.dmg`

## Commands

- Build controller bundle from source (re-using installed native deps):
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.73 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.73.dmg --native-deps /usr/local/farm-dashboard/native`
- Point setup-daemon at the new stable bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.73.dmg"}'`
- Upgrade/refresh the installed controller:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8800/healthz`
  - `curl -fsS http://127.0.0.1:8000/healthz`

## Result

- **Pass:** Installed controller upgraded and `:8000/healthz` returned 200 after restart.
- **Note:** `/api/nodes/*/renogy-bt2/settings/*` endpoints require auth (`config.write`) and are exercised via the dashboard UI; hardware apply validation is tracked separately (NA-61).

