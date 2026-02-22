# RUN-20260119 — Tier A DW-158 “Overlap (n)” + in-context Trends keys (installed controller `0.1.9.174`)

## Goal
- Tier A validation (installed controller; **no DB/settings reset**): Verify Trends’ variable wording is clear for non-technical operators by using **Overlap (n)** terminology (instead of “Buckets (n)”), and confirm the per-panel “Key” popovers remain adjacent to the analysis UI they describe (no scroll-hunting).

## Code
- `git rev-parse HEAD` → `792e0b4`

## Installed controller version
- `/usr/local/farm-dashboard/state.json` → `current_version: "0.1.9.174"` (previous: `0.1.9.173`)
- Setup bundle path (setup-daemon config): `/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.174.dmg`

## Tests (pre-upgrade)
- `make ci-web-smoke` (pass; warnings only)
- `cd apps/dashboard-web && npm run build` (pass)

## Upgrade / refresh (Tier A)
- Built controller bundle DMG:
  - `cargo run --manifest-path apps/farmctl/Cargo.toml -- bundle --version 0.1.9.174 --output /Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.174.dmg --native-deps /usr/local/farm-dashboard/native`
- Pointed setup daemon at stable bundle path:
  - `curl -fsS -X POST http://127.0.0.1:8800/api/config -H 'Content-Type: application/json' -d '{"bundle_path":"/Users/Shared/FarmDashboardBuilds/FarmDashboardController-0.1.9.174.dmg"}'`
- Upgraded via setup daemon (no admin; launchd KeepAlive restarts services):
  - `curl -fsS -X POST http://127.0.0.1:8800/api/upgrade`
- Health checks:
  - `curl -fsS http://127.0.0.1:8000/healthz`
  - `curl -fsS http://127.0.0.1:8800/healthz`

## Evidence (screenshots)
- Sweep dir:
  - `manual_screenshots_web/tier_a_0.1.9.174_dw158_overlap_keys_20260119T195026Z`
- Viewed:
  - `manual_screenshots_web/tier_a_0.1.9.174_dw158_overlap_keys_20260119T195026Z/trends.png`
  - `manual_screenshots_web/tier_a_0.1.9.174_dw158_overlap_keys_20260119T195026Z/trends_related_sensors_key_open.png`
  - `manual_screenshots_web/tier_a_0.1.9.174_dw158_overlap_keys_20260119T195026Z/trends_relationships_key_open.png`

## Notes
- Some Related-sensors scans can hit the API guardrail (`Requested series too large … max 25000`) when Range/Interval combinations imply too many buckets. The UI guidance is to increase Interval; this run did not change the guardrail behavior.
- Tier B clean-host validation remains tracked via the existing web cluster ticket(s) (e.g., `DW-98` / `DW-114`).

