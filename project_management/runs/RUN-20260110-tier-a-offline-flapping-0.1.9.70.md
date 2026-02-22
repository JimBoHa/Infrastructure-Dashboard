# RUN-20260110-tier-a-offline-flapping-0.1.9.70

- **Context:** Fix telemetry-sidecar online/offline flapping when sensor update cadences exceed the global sidecar offline threshold (default 5s), and ensure COV sensors are not marked offline due to “no change”.
- **Host:** Installed controller (Tier A; no DB/settings reset)
- **Bundle:** `FarmDashboardController-0.1.9.70.dmg`

## Commands

- Build controller bundle from source (re-using installed native deps):
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.70 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.70.dmg --native-deps /usr/local/farm-dashboard/native`
- Point setup-daemon at the new stable bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.70.dmg"}'`
- Upgrade/refresh the installed controller:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8800/healthz`
  - `curl -fsS http://127.0.0.1:8000/healthz`
- Offline flapping check (DB query; URL/password from `/Users/Shared/FarmDashboard/setup/config.json`):
  - `.../psql postgresql://postgres:<redacted>@127.0.0.1:5432/iot -c "SELECT ... WHERE created_at > now() - interval '30 minutes' ..."`

## Result

- **Pass:** Installed controller upgraded `0.1.9.69 → 0.1.9.70`; `healthz` OK; no `Node Offline`/`Sensor Offline` alarm events were generated in the 30 minutes after upgrade (no startup flapping).
- **Note:** This DB currently has no `interval_seconds=0` sensors to exercise the “COV quiet” path; clean-host Tier‑B validation is tracked separately.

## Artifacts

- None (DB query + version bump evidence above).

